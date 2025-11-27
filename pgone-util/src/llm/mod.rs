mod config;
mod error;

pub mod chat;
pub mod embeddings;
pub mod images;
pub mod audio;
pub mod files;
pub mod models;
pub mod tools;

pub use config::Config;
pub use error::{LlmError, Result};

use async_openai::Client as OpenAiClient;
use async_openai::config::OpenAIConfig;

pub struct Client {
    inner: OpenAiClient<OpenAIConfig>,
    config: Config,
}

impl Client {
    pub fn new(config: Config) -> crate::llm::error::Result<Self> {
        config.validate()?;
        let mut openai_config = OpenAIConfig::new().with_api_key(config.api_key.clone());
        
        if let Some(ref base_url) = config.base_url {
            openai_config = openai_config.with_api_base(base_url.clone());
        }

        let inner = OpenAiClient::with_config(openai_config);
        
        Ok(Self { inner, config })
    }

    pub fn from_api_key(api_key: String) -> crate::llm::error::Result<Self> {
        Self::new(Config::new(api_key))
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn inner(&self) -> &OpenAiClient<OpenAIConfig> {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut OpenAiClient<OpenAIConfig> {
        &mut self.inner
    }
}

// 向后兼容函数
pub async fn chat_once(api_key: String, model: String, prompt: String) -> std::result::Result<String, String> {
    let client = match Client::from_api_key(api_key) {
        Ok(c) => c,
        Err(e) => return Err(e.to_string()),
    };

    let request = chat::ChatRequest::new(model)
        .with_messages(vec![
            chat::ChatMessage::system("You are a helpful assistant.".to_string()),
            chat::ChatMessage::user(prompt),
        ]);

    match client.chat_create(request).await {
        Ok(resp) => Ok(resp.content),
        Err(e) => Err(e.to_string()),
    }
}

pub async fn chat_with_tools(
    api_key: String,
    model: String,
    prompt: String,
) -> std::result::Result<String, String> {
    let client = match Client::from_api_key(api_key) {
        Ok(c) => c,
        Err(e) => return Err(e.to_string()),
    };

    let request = chat::ChatRequest::new(model)
        .with_messages(vec![
            chat::ChatMessage::system("You are a helpful assistant.".to_string()),
            chat::ChatMessage::user(prompt),
        ]);

    match client.chat_create(request).await {
        Ok(resp) => Ok(resp.content),
        Err(e) => Err(e.to_string()),
    }
}

