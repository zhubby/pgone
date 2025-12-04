use super::ResultsTable;

impl ResultsTable {
    /// Render SQL editor with syntax highlighting
    pub fn ui_sql_editor(&mut self, ui: &mut egui::Ui, show_execute: bool) {
        // 标题栏：标题靠左，按钮在右侧垂直居中
        ui.horizontal(|ui| {
            // 标题靠左显示
            ui.heading(format!("{} SQL Editor", egui_phosphor::regular::CODE));
            
            // 右侧按钮区域，垂直居中
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(4.0);

                if show_execute {
                    if ui
                        .add(
                            egui::Button::new(egui_phosphor::regular::PLAY)
                                .min_size(egui::vec2(20.0, 20.0)),
                        )
                        .clicked()
                    {
                        self.execute_sql_requested = true;
                    }
                    ui.add_space(4.0);
                }
                if ui
                    .add(
                        egui::Button::new(egui_phosphor::regular::MAGIC_WAND)
                            .min_size(egui::vec2(20.0, 20.0)),
                    )
                    .clicked()
                {
                    self.check_sql();
                    self.sql_input = crate::sql::format_sql(&self.sql_input);
                }

                if show_execute {
                    ui.horizontal(|ui| {
                        
                        egui::ComboBox::from_id_salt("database_selector")
                            .selected_text(
                                self.selected_database
                                    .as_ref()
                                    .map(|s| s.as_str())
                                    .unwrap_or("<Default>"),
                            )
                            .show_ui(ui, |ui| {
                                // Option to use DSN database
                                if ui
                                    .selectable_value(&mut self.selected_database, None, "<Default>")
                                    .clicked()
                                {
                                    // Reset to DSN database
                                }
        
                                // List available databases
                                for db_name in &self.available_databases {
                                    if ui
                                        .selectable_value(
                                            &mut self.selected_database,
                                            Some(db_name.clone()),
                                            db_name,
                                        )
                                        .clicked()
                                    {
                                        // Database selected
                                    }
                                }
                            });

                            ui.label("Database:");
                    });
                }
            });
        });
        
        ui.separator();

        let current_sql = self.sql_input.clone();
        // Use available height minus header and separator space
        let available_height = ui.available_height() - 10.0;

        // 添加左右边距（各 5）
        ui.horizontal(|ui| {
            ui.add_space(5.0);
            
            let editor = ui.add_sized(
                egui::Vec2::new(ui.available_width() - 5.0, available_height),
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
        });
    }

    /// Check SQL syntax
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
}

