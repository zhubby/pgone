use eframe::egui::{self, CentralPanel, Context, SidePanel, TopBottomPanel, ScrollArea, TextEdit};
use egui_extras::{TableBuilder, StripBuilder, Size};
use sqlx::{Row, Column};
use sqlx::postgres::{PgRow, PgPool, PgPoolOptions};
use chrono::{Utc, DateTime};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use eframe::egui::widgets::Image as EguiImage;
// removed unused imports
use std::collections::HashMap;

mod models; use models::*;
mod markdown; use markdown::render_markdown;
mod sql; use sql::{highlight_sql, format_cell};
mod media; use media::MediaCache;
mod openai_client; use openai_client::chat_once;

#[derive(Debug, Clone)]
struct PreviewState { path: PathBuf, zoom: f32 }

#[derive(Debug)]
enum AppMsg {
    SqlResult { columns: Vec<String>, rows: Vec<Vec<String>> },
    SqlError(String),
    Connected { session_id: u64, pool: Result<PgPool, String> },
    OpenAIResult { session_id: u64, result: Result<String, String> },
}

pub struct IntelliGuiApp {
    input: String,
    state: PersistedState,
    media: MediaCache,
    show_settings: bool,
    renaming_index: Option<usize>,
    rename_buffer: String,
    preview: Option<PreviewState>,
    // no md cache when using pulldown_cmark
    sql_input: String,
    sql_error: Option<String>,
    query_columns: Vec<String>,
    query_rows: Vec<Vec<String>>,
    pools: HashMap<u64, PgPool>,
    rt: tokio::runtime::Runtime,
}

impl IntelliGuiApp {
    const SESSIONS_PATH: &'static str = "sessions.json";

    fn new() -> Self {
        let state = Self::load_state().unwrap_or_else(|| PersistedState {
            sessions: vec![Session { id: 1, title: "New Session".to_string(), messages: Vec::new(), db: DbConfig { engine: "postgres".to_string(), dsn: String::new() } }],
            current_index: 0,
            next_session_id: 2,
            settings: Settings::default(),
        });
        Self {
            input: String::new(),
            state,
            media: Default::default(),
            show_settings: false,
            renaming_index: None,
            rename_buffer: String::new(),
            preview: None,
            sql_input: String::new(),
            sql_error: None,
            query_columns: vec![],
            query_rows: vec![],
            pools: HashMap::new(),
            rt: tokio::runtime::Runtime::new().expect("tokio runtime"),
        }
    }

    fn load_state() -> Option<PersistedState> {
        if !Path::new(Self::SESSIONS_PATH).exists() {
            return None;
        }
        let data = fs::read_to_string(Self::SESSIONS_PATH).ok()?;
        // Try new format first
        if let Ok(state) = serde_json::from_str::<PersistedState>(&data) {
            return Some(state);
        }
        // Fallback to legacy format (content: String)
        #[derive(Deserialize)]
        struct LegacyMessage { role: Role, timestamp: DateTime<Utc>, content: String }
        #[derive(Deserialize)]
        struct LegacySession { id: u64, title: String, messages: Vec<LegacyMessage> }
        #[derive(Deserialize)]
        struct LegacyState { sessions: Vec<LegacySession>, current_index: usize, next_session_id: u64 }
        if let Ok(old) = serde_json::from_str::<LegacyState>(&data) {
            let sessions = old.sessions.into_iter().map(|s| Session {
                id: s.id,
                title: s.title,
                messages: s.messages.into_iter().map(|m| Message {
                    role: m.role,
                    timestamp: m.timestamp,
                    content: MessageContent::Markdown(m.content),
                }).collect(),
                db: DbConfig::default(),
            }).collect();
            return Some(PersistedState { sessions, current_index: old.current_index, next_session_id: old.next_session_id, settings: Settings::default() });
        }
        None
    }

    fn save_state(&self) {
        let _ = fs::write(Self::SESSIONS_PATH, serde_json::to_string_pretty(&self.state).unwrap_or_default());
    }
}

impl eframe::App for IntelliGuiApp {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        TopBottomPanel::top("menu_top").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Add Image...").clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("Images", &[
                            "png", "jpg", "jpeg", "gif", "bmp", "webp"
                        ]).pick_file() {
                            self.add_image_message(path);
                        }
                        ui.close();
                    }
                    if ui.button("Add Video...").clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("Videos", &[
                            "mp4", "mov", "m4v", "mkv", "webm"
                        ]).pick_file() {
                            self.add_video_message(path);
                        }
                        ui.close();
                    }
                });
                ui.menu_button("View", |ui| {
                    if ui.button("Clear Current Session").clicked() {
                        self.clear_current_session();
                        ui.close();
                    }
                });
                ui.menu_button("Settings", |ui| {
                    if ui.button("Open Settings").clicked() {
                        self.show_settings = true;
                        ui.close();
                    }
                });
                ui.menu_button("Help", |_| {});
            });
        });

        // Settings window
        if self.show_settings {
            let mut open = true;
            egui::Window::new("Settings").open(&mut open).show(ctx, |ui| {
                ui.heading("Appearance");
                let mut dark = self.state.settings.dark_theme;
                if ui.checkbox(&mut dark, "Dark theme").clicked() {
                    self.state.settings.dark_theme = dark;
                    if dark { ctx.set_visuals(egui::Visuals::dark()); } else { ctx.set_visuals(egui::Visuals::light()); }
                    self.save_state();
                }
                ui.separator();
                ui.heading("Send Shortcut");
                let mut sc = self.state.settings.send_shortcut;
                if ui.radio_value(&mut sc, SendShortcut::Enter, "Enter").clicked() {
                    self.state.settings.send_shortcut = sc; self.save_state();
                }
                if ui.radio_value(&mut sc, SendShortcut::CmdEnter, "Cmd+Enter").clicked() {
                    self.state.settings.send_shortcut = sc; self.save_state();
                }
                ui.separator();
                ui.heading("OpenAI");
                let mut key = self.state.settings.openai_api_key.clone().unwrap_or_default();
                if ui.add(egui::TextEdit::singleline(&mut key).hint_text("API Key")).changed() {
                    if key.trim().is_empty() { self.state.settings.openai_api_key = None; } else { self.state.settings.openai_api_key = Some(key.clone()); }
                    self.save_state();
                }
                ui.horizontal(|ui| {
                    ui.label("Model");
                    let changed = ui.text_edit_singleline(&mut self.state.settings.openai_model).changed();
                    if changed { self.save_state(); }
                });
            });
            if !open { self.show_settings = false; }
        }

        // 左栏：会话 + DB 配置
        SidePanel::left("session_panel").resizable(true).min_width(220.0).show(ctx, |ui| {
            ui.heading("Sessions");
            ui.separator();
            if ui.button("+ New Session").clicked() {
                let id = self.state.next_session_id;
                self.state.next_session_id += 1;
                self.state.sessions.push(Session { id, title: format!("Session {}", id), messages: Vec::new(), db: DbConfig { engine: "postgres".to_string(), dsn: String::new() } });
                self.state.current_index = self.state.sessions.len() - 1;
                self.save_state();
            }
            ui.separator();

            let items: Vec<(usize, String)> = self.state.sessions.iter().enumerate()
                .map(|(i, s)| (i, s.title.clone())).collect();
            for (idx, title) in items {
                ui.horizontal(|ui| {
                    let selected = idx == self.state.current_index;
                    if ui.selectable_label(selected, &title).clicked() {
                        self.state.current_index = idx;
                        self.save_state();
                    }
                    if ui.small_button("Rename").clicked() {
                        self.renaming_index = Some(idx);
                        self.rename_buffer = title.clone();
                    }
                    if ui.small_button("Delete").clicked() {
                        self.delete_session(idx);
                    }
                });
                if self.renaming_index == Some(idx) {
                    ui.horizontal(|ui| {
                        let resp = ui.add(egui::TextEdit::singleline(&mut self.rename_buffer));
                        let press_enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if ui.button("Save").clicked() || (resp.lost_focus() && press_enter) {
                            if let Some(s) = self.state.sessions.get_mut(idx) { s.title = self.rename_buffer.trim().to_string(); }
                            self.renaming_index = None;
                            self.save_state();
                        }
                        if ui.button("Cancel").clicked() { self.renaming_index = None; }
                    });
                }
            }

            ui.separator();
            ui.heading("DB Config");
            if let Some(sess) = self.state.sessions.get_mut(self.state.current_index) {
                ui.label("Engine");
                ui.text_edit_singleline(&mut sess.db.engine);
                ui.label("DSN");
                let changed = ui.text_edit_singleline(&mut sess.db.dsn).changed();
                if changed { self.save_state(); }
                let sid = self.state.sessions[self.state.current_index].id;
                let connected = self.pools.contains_key(&sid);
                ui.horizontal(|ui| {
                    if !connected {
                        if ui.button("Connect").clicked() { self.connect_current_session(); }
                    } else {
                        ui.colored_label(egui::Color32::GREEN, "Connected");
                        if ui.button("Disconnect").clicked() { self.disconnect_current_session(); }
                    }
                });
            }
        });

        // 中栏：SQL 编辑 + 结果表
        CentralPanel::default().show(ctx, |ui| {
            StripBuilder::new(ui)
                .size(Size::relative(0.55)) // editor area
                .size(Size::remainder())     // results
                .vertical(|mut strip| {
                    strip.cell(|ui| {
                        ui.heading("SQL Editor");
                        ui.separator();
                        let current_sql = self.sql_input.clone();
                        let editor = ui.add(TextEdit::multiline(&mut self.sql_input)
                            .desired_rows(8)
                            .layouter(&mut move |ui, _text, wrap_width| {
                                let mut job = highlight_sql(&current_sql, ui.visuals());
                                job.wrap.max_width = wrap_width;
                                ui.fonts(|f| f.layout_job(job))
                            })
                        );
                        ui.horizontal(|ui| {
                            if ui.button("Check").clicked() { self.check_sql(); }
                            if ui.button("Run").clicked() { self.run_sql(); }
                        });
                        if let Some(err) = &self.sql_error { ui.colored_label(egui::Color32::RED, err); }
                        if editor.changed() { self.sql_error = None; }
                    });
                    strip.cell(|ui| {
                        ui.heading("Results");
                        ui.separator();
                        if self.query_columns.is_empty() {
                            ui.label("No results");
                        } else {
                            if ui.button("Export CSV...").clicked() { self.export_csv(); }
                            let mut table = TableBuilder::new(ui).striped(true).cell_layout(egui::Layout::left_to_right(egui::Align::Center));
                            for _ in &self.query_columns { table = table.column(egui_extras::Column::auto()); }
                            table.header(20.0, |mut header| {
                                for col in &self.query_columns { header.col(|ui| { ui.strong(col); }); }
                            }).body(|mut body| {
                                for row in &self.query_rows {
                                    body.row(18.0, |mut r| {
                                        for cell in row { r.col(|ui| { ui.label(cell); }); }
                                    });
                                }
                            });
                        }
                    });
                });
        });

        // 右栏：聊天
        SidePanel::right("chat_panel").resizable(true).min_width(260.0).show(ctx, |ui| {
            ui.heading("Chat");
            ui.separator();
            StripBuilder::new(ui)
                .size(Size::remainder())
                .size(Size::exact(120.0))
                .vertical(|mut strip| {
                    strip.cell(|ui| {
                        ScrollArea::vertical()
                            .auto_shrink([false; 2])
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                let messages: Vec<Message> = self.state.sessions.get(self.state.current_index)
                                    .map(|s| s.messages.clone()).unwrap_or_default();
                                for msg in &messages {
                                    ui.horizontal(|ui| {
                                        ui.strong(match msg.role { Role::User => "User", Role::Assistant => "Assistant", Role::System => "System" });
                                        ui.label(msg.timestamp.format("%Y-%m-%d %H:%M:%S").to_string());
                                        if ui.small_button("Copy").clicked() { if let MessageContent::Markdown(text) = &msg.content { ui.ctx().copy_text(text.clone()); } }
                                    });
                                    match &msg.content {
                                        MessageContent::Markdown(text) => render_markdown(ui, text),
                                        MessageContent::Image { path, width, height } => {
                                        if let Some(handle) = self.media.ensure_texture(ui.ctx(), path) {
                                                let size = egui::vec2(*width as f32, *height as f32).min(egui::vec2(512.0, 512.0));
                                                let img = EguiImage::new(&handle).fit_to_exact_size(size);
                                                let resp = ui.add(img);
                                                if resp.clicked() { self.preview = Some(PreviewState { path: path.clone(), zoom: 1.0 }); }
                                            } else { ui.label(format!("[image missing] {}", path.display())); }
                                        }
                                        MessageContent::Video { path, .. } => { if ui.link(path.display().to_string()).clicked() { let _ = open::that(path); } }
                                    }
                                    ui.separator();
                                }
                            });
                    });
                    strip.cell(|ui| {
                        ui.label("Message");
                        let editor = ui.add(TextEdit::multiline(&mut self.input).desired_rows(4));
                        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                        let cmd = ui.input(|i| i.modifiers.command);
                        let send_via_shortcut = match self.state.settings.send_shortcut { SendShortcut::Enter => enter && editor.has_focus() && !cmd, SendShortcut::CmdEnter => enter && editor.has_focus() && cmd };
                        if ui.button("Send").clicked() || send_via_shortcut { self.commit_input(); }
                        ui.horizontal(|ui| {
                            if ui.button("Send (OpenAI)").clicked() { self.send_openai(); }
                            if ui.button("Send MCP").clicked() { /* TODO: integrate MCP */ }
                        });
                        ui.small("[Planned] Connect OpenAI and MCP clients here");
                    });
                });
        });

        // Image preview window
        if self.preview.is_some() {
            // Take a snapshot to avoid borrow across window closure
            let (mut path, mut zoom) = {
                let p = self.preview.as_ref().unwrap();
                (p.path.clone(), p.zoom)
            };
            let mut open = true;
            egui::Window::new("Image Preview").open(&mut open).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("{}", path.display()));
                    ui.add(egui::Slider::new(&mut zoom, 0.1..=5.0).text("Zoom"));
                });
                if let Some(handle) = self.media.ensure_texture(ui.ctx(), &path) {
                    let tex_size = handle.size_vec2();
                    let size = tex_size * zoom;
                    ui.add(EguiImage::new(&handle).fit_to_exact_size(size));
                } else {
                    ui.label("[image not available]");
                }
            });
            if open {
                if let Some(p) = &mut self.preview { p.zoom = zoom; p.path = path; }
            } else {
                self.preview = None;
            }
        }
    }
}

impl IntelliGuiApp {
    fn commit_input(&mut self) {
        let text = self.input.trim();
        if !text.is_empty() {
            if let Some(session) = self.state.sessions.get_mut(self.state.current_index) {
                session.messages.push(Message {
                    role: Role::User,
                    timestamp: Utc::now(),
                    content: MessageContent::Markdown(text.to_owned()),
                });
            }
            self.save_state();
        }
        self.input.clear();
    }

    fn add_image_message(&mut self, path: PathBuf) {
        let (w, h) = match image::open(&path) {
            Ok(img) => (img.width(), img.height()),
            Err(_) => (0, 0),
        };
        if let Some(session) = self.state.sessions.get_mut(self.state.current_index) {
            session.messages.push(Message {
                role: Role::User,
                timestamp: Utc::now(),
                content: MessageContent::Image { path, width: w, height: h },
            });
        }
        self.save_state();
    }

    fn add_video_message(&mut self, path: PathBuf) {
        if let Some(session) = self.state.sessions.get_mut(self.state.current_index) {
            session.messages.push(Message {
                role: Role::User,
                timestamp: Utc::now(),
                content: MessageContent::Video { path, duration_ms: None, thumbnail: None },
            });
        }
        self.save_state();
    }

    // texture loading now handled by media::MediaCache

    fn check_sql(&mut self) {
        self.sql_error = None;
        let dialect = sqlparser::dialect::PostgreSqlDialect {};
        match sqlparser::parser::Parser::parse_sql(&dialect, &self.sql_input) {
            Ok(_) => { self.sql_error = None; }
            Err(e) => { self.sql_error = Some(format!("{}", e)); }
        }
    }

    fn run_sql(&mut self) {
        self.sql_error = None;
        let Some(sess) = self.state.sessions.get(self.state.current_index).cloned() else {
            self.sql_error = Some("No active session".into()); return;
        };
        let dsn = sess.db.dsn.clone();
        if dsn.trim().is_empty() { self.sql_error = Some("DSN is empty".into()); return; }
        let sql = self.sql_input.clone();

        let rt = match tokio::runtime::Runtime::new() { Ok(rt) => rt, Err(e) => { self.sql_error = Some(format!("runtime error: {}", e)); return; } };
        let pool_opt = self.pools.get(&sess.id).cloned();
        let res: Result<(Vec<String>, Vec<Vec<String>>), String> = rt.block_on(async move {
            let pool = match pool_opt {
                Some(p) => p,
                None => PgPoolOptions::new().max_connections(1).connect(&dsn).await.map_err(|e| e.to_string())?,
            };
            let rows: Vec<PgRow> = sqlx::query(&sql).fetch_all(&pool).await.map_err(|e| e.to_string())?;
            let mut cols: Vec<String> = Vec::new();
            let mut data: Vec<Vec<String>> = Vec::new();
            if let Some(first) = rows.get(0) {
                for c in first.columns() { cols.push(c.name().to_string()); }
            }
            for row in rows.into_iter().take(100) {
                let mut r: Vec<String> = Vec::new();
                let n = if cols.is_empty() { row.len() } else { cols.len() };
                for i in 0..n { r.push(format_cell(&row, i)); }
                data.push(r);
            }
            Ok((cols, data))
        });
        match res {
            Ok((cols, rows)) => { self.query_columns = cols; self.query_rows = rows; }
            Err(e) => { self.sql_error = Some(e); }
        }
    }

    fn clear_current_session(&mut self) {
        if let Some(s) = self.state.sessions.get_mut(self.state.current_index) {
            s.messages.clear();
            self.save_state();
        }
    }

    fn connect_current_session(&mut self) {
        if let Some(sess) = self.state.sessions.get(self.state.current_index).cloned() {
            let dsn = sess.db.dsn.clone();
            if dsn.trim().is_empty() { self.sql_error = Some("DSN is empty".into()); return; }
            let rt = match tokio::runtime::Runtime::new() { Ok(rt) => rt, Err(e) => { self.sql_error = Some(format!("runtime error: {}", e)); return; } };
            match rt.block_on(async move { PgPoolOptions::new().max_connections(5).connect(&dsn).await }) {
                Ok(pool) => { self.pools.insert(sess.id, pool); }
                Err(e) => { self.sql_error = Some(format!("connect error: {}", e)); }
            }
        }
    }

    fn disconnect_current_session(&mut self) {
        if let Some(sess) = self.state.sessions.get(self.state.current_index) {
            self.pools.remove(&sess.id);
        }
    }

    fn export_csv(&mut self) {
        if self.query_columns.is_empty() { return; }
        if let Some(path) = rfd::FileDialog::new().set_title("Save CSV").add_filter("CSV", &["csv"]).save_file() {
            if let Ok(mut wtr) = csv::Writer::from_path(&path) {
                let _ = wtr.write_record(&self.query_columns);
                for row in &self.query_rows { let _ = wtr.write_record(row); }
                let _ = wtr.flush();
            }
        }
    }

    fn send_openai(&mut self) {
        let Some(key) = self.state.settings.openai_api_key.clone() else { self.sql_error = Some("OpenAI API key not set".into()); return; };
        let model = self.state.settings.openai_model.clone();
        let prompt = self.input.trim().to_string();
        if prompt.is_empty() { return; }
        let mut session_id = None;
        if let Some(sess) = self.state.sessions.get(self.state.current_index) { session_id = Some(sess.id); }
        let rt = match tokio::runtime::Runtime::new() { Ok(rt) => rt, Err(e) => { self.sql_error = Some(format!("runtime error: {}", e)); return; } };
        let res: Result<String, String> = rt.block_on(async move { chat_once(key, model, prompt).await });
        match res {
            Ok(answer) => {
                if let Some(id) = session_id {
                    if let Some(sess) = self.state.sessions.iter_mut().find(|s| s.id == id) {
                        sess.messages.push(Message { role: Role::Assistant, timestamp: Utc::now(), content: MessageContent::Markdown(answer) });
                        self.save_state();
                    }
                }
            }
            Err(e) => { self.sql_error = Some(format!("openai error: {}", e)); }
        }
    }
}

impl IntelliGuiApp {
    fn delete_session(&mut self, idx: usize) {
        if idx < self.state.sessions.len() {
            self.state.sessions.remove(idx);
            if self.state.sessions.is_empty() {
                self.state.sessions.push(Session {
                    id: self.state.next_session_id,
                    title: "New Session".to_string(),
                    messages: Vec::new(),
                    db: DbConfig { engine: "postgres".to_string(), dsn: String::new() }
                });
                self.state.next_session_id += 1;
            }
            if self.state.current_index >= self.state.sessions.len() {
                self.state.current_index = self.state.sessions.len() - 1;
            }
            self.save_state();
        }
    }
}


pub fn run() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "IntelliD GUI",
        native_options,
        Box::new(|_| Ok(Box::new(IntelliGuiApp::new()))),
    )
}
