#![allow(clippy::all, dead_code, deprecated, unused_mut)]

use eframe::egui::{self, Context, ThemePreference};
use egui_phosphor::Variant as PhosphorVariant;
use icns::{IconFamily, IconType};
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

mod futures;
mod layout_settings;
mod models;
use models::*;
mod notify;
pub mod sql;
mod storage;
use storage::SessionStorage;
mod storage_handle;
use storage_handle::GuiStorage;

mod components;
use components::{
    ChatPanel, DbManager, DbTree, ExportWindow, ImportWindow, PreviewManager, ResultsTable,
    SettingsPanel,
};
mod mcp;
mod prompt;
mod settings_store;
use settings_store::SettingsStore;
mod skeletons;
mod styles;

const DOCK_LAYOUT_SAVE_INTERVAL: Duration = Duration::from_millis(750);

fn asset_path(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();

    if let Ok(exe_path) = std::env::current_exe()
        && let Some(resources_dir) = exe_path
            .parent()
            .and_then(Path::parent)
            .map(|contents_dir| contents_dir.join("Resources"))
        && resources_dir.exists()
    {
        return resources_dir.join(path);
    }

    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join(path)
}

fn font_dirs() -> [PathBuf; 2] {
    [asset_path("fonts"), asset_path("")]
}

async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut sigterm = match signal(SignalKind::terminate()) {
            Ok(sigterm) => Some(sigterm),
            Err(e) => {
                tracing::warn!("Failed to register SIGTERM shutdown signal: {}", e);
                None
            }
        };

        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                if let Err(e) = result {
                    tracing::warn!("Failed to listen for Ctrl+C shutdown signal: {}", e);
                }
            }
            _ = async {
                if let Some(sigterm) = sigterm.as_mut() {
                    sigterm.recv().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {}
        }
    }

    #[cfg(not(unix))]
    {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::warn!("Failed to listen for Ctrl+C shutdown signal: {}", e);
        }
    }

    tracing::info!("Shutdown signal received");
}

fn install_shutdown_signal_handler(ctx: Context) {
    futures::spawn(async move {
        wait_for_shutdown_signal().await;
        tracing::info!("Starting GUI shutdown");
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        ctx.request_repaint();
    });
}

pub struct AppFrame {
    #[allow(dead_code)]
    state: PersistedState,
    show_settings: bool,
    show_about: bool,
    db: DbManager,
    results_table: ResultsTable,
    preview: PreviewManager,
    chat: ChatPanel,
    db_tree: DbTree,
    settings_panel: SettingsPanel,
    dock_layout: skeletons::dock::DockLayout,
    last_saved_layout_json: Option<String>,
    last_layout_save_check: Instant,
    pending_theme_preference: Option<ThemePreference>,
    session_storage: SessionStorage,
    gui_storage: GuiStorage,
    sessions_loaded_from_storage: bool,
    show_monitor: Option<skeletons::monitors::MonitorMetric>,
    show_export: bool,
    export_window: ExportWindow,
    show_import: bool,
    import_window: ImportWindow,
    shutdown_complete: bool,
    // mcp_client: Option<McpClientManager>,
}

impl AppFrame {
    fn new(ctx: Context, initial_settings: Settings) -> Self {
        let gui_storage = GuiStorage::new(ctx.clone());
        let mut db_manager = components::DbManager::default();
        db_manager.set_storage(gui_storage.clone());

        // Initialize session storage
        let session_storage = SessionStorage::new(ctx);

        // Load sessions from database
        let mut state = PersistedState {
            current_db_config_id: None,
            settings: initial_settings,
            sessions: vec![],
            current_index: 0,
            next_session_id: 1,
        };

        state.sessions = vec![ChatSession::default_with_timestamp("0".to_string())];

        // No need to initialize extra processes to provide tools
        // Initialize MCP client
        // let mcp_client = if db_manager.storage.is_some() {
        //     // Use pgone_storage::database_path()
        //     let storage_path = pgone_storage::database_path();

        //     // Start MCP server and create client
        //     match futures::block_on_async(async {
        //         McpClientManager::new(storage_path).await
        //     }) {
        //         Ok(client) => {
        //             tracing::info!("MCP client initialized successfully");
        //             Some(client)
        //         }
        //         Err(e) => {
        //             tracing::warn!("MCP client initialization failed: {}", e);
        //             None
        //         }
        //     }
        // } else {
        //     tracing::warn!("Storage unavailable, skipping MCP client initialization");
        //     None
        // };

        Self {
            state,
            show_settings: false,
            show_about: false,
            db: db_manager,
            results_table: Default::default(),
            preview: Default::default(),
            chat: Default::default(),
            db_tree: Default::default(),
            settings_panel: Default::default(),
            dock_layout: layout_settings::load_dock_layout(),
            last_saved_layout_json: None,
            last_layout_save_check: Instant::now(),
            pending_theme_preference: None,
            session_storage,
            gui_storage,
            sessions_loaded_from_storage: false,
            show_monitor: None,
            show_export: false,
            export_window: ExportWindow::default(),
            show_import: false,
            import_window: ImportWindow::default(),
            shutdown_complete: false,
            // mcp_client,
        }
    }

    #[allow(dead_code)]
    fn save_state(&mut self) {
        self.save_settings();
    }

    /// Save settings to the local GUI settings file.
    pub fn save_settings(&mut self) {
        if let Err(error) = SettingsStore::save_app_settings(&self.state.settings) {
            tracing::warn!("Failed to save GUI app settings: {error:#}");
        }
    }

    fn save_dock_layout_if_changed(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.last_layout_save_check) < DOCK_LAYOUT_SAVE_INTERVAL {
            return;
        }
        self.last_layout_save_check = now;

        let current_json = match layout_settings::dock_layout_json(&self.dock_layout) {
            Ok(json) => json,
            Err(error) => {
                tracing::warn!("Failed to serialize dock layout: {}", error);
                return;
            }
        };

        if self.last_saved_layout_json.as_deref() == Some(current_json.as_str()) {
            return;
        }

        match layout_settings::save_dock_layout(&self.dock_layout) {
            Ok(saved_json) => {
                self.last_saved_layout_json = Some(saved_json);
            }
            Err(error) => {
                tracing::warn!("Failed to save dock layout: {error:#}");
            }
        }
    }

    fn apply_storage_updates(&mut self) {
        self.db.process_storage_events();

        if !self.sessions_loaded_from_storage {
            if let Some(loaded_sessions) = self.session_storage.take_loaded_sessions() {
                if !loaded_sessions.is_empty() {
                    self.state.sessions = loaded_sessions;
                    if self.state.current_index >= self.state.sessions.len() {
                        self.state.current_index = 0;
                    }
                    if let Some(max_id) = self
                        .state
                        .sessions
                        .iter()
                        .filter_map(|session| session.id.parse::<u64>().ok())
                        .max()
                    {
                        self.state.next_session_id = max_id + 1;
                    }
                }
                self.sessions_loaded_from_storage = true;
            }
        }
    }

    fn apply_pending_theme_preference(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        if let Some(theme_preference) = self.pending_theme_preference.take() {
            ctx.set_theme(theme_preference);
            ui.set_style(ctx.style());
            ctx.request_repaint();
        }
    }

    fn show_export_window(&mut self, ctx: &Context) {
        if !self.show_export {
            return;
        }

        let mut open = true;
        egui::Window::new("Export Data")
            .id(egui::Id::new("export_window"))
            .open(&mut open)
            .default_pos(ctx.content_rect().center())
            .pivot(egui::Align2::CENTER_CENTER)
            .default_size([550.0, 600.0])
            .show(ctx, |ui| {
                self.export_window.check_export_progress();
                self.export_window.ui(ui, &mut self.db);
            });

        if !open {
            if self.export_window.is_exporting() {
                self.export_window.cancel_export();
            } else {
                self.export_window = ExportWindow::default();
            }
            self.show_export = false;
        }
    }

    fn show_import_window(&mut self, ctx: &Context) {
        if !self.show_import {
            return;
        }

        let mut open = true;
        egui::Window::new("Import Data")
            .id(egui::Id::new("import_window"))
            .open(&mut open)
            .default_pos(ctx.content_rect().center())
            .pivot(egui::Align2::CENTER_CENTER)
            .default_size([600.0, 700.0])
            .show(ctx, |ui| {
                self.import_window.check_import_progress();
                self.import_window.ui(ui, &mut self.db);
            });

        if !open {
            if self.import_window.is_importing() {
                self.import_window.cancel_import();
            } else {
                self.import_window = ImportWindow::default();
            }
            self.show_import = false;
        }
    }

    pub fn shutdown(&mut self) {
        if self.shutdown_complete {
            return;
        }

        self.shutdown_complete = true;
        tracing::info!("Shutting down GUI resources");
        self.db.shutdown();
        tracing::info!("GUI resources shut down successfully");
    }
}

impl eframe::App for AppFrame {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let mut reset_dock_layout = false;
        self.apply_pending_theme_preference(ui, &ctx);
        self.apply_storage_updates();

        // Menu bar
        skeletons::menu_bar::show_menu_bar(
            ui,
            frame,
            &mut self.db,
            &mut reset_dock_layout,
            &mut self.show_settings,
            &mut self.show_about,
            &mut self.show_monitor,
            &mut self.show_export,
            &mut self.show_import,
        );

        if reset_dock_layout {
            self.dock_layout.reset();
            self.last_saved_layout_json = None;
            self.last_layout_save_check = Instant::now() - DOCK_LAYOUT_SAVE_INTERVAL;
        }

        // Status bar
        if let Some(theme_preference) =
            skeletons::status_bar::show_status_bar(ui, &ctx, &mut self.db, &self.state.settings)
        {
            self.pending_theme_preference = Some(theme_preference);
        }

        // Database management windows
        self.db.ui_add_db_window(&ctx);
        self.db.ui_delete_confirm_window(&ctx);
        self.db.ui_edit_db_window(&ctx);

        // Settings window
        if skeletons::windows::show_settings_window(
            &ctx,
            &mut self.show_settings,
            &mut self.state,
            &mut self.settings_panel,
        ) {
            // Save settings to database if changed
            self.save_settings();
        }

        // About window
        skeletons::windows::show_about_window(&ctx, &mut self.show_about);

        // Monitor window
        skeletons::monitors::window::show_monitor_window(
            &ctx,
            &mut self.show_monitor,
            &mut self.db,
        );

        // Check for pending graph window open
        if let Some(schema_info) = self.db_tree.take_pending_open_graph() {
            if let Some(dsn) = self.db.dsn_for_database(&schema_info.0) {
                self.results_table
                    .open_graph_viewer(schema_info.0, schema_info.1, dsn);
            }
        }

        // Export window
        self.show_export_window(&ctx);

        // Import window
        self.show_import_window(&ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(&ctx.style()).inner_margin(0.))
            .show_inside(ui, |ui| {
                self.dock_layout.ui(
                    ui,
                    &mut self.db_tree,
                    &mut self.db,
                    &mut self.results_table,
                    &mut self.chat,
                    &mut self.state,
                    &mut self.preview,
                    &mut self.session_storage,
                );
            });

        self.save_dock_layout_if_changed();

        // Image preview window
        self.preview.ui_window(&ctx);

        // Show notifications
        notify::show(&ctx);
    }

    fn on_exit(&mut self) {
        self.shutdown();
    }
}

impl Drop for AppFrame {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub fn run() -> anyhow::Result<()> {
    let file = BufReader::new(File::open(asset_path("icon.icns"))?);
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
            .with_app_id("com.github.zhubby.pgone")
            .with_maximized(true)
            .with_icon(icon)
            .with_decorations(false),
        ..Default::default()
    };

    eframe::run_native(
        title,
        native_options,
        Box::new(|cc| {
            // Inject phosphor font once at creation to avoid runtime deadlocks
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, PhosphorVariant::Regular);
            install_shutdown_signal_handler(cc.egui_ctx.clone());

            // Load all fonts from the crate assets directories.
            let mut loaded_fonts = Vec::new();

            for fonts_dir in font_dirs() {
                if let Ok(entries) = fs::read_dir(fonts_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                            if ext == "ttf" || ext == "otf" {
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
                }
            }

            let settings = SettingsStore::load_app_settings();
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
            cc.egui_ctx.all_styles_mut(|style| {
                for text_style in style.text_styles.values_mut() {
                    text_style.size = font_size;
                }
            });

            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(AppFrame::new(cc.egui_ctx.clone(), settings)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {}", e))
}
