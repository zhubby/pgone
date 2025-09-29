use crate::IntelliGuiApp;

impl IntelliGuiApp {
    pub fn ui_results(&mut self, ui: &mut egui::Ui) {
        ui.heading("Results");
        ui.separator();
        
        if self.query_columns.is_empty() {
            ui.label("No results");
        } else {
            if ui.button("Export CSV...").clicked() { 
                self.export_csv(); 
            }
            
            // 计算可用空间和需要填充的空白行数
            let available_height = ui.available_height() - 40.0; // 减去按钮和标题的高度
            let row_height = 20.0;
            let max_visible_rows = (available_height / row_height).floor() as usize;
            
            // 使用ScrollArea来确保表格可以滚动
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    // 使用Grid来创建表格，支持自动填充空白行
                    egui::Grid::new("results_table")
                        .striped(true)
                        .spacing([8.0, 4.0])
                        .show(ui, |ui| {
                            // 渲染表头 - 使用字段名作为表头
                            for col in &self.query_columns {
                                ui.strong(col);
                            }
                            ui.end_row();
                            
                            // 渲染数据行
                            for row in &self.query_rows {
                                for cell in row {
                                    ui.label(cell);
                                }
                                ui.end_row();
                            }
                            
                            // 如果数据行数少于最大可见行数，填充空白行
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
    }
}


