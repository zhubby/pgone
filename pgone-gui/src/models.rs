use chrono::{DateTime, Utc};
use pgone_llm::LLMProvider;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum MessageContent {
    Markdown(String),
    Image {
        path: PathBuf,
        width: u32,
        height: u32,
    },
    Video {
        path: PathBuf,
        duration_ms: Option<u64>,
        thumbnail: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub timestamp: DateTime<Utc>,
    pub content: MessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: String,
    pub title: String,
    pub messages: Vec<Message>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ChatSession {
    pub fn new(id: String, title: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            title,
            messages: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn default_with_timestamp(id: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            title: format!("新会话-{}", now.timestamp().to_string()),
            messages: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbConfig {
    pub engine: String,
    pub dsn: String,
}

impl DbConfig {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Engine");
            ui.text_edit_singleline(&mut self.engine);
        });
    }
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            engine: "postgres".to_string(),
            dsn: String::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PersistedState {
    pub current_db_config_id: Option<String>,
    pub settings: Settings,
    #[serde(default)]
    pub sessions: Vec<ChatSession>,
    #[serde(default)]
    pub current_index: usize,
    #[serde(default = "default_next_session_id")]
    pub next_session_id: u64,
}

fn default_next_session_id() -> u64 {
    1
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            current_db_config_id: None,
            settings: Settings::default(),
            sessions: vec![ChatSession::default_with_timestamp("0".to_string())],
            current_index: 0,
            next_session_id: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SendShortcut {
    Enter,
    CmdEnter,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub send_shortcut: SendShortcut,
    pub openai_api_key: Option<String>,
    pub openai_base_url: Option<String>,
    pub openai_model: String,
    pub font_family: String,
    pub font_size: f32,
    #[serde(default = "default_llm_provider")]
    pub llm_provider: LLMProvider,
    #[serde(default = "default_enable_monitor")]
    pub enable_monitor: bool,
    #[serde(default = "default_proxy_enabled")]
    pub proxy_enabled: bool,
    pub proxy_host: Option<String>,
    pub proxy_port: Option<u16>,
    #[serde(default = "default_enable_stream_api")]
    pub enable_stream_api: bool,
}

fn default_llm_provider() -> LLMProvider {
    LLMProvider::OpenAI
}

fn default_enable_monitor() -> bool {
    false
}

fn default_proxy_enabled() -> bool {
    false
}

fn default_enable_stream_api() -> bool {
    false
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            send_shortcut: SendShortcut::CmdEnter,
            openai_api_key: None,
            openai_base_url: None,
            openai_model: "gpt-4o-mini".to_string(),
            font_family: "LXGWWenKai-Regular".to_string(),
            font_size: 12.0,
            llm_provider: LLMProvider::OpenAI,
            enable_monitor: false,
            proxy_enabled: false,
            proxy_host: None,
            proxy_port: None,
            enable_stream_api: false,
        }
    }
}
