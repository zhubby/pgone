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
mod skeletons;
mod styles;
mod media;
mod agents;
mod prompt;

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
        let mut session_storage = SessionStorage::new();
        
        // Load sessions from database
        let mut state = PersistedState {
            current_db_config_id: None,
            settings: settings.clone(),
            sessions: vec![],
            current_index: 0,
            next_session_id: 1,
        };
        
        // Load sessions from database
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
            } else {
                // 如果没有会话，创建一个默认会话
                state.sessions = vec![ChatSession::default_with_timestamp("0".to_string())];
            }
        } else {
            // 如果加载失败，创建一个默认会话
            state.sessions = vec![ChatSession::default_with_timestamp("0".to_string())];
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
        skeletons::menu_bar::show_menu_bar(
            ctx,
            &mut self.db,
            &mut self.left_panel_visible,
            &mut self.right_panel_visible,
            &mut self.show_settings,
            &mut self.show_about,
        );

        // Status bar
        skeletons::status_bar::show_status_bar(ctx, &mut self.db, &self.state.settings);

        // Database management windows
        self.db.ui_add_db_window(ctx);
        self.db.ui_manage_db_window(ctx);
        self.db.ui_edit_db_window(ctx);

        // Settings window
        if skeletons::windows::show_settings_window(
            ctx,
            &mut self.show_settings,
            &mut self.state,
            &mut self.settings_panel,
        ) {
            // Save settings to database if changed
            self.save_settings();
        }

        // About window
        skeletons::windows::show_about_window(ctx, &mut self.show_about);

        // Check for pending graph window open
        if let Some(schema_info) = self.db_tree.take_pending_open_graph() {
            self.show_graph = true;
            self.graph_schema = Some(schema_info.clone());
            // Reinitialize graph with new schema
            self.graph = SchemaGraph::new(schema_info.0.clone(), schema_info.1.clone());
        }

        // Graph window
        skeletons::windows::show_graph_window(
            ctx,
            &mut self.show_graph,
            self.graph_schema.clone(),
            &mut self.db,
            &mut self.graph,
        );

        // Panels
        skeletons::panels::show_left_panel(
            ctx,
            self.left_panel_visible,
            self.left_panel_width,
            &mut self.db_tree,
            &mut self.db,
            &mut self.results_table,
        );

        skeletons::panels::show_right_panel(
            ctx,
            self.right_panel_visible,
            self.right_panel_width,
            &mut self.chat,
            &mut self.state,
            &mut self.preview,
            &mut self.session_storage,
        );

        skeletons::panels::show_center_panel(ctx, &mut self.db, &mut self.results_table, &self.state);

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

            // Set default font family based on settings
            let default_font = &settings.font_family;
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
            let font_size = settings.font_size;
            let mut style = (*cc.egui_ctx.style()).clone();
            for text_style in style.text_styles.values_mut() {
                text_style.size = font_size;
            }
            cc.egui_ctx.set_style(style);

            // Apply theme
            SettingsPanel::apply_theme(&cc.egui_ctx, settings.theme);

            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(AppFrame::new()))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {}", e))
}
