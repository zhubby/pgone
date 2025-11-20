use crate::components::SqlCtx;
use sqlx::postgres::{PgPoolOptions, PgRow};
use sqlx::{Column, Row};

#[derive(Default)]
pub struct SqlPanel {
    pub sql_input: String,
    pub sql_error: Option<String>,
    pub query_columns: Vec<String>,
    pub query_rows: Vec<Vec<String>>,
}

// Default is derived

impl SqlPanel {
    pub fn ui_editor(&mut self, ctxs: &mut SqlCtx, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(format!("{} SQL Editor", egui_phosphor::regular::QUESTION));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(egui_phosphor::regular::PLAY)
                            .min_size(egui::vec2(28.0, 28.0)),
                    )
                    .clicked()
                {
                    self.run_sql(ctxs);
                }
                ui.add_space(8.0);
                if ui
                    .add(
                        egui::Button::new(egui_phosphor::regular::CHECK)
                            .min_size(egui::vec2(28.0, 28.0)),
                    )
                    .clicked()
                {
                    self.check_sql();
                }
            });
        });
        ui.separator();
        let current_sql = self.sql_input.clone();
        let available_height = ui.available_height() - 10.0;
        let editor = ui.add(
            egui::TextEdit::multiline(&mut self.sql_input)
                .desired_rows((available_height / 20.0) as usize)
                .layouter(&mut move |ui, _text, wrap_width| {
                    let mut job = crate::sql::highlight_sql(&current_sql, ui.visuals());
                    job.wrap.max_width = wrap_width;
                    ui.fonts(|f| f.layout_job(job))
                }),
        );
        if let Some(err) = &self.sql_error {
            ui.colored_label(egui::Color32::RED, err);
        }
        if editor.changed() {
            self.sql_error = None;
        }
    }

    pub fn ui_results(&mut self, ui: &mut egui::Ui) {
        ui.heading("Results");
        ui.separator();
        if self.query_columns.is_empty() {
            ui.label("No results");
            return;
        }
        if ui.button("Export CSV...").clicked() {
            self.export_csv();
        }
        let available_height = ui.available_height() - 40.0;
        let row_height = 20.0;
        let max_visible_rows = (available_height / row_height).floor() as usize;
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                egui::Grid::new("results_table")
                    .striped(true)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        for col in &self.query_columns {
                            ui.strong(col);
                        }
                        ui.end_row();
                        for row in &self.query_rows {
                            for cell in row {
                                ui.label(cell);
                            }
                            ui.end_row();
                        }
                        let data_rows = self.query_rows.len();
                        if data_rows < max_visible_rows {
                            let empty_rows_needed = max_visible_rows - data_rows;
                            for _ in 0..empty_rows_needed {
                                for _ in &self.query_columns {
                                    ui.add_space(0.0);
                                }
                                ui.end_row();
                            }
                        }
                    });
            });
    }

    pub fn check_sql(&mut self) {
        self.sql_error = None;
        let dialect = sqlparser::dialect::PostgreSqlDialect {};
        match sqlparser::parser::Parser::parse_sql(&dialect, &self.sql_input) {
            Ok(_) => {
                self.sql_error = None;
            }
            Err(e) => {
                self.sql_error = Some(format!("{}", e));
            }
        }
    }

    pub fn run_sql(&mut self, ctxs: &mut SqlCtx) {
        self.sql_error = None;
        let Some(sess) = ctxs.state.sessions.get(ctxs.state.current_index).cloned() else {
            self.sql_error = Some("No active session".into());
            return;
        };
        let dsn = sess.db.dsn.clone();
        if dsn.trim().is_empty() {
            self.sql_error = Some("DSN is empty".into());
            return;
        }
        let sql = self.sql_input.clone();
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                self.sql_error = Some(format!("runtime error: {}", e));
                return;
            }
        };
        let pool_opt = ctxs.db.pools.get(&sess.id).cloned();
        let res: Result<(Vec<String>, Vec<Vec<String>>), String> = rt.block_on(async move {
            let pool = match pool_opt {
                Some(p) => p,
                None => PgPoolOptions::new()
                    .max_connections(1)
                    .connect(&dsn)
                    .await
                    .map_err(|e| e.to_string())?,
            };
            let rows: Vec<PgRow> = sqlx::query(&sql)
                .fetch_all(&pool)
                .await
                .map_err(|e| e.to_string())?;
            let mut cols: Vec<String> = Vec::new();
            let mut data: Vec<Vec<String>> = Vec::new();
            if let Some(first) = rows.first() {
                for c in first.columns() {
                    cols.push(c.name().to_string());
                }
            }
            for row in rows.into_iter().take(100) {
                let mut r: Vec<String> = Vec::new();
                let n = if cols.is_empty() {
                    row.len()
                } else {
                    cols.len()
                };
                for i in 0..n {
                    r.push(crate::sql::format_cell(&row, i));
                }
                data.push(r);
            }
            Ok((cols, data))
        });
        match res {
            Ok((cols, rows)) => {
                self.query_columns = cols;
                self.query_rows = rows;
            }
            Err(e) => {
                self.sql_error = Some(e);
            }
        }
    }

    pub fn export_csv(&mut self) {
        if self.query_columns.is_empty() {
            return;
        }
        if rfd::FileDialog::new()
            .set_title("Save CSV")
            .add_filter("CSV", &["csv"])
            .save_file()
            .and_then(|path| csv::Writer::from_path(&path).ok())
            .map(|mut wtr| {
                let _ = wtr.write_record(&self.query_columns);
                for row in &self.query_rows {
                    let _ = wtr.write_record(row);
                }
                let _ = wtr.flush();
            })
            .is_some()
        {}
    }
}
