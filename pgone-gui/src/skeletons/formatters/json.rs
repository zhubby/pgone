use super::screen_center;
use eframe::egui::text::LayoutJob;
use eframe::egui::{Align2, Context, TextFormat, Window};

/// Format JSON text
pub fn format_json(text: &str) -> Result<String, String> {
    // Attempt to parse JSON
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("JSON parse error: {}", e))?;

    // Format output
    serde_json::to_string_pretty(&value).map_err(|e| format!("JSON format error: {}", e))
}

/// JSON syntax highlighting
pub fn highlight_json(text: &str, visuals: &egui::Visuals) -> LayoutJob {
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
        color: egui::Color32::from_rgb(198, 120, 221), // Purple - keywords (true, false, null)
        ..Default::default()
    };
    let punctuation = TextFormat {
        color: egui::Color32::from_rgb(180, 180, 180), // Light gray - punctuation
        ..Default::default()
    };

    let bytes = text.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let c = bytes[i] as char;

        // Handle whitespace
        if c.is_whitespace() {
            job.append(&text[i..i + 1], 0.0, normal.clone());
            i += 1;
            continue;
        }

        // Handle strings (double quotes)
        if c == '"' {
            let start = i;
            i += 1;
            let mut escaped = false;
            while i < bytes.len() {
                let ch = bytes[i] as char;
                if escaped {
                    escaped = false;
                    i += 1;
                } else if ch == '\\' {
                    escaped = true;
                    i += 1;
                } else if ch == '"' {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
            job.append(&text[start..i], 0.0, string.clone());
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

        // Handle punctuation
        if matches!(c, '{' | '}' | '[' | ']' | ',' | ':') {
            job.append(&text[i..i + 1], 0.0, punctuation.clone());
            i += 1;
            continue;
        }

        // Handle keywords (true, false, null)
        let start = i;
        while i < bytes.len() {
            let ch = bytes[i] as char;
            if !ch.is_alphanumeric() {
                break;
            }
            i += 1;
        }

        if start < i {
            let token = &text[start..i];
            let fmt = match token {
                "true" | "false" | "null" => keyword.clone(),
                _ => normal.clone(),
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

/// Show JSON formatter window
pub fn show_json_formatter_window(ctx: &Context, show: &mut bool, content: &str) {
    if !*show {
        return;
    }

    let mut open = true;
    let mut formatted_text = format_json(content)
        .unwrap_or_else(|e| format!("Format error: {}\n\nOriginal content:\n{}", e, content));

    let mut should_close = false;
    Window::new("JSON Formatter")
        .id(egui::Id::new("json_formatter_window"))
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
                            let mut job = highlight_json(&text_for_highlight, ui.visuals());
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
