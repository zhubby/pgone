use eframe::egui::{self, CentralPanel, Context, SidePanel, TopBottomPanel};
use egui_dock::{DockArea, DockState};
use egui_phosphor::Variant as PhosphorVariant;
use egui_extras::{StripBuilder, Size};
use sqlx::{Row, Column};
use sqlx::postgres::{PgRow, PgPool, PgPoolOptions};
use chrono::{Utc, DateTime};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use eframe::egui::widgets::Image as EguiImage;
// removed unused imports
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

mod models; use models::*;
use intellid_storage::blocking::StorageBlocking;
mod markdown;
mod sql; use sql::format_cell;
mod media; use media::MediaCache;
mod openai_client; use openai_client::chat_once;
mod ui;
use ui::tabs::{LeftTab, RightTab, CenterTopTab, CenterBottomTab, LeftViewer, RightViewer, CenterTopViewer, CenterBottomViewer};

#[derive(Debug, Clone)]
struct PreviewState { path: PathBuf, zoom: f32 }

// removed unused AppMsg

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
    storage: Option<StorageBlocking>,
    // current active db config id (from storage)
    active_db_config_id: Option<String>,
    show_add_db: bool,
    add_db_engine: String,
    add_db_name: String,
    add_db_host: String,
    add_db_port: String,
    add_db_database: String,
    add_db_user: String,
    add_db_password: String,
    add_db_error: Option<String>,
    show_manage_db: bool,
    // collapsible side panels flags
    show_left_panel: bool,
    show_right_panel: bool,
    // Dock trees for sidebars and center (top/bottom)
    left_tree: DockState<LeftTab>,
    right_tree: DockState<RightTab>,
    center_top_tree: DockState<CenterTopTab>,
    center_bottom_tree: DockState<CenterBottomTab>,
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
        // initialize dock trees
        let left_tree = DockState::new(vec![LeftTab::Sessions, LeftTab::DbConfig]);
        let right_tree = DockState::new(vec![RightTab::Chat]);
        let center_top_tree = DockState::new(vec![CenterTopTab::SqlEditor]);
        let center_bottom_tree = DockState::new(vec![CenterBottomTab::Results]);

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
            storage: None,
            active_db_config_id: None,
            show_add_db: false,
            add_db_engine: "postgres".to_string(),
            add_db_name: String::new(),
            add_db_host: "localhost".to_string(),
            add_db_port: "5432".to_string(),
            add_db_database: String::new(),
            add_db_user: String::new(),
            add_db_password: String::new(),
            add_db_error: None,
            show_manage_db: false,
            show_left_panel: true,
            show_right_panel: true,
            left_tree,
            right_tree,
            center_top_tree,
            center_bottom_tree,
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

    fn save_state(&self) { let _ = fs::write(Self::SESSIONS_PATH, serde_json::to_string_pretty(&self.state).unwrap_or_default()); }

    fn ensure_storage(&mut self) {
        if self.storage.is_some() { return; }
        if let Ok(storage) = self.rt.block_on(async { StorageBlocking::open_local("intellid.db").await }) {
            // one-time migrate from sessions.json if exists
            if Path::new(Self::SESSIONS_PATH).exists() {
                let _ = self.migrate_from_json();
                let _ = std::fs::remove_file(Self::SESSIONS_PATH);
            }
            self.storage = Some(storage);
        } else {
            self.sql_error = Some("storage open failed".into());
        }
    }

    fn migrate_from_json(&mut self) -> Result<(), String> {
        let Some(storage) = self.storage.as_ref() else { return Err("storage missing".into()); };
        // migrate sessions and messages
        for s in &self.state.sessions {
            let sess = intellid_storage::models::Session { id: s.id.to_string(), title: s.title.clone(), config_id: None, created_at: 0, updated_at: 0 };
            let _ = self.rt.block_on(async { storage.create_session(&sess).await });
            for m in &s.messages {
                match &m.content {
                    MessageContent::Markdown(text) => { let _ = self.rt.block_on(async { storage.append_markdown(&sess.id, intellid_storage::models::Role::User, text).await }); }
                    MessageContent::Image { path, width, height } => { let _ = self.rt.block_on(async { storage.append_image(&sess.id, intellid_storage::models::Role::User, &path.display().to_string(), *width as i64, *height as i64).await }); }
                    MessageContent::Video { path, duration_ms, .. } => { let _ = self.rt.block_on(async { storage.append_video(&sess.id, intellid_storage::models::Role::User, &path.display().to_string(), duration_ms.map(|v| v as i64)).await }); }
                }
            }
        }
        Ok(())
    }

    fn now_ts() -> i64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64 }

    fn save_new_database(&mut self) -> Result<(), String> {
        self.ensure_storage();
        let Some(storage) = self.storage.as_ref() else { return Err("storage not ready".into()); };

        // basic validations
        if self.add_db_name.trim().is_empty() { return Err("Name is required".into()); }
        if self.add_db_engine.trim().is_empty() { return Err("Type is required".into()); }
        if self.add_db_host.trim().is_empty() { return Err("Host is required".into()); }
        let port: u16 = self.add_db_port.parse().map_err(|_| "Port must be a number")?;
        if port == 0 { return Err("Port must be > 0".into()); }
        if self.add_db_user.trim().is_empty() { return Err("User is required".into()); }

        // build DSN (postgres as example)
        let dbname = if self.add_db_database.trim().is_empty() { String::new() } else { self.add_db_database.trim().to_string() };
        let dsn = format!("{}://{}:{}@{}:{}{}",
            self.add_db_engine.trim(),
            urlencoding::encode(self.add_db_user.trim()),
            urlencoding::encode(self.add_db_password.trim()),
            self.add_db_host.trim(),
            port,
            if dbname.is_empty() { String::new() } else { format!("/{}", dbname) }
        );

        let now = Self::now_ts();
        let cfg = intellid_storage::models::DbConfig {
            id: self.add_db_name.trim().to_string(),
            engine: self.add_db_engine.trim().to_string(),
            dsn,
            default_schemas: None,
            include_system: Some(false),
            created_at: now,
            updated_at: now,
        };
        let res = self.rt.block_on(async { storage.upsert_db_config(&cfg).await });
        match res { Ok(_) => Ok(()), Err(e) => Err(e.to_string()) }
    }
}

impl eframe::App for IntelliGuiApp {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        // fonts are initialized in run() creation context to avoid runtime deadlocks
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
                    if ui.button("New Database...").clicked() {
                        self.show_add_db = true;
                        ui.close();
                    }
                    if ui.button("Manage Databases...").clicked() {
                        self.show_manage_db = true;
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

        // Bottom status bar
        TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Ready");
                ui.add_space(ui.available_width() - 120.0);
                let name = self.active_db_config_id.clone().unwrap_or_else(|| "<no db>".to_string());
                ui.label(format!("DB: {}", name));
            });
        });

        // fixed three-column layout; no edge toggle buttons

        // Settings window
        if self.show_add_db {
            let mut open = true;
            egui::Window::new("New Database").open(&mut open).show(ctx, |ui| {
                ui.horizontal(|ui| { ui.label("Type"); ui.text_edit_singleline(&mut self.add_db_engine); });
                ui.horizontal(|ui| { ui.label("Name"); ui.text_edit_singleline(&mut self.add_db_name); });
                ui.horizontal(|ui| { ui.label("Host"); ui.text_edit_singleline(&mut self.add_db_host); });
                ui.horizontal(|ui| { ui.label("Port"); ui.text_edit_singleline(&mut self.add_db_port); });
                ui.horizontal(|ui| { ui.label("Database"); ui.text_edit_singleline(&mut self.add_db_database); });
                ui.horizontal(|ui| { ui.label("User"); ui.text_edit_singleline(&mut self.add_db_user); });
                ui.horizontal(|ui| { ui.label("Password"); ui.add(egui::TextEdit::singleline(&mut self.add_db_password).password(true)); });
                if let Some(err) = &self.add_db_error { ui.colored_label(egui::Color32::RED, err); }
                if ui.button("Save").clicked() {
                    if let Err(e) = self.save_new_database() { self.add_db_error = Some(e); } else { self.show_add_db = false; self.add_db_error = None; }
                }
            });
            if !open { self.show_add_db = false; }
        }

        if self.show_manage_db {
            let mut open = true;
            egui::Window::new("Databases").open(&mut open).show(ctx, |ui| {
                self.ensure_storage();
                if let Some(storage) = &self.storage {
                    let list = self.rt.block_on(async { storage.list_db_configs(None).await }).unwrap_or_default();
                    for cfg in list {
                        ui.horizontal(|ui| {
                            ui.label(format!("{}", cfg.id));
                            ui.small(format!("[{}]", cfg.engine));
                            if ui.small_button("Delete").clicked() {
                                let _ = self.rt.block_on(async { storage.delete_db_config(&cfg.id).await });
                            }
                        });
                    }
                } else {
                    ui.label("Storage not ready");
                }
            });
            if !open { self.show_manage_db = false; }
        }
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

        // 左栏：使用 Dock Tabs（Sessions/DB Config）
        SidePanel::left("session_panel").resizable(true).min_width(220.0).show(ctx, |ui| {
            // Dock tabs on left (uses TabViewer titles with icons)
            let mut tmp = DockState::new(Vec::new());
            std::mem::swap(&mut self.left_tree, &mut tmp);
            ui.push_id("left_dock", |ui| {
                let mut viewer = LeftViewer { app: self };
                DockArea::new(&mut tmp).show_inside(ui, &mut viewer);
            });
            std::mem::swap(&mut self.left_tree, &mut tmp);
        });

        // 中栏：上下分别为 Dock tabs（SQL / Results）
        CentralPanel::default().show(ctx, |ui| {
            StripBuilder::new(ui)
                .size(Size::relative(0.55)) // editor area
                .size(Size::remainder())     // results
                .vertical(|mut strip| {
                    strip.cell(|ui| {
                        let mut tmp = DockState::new(Vec::new());
                        std::mem::swap(&mut self.center_top_tree, &mut tmp);
                        ui.push_id("center_top_dock", |ui| {
                            let mut viewer = CenterTopViewer { app: self };
                            DockArea::new(&mut tmp).show_inside(ui, &mut viewer);
                        });
                        std::mem::swap(&mut self.center_top_tree, &mut tmp);
                    });
                    strip.cell(|ui| {
                        let mut tmp = DockState::new(Vec::new());
                        std::mem::swap(&mut self.center_bottom_tree, &mut tmp);
                        ui.push_id("center_bottom_dock", |ui| {
                            let mut viewer = CenterBottomViewer { app: self };
                            DockArea::new(&mut tmp).show_inside(ui, &mut viewer);
                        });
                        std::mem::swap(&mut self.center_bottom_tree, &mut tmp);
                    });
                });
        });

        // 右栏：Dock tabs（Chat）
        SidePanel::right("chat_panel").resizable(true).min_width(260.0).show(ctx, |ui| {
            let mut tmp = DockState::new(Vec::new());
            std::mem::swap(&mut self.right_tree, &mut tmp);
            ui.push_id("right_dock", |ui| {
                let mut viewer = RightViewer { app: self };
                DockArea::new(&mut tmp).show_inside(ui, &mut viewer);
            });
            std::mem::swap(&mut self.right_tree, &mut tmp);
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
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_maximized(true),
        ..Default::default()
    };
    eframe::run_native(
        "Intelligent Database",
        native_options,
        Box::new(|cc| {
            // Inject phosphor font once at creation to avoid runtime deadlocks
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, PhosphorVariant::Regular);
            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(IntelliGuiApp::new()))
        }),
    )
}
