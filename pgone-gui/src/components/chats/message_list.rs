use crate::components::ChatCtx;
use crate::models::{Message, MessageContent, Role};

pub struct MessageList;

impl MessageList {
    pub fn ui(ctxs: &mut ChatCtx, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                let messages: Vec<Message> = ctxs
                    .state
                    .sessions
                    .get(ctxs.state.current_index)
                    .map(|s| s.messages.clone())
                    .unwrap_or_default();
                for msg in &messages {
                    ui.horizontal(|ui| {
                        ui.strong(match msg.role {
                            Role::User => "User",
                            Role::Assistant => "Assistant",
                            Role::System => "System",
                        });
                        ui.label(msg.timestamp.format("%Y-%m-%d %H:%M:%S").to_string());
                        if ui.small_button("Copy").clicked()
                            && let MessageContent::Markdown(text) = &msg.content
                        {
                            ui.ctx().copy_text(text.clone());
                        }
                    });
                    match &msg.content {
                        MessageContent::Markdown(text) => {
                            crate::markdown::render_markdown(ui, text)
                        }
                        MessageContent::Image {
                            path,
                            width,
                            height,
                        } => {
                            if let Some(handle) =
                                ctxs.preview.ensure_texture(ui.ctx(), path)
                            {
                                let size = egui::vec2(*width as f32, *height as f32)
                                    .min(egui::vec2(512.0, 512.0));
                                let img = egui::widgets::Image::new(&handle)
                                    .fit_to_exact_size(size);
                                let resp = ui.add(img);
                                if resp.clicked() {
                                    ctxs.preview.open(path.clone());
                                }
                            } else {
                                ui.label(format!("[image missing] {}", path.display()));
                            }
                        }
                        MessageContent::Video { path, .. } => {
                            if ui.link(path.display().to_string()).clicked() {
                                let _ = open::that(path);
                            }
                        }
                    }
                    ui.separator();
                }
            });
    }
}

