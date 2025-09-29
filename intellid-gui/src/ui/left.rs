use crate::{IntelliGuiApp, models::Session, models::DbConfig};
use egui::{Response};

impl IntelliGuiApp {
    pub fn ui_sessions(&mut self, ui: &mut egui::Ui) {
        ui.heading("Sessions");
        ui.separator();
        if ui.button("+ New Session").clicked() {
            let id = self.state.next_session_id;
            self.state.next_session_id += 1;
            self.state.sessions.push(Session { id, title: format!("Session {}", id), messages: Vec::new(), db: DbConfig { engine: "postgres".to_string(), dsn: String::new() } });
            self.state.current_index = self.state.sessions.len() - 1;
            self.save_state();
            self.ensure_storage();
            if let Some(storage) = &self.storage {
                let sess = intellid_storage::models::Session { id: id.to_string(), title: format!("Session {}", id), config_id: None, created_at: 0, updated_at: 0 };
                let _ = self.rt.block_on(async { storage.create_session(&sess).await });
            }
        }
        ui.separator();
        let items: Vec<(usize, String)> = self.state.sessions.iter().enumerate().map(|(i, s)| (i, s.title.clone())).collect();
        for (idx, title) in items {
            ui.horizontal(|ui| {
                let selected = idx == self.state.current_index;
                if ui.selectable_label(selected, &title).clicked() {
                    self.state.current_index = idx;
                    self.save_state();
                }
                if ui.small_button("Rename").clicked() {
                    self.renaming_index = Some(idx);
                    self.rename_buffer = title.clone();
                }
                if ui.small_button("Delete").clicked() {
                    self.delete_session(idx);
                }
            });
            if self.renaming_index == Some(idx) {
                ui.horizontal(|ui| {
                    let resp = ui.add(egui::TextEdit::singleline(&mut self.rename_buffer));
                    let press_enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if ui.button("Save").clicked() || (resp.lost_focus() && press_enter) {
                        if let Some(s) = self.state.sessions.get_mut(idx) { s.title = self.rename_buffer.trim().to_string(); }
                        self.renaming_index = None;
                        self.save_state();
                    }
                    if ui.button("Cancel").clicked() { self.renaming_index = None; }
                });
            }
        }
    }

    pub fn ui_db_config(&mut self, ui: &mut egui::Ui) {
        self.ensure_storage();
        let mut to_switch: Option<String> = None;
        if let Some(storage) = &self.storage {
            let list = self.rt.block_on(async { storage.list_db_configs(None).await }).unwrap_or_default();
            for cfg in list {
                let icon = egui_phosphor::regular::DATABASE;
                let label = if Some(cfg.id.clone()) == self.active_db_config_id { format!("{} {} (active)", icon, cfg.id) } else { format!("{} {}", icon, cfg.id) };
                let resp: Response = ui.selectable_label(false, label);
                if resp.double_clicked() {
                    to_switch = Some(cfg.id.clone());
                }
            }
        } else {
            ui.label("Storage not ready");
        }

        // confirmation window when switching
        if let Some(target) = to_switch {
            let mut open = true;
            egui::Window::new("Switch Database Config").open(&mut open).collapsible(false).resizable(false).show(ui.ctx(), |ui| {
                ui.label(format!("Switch active DB config to '{}' ?", target));
                ui.horizontal(|ui| {
                    if ui.button("Confirm").clicked() {
                        self.active_db_config_id = Some(target.clone());
                    }
                    if ui.button("Cancel").clicked() {
                        // do nothing
                    }
                });
            });
        }
    }
}


