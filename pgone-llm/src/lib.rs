mod config;
mod error;
mod providers;

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
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

pub struct Client {
    inner: OpenAiClient<OpenAIConfig>,
    config: Config,
    provider: LLMProvider,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Display, EnumString)]
pub enum LLMProvider {
    #[default]
    OpenAI,
    Gemini,
    Moonshot,
    DeepSeek,
    Ollama,
    BigModel,
}

impl Client {
    pub fn new(config: Config, provider: LLMProvider) -> crate::error::Result<Self> {
        config.validate()?;
        let mut openai_config = OpenAIConfig::new().with_api_key(config.api_key.clone());
        
        if let Some(ref base_url) = config.base_url {
            openai_config = openai_config.with_api_base(base_url.clone());
        }

        let inner = OpenAiClient::with_config(openai_config);
        
        Ok(Self { inner, config, provider })
    }

    pub fn from_api_key(api_key: String) -> crate::error::Result<Self> {
        Self::new(Config::new(api_key), LLMProvider::OpenAI)
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn provider(&self) -> LLMProvider {
        self.provider
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
    chat_once_with_provider(api_key, model, prompt, LLMProvider::OpenAI).await
}

pub async fn chat_once_with_provider(
    api_key: String,
    model: String,
    prompt: String,
    provider: LLMProvider,
) -> std::result::Result<String, String> {
    let client = match Client::new(Config::new(api_key), provider) {
        Ok(c) => c,
        Err(e) => return Err(e.to_string()),
    };

    let request = chat::ChatRequest::new(model)
        .with_messages(vec![
            chat::ChatMessage::system("You are a helpful assistant.".to_string()),
            chat::ChatMessage::user(prompt),
        ]);

    match provider {
        LLMProvider::OpenAI | LLMProvider::Moonshot | LLMProvider::DeepSeek | LLMProvider::Ollama | LLMProvider::BigModel => {
            // 使用 OpenAI 兼容的 API
            match client.chat_create(request).await {
                Ok(resp) => Ok(resp.content),
                Err(e) => Err(e.to_string()),
            }
        }
        LLMProvider::Gemini => {
            // 使用 Gemini 客户端
            let gemini_client = match providers::gemini::GeminiClient::new(client.config().api_key.clone()) {
                Ok(c) => c,
                Err(e) => return Err(e.to_string()),
            };
            match gemini_client.chat_create(request).await {
                Ok(resp) => Ok(resp.content),
                Err(e) => Err(e.to_string()),
            }
        }
    }
}

pub async fn chat_with_tools(
    api_key: String,
    model: String,
    prompt: String,
) -> std::result::Result<String, String> {
    chat_with_tools_with_provider(api_key, model, prompt, LLMProvider::OpenAI).await
}

pub async fn chat_with_tools_with_provider(
    api_key: String,
    model: String,
    prompt: String,
    provider: LLMProvider,
) -> std::result::Result<String, String> {
    let client = match Client::new(Config::new(api_key), provider) {
        Ok(c) => c,
        Err(e) => return Err(e.to_string()),
    };

    let request = chat::ChatRequest::new(model)
        .with_messages(vec![
            chat::ChatMessage::system("You are a helpful assistant.".to_string()),
            chat::ChatMessage::user(prompt),
        ]);

    match provider {
        LLMProvider::OpenAI | LLMProvider::Moonshot | LLMProvider::DeepSeek | LLMProvider::Ollama | LLMProvider::BigModel => {
            // 使用 OpenAI 兼容的 API
            match client.chat_create(request).await {
                Ok(resp) => Ok(resp.content),
                Err(e) => Err(e.to_string()),
            }
        }
        LLMProvider::Gemini => {
            // 使用 Gemini 客户端
            let gemini_client = match providers::gemini::GeminiClient::new(client.config().api_key.clone()) {
                Ok(c) => c,
                Err(e) => return Err(e.to_string()),
            };
            match gemini_client.chat_create(request).await {
                Ok(resp) => Ok(resp.content),
                Err(e) => Err(e.to_string()),
            }
        }
    }
}

pub async fn chat_with_tools_custom_endpoint(
    api_key: String,
    base_url: String,
    model: String,
    prompt: String,
) -> std::result::Result<String, String> {
    chat_with_tools_custom_endpoint_with_provider(api_key, base_url, model, prompt, LLMProvider::OpenAI).await
}

pub async fn chat_with_tools_custom_endpoint_with_provider(
    api_key: String,
    base_url: String,
    model: String,
    prompt: String,
    provider: LLMProvider,
) -> std::result::Result<String, String> {
    let config = Config::new(api_key).with_base_url(base_url);
    let client = match Client::new(config, provider) {
        Ok(c) => c,
        Err(e) => return Err(e.to_string()),
    };

    let request = chat::ChatRequest::new(model)
        .with_messages(vec![
            chat::ChatMessage::system("You are a helpful assistant.".to_string()),
            chat::ChatMessage::user(prompt),
        ]);

    match provider {
        LLMProvider::OpenAI | LLMProvider::Moonshot | LLMProvider::DeepSeek | LLMProvider::Ollama | LLMProvider::BigModel => {
            // 使用 OpenAI 兼容的 API
            match client.chat_create(request).await {
                Ok(resp) => Ok(resp.content),
                Err(e) => Err(e.to_string()),
            }
        }
        LLMProvider::Gemini => {
            // 使用 Gemini 客户端
            let gemini_client = match providers::gemini::GeminiClient::new(client.config().api_key.clone()) {
                Ok(c) => c,
                Err(e) => return Err(e.to_string()),
            };
            match gemini_client.chat_create(request).await {
                Ok(resp) => Ok(resp.content),
                Err(e) => Err(e.to_string()),
            }
        }
    }
}
