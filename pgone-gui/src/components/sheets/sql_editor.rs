use super::ResultsTable;
use crate::sql;

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
            
            let editor_id = ui.make_persistent_id("sql_editor");
            
            // 清除待设置的光标位置标记（不再使用添加空格的方法）
            // TextEdit 在替换文本后会自动将光标放在替换范围的末尾
            let _ = self.pending_cursor_pos.take();
            
            let editor = ui.add_sized(
                egui::Vec2::new(ui.available_width() - 5.0, available_height),
                egui::TextEdit::multiline(&mut self.sql_input)
                    .id(editor_id)
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

            let text_changed = editor.changed();
            if text_changed {
                self.sql_error = None;
            }

            // 处理自动补全
            if editor.has_focus() {
                self.handle_completion(ui, &editor, text_changed);
            } else {
                // 失去焦点时关闭补全窗口
                self.show_completion = false;
            }
        });
    }

    /// 处理自动补全逻辑
    fn handle_completion(&mut self, ui: &mut egui::Ui, editor: &egui::Response, text_changed: bool) {
        // 获取输入状态
        let input = ui.input(|i| i.clone());
        
        // 处理键盘事件
        if self.show_completion && !self.completion_suggestions.is_empty() {
            if input.key_pressed(egui::Key::ArrowDown) {
                self.completion_selected_index = (self.completion_selected_index + 1) % self.completion_suggestions.len();
                ui.ctx().input_mut(|i| {
                    i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown);
                });
            } else if input.key_pressed(egui::Key::ArrowUp) {
                if self.completion_selected_index == 0 {
                    self.completion_selected_index = self.completion_suggestions.len() - 1;
                } else {
                    self.completion_selected_index -= 1;
                }
                ui.ctx().input_mut(|i| {
                    i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp);
                });
            } else if input.key_pressed(egui::Key::Enter) || input.key_pressed(egui::Key::Tab) {
                self.insert_completion();
                ui.ctx().input_mut(|i| {
                    i.consume_key(egui::Modifiers::NONE, egui::Key::Enter);
                    i.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
                });
                return;
            } else if input.key_pressed(egui::Key::Escape) {
                self.show_completion = false;
                ui.ctx().input_mut(|i| {
                    i.consume_key(egui::Modifiers::NONE, egui::Key::Escape);
                });
                return;
            }
        }

        // 当文本改变时，更新补全建议
        if text_changed {
            let text = &self.sql_input;
            
            // 检测文本变化位置（光标位置）
            let cursor_pos = if text.len() > self.previous_sql_input.len() {
                // 文本增加了，光标应该在新增字符之后
                text.len()
            } else if text.len() < self.previous_sql_input.len() {
                // 文本减少了（删除），找到第一个不同的位置
                let mut pos = 0;
                let old_bytes = self.previous_sql_input.as_bytes();
                let new_bytes = text.as_bytes();
                let min_len = old_bytes.len().min(new_bytes.len());
                while pos < min_len && old_bytes[pos] == new_bytes[pos] {
                    pos += 1;
                }
                pos
            } else {
                // 文本长度相同，可能是替换，找到第一个不同的位置
                let mut pos = 0;
                let old_bytes = self.previous_sql_input.as_bytes();
                let new_bytes = text.as_bytes();
                let min_len = old_bytes.len().min(new_bytes.len());
                while pos < min_len && old_bytes[pos] == new_bytes[pos] {
                    pos += 1;
                }
                // 找到变化结束的位置
                while pos < new_bytes.len() && pos < old_bytes.len() && old_bytes[pos] != new_bytes[pos] {
                    pos += 1;
                }
                pos
            };
            
            // 从光标位置提取当前词
            let (word, word_start, word_end) = sql::extract_current_word(text, cursor_pos);
            
            // 检查是否在字符串或注释中（简单检查）
            let in_string = self.is_in_string_or_comment(text, cursor_pos);
            
            if !in_string && !word.is_empty() && word.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$') {
                // 提取到有效的词，匹配关键字
                let suggestions = sql::match_keywords(&word);
                if !suggestions.is_empty() {
                    self.completion_suggestions = suggestions;
                    self.completion_selected_index = 0;
                    self.completion_prefix = word.clone();
                    self.completion_cursor_pos = cursor_pos;
                    self.completion_word_start = word_start;
                    self.completion_word_end = word_end;
                    self.show_completion = true;
                } else {
                    self.show_completion = false;
                }
            } else {
                self.show_completion = false;
            }
            
            // 更新上一次的文本
            self.previous_sql_input = text.clone();
        }

        // 显示补全弹出窗口
        if self.show_completion && !self.completion_suggestions.is_empty() {
            let popup_id = ui.make_persistent_id("completion_popup");
            let editor_rect = editor.rect;
            
            // 计算光标在屏幕上的位置
            // 由于 egui 的 TextEdit 不直接提供光标位置，我们通过文本布局来计算
            let cursor_screen_pos = self.calculate_cursor_screen_pos(ui, editor, self.completion_cursor_pos);
            
            // 计算弹出窗口位置（在光标位置下方）
            let popup_pos = cursor_screen_pos + egui::vec2(0.0, ui.text_style_height(&egui::TextStyle::Body));
            let popup_width = editor_rect.width().min(300.0);
            
            egui::Area::new(popup_id)
                .order(egui::Order::Foreground)
                .fixed_pos(popup_pos)
                .constrain(true)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style())
                        .show(ui, |ui| {
                            ui.set_width(popup_width);
                            egui::ScrollArea::vertical()
                                .max_height(200.0)
                                .show(ui, |ui| {
                                    let mut clicked_index = None;
                                    
                                    for (idx, suggestion) in self.completion_suggestions.iter().enumerate() {
                                        let is_selected = idx == self.completion_selected_index;
                                        
                                        let response = ui.selectable_label(is_selected, suggestion);
                                        
                                        if response.clicked() {
                                            clicked_index = Some(idx);
                                        }
                                        
                                        if is_selected {
                                            response.scroll_to_me(Some(egui::Align::Center));
                                        }
                                    }
                                    
                                    // 在循环外处理点击，避免借用冲突
                                    if let Some(idx) = clicked_index {
                                        self.completion_selected_index = idx;
                                        self.insert_completion();
                                    }
                                });
                        });
                });
        }
    }

    /// 计算光标在屏幕上的位置
    fn calculate_cursor_screen_pos(&self, ui: &egui::Ui, editor: &egui::Response, cursor_pos: usize) -> egui::Pos2 {
        let editor_rect = editor.rect;
        let text = &self.sql_input;
        
        if cursor_pos > text.len() || text.is_empty() {
            return editor_rect.min + egui::vec2(5.0, ui.text_style_height(&egui::TextStyle::Body));
        }
        
        // 获取文本到光标位置的部分
        let text_before_cursor = &text[..cursor_pos.min(text.len())];
        
        // 计算文本的布局
        let text_style = egui::TextStyle::Body;
        let font_height = ui.text_style_height(&text_style);
        
        // 获取字体（使用与 TextEdit 相同的字体）
        let font_id = ui.style().text_styles.get(&text_style).cloned().unwrap_or_else(|| {
            egui::FontId::new(font_height, egui::FontFamily::Proportional)
        });
        
        // 计算文本布局（使用与 TextEdit 相同的 wrap_width 和 layouter）
        let wrap_width = editor_rect.width() - 10.0; // 减去左右边距
        
        // 使用与 TextEdit 相同的布局逻辑（包括语法高亮）
        let current_sql = text_before_cursor.to_string();
        let galley = ui.fonts(|f| {
            let mut job = crate::sql::highlight_sql(&current_sql, ui.visuals());
            job.wrap.max_width = wrap_width;
            f.layout_job(job)
        });
        
        // 计算光标所在的行和列
        let mut line_start = 0;
        let mut line_index = 0;
        
        for (i, ch) in text_before_cursor.char_indices() {
            if ch == '\n' {
                if i < cursor_pos {
                    line_start = i + 1;
                    line_index += 1;
                } else {
                    break;
                }
            }
        }
        
        // 获取当前行的文本（到光标位置）
        let line_text = &text_before_cursor[line_start..];
        let cursor_in_line = cursor_pos - line_start;
        
        // 计算光标在该行中的 x 位置
        let cursor_text = &line_text[..cursor_in_line.min(line_text.len())];
        let cursor_x = ui.fonts(|f| {
            cursor_text.chars().map(|c| f.glyph_width(&font_id, c)).sum::<f32>()
        });
        
        // 计算 y 位置
        // 使用 galley 的行信息来获取准确的行高
        let cursor_y = if line_index < galley.rows.len() {
            let row = &galley.rows[line_index];
            row.rect().min.y - galley.rect.min.y + font_height
        } else {
            // 如果行索引超出范围，使用估算值
            line_index as f32 * font_height + font_height
        };
        
        editor_rect.min + egui::vec2(5.0 + cursor_x, cursor_y)
    }

    /// 检查光标位置是否在字符串或注释中
    fn is_in_string_or_comment(&self, text: &str, pos: usize) -> bool {
        if pos > text.len() {
            return false;
        }
        
        let bytes = text.as_bytes();
        let mut i = 0;
        
        while i < pos.min(bytes.len()) {
            let c = bytes[i] as char;
            
            // 检查单行注释
            if i + 1 < bytes.len() && c == '-' && bytes[i + 1] as char == '-' {
                // 找到下一个换行符
                while i < bytes.len() && bytes[i] as char != '\n' {
                    if i >= pos {
                        return true; // 在注释中
                    }
                    i += 1;
                }
                continue;
            }
            
            // 检查多行注释
            if i + 1 < bytes.len() && c == '/' && bytes[i + 1] as char == '*' {
                i += 2;
                while i + 1 < bytes.len() {
                    if bytes[i] as char == '*' && bytes[i + 1] as char == '/' {
                        i += 2;
                        break;
                    }
                    if i >= pos {
                        return true; // 在注释中
                    }
                    i += 1;
                }
                continue;
            }
            
            // 检查单引号字符串
            if c == '\'' {
                i += 1;
                while i < bytes.len() {
                    let ch = bytes[i] as char;
                    if ch == '\'' {
                        // 检查是否是转义的单引号 ''
                        if i + 1 < bytes.len() && bytes[i + 1] as char == '\'' {
                            i += 2;
                        } else {
                            i += 1;
                            break;
                        }
                    } else if ch == '\\' && i + 1 < bytes.len() {
                        i += 2; // 转义字符
                    } else {
                        if i >= pos {
                            return true; // 在字符串中
                        }
                        i += 1;
                    }
                }
                continue;
            }
            
            // 检查双引号标识符
            if c == '"' {
                i += 1;
                while i < bytes.len() {
                    if bytes[i] as char == '"' {
                        if i + 1 < bytes.len() && bytes[i + 1] as char == '"' {
                            i += 2; // 转义的双引号
                        } else {
                            i += 1;
                            break;
                        }
                    } else {
                        if i >= pos {
                            return true; // 在标识符中
                        }
                        i += 1;
                    }
                }
                continue;
            }
            
            i += 1;
        }
        
        false
    }

    /// 插入选中的补全项
    fn insert_completion(&mut self) {
        if self.completion_selected_index < self.completion_suggestions.len() {
            let selected = self.completion_suggestions[self.completion_selected_index].clone();
            
            // 替换当前词为选中的关键字
            if self.completion_word_start < self.sql_input.len() && self.completion_word_end <= self.sql_input.len() {
                // 检查关键字后面是否已经有空格或换行符
                let after_pos = self.completion_word_start + selected.len();
                let needs_space = after_pos < self.sql_input.len() && 
                                 !self.sql_input[after_pos..].chars().next().map(|c| c.is_whitespace()).unwrap_or(true);
                
                // 构建要插入的文本（关键字 + 可选的空格）
                let text_to_insert = if needs_space {
                    format!("{} ", selected)
                } else {
                    selected
                };
                
                // 使用 replace_range 替换文本
                // TextEdit 会自动将光标放在替换范围的末尾
                self.sql_input.replace_range(self.completion_word_start..self.completion_word_end, &text_to_insert);
                
                // 立即更新上一次的文本，这样下次检测文本变化时不会误判
                self.previous_sql_input = self.sql_input.clone();
            }
            
            // 关闭补全窗口
            self.show_completion = false;
            self.completion_suggestions.clear();
        }
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

