use crate::IntelliGuiApp;

impl IntelliGuiApp {
    pub fn ui_results(&mut self, ui: &mut egui::Ui) {
        ui.heading("Results");
        ui.separator();
        if self.query_columns.is_empty() {
            ui.label("No results");
        } else {
            if ui.button("Export CSV...").clicked() { self.export_csv(); }
            let mut table = egui_extras::TableBuilder::new(ui).striped(true).cell_layout(egui::Layout::left_to_right(egui::Align::Center));
            for _ in &self.query_columns { table = table.column(egui_extras::Column::auto()); }
            table.header(20.0, |mut header| {
                for col in &self.query_columns { header.col(|ui| { ui.strong(col); }); }
            }).body(|mut body| {
                for row in &self.query_rows {
                    body.row(18.0, |mut r| {
                        for cell in row { r.col(|ui| { ui.label(cell); }); }
                    });
                }
            });
        }
    }
}


