use super::ResultsTable;
use tracing::debug;

impl ResultsTable {
    /// Render results table with enhanced pagination and field display
    pub fn ui_results_table(&mut self, ui: &mut egui::Ui, show_refresh: bool) {
        // Update current SQL statement
        let new_sql = Some(self.sql_input.clone());

        // Reset to first page if SQL statement changed
        if self.previous_sql != new_sql {
            self.current_page = 1;
            self.previous_sql = new_sql.clone();
        }

        self.current_sql = new_sql;

        ui.horizontal(|ui| {
            ui.heading(format!("{} Results", egui_phosphor::regular::TABLE));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if show_refresh {
                    if ui.button(egui_phosphor::regular::ARROW_CLOCKWISE).clicked() {
                        self.refresh_requested = true;
                    }
                    ui.add_space(8.0);
                }
                if ui.button("Export CSV...").clicked() {
                    self.export_csv(&self.query_columns, &self.query_rows);
                }
            });
        });
        ui.separator();

        // Toolbar with SQL statement and pagination
        ui.horizontal(|ui| {
            // Display SQL statement (truncated if too long)
            if let Some(ref sql) = self.current_sql {
                // Take only the first line (before any newline)
                let first_line = sql.lines().next().unwrap_or("");
                // Truncate to max 100 characters
                let truncated_sql = if first_line.len() > 100 {
                    format!("{}...", &first_line[..100])
                } else {
                    first_line.to_string()
                };
                ui.label(
                    egui::RichText::new(truncated_sql)
                        .color(egui::Color32::GRAY),
                );
            } else {
                ui.label(
                    egui::RichText::new("No SQL statement")
                        .color(egui::Color32::GRAY),
                );
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Enhanced pagination controls
                let total_rows = self.query_rows.len();
                let rows_per_page = self.rows_per_page.max(1);
                let total_pages = if total_rows == 0 {
                    1
                } else {
                    (total_rows + rows_per_page - 1) / rows_per_page
                };

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

                // Page size selector
                egui::ComboBox::from_id_salt("page_size")
                    .selected_text(format!("{} / 页", self.rows_per_page))
                    .show_ui(ui, |ui| {
                        for &size in &self.page_size_options {
                            if ui
                                .selectable_value(
                                    &mut self.rows_per_page,
                                    size,
                                    format!("{} / 页", size),
                                )
                                .clicked()
                            {
                                self.current_page = 1; // Reset to first page when changing page size
                            }
                        }
                    });
                ui.add_space(8.0);

                // Page jump input
                ui.add(
                    egui::TextEdit::singleline(&mut self.page_jump_input)
                        .desired_width(50.0)
                        .hint_text("页码"),
                );
                if ui.button("跳转").clicked() {
                    if let Ok(page_num) = self.page_jump_input.parse::<usize>() {
                        if page_num >= 1 && page_num <= total_pages {
                            self.current_page = page_num;
                            self.page_jump_input.clear();
                        }
                    }
                }
                ui.add_space(8.0);

                // Page info
                if total_rows > 0 {
                    let start_row = (self.current_page - 1) * rows_per_page + 1;
                    let end_row = (start_row + rows_per_page - 1).min(total_rows);
                    ui.label(format!("{} - {} / {}", start_row, end_row, total_rows));
                    ui.add_space(8.0);

                    // Next page button
                    if ui
                        .add_enabled(
                            self.current_page < total_pages,
                            egui::Button::new(egui_phosphor::regular::CARET_RIGHT),
                        )
                        .clicked()
                    {
                        if self.current_page < total_pages {
                            self.current_page += 1;
                        }
                    }

                    // Previous page button
                    if ui
                        .add_enabled(
                            self.current_page > 1,
                            egui::Button::new(egui_phosphor::regular::CARET_LEFT),
                        )
                        .clicked()
                    {
                        if self.current_page > 1 {
                            self.current_page -= 1;
                        }
                    }

                    ui.add_space(4.0);

                    // First page button
                    if ui
                        .add_enabled(
                            self.current_page > 1,
                            egui::Button::new(egui_phosphor::regular::CARET_DOUBLE_LEFT),
                        )
                        .clicked()
                    {
                        self.current_page = 1;
                    }

                    // Last page button
                    if ui
                        .add_enabled(
                            self.current_page < total_pages,
                            egui::Button::new(egui_phosphor::regular::CARET_DOUBLE_RIGHT),
                        )
                        .clicked()
                    {
                        self.current_page = total_pages;
                    }
                } else {
                    ui.label("0 / 0");
                }
            });
        });
        ui.separator();

        if self.query_columns.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(format!("{} No results", egui_phosphor::regular::EMPTY));
            });
            return;
        }

        debug!("query_columns: {:?}", self.query_columns);
        debug!("query_rows: {:?}", self.query_rows);

        // Calculate pagination
        let total_rows = self.query_rows.len();
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
        let page_rows = &self.query_rows;

        debug!("page_rows: {:?}", page_rows);

        let available_height = ui.available_height() - 40.0;
        let row_height = 20.0;
        let max_visible_rows = (available_height / row_height).floor() as usize;

        let pk_cols: Vec<String> = self.primary_key_columns.iter().cloned().collect();
        let sort_column = self.sort_column.clone();
        let sort_ascending = self.sort_ascending;
        let query_columns = self.query_columns.clone();

        // Track which column was clicked for sorting
        let mut clicked_column: Option<String> = None;

        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                egui::Grid::new("results_table")
                    .striped(true)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        // Header row - add blank column at the beginning
                        ui.label(""); // Blank column
                        for col in &query_columns {
                            let is_sorted = sort_column.as_ref().map(|s| s == col).unwrap_or(false);
                            let sort_indicator = if is_sorted {
                                if sort_ascending {
                                    egui_phosphor::regular::CARET_UP
                                } else {
                                    egui_phosphor::regular::CARET_DOWN
                                }
                            } else {
                                ""
                            };

                            let response = ui
                                .horizontal(|ui| {
                                    // Show key icon for primary key columns
                                    if pk_cols.contains(col) {
                                        ui.label(egui_phosphor::regular::KEY);
                                        ui.add_space(4.0);
                                    }
                                    ui.strong(col);
                                    if is_sorted {
                                        ui.label(sort_indicator);
                                    }
                                })
                                .response;

                            // Track clicked column for sorting (outside closure)
                            if response.clicked() {
                                clicked_column = Some(col.clone());
                            }
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
                                for _ in &query_columns {
                                    ui.label("");
                                }
                                ui.end_row();
                            }
                        }
                    });
            });

        // Apply sorting outside the closure
        if let Some(col) = clicked_column {
            self.sort_rows(&col);
        }
    }

    /// Sort rows by column
    pub fn sort_rows(&mut self, column: &str) {
        if let Some(col_idx) = self.query_columns.iter().position(|c| c == column) {
            let ascending = if self
                .sort_column
                .as_ref()
                .map(|s| s == column)
                .unwrap_or(false)
            {
                !self.sort_ascending
            } else {
                true
            };

            self.sort_column = Some(column.to_string());
            self.sort_ascending = ascending;

            self.query_rows.sort_by(|a, b| {
                let a_val = a.get(col_idx).map(|s| s.as_str()).unwrap_or("");
                let b_val = b.get(col_idx).map(|s| s.as_str()).unwrap_or("");
                let cmp = a_val.cmp(b_val);
                if ascending { cmp } else { cmp.reverse() }
            });

            // Reset to first page after sorting
            self.current_page = 1;
        }
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
                let galley =
                    f.layout_no_wrap(ellipsis.to_string(), font_id.clone(), egui::Color32::GRAY);
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
                    let galley = f.layout_no_wrap(
                        truncated.to_string(),
                        font_id.clone(),
                        egui::Color32::GRAY,
                    );
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

    pub fn export_csv(&self, columns: &[String], rows: &[Vec<String>]) {
        if columns.is_empty() {
            return;
        }

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

