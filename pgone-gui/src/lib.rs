use eframe::egui::{self, Context};
use egui_phosphor::Variant as PhosphorVariant;
use icns::{IconFamily, IconType};
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

mod futures;
mod models;
use models::*;
mod markdown;
mod notify;
mod sql;
mod storage;
use storage::SessionStorage;

mod components;
use components::{ChatPanel, DbManager, DbTree, PreviewManager, ResultsTable, SchemaGraph, SettingsPanel};
mod layout;
mod media;

pub struct AppFrame {
    #[allow(dead_code)]
    state: PersistedState,
    show_settings: bool,
    show_about: bool,
    show_graph: bool,
    graph_schema: Option<(String, String)>, // (database, schema)
    graph: SchemaGraph,
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
    session_storage: SessionStorage,
}

impl AppFrame {
    // const SESSIONS_PATH: &'static str = "sessions.json";

    fn new() -> Self {
        // Initialize storage first to load settings
        let mut db_manager = components::DbManager::default();
        db_manager.ensure_storage();
        
        // Load settings from database
        let settings = if let Some(ref storage) = db_manager.storage {
            if let Ok(kv_map) = futures::block_on_async(async {
                storage.get_all_settings().await
            }) {
                let loaded_settings = Settings::from_kv_map(&kv_map);
                tracing::debug!("Loaded settings from DB: {:?}", loaded_settings);
                tracing::debug!("KV map: {:?}", kv_map);
                loaded_settings
            } else {
                tracing::warn!("Failed to load settings from database");
                Settings::default()
            }
        } else {
            tracing::warn!("Storage not available, using default settings");
            Settings::default()
        };
        
        // Initialize session storage
        let session_storage = SessionStorage::new();
        
        // Load sessions from JSON file
        let mut state = Self::load_state().map(|mut s| {
            // Override settings with database-loaded settings
            s.settings = settings.clone();
            s
        }).unwrap_or_else(|| PersistedState {
            current_db_config_id: None,
            settings: settings.clone(),
            sessions: vec![ChatSession::new("0".to_string(), "新会话".to_string())],
            current_index: 0,
            next_session_id: 1,
        });
        
        // Load sessions from file and merge with state
        if let Ok(loaded_sessions) = session_storage.load_sessions() {
            if !loaded_sessions.is_empty() {
                state.sessions = loaded_sessions;
                // Ensure current_index is valid
                if state.current_index >= state.sessions.len() {
                    state.current_index = 0;
                }
                // Update next_session_id based on loaded sessions
                if let Some(max_id) = state.sessions.iter()
                    .filter_map(|s| s.id.parse::<u64>().ok())
                    .max() {
                    state.next_session_id = max_id + 1;
                }
            }
        }

        Self {
            state,
            show_settings: false,
            show_about: false,
            show_graph: false,
            graph_schema: None,
            graph: SchemaGraph::default(),
            db: db_manager,
            results_table: Default::default(),
            preview: Default::default(),
            chat: Default::default(),
            db_tree: Default::default(),
            settings_panel: Default::default(),
            left_panel_visible: true,
            right_panel_visible: true,
            left_panel_width: 250.0,
            right_panel_width: 300.0,
            session_storage,
        }
    }

    fn load_state() -> Option<PersistedState> {
        Some(PersistedState {
            current_db_config_id: None,
            settings: Settings::default(),
            sessions: vec![],
            current_index: 0,
            next_session_id: 1,
        })
        // if !Path::new(Self::SESSIONS_PATH).exists() {
        //     return None;
        // }
        // let data = fs::read_to_string(Self::SESSIONS_PATH).ok()?;
        // // Try new format first
        // if let Ok(state) = serde_json::from_str::<PersistedState>(&data) {
        //     return Some(state);
        // }
        // // Fallback to legacy format (content: String)
        // #[derive(Deserialize)]
        // struct LegacyMessage {
        //     role: Role,
        //     timestamp: DateTime<Utc>,
        //     content: String,
        // }
        // #[derive(Deserialize)]
        // struct LegacySession {
        //     id: u64,
        //     title: String,
        //     messages: Vec<LegacyMessage>,
        // }
        // #[derive(Deserialize)]
        // struct LegacyState {
        //     sessions: Vec<LegacySession>,
        //     current_index: usize,
        //     next_session_id: u64,
        // }
        // if let Ok(old) = serde_json::from_str::<LegacyState>(&data) {
        //     let sessions = old
        //         .sessions
        //         .into_iter()
        //         .map(|s| Session {
        //             id: s.id,
        //             title: s.title,
        //             messages: s
        //                 .messages
        //                 .into_iter()
        //                 .map(|m| Message {
        //                     role: m.role,
        //                     timestamp: m.timestamp,
        //                     content: MessageContent::Markdown(m.content),
        //                 })
        //                 .collect(),
        //             db: DbConfig::default(),
        //         })
        //         .collect();
        //     return Some(PersistedState {
        //         sessions,
        //         current_index: old.current_index,
        //         next_session_id: old.next_session_id,
        //         settings: Settings::default(),
        //     });
        // }
        // None
    }

    #[allow(dead_code)]
    fn save_state(&mut self) {
        // Save settings to database
        self.db.ensure_storage();
        if let Some(ref storage) = self.db.storage {
            let kv_map = self.state.settings.to_kv_map();
            let _ = futures::block_on_async(async {
                for (key, value) in kv_map {
                    let _ = storage.upsert_setting(&key, &value).await;
                }
            });
        }
        
        // // Also save to JSON for backward compatibility (sessions only)
        // let sessions_only = serde_json::json!({
        //     "sessions": self.state.sessions,
        //     "current_index": self.state.current_index,
        //     "next_session_id": self.state.next_session_id,
        // });
        // let _ = fs::write(
        //     Self::SESSIONS_PATH,
        //     serde_json::to_string_pretty(&sessions_only).unwrap_or_default(),
        // );
    }
    
    /// Save settings to database
    pub fn save_settings(&mut self) {
        self.db.ensure_storage();
        if let Some(ref storage) = self.db.storage {
            let kv_map = self.state.settings.to_kv_map();
            let _ = futures::block_on_async(async {
                for (key, value) in kv_map {
                    let _ = storage.upsert_setting(&key, &value).await;
                }
            });
        }
    }
}

impl eframe::App for AppFrame {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        // Menu bar
        layout::menu_bar::show_menu_bar(
            ctx,
            &mut self.db,
            &mut self.left_panel_visible,
            &mut self.right_panel_visible,
            &mut self.show_settings,
            &mut self.show_about,
        );

        // Status bar
        layout::status_bar::show_status_bar(ctx, &mut self.db);

        // Database management windows
        self.db.ui_add_db_window(ctx);
        self.db.ui_manage_db_window(ctx);
        self.db.ui_edit_db_window(ctx);

        // Settings window
        if layout::windows::show_settings_window(
            ctx,
            &mut self.show_settings,
            &mut self.state,
            &mut self.settings_panel,
        ) {
            // Save settings to database if changed
            self.save_settings();
        }

        // About window
        layout::windows::show_about_window(ctx, &mut self.show_about);

        // Check for pending graph window open
        if let Some(schema_info) = self.db_tree.take_pending_open_graph() {
            self.show_graph = true;
            self.graph_schema = Some(schema_info.clone());
            // Reinitialize graph with new schema
            self.graph = SchemaGraph::new(schema_info.0.clone(), schema_info.1.clone());
        }

        // Graph window
        layout::windows::show_graph_window(
            ctx,
            &mut self.show_graph,
            self.graph_schema.clone(),
            &mut self.db,
            &mut self.graph,
        );

        // Panels
        layout::panels::show_left_panel(
            ctx,
            self.left_panel_visible,
            self.left_panel_width,
            &mut self.db_tree,
            &mut self.db,
            &mut self.results_table,
        );

        layout::panels::show_right_panel(
            ctx,
            self.right_panel_visible,
            self.right_panel_width,
            &mut self.chat,
            &mut self.state,
            &mut self.preview,
            &self.session_storage,
        );

        layout::panels::show_center_panel(ctx, &mut self.db, &mut self.results_table, &self.state);

        // Image preview window
        self.preview.ui_window(ctx);

        // 显示通知
        notify::show(ctx);
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

            // Load settings from database
            let mut db_manager = components::DbManager::default();
            db_manager.ensure_storage();
            
            let settings = if let Some(ref storage) = db_manager.storage {
                if let Ok(kv_map) = futures::block_on_async(async {
                    storage.get_all_settings().await
                }) {
                    Settings::from_kv_map(&kv_map)
                } else {
                    Settings::default()
                }
            } else {
                Settings::default()
            };

            tracing::debug!("settings: {:?}", settings);
            
            // Load state (sessions) from JSON
            let state = AppFrame::load_state().map(|mut s| {
                s.settings = settings.clone();
                s
            }).unwrap_or_else(|| PersistedState {
                current_db_config_id: None,
                settings: settings.clone(),
                sessions: vec![ChatSession::new("0".to_string(), "新会话".to_string())],
                current_index: 0,
                next_session_id: 1,
            });

            tracing::debug!("state: {:?}",state);

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
