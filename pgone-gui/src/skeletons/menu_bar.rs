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
) {
    TopBottomPanel::top("menu_top").show(ctx, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
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
                if ui.button("连接状态").clicked() {
                    *show_monitor = Some(MonitorMetric::Activity);
                    ui.close();
                }
                if ui.button("查询性能").clicked() {
                    *show_monitor = Some(MonitorMetric::Statements);
                    ui.close();
                }
                if ui.button("表读写统计").clicked() {
                    *show_monitor = Some(MonitorMetric::Tables);
                    ui.close();
                }
                if ui.button("索引使用情况").clicked() {
                    *show_monitor = Some(MonitorMetric::Indexes);
                    ui.close();
                }
                if ui.button("资源使用率").clicked() {
                    *show_monitor = Some(MonitorMetric::Bgwriter);
                    ui.close();
                }
                if ui.button("复制状态（主从）").clicked() {
                    *show_monitor = Some(MonitorMetric::Replication);
                    ui.close();
                }
                if ui.button("锁状态").clicked() {
                    *show_monitor = Some(MonitorMetric::Locks);
                    ui.close();
                }
            });
            ui.menu_button("Settings", |ui| {
                if ui.button("Open Settings").clicked() {
                    *show_settings = true;
                    ui.close();
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("关于").clicked() {
                    *show_about = true;
                    ui.close();
                }
                if ui.button("项目地址").clicked() {
                    let _ = webbrowser::open("https://github.com/zhubby/pgone");
                    ui.close();
                }
            });
        });
    });
}

