use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct ResultsTable {
    pub filter_values: HashMap<usize, String>,
    pub refresh_requested: bool,
    pub current_sql: Option<String>,
    pub previous_sql: Option<String>,
    pub current_page: usize,
    pub rows_per_page: usize,
}

impl ResultsTable {
    pub fn new() -> Self {
        Self {
            filter_values: HashMap::new(),
            refresh_requested: false,
            current_sql: None,
            previous_sql: None,
            current_page: 1,
            rows_per_page: 100,
        }
    }

    pub fn watch_ui(&mut self, _ui: &mut egui::Ui, _pipe: &mut ()) {

    }

    fn truncate_text(ui: &egui::Ui, text: &str, max_width: f32) -> String {
        let font_id = egui::TextStyle::Body.resolve(ui.style());
        let text_width = ui.fonts(|f| {
            let galley = f.layout_no_wrap(text.to_string(), font_id.clone(), egui::Color32::GRAY);
            galley.size().x
        });
        
        if text_width <= max_width {
            text.to_string()
        } else {
            let ellipsis = "...";
            let ellipsis_width = ui.fonts(|f| {
                let galley = f.layout_no_wrap(ellipsis.to_string(), font_id.clone(), egui::Color32::GRAY);
                galley.size().x
            });
            let available_width = max_width - ellipsis_width;
            
            // Binary search for the right truncation point
            let mut low = 0;
            let mut high = text.len();
            while low < high {
                let mid = (low + high + 1) / 2;
                let truncated = &text[..mid];
                let width = ui.fonts(|f| {
                    let galley = f.layout_no_wrap(truncated.to_string(), font_id.clone(), egui::Color32::GRAY);
                    galley.size().x
                });
                if width <= available_width {
                    low = mid;
                } else {
                    high = mid - 1;
                }
            }
            format!("{}...", &text[..low])
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, columns: &[String], rows: &[Vec<String>], primary_key_columns: Option<&HashSet<String>>, show_refresh: bool, sql: Option<&str>) {
        // Update current SQL statement
        let new_sql = sql.map(|s| s.to_string());
        
        // Reset to first page if SQL statement changed
        if self.previous_sql != new_sql {
            self.current_page = 1;
            self.previous_sql = new_sql.clone();
        }
        
        self.current_sql = new_sql;
        ui.horizontal(|ui| {
            ui.heading("Results");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if show_refresh {
                    if ui.button(egui_phosphor::regular::ARROW_CLOCKWISE).clicked() {
                        self.refresh_requested = true;
                    }
                    ui.add_space(8.0);
                }
                if ui.button("Export CSV...").clicked() {
                    self.export_csv(columns, rows);
                }
            });
        });
        ui.separator();
        
        // Toolbar with SQL statement and pagination
        ui.horizontal(|ui| {
            // Display SQL statement (truncated if too long)
            if let Some(ref sql) = self.current_sql {
                let available_width = ui.available_width() - 200.0; // Reserve space for pagination buttons
                let truncated_sql = Self::truncate_text(ui, sql, available_width.max(100.0));
                ui.label(egui::RichText::new(truncated_sql).color(egui::Color32::GRAY).small());
            } else {
                ui.label(egui::RichText::new("No SQL statement").color(egui::Color32::GRAY).small());
            }
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Pagination controls
                let total_rows = rows.len();
                let rows_per_page = self.rows_per_page.max(1);
                let total_pages = if total_rows == 0 { 1 } else { (total_rows + rows_per_page - 1) / rows_per_page };
                
                // Ensure current_page is valid
                if total_pages > 0 {
                    if self.current_page > total_pages {
                        self.current_page = total_pages;
                    }
                    if self.current_page < 1 {
                        self.current_page = 1;
                    }
                } else {
                    self.current_page = 1;
                }
                
                // Page info
                if total_rows > 0 {
                    let start_row = (self.current_page - 1) * rows_per_page + 1;
                    let end_row = (start_row + rows_per_page - 1).min(total_rows);
                    ui.label(format!("{} - {} / {}", start_row, end_row, total_rows));
                    ui.add_space(8.0);
                    
                    // Next page button
                    if ui.add_enabled(self.current_page < total_pages, egui::Button::new(egui_phosphor::regular::CARET_RIGHT)).clicked() {
                        if self.current_page < total_pages {
                            self.current_page += 1;
                        }
                    }
                    
                    // Previous page button
                    if ui.add_enabled(self.current_page > 1, egui::Button::new(egui_phosphor::regular::CARET_LEFT)).clicked() {
                        if self.current_page > 1 {
                            self.current_page -= 1;
                        }
                    }
                    
                    ui.add_space(4.0);
                    
                    // First page button
                    if ui.add_enabled(self.current_page > 1, egui::Button::new(egui_phosphor::regular::CARET_DOUBLE_LEFT)).clicked() {
                        self.current_page = 1;
                    }
                    
                    // Last page button
                    if ui.add_enabled(self.current_page < total_pages, egui::Button::new(egui_phosphor::regular::CARET_DOUBLE_RIGHT)).clicked() {
                        self.current_page = total_pages;
                    }
                } else {
                    ui.label("0 / 0");
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

        // Calculate pagination
        let total_rows = rows.len();
        let total_pages = if self.rows_per_page > 0 {
            (total_rows + self.rows_per_page - 1) / self.rows_per_page
        } else {
            1
        };
        
        // Ensure current_page is valid
        if self.current_page > total_pages.max(1) {
            self.current_page = total_pages.max(1);
        }
        if self.current_page < 1 {
            self.current_page = 1;
        }
        
        // Get current page rows
        let start_idx = if total_rows == 0 { 0 } else { (self.current_page - 1) * self.rows_per_page };
        let end_idx = (start_idx + self.rows_per_page).min(total_rows);
        let page_rows = if start_idx < total_rows {
            &rows[start_idx..end_idx]
        } else {
            &[]
        };
        
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
                        for row in page_rows {
                            ui.label(""); // Blank cell
                            for cell in row {
                                ui.label(cell);
                            }
                            ui.end_row();
                        }
                        
                        // Empty rows for better visibility
                        let data_rows = page_rows.len();
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

