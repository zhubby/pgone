use crate::IntelliGuiApp;
use crate::components::SessionsCtx;

impl IntelliGuiApp {
    pub fn ui_sessions(&mut self, ui: &mut egui::Ui) {
        let mut sessions = std::mem::take(&mut self.sessions);
        let mut ctxs = SessionsCtx { state: &mut self.state, db: &mut self.db };
        sessions.ui(&mut ctxs, ui);
        self.sessions = sessions;
    }

    pub fn ui_db_config(&mut self, ui: &mut egui::Ui) {
        // 仍保留 db_manager 的 app-free 设计：仅传 ui 与自身
        let mut db = std::mem::take(&mut self.db);
        db.ui_db_config(self, ui);
        self.db = db;
    }
}


