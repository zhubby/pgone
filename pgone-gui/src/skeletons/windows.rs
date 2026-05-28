use crate::components::SettingsPanel;
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
    Window::new("Settings")
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
    Window::new("About")
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
                    egui::RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).size(14.0),
                );
                ui.add_space(20.0);
                ui.separator();
                ui.add_space(10.0);
                ui.label(egui::RichText::new(
                    "PGone is a local development tool suite focused on database intelligence, including a desktop GUI, MCP Server, and local storage layer, providing:",
                )
                .size(12.0));
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new("• Conversational graphical interface and SQL playground").size(12.0),
                );
                ui.label(
                    egui::RichText::new("• Agent-oriented database capabilities (MCP protocol)").size(12.0),
                );
                ui.label(
                    egui::RichText::new("• Lightweight, locally embeddable persistent storage").size(12.0),
                );
                ui.add_space(20.0);
                if ui.button("Close").clicked() {
                    *show_about = false;
                }
            });
        });
    if !open {
        *show_about = false;
    }
}

fn screen_center(ctx: &Context) -> eframe::egui::Pos2 {
    ctx.content_rect().center()
}
