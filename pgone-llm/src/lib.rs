mod config;
mod error;
mod providers;

pub mod audit;
pub mod chat;
pub mod embeddings;
pub mod images;
pub mod audio;
pub mod files;
pub mod models;
pub mod tools;
pub mod services;

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
        
        // Build HTTP client with proxy if configured
        let http_client = if config.proxy_enabled {
            reqwest::Client::builder()
                .proxy(reqwest::Proxy::http(config.proxy_url().unwrap())?)
                .build()?
        } else {
            reqwest::Client::builder().no_proxy().build()?
        };
        
        let mut openai_config = OpenAIConfig::new()
            .with_api_key(config.api_key.clone());
            // .with_http_client(http_client);
        
        if let Some(ref base_url) = config.base_url {
            openai_config = openai_config.with_api_base(base_url.clone());
        }

        let inner = OpenAiClient::with_config(openai_config).with_http_client(http_client);
        
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
