use super::ResultsTable;

pub fn ui(ui: &mut egui::Ui, results_table: &mut ResultsTable, id: u64) {
    let mut send_to_editor = false;
    let mut copy_sql = None;

    if let Some(tab) = results_table.sql_draft_tabs.get_mut(&id) {
        ui.horizontal(|ui| {
            ui.label(format!("Database: {}", tab.database));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(egui_phosphor::regular::COPY)
                    .on_hover_text("Copy SQL")
                    .clicked()
                {
                    copy_sql = Some(tab.sql.clone());
                }
                if ui
                    .button(egui_phosphor::regular::ARROW_SQUARE_OUT)
                    .on_hover_text("Use in SQL editor")
                    .clicked()
                {
                    send_to_editor = true;
                }
            });
        });

        ui.separator();

        let current_sql = tab.sql.clone();
        let available_height = ui.available_height().max(120.0);
        ui.horizontal(|ui| {
            ui.add_space(5.0);
            ui.add_sized(
                egui::vec2((ui.available_width() - 5.0).max(0.0), available_height),
                egui::TextEdit::multiline(&mut tab.sql)
                    .id(ui.make_persistent_id(("sql_draft", id)))
                    .desired_rows((available_height / 20.0) as usize)
                    .layouter(&mut move |ui, _text, wrap_width| {
                        let mut job = crate::sql::highlight_sql(&current_sql, ui.visuals());
                        job.wrap.max_width = wrap_width;
                        ui.fonts_mut(|fonts| fonts.layout_job(job))
                    }),
            );
        });
    }

    if let Some(sql) = copy_sql {
        ui.ctx().copy_text(sql);
    }

    if send_to_editor && let Some(tab) = results_table.sql_draft_tabs.get(&id) {
        results_table.sql_input = tab.sql.clone();
        results_table.selected_database = Some(tab.database.clone());
        results_table.sql_error = None;
    }
}
