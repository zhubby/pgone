use chrono::{DateTime, Utc};
use eframe::egui::{self, CentralPanel, Context, SidePanel, TopBottomPanel};
use egui::Frame;
use egui_phosphor::Variant as PhosphorVariant;
use icns::{IconFamily, IconType};
use serde::Deserialize;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

mod futures;
mod models;
use models::*;
mod markdown;
mod notify;
mod sql;

mod components;
use components::{ChatPanel, DbManager, DbTree, PreviewManager, SqlPanel};
mod media;

pub struct AppFrame {
    #[allow(dead_code)]
    state: PersistedState,
    show_settings: bool,
    show_sql_editor: bool,
    db: DbManager,
    sql: SqlPanel,
    preview: PreviewManager,
    chat: ChatPanel,
    db_tree: DbTree,
    left_panel_visible: bool,
    right_panel_visible: bool,
    left_panel_width: f32,
    right_panel_width: f32,
}

impl AppFrame {
    const SESSIONS_PATH: &'static str = "sessions.json";

    /// Get the center position of the screen for window placement
    fn screen_center(ctx: &egui::Context) -> egui::Pos2 {
        ctx.screen_rect().center()
    }

    fn new() -> Self {
        let state = Self::load_state().unwrap_or_else(|| PersistedState {
            sessions: vec![Session {
                id: 1,
                title: "New Session".to_string(),
                messages: Vec::new(),
                db: DbConfig {
                    engine: "postgres".to_string(),
                    dsn: String::new(),
                },
            }],

            current_index: 0,
            next_session_id: 2,
            settings: Settings::default(),
        });

        Self {
            state,
            show_settings: false,
            show_sql_editor: false,
            db: Default::default(),
            sql: Default::default(),
            preview: Default::default(),
            chat: Default::default(),
            db_tree: Default::default(),
            left_panel_visible: true,
            right_panel_visible: true,
            left_panel_width: 250.0,
            right_panel_width: 300.0,
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
        struct LegacyMessage {
            role: Role,
            timestamp: DateTime<Utc>,
            content: String,
        }
        #[derive(Deserialize)]
        struct LegacySession {
            id: u64,
            title: String,
            messages: Vec<LegacyMessage>,
        }
        #[derive(Deserialize)]
        struct LegacyState {
            sessions: Vec<LegacySession>,
            current_index: usize,
            next_session_id: u64,
        }
        if let Ok(old) = serde_json::from_str::<LegacyState>(&data) {
            let sessions = old
                .sessions
                .into_iter()
                .map(|s| Session {
                    id: s.id,
                    title: s.title,
                    messages: s
                        .messages
                        .into_iter()
                        .map(|m| Message {
                            role: m.role,
                            timestamp: m.timestamp,
                            content: MessageContent::Markdown(m.content),
                        })
                        .collect(),
                    db: DbConfig::default(),
                })
                .collect();
            return Some(PersistedState {
                sessions,
                current_index: old.current_index,
                next_session_id: old.next_session_id,
                settings: Settings::default(),
            });
        }
        None
    }

    #[allow(dead_code)]
    fn save_state(&self) {
        let _ = fs::write(
            Self::SESSIONS_PATH,
            serde_json::to_string_pretty(&self.state).unwrap_or_default(),
        );
    }

    #[allow(dead_code)]
    fn migrate_from_json(&mut self) -> Result<(), String> {
        self.db.ensure_storage();
        let Some(storage) = self.db.storage.as_ref() else {
            return Err("storage missing".into());
        };
        // migrate sessions and messages
        for s in &self.state.sessions {
            let sess = pgone_storage::models::Session {
                id: s.id.to_string(),
                title: s.title.clone(),
                config_id: None,
                created_at: 0,
                updated_at: 0,
            };
            let _ = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    storage.create_session(&sess).await
                })
            });
            for m in &s.messages {
                match &m.content {
                    MessageContent::Markdown(text) => {
                        let _ = tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(async {
                                storage
                                    .append_markdown(
                                        &sess.id,
                                        pgone_storage::models::Role::User,
                                        text,
                                    )
                                    .await
                            })
                        });
                    }
                    MessageContent::Image {
                        path,
                        width,
                        height,
                    } => {
                        let _ = tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(async {
                                storage
                                    .append_image(
                                        &sess.id,
                                        pgone_storage::models::Role::User,
                                        &path.display().to_string(),
                                        *width as i64,
                                        *height as i64,
                                    )
                                    .await
                            })
                        });
                    }
                    MessageContent::Video {
                        path, duration_ms, ..
                    } => {
                        let _ = tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(async {
                                storage
                                    .append_video(
                                        &sess.id,
                                        pgone_storage::models::Role::User,
                                        &path.display().to_string(),
                                        duration_ms.map(|v| v as i64),
                                    )
                                    .await
                            })
                        });
                    }
                }
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    fn now_ts() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }
}

impl eframe::App for AppFrame {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        let Self { db, .. } = self;
        // fonts are initialized in run() creation context to avoid runtime deadlocks
        TopBottomPanel::top("menu_top").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Database...").clicked() {
                        db.show_add_db = true;
                        ui.close();
                    }
                    if ui.button("Manage Databases...").clicked() {
                        db.show_manage_db = true;
                        ui.close();
                    }
                    if ui.button("SQL Editor...").clicked() {
                        self.show_sql_editor = true;
                        ui.close();
                    }
                });
                ui.menu_button("View", |ui| {
                    if ui.checkbox(&mut self.left_panel_visible, "Left Panel").changed() {
                        // Panel visibility toggled
                    }
                    if ui.checkbox(&mut self.right_panel_visible, "Right Panel").changed() {
                        // Panel visibility toggled
                    }
                    ui.separator();
                    if ui.button("Clear Current Session").clicked() {
                        // self.clear_current_session();
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
                ui.add_space(ui.available_width() - 200.0);
                
                // Database selection button and display
                ui.horizontal(|ui| {
                    if ui.button("Select Database").clicked() {
                        db.show_manage_db = true;
                    }
                    ui.separator();
                    let active_id = db.active_db_config_id.clone();
                    let db_name = if let Some(id) = active_id {
                        db.get_db_name(&id).unwrap_or_else(|| id)
                    } else {
                        "<no db>".to_string()
                    };
                    ui.label(format!("DB: {}", db_name));
                });
            });
        });

        db.ui_add_db_window(ctx);
        db.ui_manage_db_window(ctx);
        db.ui_edit_db_window(ctx);
        
        // SQL Editor window
        if self.show_sql_editor {
            let mut open = true;
            egui::Window::new("SQL Editor")
                .open(&mut open)
                .default_size(egui::vec2(800.0, 600.0))
                .default_pos(Self::screen_center(ctx))
                .pivot(egui::Align2::CENTER_CENTER)
                .show(ctx, |ui| {
                    let mut sql_ctx = components::SqlCtx {
                        state: self.state.clone(),
                        db: crate::components::DbManager {
                            pools: self.db.pools.clone(),
                            ..Default::default()
                        },
                    };
                    self.sql.ui_editor(&mut sql_ctx, ui);
                });
            if !open {
                self.show_sql_editor = false;
            }
        }
        
        if self.show_settings {
            let mut open = true;
            egui::Window::new("Settings")
                .open(&mut open)
                .default_pos(Self::screen_center(ctx))
                .pivot(egui::Align2::CENTER_CENTER)
                .show(ctx, |_ui| {});
            if !open {
                self.show_settings = false;
            }
        }
        
        // Left panel - Database structure tree
        SidePanel::left("left_panel")
            .resizable(true)
            .default_width(self.left_panel_width)
            .min_width(100.0)
            .max_width(500.0)
            .show_animated(ctx, self.left_panel_visible, |ui| {
                self.db_tree.ui(ui, &mut self.db, &mut self.sql);
            });
        
        // Right panel - Chat
        SidePanel::right("right_panel")
            .resizable(true)
            .default_width(self.right_panel_width)
            .min_width(100.0)
            .max_width(500.0)
            .show_animated(ctx, self.right_panel_visible, |ui| {
                let settings = self.state.settings.clone();
                let mut chat_ctx = components::ChatCtx {
                    state: &mut self.state,
                    preview: &mut self.preview,
                    send_shortcut: settings.send_shortcut,
                    openai_api_key: settings.openai_api_key.clone(),
                    openai_model: settings.openai_model.clone(),
                };
                self.chat.ui(&mut chat_ctx, ui);
            });
        
        // Center panel - Results table
        CentralPanel::default()
            .frame(Frame::central_panel(&ctx.style()).inner_margin(0.))
            .show(ctx, |ui| {
                self.sql.ui_results(ui);
            });

        // Image preview window
        self.preview.ui_window(ctx);
        
        // 显示通知
        notify::show(ctx);
    }
}

impl AppFrame {
    #[allow(dead_code)]
    fn clear_current_session(&mut self) {
        if let Some(s) = self.state.sessions.get_mut(self.state.current_index) {
            s.messages.clear();
            self.save_state();
        }
    }
}

impl AppFrame {
    #[allow(dead_code)]
    fn delete_session(&mut self, idx: usize) {
        if idx < self.state.sessions.len() {
            self.state.sessions.remove(idx);
            if self.state.sessions.is_empty() {
                self.state.sessions.push(Session {
                    id: self.state.next_session_id,
                    title: "New Session".to_string(),
                    messages: Vec::new(),
                    db: DbConfig {
                        engine: "postgres".to_string(),
                        dsn: String::new(),
                    },
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

pub fn run() -> anyhow::Result<()> {
    let file = BufReader::new(File::open("assets/icon.icns").unwrap());
    let icon_family = IconFamily::read(file).unwrap();
    let image = icon_family
        .get_icon_with_type(IconType::RGBA32_512x512_2x)
        .unwrap();
    let mut buf = Vec::new();
    image.write_png(&mut buf)?;
    let icon = eframe::icon_data::from_png_bytes(&buf).expect("Failed to load icon");
    let title = "PGone";
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_maximized(true)
            .with_icon(icon)
            .with_title_shown(false),
        ..Default::default()
    };

    eframe::run_native(
        title,
        native_options,
        Box::new(|cc| {
            // Inject phosphor font once at creation to avoid runtime deadlocks
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, PhosphorVariant::Regular);
            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(AppFrame::new()))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {}", e))
}
