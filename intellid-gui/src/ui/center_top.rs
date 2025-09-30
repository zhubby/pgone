use crate::IntelliGuiApp;

impl IntelliGuiApp {
    pub fn ui_sql_editor(&mut self, ui: &mut egui::Ui) {
        let mut sql = std::mem::take(&mut self.sql);
        sql.ui_editor(self, ui);
        self.sql = sql;
    }
}


