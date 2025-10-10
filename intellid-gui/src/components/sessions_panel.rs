use crate::components::SessionsCtx;
use crate::models::{DbConfig, Session};

#[derive(Clone)]
pub struct SessionsPanel {
    pub renaming_index: Option<usize>,
    pub rename_buffer: String,
}

impl Default for SessionsPanel {
    fn default() -> Self {
        Self {
            renaming_index: None,
            rename_buffer: String::new(),
        }
    }
}

impl SessionsPanel {
    pub fn ui(&mut self, ctxs: &mut SessionsCtx, ui: &mut egui::Ui) {
        ui.heading("Sessions");
        ui.separator();
        if ui.button("+ New Session").clicked() {
            let id = ctxs.state.next_session_id;
            ctxs.state.next_session_id += 1;
            ctxs.state.sessions.push(Session {
                id,
                title: format!("Session {}", id),
                messages: Vec::new(),
                db: DbConfig {
                    engine: "postgres".to_string(),
                    dsn: String::new(),
                },
            });
            ctxs.state.current_index = ctxs.state.sessions.len() - 1;
            // persist via storage if available
            ctxs.db.ensure_storage();
            if let Some(storage) = &ctxs.db.storage {
                let sess = intellid_storage::models::Session {
                    id: id.to_string(),
                    title: format!("Session {}", id),
                    config_id: None,
                    created_at: 0,
                    updated_at: 0,
                };
                let _ = ctxs
                    .db
                    .rt
                    .block_on(async { storage.create_session(&sess).await });
            }
        }
        ui.separator();
        let items: Vec<(usize, String)> = ctxs
            .state
            .sessions
            .iter()
            .enumerate()
            .map(|(i, s)| (i, s.title.clone()))
            .collect();
        for (idx, title) in items {
            ui.horizontal(|ui| {
                let selected = idx == ctxs.state.current_index;
                if ui.selectable_label(selected, &title).clicked() {
                    ctxs.state.current_index = idx;
                }
                if ui.small_button("Rename").clicked() {
                    self.renaming_index = Some(idx);
                    self.rename_buffer = title.clone();
                }
                if ui.small_button("Delete").clicked() {}
            });
            if self.renaming_index == Some(idx) {
                ui.horizontal(|ui| {
                    let resp = ui.add(egui::TextEdit::singleline(&mut self.rename_buffer));
                    let press_enter = if resp.has_focus() {
                        let input = ui.input(|i| i.clone());
                        let enter_pressed = input.key_pressed(egui::Key::Enter);
                        let shift_pressed = input.modifiers.shift;
                        enter_pressed && !shift_pressed
                    } else {
                        false
                    };
                    if ui.button("Save").clicked() || (resp.lost_focus() && press_enter) {
                        if let Some(s) = ctxs.state.sessions.get_mut(idx) {
                            s.title = self.rename_buffer.trim().to_string();
                        }
                        self.renaming_index = None;
                    }
                    if ui.button("Cancel").clicked() {
                        self.renaming_index = None;
                    }
                });
            }
        }
    }
}
