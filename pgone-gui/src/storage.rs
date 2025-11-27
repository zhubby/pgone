use crate::models::{ChatSession, Message};
use anyhow::{Context, Result};
use chrono::Utc;
use std::fs;
use std::path::Path;

const SESSIONS_FILE: &str = "chat_sessions.json";

pub struct SessionStorage {
    file_path: String,
}

impl SessionStorage {
    pub fn new() -> Self {
        Self {
            file_path: SESSIONS_FILE.to_string(),
        }
    }

    pub fn with_path(path: impl Into<String>) -> Self {
        Self {
            file_path: path.into(),
        }
    }

    /// 加载所有会话
    pub fn load_sessions(&self) -> Result<Vec<ChatSession>> {
        if !Path::new(&self.file_path).exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&self.file_path)
            .with_context(|| format!("无法读取会话文件: {}", self.file_path))?;

        let sessions: Vec<ChatSession> = serde_json::from_str(&content)
            .with_context(|| "解析会话文件失败")?;

        Ok(sessions)
    }

    /// 保存所有会话
    pub fn save_sessions(&self, sessions: &[ChatSession]) -> Result<()> {
        let json = serde_json::to_string_pretty(sessions)
            .with_context(|| "序列化会话数据失败")?;

        fs::write(&self.file_path, json)
            .with_context(|| format!("写入会话文件失败: {}", self.file_path))?;

        tracing::debug!("已保存 {} 个会话到 {}", sessions.len(), self.file_path);
        Ok(())
    }

    /// 添加或更新单个会话
    pub fn save_session(&self, session: &ChatSession) -> Result<()> {
        let mut sessions = self.load_sessions().unwrap_or_default();

        // 更新或添加会话
        if let Some(existing) = sessions.iter_mut().find(|s| s.id == session.id) {
            *existing = session.clone();
        } else {
            sessions.push(session.clone());
        }

        // 按更新时间降序排序
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        self.save_sessions(&sessions)
    }

    /// 删除会话
    pub fn delete_session(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.load_sessions().unwrap_or_default();
        sessions.retain(|s| s.id != session_id);
        self.save_sessions(&sessions)
    }

    /// 添加消息到会话
    pub fn add_message(&self, session_id: &str, message: Message) -> Result<()> {
        let mut sessions = self.load_sessions().unwrap_or_default();

        if let Some(session) = sessions.iter_mut().find(|s| s.id == session_id) {
            session.messages.push(message);
            session.updated_at = Utc::now();
            self.save_sessions(&sessions)
        } else {
            anyhow::bail!("会话不存在: {}", session_id)
        }
    }
}

impl Default for SessionStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MessageContent, Role};
    use chrono::Utc;
    use std::fs;

    #[test]
    fn test_session_storage() {
        let temp_file = format!("test_sessions_{}.json", std::process::id());
        let storage = SessionStorage::with_path(&temp_file);

        // 清理测试文件
        let _ = fs::remove_file(&temp_file);

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

        // 清理测试文件
        let _ = fs::remove_file(&temp_file);
    }
}

