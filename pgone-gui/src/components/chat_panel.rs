use crate::components::ChatCtx;
use crate::models::{Message, MessageContent, Role};
use chrono::Utc;

#[derive(Clone, Default)]
pub struct ChatPanel {
    pub input: String,
    // pending: Option<std::sync::mpsc::Receiver<Result<String, String>>>,
}
// Default is derived

impl ChatPanel {
    pub fn ui(&mut self, ctxs: &mut ChatCtx, ui: &mut egui::Ui) {
        egui_extras::StripBuilder::new(ui)
            .size(egui_extras::Size::remainder())
            .size(egui_extras::Size::exact(120.0))
            .vertical(|mut strip| {
                strip.cell(|ui| {
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
                });
                strip.cell(|ui| {
                    ui.label("Message");
                    let editor = ui.add(egui::TextEdit::multiline(&mut self.input).desired_rows(4));
                    let send_via_shortcut = if editor.has_focus() {
                        let input = ui.input(|i| i.clone());
                        let enter_pressed = input.key_pressed(egui::Key::Enter);
                        let cmd_pressed = input.modifiers.command;
                        let shift_pressed = input.modifiers.shift;
                        if shift_pressed {
                            false
                        } else {
                            match ctxs.send_shortcut {
                                crate::models::SendShortcut::Enter => enter_pressed && !cmd_pressed,
                                crate::models::SendShortcut::CmdEnter => {
                                    enter_pressed && cmd_pressed
                                }
                            }
                        }
                    } else {
                        false
                    };
                    if ui.button("Ask").clicked() || send_via_shortcut {
                        self.send_openai_with_tools(ctxs);
                    }
                    // poll pending result
                    // if let Some(rx) = &self.pending {
                    //     if let Ok(Ok(text)) = rx.try_recv() {
                    //         if let Some(sess) = ctxs.state.sessions.get_mut(ctxs.state.current_index) {
                    //             sess.messages.push(Message { role: Role::Assistant, timestamp: Utc::now(), content: MessageContent::Markdown(text) });
                    //         }
                    //         self.pending = None;
                    //     }
                    // }
                });
            });
    }

    #[allow(dead_code)]
    pub fn commit_input(&mut self, ctxs: &mut ChatCtx) {
        let text = self.input.trim();
        if !text.is_empty()
            && let Some(session) = ctxs.state.sessions.get_mut(ctxs.state.current_index)
        {
            session.messages.push(Message {
                role: Role::User,
                timestamp: Utc::now(),
                content: MessageContent::Markdown(text.to_owned()),
            });
        }
        self.input.clear();
    }

    #[allow(dead_code)]
    pub fn add_image_message(&mut self, ctxs: &mut ChatCtx, path: std::path::PathBuf) {
        let (w, h) = match image::open(&path) {
            Ok(img) => (img.width(), img.height()),
            Err(_) => (0, 0),
        };
        if let Some(session) = ctxs.state.sessions.get_mut(ctxs.state.current_index) {
            session.messages.push(Message {
                role: Role::User,
                timestamp: Utc::now(),
                content: MessageContent::Image {
                    path,
                    width: w,
                    height: h,
                },
            });
        }
    }

    #[allow(dead_code)]
    pub fn add_video_message(&mut self, ctxs: &mut ChatCtx, path: std::path::PathBuf) {
        if let Some(session) = ctxs.state.sessions.get_mut(ctxs.state.current_index) {
            session.messages.push(Message {
                role: Role::User,
                timestamp: Utc::now(),
                content: MessageContent::Video {
                    path,
                    duration_ms: None,
                    thumbnail: None,
                },
            });
        }
    }

    #[allow(dead_code)]
    pub fn send_openai(&mut self, ctxs: &mut ChatCtx) {
        let Some(key) = ctxs.openai_api_key.clone() else {
            return;
        };
        let model = ctxs.openai_model.clone();
        let prompt = self.input.trim().to_string();
        if prompt.is_empty() {
            return;
        }
        let mut session_id = None;
        if let Some(sess) = ctxs.state.sessions.get(ctxs.state.current_index) {
            session_id = Some(sess.id);
        }
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(_) => {
                return;
            }
        };
        let res: Result<String, String> =
            rt.block_on(async move { crate::openai_client::chat_once(key, model, prompt).await });
        match res {
            Ok(answer) => {
                if let Some(id) = session_id
                    && let Some(sess) = ctxs.state.sessions.iter_mut().find(|s| s.id == id)
                {
                    sess.messages.push(Message {
                        role: Role::Assistant,
                        timestamp: Utc::now(),
                        content: MessageContent::Markdown(answer),
                    });
                }
            }
            Err(_e) => {}
        }
    }

    #[allow(dead_code)]
    pub fn send_mcp(&mut self, ctxs: &mut ChatCtx) {
        // 简单示例：请求 introspect_all 并把 markdown/结果消息追加到当前会话
        let _conn_id = ctxs
            .state
            .sessions
            .get(ctxs.state.current_index)
            .map(|s| s.db.dsn.clone())
            .filter(|s| !s.is_empty());
        let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let ret: Result<String, String> = rt.block_on(async move {
                let cli = match crate::mcp_client::McpClient::spawn_with_default().await {
                    Ok(c) => c,
                    Err(e) => return Err(e.to_string()),
                };
                let params = serde_json::json!({ "connectionId": "default", "format": "markdown" });
                match cli.call("introspect_all", params).await {
                    Ok(v) => {
                        if let Some(md) = v.get("markdown").and_then(|x| x.as_str()) {
                            Ok(md.to_string())
                        } else {
                            Ok(v.to_string())
                        }
                    }
                    Err(e) => Err(e.to_string()),
                }
            });
            let _ = tx.send(ret);
        });
        if let Ok(Ok(text)) = rx.recv()
            && let Some(sess) = ctxs.state.sessions.get_mut(ctxs.state.current_index)
        {
            sess.messages.push(Message {
                role: Role::Assistant,
                timestamp: Utc::now(),
                content: MessageContent::Markdown(text),
            });
        }
    }

    pub fn send_openai_with_tools(&mut self, ctxs: &mut ChatCtx) {
        let Some(key) = ctxs.openai_api_key.clone() else {
            return;
        };
        let model = ctxs.openai_model.clone();
        let prompt = self.input.trim().to_string();
        if prompt.is_empty() {
            return;
        }
        // append user message first
        if let Some(session) = ctxs.state.sessions.get_mut(ctxs.state.current_index) {
            session.messages.push(Message {
                role: Role::User,
                timestamp: Utc::now(),
                content: MessageContent::Markdown(prompt.clone()),
            });
        }
        self.input.clear();
        let (tx, _rx) = std::sync::mpsc::channel::<Result<String, String>>();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let ret = rt.block_on(async move {
                crate::openai_client::chat_with_tools(key, model, prompt).await
            });
            let _ = tx.send(ret);
        });
        // self.pending = Some(rx);
    }
}
