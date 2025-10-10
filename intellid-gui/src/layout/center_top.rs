use crate::IntelliGuiApp;
use crate::components::SqlCtx;

impl IntelliGuiApp {
    pub fn ui_sql_editor(&mut self, ui: &mut egui::Ui) {
        let mut sql = std::mem::take(&mut self.sql);
        let mut ctxs = SqlCtx { state: &mut self.state, db: &mut self.db };
        sql.ui_editor(&mut ctxs, ui);
        self.sql = sql;
    }
}


