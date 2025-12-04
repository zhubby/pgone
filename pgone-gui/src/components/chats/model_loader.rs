use crate::components::ChatCtx;
use crate::futures;
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
        // 在首次显示时加载模型列表
        if !self.models_loaded
            && ctxs.openai_api_key.is_some()
            && self.models_receiver.is_none()
        {
            self.load_models(ctxs);
        }

        // 检查模型加载结果
        if let Some(ref mut receiver) = self.models_receiver {
            match receiver.try_recv() {
                Ok(result) => {
                    match result {
                        Ok(models) => {
                            self.available_models = models;
                            self.models_loaded = true;
                        }
                        Err(e) => {
                            tracing::error!("加载模型列表失败: {}", e);
                            // 如果加载失败，使用默认模型列表
                            self.available_models = vec!["Unknown".to_string()];
                            self.models_loaded = true;
                        }
                    }
                    self.models_receiver = None;
                }
                Err(mpsc::error::TryRecvError::Empty) => {
                    // 还没有结果，继续等待
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    // Channel已断开，清理
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
        let provider = ctxs.state.settings.llm_provider;
        let proxy_enabled = ctxs.state.settings.proxy_enabled;
        let proxy_host = ctxs.state.settings.proxy_host.clone();
        let proxy_port = ctxs.state.settings.proxy_port;
        let (sender, receiver) = mpsc::channel(1);
        self.models_receiver = Some(receiver);
        
        futures::spawn(async move {
            let mut config = pgone_llm::Config::new(api_key);
            if let Some(url) = base_url {
                config = config.with_base_url(url);
            }
            if proxy_enabled {
                if let (Some(host), Some(port)) = (proxy_host, proxy_port) {
                    config = config.with_proxy(host, port);
                }
            }
            
            let result = match pgone_llm::Client::new(config, provider) {
                Ok(client) => {
                    match client.models_list().await {
                        Ok(models) => {
                            let model_ids: Vec<String> = models
                                .into_iter()
                                .map(|m| m.id)
                                .collect();
                            Ok(model_ids)
                        }
                        Err(e) => Err(e.to_string()),
                    }
                }
                Err(e) => Err(e.to_string()),
            };
            
            let _ = sender.send(result).await;
        });
    }
}

