use crate::IntelliGuiApp;
use crate::components::{ChatCtx};

impl IntelliGuiApp {
    pub fn ui_chat(&mut self, ui: &mut egui::Ui) {
        let mut chat = std::mem::take(&mut self.chat);
        let settings = self.state.settings.clone();
        let mut ctxs = ChatCtx { state: &mut self.state, preview: &mut self.preview, send_shortcut: settings.send_shortcut, openai_api_key: settings.openai_api_key.clone(), openai_model: settings.openai_model.clone() };
        chat.ui(&mut ctxs, ui);
        self.chat = chat;
    }
}


