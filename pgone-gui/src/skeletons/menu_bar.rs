use crate::components::DbManager;
use crate::skeletons::monitors::MonitorMetric;
use eframe::egui::{Context, TopBottomPanel};

pub fn show_menu_bar(
    ctx: &Context,
    db: &mut DbManager,
    left_panel_visible: &mut bool,
    right_panel_visible: &mut bool,
    show_settings: &mut bool,
    show_about: &mut bool,
    show_monitor: &mut Option<MonitorMetric>,
    show_export: &mut bool,
    show_import: &mut bool,
) {
    TopBottomPanel::top("menu_top").show(ctx, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
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
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    ui.close();
                }
            });
            ui.menu_button("View", |ui| {
                if ui.checkbox(left_panel_visible, "Left Panel").changed() {
                    // Panel visibility toggled
                }
                if ui.checkbox(right_panel_visible, "Right Panel").changed() {
                    // Panel visibility toggled
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
        });
    });
}

