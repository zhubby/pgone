use super::ResultsTable;
use crate::components::SqlCtx;
use egui_extras::{Column, TableBuilder};
use serde_json::Value;

fn truncate_cell_text(ui: &egui::Ui, text: &str, available_width: f32) -> (String, bool) {
    let (first_line, truncated_by_line) = first_display_line(text);
    let display = if truncated_by_line {
        format!("{first_line}...")
    } else {
        first_line.clone()
    };

    if text_width(ui, &display) <= available_width {
        return (display, truncated_by_line);
    }

    let ellipsis = "...";
    if text_width(ui, ellipsis) > available_width {
        return (ellipsis.to_string(), true);
    }

    let char_count = first_line.chars().count();
    let mut low = 0;
    let mut high = char_count;
    while low < high {
        let mid = (low + high).div_ceil(2);
        let candidate = format!(
            "{}{}",
            first_line.chars().take(mid).collect::<String>(),
            ellipsis
        );
        if text_width(ui, &candidate) <= available_width {
            low = mid;
        } else {
            high = mid - 1;
        }
    }

    (
        format!(
            "{}{}",
            first_line.chars().take(low).collect::<String>(),
            ellipsis
        ),
        true,
    )
}

fn first_display_line(text: &str) -> (String, bool) {
    let line_end = text
        .find("\r\n")
        .or_else(|| text.find('\n'))
        .or_else(|| text.find('\r'));

    if let Some(line_end) = line_end {
        (text.chars().take(line_end).collect(), true)
    } else {
        (text.to_string(), false)
    }
}

fn text_width(ui: &egui::Ui, text: &str) -> f32 {
    let font_id = egui::TextStyle::Body.resolve(ui.style());
    ui.painter()
        .layout_no_wrap(text.to_string(), font_id, ui.visuals().text_color())
        .size()
        .x
}

fn parse_json_cell(value: &str) -> Option<Value> {
    let trimmed = value.trim();
    if !((trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']')))
    {
        return None;
    }

    serde_json::from_str::<Value>(trimmed)
        .ok()
        .filter(|value| value.is_object() || value.is_array())
}

impl ResultsTable {
    /// Execute SQL query and update results
    /// Get database connection from SqlCtx, execute SQL statement, and store results in the table
    fn execute_sql(&mut self, sql: &str, ctxs: &mut SqlCtx) {
        let Some((dsn, sql)) = self.query_request(ctxs, sql.to_string()) else {
            return;
        };
        self.start_query(ctxs.db.pools.clone(), dsn, sql);
    }

    /// Render query results table
    /// Accept SQL statement and SqlCtx, execute SQL internally and render results
    /// Supports primary key column identification, CSV export, and auto-refresh
    pub fn ui_results_table(
        &mut self,
        ui: &mut egui::Ui,
        sql: Option<&str>,
        ctxs: Option<&mut SqlCtx>,
        show_refresh: bool,
    ) {
        self.poll_query_promise();

        // Update current SQL statement (but do not auto-execute)
        if let Some(sql_str) = sql {
            // Only update current SQL, do not auto-execute
            let sql_changed = self
                .current_sql
                .as_ref()
                .map(|s| s != sql_str)
                .unwrap_or(true);
            if sql_changed {
                self.current_sql = Some(sql_str.to_string());
                self.previous_sql = self.current_sql.clone();
            }
        }

        // Check if refresh is needed
        let should_refresh = self.refresh_requested;
        if should_refresh {
            self.refresh_requested = false;
        }

        // Check if there is an execution request (triggered by clicking the run button)
        let should_execute_requested = self.execute_sql_requested;
        if should_execute_requested {
            self.execute_sql_requested = false;
        }

        // Execute SQL (only when run button or refresh button is clicked, not auto-executed)
        if (should_refresh || should_execute_requested) && sql.is_some() {
            if let Some(ctxs) = ctxs {
                self.execute_sql(sql.unwrap(), ctxs);
            }
        }

        // Top toolbar: SQL preview, refresh button, CSV export button
        ui.horizontal(|ui| {
            if let Some(ref sql_str) = self.current_sql {
                // Show only the first line, up to 300 characters
                let first_line = sql_str.lines().next().unwrap_or("");
                let truncated_sql = if first_line.chars().count() > 300 {
                    format!("{}...", first_line.chars().take(300).collect::<String>())
                } else {
                    first_line.to_string()
                };
                ui.label(egui::RichText::new(truncated_sql).color(egui::Color32::GRAY));
            } else {
                ui.label(egui::RichText::new("No SQL statement").color(egui::Color32::GRAY));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(egui_phosphor::regular::DOWNLOAD_SIMPLE)
                    .on_hover_text("Export CSV")
                    .clicked()
                {
                    self.export_csv(&self.query_columns, &self.query_rows);
                }

                if show_refresh {
                    if ui.button(egui_phosphor::regular::ARROW_CLOCKWISE).clicked() {
                        self.refresh_requested = true;
                    }
                    ui.add_space(8.0);
                }

                if let Some(ref explain_info) = self.explain_info {
                    // Show EXPLAIN information: type | cost | rows
                    let info_text = format!(
                        "{} {} | Cost: {} | Rows: {}",
                        egui_phosphor::regular::INFO,
                        explain_info.scan_type,
                        explain_info.cost,
                        explain_info.rows
                    );
                    ui.label(
                        egui::RichText::new(info_text)
                            .color(egui::Color32::from_rgb(100, 150, 200)),
                    );
                } else if let Some(ref error) = self.explain_error {
                    // Show EXPLAIN error
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {}",
                            egui_phosphor::regular::WARNING,
                            error
                        ))
                        .color(egui::Color32::from_rgb(200, 100, 100))
                        .small(),
                    );
                } else {
                    // Show placeholder when no EXPLAIN information is available
                    ui.label(
                        egui::RichText::new(format!("{} No plan", egui_phosphor::regular::INFO))
                            .color(egui::Color32::GRAY)
                            .small(),
                    );
                }
            });
        });
        ui.separator();

        // Show error message (if any)
        if let Some(ref error) = self.sql_error {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "{} Error: {}",
                        egui_phosphor::regular::WARNING,
                        error
                    ))
                    .color(egui::Color32::RED),
                );
            });
            ui.separator();
        }

        // Show empty state if no query results
        if self.query_columns.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(format!("{} No results", egui_phosphor::regular::EMPTY));
            });
            return;
        }

        // debug!("query_columns: {:?}", self.query_columns);
        // debug!("query_rows: {:?}", self.query_rows);

        let columns = self.query_columns.clone();
        let rows = self.query_rows.clone();
        let primary_keys = self.primary_key_columns.clone();
        let mut json_viewer_requests = Vec::new();

        egui::ScrollArea::both().show(ui, |ui| {
            let table = TableBuilder::new(ui)
                .id_salt("query_results_table")
                .striped(true)
                .resizable(true)
                .columns(Column::auto().at_least(96.0), columns.len());

            table
                .header(22.0, |mut header| {
                    for column in &columns {
                        header.col(|ui| {
                            if primary_keys.contains(column) {
                                ui.strong(format!("{} {}", egui_phosphor::regular::KEY, column));
                            } else {
                                ui.strong(column);
                            }
                        });
                    }
                })
                .body(|mut body| {
                    let mut selected_row = self.selected_result_row;
                    for (row_index, row) in rows.iter().enumerate() {
                        body.row(22.0, |mut table_row| {
                            table_row.set_selected(selected_row == Some(row_index));
                            let mut row_clicked = false;
                            for index in 0..columns.len() {
                                table_row.col(|ui| {
                                    let value = row.get(index).map(String::as_str).unwrap_or("");
                                    let json_value = parse_json_cell(value);
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 4.0;
                                        let button_width =
                                            if json_value.is_some() { 22.0 } else { 0.0 };
                                        let available_width =
                                            (ui.available_width() - button_width - 4.0).max(0.0);
                                        let (display_value, truncated) =
                                            truncate_cell_text(ui, value, available_width);
                                        let response = ui.add(
                                            egui::Label::new(display_value)
                                                .sense(egui::Sense::click()),
                                        );
                                        row_clicked |= response.clicked();
                                        if truncated {
                                            response.on_hover_text(value);
                                        }

                                        if let Some(json_value) = json_value {
                                            if ui
                                                .small_button(
                                                    egui_phosphor::regular::BRACKETS_CURLY,
                                                )
                                                .on_hover_text("Open JSON viewer")
                                                .clicked()
                                            {
                                                json_viewer_requests.push((
                                                    row_index,
                                                    columns[index].clone(),
                                                    json_value,
                                                ));
                                            }
                                        }
                                    });
                                });
                            }
                            if row_clicked {
                                selected_row = Some(row_index);
                            }
                        });
                    }
                    self.selected_result_row = selected_row.filter(|index| *index < rows.len());
                });
        });

        for (row_index, column, value) in json_viewer_requests {
            self.open_json_viewer(row_index, &column, value);
        }
    }

    /// Export query results to CSV file
    /// Use file dialog to select save location, then write query results to CSV file
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
