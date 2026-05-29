use crate::components::DbManager;
use crate::models::PersistedState;
use crate::skeletons::dock::{DockLayout, DockPanel};
use crate::skeletons::monitors::MonitorMetric;
use eframe::egui::{Panel, Ui};
#[cfg(target_os = "macos")]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

pub fn show_menu_bar(
    root_ui: &mut Ui,
    frame: &eframe::Frame,
    db: &mut DbManager,
    dock_layout: &mut DockLayout,
    state: &mut PersistedState,
    reset_dock_layout: &mut bool,
    show_settings: &mut bool,
    show_about: &mut bool,
    show_monitor: &mut Option<MonitorMetric>,
    show_export: &mut bool,
    show_import: &mut bool,
) {
    Panel::top("menu_top").show_inside(root_ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if menu_button(ui, egui_phosphor::regular::DATABASE, "New Connection").clicked() {
                    db.show_add_db = true;
                    ui.close();
                }
                ui.separator();
                if menu_button(ui, egui_phosphor::regular::EXPORT, "Export").clicked() {
                    *show_export = true;
                    ui.close();
                }
                if menu_button(ui, egui_phosphor::regular::DOWNLOAD_SIMPLE, "Import").clicked() {
                    *show_import = true;
                    ui.close();
                }

                ui.separator();

                if menu_button(ui, egui_phosphor::regular::GEAR, "Preferences").clicked() {
                    *show_settings = true;
                    ui.close();
                }

                ui.separator();

                if menu_button(ui, egui_phosphor::regular::SIGN_OUT, "Exit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    ui.close();
                }
            });
            ui.menu_button("View", |ui| {
                if toggle_panel_button(
                    ui,
                    dock_layout,
                    state,
                    DockPanel::Structure,
                    egui_phosphor::regular::TREE_STRUCTURE,
                    "Structure",
                )
                .clicked()
                {
                    ui.close();
                }
                if toggle_panel_button(
                    ui,
                    dock_layout,
                    state,
                    DockPanel::Agent,
                    egui_phosphor::regular::SPARKLE,
                    "Agent",
                )
                .clicked()
                {
                    ui.close();
                }
                if toggle_panel_button(
                    ui,
                    dock_layout,
                    state,
                    DockPanel::Sql,
                    egui_phosphor::regular::CODE,
                    "SQL",
                )
                .clicked()
                {
                    ui.close();
                }
                if toggle_panel_button(
                    ui,
                    dock_layout,
                    state,
                    DockPanel::Results,
                    egui_phosphor::regular::TABLE,
                    "Results",
                )
                .clicked()
                {
                    ui.close();
                }
                ui.separator();
                if menu_button(ui, egui_phosphor::regular::LAYOUT, "Reset Layout").clicked() {
                    *reset_dock_layout = true;
                    ui.close();
                }
                ui.separator();
                if menu_button(
                    ui,
                    egui_phosphor::regular::CHAT_CIRCLE_TEXT,
                    "Clear Current Session",
                )
                .clicked()
                {
                    // self.clear_current_session();
                    ui.close();
                }
            });
            ui.menu_button("Monitor", |ui| {
                if menu_button(ui, egui_phosphor::regular::ACTIVITY, "Activity").clicked() {
                    *show_monitor = Some(MonitorMetric::Activity);
                    ui.close();
                }
                if menu_button(ui, egui_phosphor::regular::FILE_TEXT, "Statements").clicked() {
                    *show_monitor = Some(MonitorMetric::Statements);
                    ui.close();
                }
                if menu_button(ui, egui_phosphor::regular::TABLE, "Tables").clicked() {
                    *show_monitor = Some(MonitorMetric::Tables);
                    ui.close();
                }
                if menu_button(ui, egui_phosphor::regular::LIST_MAGNIFYING_GLASS, "Indexes")
                    .clicked()
                {
                    *show_monitor = Some(MonitorMetric::Indexes);
                    ui.close();
                }
                if menu_button(ui, egui_phosphor::regular::PENCIL_SIMPLE_LINE, "Bgwriter").clicked()
                {
                    *show_monitor = Some(MonitorMetric::Bgwriter);
                    ui.close();
                }
                if menu_button(ui, egui_phosphor::regular::ARROWS_CLOCKWISE, "Replication")
                    .clicked()
                {
                    *show_monitor = Some(MonitorMetric::Replication);
                    ui.close();
                }
                if menu_button(ui, egui_phosphor::regular::LOCK, "Locks").clicked() {
                    *show_monitor = Some(MonitorMetric::Locks);
                    ui.close();
                }
            });
            // ui.menu_button("Settings", |ui| {
            //     if ui.button("Open Settings").clicked() {
            //         *show_settings = true;
            //         ui.close();
            //     }
            // });
            ui.menu_button("Help", |ui| {
                if menu_button(ui, egui_phosphor::regular::INFO, "About").clicked() {
                    *show_about = true;
                    ui.close();
                }
                if menu_button(ui, egui_phosphor::regular::GITHUB_LOGO, "Project Address").clicked()
                {
                    let _ = webbrowser::open("https://github.com/zhubby/pgone");
                    ui.close();
                }
            });

            ui.add_space(8.0);
            let control_button_width = 3.0 * ui.spacing().interact_size.x
                + 2.0 * ui.spacing().item_spacing.x
                + ui.spacing().item_spacing.x;
            let mut drag_rect = ui.available_rect_before_wrap();
            drag_rect.max.x = (drag_rect.max.x - control_button_width).max(drag_rect.min.x);

            if drag_rect.is_positive() {
                let drag_response = ui.interact(
                    drag_rect,
                    ui.id().with("title_bar_drag_region"),
                    egui::Sense::click_and_drag(),
                );
                if drag_response.drag_started()
                    && !crate::pointer_in_linux_window_resize_zone(ui.ctx())
                {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(egui_phosphor::regular::X)
                    .on_hover_text("Close")
                    .clicked()
                {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }

                if ui
                    .button(egui_phosphor::regular::SQUARE)
                    .on_hover_text("Maximize / Restore")
                    .clicked()
                {
                    let maximized = ui
                        .ctx()
                        .input(|input| input.viewport().maximized.unwrap_or(false));
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                }

                if ui
                    .button(egui_phosphor::regular::MINUS)
                    .on_hover_text("Minimize")
                    .clicked()
                {
                    minimize_window(ui, frame);
                }
            });
        });
    });
}

fn menu_button(ui: &mut Ui, icon: &str, label: &str) -> egui::Response {
    ui.button(format!("{icon} {label}"))
}

fn toggle_panel_button(
    ui: &mut Ui,
    dock_layout: &mut DockLayout,
    state: &mut PersistedState,
    panel: DockPanel,
    icon: &str,
    label: &str,
) -> egui::Response {
    let visible = dock_layout.is_panel_visible(panel);
    let check = if visible {
        egui_phosphor::regular::CHECK
    } else {
        " "
    };
    let response = ui.button(format!("{check} {icon} {label}"));
    if response.clicked() {
        dock_layout.toggle_panel(panel, state);
    }
    response
}

fn minimize_window(ui: &Ui, frame: &eframe::Frame) {
    if !minimize_window_with_platform_api(frame) {
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }
}

#[cfg(target_os = "macos")]
fn minimize_window_with_platform_api(frame: &eframe::Frame) -> bool {
    use objc2_app_kit::NSView;

    let Ok(window_handle) = frame.window_handle() else {
        return false;
    };

    let RawWindowHandle::AppKit(appkit_handle) = window_handle.as_raw() else {
        return false;
    };

    let ns_view_ptr = appkit_handle.ns_view.as_ptr().cast::<NSView>();
    if ns_view_ptr.is_null() {
        return false;
    }

    let Some(ns_view) = (unsafe { ns_view_ptr.as_ref() }) else {
        return false;
    };
    let Some(ns_window) = ns_view.window() else {
        return false;
    };

    ns_window.miniaturize(None);
    true
}

#[cfg(not(target_os = "macos"))]
fn minimize_window_with_platform_api(_: &eframe::Frame) -> bool {
    false
}
