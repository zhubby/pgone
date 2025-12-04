use crate::models::{ChatSession, Message, MessageContent, Role};
use crate::futures;
use anyhow::Result;
use chrono::{DateTime, Utc};
use pgone_storage::blocking::StorageBlocking;
use pgone_storage::models::{Message as StorageMessage, Role as StorageRole, Session as StorageSession, MessageKind as StorageMessageKind};

pub struct SessionStorage {
    storage: Option<StorageBlocking>,
}

impl SessionStorage {
    pub fn new() -> Self {
        Self { storage: None }
    }

    fn ensure_storage(&mut self) -> Result<&mut StorageBlocking> {
        if self.storage.is_none() {
            let storage = futures::block_on_async(async {
                StorageBlocking::open_local("pgone.db").await
            })?;
            self.storage = Some(storage);
        }
        Ok(self.storage.as_mut().unwrap())
    }

    /// 加载所有会话
    pub fn load_sessions(&mut self) -> Result<Vec<ChatSession>> {
        let storage = self.ensure_storage()?;
        
        let storage_sessions = futures::block_on_async(async {
            storage.list_sessions(1000).await
        })?;

        let mut chat_sessions = Vec::new();
        for storage_session in storage_sessions {
            let messages = futures::block_on_async(async {
                storage.list_messages(&storage_session.id, 10000).await
            })?;

            let chat_messages: Vec<Message> = messages
                .into_iter()
                .map(|m| storage_message_to_chat_message(m))
                .collect();

            chat_sessions.push(ChatSession {
                id: storage_session.id,
                title: storage_session.title,
                messages: chat_messages,
                created_at: timestamp_to_datetime(storage_session.created_at),
                updated_at: timestamp_to_datetime(storage_session.updated_at),
            });
        }

        // 按更新时间降序排序
        chat_sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(chat_sessions)
    }

    /// 保存所有会话（批量保存）
    pub fn save_sessions(&mut self, sessions: &[ChatSession]) -> Result<()> {
        for session in sessions {
            self.save_session(session)?;
        }

        tracing::debug!("已保存 {} 个会话到数据库", sessions.len());
        Ok(())
    }

    /// 添加或更新单个会话
    pub fn save_session(&mut self, session: &ChatSession) -> Result<()> {
        let storage = self.ensure_storage()?;
        
        // 创建或更新会话
        let storage_session = StorageSession {
            id: session.id.clone(),
            title: session.title.clone(),
            config_id: None,
            created_at: datetime_to_timestamp(session.created_at),
            updated_at: datetime_to_timestamp(session.updated_at),
        };

        // 先删除会话（这会删除所有消息）
        let _ = futures::block_on_async(async {
            storage.delete_session(&session.id).await
        });

        // 重新创建会话
        futures::block_on_async(async {
            storage.create_session(&storage_session).await
        })?;

        // 插入所有消息
        for msg in &session.messages {
            match &msg.content {
                MessageContent::Markdown(text) => {
                    futures::block_on_async(async {
                        storage.append_markdown(&session.id, chat_role_to_storage_role(msg.role), text).await
                    })?;
                }
                MessageContent::Image { path, width, height } => {
                    futures::block_on_async(async {
                        storage.append_image(
                            &session.id,
                            chat_role_to_storage_role(msg.role),
                            &path.to_string_lossy(),
                            *width as i64,
                            *height as i64,
                        ).await
                    })?;
                }
                MessageContent::Video { path, duration_ms, .. } => {
                    futures::block_on_async(async {
                        storage.append_video(
                            &session.id,
                            chat_role_to_storage_role(msg.role),
                            &path.to_string_lossy(),
                            duration_ms.map(|d| d as i64),
                        ).await
                    })?;
                }
            }
        }

        Ok(())
    }

    /// 删除会话
    pub fn delete_session(&mut self, session_id: &str) -> Result<()> {
        let storage = self.ensure_storage()?;
        futures::block_on_async(async {
            storage.delete_session(session_id).await
        })?;
        Ok(())
    }

    /// 添加消息到会话
    pub fn add_message(&mut self, session_id: &str, message: Message) -> Result<()> {
        let storage = self.ensure_storage()?;

        match &message.content {
            MessageContent::Markdown(text) => {
                futures::block_on_async(async {
                    storage.append_markdown(session_id, chat_role_to_storage_role(message.role), text).await
                })?;
            }
            MessageContent::Image { path, width, height } => {
                futures::block_on_async(async {
                    storage.append_image(
                        session_id,
                        chat_role_to_storage_role(message.role),
                        &path.to_string_lossy(),
                        *width as i64,
                        *height as i64,
                    ).await
                })?;
            }
            MessageContent::Video { path, duration_ms, .. } => {
                futures::block_on_async(async {
                    storage.append_video(
                        session_id,
                        chat_role_to_storage_role(message.role),
                        &path.to_string_lossy(),
                        duration_ms.map(|d| d as i64),
                    ).await
                })?;
            }
        }

        // 更新会话的 updated_at（通过更新标题来触发 updated_at 更新）
        futures::block_on_async(async {
            storage.update_session_title(session_id, "").await
        })?;

        Ok(())
    }

    /// 查询会话的历史消息（最近10条，按时间降序）
    pub fn query_messages_by_session(&mut self, session_id: &str) -> Result<Vec<Message>> {
        let storage = self.ensure_storage()?;
        
        let storage_messages = futures::block_on_async(async {
            storage.query_messages_by_session(session_id).await
        })?;

        let chat_messages: Vec<Message> = storage_messages
            .into_iter()
            .map(|m| storage_message_to_chat_message(m))
            .collect();

        Ok(chat_messages)
    }
}

impl Default for SessionStorage {
    fn default() -> Self {
        Self::new()
    }
}

// 转换函数
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
        StorageMessageKind::Image => {
            MessageContent::Image {
                path: msg.image_path.map(|p| p.into()).unwrap_or_default(),
                width: msg.image_w.map(|w| w as u32).unwrap_or(0),
                height: msg.image_h.map(|h| h as u32).unwrap_or(0),
            }
        }
        StorageMessageKind::Video => {
            MessageContent::Video {
                path: msg.video_path.map(|p| p.into()).unwrap_or_default(),
                duration_ms: msg.video_duration_ms.map(|d| d as u64),
                thumbnail: None,
            }
        }
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
    DateTime::from_timestamp(ts, 0).unwrap_or_else(|| Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MessageContent, Role};
    use chrono::Utc;

    #[test]
    fn test_session_storage() {
        let mut storage = SessionStorage::new();

        // 创建测试会话
        let mut session = ChatSession::new("test-1".to_string(), "测试会话".to_string());
        session.messages.push(Message {
            role: Role::User,
            timestamp: Utc::now(),
            content: MessageContent::Markdown("测试消息".to_string()),
        });

        // 保存会话
        assert!(storage.save_session(&session).is_ok());

        // 加载会话
        let sessions = storage.load_sessions().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "test-1");
        assert_eq!(sessions[0].messages.len(), 1);

        // 添加消息
        let new_message = Message {
            role: Role::Assistant,
            timestamp: Utc::now(),
            content: MessageContent::Markdown("回复消息".to_string()),
        };
        assert!(storage.add_message("test-1", new_message).is_ok());

        // 验证消息已添加
        let sessions = storage.load_sessions().unwrap();
        assert_eq!(sessions[0].messages.len(), 2);

        // 删除会话
        assert!(storage.delete_session("test-1").is_ok());
        let sessions = storage.load_sessions().unwrap();
        assert_eq!(sessions.len(), 0);
    }
}

