use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbConfig {
    pub id: String,
    pub engine: String,
    pub dsn: String,
    pub default_schemas: Option<String>,
    pub include_system: Option<bool>,
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
pub enum Role { User, Assistant, System }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageKind { Markdown, Image, Video }

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


