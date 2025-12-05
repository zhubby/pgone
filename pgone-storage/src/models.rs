use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbConfig {
    pub id: String,
    pub engine: String,
    pub dsn: String,
    pub default_schemas: Option<String>,
    pub include_system: Option<bool>,
    pub default_config: Option<bool>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub config_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Role {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageKind {
    Markdown,
    Image,
    Video,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: Role,
    pub timestamp: i64,
    pub kind: MessageKind,
    pub content_markdown: Option<String>,
    pub image_path: Option<String>,
    pub image_w: Option<i64>,
    pub image_h: Option<i64>,
    pub video_path: Option<String>,
    pub video_duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub id: String,
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub email: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    pub id: String,
    pub user_id: String,
    pub provider: String,
    pub access_token: String,
    pub scope: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileIndex {
    pub id: String,
    pub current_path: String,
    pub original_path: String,
    pub file_size: i64,
    pub file_type: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmAuditLog {
    pub id: String,
    pub session_id: Option<String>,
    pub provider: String,
    pub model: String,
    pub request_time: i64,
    pub response_time: Option<i64>,
    pub request_size: Option<i64>,
    pub response_size: Option<i64>,
    pub request_content: Option<String>,
    pub response_content: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub duration_ms: Option<i64>,
    pub created_at: i64,
}
