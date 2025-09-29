use crate::IntelliGuiApp;

impl IntelliGuiApp {
    pub fn ui_sql_editor(&mut self, ui: &mut egui::Ui) {
        ui.heading("SQL Editor");
        ui.separator();
        let current_sql = self.sql_input.clone();
        let editor = ui.add(egui::TextEdit::multiline(&mut self.sql_input)
            .desired_rows(8)
            .layouter(&mut move |ui, _text, wrap_width| {
                let mut job = crate::sql::highlight_sql(&current_sql, ui.visuals());
                job.wrap.max_width = wrap_width;
                ui.fonts(|f| f.layout_job(job))
            })
        );
        ui.horizontal(|ui| {
            if ui.button("Check").clicked() { self.check_sql(); }
            if ui.button("Run").clicked() { self.run_sql(); }
        });
        if let Some(err) = &self.sql_error { ui.colored_label(egui::Color32::RED, err); }
        if editor.changed() { self.sql_error = None; }
    }
}


