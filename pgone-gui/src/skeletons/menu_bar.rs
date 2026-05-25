use crate::components::DbManager;
use crate::skeletons::monitors::MonitorMetric;
use eframe::egui::{Panel, Ui};

pub fn show_menu_bar(
    root_ui: &mut Ui,
    db: &mut DbManager,
    reset_dock_layout: &mut bool,
    show_settings: &mut bool,
    show_about: &mut bool,
    show_monitor: &mut Option<MonitorMetric>,
    show_export: &mut bool,
    show_import: &mut bool,
) {
    Panel::top("menu_top").show_inside(root_ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.label(egui::RichText::new("PGone").strong());
            ui.separator();

            ui.menu_button("File", |ui| {
                if ui.button("New Connection").clicked() {
                    db.show_add_db = true;
                    ui.close();
                }
                if ui.button("Manage Connections").clicked() {
                    db.show_manage_db = true;
                    ui.close();
                }
                ui.separator();
                if ui.button("Export").clicked() {
                    *show_export = true;
                    ui.close();
                }
                if ui.button("Import").clicked() {
                    *show_import = true;
                    ui.close();
                }

                ui.separator();

                if ui.button("Preferences").clicked() {
                    *show_settings = true;
                    ui.close();
                }

                ui.separator();

                if ui.button("Exit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    ui.close();
                }
            });
            ui.menu_button("View", |ui| {
                if ui.button("Reset Layout").clicked() {
                    *reset_dock_layout = true;
                    ui.close();
                }
                ui.separator();
                if ui.button("Clear Current Session").clicked() {
                    // self.clear_current_session();
                    ui.close();
                }
            });
            ui.menu_button("Monitor", |ui| {
                if ui.button("Activity").clicked() {
                    *show_monitor = Some(MonitorMetric::Activity);
                    ui.close();
                }
                if ui.button("Statements").clicked() {
                    *show_monitor = Some(MonitorMetric::Statements);
                    ui.close();
                }
                if ui.button("Tables").clicked() {
                    *show_monitor = Some(MonitorMetric::Tables);
                    ui.close();
                }
                if ui.button("Indexes").clicked() {
                    *show_monitor = Some(MonitorMetric::Indexes);
                    ui.close();
                }
                if ui.button("Bgwriter").clicked() {
                    *show_monitor = Some(MonitorMetric::Bgwriter);
                    ui.close();
                }
                if ui.button("Replication").clicked() {
                    *show_monitor = Some(MonitorMetric::Replication);
                    ui.close();
                }
                if ui.button("Locks").clicked() {
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
                if ui.button("About").clicked() {
                    *show_about = true;
                    ui.close();
                }
                if ui.button("Project Address").clicked() {
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
                if drag_response.drag_started() {
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
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
            });
        });
    });
}
