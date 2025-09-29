use crate::IntelliGuiApp;

impl IntelliGuiApp {
    pub fn ui_sql_editor(&mut self, ui: &mut egui::Ui) {
        // 标题和按钮在同一行，靠右布置
        ui.horizontal(|ui| {
            ui.heading(format!("{} SQL Editor", egui_phosphor::regular::QUESTION));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Run按钮 - 使用播放图标
                if ui.add(egui::Button::new(egui_phosphor::regular::PLAY)
                    .min_size(egui::vec2(28.0, 28.0))
                ).clicked() {
                    self.run_sql();
                }
                
                ui.add_space(8.0);
                
                // Check按钮 - 使用检查图标
                if ui.add(egui::Button::new(egui_phosphor::regular::CHECK)
                    .min_size(egui::vec2(28.0, 28.0))
                ).clicked() {
                    self.check_sql();
                }
            });
        });
        
        ui.separator();
        
        // 文本输入框铺满剩余空间
        let current_sql = self.sql_input.clone();
        let available_height = ui.available_height() - 10.0; // 留一些边距
        let editor = ui.add(egui::TextEdit::multiline(&mut self.sql_input)
            .desired_rows((available_height / 20.0) as usize) // 根据可用高度计算行数
            .layouter(&mut move |ui, _text, wrap_width| {
                let mut job = crate::sql::highlight_sql(&current_sql, ui.visuals());
                job.wrap.max_width = wrap_width;
                ui.fonts(|f| f.layout_job(job))
            })
        );
        
        // 错误信息显示在底部
        if let Some(err) = &self.sql_error { 
            ui.colored_label(egui::Color32::RED, err); 
        }
        
        if editor.changed() { 
            self.sql_error = None; 
        }
    }
}


