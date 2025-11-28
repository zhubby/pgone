use crate::error::{LlmError, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub api_key: String,
    pub base_url: Option<String>,
    pub timeout: Option<std::time::Duration>,
    pub max_retries: u32,
    pub default_model: Option<String>,
    pub default_temperature: Option<f32>,
    pub default_top_p: Option<f32>,
    pub default_max_tokens: Option<u32>,
}

impl Config {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: None,
            timeout: Some(std::time::Duration::from_secs(60)),
            max_retries: 3,
            default_model: None,
            default_temperature: None,
            default_top_p: None,
            default_max_tokens: None,
        }
    }

    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = Some(base_url);
        self
    }

    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    pub fn with_default_model(mut self, model: String) -> Self {
        self.default_model = Some(model);
        self
    }

    pub fn with_default_temperature(mut self, temperature: f32) -> Self {
        self.default_temperature = Some(temperature);
        self
    }

    pub fn with_default_top_p(mut self, top_p: f32) -> Self {
        self.default_top_p = Some(top_p);
        self
    }

    pub fn with_default_max_tokens(mut self, max_tokens: u32) -> Self {
        self.default_max_tokens = Some(max_tokens);
        self
    }

    pub fn validate(&self) -> Result<()> {
        if self.api_key.is_empty() {
            return Err(LlmError::InvalidApiKey);
        }
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: None,
            timeout: Some(std::time::Duration::from_secs(60)),
            max_retries: 3,
            default_model: None,
            default_temperature: None,
            default_top_p: None,
            default_max_tokens: None,
        }
    }
}

