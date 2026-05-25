use eframe::egui::{Align2, Context, TextFormat, Window};
use eframe::egui::text::LayoutJob;
use super::screen_center;

/// 格式化 TOML 文本
pub fn format_toml(text: &str) -> Result<String, String> {
    // 尝试解析 TOML
    let value: toml::Value = toml::from_str(text)
        .map_err(|e| format!("TOML 解析错误: {}", e))?;
    
    // 格式化输出
    toml::to_string_pretty(&value)
        .map_err(|e| format!("TOML 格式化错误: {}", e))
}

/// TOML 语法高亮
pub fn highlight_toml(text: &str, visuals: &egui::Visuals) -> LayoutJob {
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
        color: egui::Color32::from_rgb(198, 120, 221), // 紫色 - 关键字 (true, false)
        ..Default::default()
    };
    let comment = TextFormat {
        color: egui::Color32::from_rgb(128, 128, 128), // 灰色 - 注释
        ..Default::default()
    };
    let table_header = TextFormat {
        color: egui::Color32::from_rgb(86, 182, 194), // 青色 - 表头
        ..Default::default()
    };
    let key = TextFormat {
        color: egui::Color32::from_rgb(86, 182, 194), // 青色 - 键
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
        
        // 处理注释
        if c == '#' {
            let start = i;
            while i < bytes.len() && bytes[i] as char != '\n' {
                i += 1;
            }
            job.append(&text[start..i], 0.0, comment.clone());
            continue;
        }
        
        // 处理空白字符
        if c.is_whitespace() {
            job.append(&text[i..i + 1], 0.0, normal.clone());
            i += 1;
            continue;
        }
        
        // 处理表头 [table] 或 [[array]]
        if c == '[' {
            let start = i;
            i += 1;
            // 处理 [[array]]
            if i < bytes.len() && bytes[i] as char == '[' {
                i += 1;
            }
            // 跳过表名
            while i < bytes.len() {
                let ch = bytes[i] as char;
                if ch == ']' {
                    i += 1;
                    // 处理 ]]
                    if i < bytes.len() && bytes[i] as char == ']' {
                        i += 1;
                    }
                    break;
                }
                i += 1;
            }
            job.append(&text[start..i], 0.0, table_header.clone());
            continue;
        }
        
        // 处理字符串（单引号、双引号或三引号）
        if c == '"' || c == '\'' {
            let string_char = c;
            let start = i;
            i += 1;
            
            // 检查是否是三引号
            let is_triple = i + 1 < bytes.len() 
                && bytes[i] as char == string_char 
                && bytes[i + 1] as char == string_char;
            
            if is_triple {
                i += 2;
                // 三引号字符串
                while i + 2 < bytes.len() {
                    if bytes[i] as char == string_char 
                        && bytes[i + 1] as char == string_char 
                        && bytes[i + 2] as char == string_char {
                        i += 3;
                        break;
                    }
                    i += 1;
                }
            } else {
                // 单引号或双引号字符串
                while i < bytes.len() {
                    let ch = bytes[i] as char;
                    if ch == string_char {
                        // 检查是否是转义的引号
                        if string_char == '\'' && i + 1 < bytes.len() && bytes[i + 1] as char == '\'' {
                            i += 2;
                        } else {
                            i += 1;
                            break;
                        }
                    } else if ch == '\\' && string_char == '"' && i + 1 < bytes.len() {
                        i += 2; // 跳过转义字符
                    } else {
                        i += 1;
                    }
                }
            }
            
            job.append(&text[start..i], 0.0, string.clone());
            continue;
        }
        
        // 处理等号（键值分隔符）
        if c == '=' {
            job.append(&text[i..i + 1], 0.0, punctuation.clone());
            i += 1;
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
        
        // 处理数组标记
        if c == '[' && i > 0 && bytes[i - 1] as char != '[' {
            job.append(&text[i..i + 1], 0.0, punctuation.clone());
            i += 1;
            continue;
        }
        
        if c == ']' {
            job.append(&text[i..i + 1], 0.0, punctuation.clone());
            i += 1;
            continue;
        }
        
        // 处理其他标点符号
        if matches!(c, '{' | '}' | ',' | '.') {
            job.append(&text[i..i + 1], 0.0, punctuation.clone());
            i += 1;
            continue;
        }
        
        // 处理标识符和关键字
        let start = i;
        while i < bytes.len() {
            let ch = bytes[i] as char;
            if ch.is_whitespace() || ch == '=' || ch == '#' || ch == '[' || ch == ']' || ch == ',' || ch == '{' || ch == '}' {
                break;
            }
            i += 1;
        }
        
        if start < i {
            let token = &text[start..i];
            let fmt = match token {
                "true" | "false" => keyword.clone(),
                _ => {
                    // 检查是否在等号之前（键）
                    if i < bytes.len() && bytes[i] as char == '=' {
                        key.clone()
                    } else {
                        normal.clone()
                    }
                }
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

/// 显示 TOML 格式化器弹窗
pub fn show_toml_formatter_window(
    ctx: &Context,
    show: &mut bool,
    content: &str,
) {
    if !*show {
        return;
    }
    
    let mut open = true;
    let mut formatted_text = format_toml(content).unwrap_or_else(|e| {
        format!("格式化错误: {}\n\n原始内容:\n{}", e, content)
    });
    
    let mut should_close = false;
    Window::new("TOML 格式化器")
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
                                let mut job = highlight_toml(&text_for_highlight, ui.visuals());
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
