use crate::components::ChatCtx;
use crate::futures;
use crate::models::{Message, MessageContent, Role};
use chrono::Utc;
use egui::Widget;
use tokio::sync::mpsc;
use pgone_llm::{Client, Config};
use pgone_mcp::mcp::PgoneMcpServer;
use serde_json::Value;

use super::input_area::InputArea;
use super::message_list::MessageList;
use super::model_loader::ModelLoader;
use super::session_selector::SessionSelector;

pub struct ChatPanel {
    pub input_area: InputArea,
    openai_receiver: Option<mpsc::Receiver<Result<String, String>>>,
    stream_receiver: Option<mpsc::Receiver<Result<String, String>>>,
    model_loader: ModelLoader,
    enable_thinking: bool,
    enable_search: bool,
    show_delete_confirm: bool,
}

impl Default for ChatPanel {
    fn default() -> Self {
        Self {
            input_area: InputArea::default(),
            openai_receiver: None,
            stream_receiver: None,
            model_loader: ModelLoader::default(),
            enable_thinking: false,
            enable_search: false,
            show_delete_confirm: false,
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
            stream_receiver: None, // Receivers cannot be cloned, reset on clone
            model_loader: ModelLoader {
                available_models: self.model_loader.available_models.clone(),
                models_receiver: None, // Receivers cannot be cloned, reset on clone
                models_loaded: self.model_loader.models_loaded,
            },
            enable_thinking: self.enable_thinking,
            enable_search: self.enable_search,
            show_delete_confirm: false, // 重置确认对话框状态
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
                // 关闭按钮
                if ui.button(format!("{}", egui_phosphor::regular::X)).clicked() {
                    self.show_delete_confirm = true;
                }
                ui.add_space(5.0);
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
                    
                    // 检查流式响应
                    if let Some(ref mut receiver) = self.stream_receiver {
                        let mut has_update = false;
                        loop {
                            match receiver.try_recv() {
                                Ok(result) => {
                                    match result {
                                        Ok(chunk) => {
                                            if let Some(sess) = ctxs.state.sessions.get_mut(ctxs.state.current_index) {
                                                // 更新最后一条 assistant 消息，如果不存在则创建
                                                if let Some(last_msg) = sess.messages.last_mut() {
                                                    if matches!(last_msg.role, Role::Assistant) {
                                                        if let MessageContent::Markdown(ref mut content) = last_msg.content {
                                                            content.push_str(&chunk);
                                                            has_update = true;
                                                        }
                                                    } else {
                                                        // 最后一条不是 assistant 消息，创建新的
                                                        let message = Message {
                                                            role: Role::Assistant,
                                                            timestamp: Utc::now(),
                                                            content: MessageContent::Markdown(chunk),
                                                        };
                                                        sess.messages.push(message);
                                                        has_update = true;
                                                    }
                                                } else {
                                                    // 没有消息，创建新的
                                                    let message = Message {
                                                        role: Role::Assistant,
                                                        timestamp: Utc::now(),
                                                        content: MessageContent::Markdown(chunk),
                                                    };
                                                    sess.messages.push(message);
                                                    has_update = true;
                                                }
                                                sess.updated_at = Utc::now();
                                            }
                                        }
                                        Err(e) => {
                                            let message = format!("流式 API 错误: {}", e);
                                            crate::notify::error(&message);
                                            tracing::error!("{}", message);
                                            self.stream_receiver = None;
                                            break;
                                        }
                                    }
                                }
                                Err(mpsc::error::TryRecvError::Empty) => {
                                    // 没有更多数据，退出循环
                                    break;
                                }
                                Err(mpsc::error::TryRecvError::Disconnected) => {
                                    // Channel已断开，流式响应完成，保存最终消息
                                    if let Some(sess) = ctxs.state.sessions.get_mut(ctxs.state.current_index) {
                                        if let Err(e) = ctxs.storage.save_session(sess) {
                                            tracing::error!("保存会话失败: {}", e);
                                        }
                                    }
                                    self.stream_receiver = None;
                                    break;
                                }
                            }
                        }
                        if has_update {
                            ctxs.should_scroll_to_bottom = true;
                        }
                    }
                    
                    // 检查OpenAI请求结果（非流式）
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
                                            // 设置滚动到底部标志
                                            ctxs.should_scroll_to_bottom = true;
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
                        if ui.button(format!("{} Send", egui_phosphor::regular::PAPER_PLANE_RIGHT)).clicked() {
                            self.send_openai_with_tools(ctxs);
                        }

                        egui::widgets::Checkbox::new(&mut self.enable_search, "联网搜索").ui(ui);
                        egui::widgets::Checkbox::new(&mut self.enable_thinking, "深度思考").ui(ui);
                        
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

        // 显示删除确认对话框
        if self.show_delete_confirm {
            let mut open = true;
            let center = ui.ctx().screen_rect().center();
            let current_session_title = ctxs
                .state
                .sessions
                .get(ctxs.state.current_index)
                .map(|s| s.title.clone())
                .unwrap_or_else(|| "当前会话".to_string());

            egui::Window::new("确认删除会话")
                .open(&mut open)
                .default_pos(center)
                .pivot(egui::Align2::CENTER_CENTER)
                .show(ui.ctx(), |ui| {
                    ui.label(format!("确定要删除会话 '{}' 吗？", current_session_title));
                    ui.label(egui::RichText::new("此操作不可恢复，会话及其所有消息将被永久删除。").color(egui::Color32::RED));
                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        if ui.button("取消").clicked() {
                            self.show_delete_confirm = false;
                        }
                        if ui.button(egui::RichText::new("确认删除").color(egui::Color32::RED)).clicked() {
                            self.delete_current_session(ctxs);
                            self.show_delete_confirm = false;
                        }
                    });
                });

            if !open {
                self.show_delete_confirm = false;
            }
        }
    }

    fn delete_current_session(&mut self, ctxs: &mut ChatCtx) {
        if ctxs.state.sessions.is_empty() {
            return;
        }

        let current_index = ctxs.state.current_index;
        if let Some(session) = ctxs.state.sessions.get(current_index) {
            let session_id = session.id.clone();
            
            // 从存储中删除会话
            if let Err(e) = ctxs.storage.delete_session(&session_id) {
                tracing::error!("删除会话失败: {}", e);
                crate::notify::error(&format!("删除会话失败: {}", e));
                return;
            }

            // 从内存中删除会话
            ctxs.state.sessions.remove(current_index);

            // 调整当前索引
            if ctxs.state.sessions.is_empty() {
                // 如果没有会话了，创建一个新的默认会话
                let new_id = ctxs.state.next_session_id.to_string();
                ctxs.state.next_session_id += 1;
                let new_session = crate::models::ChatSession::default_with_timestamp(new_id.clone());
                ctxs.state.sessions.push(new_session.clone());
                ctxs.state.current_index = 0;
                
                // 保存新会话
                if let Err(e) = ctxs.storage.save_session(&new_session) {
                    tracing::error!("保存新会话失败: {}", e);
                }
            } else {
                // 如果删除的不是最后一个，保持索引不变（因为后面的元素会前移）
                // 如果删除的是最后一个，需要将索引减1
                if current_index >= ctxs.state.sessions.len() {
                    ctxs.state.current_index = ctxs.state.sessions.len() - 1;
                }
            }

            crate::notify::info("会话已删除");
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
        
        // 查询历史消息（在保存当前消息之前）
        let mut history_messages = Vec::new();
        if let Some(sess) = ctxs.state.sessions.get(ctxs.state.current_index) {
            match ctxs.storage.query_messages_by_session(&sess.id) {
                Ok(messages) => {
                    // 反转结果（查询结果是降序，需要转为升序）
                    let reversed_messages: Vec<_> = messages.into_iter().rev().collect();
                    // 转换为 ChatMessage，只处理 Markdown 类型
                    for msg in reversed_messages {
                        if let MessageContent::Markdown(content) = &msg.content {
                            let chat_msg = match msg.role {
                                Role::User => pgone_llm::chat::ChatMessage::user(content.clone()),
                                Role::Assistant => pgone_llm::chat::ChatMessage::assistant(content.clone()),
                                Role::System => pgone_llm::chat::ChatMessage::system(content.clone()),
                            };
                            history_messages.push(chat_msg);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("查询历史消息失败: {}", e);
                }
            }
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
                // 发送消息后也滚动到底部
                ctxs.should_scroll_to_bottom = true;
            }
        }
        
        // 检查是否选择了数据库
        let dbconfig_id = ctxs.active_db_config_id.clone();
        if dbconfig_id.is_none() {
            crate::notify::error("请先选择一个数据库配置");
            return;
        }
        let dbconfig_id = dbconfig_id.unwrap();
        
        self.input_area.input.clear();
        let key_clone = key.clone();
        let model_clone = model.clone();
        let prompt_clone = prompt.clone();
        let provider = ctxs.state.settings.llm_provider;
        let proxy_enabled = ctxs.state.settings.proxy_enabled;
        let proxy_host = ctxs.state.settings.proxy_host.clone();
        let proxy_port = ctxs.state.settings.proxy_port;
        let tools = pgone_mcp::mcp::list_tools();
        let dbconfig_id_clone = dbconfig_id.clone();
        let enable_stream_api = ctxs.state.settings.enable_stream_api;

        // 获取当前会话ID（在spawn之前）
        let session_id = if let Some(sess) = ctxs.state.sessions.get(ctxs.state.current_index) {
            Some(sess.id.clone())
        } else {
            None
        };

        // 构建消息列表：系统提示 + 历史消息 + 当前用户输入
        let mut chat_messages = vec![
            pgone_llm::chat::ChatMessage::system(crate::prompt::system_prompt()),
        ];
        chat_messages.extend(history_messages);
        if !prompt_clone.is_empty() {
            chat_messages.push(pgone_llm::chat::ChatMessage::user(prompt_clone.clone()));
        }

        // 如果启用流式 API，使用流式模式
        if enable_stream_api {
            // 创建临时的 assistant 消息
            if let Some(sess) = ctxs.state.sessions.get_mut(ctxs.state.current_index) {
                let temp_message = Message {
                    role: Role::Assistant,
                    timestamp: Utc::now(),
                    content: MessageContent::Markdown(String::new()),
                };
                sess.messages.push(temp_message);
                sess.updated_at = Utc::now();
            }

            let (stream_sender, stream_receiver) = mpsc::channel(100);
            self.stream_receiver = Some(stream_receiver);

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
                        let _ = stream_sender.send(Err(e.to_string())).await;
                        return;
                    }
                };

                // 创建流式请求
                let mut request = pgone_llm::chat::ChatRequest::new(model_clone.clone())
                    .with_messages(chat_messages.clone())
                    .with_tools(tools.iter().map(|t| t.clone().into()).collect());
                
                // 设置会话ID用于审计
                if let Some(ref sid) = session_id {
                    request = request.with_session_id(sid.clone());
                }

                // 获取流式响应
                let stream = client.chat_create_stream(request);
                use futures_util::StreamExt;
                use std::pin::Pin;
                // stream 已经是 Box<dyn Stream>，使用 Pin::from 转换为 Pin<Box<dyn Stream>>
                // 注意：这里需要明确指定 Stream trait 的类型
                let mut stream = Pin::from(stream);
                let mut accumulated_content = String::new();

                // Pin<Box<T>> 实现了 Unpin，所以可以直接调用 next()
                while let Some(result) = stream.as_mut().next().await {
                    match result {
                        Ok(chunk) => {
                            // 提取内容增量
                            if let Some(delta) = chunk.choices.first()
                                .and_then(|c| c.delta.content.as_ref())
                            {
                                accumulated_content.push_str(delta);
                                let _ = stream_sender.send(Ok(delta.to_string())).await;
                            }
                            
                            // 检查是否完成
                            if chunk.choices.first()
                                .and_then(|c| c.finish_reason.as_ref())
                                .is_some()
                            {
                                // 流式响应完成
                                // 注意：流式响应中工具调用的处理较复杂，这里先不处理
                                // 如果需要工具调用，可能需要回退到非流式模式
                                break;
                            }
                        }
                        Err(e) => {
                            let _ = stream_sender.send(Err(e.to_string())).await;
                            return;
                        }
                    }
                }
            });
        } else {
            // 非流式模式，使用原有逻辑
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
                
                // 创建 MCP 服务器实例
                let mcp_server = match PgoneMcpServer::new(dbconfig_id_clone.clone()).await {
                    Ok(server) => server,
                    Err(e) => {
                        let _ = sender.send(Err(format!("创建 MCP 服务器失败: {}", e))).await;
                        return;
                    }
                };
                
                // 处理多轮对话，直到没有 tool_calls
                let mut current_messages = chat_messages;
                let mut max_iterations = 10; // 防止无限循环
                
                loop {
                    if max_iterations == 0 {
                        let _ = sender.send(Err("达到最大工具调用轮次限制".to_string())).await;
                        return;
                    }
                    max_iterations -= 1;
                    
                    let mut request = pgone_llm::chat::ChatRequest::new(model_clone.clone())
                        .with_messages(current_messages.clone())
                        .with_tools(tools.iter().map(|t| t.clone().into()).collect());
                    
                    // 设置会话ID用于审计
                    if let Some(ref sid) = session_id {
                        request = request.with_session_id(sid.clone());
                    }
                    
                    let resp = match client.chat_create(request).await {
                        Ok(resp) => resp,
                        Err(e) => {
                            let _ = sender.send(Err(e.to_string())).await;
                            return;
                        }
                    };
                    
                    // 检查是否有 tool_calls
                    if let Some(tool_calls) = &resp.tool_calls {
                        if !tool_calls.is_empty() {
                            // 需要调用工具
                            let mut function_messages = Vec::new();
                            
                            for tool_call in tool_calls {
                                // 解析参数
                                let args: Value = match serde_json::from_str(&tool_call.arguments) {
                                    Ok(v) => v,
                                    Err(e) => {
                                        let error_msg = format!("解析工具参数失败: {}", e);
                                        function_messages.push(pgone_llm::chat::ChatMessage::function(
                                            tool_call.name.clone(),
                                            error_msg,
                                        ));
                                        continue;
                                    }
                                };
                                
                                // 调用工具
                                match mcp_server.call_tool_direct(&tool_call.name, args).await {
                                    Ok(result) => {
                                        // 将结果转换为字符串
                                        let result_str = match serde_json::to_string(&result) {
                                            Ok(s) => s,
                                            Err(e) => format!("序列化工具结果失败: {}", e),
                                        };
                                        function_messages.push(pgone_llm::chat::ChatMessage::function(
                                            tool_call.name.clone(),
                                            result_str,
                                        ));
                                    }
                                    Err(e) => {
                                        let error_msg = format!("工具调用失败: {}", e);
                                        function_messages.push(pgone_llm::chat::ChatMessage::function(
                                            tool_call.name.clone(),
                                            error_msg,
                                        ));
                                    }
                                }
                            }
                            
                            // 将 assistant 消息（包含 tool_calls）和 function messages 添加到消息历史
                            // 即使 content 为空，也需要添加 assistant 消息以保持对话连续性
                            current_messages.push(pgone_llm::chat::ChatMessage::assistant(resp.content.clone()));
                            current_messages.extend(function_messages);
                            
                            // 继续下一轮
                            continue;
                        }
                    }
                    
                    // 没有 tool_calls，返回最终响应
                    let final_response = resp.content;
                    let _ = sender.send(Ok(final_response)).await;
                    return;
                }
            });
        }
    }
}
