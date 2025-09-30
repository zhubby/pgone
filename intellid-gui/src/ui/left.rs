use crate::IntelliGuiApp;
use egui::Response;

impl IntelliGuiApp {
    pub fn ui_sessions(&mut self, ui: &mut egui::Ui) {
        let mut sessions = std::mem::take(&mut self.sessions);
        sessions.ui(self, ui);
        self.sessions = sessions;
    }

    pub fn ui_db_config(&mut self, ui: &mut egui::Ui) {
        let mut db = std::mem::take(&mut self.db);
        db.ui_db_config(self, ui);
        self.db = db;
    }
}


