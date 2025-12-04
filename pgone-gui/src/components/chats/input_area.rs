use crate::components::ChatCtx;
use crate::models::{Message, MessageContent, Role};
use chrono::Utc;
use std::path::PathBuf;

pub struct InputArea {
    pub input: String,
    pub pending_resources: Vec<PathBuf>,
}

impl Default for InputArea {
    fn default() -> Self {
        Self {
            input: String::new(),
            pending_resources: Vec::new(),
        }
    }
}

impl InputArea {
    pub fn ui(&mut self, ctxs: &mut ChatCtx, ui: &mut egui::Ui, should_send: &mut bool) {
        ui.separator();

        ui.horizontal(|ui| {
            ui.label(format!("{} Message", egui_phosphor::regular::PAPER_PLANE_TILT));
        });

        ui.separator();

        // 文件选择按钮
        ui.horizontal(|ui| {
            if ui.button(format!("{} 选择图片", egui_phosphor::regular::IMAGE)).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Image", &["png", "jpg", "jpeg", "gif", "webp", "bmp"])
                    .pick_file()
                {
                    self.pending_resources.push(path);
                }
            }
            
            if ui.button(format!("{} 选择文件", egui_phosphor::regular::FILE)).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .pick_file()
                {
                    self.pending_resources.push(path);
                }
            }
        });

        // 资源列表显示
        if !self.pending_resources.is_empty() {
            ui.separator();
            ui.group(|ui| {
                ui.label("待发送的资源:");
                let mut to_remove = Vec::new();
                for (idx, path) in self.pending_resources.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let file_name = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("未知文件");
                        
                        // 如果是图片，尝试显示缩略图
                        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                            let is_image = matches!(ext.to_lowercase().as_str(), 
                                "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp");
                            
                            if is_image {
                                if let Some(handle) = ctxs.preview.ensure_texture(ui.ctx(), path) {
                                    let thumb_size = egui::vec2(40.0, 40.0);
                                    let img = egui::widgets::Image::new(&handle)
                                        .fit_to_exact_size(thumb_size);
                                    ui.add(img);
                                }
                            } else {
                                ui.label(format!("{}", egui_phosphor::regular::FILE));
                            }
                        }
                        
                        ui.label(file_name);
                        
                        if ui.small_button(format!("{}", egui_phosphor::regular::X)).clicked() {
                            to_remove.push(idx);
                        }
                    });
                }
                // 从后往前删除，避免索引问题
                for &idx in to_remove.iter().rev() {
                    self.pending_resources.remove(idx);
                }
            });
            ui.separator();
        }

        // 输入框
        let editor = ui.add_sized(
            egui::Vec2::new(ui.available_width(), ui.available_height()),
            egui::TextEdit::multiline(&mut self.input).desired_rows(4)
        );
        
        if editor.has_focus() {
            let input = ui.input(|i| i.clone());
            let enter_pressed = input.key_pressed(egui::Key::Enter);
            let shift_pressed = input.modifiers.shift;
            
            // Enter 发送，Shift+Enter 换行
            if enter_pressed && !shift_pressed {
                // 阻止默认的换行行为
                ui.ctx().input_mut(|i| {
                    i.consume_key(egui::Modifiers::NONE, egui::Key::Enter);
                });
                // 移除可能已经插入的换行符
                if self.input.ends_with('\n') {
                    self.input.pop();
                }
                *should_send = true;
            }
            // Shift+Enter 时允许默认换行行为（不处理，让 TextEdit 正常换行）
        }
    }

    pub fn send_resources(&mut self, ctxs: &mut ChatCtx) {
        // 先收集所有资源路径
        let resources: Vec<PathBuf> = self.pending_resources.drain(..).collect();
        
        // 发送每个资源
        for path in resources {
            // 检查是否是图片
            let is_image = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|ext| {
                    matches!(ext.to_lowercase().as_str(), 
                        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp")
                })
                .unwrap_or(false);

            if is_image {
                self.add_image_message(ctxs, path);
            } else {
                // 其他文件作为 Markdown 消息，包含文件路径
                if let Some(session) = ctxs.state.sessions.get_mut(ctxs.state.current_index) {
                    let message = Message {
                        role: Role::User,
                        timestamp: Utc::now(),
                        content: MessageContent::Markdown(
                            format!("[文件] {}", path.display())
                        ),
                    };
                    session.messages.push(message);
                    session.updated_at = Utc::now();
                    
                    if let Err(e) = ctxs.storage.save_session(session) {
                        tracing::error!("保存文件消息失败: {}", e);
                    }
                }
            }
        }
    }

    pub(crate) fn add_image_message(&mut self, ctxs: &mut ChatCtx, path: PathBuf) {
        let (w, h) = match image::open(&path) {
            Ok(img) => (img.width(), img.height()),
            Err(_) => (0, 0),
        };
        if let Some(session) = ctxs.state.sessions.get_mut(ctxs.state.current_index) {
            let message = Message {
                role: Role::User,
                timestamp: Utc::now(),
                content: MessageContent::Image {
                    path,
                    width: w,
                    height: h,
                },
            };
            session.messages.push(message);
            session.updated_at = Utc::now();
            
            if let Err(e) = ctxs.storage.save_session(session) {
                tracing::error!("保存图片消息失败: {}", e);
            }
        }
    }
}

