use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use pgone_llm::LLMProvider;

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
            sessions: vec![ChatSession::new(
                "0".to_string(),
                "新会话".to_string(),
            )],
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    System,
    Latte,
    Frappe,
    Macchiato,
    Mocha,
}

impl Theme {
    pub fn display_name(&self) -> &'static str {
        match self {
            Theme::System => "跟随系统",
            Theme::Latte => "Catppuccin Latte",
            Theme::Frappe => "Catppuccin Frappe",
            Theme::Macchiato => "Catppuccin Macchiato",
            Theme::Mocha => "Catppuccin Mocha",
        }
    }

    pub fn all() -> &'static [Theme] {
        &[Theme::System, Theme::Latte, Theme::Frappe, Theme::Macchiato, Theme::Mocha]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub send_shortcut: SendShortcut,
    pub openai_api_key: Option<String>,
    pub openai_base_url: Option<String>,
    pub openai_model: String,
    pub font_family: String,
    pub font_size: f32,
    #[serde(default = "default_theme")]
    pub theme: Theme,
    #[serde(default = "default_llm_provider")]
    pub llm_provider: LLMProvider,
    #[serde(default = "default_enable_monitor")]
    pub enable_monitor: bool,
}

fn default_theme() -> Theme {
    Theme::System
}

fn default_llm_provider() -> LLMProvider {
    LLMProvider::OpenAI
}

fn default_enable_monitor() -> bool {
    false
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            send_shortcut: SendShortcut::CmdEnter,
            openai_api_key: None,
            openai_base_url: None,
            openai_model: "gpt-4o-mini".to_string(),
            font_family: "LXGWWenKai-Medium".to_string(),
            font_size: 12.0,
            theme: Theme::System,
            llm_provider: LLMProvider::OpenAI,
            enable_monitor: false,
        }
    }
}

impl Settings {
    /// Convert Settings to key-value HashMap for storage
    pub fn to_kv_map(&self) -> std::collections::HashMap<String, String> {
        use std::collections::HashMap;
        let mut map = HashMap::new();
        
        // Serialize enum as JSON string
        map.insert("send_shortcut".to_string(), serde_json::to_string(&self.send_shortcut).unwrap_or_default());
        map.insert("theme".to_string(), serde_json::to_string(&self.theme).unwrap_or_default());
        map.insert("llm_provider".to_string(), serde_json::to_string(&self.llm_provider).unwrap_or_default());
        
        // Store Option<String> as JSON (null or string)
        if let Some(ref key) = self.openai_api_key {
            map.insert("openai_api_key".to_string(), key.clone());
        } else {
            map.insert("openai_api_key".to_string(), "".to_string());
        }
        
        if let Some(ref url) = self.openai_base_url {
            map.insert("openai_base_url".to_string(), url.clone());
        } else {
            map.insert("openai_base_url".to_string(), "".to_string());
        }
        
        // Simple string values
        map.insert("openai_model".to_string(), self.openai_model.clone());
        map.insert("font_family".to_string(), self.font_family.clone());
        map.insert("font_size".to_string(), self.font_size.to_string());
        map.insert("enable_monitor".to_string(), self.enable_monitor.to_string());
        
        map
    }
    
    /// Create Settings from key-value HashMap
    pub fn from_kv_map(map: &std::collections::HashMap<String, String>) -> Self {
        let mut settings = Settings::default();
        
        tracing::debug!("from_kv_map: input map = {:?}", map);
        
        // Parse send_shortcut
        if let Some(value) = map.get("send_shortcut") {
            if let Ok(shortcut) = serde_json::from_str::<SendShortcut>(value) {
                settings.send_shortcut = shortcut;
            }
        }
        
        // Parse theme
        if let Some(value) = map.get("theme") {
            if let Ok(theme) = serde_json::from_str::<Theme>(value) {
                settings.theme = theme;
            }
        }
        
        // Parse llm_provider
        if let Some(value) = map.get("llm_provider") {
            if let Ok(provider) = serde_json::from_str::<LLMProvider>(value) {
                settings.llm_provider = provider;
            }
        }
        
        // Parse openai_api_key (empty string means None, but if key exists, use the value)
        if let Some(value) = map.get("openai_api_key") {
            tracing::debug!("Found openai_api_key in map: '{}'", value);
            if !value.is_empty() {
                settings.openai_api_key = Some(value.clone());
            } else {
                // Explicitly set to None if empty string
                settings.openai_api_key = None;
            }
        } else {
            tracing::debug!("openai_api_key not found in map");
        }
        
        // Parse openai_base_url (empty string means None, but if key exists, use the value)
        if let Some(value) = map.get("openai_base_url") {
            tracing::debug!("Found openai_base_url in map: '{}'", value);
            if !value.is_empty() {
                settings.openai_base_url = Some(value.clone());
            } else {
                // Explicitly set to None if empty string
                settings.openai_base_url = None;
            }
        } else {
            tracing::debug!("openai_base_url not found in map");
        }
        
        // Parse openai_model
        if let Some(value) = map.get("openai_model") {
            if !value.is_empty() {
                settings.openai_model = value.clone();
            }
        }
        
        // Parse font_family
        if let Some(value) = map.get("font_family") {
            if !value.is_empty() {
                settings.font_family = value.clone();
            }
        }
        
        // Parse font_size
        if let Some(value) = map.get("font_size") {
            if let Ok(size) = value.parse::<f32>() {
                settings.font_size = size;
            }
        }
        
        // Parse enable_monitor
        if let Some(value) = map.get("enable_monitor") {
            if let Ok(enabled) = value.parse::<bool>() {
                settings.enable_monitor = enabled;
            }
        }
        
        tracing::debug!("from_kv_map: result settings = {:?}", settings);
        settings
    }
}
