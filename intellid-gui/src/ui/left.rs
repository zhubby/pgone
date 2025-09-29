use crate::{IntelliGuiApp, models::Session, models::DbConfig};

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
        ui.heading("DB Config");
        ui.separator();
        if let Some(sess) = self.state.sessions.get_mut(self.state.current_index) {
            ui.label("Engine");
            ui.text_edit_singleline(&mut sess.db.engine);
            ui.label("DSN");
            let changed = ui.text_edit_singleline(&mut sess.db.dsn).changed();
            if changed { self.save_state(); }
            let sid = self.state.sessions[self.state.current_index].id;
            let connected = self.pools.contains_key(&sid);
            ui.horizontal(|ui| {
                if !connected {
                    if ui.button("Connect").clicked() { self.connect_current_session(); }
                } else {
                    ui.colored_label(egui::Color32::GREEN, "Connected");
                    if ui.button("Disconnect").clicked() { self.disconnect_current_session(); }
                }
            });
        }
    }
}


