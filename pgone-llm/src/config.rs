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
    pub proxy_enabled: bool,
    pub proxy_host: Option<String>,
    pub proxy_port: Option<u16>,
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
            proxy_enabled: false,
            proxy_host: None,
            proxy_port: None,
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

    pub fn with_proxy(mut self, host: String, port: u16) -> Self {
        self.proxy_enabled = true;
        self.proxy_host = Some(host);
        self.proxy_port = Some(port);
        self
    }

    pub fn proxy_url(&self) -> Option<String> {
        if self.proxy_enabled {
            if let (Some(host), Some(port)) = (&self.proxy_host, &self.proxy_port) {
                Some(format!("http://{}:{}", host, port))
            } else {
                None
            }
        } else {
            None
        }
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
            proxy_enabled: false,
            proxy_host: None,
            proxy_port: None,
        }
    }
}

