use chrono::{DateTime, Utc};
use eframe::egui::{self, CentralPanel, Context, SidePanel, TopBottomPanel};
use egui_dock::{DockArea, DockState};
use egui_extras::{Size, StripBuilder};
use egui_phosphor::Variant as PhosphorVariant;
use icns::{IconFamily, IconType, Image};
use serde::Deserialize;
use std::fs;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

mod models;
use models::*;
mod layout;
mod markdown;
mod mcp_client;
mod openai_client;
mod sql;
use layout::tabs::{CenterBottomTab, CenterBottomViewer, CenterTopTab, CenterTopViewer};

use layout::left::LeftTab;
use layout::left::LeftViewer;

use layout::right::RightTab;
use layout::right::RightViewer;

mod components;
use components::{ChatCtx, ChatPanel, DbManager, PreviewManager, SessionsPanel, SqlPanel};
mod media;

pub struct AppFrame {
    state: PersistedState,
    show_settings: bool,
    // components
    sessions: SessionsPanel,
    db: DbManager,
    sql: SqlPanel,
    preview: PreviewManager,
    // Dock trees for sidebars and center (top/bottom)
    left_tree: DockState<LeftTab>,
    right_tree: DockState<RightTab>,
    center_top_tree: DockState<CenterTopTab>,
    center_bottom_tree: DockState<CenterBottomTab>,
}

impl AppFrame {
    const SESSIONS_PATH: &'static str = "sessions.json";

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
        // initialize dock trees
        let left_tree = DockState::new(vec![LeftTab::Sessions, LeftTab::DbConfig]);
        let right_tree = DockState::new(vec![RightTab::Chat]);
        let center_top_tree = DockState::new(vec![CenterTopTab::SqlEditor]);
        let center_bottom_tree = DockState::new(vec![CenterBottomTab::Results]);

        Self {
            state,
            show_settings: false,
            // chat: Default::default(),
            sessions: Default::default(),
            db: Default::default(),
            sql: Default::default(),
            preview: Default::default(),
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

    fn save_state(&self) {
        let _ = fs::write(
            Self::SESSIONS_PATH,
            serde_json::to_string_pretty(&self.state).unwrap_or_default(),
        );
    }

    fn migrate_from_json(&mut self) -> Result<(), String> {
        self.db.ensure_storage();
        let Some(storage) = self.db.storage.as_ref() else {
            return Err("storage missing".into());
        };
        // migrate sessions and messages
        for s in &self.state.sessions {
            let sess = intellid_storage::models::Session {
                id: s.id.to_string(),
                title: s.title.clone(),
                config_id: None,
                created_at: 0,
                updated_at: 0,
            };
            let _ = self
                .db
                .rt
                .block_on(async { storage.create_session(&sess).await });
            for m in &s.messages {
                match &m.content {
                    MessageContent::Markdown(text) => {
                        let _ = self.db.rt.block_on(async {
                            storage
                                .append_markdown(
                                    &sess.id,
                                    intellid_storage::models::Role::User,
                                    text,
                                )
                                .await
                        });
                    }
                    MessageContent::Image {
                        path,
                        width,
                        height,
                    } => {
                        let _ = self.db.rt.block_on(async {
                            storage
                                .append_image(
                                    &sess.id,
                                    intellid_storage::models::Role::User,
                                    &path.display().to_string(),
                                    *width as i64,
                                    *height as i64,
                                )
                                .await
                        });
                    }
                    MessageContent::Video {
                        path, duration_ms, ..
                    } => {
                        let _ = self.db.rt.block_on(async {
                            storage
                                .append_video(
                                    &sess.id,
                                    intellid_storage::models::Role::User,
                                    &path.display().to_string(),
                                    duration_ms.map(|v| v as i64),
                                )
                                .await
                        });
                    }
                }
            }
        }
        Ok(())
    }

    fn now_ts() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }
}

impl eframe::App for AppFrame {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        let Self {
            left_tree,
            right_tree,
            state,
            preview,
            db,
            ..
        } = self;
        // fonts are initialized in run() creation context to avoid runtime deadlocks
        TopBottomPanel::top("menu_top").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Add Image...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Images", &["png", "jpg", "jpeg", "gif", "bmp", "webp"])
                            .pick_file()
                        {

                            // let settings = self.state.settings.clone();
                            // let mut ctxs = ChatCtx {
                            //     state: &mut self.state,
                            //     preview: &mut self.preview,
                            //     send_shortcut: settings.send_shortcut,
                            //     openai_api_key: settings.openai_api_key.clone(),
                            //     openai_model: settings.openai_model.clone(),
                            // };
                            // self.chat.add_image_message(&mut ctxs, path);
                        }
                        ui.close();
                    }
                    if ui.button("Add Video...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Videos", &["mp4", "mov", "m4v", "mkv", "webm"])
                            .pick_file()
                        {
                            // let mut chat = std::mem::take(&mut self.chat);
                            // let settings = self.state.settings.clone();
                            // let mut ctxs = ChatCtx {
                            //     state: &mut self.state,
                            //     preview: &mut self.preview,
                            //     send_shortcut: settings.send_shortcut,
                            //     openai_api_key: settings.openai_api_key.clone(),
                            //     openai_model: settings.openai_model.clone(),
                            // };
                            // chat.add_video_message(&mut ctxs, path);
                            // self.chat = chat;
                        }
                        ui.close();
                    }
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
                ui.add_space(ui.available_width() - 120.0);
                let name = db
                    .active_db_config_id
                    .clone()
                    .unwrap_or_else(|| "<no db>".to_string());
                ui.label(format!("DB: {}", name));
            });
        });

        // fixed three-column layout; no edge toggle buttons

        // Settings window
        {
            db.ui_add_db_window(ctx);
        }

        {
            db.ui_manage_db_window(ctx);
        }
        if self.show_settings {
            let mut open = true;
            egui::Window::new("Settings")
                .open(&mut open)
                .show(ctx, |ui| {
                    // ui.heading("Appearance");
                    // let mut dark = self.state.settings.dark_theme;
                    // if ui.checkbox(&mut dark, "Dark theme").clicked() {
                    //     self.state.settings.dark_theme = dark;
                    //     if dark {
                    //         ctx.set_visuals(egui::Visuals::dark());
                    //     } else {
                    //         ctx.set_visuals(egui::Visuals::light());
                    //     }
                    //     self.save_state();
                    // }
                    // ui.separator();
                    // ui.heading("Send Shortcut");
                    // let mut sc = self.state.settings.send_shortcut;
                    // if ui
                    //     .radio_value(&mut sc, SendShortcut::Enter, "Enter")
                    //     .clicked()
                    // {
                    //     self.state.settings.send_shortcut = sc;
                    //     self.save_state();
                    // }
                    // if ui
                    //     .radio_value(&mut sc, SendShortcut::CmdEnter, "Cmd+Enter")
                    //     .clicked()
                    // {
                    //     self.state.settings.send_shortcut = sc;
                    //     self.save_state();
                    // }
                    // ui.separator();
                    // ui.heading("OpenAI");
                    // let mut key = self
                    //     .state
                    //     .settings
                    //     .openai_api_key
                    //     .clone()
                    //     .unwrap_or_default();
                    // if ui
                    //     .add(egui::TextEdit::singleline(&mut key).hint_text("API Key"))
                    //     .changed()
                    // {
                    //     if key.trim().is_empty() {
                    //         self.state.settings.openai_api_key = None;
                    //     } else {
                    //         self.state.settings.openai_api_key = Some(key.clone());
                    //     }
                    //     self.save_state();
                    // }
                    // ui.horizontal(|ui| {
                    //     ui.label("Model");
                    //     let changed = ui
                    //         .text_edit_singleline(&mut self.state.settings.openai_model)
                    //         .changed();
                    //     if changed {
                    //         self.save_state();
                    //     }
                    // });
                });
            if !open {
                self.show_settings = false;
            }
        }

        // 左栏：使用 Dock Tabs（Sessions/DB Config）
        SidePanel::left("left_panel")
            .resizable(true)
            .min_width(220.0)
            .show(ctx, |ui| {
                let mut viewer = LeftViewer {};
                DockArea::new(left_tree).show_inside(ui, &mut viewer);
            });

        // // 中栏：上下分别为 Dock tabs（SQL / Results）
        // CentralPanel::default().show(ctx, |ui| {
        //     StripBuilder::new(ui)
        //         .size(Size::relative(0.55)) // editor area
        //         .size(Size::remainder()) // results
        //         .vertical(|mut strip| {
        //             strip.cell(|ui| {
        //                 let mut tmp = DockState::new(Vec::new());
        //                 std::mem::swap(&mut self.center_top_tree, &mut tmp);
        //                 ui.push_id("center_top_dock", |ui| {
        //                     let mut viewer = CenterTopViewer { app: self };
        //                     DockArea::new(&mut tmp).show_inside(ui, &mut viewer);
        //                 });
        //                 std::mem::swap(&mut self.center_top_tree, &mut tmp);
        //             });
        //             strip.cell(|ui| {
        //                 let mut tmp = DockState::new(Vec::new());
        //                 std::mem::swap(&mut self.center_bottom_tree, &mut tmp);
        //                 ui.push_id("center_bottom_dock", |ui| {
        //                     let mut viewer = CenterBottomViewer { app: self };
        //                     DockArea::new(&mut tmp).show_inside(ui, &mut viewer);
        //                 });
        //                 std::mem::swap(&mut self.center_bottom_tree, &mut tmp);
        //             });
        //         });
        // });

        // 右栏：Dock tabs（Chat）
        SidePanel::right("right_panel")
            .resizable(true)
            .min_width(260.0)
            .show(ctx, |ui| {
                let mut viewer = RightViewer {
                    preview: self.preview.clone(),
                    chat: ChatPanel::default(),
                    state: self.state.clone(),
                };
                DockArea::new(right_tree).show_inside(ui, &mut viewer);
            });

        // Image preview window
        self.preview.ui_window(ctx);
    }
}

impl AppFrame {
    fn clear_current_session(&mut self) {
        if let Some(s) = self.state.sessions.get_mut(self.state.current_index) {
            s.messages.clear();
            self.save_state();
        }
    }
}

impl AppFrame {
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
    let title = "Intelligent Database";
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