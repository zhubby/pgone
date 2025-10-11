use chrono::{DateTime, Utc};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: u64,
    pub title: String,
    pub messages: Vec<Message>,
    pub db: DbConfig,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct PersistedState {
    pub sessions: Vec<Session>,
    pub current_index: usize,
    pub next_session_id: u64,
    pub settings: Settings,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SendShortcut {
    Enter,
    CmdEnter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub dark_theme: bool,
    pub send_shortcut: SendShortcut,
    pub openai_api_key: Option<String>,
    pub openai_model: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            dark_theme: true,
            send_shortcut: SendShortcut::CmdEnter,
            openai_api_key: None,
            openai_model: "gpt-4o-mini".to_string(),
        }
    }
}
