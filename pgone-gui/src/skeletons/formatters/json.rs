use eframe::egui::{Align2, Context, TextFormat, Window};
use eframe::egui::text::LayoutJob;
use super::screen_center;

/// 格式化 JSON 文本
pub fn format_json(text: &str) -> Result<String, String> {
    // 尝试解析 JSON
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|e| format!("JSON 解析错误: {}", e))?;
    
    // 格式化输出
    serde_json::to_string_pretty(&value)
        .map_err(|e| format!("JSON 格式化错误: {}", e))
}

/// JSON 语法高亮
pub fn highlight_json(text: &str, visuals: &egui::Visuals) -> LayoutJob {
    let mut job = LayoutJob::default();
    
    // 定义文本格式
    let normal = TextFormat {
        color: visuals.text_color(),
        ..Default::default()
    };
    let string = TextFormat {
        color: egui::Color32::from_rgb(152, 195, 121), // 绿色 - 字符串
        ..Default::default()
    };
    let number = TextFormat {
        color: egui::Color32::from_rgb(209, 154, 102), // 橙色 - 数字
        ..Default::default()
    };
    let keyword = TextFormat {
        color: egui::Color32::from_rgb(198, 120, 221), // 紫色 - 关键字 (true, false, null)
        ..Default::default()
    };
    let punctuation = TextFormat {
        color: egui::Color32::from_rgb(180, 180, 180), // 浅灰色 - 标点符号
        ..Default::default()
    };
    
    let bytes = text.as_bytes();
    let mut i = 0;
    
    while i < bytes.len() {
        let c = bytes[i] as char;
        
        // 处理空白字符
        if c.is_whitespace() {
            job.append(&text[i..i + 1], 0.0, normal.clone());
            i += 1;
            continue;
        }
        
        // 处理字符串（双引号）
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
        
        // 处理数字
        if c.is_ascii_digit() || (c == '-' && i + 1 < bytes.len() && (bytes[i + 1] as char).is_ascii_digit()) {
            let start = i;
            i += 1;
            
            // 整数部分
            while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
                i += 1;
            }
            
            // 小数部分
            if i < bytes.len() && bytes[i] as char == '.' {
                i += 1;
                while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
                    i += 1;
                }
            }
            
            // 科学计数法
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
        
        // 处理标点符号
        if matches!(c, '{' | '}' | '[' | ']' | ',' | ':') {
            job.append(&text[i..i + 1], 0.0, punctuation.clone());
            i += 1;
            continue;
        }
        
        // 处理关键字 (true, false, null)
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
            // 未知字符，按普通文本处理
            job.append(&text[i..i + 1], 0.0, normal.clone());
            i += 1;
        }
    }
    
    job
}

/// 显示 JSON 格式化器弹窗
pub fn show_json_formatter_window(
    ctx: &Context,
    show: &mut bool,
    content: &str,
) {
    if !*show {
        return;
    }
    
    let mut open = true;
    let mut formatted_text = format_json(content).unwrap_or_else(|e| {
        format!("格式化错误: {}\n\n原始内容:\n{}", e, content)
    });
    
    let mut should_close = false;
    Window::new("JSON 格式化器")
        .open(&mut open)
        .default_pos(screen_center(ctx))
        .pivot(Align2::CENTER_CENTER)
        .default_size([800.0, 600.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("复制").clicked() {
                    ui.ctx().copy_text(formatted_text.clone());
                }
                if ui.button("关闭").clicked() {
                    should_close = true;
                }
            });
            ui.separator();
            
            let text_for_highlight = formatted_text.clone();
            egui::ScrollArea::vertical()
                .show(ui, |ui| {
                    ui.add_sized(
                        ui.available_size(),
                        egui::TextEdit::multiline(&mut formatted_text)
                            .desired_rows(20)
                            .layouter(&mut |ui, _text, wrap_width| {
                                let mut job = highlight_json(&text_for_highlight, ui.visuals());
                                job.wrap.max_width = wrap_width;
                                ui.fonts(|f| f.layout_job(job))
                            }),
                    );
                });
        });
    
    if !open || should_close {
        *show = false;
    }
}

