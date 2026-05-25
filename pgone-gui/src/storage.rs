use crate::futures;
use crate::models::{ChatSession, Message, MessageContent, Role};
use anyhow::Result;
use chrono::{DateTime, Utc};
use pgone_storage::models::{
    Message as StorageMessage, MessageKind as StorageMessageKind, Role as StorageRole,
    Session as StorageSession,
};
use pgone_storage::service::StorageService;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

enum SessionStorageCommand {
    SaveSession(ChatSession),
    DeleteSession(String),
    AddMessage {
        session_id: String,
        message: Message,
    },
}

#[derive(Default)]
struct SessionStorageState {
    loaded_sessions: Option<Vec<ChatSession>>,
    last_error: Option<String>,
}

pub struct SessionStorage {
    state: Arc<Mutex<SessionStorageState>>,
    commands: mpsc::UnboundedSender<SessionStorageCommand>,
}

impl SessionStorage {
    pub fn new(ctx: egui::Context) -> Self {
        let state = Arc::new(Mutex::new(SessionStorageState::default()));
        let (commands, mut receiver) = mpsc::unbounded_channel();
        let worker_state = Arc::clone(&state);

        futures::spawn(async move {
            let storage = match StorageService::open_default().await {
                Ok(storage) => storage,
                Err(error) => {
                    set_error(&worker_state, error.to_string());
                    ctx.request_repaint();
                    return;
                }
            };

            load_sessions_into_state(&storage, &worker_state).await;
            ctx.request_repaint();

            while let Some(command) = receiver.recv().await {
                match command {
                    SessionStorageCommand::SaveSession(session) => {
                        if let Err(error) = save_session_to_storage(&storage, &session).await {
                            set_error(&worker_state, error.to_string());
                        }
                    }
                    SessionStorageCommand::DeleteSession(session_id) => {
                        if let Err(error) = storage.delete_session(&session_id).await {
                            set_error(&worker_state, error.to_string());
                        }
                    }
                    SessionStorageCommand::AddMessage {
                        session_id,
                        message,
                    } => {
                        if let Err(error) =
                            add_message_to_storage(&storage, &session_id, &message).await
                        {
                            set_error(&worker_state, error.to_string());
                        }
                    }
                }
                ctx.request_repaint();
            }
        });

        Self { state, commands }
    }

    pub fn take_loaded_sessions(&mut self) -> Option<Vec<ChatSession>> {
        self.state
            .lock()
            .ok()
            .and_then(|mut state| state.loaded_sessions.take())
    }

    pub fn last_error(&self) -> Option<String> {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.last_error.clone())
    }

    pub fn save_sessions(&mut self, sessions: &[ChatSession]) -> Result<()> {
        for session in sessions {
            self.save_session(session)?;
        }
        Ok(())
    }

    pub fn save_session(&mut self, session: &ChatSession) -> Result<()> {
        let _ = self
            .commands
            .send(SessionStorageCommand::SaveSession(session.clone()));
        Ok(())
    }

    pub fn delete_session(&mut self, session_id: &str) -> Result<()> {
        let _ = self
            .commands
            .send(SessionStorageCommand::DeleteSession(session_id.to_string()));
        Ok(())
    }

    pub fn add_message(&mut self, session_id: &str, message: Message) -> Result<()> {
        let _ = self.commands.send(SessionStorageCommand::AddMessage {
            session_id: session_id.to_string(),
            message,
        });
        Ok(())
    }

    pub fn query_messages_by_session(&mut self, _session_id: &str) -> Result<Vec<Message>> {
        Ok(Vec::new())
    }
}

async fn load_sessions_into_state(
    storage: &StorageService,
    state: &Arc<Mutex<SessionStorageState>>,
) {
    match load_sessions_from_storage(storage).await {
        Ok(sessions) => {
            if let Ok(mut state) = state.lock() {
                state.loaded_sessions = Some(sessions);
                state.last_error = None;
            }
        }
        Err(error) => set_error(state, error.to_string()),
    }
}

async fn load_sessions_from_storage(storage: &StorageService) -> anyhow::Result<Vec<ChatSession>> {
    let storage_sessions = storage.list_sessions(1000).await?;

    let mut chat_sessions = Vec::new();
    for storage_session in storage_sessions {
        let messages = storage.list_messages(&storage_session.id, 10000).await?;
        let chat_messages = messages
            .into_iter()
            .map(storage_message_to_chat_message)
            .collect();

        chat_sessions.push(ChatSession {
            id: storage_session.id,
            title: storage_session.title,
            messages: chat_messages,
            created_at: timestamp_to_datetime(storage_session.created_at),
            updated_at: timestamp_to_datetime(storage_session.updated_at),
        });
    }

    chat_sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(chat_sessions)
}

async fn save_session_to_storage(
    storage: &StorageService,
    session: &ChatSession,
) -> anyhow::Result<()> {
    let storage_session = StorageSession {
        id: session.id.clone(),
        title: session.title.clone(),
        config_id: None,
        created_at: datetime_to_timestamp(session.created_at),
        updated_at: datetime_to_timestamp(session.updated_at),
    };

    let _ = storage.delete_session(&session.id).await;
    storage.create_session(&storage_session).await?;

    for message in &session.messages {
        append_message(storage, &session.id, message).await?;
    }

    Ok(())
}

async fn add_message_to_storage(
    storage: &StorageService,
    session_id: &str,
    message: &Message,
) -> anyhow::Result<()> {
    append_message(storage, session_id, message).await?;
    storage.update_session_title(session_id, "").await?;
    Ok(())
}

async fn append_message(
    storage: &StorageService,
    session_id: &str,
    message: &Message,
) -> anyhow::Result<()> {
    match &message.content {
        MessageContent::Markdown(text) => {
            storage
                .append_markdown(session_id, chat_role_to_storage_role(message.role), text)
                .await?;
        }
        MessageContent::Image {
            path,
            width,
            height,
        } => {
            storage
                .append_image(
                    session_id,
                    chat_role_to_storage_role(message.role),
                    &path.to_string_lossy(),
                    *width as i64,
                    *height as i64,
                )
                .await?;
        }
        MessageContent::Video {
            path, duration_ms, ..
        } => {
            storage
                .append_video(
                    session_id,
                    chat_role_to_storage_role(message.role),
                    &path.to_string_lossy(),
                    duration_ms.map(|duration| duration as i64),
                )
                .await?;
        }
    }

    Ok(())
}

fn set_error(state: &Arc<Mutex<SessionStorageState>>, error: String) {
    if let Ok(mut state) = state.lock() {
        state.last_error = Some(error);
    }
}

fn chat_role_to_storage_role(role: Role) -> StorageRole {
    match role {
        Role::User => StorageRole::User,
        Role::Assistant => StorageRole::Assistant,
        Role::System => StorageRole::System,
    }
}

fn storage_role_to_chat_role(role: StorageRole) -> Role {
    match role {
        StorageRole::User => Role::User,
        StorageRole::Assistant => Role::Assistant,
        StorageRole::System => Role::System,
    }
}

fn storage_message_to_chat_message(msg: StorageMessage) -> Message {
    let content = match msg.kind {
        StorageMessageKind::Markdown => {
            MessageContent::Markdown(msg.content_markdown.unwrap_or_default())
        }
        StorageMessageKind::Image => MessageContent::Image {
            path: msg.image_path.map(|path| path.into()).unwrap_or_default(),
            width: msg.image_w.map(|width| width as u32).unwrap_or(0),
            height: msg.image_h.map(|height| height as u32).unwrap_or(0),
        },
        StorageMessageKind::Video => MessageContent::Video {
            path: msg.video_path.map(|path| path.into()).unwrap_or_default(),
            duration_ms: msg.video_duration_ms.map(|duration| duration as u64),
            thumbnail: None,
        },
    };

    Message {
        role: storage_role_to_chat_role(msg.role),
        timestamp: timestamp_to_datetime(msg.timestamp),
        content,
    }
}

fn datetime_to_timestamp(dt: DateTime<Utc>) -> i64 {
    dt.timestamp()
}

fn timestamp_to_datetime(ts: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(ts, 0).unwrap_or_else(Utc::now)
}
