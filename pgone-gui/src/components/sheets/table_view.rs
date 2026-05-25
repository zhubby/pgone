use super::ResultsTable;
use crate::components::SqlCtx;
use egui_extras::{Column, TableBuilder};

fn truncate_cell_text(text: &str) -> String {
    const MAX_LENGTH: usize = 12;

    let first_line = if let Some(crlf_pos) = text.find("\r\n") {
        text.chars().take(crlf_pos).collect::<String>() + "..."
    } else if let Some(newline_pos) = text.find('\n') {
        text.chars().take(newline_pos).collect::<String>() + "..."
    } else if let Some(carriage_pos) = text.find('\r') {
        text.chars().take(carriage_pos).collect::<String>() + "..."
    } else {
        text.to_string()
    };

    if first_line.chars().count() <= MAX_LENGTH {
        first_line
    } else {
        format!(
            "{}...",
            first_line.chars().take(MAX_LENGTH).collect::<String>()
        )
    }
}

impl ResultsTable {
    /// 执行 SQL 查询并更新结果
    /// 从 SqlCtx 获取数据库连接，执行 SQL 语句，并将结果存储到表格中
    fn execute_sql(&mut self, sql: &str, ctxs: &mut SqlCtx) {
        let Some((dsn, sql)) = self.query_request(ctxs, sql.to_string()) else {
            return;
        };
        self.start_query(dsn, sql);
    }

    /// 渲染查询结果表格
    /// 接收 SQL 语句和 SqlCtx，内部执行 SQL 并渲染结果
    /// 支持主键列标识、CSV 导出和自动刷新
    pub fn ui_results_table(
        &mut self,
        ui: &mut egui::Ui,
        sql: Option<&str>,
        ctxs: Option<&mut SqlCtx>,
        show_refresh: bool,
    ) {
        self.poll_query_promise();

        // 更新当前 SQL 语句（但不自动执行）
        if let Some(sql_str) = sql {
            // 只更新当前 SQL，不自动执行
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

        // 检查是否需要刷新
        let should_refresh = self.refresh_requested;
        if should_refresh {
            self.refresh_requested = false;
        }

        // 检查是否有执行请求（通过点击运行按钮触发）
        let should_execute_requested = self.execute_sql_requested;
        if should_execute_requested {
            self.execute_sql_requested = false;
        }

        // 执行 SQL（仅在点击运行按钮或刷新按钮时执行，不自动执行）
        if (should_refresh || should_execute_requested) && sql.is_some() {
            if let Some(ctxs) = ctxs {
                self.execute_sql(sql.unwrap(), ctxs);
            }
        }

        // 顶部工具栏：标题、刷新按钮、CSV 导出按钮
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

        // SQL 语句预览工具栏：左侧固定宽度显示 SQL，右侧显示 EXPLAIN 信息
        ui.horizontal(|ui| {
            // 左侧：SQL 显示区域
            if let Some(ref sql_str) = self.current_sql {
                // 只显示第一行，最多 300 个字符
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

            // 右侧：EXPLAIN 信息显示区域，固定宽度
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(ref explain_info) = self.explain_info {
                    // 显示 EXPLAIN 信息：类型 | 成本 | 行数
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
                    // 显示 EXPLAIN 错误
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
                    // 没有 EXPLAIN 信息时显示占位符
                    ui.label(
                        egui::RichText::new(format!("{} No plan", egui_phosphor::regular::INFO))
                            .color(egui::Color32::GRAY)
                            .small(),
                    );
                }
            });
        });
        ui.separator();

        // 显示错误信息（如果有）
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

        // 如果没有查询结果，显示空状态
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

        egui::ScrollArea::both().show(ui, |ui| {
            let table = TableBuilder::new(ui)
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
                    for row in &rows {
                        body.row(22.0, |mut table_row| {
                            for index in 0..columns.len() {
                                table_row.col(|ui| {
                                    let value = row.get(index).map(String::as_str).unwrap_or("");
                                    ui.label(truncate_cell_text(value));
                                });
                            }
                        });
                    }
                });
        });
    }

    /// 导出查询结果为 CSV 文件
    /// 使用文件对话框选择保存位置，然后将查询结果写入 CSV 文件
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
