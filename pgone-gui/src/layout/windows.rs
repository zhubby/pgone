use crate::components::SettingsPanel;
use crate::models::{PersistedState, Settings};
use eframe::egui::{Align2, Context, Window};

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
    let mut settings_changed = false;
    Window::new("设置")
        .open(&mut open)
        .default_pos(screen_center(ctx))
        .pivot(Align2::CENTER_CENTER)
        .default_size([400.0, 300.0])
        .show(ctx, |ui| {
            let old_settings: Settings = state.settings.clone();
            settings_panel.ui(ui, &mut state.settings, ctx);
            if state.settings.font_family != old_settings.font_family
                || state.settings.font_size != old_settings.font_size
            {
                settings_changed = true;
            }
        });
    if !open {
        *show_settings = false;
    }
    settings_changed
}

pub fn show_about_window(ctx: &Context, show_about: &mut bool) {
    if !*show_about {
        return;
    }

    let mut open = true;
    Window::new("关于")
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

fn screen_center(ctx: &Context) -> eframe::egui::Pos2 {
    ctx.screen_rect().center()
}

