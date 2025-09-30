use crate::models::{Message, MessageContent, Role};
use chrono::Utc;

pub struct ChatPanel {
    pub input: String,
}

impl Default for ChatPanel {
    fn default() -> Self { Self { input: String::new() } }
}

impl ChatPanel {
    pub fn ui(&mut self, app: &mut crate::IntelliGuiApp, ui: &mut egui::Ui) {
        egui_extras::StripBuilder::new(ui)
            .size(egui_extras::Size::remainder())
            .size(egui_extras::Size::exact(120.0))
            .vertical(|mut strip| {
                strip.cell(|ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            let messages: Vec<Message> = app.state.sessions.get(app.state.current_index)
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
                                        if let Some(handle) = app.preview.ensure_texture(ui.ctx(), path) {
                                            let size = egui::vec2(*width as f32, *height as f32).min(egui::vec2(512.0, 512.0));
                                            let img = egui::widgets::Image::new(&handle).fit_to_exact_size(size);
                                            let resp = ui.add(img);
                                            if resp.clicked() { app.preview.open(path.clone()); }
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
                    let send_via_shortcut = if editor.has_focus() {
                        let input = ui.input(|i| i.clone());
                        let enter_pressed = input.key_pressed(egui::Key::Enter);
                        let cmd_pressed = input.modifiers.command;
                        let shift_pressed = input.modifiers.shift;
                        if shift_pressed { false } else {
                            match app.state.settings.send_shortcut {
                                crate::models::SendShortcut::Enter => enter_pressed && !cmd_pressed,
                                crate::models::SendShortcut::CmdEnter => enter_pressed && cmd_pressed,
                            }
                        }
                    } else { false };
                    if ui.button("Send").clicked() || send_via_shortcut { self.commit_input(app); }
                    ui.horizontal(|ui| {
                        if ui.button("Send (OpenAI)").clicked() { self.send_openai(app); }
                        if ui.button("Send MCP").clicked() { /* TODO */ }
                    });
                    ui.small("[Planned] Connect OpenAI and MCP clients here");
                });
            });
    }

    pub fn commit_input(&mut self, app: &mut crate::IntelliGuiApp) {
        let text = self.input.trim();
        if !text.is_empty() {
            if let Some(session) = app.state.sessions.get_mut(app.state.current_index) {
                session.messages.push(Message { role: Role::User, timestamp: Utc::now(), content: MessageContent::Markdown(text.to_owned()) });
            }
            app.save_state();
        }
        self.input.clear();
    }

    pub fn add_image_message(&mut self, app: &mut crate::IntelliGuiApp, path: std::path::PathBuf) {
        let (w, h) = match image::open(&path) { Ok(img) => (img.width(), img.height()), Err(_) => (0, 0) };
        if let Some(session) = app.state.sessions.get_mut(app.state.current_index) {
            session.messages.push(Message { role: Role::User, timestamp: Utc::now(), content: MessageContent::Image { path, width: w, height: h } });
        }
        app.save_state();
    }

    pub fn add_video_message(&mut self, app: &mut crate::IntelliGuiApp, path: std::path::PathBuf) {
        if let Some(session) = app.state.sessions.get_mut(app.state.current_index) {
            session.messages.push(Message { role: Role::User, timestamp: Utc::now(), content: MessageContent::Video { path, duration_ms: None, thumbnail: None } });
        }
        app.save_state();
    }

    pub fn send_openai(&mut self, app: &mut crate::IntelliGuiApp) {
        let Some(key) = app.state.settings.openai_api_key.clone() else { app.sql.sql_error = Some("OpenAI API key not set".into()); return; };
        let model = app.state.settings.openai_model.clone();
        let prompt = self.input.trim().to_string();
        if prompt.is_empty() { return; }
        let mut session_id = None;
        if let Some(sess) = app.state.sessions.get(app.state.current_index) { session_id = Some(sess.id); }
        let rt = match tokio::runtime::Runtime::new() { Ok(rt) => rt, Err(e) => { app.sql.sql_error = Some(format!("runtime error: {}", e)); return; } };
        let res: Result<String, String> = rt.block_on(async move { crate::openai_client::chat_once(key, model, prompt).await });
        match res {
            Ok(answer) => {
                if let Some(id) = session_id {
                    if let Some(sess) = app.state.sessions.iter_mut().find(|s| s.id == id) {
                        sess.messages.push(Message { role: Role::Assistant, timestamp: Utc::now(), content: MessageContent::Markdown(answer) });
                        app.save_state();
                    }
                }
            }
            Err(e) => { app.sql.sql_error = Some(format!("openai error: {}", e)); }
        }
    }
}


