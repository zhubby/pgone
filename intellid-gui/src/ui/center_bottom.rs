use crate::IntelliGuiApp;

impl IntelliGuiApp {
    pub fn ui_results(&mut self, ui: &mut egui::Ui) {
        let mut sql = std::mem::take(&mut self.sql);
        sql.ui_results(ui);
        self.sql = sql;
    }
}


