use std::collections::{HashMap, HashSet};
use etl::{
    config::{BatchConfig, PgConnectionConfig, PipelineConfig, TlsConfig},
    destination::memory::MemoryDestination,
    pipeline::Pipeline,
    store::both::memory::MemoryStore,
};

#[derive(Default)]
pub struct ResultsTable {
    pub filter_values: HashMap<usize, String>,
}

impl ResultsTable {

    pub fn watch_ui(&mut self, ui: &mut egui::Ui , pipe: &mut Pipeline<MemoryStore, MemoryDestination>) {

    }


    pub fn ui(&mut self, ui: &mut egui::Ui, columns: &[String], rows: &[Vec<String>], primary_key_columns: Option<&HashSet<String>>) {
        ui.horizontal(|ui| {
            ui.heading("Results");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Export CSV...").clicked() {
                    self.export_csv(columns, rows);
                }
            });
        });
        ui.separator();
        
        if columns.is_empty() {
            ui.label("No results");
            return;
        }

        // Filter inputs row
        // ui.horizontal(|ui| {
        //     for (col_idx, col_name) in columns.iter().enumerate() {
        //         let filter_value = self.filter_values.entry(col_idx).or_insert_with(String::new);
        //         ui.vertical(|ui| {
        //             ui.label(col_name);
        //             ui.text_edit_singleline(filter_value);
        //         });
        //     }
        // });
        // ui.separator();

        // Filter rows based on filter values
        // let filtered_rows: Vec<&Vec<String>> = rows
        //     .iter()
        //     .filter(|row| {
        //         self.filter_values.iter().all(|(col_idx, filter_text)| {
        //             if filter_text.is_empty() {
        //                 true
        //             } else {
        //                 *col_idx < row.len()
        //                     && row[*col_idx]
        //                         .to_lowercase()
        //                         .contains(&filter_text.to_lowercase())
        //             }
        //         })
        //     })
        //     .collect();

        let available_height = ui.available_height() - 40.0;
        let row_height = 20.0;
        let max_visible_rows = (available_height / row_height).floor() as usize;
        
        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                egui::Grid::new("results_table")
                    .striped(true)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        // Header row - add blank column at the beginning
                        ui.label(""); // Blank column
                        for col in columns {
                            ui.horizontal(|ui| {
                                // Show key icon for primary key columns
                                if let Some(pk_cols) = primary_key_columns {
                                    if pk_cols.contains(col) {
                                        ui.label(egui_phosphor::regular::KEY);
                                        ui.add_space(4.0);
                                    }
                                }
                                ui.strong(col);
                            });
                        }
                        ui.end_row();
                        
                        // Data rows - add blank cell at the beginning of each row
                        for row in rows {
                            ui.label(""); // Blank cell
                            for cell in row {
                                ui.label(cell);
                            }
                            ui.end_row();
                        }
                        
                        // Empty rows for better visibility
                        let data_rows = rows.len();
                        if data_rows < max_visible_rows {
                            let empty_rows_needed = max_visible_rows - data_rows;
                            for _ in 0..empty_rows_needed {
                                ui.label(""); // Blank cell
                                for _ in columns {
                                    ui.label("");
                                }
                                ui.end_row();
                            }
                        }
                    });
            });
    }

    fn export_csv(&self, columns: &[String], rows: &[Vec<String>]) {
        if columns.is_empty() {
            return;
        }
        
        // Apply filters when exporting
        // let filtered_rows: Vec<&Vec<String>> = rows
        //     .iter()
        //     .filter(|row| {
        //         self.filter_values.iter().all(|(col_idx, filter_text)| {
        //             if filter_text.is_empty() {
        //                 true
        //             } else {
        //                 *col_idx < row.len()
        //                     && row[*col_idx]
        //                         .to_lowercase()
        //                         .contains(&filter_text.to_lowercase())
        //             }
        //         })
        //     })
        //     .collect();

        if rfd::FileDialog::new()
            .set_title("Save CSV")
            .add_filter("CSV", &["csv"])
            .save_file()
            .and_then(|path| csv::Writer::from_path(&path).ok())
            .map(|mut wtr| {
                let _ = wtr.write_record(columns);
                for row in rows {
                    let _ = wtr.write_record(row);
                }
                let _ = wtr.flush();
            })
            .is_some()
        {}
    }
}

