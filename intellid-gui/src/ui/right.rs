use crate::IntelliGuiApp;

impl IntelliGuiApp {
    pub fn ui_chat(&mut self, ui: &mut egui::Ui) {
        // 避免可变借用冲突：先取出组件，再归还
        let mut chat = std::mem::take(&mut self.chat);
        chat.ui(self, ui);
        self.chat = chat;
    }
}


