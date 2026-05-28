use crate::components::ChatCtx;
use crate::futures;
use pgone_agent::{LlmConfig, LlmProviderKind, list_models};
use tokio::sync::mpsc;

pub struct ModelLoader {
    pub available_models: Vec<String>,
    pub models_receiver: Option<mpsc::Receiver<Result<Vec<String>, String>>>,
    pub models_loaded: bool,
}

impl Default for ModelLoader {
    fn default() -> Self {
        Self {
            available_models: Vec::new(),
            models_receiver: None,
            models_loaded: false,
        }
    }
}

impl ModelLoader {
    pub fn check_and_load(&mut self, ctxs: &ChatCtx) {
        // Load model list on first display
        if !self.models_loaded && ctxs.openai_api_key.is_some() && self.models_receiver.is_none() {
            self.load_models(ctxs);
        }

        // Check model loading results
        if let Some(ref mut receiver) = self.models_receiver {
            match receiver.try_recv() {
                Ok(result) => {
                    match result {
                        Ok(models) => {
                            self.available_models = models;
                            self.models_loaded = true;
                        }
                        Err(e) => {
                            tracing::error!("Failed to load model list: {}", e);
                            // If loading fails, use the default model list
                            self.available_models = vec!["Unknown".to_string()];
                            self.models_loaded = true;
                        }
                    }
                    self.models_receiver = None;
                }
                Err(mpsc::error::TryRecvError::Empty) => {
                    // No results yet, continue waiting
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    // Channel disconnected, clean up
                    self.models_receiver = None;
                }
            }
        }
    }

    fn load_models(&mut self, ctxs: &ChatCtx) {
        let Some(api_key) = ctxs.openai_api_key.clone() else {
            return;
        };
        let base_url = ctxs.state.settings.openai_base_url.clone();
        let proxy_enabled = ctxs.state.settings.proxy_enabled;
        let proxy_host = ctxs.state.settings.proxy_host.clone();
        let proxy_port = ctxs.state.settings.proxy_port;
        let (sender, receiver) = mpsc::channel(1);
        self.models_receiver = Some(receiver);

        futures::spawn(async move {
            let mut config = LlmConfig::new(api_key);
            if let Some(url) = base_url {
                config = config.with_base_url(url);
            }
            if proxy_enabled {
                if let (Some(host), Some(port)) = (proxy_host, proxy_port) {
                    config = config.with_proxy(host, port);
                }
            }

            let result = match list_models(&config, LlmProviderKind::OpenAI).await {
                Ok(models) => {
                    let model_ids: Vec<String> = models.into_iter().map(|m| m.id).collect();
                    Ok(model_ids)
                }
                Err(e) => Err(e.to_string()),
            };

            let _ = sender.send(result).await;
        });
    }
}
