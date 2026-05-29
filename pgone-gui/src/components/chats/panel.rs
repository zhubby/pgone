use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::components::ChatCtx;
use crate::futures;
use crate::models::{Message, MessageContent, Role};
use chrono::Utc;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use egui_file_dialog::FileDialog;
use pgone_agent::{
    AgentContext, AgentEvent, AgentMessage, AgentRole, AgentStreamEvent, AgentTurnRequest,
    OpenAiCompatibleProvider, PgOneAgentService, ProviderConfig, StorageBackedAgentToolServices,
};
use serde::Deserialize;
use tokio::sync::mpsc;

use super::model_loader::ModelLoader;

const AGENT_COMPOSER_OUTER_HEIGHT: f32 = 118.0;
const AGENT_SECTION_SPACING: f32 = 8.0;
const AGENT_PANEL_INNER_MARGIN: i8 = 8;
const USER_BUBBLE_MAX_WIDTH_FRACTION: f32 = 0.82;
const AGENT_BUBBLE_MIN_CONTENT_WIDTH: f32 = 120.0;
const AGENT_BUBBLE_HORIZONTAL_MARGIN: i8 = 10;
const DEFAULT_OPENAI_CHAT_COMPLETIONS_URL: &str = "https://api.openai.com/v1/chat/completions";

fn image_file_dialog() -> FileDialog {
    FileDialog::new()
        .add_file_filter_extensions("Images", vec!["png", "jpg", "jpeg", "gif", "webp", "bmp"])
        .default_file_filter("Images")
        .id("agent_image_file_dialog")
}

struct AgentTurnResult {
    request_id: u64,
    event: AgentStreamEvent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentSqlPreviewRequest {
    pub title: String,
    pub sql: String,
    pub database: String,
}

#[derive(Deserialize)]
struct PreviewSqlToolResult {
    title: Option<String>,
    sql: String,
    database_name: Option<String>,
}

pub struct ChatPanel {
    input: String,
    pending_resources: Vec<PathBuf>,
    markdown_cache: CommonMarkCache,
    events: Vec<AgentEvent>,
    partial_response: String,
    response_receiver: Option<mpsc::Receiver<AgentTurnResult>>,
    model_loader: ModelLoader,
    image_file_dialog: FileDialog,
    file_dialog: FileDialog,
    in_flight: Option<u64>,
    error: Option<String>,
    next_request_id: u64,
    show_delete_confirm: bool,
    pending_agent_tab_requests: Vec<String>,
    pending_sql_preview_requests: Vec<AgentSqlPreviewRequest>,
    handled_sql_preview_results: Vec<String>,
}

impl Default for ChatPanel {
    fn default() -> Self {
        Self {
            input: String::new(),
            pending_resources: Vec::new(),
            markdown_cache: CommonMarkCache::default(),
            events: Vec::new(),
            partial_response: String::new(),
            response_receiver: None,
            model_loader: ModelLoader::default(),
            image_file_dialog: image_file_dialog(),
            file_dialog: FileDialog::new().id("agent_file_dialog"),
            in_flight: None,
            error: None,
            next_request_id: 1,
            show_delete_confirm: false,
            pending_agent_tab_requests: Vec::new(),
            pending_sql_preview_requests: Vec::new(),
            handled_sql_preview_results: Vec::new(),
        }
    }
}

impl Clone for ChatPanel {
    fn clone(&self) -> Self {
        Self {
            input: self.input.clone(),
            pending_resources: self.pending_resources.clone(),
            markdown_cache: CommonMarkCache::default(),
            events: self.events.clone(),
            partial_response: self.partial_response.clone(),
            response_receiver: None,
            model_loader: ModelLoader {
                available_models: self.model_loader.available_models.clone(),
                models_receiver: None,
                models_loaded: self.model_loader.models_loaded,
            },
            image_file_dialog: image_file_dialog(),
            file_dialog: FileDialog::new().id("agent_file_dialog"),
            in_flight: None,
            error: self.error.clone(),
            next_request_id: self.next_request_id,
            show_delete_confirm: false,
            pending_agent_tab_requests: Vec::new(),
            pending_sql_preview_requests: Vec::new(),
            handled_sql_preview_results: Vec::new(),
        }
    }
}

impl ChatPanel {
    pub fn take_pending_agent_tab_requests(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_agent_tab_requests)
    }

    pub fn take_pending_sql_preview_requests(&mut self) -> Vec<AgentSqlPreviewRequest> {
        std::mem::take(&mut self.pending_sql_preview_requests)
    }

    pub fn ui(&mut self, ctxs: &mut ChatCtx, ui: &mut egui::Ui) {
        self.model_loader.check_and_load(ctxs);
        self.process_agent_response(ctxs);

        let message_height = (ui.available_height() - AGENT_COMPOSER_OUTER_HEIGHT).max(120.0);
        self.show_agent_messages(ctxs, ui, message_height);

        if self.show_agent_composer(ctxs, ui) {
            self.send_agent_turn(ctxs);
        }

        self.show_delete_confirmation(ctxs, ui);
    }

    fn process_agent_response(&mut self, ctxs: &mut ChatCtx) {
        loop {
            let result = {
                let Some(receiver) = self.response_receiver.as_mut() else {
                    return;
                };
                receiver.try_recv()
            };

            match result {
                Ok(result) => {
                    if self.in_flight != Some(result.request_id) {
                        continue;
                    }

                    match result.event {
                        AgentStreamEvent::AssistantDelta { content } => {
                            self.partial_response.push_str(&content);
                            ctxs.should_scroll_to_bottom = true;
                        }
                        AgentStreamEvent::Event { event } => {
                            self.handle_agent_event(&event);
                            self.events.push(event);
                            ctxs.should_scroll_to_bottom = true;
                        }
                        AgentStreamEvent::Completed { response } => {
                            for event in &response.events {
                                self.handle_agent_event(event);
                            }
                            let fallback_content = std::mem::take(&mut self.partial_response);
                            let content = if response.message.content.trim().is_empty()
                                && !fallback_content.trim().is_empty()
                            {
                                fallback_content
                            } else {
                                response.message.content
                            };
                            self.in_flight = None;
                            self.response_receiver = None;
                            self.events = response.events;
                            self.error = None;
                            self.partial_response.clear();
                            if content.trim().is_empty() {
                                let error =
                                    "Model returned empty response, assistant message not saved"
                                        .to_owned();
                                self.error = Some(error.clone());
                                crate::notify::warning(&error);
                                tracing::warn!("{error}");
                                return;
                            }
                            if let Some(sess) =
                                ctxs.state.sessions.get_mut(ctxs.state.current_index)
                            {
                                sess.messages.push(Message {
                                    role: Role::Assistant,
                                    timestamp: Utc::now(),
                                    content: MessageContent::Markdown(content),
                                });
                                sess.updated_at = Utc::now();
                                if let Err(error) = ctxs.storage.save_session(sess) {
                                    tracing::error!("Failed to save Agent response: {error}");
                                }
                                ctxs.should_scroll_to_bottom = true;
                            }
                            return;
                        }
                        AgentStreamEvent::Failed { error } => {
                            self.in_flight = None;
                            self.response_receiver = None;
                            self.partial_response.clear();
                            self.error = Some(error.clone());
                            crate::notify::error(&error);
                            tracing::error!("Agent turn failed: {error}");
                            return;
                        }
                    }
                }
                Err(mpsc::error::TryRecvError::Empty) => return,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    self.in_flight = None;
                    self.response_receiver = None;
                    self.partial_response.clear();
                    let error = "Agent request channel disconnected".to_owned();
                    self.error = Some(error.clone());
                    tracing::error!("{error}");
                    return;
                }
            }
        }
    }

    fn handle_agent_event(&mut self, event: &AgentEvent) {
        let AgentEvent::ToolFinished { name, result } = event else {
            return;
        };
        if name != "preview_sql" {
            return;
        }
        if self
            .handled_sql_preview_results
            .iter()
            .any(|handled| handled == result)
        {
            return;
        }
        self.handled_sql_preview_results.push(result.clone());

        match preview_request_from_tool_result(result) {
            Some(request) => self.pending_sql_preview_requests.push(request),
            None => tracing::warn!("Ignoring malformed preview_sql result"),
        }
    }

    fn show_agent_messages(&mut self, ctxs: &mut ChatCtx, ui: &mut egui::Ui, message_height: f32) {
        let should_scroll = ctxs.should_scroll_to_bottom;
        if should_scroll {
            ctxs.should_scroll_to_bottom = false;
        }

        let content_width = frame_content_width(ui.available_width(), AGENT_PANEL_INNER_MARGIN);
        egui::Frame::new()
            .fill(ui.visuals().panel_fill)
            .stroke(egui::Stroke::new(
                1.0,
                ui.visuals().widgets.noninteractive.bg_stroke.color,
            ))
            .corner_radius(egui::CornerRadius::same(6))
            .inner_margin(egui::Margin::same(AGENT_PANEL_INNER_MARGIN))
            .show(ui, |ui| {
                ui.set_width(content_width);
                ui.set_max_width(content_width);
                egui::ScrollArea::vertical()
                    .id_salt("agent_messages")
                    .max_width(content_width)
                    .max_height(message_height)
                    .min_scrolled_height(message_height)
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        let viewport_width = ui.available_width();
                        ui.set_width(viewport_width);
                        ui.set_max_width(viewport_width);

                        let messages: Vec<Message> = ctxs
                            .state
                            .sessions
                            .get(ctxs.state.current_index)
                            .map(|session| session.messages.clone())
                            .unwrap_or_default();

                        if messages.is_empty() {
                            show_agent_empty_state(ui);
                        }

                        let final_message_index = current_turn_final_message_index(
                            &messages,
                            self.in_flight.is_some(),
                            &self.events,
                        );
                        for (message_index, message) in messages.iter().enumerate() {
                            if final_message_index == Some(message_index) {
                                show_agent_tool_activity(ui, &self.events, true);
                                ui.add_space(8.0);
                            }
                            show_agent_message(
                                ctxs,
                                ui,
                                message,
                                viewport_width,
                                &mut self.markdown_cache,
                                egui::Id::new("agent_message_markdown")
                                    .with(ctxs.state.current_index)
                                    .with(message_index),
                            );
                            ui.add_space(8.0);
                        }

                        if final_message_index.is_none() {
                            show_agent_tool_activity(ui, &self.events, false);
                            ui.add_space(8.0);
                        }

                        if let Some(error) = &self.error {
                            show_agent_error(ui, error);
                        }

                        if !self.partial_response.is_empty() {
                            let message = Message {
                                role: Role::Assistant,
                                timestamp: Utc::now(),
                                content: MessageContent::Markdown(self.partial_response.clone()),
                            };
                            show_assistant_message(
                                ui,
                                &message,
                                viewport_width,
                                &mut self.markdown_cache,
                                egui::Id::new("agent_partial_markdown")
                                    .with(ctxs.state.current_index)
                                    .with(self.in_flight),
                            );
                            ui.add_space(8.0);
                        }

                        if self.in_flight.is_some() {
                            show_agent_thinking(ui);
                        }

                        if should_scroll {
                            ui.allocate_space(egui::Vec2::ZERO);
                            ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                        }
                    });
            });
    }

    fn show_agent_composer(&mut self, ctxs: &mut ChatCtx, ui: &mut egui::Ui) -> bool {
        let can_send = self.in_flight.is_none() && !self.input.trim().is_empty();
        let mut send_clicked = false;
        let content_width = frame_content_width(ui.available_width(), AGENT_PANEL_INNER_MARGIN);

        egui::Frame::new()
            .fill(ui.visuals().extreme_bg_color)
            .stroke(egui::Stroke::new(
                1.0,
                ui.visuals().widgets.noninteractive.bg_stroke.color,
            ))
            .corner_radius(egui::CornerRadius::same(6))
            .inner_margin(egui::Margin::symmetric(AGENT_PANEL_INNER_MARGIN, 8))
            .show(ui, |ui| {
                ui.set_width(content_width);
                ui.set_max_width(content_width);
                self.show_pending_resources(ctxs, ui);
                let input_response = ui.add(
                    egui::TextEdit::multiline(&mut self.input)
                        .desired_rows(3)
                        .desired_width(content_width)
                        .return_key(egui::KeyboardShortcut::new(
                            egui::Modifiers::SHIFT,
                            egui::Key::Enter,
                        ))
                        .hint_text("Ask PgOne Agent about this database..."),
                );
                let enter_pressed = input_response.has_focus()
                    && ui.input(|input| {
                        input.key_pressed(egui::Key::Enter) && !input.modifiers.shift
                    });

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    show_agent_conversation_menu(ui, self, ctxs);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.in_flight.is_some()
                            && ui
                                .button(egui_phosphor::regular::STOP)
                                .on_hover_text("Stop")
                                .clicked()
                        {
                            self.in_flight = None;
                            self.response_receiver = None;
                            self.partial_response.clear();
                            self.events.push(AgentEvent::Completed {
                                status: pgone_agent::AgentTurnStatus::Partial,
                                summary: "Agent turn stopped.".to_owned(),
                            });
                        }

                        let send_text = egui::RichText::new(
                            egui_phosphor::regular::PAPER_PLANE_TILT,
                        )
                        .color(if can_send {
                            ui.visuals().hyperlink_color
                        } else {
                            ui.visuals().weak_text_color()
                        });
                        send_clicked = ui
                            .add_enabled(can_send, egui::Button::new(send_text))
                            .on_hover_text("Send")
                            .clicked();

                        self.show_resource_buttons(ui);
                    });
                });

                if can_send && enter_pressed {
                    send_clicked = true;
                    ui.ctx().input_mut(|input| {
                        input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                    });
                    if self.input.ends_with('\n') {
                        self.input.pop();
                    }
                }
            });

        send_clicked
    }

    fn show_resource_buttons(&mut self, ui: &mut egui::Ui) {
        if ui
            .button(egui_phosphor::regular::IMAGE)
            .on_hover_text("Attach image")
            .clicked()
        {
            self.image_file_dialog.pick_file();
        }

        if ui
            .button(egui_phosphor::regular::FILE)
            .on_hover_text("Attach file")
            .clicked()
        {
            self.file_dialog.pick_file();
        }

        if let Some(path) = self.image_file_dialog.update(ui.ctx()).picked() {
            self.pending_resources.push(path.to_path_buf());
        }

        if let Some(path) = self.file_dialog.update(ui.ctx()).picked() {
            self.pending_resources.push(path.to_path_buf());
        }
    }

    fn show_pending_resources(&mut self, ctxs: &mut ChatCtx, ui: &mut egui::Ui) {
        if self.pending_resources.is_empty() {
            return;
        }

        ui.horizontal_wrapped(|ui| {
            let mut remove = Vec::new();
            for (index, path) in self.pending_resources.iter().enumerate() {
                egui::Frame::new()
                    .fill(ui.visuals().widgets.inactive.bg_fill)
                    .stroke(ui.visuals().widgets.inactive.bg_stroke)
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(6, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if is_image_path(path) {
                                if let Some(handle) = ctxs.preview.ensure_texture(ui.ctx(), path) {
                                    let image = egui::widgets::Image::new(&handle)
                                        .fit_to_exact_size(egui::vec2(24.0, 24.0));
                                    ui.add(image);
                                } else {
                                    ui.label(egui_phosphor::regular::IMAGE);
                                }
                            } else {
                                ui.label(egui_phosphor::regular::FILE);
                            }
                            ui.label(
                                egui::RichText::new(file_name(path))
                                    .small()
                                    .color(ui.visuals().text_color()),
                            );
                            if ui
                                .small_button(egui_phosphor::regular::X)
                                .on_hover_text("Remove")
                                .clicked()
                            {
                                remove.push(index);
                            }
                        });
                    });
            }
            for index in remove.into_iter().rev() {
                self.pending_resources.remove(index);
            }
        });
        ui.add_space(4.0);
    }

    fn send_agent_turn(&mut self, ctxs: &mut ChatCtx) {
        let prompt = self.input.trim().to_owned();
        if prompt.is_empty() {
            return;
        }

        let Some(api_key) = ctxs.openai_api_key.clone() else {
            let error = "Please configure API Key first".to_owned();
            self.error = Some(error.clone());
            crate::notify::error(&error);
            return;
        };

        let Some(dbconfig_id) = ctxs.active_db_config_id.clone() else {
            let error = "Please select a database configuration first".to_owned();
            self.error = Some(error.clone());
            crate::notify::error(&error);
            return;
        };

        let Some(session) = ctxs.state.sessions.get(ctxs.state.current_index) else {
            return;
        };

        let session_id = session.id.clone();
        let history = session
            .messages
            .iter()
            .filter_map(message_to_agent_history)
            .collect::<Vec<_>>();
        let attachment_context = build_attachment_context(&self.pending_resources);
        let agent_message = combine_prompt_and_attachments(&prompt, &attachment_context);

        self.send_resources(ctxs);

        if let Some(session) = ctxs.state.sessions.get_mut(ctxs.state.current_index) {
            session.config_id = Some(dbconfig_id.clone());
            session.messages.push(Message {
                role: Role::User,
                timestamp: Utc::now(),
                content: MessageContent::Markdown(prompt.clone()),
            });
            session.updated_at = Utc::now();
            if let Err(error) = ctxs.storage.save_session(session) {
                tracing::error!("Failed to save user message: {error}");
            }
            ctxs.should_scroll_to_bottom = true;
        }

        self.input.clear();
        self.error = None;
        self.events.clear();
        self.handled_sql_preview_results.clear();
        self.partial_response.clear();
        let request_id = self.next_request_id;
        self.next_request_id += 1;
        self.in_flight = Some(request_id);

        let base_url =
            normalize_chat_completions_url(ctxs.state.settings.openai_base_url.as_deref());
        let model = ctxs.openai_model.clone();
        let stream = ctxs.state.settings.enable_stream_api;
        let proxy_enabled = ctxs.state.settings.proxy_enabled;
        let proxy_host = ctxs.state.settings.proxy_host.clone();
        let proxy_port = ctxs.state.settings.proxy_port;
        let selected_database = ctxs.selected_database.clone();

        let request = AgentTurnRequest {
            session_id,
            message: agent_message,
            context: AgentContext {
                dbconfig_id: Some(dbconfig_id),
                database_name: selected_database,
                selected_schema: ctxs.selected_schema.clone(),
                selected_table: ctxs.selected_table.clone(),
            },
            history,
        };

        let (sender, receiver) = mpsc::channel(1);
        self.response_receiver = Some(receiver);

        futures::spawn(async move {
            let mut provider_config = match ProviderConfig::new(base_url, api_key, model, stream) {
                Ok(config) => config,
                Err(error) => {
                    let _ = sender
                        .send(AgentTurnResult {
                            request_id,
                            event: AgentStreamEvent::Failed {
                                error: error.to_string(),
                            },
                        })
                        .await;
                    return;
                }
            };
            provider_config.proxy_enabled = proxy_enabled;
            provider_config.proxy_host = proxy_host;
            provider_config.proxy_port = proxy_port;

            let provider = match OpenAiCompatibleProvider::new(provider_config) {
                Ok(provider) => provider,
                Err(error) => {
                    let _ = sender
                        .send(AgentTurnResult {
                            request_id,
                            event: AgentStreamEvent::Failed {
                                error: error.to_string(),
                            },
                        })
                        .await;
                    return;
                }
            };

            let service = PgOneAgentService::new(
                Arc::new(provider),
                Arc::new(StorageBackedAgentToolServices::new()),
            );
            let (event_sender, mut event_receiver) = mpsc::channel(32);
            let ui_sender = sender.clone();
            futures::spawn(async move {
                while let Some(event) = event_receiver.recv().await {
                    let _ = ui_sender.send(AgentTurnResult { request_id, event }).await;
                }
            });
            let _ = service.run_agent_turn_stream(request, event_sender).await;
        });
    }

    fn send_resources(&mut self, ctxs: &mut ChatCtx) {
        let resources = self.pending_resources.drain(..).collect::<Vec<_>>();
        for path in resources {
            if is_image_path(&path) {
                self.add_image_message(ctxs, path);
            } else if let Some(session) = ctxs.state.sessions.get_mut(ctxs.state.current_index) {
                let label = if is_supported_text_attachment(&path) {
                    format!("[File sent to Agent] {}", path.display())
                } else {
                    format!("[File not sent to Agent] {}", path.display())
                };
                session.messages.push(Message {
                    role: Role::User,
                    timestamp: Utc::now(),
                    content: MessageContent::Markdown(label),
                });
                session.updated_at = Utc::now();
                if let Err(error) = ctxs.storage.save_session(session) {
                    tracing::error!("Failed to save file message: {error}");
                }
            }
        }
    }

    fn add_image_message(&mut self, ctxs: &mut ChatCtx, path: PathBuf) {
        let (width, height) = match image::open(&path) {
            Ok(image) => (image.width(), image.height()),
            Err(_) => (0, 0),
        };

        if let Some(session) = ctxs.state.sessions.get_mut(ctxs.state.current_index) {
            session.messages.push(Message {
                role: Role::User,
                timestamp: Utc::now(),
                content: MessageContent::Image {
                    path,
                    width,
                    height,
                },
            });
            session.messages.push(Message {
                role: Role::System,
                timestamp: Utc::now(),
                content: MessageContent::Markdown(
                    "Image attachment is shown locally and was not sent to the model.".to_owned(),
                ),
            });
            session.updated_at = Utc::now();

            if let Err(error) = ctxs.storage.save_session(session) {
                tracing::error!("Failed to save image message: {error}");
            }
        }
    }

    fn show_delete_confirmation(&mut self, ctxs: &mut ChatCtx, ui: &mut egui::Ui) {
        if !self.show_delete_confirm {
            return;
        }

        let mut open = true;
        let center = ui.ctx().content_rect().center();
        let current_session_title = ctxs
            .state
            .sessions
            .get(ctxs.state.current_index)
            .map(|session| session.title.clone())
            .unwrap_or_else(|| "Current session".to_owned());

        egui::Window::new("Confirm Delete Session")
            .id(egui::Id::new("confirm_delete_session_window"))
            .open(&mut open)
            .default_pos(center)
            .pivot(egui::Align2::CENTER_CENTER)
            .show(ui.ctx(), |ui| {
                ui.label(format!("Are you sure you want to delete session '{}'?", current_session_title));
                ui.label(
                    egui::RichText::new("This action cannot be undone. The session and all its messages will be permanently deleted.")
                        .color(ui.visuals().error_fg_color),
                );
                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.show_delete_confirm = false;
                    }
                    if ui
                        .button(egui::RichText::new("Confirm Delete").color(ui.visuals().error_fg_color))
                        .clicked()
                    {
                        self.delete_current_session(ctxs);
                        self.show_delete_confirm = false;
                    }
                });
            });

        if !open {
            self.show_delete_confirm = false;
        }
    }

    fn delete_current_session(&mut self, ctxs: &mut ChatCtx) {
        if ctxs.state.sessions.is_empty() {
            return;
        }

        let current_index = ctxs.state.current_index;
        if let Some(session) = ctxs.state.sessions.get(current_index) {
            let session_id = session.id.clone();
            if let Err(error) = ctxs.storage.delete_session(&session_id) {
                tracing::error!("Failed to delete session: {error}");
                crate::notify::error(&format!("Failed to delete session: {error}"));
                return;
            }

            ctxs.state.sessions.remove(current_index);
            self.events.clear();
            self.error = None;

            if ctxs.state.sessions.is_empty() {
                let new_id = ctxs.state.next_session_id.to_string();
                ctxs.state.next_session_id += 1;
                let new_session = crate::models::ChatSession::default_with_timestamp(new_id);
                ctxs.state.sessions.push(new_session.clone());
                ctxs.state.current_index = 0;
                self.pending_agent_tab_requests.push(new_session.id.clone());
                if let Err(error) = ctxs.storage.save_session(&new_session) {
                    tracing::error!("Failed to save new session: {error}");
                }
            } else if current_index >= ctxs.state.sessions.len() {
                ctxs.state.current_index = ctxs.state.sessions.len() - 1;
            }
            if let Some(session) = ctxs.state.sessions.get(ctxs.state.current_index) {
                self.pending_agent_tab_requests.push(session.id.clone());
            }

            crate::notify::info("Session deleted");
        }
    }
}

fn show_agent_conversation_menu(ui: &mut egui::Ui, panel: &mut ChatPanel, ctxs: &mut ChatCtx) {
    ui.menu_button(egui_phosphor::regular::CHATS, |ui| {
        if ui
            .button(format!("{} New chat", egui_phosphor::regular::PLUS))
            .clicked()
        {
            let session_id = create_new_session(ctxs);
            panel.pending_agent_tab_requests.push(session_id);
            panel.events.clear();
            panel.error = None;
            ui.close();
        }

        if ui
            .add_enabled(
                !ctxs.state.sessions.is_empty(),
                egui::Button::new(format!("{} Delete current", egui_phosphor::regular::TRASH)),
            )
            .clicked()
        {
            panel.show_delete_confirm = true;
            ui.close();
        }

        if !ctxs.state.sessions.is_empty() {
            ui.separator();
        }

        for (index, session) in ctxs.state.sessions.iter().enumerate() {
            let selected = index == ctxs.state.current_index;
            let label = if selected {
                format!("{} {}", egui_phosphor::regular::CHECK, session.title)
            } else {
                session.title.clone()
            };
            if ui.selectable_label(selected, label).clicked() {
                ctxs.state.current_index = index;
                panel.pending_agent_tab_requests.push(session.id.clone());
                panel.events.clear();
                panel.error = None;
                ui.close();
            }
        }
    })
    .response
    .on_hover_text("Agent conversations");
}

fn create_new_session(ctxs: &mut ChatCtx) -> String {
    let new_id = ctxs.state.next_session_id.to_string();
    ctxs.state.next_session_id += 1;
    let new_session = crate::models::ChatSession::default_with_timestamp(new_id.clone());
    ctxs.state.sessions.push(new_session.clone());
    ctxs.state.current_index = ctxs.state.sessions.len() - 1;

    if let Err(error) = ctxs.storage.save_session(&new_session) {
        tracing::error!("Failed to save new session: {error}");
    }

    new_id
}

fn show_agent_empty_state(ui: &mut egui::Ui) {
    ui.add_space(24.0);
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new(egui_phosphor::regular::SPARKLE)
                .size(22.0)
                .color(ui.visuals().hyperlink_color),
        );
        ui.label(egui::RichText::new("Ask PgOne about this database").strong());
        ui.label(
            egui::RichText::new("Inspect schemas, explain tables, or render relationships.")
                .small()
                .color(ui.visuals().weak_text_color()),
        );
    });
}

fn show_agent_message(
    ctxs: &mut ChatCtx,
    ui: &mut egui::Ui,
    message: &Message,
    viewport_width: f32,
    markdown_cache: &mut CommonMarkCache,
    markdown_id: egui::Id,
) {
    match message.role {
        Role::User => show_user_message(ctxs, ui, message, viewport_width),
        Role::Assistant => {
            show_assistant_message(ui, message, viewport_width, markdown_cache, markdown_id)
        }
        Role::System => show_system_message(ui, message, viewport_width),
    }
}

fn show_user_message(
    ctxs: &mut ChatCtx,
    ui: &mut egui::Ui,
    message: &Message,
    viewport_width: f32,
) {
    let width = user_bubble_width(viewport_width);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
        message_bubble(
            ctxs,
            ui,
            message,
            MessageBubbleStyle {
                label: "You",
                icon: egui_phosphor::regular::USER,
                max_width: width,
                fill: ui.visuals().selection.bg_fill,
                stroke: ui.visuals().selection.stroke,
                accent: ui.visuals().selection.stroke.color,
                markdown: false,
            },
        );
    });
}

fn show_assistant_message(
    ui: &mut egui::Ui,
    message: &Message,
    viewport_width: f32,
    markdown_cache: &mut CommonMarkCache,
    markdown_id: egui::Id,
) {
    markdown_message_bubble(
        ui,
        message,
        markdown_cache,
        markdown_id,
        MessageBubbleStyle {
            label: "PgOne",
            icon: egui_phosphor::regular::SPARKLE,
            max_width: assistant_bubble_width(viewport_width),
            fill: ui.visuals().extreme_bg_color,
            stroke: egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
            accent: ui.visuals().hyperlink_color,
            markdown: true,
        },
    );
}

fn show_system_message(ui: &mut egui::Ui, message: &Message, viewport_width: f32) {
    plain_message_bubble(
        ui,
        message,
        MessageBubbleStyle {
            label: "System",
            icon: egui_phosphor::regular::USER_GEAR,
            max_width: assistant_bubble_width(viewport_width),
            fill: ui.visuals().widgets.inactive.bg_fill,
            stroke: ui.visuals().widgets.inactive.bg_stroke,
            accent: ui.visuals().weak_text_color(),
            markdown: false,
        },
    );
}

struct MessageBubbleStyle<'a> {
    label: &'a str,
    icon: &'a str,
    max_width: f32,
    fill: egui::Color32,
    stroke: egui::Stroke,
    accent: egui::Color32,
    markdown: bool,
}

fn message_bubble(
    ctxs: &mut ChatCtx,
    ui: &mut egui::Ui,
    message: &Message,
    style: MessageBubbleStyle,
) {
    egui::Frame::new()
        .fill(style.fill)
        .stroke(style.stroke)
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(AGENT_BUBBLE_HORIZONTAL_MARGIN, 8))
        .show(ui, |ui| {
            constrained_bubble_ui(ui, style.max_width, |ui| {
                bubble_header(ui, &style);
                show_message_content(ctxs, ui, message, style.max_width);
            });
        });
}

fn plain_message_bubble(ui: &mut egui::Ui, message: &Message, style: MessageBubbleStyle) {
    egui::Frame::new()
        .fill(style.fill)
        .stroke(style.stroke)
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(AGENT_BUBBLE_HORIZONTAL_MARGIN, 8))
        .show(ui, |ui| {
            constrained_bubble_ui(ui, style.max_width, |ui| {
                bubble_header(ui, &style);
                if let MessageContent::Markdown(text) = &message.content {
                    ui.add(egui::Label::new(text.as_str()).wrap());
                }
            });
        });
}

fn markdown_message_bubble(
    ui: &mut egui::Ui,
    message: &Message,
    markdown_cache: &mut CommonMarkCache,
    markdown_id: egui::Id,
    style: MessageBubbleStyle,
) {
    egui::Frame::new()
        .fill(style.fill)
        .stroke(style.stroke)
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(AGENT_BUBBLE_HORIZONTAL_MARGIN, 8))
        .show(ui, |ui| {
            constrained_bubble_ui(ui, style.max_width, |ui| {
                bubble_header(ui, &style);
                if let MessageContent::Markdown(text) = &message.content {
                    ui.push_id(markdown_id, |ui| {
                        egui::ScrollArea::horizontal()
                            .id_salt("assistant_markdown_horizontal_scroll")
                            .auto_shrink([false, true])
                            .max_width(style.max_width)
                            .show(ui, |ui| {
                                CommonMarkViewer::new()
                                    .default_width(Some(style.max_width as usize))
                                    .show(ui, markdown_cache, text);
                            });
                    });
                }
            });
        });
}

fn constrained_bubble_ui(ui: &mut egui::Ui, width: f32, add_contents: impl FnOnce(&mut egui::Ui)) {
    ui.allocate_ui_with_layout(
        egui::vec2(width, 0.0),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.set_min_width(width);
            ui.set_max_width(width);
            add_contents(ui);
        },
    );
}

fn frame_content_width(available_width: f32, inner_margin: i8) -> f32 {
    (available_width - f32::from(inner_margin) * 2.0).max(0.0)
}

fn bubble_header(ui: &mut egui::Ui, style: &MessageBubbleStyle) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(style.icon).small().color(style.accent));
        ui.label(
            egui::RichText::new(style.label)
                .small()
                .strong()
                .color(style.accent),
        );
    });
}

fn show_message_content(ctxs: &mut ChatCtx, ui: &mut egui::Ui, message: &Message, max_width: f32) {
    match &message.content {
        MessageContent::Markdown(text) => {
            ui.add(egui::Label::new(text.as_str()).wrap());
        }
        MessageContent::Image {
            path,
            width,
            height,
        } => {
            if let Some(handle) = ctxs.preview.ensure_texture(ui.ctx(), path) {
                let original_size = egui::vec2(*width as f32, *height as f32);
                let bounded = if original_size.x > 0.0 && original_size.y > 0.0 {
                    original_size.min(egui::vec2(max_width, 360.0))
                } else {
                    egui::vec2(max_width.min(360.0), 240.0)
                };
                let response =
                    ui.add(egui::widgets::Image::new(&handle).fit_to_exact_size(bounded));
                if response.clicked() {
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
}

fn user_bubble_width(available_width: f32) -> f32 {
    bubble_content_width(available_width, USER_BUBBLE_MAX_WIDTH_FRACTION)
}

fn assistant_bubble_width(available_width: f32) -> f32 {
    bubble_content_width(available_width, 1.0)
}

fn bubble_content_width(available_width: f32, width_fraction: f32) -> f32 {
    let horizontal_margin = f32::from(AGENT_BUBBLE_HORIZONTAL_MARGIN) * 2.0;
    let available_content_width = (available_width - horizontal_margin).max(0.0);
    let preferred_width = available_width * width_fraction - horizontal_margin;

    preferred_width
        .max(AGENT_BUBBLE_MIN_CONTENT_WIDTH.min(available_content_width))
        .min(available_content_width)
}

fn current_turn_final_message_index(
    messages: &[Message],
    request_in_flight: bool,
    events: &[AgentEvent],
) -> Option<usize> {
    if request_in_flight || events.is_empty() {
        return None;
    }

    messages
        .iter()
        .rposition(|message| matches!(message.role, Role::Assistant))
}

fn show_agent_tool_activity(
    ui: &mut egui::Ui,
    events: &[AgentEvent],
    hide_completion_summary: bool,
) {
    let visible_events = events
        .iter()
        .filter(|event| !(hide_completion_summary && matches!(event, AgentEvent::Completed { .. })))
        .collect::<Vec<_>>();
    if visible_events.is_empty() {
        return;
    }

    egui::CollapsingHeader::new(format!("{} Tool activity", egui_phosphor::regular::WRENCH))
        .default_open(false)
        .show(ui, |ui| {
            for event in visible_events {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(agent_event_icon(event))
                            .color(agent_event_color(ui, event)),
                    );
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(format_agent_event(event))
                                .small()
                                .color(ui.visuals().weak_text_color()),
                        )
                        .wrap(),
                    );
                });
            }
        });
}

fn show_agent_error(ui: &mut egui::Ui, error: &str) {
    status_frame(ui, ui.visuals().error_fg_color, |ui| {
        ui.label(
            egui::RichText::new(egui_phosphor::regular::WARNING_CIRCLE)
                .color(ui.visuals().error_fg_color),
        );
        ui.label(egui::RichText::new(error).color(ui.visuals().error_fg_color));
    });
}

fn show_agent_thinking(ui: &mut egui::Ui) {
    status_frame(ui, ui.visuals().hyperlink_color, |ui| {
        ui.add(egui::Spinner::new().size(14.0));
        ui.label(
            egui::RichText::new("PgOne is thinking...")
                .small()
                .color(ui.visuals().weak_text_color()),
        );
    });
}

fn status_frame<R>(
    ui: &mut egui::Ui,
    color: egui::Color32,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) {
    egui::Frame::new()
        .fill(ui.visuals().widgets.inactive.bg_fill)
        .stroke(egui::Stroke::new(1.0, color))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.horizontal(add_contents);
        });
}

fn agent_event_icon(event: &AgentEvent) -> &'static str {
    match event {
        AgentEvent::ToolStarted { .. } => egui_phosphor::regular::CIRCLE_NOTCH,
        AgentEvent::ToolFinished { .. } => egui_phosphor::regular::CHECK_CIRCLE,
        AgentEvent::ToolFailed { .. } => egui_phosphor::regular::WARNING_CIRCLE,
        AgentEvent::Completed { .. } => egui_phosphor::regular::CHECK,
    }
}

fn agent_event_color(ui: &egui::Ui, event: &AgentEvent) -> egui::Color32 {
    match event {
        AgentEvent::ToolStarted { .. } => ui.visuals().hyperlink_color,
        AgentEvent::ToolFinished { .. } | AgentEvent::Completed { .. } => {
            ui.visuals().weak_text_color()
        }
        AgentEvent::ToolFailed { .. } => ui.visuals().error_fg_color,
    }
}

fn format_agent_event(event: &AgentEvent) -> String {
    match event {
        AgentEvent::ToolStarted { name, .. } => format!("Started {name}"),
        AgentEvent::ToolFinished { name, .. } => format!("Finished {name}"),
        AgentEvent::ToolFailed { name, error } => format!("{name} failed: {error}"),
        AgentEvent::Completed { status, summary } => format!("{status:?}: {summary}"),
    }
}

fn message_to_agent_history(message: &Message) -> Option<AgentMessage> {
    let MessageContent::Markdown(content) = &message.content else {
        return None;
    };

    match message.role {
        Role::User => Some(AgentMessage {
            role: AgentRole::User,
            content: content.clone(),
        }),
        Role::Assistant => Some(AgentMessage {
            role: AgentRole::Assistant,
            content: content.clone(),
        }),
        Role::System => Some(AgentMessage {
            role: AgentRole::System,
            content: content.clone(),
        }),
    }
}

fn normalize_chat_completions_url(base_url: Option<&str>) -> String {
    let Some(base_url) = base_url.map(str::trim).filter(|value| !value.is_empty()) else {
        return DEFAULT_OPENAI_CHAT_COMPLETIONS_URL.to_owned();
    };

    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/chat/completions") {
        trimmed.to_owned()
    } else if trimmed.ends_with("/v1") {
        format!("{trimmed}/chat/completions")
    } else {
        trimmed.to_owned()
    }
}

fn preview_request_from_tool_result(result: &str) -> Option<AgentSqlPreviewRequest> {
    let result: PreviewSqlToolResult = serde_json::from_str(result).ok()?;
    let sql = result.sql.trim().to_owned();
    if sql.is_empty() {
        return None;
    }

    let title = result
        .title
        .map(|title| title.trim().to_owned())
        .filter(|title| !title.is_empty())
        .map(|title| format!("Preview SQL: {title}"))
        .unwrap_or_else(|| "Preview SQL".to_owned());
    let database = result
        .database_name
        .map(|database| database.trim().to_owned())
        .filter(|database| !database.is_empty())
        .unwrap_or_default();

    Some(AgentSqlPreviewRequest {
        title,
        sql,
        database,
    })
}

const MAX_ATTACHMENT_BYTES: u64 = 64 * 1024;

fn build_attachment_context(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .filter_map(|path| attachment_context_entry(path))
        .collect()
}

fn attachment_context_entry(path: &Path) -> Option<String> {
    if is_image_path(path) {
        return Some(format!(
            "Image attachment not sent to the model: {}",
            path.display()
        ));
    }
    if !is_supported_text_attachment(path) {
        return Some(format!(
            "Unsupported attachment not sent to the model: {}",
            path.display()
        ));
    }

    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => {
            return Some(format!(
                "Attachment could not be read: {} ({error})",
                path.display()
            ));
        }
    };
    if metadata.len() > MAX_ATTACHMENT_BYTES {
        return Some(format!(
            "Attachment too large and not sent to the model: {} ({} bytes)",
            path.display(),
            metadata.len()
        ));
    }

    match std::fs::read_to_string(path) {
        Ok(content) => Some(format!(
            "Attachment: {}\n```text\n{}\n```",
            path.display(),
            content
        )),
        Err(error) => Some(format!(
            "Attachment could not be decoded as UTF-8 text: {} ({error})",
            path.display()
        )),
    }
}

fn combine_prompt_and_attachments(prompt: &str, attachments: &[String]) -> String {
    if attachments.is_empty() {
        return prompt.to_owned();
    }

    format!(
        "{prompt}\n\nAttached context:\n{}",
        attachments
            .iter()
            .map(|entry| format!("- {entry}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
            )
        })
        .unwrap_or(false)
}

fn is_supported_text_attachment(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_lowercase().as_str(),
                "sql" | "md" | "txt" | "json" | "yaml" | "yml" | "toml" | "csv" | "log"
            )
        })
        .unwrap_or(false)
}

fn file_name(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_chat_completions_url_defaults_to_openai_endpoint() {
        assert_eq!(
            normalize_chat_completions_url(None),
            DEFAULT_OPENAI_CHAT_COMPLETIONS_URL
        );
        assert_eq!(
            normalize_chat_completions_url(Some(" ")),
            DEFAULT_OPENAI_CHAT_COMPLETIONS_URL
        );
    }

    #[test]
    fn normalize_chat_completions_url_expands_v1_root() {
        assert_eq!(
            normalize_chat_completions_url(Some("https://api.openai.com/v1/")),
            DEFAULT_OPENAI_CHAT_COMPLETIONS_URL
        );
    }

    #[test]
    fn text_attachment_context_includes_supported_file_content() {
        let path =
            std::env::temp_dir().join(format!("pgone-agent-attachment-{}.sql", std::process::id()));
        std::fs::write(&path, "SELECT 1;").unwrap();

        let context = build_attachment_context(std::slice::from_ref(&path));

        assert_eq!(context.len(), 1);
        assert!(context[0].contains("SELECT 1;"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn image_attachment_context_marks_image_as_not_sent() {
        let path = PathBuf::from("/tmp/example.png");

        let context = build_attachment_context(&[path]);

        assert_eq!(context.len(), 1);
        assert!(context[0].contains("not sent to the model"));
    }

    #[test]
    fn combines_prompt_and_attachment_context() {
        let prompt =
            combine_prompt_and_attachments("Explain this", &["Attachment: query.sql".to_owned()]);

        assert!(prompt.contains("Explain this"));
        assert!(prompt.contains("Attached context"));
        assert!(prompt.contains("query.sql"));
    }

    #[test]
    fn frame_content_width_subtracts_horizontal_inner_margin() {
        assert_eq!(frame_content_width(320.0, 8), 304.0);
    }

    #[test]
    fn frame_content_width_does_not_go_negative() {
        assert_eq!(frame_content_width(12.0, 8), 0.0);
    }

    #[test]
    fn preview_sql_tool_result_becomes_preview_request() {
        let request = preview_request_from_tool_result(
            r#"{"title":"Create audit table","sql":" CREATE TABLE audit_log (id bigint); ","database_name":"app"}"#,
        )
        .unwrap();

        assert_eq!(request.title, "Preview SQL: Create audit table");
        assert_eq!(request.sql, "CREATE TABLE audit_log (id bigint);");
        assert_eq!(request.database, "app");
    }

    #[test]
    fn preview_sql_tool_result_defaults_title_and_database() {
        let request = preview_request_from_tool_result(r#"{"sql":"SELECT 1"}"#).unwrap();

        assert_eq!(request.title, "Preview SQL");
        assert_eq!(request.database, "");
    }

    #[test]
    fn preview_sql_tool_result_rejects_malformed_or_empty_sql() {
        assert!(preview_request_from_tool_result("not json").is_none());
        assert!(preview_request_from_tool_result(r#"{"sql":"  "}"#).is_none());
    }

    #[test]
    fn preview_sql_tool_finished_event_enqueues_once() {
        let mut panel = ChatPanel::default();
        let event = AgentEvent::ToolFinished {
            name: "preview_sql".to_owned(),
            result: r#"{"title":"Query","sql":"SELECT 1","database_name":"app"}"#.to_owned(),
        };

        panel.handle_agent_event(&event);
        panel.handle_agent_event(&event);

        let requests = panel.take_pending_sql_preview_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].title, "Preview SQL: Query");
    }

    #[test]
    fn malformed_preview_sql_event_is_ignored() {
        let mut panel = ChatPanel::default();
        panel.handle_agent_event(&AgentEvent::ToolFinished {
            name: "preview_sql".to_owned(),
            result: "not json".to_owned(),
        });

        assert!(panel.take_pending_sql_preview_requests().is_empty());
    }

    #[test]
    fn current_turn_final_message_is_last_assistant_after_tool_events_complete() {
        let messages = vec![
            Message {
                role: Role::User,
                timestamp: Utc::now(),
                content: MessageContent::Markdown("list tables".to_owned()),
            },
            Message {
                role: Role::Assistant,
                timestamp: Utc::now(),
                content: MessageContent::Markdown("there are 25 tables".to_owned()),
            },
        ];
        let events = vec![AgentEvent::ToolFinished {
            name: "introspect_database".to_owned(),
            result: "done".to_owned(),
        }];

        assert_eq!(
            current_turn_final_message_index(&messages, false, &events),
            Some(1)
        );
    }

    #[test]
    fn current_turn_final_message_is_not_selected_while_request_is_in_flight() {
        let messages = vec![Message {
            role: Role::Assistant,
            timestamp: Utc::now(),
            content: MessageContent::Markdown("previous answer".to_owned()),
        }];
        let events = vec![AgentEvent::ToolStarted {
            name: "introspect_database".to_owned(),
            arguments: serde_json::json!({}),
        }];

        assert_eq!(
            current_turn_final_message_index(&messages, true, &events),
            None
        );
    }
}
