use crate::components::{DbManager, SchemaGraph, SettingsPanel};
use crate::models::PersistedState;
use eframe::egui::{Align2, Context, Id, Window};

pub fn show_settings_window(
    ctx: &Context,
    show_settings: &mut bool,
    state: &mut PersistedState,
    settings_panel: &mut SettingsPanel,
) -> bool {
    if !*show_settings {
        return false;
    }

    let mut open = true;
    let mut should_save = false;
    Window::new("设置")
        .id(Id::new("settings_window"))
        .open(&mut open)
        .default_pos(screen_center(ctx))
        .pivot(Align2::CENTER_CENTER)
        .default_size([560.0, 500.0])
        .show(ctx, |ui| {
            // Initialize original settings on first show
            if !settings_panel.has_original_settings() {
                settings_panel.init_original_settings(&state.settings);
            }

            // Render UI and check if save button was clicked
            if settings_panel.ui(ui, &mut state.settings, ctx) {
                should_save = true;
            }
        });
    if !open {
        *show_settings = false;
        // Reset original settings when closing
        settings_panel.clear_original_settings();
    }
    should_save
}

pub fn show_about_window(ctx: &Context, show_about: &mut bool) {
    if !*show_about {
        return;
    }

    let mut open = true;
    Window::new("关于")
        .id(Id::new("about_window"))
        .open(&mut open)
        .default_pos(screen_center(ctx))
        .pivot(Align2::CENTER_CENTER)
        .default_size([500.0, 300.0])
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.heading("PGone");
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new(format!("版本 {}", env!("CARGO_PKG_VERSION"))).size(14.0),
                );
                ui.add_space(20.0);
                ui.separator();
                ui.add_space(10.0);
                ui.label(egui::RichText::new(
                    "PGone 是一套围绕数据库智能化的本地开发工具集，包含桌面 GUI、MCP Server 以及本地存储层，旨在提供：",
                )
                .size(12.0));
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new("• 会话式图形交互与 SQL 试验台").size(12.0),
                );
                ui.label(
                    egui::RichText::new("• 面向 Agent 的数据库能力暴露（MCP 协议）").size(12.0),
                );
                ui.label(
                    egui::RichText::new("• 轻量、本地可嵌入的持久化存储").size(12.0),
                );
                ui.add_space(20.0);
                if ui.button("关闭").clicked() {
                    *show_about = false;
                }
            });
        });
    if !open {
        *show_about = false;
    }
}

pub fn show_graph_window(
    ctx: &Context,
    show_graph: &mut bool,
    schema_info: Option<(String, String)>, // (database, schema)
    db_manager: &mut DbManager,
    graph: &mut SchemaGraph,
) {
    if !*show_graph {
        return;
    }

    let mut open = true;
    let title = if let Some((_database, _schema)) = &schema_info {
        format!("Schema Graph: {}.{}", _database, _schema)
    } else {
        "Schema Graph".to_string()
    };

    // Get DSN before opening window to avoid borrow checker issues
    let dsn = schema_info
        .as_ref()
        .and_then(|(_database, _schema)| db_manager.active_dsn());
    let pools = db_manager.pools.clone();

    Window::new(title)
        .id(Id::new("schema_graph_window"))
        .open(&mut open)
        .default_pos(screen_center(ctx))
        .pivot(Align2::CENTER_CENTER)
        .default_size([400.0, 600.0])
        .show(ctx, |ui| {
            if schema_info.is_some() {
                graph.ui(ui, pools.clone(), dsn.as_deref());
            } else {
                ui.label("请选择一个 schema");
            }
        });

    if !open {
        *show_graph = false;
    }
}

fn screen_center(ctx: &Context) -> eframe::egui::Pos2 {
    ctx.content_rect().center()
}
