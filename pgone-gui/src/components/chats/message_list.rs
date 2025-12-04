use crate::components::ChatCtx;
use crate::models::{Message, MessageContent, Role};

pub struct MessageList;

impl MessageList {
    pub fn ui(ctxs: &mut ChatCtx, ui: &mut egui::Ui) {
        let should_scroll = ctxs.should_scroll_to_bottom;
        if should_scroll {
            ctxs.should_scroll_to_bottom = false;
        }
        
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom(true)
            .id_salt("message_list_scroll")
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
                            Role::User => format!("{} User",egui_phosphor::regular::USER),
                            Role::Assistant => format!("{} Assistant",egui_phosphor::regular::ROBOT),
                            Role::System => format!("{} System",egui_phosphor::regular::USER_GEAR),
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
                            crate::skeletons::formatters::md::render_markdown(ui, text)
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
                // 如果需要滚动到底部，在最后添加一个标记并滚动到它
                if should_scroll {
                    ui.allocate_space(egui::Vec2::ZERO);
                    ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                }
            });
    }
}

