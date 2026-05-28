use crate::components::ChatCtx;
use crate::models::ChatSession;

pub struct SessionSelector;

impl SessionSelector {
    pub fn ui(ctxs: &mut ChatCtx, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Session dropdown
            let current_session = ctxs
                .state
                .sessions
                .get(ctxs.state.current_index)
                .map(|s| s.title.clone())
                .unwrap_or_else(|| "No sessions".to_string());

            let mut selected_index = ctxs.state.current_index;
            egui::ComboBox::from_id_salt("session_selector")
                .selected_text(&current_session)
                .width(200.0)
                .show_ui(ui, |ui| {
                    for (idx, session) in ctxs.state.sessions.iter().enumerate() {
                        if ui
                            .selectable_value(&mut selected_index, idx, &session.title)
                            .clicked()
                        {
                            ctxs.state.current_index = idx;
                        }
                    }
                });

            // New Session button
            if ui
                .button(format!("{} New", egui_phosphor::regular::PLUS))
                .clicked()
            {
                Self::create_new_session(ctxs);
            }
        });
    }

    fn create_new_session(ctxs: &mut ChatCtx) {
        let new_id = ctxs.state.next_session_id.to_string();
        ctxs.state.next_session_id += 1;

        let new_session = ChatSession::default_with_timestamp(new_id.clone());

        ctxs.state.sessions.push(new_session.clone());
        ctxs.state.current_index = ctxs.state.sessions.len() - 1;

        // Save to storage
        if let Err(e) = ctxs.storage.save_session(&new_session) {
            tracing::error!("Failed to save new session: {}", e);
        }
    }
}
