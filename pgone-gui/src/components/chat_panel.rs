use crate::components::ChatCtx;
use crate::futures;
use crate::models::{Message, MessageContent, Role};
use chrono::Utc;
use tokio::sync::mpsc;

pub struct ChatPanel {
    pub input: String,
    openai_receiver: Option<mpsc::Receiver<Result<String, String>>>,
    available_models: Vec<String>,
    models_receiver: Option<mpsc::Receiver<Result<Vec<String>, String>>>,
    models_loaded: bool,
}

impl Default for ChatPanel {
    fn default() -> Self {
        Self {
            input: String::new(),
            openai_receiver: None,
            available_models: Vec::new(),
            models_receiver: None,
            models_loaded: false,
        }
    }
}

impl Clone for ChatPanel {
    fn clone(&self) -> Self {
        Self {
            input: self.input.clone(),
            openai_receiver: None, // Receivers cannot be cloned, reset on clone
            available_models: self.available_models.clone(),
            models_receiver: None, // Receivers cannot be cloned, reset on clone
            models_loaded: self.models_loaded,
        }
    }
}
// Default is derived

impl ChatPanel {
    pub fn ui(&mut self, ctxs: &mut ChatCtx, ui: &mut egui::Ui) {
        // 在首次显示时加载模型列表
        if !self.models_loaded
            && ctxs.openai_api_key.is_some()
            && self.models_receiver.is_none()
        {
            self.load_models(ctxs);
        }

        // 检查模型加载结果
        if let Some(ref mut receiver) = self.models_receiver {
            match receiver.try_recv() {
                Ok(result) => {
                    match result {
                        Ok(models) => {
                            self.available_models = models;
                            self.models_loaded = true;
                        }
                        Err(e) => {
                            tracing::error!("加载模型列表失败: {}", e);
                            // 如果加载失败，使用默认模型列表
                            self.available_models = vec![
                                "Unknown".to_string(),
                            ];
                            self.models_loaded = true;
                        }
                    }
                    self.models_receiver = None;
                }
                Err(mpsc::error::TryRecvError::Empty) => {
                    // 还没有结果，继续等待
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    // Channel已断开，清理
                    self.models_receiver = None;
                }
            }
        }

        ui.heading(format!("{} Chat", egui_phosphor::regular::CHATS));
        ui.separator();
        egui_extras::StripBuilder::new(ui)
            .size(egui_extras::Size::remainder())
            .size(egui_extras::Size::exact(200.0))
            .size(egui_extras::Size::exact(50.0))
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
                    ui.horizontal(|ui| {

                        ui.label("Message");
                        
                    });

                    ui.separator();

                    let editor = ui.add_sized(egui::Vec2::new(ui.available_width(), ui.available_height()), egui::TextEdit::multiline(&mut self.input).desired_rows(4));
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

                    if send_via_shortcut {
                        self.send_openai_with_tools(ctxs);
                    }
                    
                    // 检查OpenAI请求结果
                    if let Some(ref mut receiver) = self.openai_receiver {
                        match receiver.try_recv() {
                            Ok(result) => {
                                match result {
                                    Ok(text) => {
                                        if let Some(sess) = ctxs.state.sessions.get_mut(ctxs.state.current_index) {
                                            let message = Message {
                                                role: Role::Assistant,
                                                timestamp: Utc::now(),
                                                content: MessageContent::Markdown(text.clone()),
                                            };
                                            sess.messages.push(message.clone());
                                            sess.updated_at = Utc::now();
                                            
                                            // 保存到持久化存储
                                            if let Err(e) = ctxs.storage.save_session(sess) {
                                                tracing::error!("保存会话失败: {}", e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        let message = format!("OpenAI error: {}", e);
                                        crate::notify::error(&message);
                                        tracing::error!("{}", message);
                                    }
                                }
                                self.openai_receiver = None;
                            }
                            Err(mpsc::error::TryRecvError::Empty) => {
                                // 还没有结果，继续等待
                            }
                            Err(mpsc::error::TryRecvError::Disconnected) => {
                                // Channel已断开，清理
                                self.openai_receiver = None;
                            }
                        }
                    }
                });

                strip.cell(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Send").clicked() {
                            self.send_openai_with_tools(ctxs);
                        }
                        
                        // 模型选择下拉框
                        ui.add_space(10.0);
                        let available_models = if self.available_models.is_empty() {
                            vec![
                                "gpt-4o-mini".to_string(),
                                "gpt-4o".to_string(),
                                "gpt-4-turbo".to_string(),
                                "gpt-4".to_string(),
                                "gpt-3.5-turbo".to_string(),
                            ]
                        } else {
                            self.available_models.clone()
                        };
                        
                        let mut selected_model = ctxs.openai_model.clone();
                        egui::ComboBox::from_id_salt("model_selector")
                            .selected_text(&selected_model)
                            .width(150.0)
                            .show_ui(ui, |ui| {
                                for model in available_models.iter() {
                                    let model_str = model.clone();
                                    ui.selectable_value(
                                        &mut selected_model,
                                        model_str.clone(),
                                        model_str,
                                    );
                                }
                            });
                        
                        // 如果模型改变了，更新到ctxs和settings
                        if selected_model != ctxs.openai_model {
                            ctxs.openai_model = selected_model.clone();
                            ctxs.state.settings.openai_model = selected_model;
                        }
                    });
                })
            });
    }

    #[allow(dead_code)]
    pub fn commit_input(&mut self, ctxs: &mut ChatCtx) {
        let text = self.input.trim();
        if !text.is_empty()
            && let Some(session) = ctxs.state.sessions.get_mut(ctxs.state.current_index)
        {
            let message = Message {
                role: Role::User,
                timestamp: Utc::now(),
                content: MessageContent::Markdown(text.to_owned()),
            };
            session.messages.push(message);
            session.updated_at = Utc::now();
            
            // 保存到持久化存储
            if let Err(e) = ctxs.storage.save_session(session) {
                tracing::error!("保存会话失败: {}", e);
            }
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
            
            // 保存到持久化存储
            if let Err(e) = ctxs.storage.save_session(session) {
                tracing::error!("保存会话失败: {}", e);
            }
        }
    }

    #[allow(dead_code)]
    pub fn add_video_message(&mut self, ctxs: &mut ChatCtx, path: std::path::PathBuf) {
        if let Some(session) = ctxs.state.sessions.get_mut(ctxs.state.current_index) {
            let message = Message {
                role: Role::User,
                timestamp: Utc::now(),
                content: MessageContent::Video {
                    path,
                    duration_ms: None,
                    thumbnail: None,
                },
            };
            session.messages.push(message);
            session.updated_at = Utc::now();
            
            // 保存到持久化存储
            if let Err(e) = ctxs.storage.save_session(session) {
                tracing::error!("保存会话失败: {}", e);
            }
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
            session_id = Some(sess.id.clone());
        }
        let res: Result<String, String> = futures::block_on_async(async move {
            pgone_llm::chat_once(key, model, prompt).await
        });
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


    pub fn send_openai_with_tools(&mut self, ctxs: &mut ChatCtx) {
        let Some(key) = ctxs.openai_api_key.clone() else {
            return;
        };
        let base_url = ctxs.state.settings.openai_base_url.clone();
        let model = ctxs.openai_model.clone();
        let prompt = self.input.trim().to_string();
        if prompt.is_empty() {
            return;
        }
        
        // 保存用户消息
        if let Some(sess) = ctxs.state.sessions.get_mut(ctxs.state.current_index) {
            let user_message = Message {
                role: Role::User,
                timestamp: Utc::now(),
                content: MessageContent::Markdown(prompt.clone()),
            };
            sess.messages.push(user_message);
            sess.updated_at = Utc::now();
            
            // 保存到持久化存储
            if let Err(e) = ctxs.storage.save_session(sess) {
                tracing::error!("保存用户消息失败: {}", e);
            }
        }
        
        self.input.clear();
        let key_clone = key.clone();
        let model_clone = model.clone();
        let prompt_clone = prompt.clone();

        let (sender, receiver) = mpsc::channel(1);
        self.openai_receiver = Some(receiver);

        // let free_model = "glm-4.5-flash";
        
        futures::spawn(async move {
            if let Some(base_url) = base_url {
                let result = pgone_llm::chat_with_tools_custom_endpoint(key_clone, base_url, model_clone, prompt_clone).await;
                let _ = sender.send(result).await;
            } else {
                let result = pgone_llm::chat_with_tools(key_clone, model_clone, prompt_clone).await;
                let _ = sender.send(result).await;
            }
        });
    }

    fn load_models(&mut self, ctxs: &ChatCtx) {
        let Some(api_key) = ctxs.openai_api_key.clone() else {
            return;
        };
        let base_url = ctxs.state.settings.openai_base_url.clone();
        let provider = ctxs.state.settings.llm_provider;
        let (sender, receiver) = mpsc::channel(1);
        self.models_receiver = Some(receiver);
        
        futures::spawn(async move {
            let mut config = pgone_llm::Config::new(api_key);
            if let Some(url) = base_url {
                config = config.with_base_url(url);
            }
            
            let result = match pgone_llm::Client::new(config, provider) {
                Ok(client) => {
                    match client.models_list().await {
                        Ok(models) => {
                            let model_ids: Vec<String> = models
                                .into_iter()
                                .map(|m| m.id)
                                .collect();
                            Ok(model_ids)
                        }
                        Err(e) => Err(e.to_string()),
                    }
                }
                Err(e) => Err(e.to_string()),
            };
            
            let _ = sender.send(result).await;
        });
    }
}
