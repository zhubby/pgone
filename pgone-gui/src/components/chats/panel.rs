use crate::components::ChatCtx;
use crate::futures;
use crate::models::{Message, MessageContent, Role};
use chrono::Utc;
use tokio::sync::mpsc;
use pgone_llm::{Client, Config, LLMProvider};

use super::input_area::InputArea;
use super::message_list::MessageList;
use super::model_loader::ModelLoader;
use super::session_selector::SessionSelector;

pub struct ChatPanel {
    pub input_area: InputArea,
    openai_receiver: Option<mpsc::Receiver<Result<String, String>>>,
    model_loader: ModelLoader,
}

impl Default for ChatPanel {
    fn default() -> Self {
        Self {
            input_area: InputArea::default(),
            openai_receiver: None,
            model_loader: ModelLoader::default(),
        }
    }
}

impl Clone for ChatPanel {
    fn clone(&self) -> Self {
        Self {
            input_area: InputArea {
                input: self.input_area.input.clone(),
                pending_resources: self.input_area.pending_resources.clone(),
            },
            openai_receiver: None, // Receivers cannot be cloned, reset on clone
            model_loader: ModelLoader {
                available_models: self.model_loader.available_models.clone(),
                models_receiver: None, // Receivers cannot be cloned, reset on clone
                models_loaded: self.model_loader.models_loaded,
            },
        }
    }
}

impl ChatPanel {
    pub fn ui(&mut self, ctxs: &mut ChatCtx, ui: &mut egui::Ui) {
        // 检查并加载模型
        self.model_loader.check_and_load(ctxs);

        // 标题和 Session 选择器
        ui.horizontal(|ui| {
            ui.heading(format!("{} Chat", egui_phosphor::regular::CHATS));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                SessionSelector::ui(ctxs, ui);
            });
        });
        ui.separator();

        egui_extras::StripBuilder::new(ui)
            .size(egui_extras::Size::remainder())
            .size(egui_extras::Size::exact(200.0))
            .size(egui_extras::Size::exact(50.0))
            .vertical(|mut strip| {
                // 消息列表
                strip.cell(|ui| {
                    MessageList::ui(ctxs, ui);
                });

                // 输入区域
                strip.cell(|ui| {
                    let mut should_send = false;
                    self.input_area.ui(ctxs, ui, &mut should_send);
                    
                    // 检查快捷键发送
                    if should_send {
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

                // 底部按钮栏
                strip.cell(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Send").clicked() {
                            self.send_openai_with_tools(ctxs);
                        }
                        
                        // 模型选择下拉框
                        ui.add_space(10.0);
                        let available_models = if self.model_loader.available_models.is_empty() {
                            vec![
                                "gpt-4o-mini".to_string(),
                                "gpt-4o".to_string(),
                                "gpt-4-turbo".to_string(),
                                "gpt-4".to_string(),
                                "gpt-3.5-turbo".to_string(),
                            ]
                        } else {
                            self.model_loader.available_models.clone()
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
        let text = self.input_area.input.trim();
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
            
            if let Err(e) = ctxs.storage.save_session(session) {
                tracing::error!("保存会话失败: {}", e);
            }
        }
        self.input_area.input.clear();
    }

    #[allow(dead_code)]
    pub fn add_image_message(&mut self, ctxs: &mut ChatCtx, path: std::path::PathBuf) {
        // 直接添加到 pending_resources，让用户确认后再发送
        self.input_area.pending_resources.push(path);
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
        let prompt = self.input_area.input.trim().to_string();
        if prompt.is_empty() {
            return;
        }
        let mut session_id = None;
        if let Some(sess) = ctxs.state.sessions.get(ctxs.state.current_index) {
            session_id = Some(sess.id.clone());
        }
        let provider = ctxs.state.settings.llm_provider;
        let proxy_enabled = ctxs.state.settings.proxy_enabled;
        let proxy_host = ctxs.state.settings.proxy_host.clone();
        let proxy_port = ctxs.state.settings.proxy_port;
        let res: Result<String, String> = futures::block_on_async(async move {
            let mut config = Config::new(key);
            if proxy_enabled {
                if let (Some(host), Some(port)) = (proxy_host, proxy_port) {
                    config = config.with_proxy(host, port);
                }
            }
            let client = match Client::new(config, provider) {
                Ok(c) => c,
                Err(e) => return Err(e.to_string()),
            };
            let request = pgone_llm::chat::ChatRequest::new(model)
                .with_messages(vec![
                    pgone_llm::chat::ChatMessage::system(crate::prompt::system_prompt()),
                    pgone_llm::chat::ChatMessage::user(prompt),
                ]);
            match client.chat_create(request).await {
                Ok(resp) => Ok(resp.content),
                Err(e) => Err(e.to_string()),
            }
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
        let prompt = self.input_area.input.trim().to_string();
        
        // 先发送所有待发送的资源
        self.input_area.send_resources(ctxs);
        
        // 如果文本输入为空且没有资源，直接返回
        if prompt.is_empty() && self.input_area.pending_resources.is_empty() {
            return;
        }
        
        // 保存用户消息（如果有文本）
        if !prompt.is_empty() {
            if let Some(sess) = ctxs.state.sessions.get_mut(ctxs.state.current_index) {
                let user_message = Message {
                    role: Role::User,
                    timestamp: Utc::now(),
                    content: MessageContent::Markdown(prompt.clone()),
                };
                sess.messages.push(user_message);
                sess.updated_at = Utc::now();
                
                if let Err(e) = ctxs.storage.save_session(sess) {
                    tracing::error!("保存用户消息失败: {}", e);
                }
            }
        }
        
        self.input_area.input.clear();
        let key_clone = key.clone();
        let model_clone = model.clone();
        let prompt_clone = prompt.clone();
        let provider = ctxs.state.settings.llm_provider;
        let proxy_enabled = ctxs.state.settings.proxy_enabled;
        let proxy_host = ctxs.state.settings.proxy_host.clone();
        let proxy_port = ctxs.state.settings.proxy_port;

        let (sender, receiver) = mpsc::channel(1);
        self.openai_receiver = Some(receiver);

        futures::spawn(async move {
            let mut config = Config::new(key_clone);
            if let Some(url) = base_url {
                config = config.with_base_url(url);
            }
            if proxy_enabled {
                if let (Some(host), Some(port)) = (proxy_host, proxy_port) {
                    config = config.with_proxy(host, port);
                }
            }
            let client = match Client::new(config, provider) {
                Ok(c) => c,
                Err(e) => {
                    let _ = sender.send(Err(e.to_string())).await;
                    return;
                }
            };
            let request = pgone_llm::chat::ChatRequest::new(model_clone)
                .with_messages(vec![
                    pgone_llm::chat::ChatMessage::system(crate::prompt::system_prompt()),
                    pgone_llm::chat::ChatMessage::user(prompt_clone),
                ]);
            let result = match client.chat_create(request).await {
                Ok(resp) => Ok(resp.content),
                Err(e) => Err(e.to_string()),
            };
            let _ = sender.send(result).await;
        });
    }
}

