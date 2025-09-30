use crate::models::{Session, DbConfig};

pub struct SessionsPanel {
    pub renaming_index: Option<usize>,
    pub rename_buffer: String,
}

impl Default for SessionsPanel {
    fn default() -> Self { Self { renaming_index: None, rename_buffer: String::new() } }
}

impl SessionsPanel {
    pub fn ui(&mut self, app: &mut crate::IntelliGuiApp, ui: &mut egui::Ui) {
        ui.heading("Sessions");
        ui.separator();
        if ui.button("+ New Session").clicked() {
            let id = app.state.next_session_id;
            app.state.next_session_id += 1;
            app.state.sessions.push(Session { id, title: format!("Session {}", id), messages: Vec::new(), db: DbConfig { engine: "postgres".to_string(), dsn: String::new() } });
            app.state.current_index = app.state.sessions.len() - 1;
            app.save_state();
            app.db.ensure_storage();
            if let Some(storage) = &app.db.storage {
                let sess = intellid_storage::models::Session { id: id.to_string(), title: format!("Session {}", id), config_id: None, created_at: 0, updated_at: 0 };
                let _ = app.db.rt.block_on(async { storage.create_session(&sess).await });
            }
        }
        ui.separator();
        let items: Vec<(usize, String)> = app.state.sessions.iter().enumerate().map(|(i, s)| (i, s.title.clone())).collect();
        for (idx, title) in items {
            ui.horizontal(|ui| {
                let selected = idx == app.state.current_index;
                if ui.selectable_label(selected, &title).clicked() {
                    app.state.current_index = idx;
                    app.save_state();
                }
                if ui.small_button("Rename").clicked() {
                    self.renaming_index = Some(idx);
                    self.rename_buffer = title.clone();
                }
                if ui.small_button("Delete").clicked() {
                    app.delete_session(idx);
                }
            });
            if self.renaming_index == Some(idx) {
                ui.horizontal(|ui| {
                    let resp = ui.add(egui::TextEdit::singleline(&mut self.rename_buffer));
                    let press_enter = if resp.has_focus() {
                        let input = ui.input(|i| i.clone());
                        let enter_pressed = input.key_pressed(egui::Key::Enter);
                        let shift_pressed = input.modifiers.shift;
                        enter_pressed && !shift_pressed
                    } else { false };
                    if ui.button("Save").clicked() || (resp.lost_focus() && press_enter) {
                        if let Some(s) = app.state.sessions.get_mut(idx) { s.title = self.rename_buffer.trim().to_string(); }
                        self.renaming_index = None;
                        app.save_state();
                    }
                    if ui.button("Cancel").clicked() { self.renaming_index = None; }
                });
            }
        }
    }
}


