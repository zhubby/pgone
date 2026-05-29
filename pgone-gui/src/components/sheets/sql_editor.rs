use super::ResultsTable;
use crate::sql;

impl ResultsTable {
    /// Render SQL editor with syntax highlighting
    pub fn ui_sql_editor(&mut self, ui: &mut egui::Ui, show_execute: bool) {
        ui.horizontal(|ui| {
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
                                    .selectable_value(
                                        &mut self.selected_database,
                                        None,
                                        "<Default>",
                                    )
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

        let current_sql = self.sql_input.clone();
        let available_height = (ui.available_height() - 4.0).max(0.0);

        // Add left and right padding (5 each)
        ui.horizontal(|ui| {
            ui.add_space(5.0);

            let editor_id = ui.make_persistent_id("sql_editor");

            // Clear the pending cursor position marker (no longer using the space-adding method)
            // TextEdit automatically places the cursor at the end of the replacement range after replacing text
            let _ = self.pending_cursor_pos.take();

            let editor = ui.add_sized(
                egui::Vec2::new(ui.available_width() - 5.0, available_height),
                egui::TextEdit::multiline(&mut self.sql_input)
                    .id(editor_id)
                    .desired_rows(((available_height / 20.0) as usize).max(1))
                    .layouter(&mut move |ui, _text, wrap_width| {
                        let mut job = crate::sql::highlight_sql(&current_sql, ui.visuals());
                        job.wrap.max_width = wrap_width;
                        ui.fonts_mut(|f| f.layout_job(job))
                    }),
            );

            if let Some(err) = &self.sql_error {
                ui.colored_label(egui::Color32::RED, err);
            }

            let text_changed = editor.changed();
            if text_changed {
                self.sql_error = None;
            }

            // Handle auto-completion
            if editor.has_focus() {
                self.handle_completion(ui, &editor, text_changed);
            } else {
                // Close completion window when losing focus
                self.show_completion = false;
            }
        });
    }

    /// Handle auto-completion logic
    fn handle_completion(
        &mut self,
        ui: &mut egui::Ui,
        editor: &egui::Response,
        text_changed: bool,
    ) {
        // Get input state
        let input = ui.input(|i| i.clone());

        // Handle keyboard events
        if self.show_completion && !self.completion_suggestions.is_empty() {
            if input.key_pressed(egui::Key::ArrowDown) {
                self.completion_selected_index =
                    (self.completion_selected_index + 1) % self.completion_suggestions.len();
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

        // When text changes, update completion suggestions
        if text_changed {
            let text = &self.sql_input;

            // Detect text change position (cursor position)
            let cursor_pos = if text.len() > self.previous_sql_input.len() {
                // Text increased, cursor should be after the new characters
                text.len()
            } else if text.len() < self.previous_sql_input.len() {
                // Text decreased (deletion), find the first differing position
                let mut pos = 0;
                let old_bytes = self.previous_sql_input.as_bytes();
                let new_bytes = text.as_bytes();
                let min_len = old_bytes.len().min(new_bytes.len());
                while pos < min_len && old_bytes[pos] == new_bytes[pos] {
                    pos += 1;
                }
                pos
            } else {
                // Text length is the same, might be a replacement, find the first differing position
                let mut pos = 0;
                let old_bytes = self.previous_sql_input.as_bytes();
                let new_bytes = text.as_bytes();
                let min_len = old_bytes.len().min(new_bytes.len());
                while pos < min_len && old_bytes[pos] == new_bytes[pos] {
                    pos += 1;
                }
                // Find the position where the change ends
                while pos < new_bytes.len()
                    && pos < old_bytes.len()
                    && old_bytes[pos] != new_bytes[pos]
                {
                    pos += 1;
                }
                pos
            };

            // Extract the current word from the cursor position
            let (word, word_start, word_end) = sql::extract_current_word(text, cursor_pos);

            // Check if inside a string or comment (simple check)
            let in_string = self.is_in_string_or_comment(text, cursor_pos);

            if !in_string
                && !word.is_empty()
                && word
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
            {
                // Extract valid word, match keywords
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

            // Update previous text
            self.previous_sql_input = text.clone();
        }

        // Show completion popup
        if self.show_completion && !self.completion_suggestions.is_empty() {
            let popup_id = ui.make_persistent_id("completion_popup");
            let editor_rect = editor.rect;

            // Calculate cursor position on screen
            // Since egui TextEdit does not directly provide cursor position, we calculate through text layout
            let cursor_screen_pos =
                self.calculate_cursor_screen_pos(ui, editor, self.completion_cursor_pos);

            // Calculate popup position (below cursor position)
            let popup_pos =
                cursor_screen_pos + egui::vec2(0.0, ui.text_style_height(&egui::TextStyle::Body));
            let popup_width = editor_rect.width().min(300.0);

            egui::Area::new(popup_id)
                .order(egui::Order::Foreground)
                .fixed_pos(popup_pos)
                .constrain(true)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_width(popup_width);
                        egui::ScrollArea::vertical()
                            .max_height(200.0)
                            .show(ui, |ui| {
                                let mut clicked_index = None;

                                for (idx, suggestion) in
                                    self.completion_suggestions.iter().enumerate()
                                {
                                    let is_selected = idx == self.completion_selected_index;

                                    let response = ui.selectable_label(is_selected, suggestion);

                                    if response.clicked() {
                                        clicked_index = Some(idx);
                                    }

                                    if is_selected {
                                        response.scroll_to_me(Some(egui::Align::Center));
                                    }
                                }

                                // Handle clicks outside the loop to avoid borrow conflicts
                                if let Some(idx) = clicked_index {
                                    self.completion_selected_index = idx;
                                    self.insert_completion();
                                }
                            });
                    });
                });
        }
    }

    /// Calculate cursor position on screen
    fn calculate_cursor_screen_pos(
        &self,
        ui: &egui::Ui,
        editor: &egui::Response,
        cursor_pos: usize,
    ) -> egui::Pos2 {
        let editor_rect = editor.rect;
        let text = &self.sql_input;

        if cursor_pos > text.len() || text.is_empty() {
            return editor_rect.min + egui::vec2(5.0, ui.text_style_height(&egui::TextStyle::Body));
        }

        // Get text up to cursor position
        let text_before_cursor = &text[..cursor_pos.min(text.len())];

        // Calculate text layout
        let text_style = egui::TextStyle::Body;
        let font_height = ui.text_style_height(&text_style);

        // Get font (use the same font as TextEdit)
        let font_id = ui
            .style()
            .text_styles
            .get(&text_style)
            .cloned()
            .unwrap_or_else(|| egui::FontId::new(font_height, egui::FontFamily::Proportional));

        // Calculate text layout (use the same wrap_width and layouter as TextEdit)
        let wrap_width = editor_rect.width() - 10.0; // Subtract left and right padding

        // Use the same layout logic as TextEdit (including syntax highlighting)
        let current_sql = text_before_cursor.to_string();
        let galley = ui.fonts_mut(|f| {
            let mut job = crate::sql::highlight_sql(&current_sql, ui.visuals());
            job.wrap.max_width = wrap_width;
            f.layout_job(job)
        });

        // Calculate the line and column where the cursor is
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

        // Get the current line text (up to cursor position)
        let line_text = &text_before_cursor[line_start..];
        let cursor_in_line = cursor_pos - line_start;

        // Calculate the cursor x position within this line
        let cursor_text = &line_text[..cursor_in_line.min(line_text.len())];
        let cursor_x = ui.fonts_mut(|f| {
            cursor_text
                .chars()
                .map(|c| f.glyph_width(&font_id, c))
                .sum::<f32>()
        });

        // Calculate y position
        // Use galley row information to get accurate row height
        let cursor_y = if line_index < galley.rows.len() {
            let row = &galley.rows[line_index];
            row.rect().min.y - galley.rect.min.y + font_height
        } else {
            // If line index is out of range, use estimated value
            line_index as f32 * font_height + font_height
        };

        editor_rect.min + egui::vec2(5.0 + cursor_x, cursor_y)
    }

    /// Check if cursor position is inside a string or comment
    fn is_in_string_or_comment(&self, text: &str, pos: usize) -> bool {
        if pos > text.len() {
            return false;
        }

        let bytes = text.as_bytes();
        let mut i = 0;

        while i < pos.min(bytes.len()) {
            let c = bytes[i] as char;

            // Check single-line comment
            if i + 1 < bytes.len() && c == '-' && bytes[i + 1] as char == '-' {
                // Find next newline character
                while i < bytes.len() && bytes[i] as char != '\n' {
                    if i >= pos {
                        return true; // Inside comment
                    }
                    i += 1;
                }
                continue;
            }

            // Check multi-line comment
            if i + 1 < bytes.len() && c == '/' && bytes[i + 1] as char == '*' {
                i += 2;
                while i + 1 < bytes.len() {
                    if bytes[i] as char == '*' && bytes[i + 1] as char == '/' {
                        i += 2;
                        break;
                    }
                    if i >= pos {
                        return true; // Inside comment
                    }
                    i += 1;
                }
                continue;
            }

            // Check single-quoted string
            if c == '\'' {
                i += 1;
                while i < bytes.len() {
                    let ch = bytes[i] as char;
                    if ch == '\'' {
                        // Check if it is an escaped single quote ''
                        if i + 1 < bytes.len() && bytes[i + 1] as char == '\'' {
                            i += 2;
                        } else {
                            i += 1;
                            break;
                        }
                    } else if ch == '\\' && i + 1 < bytes.len() {
                        i += 2; // Escape character
                    } else {
                        if i >= pos {
                            return true; // Inside string
                        }
                        i += 1;
                    }
                }
                continue;
            }

            // Check double-quoted identifier
            if c == '"' {
                i += 1;
                while i < bytes.len() {
                    if bytes[i] as char == '"' {
                        if i + 1 < bytes.len() && bytes[i + 1] as char == '"' {
                            i += 2; // Escaped double quote
                        } else {
                            i += 1;
                            break;
                        }
                    } else {
                        if i >= pos {
                            return true; // Inside identifier
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

    /// Insert selected completion item
    fn insert_completion(&mut self) {
        if self.completion_selected_index < self.completion_suggestions.len() {
            let selected = self.completion_suggestions[self.completion_selected_index].clone();

            // Replace current word with selected keyword
            if self.completion_word_start < self.sql_input.len()
                && self.completion_word_end <= self.sql_input.len()
            {
                // Check if there is already a space or newline after the keyword
                let after_pos = self.completion_word_start + selected.len();
                let needs_space = after_pos < self.sql_input.len()
                    && !self.sql_input[after_pos..]
                        .chars()
                        .next()
                        .map(|c| c.is_whitespace())
                        .unwrap_or(true);

                // Build text to insert (keyword + optional space)
                let text_to_insert = if needs_space {
                    format!("{} ", selected)
                } else {
                    selected
                };

                // Use replace_range to replace text
                // TextEdit automatically places the cursor at the end of the replacement range
                self.sql_input.replace_range(
                    self.completion_word_start..self.completion_word_end,
                    &text_to_insert,
                );

                // Immediately update previous text so next text change detection is not misjudged
                self.previous_sql_input = self.sql_input.clone();
            }

            // Close completion window
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
