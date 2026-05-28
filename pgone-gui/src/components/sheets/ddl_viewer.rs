use super::DdlViewerTab;

pub fn ui(ui: &mut egui::Ui, tab: &DdlViewerTab) {
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button(egui_phosphor::regular::COPY)
                .on_hover_text("Copy DDL")
                .clicked()
            {
                ui.ctx().copy_text(tab.ddl.clone());
            }
        });
    });

    ui.separator();

    let ddl = tab.ddl.clone();
    let mut display = ddl.clone();
    let available_height = ui.available_height().max(120.0);

    ui.horizontal(|ui| {
        ui.add_space(5.0);
        ui.add_sized(
            egui::vec2((ui.available_width() - 5.0).max(0.0), available_height),
            egui::TextEdit::multiline(&mut display)
                .desired_rows((available_height / 20.0) as usize)
                .interactive(false)
                .layouter(&mut move |ui, _text, wrap_width| {
                    let mut job = crate::sql::highlight_sql(&ddl, ui.visuals());
                    job.wrap.max_width = wrap_width;
                    ui.fonts_mut(|fonts| fonts.layout_job(job))
                }),
        );
    });
}
