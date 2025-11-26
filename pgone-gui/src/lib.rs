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
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

mod futures;
mod models;
use models::*;
mod markdown;
mod notify;
mod sql;

mod components;
use components::{ChatPanel, DbManager, DbTree, PreviewManager, ResultsTable, SettingsPanel};
mod media;

pub struct AppFrame {
    #[allow(dead_code)]
    state: PersistedState,
    show_settings: bool,
    db: DbManager,
    results_table: ResultsTable,
    preview: PreviewManager,
    chat: ChatPanel,
    db_tree: DbTree,
    settings_panel: SettingsPanel,
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
            db: Default::default(),
            results_table: Default::default(),
            preview: Default::default(),
            chat: Default::default(),
            db_tree: Default::default(),
            settings_panel: Default::default(),
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
                tokio::runtime::Handle::current()
                    .block_on(async { storage.create_session(&sess).await })
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
                });
                ui.menu_button("View", |ui| {
                    if ui
                        .checkbox(&mut self.left_panel_visible, "Left Panel")
                        .changed()
                    {
                        // Panel visibility toggled
                    }
                    if ui
                        .checkbox(&mut self.right_panel_visible, "Right Panel")
                        .changed()
                    {
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
                // ui.label("Ready");
                // ui.add_space(ui.available_width() - 200.0);

                // Database selection button and display
                ui.horizontal(|ui| {
                    if ui.button("Select Database").clicked() {
                        db.show_manage_db = true;
                    }
                    ui.separator();
                    let active_id = db.active_db_config_id.clone();
                    let db_name = if let Some(ref id) = active_id {
                        db.get_db_name(id).unwrap_or_else(|| id.clone())
                    } else {
                        "<no db>".to_string()
                    };
                    ui.label(format!("Selected Database Config: {}", db_name));
                    ui.separator();

                    if active_id.is_some() {

                        db.ensure_storage();
                    if let Some(ref storage) = db.storage {
                        if let Ok(Some(cfg)) = tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current()
                                .block_on(async { storage.get_db_config(&active_id.as_ref().unwrap()).await })
                        }) {
                            // Parse DSN to get connection details
                            if let Some(parsed) = crate::components::DbManager::parse_dsn(&cfg.dsn)
                            {
                                ui.horizontal(|ui| {
                                    ui.label(egui_phosphor::regular::DATABASE);
                                    ui.label(egui::RichText::new(&cfg.id).strong());
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Engine:");
                                    ui.label(&cfg.engine);
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Host:");
                                    ui.label(&parsed.host);
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Database:");
                                    ui.label(if parsed.database.is_empty() {
                                        "<default>"
                                    } else {
                                        &parsed.database
                                    });
                                });
                            }
                        }
                    }
                    }

                    
                });
            });
        });

        db.ui_add_db_window(ctx);
        db.ui_manage_db_window(ctx);
        db.ui_edit_db_window(ctx);

        if self.show_settings {
            let mut open = true;
            let mut settings_changed = false;
            egui::Window::new("设置")
                .open(&mut open)
                .default_pos(Self::screen_center(ctx))
                .pivot(egui::Align2::CENTER_CENTER)
                .default_size([400.0, 300.0])
                .show(ctx, |ui| {
                    let old_settings = self.state.settings.clone();
                    self.settings_panel.ui(ui, &mut self.state.settings, ctx);
                    if self.state.settings.font_family != old_settings.font_family
                        || self.state.settings.font_size != old_settings.font_size
                    {
                        settings_changed = true;
                    }
                });
            if settings_changed {
                self.save_state();
            }
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
                self.db_tree.ui(ui, &mut self.db, &mut self.results_table);
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

        // Center panel - Results table with SQL editor
        CentralPanel::default()
            .frame(Frame::central_panel(&ctx.style()).inner_margin(0.))
            .show(ctx, |ui| {
                // Ensure storage is initialized before creating SqlCtx
                self.db.ensure_storage();
                let mut sql_ctx = components::SqlCtx {
                    state: self.state.clone(),
                    db: crate::components::DbManager {
                        active_db_config_id: self.db.active_db_config_id.clone(),
                        pools: self.db.pools.clone(),
                        storage: None, // Will be initialized in run_sql via ensure_storage()
                        ..Default::default()
                    },
                };
                // Initialize storage in SqlCtx by ensuring it's available
                if self.db.storage.is_some() {
                    sql_ctx.db.ensure_storage();
                }
                self.results_table.ui(ui, Some(&mut sql_ctx));
                // Update pools back if they were modified
                self.db.pools = sql_ctx.db.pools;
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

            // Load all fonts from assets/fonts directory
            let fonts_dir = Path::new("assets/fonts");
            let mut loaded_fonts = Vec::new();

            if let Ok(entries) = fs::read_dir(fonts_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("ttf") {
                        if let Ok(font_data) = fs::read(&path) {
                            // Extract font name from filename (without extension)
                            let font_name = path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("Unknown")
                                .to_string();

                            fonts.font_data.insert(
                                font_name.clone(),
                                Arc::new(egui::FontData::from_owned(font_data)),
                            );

                            loaded_fonts.push(font_name);
                        }
                    }
                }
            }

            // Load settings to get default font and size
            let state = AppFrame::load_state().unwrap_or_else(|| PersistedState {
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

            // Set default font family based on settings
            let default_font = &state.settings.font_family;
            if fonts.font_data.contains_key(default_font) {
                // Add selected font to the front of proportional font family
                fonts
                    .families
                    .get_mut(&egui::FontFamily::Proportional)
                    .unwrap()
                    .insert(0, default_font.clone());

                // Also add to monospace family
                fonts
                    .families
                    .get_mut(&egui::FontFamily::Monospace)
                    .unwrap()
                    .push(default_font.clone());
            } else {
                // Fallback: add all loaded fonts
                for font_name in &loaded_fonts {
                    fonts
                        .families
                        .get_mut(&egui::FontFamily::Proportional)
                        .unwrap()
                        .insert(0, font_name.clone());
                    fonts
                        .families
                        .get_mut(&egui::FontFamily::Monospace)
                        .unwrap()
                        .push(font_name.clone());
                }
            }

            // Apply default font size to text styles
            let font_size = state.settings.font_size;
            let mut style = (*cc.egui_ctx.style()).clone();
            for text_style in style.text_styles.values_mut() {
                text_style.size = font_size;
            }
            cc.egui_ctx.set_style(style);

            // Apply theme
            SettingsPanel::apply_theme(&cc.egui_ctx, state.settings.theme);

            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(AppFrame::new()))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {}", e))
}
