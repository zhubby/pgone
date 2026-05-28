use super::screen_center;
use eframe::egui::text::LayoutJob;
use eframe::egui::{Align2, Context, TextFormat, Window};

/// Format YAML text
pub fn format_yaml(text: &str) -> Result<String, String> {
    // Attempt to parse YAML
    let value: serde_yaml::Value =
        serde_yaml::from_str(text).map_err(|e| format!("YAML parse error: {}", e))?;

    // YAML is already pretty-formatted, return the formatted string directly
    serde_yaml::to_string(&value).map_err(|e| format!("YAML format error: {}", e))
}

/// YAML syntax highlighting
pub fn highlight_yaml(text: &str, visuals: &egui::Visuals) -> LayoutJob {
    let mut job = LayoutJob::default();

    // Define text formats
    let normal = TextFormat {
        color: visuals.text_color(),
        ..Default::default()
    };
    let string = TextFormat {
        color: egui::Color32::from_rgb(152, 195, 121), // Green - strings
        ..Default::default()
    };
    let number = TextFormat {
        color: egui::Color32::from_rgb(209, 154, 102), // Orange - numbers
        ..Default::default()
    };
    let keyword = TextFormat {
        color: egui::Color32::from_rgb(198, 120, 221), // Purple - keywords (true, false, null, yes, no)
        ..Default::default()
    };
    let comment = TextFormat {
        color: egui::Color32::from_rgb(128, 128, 128), // Gray - comments
        ..Default::default()
    };
    let key = TextFormat {
        color: egui::Color32::from_rgb(86, 182, 194), // Cyan - keys
        ..Default::default()
    };
    let punctuation = TextFormat {
        color: egui::Color32::from_rgb(180, 180, 180), // Light gray - punctuation
        ..Default::default()
    };

    let bytes = text.as_bytes();
    let mut i = 0;
    let mut colon_pos = None;

    while i < bytes.len() {
        let c = bytes[i] as char;

        // Handle comments
        if c == '#' {
            let start = i;
            while i < bytes.len() && bytes[i] as char != '\n' {
                i += 1;
            }
            job.append(&text[start..i], 0.0, comment.clone());
            continue;
        }

        // Handle whitespace
        if c.is_whitespace() {
            if c == '\n' {
                colon_pos = None;
            }
            job.append(&text[i..i + 1], 0.0, normal.clone());
            i += 1;
            continue;
        }

        // Handle strings (single or double quotes)
        if c == '"' || c == '\'' {
            let string_char = c;
            let start = i;
            i += 1;
            while i < bytes.len() {
                let ch = bytes[i] as char;
                if ch == string_char {
                    // Check if escaped quote
                    if string_char == '\'' && i + 1 < bytes.len() && bytes[i + 1] as char == '\'' {
                        i += 2;
                    } else {
                        i += 1;
                        break;
                    }
                } else if ch == '\\' && string_char == '"' && i + 1 < bytes.len() {
                    i += 2; // Skip escape character
                } else {
                    i += 1;
                }
            }
            job.append(&text[start..i], 0.0, string.clone());
            continue;
        }

        // Handle colon (key-value separator)
        if c == ':' {
            colon_pos = Some(i);
            job.append(&text[i..i + 1], 0.0, punctuation.clone());
            i += 1;
            continue;
        }

        // Handle list markers
        if c == '-' && (i == 0 || bytes[i - 1] as char == '\n' || bytes[i - 1] as char == ' ') {
            job.append(&text[i..i + 1], 0.0, punctuation.clone());
            i += 1;
            continue;
        }

        // Handle numbers
        if c.is_ascii_digit()
            || (c == '-' && i + 1 < bytes.len() && (bytes[i + 1] as char).is_ascii_digit())
        {
            let start = i;
            i += 1;

            // Integer part
            while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
                i += 1;
            }

            // Decimal part
            if i < bytes.len() && bytes[i] as char == '.' {
                i += 1;
                while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
                    i += 1;
                }
            }

            // Scientific notation
            if i < bytes.len() && (bytes[i] as char == 'e' || bytes[i] as char == 'E') {
                i += 1;
                if i < bytes.len() && (bytes[i] as char == '+' || bytes[i] as char == '-') {
                    i += 1;
                }
                while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
                    i += 1;
                }
            }

            job.append(&text[start..i], 0.0, number.clone());
            continue;
        }

        // Handle other punctuation
        if matches!(
            c,
            '|' | '>' | '[' | ']' | '{' | '}' | '&' | '*' | '!' | '%' | '@' | '`'
        ) {
            job.append(&text[i..i + 1], 0.0, punctuation.clone());
            i += 1;
            continue;
        }

        // Handle identifiers and keywords
        let start = i;
        while i < bytes.len() {
            let ch = bytes[i] as char;
            if ch.is_whitespace() || ch == ':' || ch == '#' || ch == '|' || ch == '>' {
                break;
            }
            i += 1;
        }

        if start < i {
            let token = &text[start..i];
            let fmt = match token {
                "true" | "false" | "null" | "yes" | "no" | "on" | "off" => keyword.clone(),
                _ => {
                    // Check if before colon (key)
                    let next_char = if i < bytes.len() {
                        Some(bytes[i] as char)
                    } else {
                        None
                    };
                    if colon_pos.is_none()
                        && (i == bytes.len()
                            || next_char == Some(':')
                            || next_char.map_or(false, |c| c.is_whitespace()))
                    {
                        key.clone()
                    } else {
                        normal.clone()
                    }
                }
            };
            job.append(token, 0.0, fmt);
        } else {
            // Unknown character, treat as plain text
            job.append(&text[i..i + 1], 0.0, normal.clone());
            i += 1;
        }
    }

    job
}

/// Show YAML formatter window
pub fn show_yaml_formatter_window(ctx: &Context, show: &mut bool, content: &str) {
    if !*show {
        return;
    }

    let mut open = true;
    let mut formatted_text = format_yaml(content)
        .unwrap_or_else(|e| format!("Format error: {}\n\nOriginal content:\n{}", e, content));

    let mut should_close = false;
    Window::new("YAML Formatter")
        .id(egui::Id::new("yaml_formatter_window"))
        .open(&mut open)
        .default_pos(screen_center(ctx))
        .pivot(Align2::CENTER_CENTER)
        .default_size([800.0, 600.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Copy").clicked() {
                    ui.ctx().copy_text(formatted_text.clone());
                }
                if ui.button("Close").clicked() {
                    should_close = true;
                }
            });
            ui.separator();

            let text_for_highlight = formatted_text.clone();
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_sized(
                    ui.available_size(),
                    egui::TextEdit::multiline(&mut formatted_text)
                        .desired_rows(20)
                        .layouter(&mut |ui, _text, wrap_width| {
                            let mut job = highlight_yaml(&text_for_highlight, ui.visuals());
                            job.wrap.max_width = wrap_width;
                            ui.fonts_mut(|f| f.layout_job(job))
                        }),
                );
            });
        });

    if !open || should_close {
        *show = false;
    }
}
