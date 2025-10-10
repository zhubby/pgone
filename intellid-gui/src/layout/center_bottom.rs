use crate::IntelliGuiApp;
use crate::components::SqlCtx;

impl IntelliGuiApp {
    pub fn ui_results(&mut self, ui: &mut egui::Ui) {
        let mut sql = std::mem::take(&mut self.sql);
        // Results 仅展示数据，不需要 ctxs
        sql.ui_results(ui);
        self.sql = sql;
    }
}


