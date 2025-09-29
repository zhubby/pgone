use crate::{IntelliGuiApp, models::{Message, MessageContent, Role}};

impl IntelliGuiApp {
    pub fn ui_chat(&mut self, ui: &mut egui::Ui) {
        egui_extras::StripBuilder::new(ui)
            .size(egui_extras::Size::remainder())
            .size(egui_extras::Size::exact(120.0))
            .vertical(|mut strip| {
                strip.cell(|ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            let messages: Vec<Message> = self.state.sessions.get(self.state.current_index)
                                .map(|s| s.messages.clone()).unwrap_or_default();
                            for msg in &messages {
                                ui.horizontal(|ui| {
                                    ui.strong(match msg.role { Role::User => "User", Role::Assistant => "Assistant", Role::System => "System" });
                                    ui.label(msg.timestamp.format("%Y-%m-%d %H:%M:%S").to_string());
                                    if ui.small_button("Copy").clicked() { if let MessageContent::Markdown(text) = &msg.content { ui.ctx().copy_text(text.clone()); } }
                                });
                                match &msg.content {
                                    MessageContent::Markdown(text) => crate::markdown::render_markdown(ui, text),
                                    MessageContent::Image { path, width, height } => {
                                        if let Some(handle) = self.media.ensure_texture(ui.ctx(), path) {
                                            let size = egui::vec2(*width as f32, *height as f32).min(egui::vec2(512.0, 512.0));
                                            let img = egui::widgets::Image::new(&handle).fit_to_exact_size(size);
                                            let resp = ui.add(img);
                                            if resp.clicked() { self.preview = Some(super::super::PreviewState { path: path.clone(), zoom: 1.0 }); }
                                        } else { ui.label(format!("[image missing] {}", path.display())); }
                                    }
                                    MessageContent::Video { path, .. } => { if ui.link(path.display().to_string()).clicked() { let _ = open::that(path); } }
                                }
                                ui.separator();
                            }
                        });
                });
                strip.cell(|ui| {
                    ui.label("Message");
                    let editor = ui.add(egui::TextEdit::multiline(&mut self.input).desired_rows(4));
                    
                    // 安全地检查键盘事件，避免Shift键导致的卡死问题
                    let send_via_shortcut = if editor.has_focus() {
                        let input = ui.input(|i| i.clone());
                        let enter_pressed = input.key_pressed(egui::Key::Enter);
                        let cmd_pressed = input.modifiers.command;
                        let shift_pressed = input.modifiers.shift;
                        
                        // 避免在按下Shift键时触发快捷键
                        if shift_pressed {
                            false
                        } else {
                            match self.state.settings.send_shortcut {
                                super::super::models::SendShortcut::Enter => enter_pressed && !cmd_pressed,
                                super::super::models::SendShortcut::CmdEnter => enter_pressed && cmd_pressed,
                            }
                        }
                    } else {
                        false
                    };
                    
                    if ui.button("Send").clicked() || send_via_shortcut { 
                        self.commit_input(); 
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Send (OpenAI)").clicked() { self.send_openai(); }
                        if ui.button("Send MCP").clicked() { /* TODO */ }
                    });
                    ui.small("[Planned] Connect OpenAI and MCP clients here");
                });
            });
    }
}


