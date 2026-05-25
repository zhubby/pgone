use crate::providers::openrouter;
use crate::{Client, LLMProvider, Result};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub owned_by: String,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    pub name: String,
    #[serde(rename = "modified_at")]
    #[allow(dead_code)]
    pub modified_at: Option<String>,
    #[allow(dead_code)]
    pub size: Option<u64>,
    #[allow(dead_code)]
    pub digest: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

impl Client {
    pub async fn models_list(&self) -> Result<Vec<ModelInfo>> {
        match self.provider() {
            LLMProvider::Ollama => self.models_list_ollama().await,
            LLMProvider::OpenRouter => self.models_list_openrouter().await,
            _ => {
                // OpenAI 兼容的 API
                let resp = self.inner().models().list().await?;

                Ok(resp
                    .data
                    .into_iter()
                    .map(|m| ModelInfo {
                        id: m.id,
                        object: m.object,
                        created: m.created as u64,
                        owned_by: m.owned_by,
                    })
                    .collect())
            }
        }
    }

    async fn models_list_ollama(&self) -> Result<Vec<ModelInfo>> {
        let base_url = self
            .config()
            .base_url
            .as_deref()
            .unwrap_or("http://localhost:11434");

        // 确保 base_url 不以 / 结尾
        let base_url = base_url.trim_end_matches('/');

        let url = format!("{}/api/tags", base_url);

        let client = if self.config().proxy_enabled {
            reqwest::Client::builder()
                .proxy(reqwest::Proxy::http(self.config().proxy_url().unwrap())?)
                .build()?
        } else {
            reqwest::Client::builder().no_proxy().build()?
        };
        let resp = client.get(&url).send().await?;

        if !resp.status().is_success() {
            return Err(crate::LlmError::Api(format!(
                "Ollama API 请求失败: {}",
                resp.status()
            )));
        }

        let ollama_resp: OllamaTagsResponse = resp.json().await?;

        // 获取当前时间戳作为 created 字段
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Ok(ollama_resp
            .models
            .into_iter()
            .map(|m| ModelInfo {
                id: m.name.clone(),
                object: "model".to_string(),
                created: now,
                owned_by: "ollama".to_string(),
            })
            .collect())
    }

    async fn models_list_openrouter(&self) -> Result<Vec<ModelInfo>> {
        let base_url = self.config().base_url.as_deref();
        let proxy_url = if self.config().proxy_enabled {
            self.config().proxy_url()
        } else {
            None
        };
        openrouter::list_models(&self.config().api_key, base_url, proxy_url).await
    }

    pub async fn models_retrieve(&self, model_id: String) -> Result<ModelInfo> {
        match self.provider() {
            LLMProvider::Ollama => {
                // 对于 Ollama，从列表中查找指定模型
                let models = self.models_list().await?;
                models
                    .into_iter()
                    .find(|m| m.id == model_id)
                    .ok_or_else(|| {
                        crate::LlmError::InvalidModel(format!("模型 {} 未找到", model_id))
                    })
            }
            LLMProvider::OpenRouter => {
                // 对于 OpenRouter，从列表中查找指定模型
                let models = self.models_list().await?;
                models
                    .into_iter()
                    .find(|m| m.id == model_id)
                    .ok_or_else(|| {
                        crate::LlmError::InvalidModel(format!("模型 {} 未找到", model_id))
                    })
            }
            _ => {
                // OpenAI 兼容的 API
                let resp = self.inner().models().retrieve(&model_id).await?;

                Ok(ModelInfo {
                    id: resp.id,
                    object: resp.object,
                    created: resp.created as u64,
                    owned_by: resp.owned_by,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Client, Config, LLMProvider};

    #[test]
    fn test_model_info_structure() {
        let model = ModelInfo {
            id: "test-model".to_string(),
            object: "model".to_string(),
            created: 1234567890,
            owned_by: "test".to_string(),
        };

        assert_eq!(model.id, "test-model");
        assert_eq!(model.object, "model");
        assert_eq!(model.created, 1234567890);
        assert_eq!(model.owned_by, "test");
    }

    #[test]
    fn test_ollama_model_deserialize() {
        let json = r#"
        {
            "name": "llama3.2",
            "modified_at": "2024-01-01T00:00:00Z",
            "size": 1234567890,
            "digest": "sha256:abc123"
        }
        "#;

        let model: OllamaModel = serde_json::from_str(json).unwrap();
        assert_eq!(model.name, "llama3.2");
        assert_eq!(model.modified_at, Some("2024-01-01T00:00:00Z".to_string()));
        assert_eq!(model.size, Some(1234567890));
        assert_eq!(model.digest, Some("sha256:abc123".to_string()));
    }

    #[test]
    fn test_ollama_tags_response_deserialize() {
        let json = r#"
        {
            "models": [
                {
                    "name": "llama3.2",
                    "modified_at": "2024-01-01T00:00:00Z",
                    "size": 1234567890,
                    "digest": "sha256:abc123"
                },
                {
                    "name": "mistral",
                    "modified_at": "2024-01-02T00:00:00Z",
                    "size": 9876543210,
                    "digest": "sha256:def456"
                }
            ]
        }
        "#;

        let response: OllamaTagsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.models.len(), 2);
        assert_eq!(response.models[0].name, "llama3.2");
        assert_eq!(response.models[1].name, "mistral");
    }

    #[test]
    fn test_ollama_model_deserialize_minimal() {
        // 测试最小字段（只有 name）
        let json = r#"
        {
            "name": "test-model"
        }
        "#;

        let model: OllamaModel = serde_json::from_str(json).unwrap();
        assert_eq!(model.name, "test-model");
        assert_eq!(model.modified_at, None);
        assert_eq!(model.size, None);
        assert_eq!(model.digest, None);
    }

    #[test]
    fn test_client_new_with_ollama_provider() {
        let config = Config::new("test-key".to_string());
        let client = Client::new(config, LLMProvider::Ollama);
        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.provider(), LLMProvider::Ollama);
    }

    #[test]
    fn test_client_new_with_openai_provider() {
        let config = Config::new("test-key".to_string());
        let client = Client::new(config, LLMProvider::OpenAI);
        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.provider(), LLMProvider::OpenAI);
    }

    #[test]
    fn test_client_with_custom_base_url() {
        let config =
            Config::new("test-key".to_string()).with_base_url("http://localhost:8080".to_string());
        let client = Client::new(config, LLMProvider::Ollama);
        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(
            client.config().base_url,
            Some("http://localhost:8080".to_string())
        );
    }

    #[tokio::test]
    #[ignore] // 需要真实的 Ollama 服务运行，默认忽略
    async fn test_models_list_ollama_integration() {
        let config =
            Config::new("ollama".to_string()).with_base_url("http://localhost:11434".to_string());
        let client = Client::new(config, LLMProvider::Ollama).unwrap();

        let result = client.models_list().await;
        assert!(result.is_ok(), "应该能够成功获取 Ollama 模型列表");

        let models = result.unwrap();
        // 至少应该有一些模型（如果 Ollama 已安装）
        // 如果没有模型，列表可能为空，这也是正常的
        for model in &models {
            assert!(!model.id.is_empty(), "模型 ID 不应该为空");
            assert_eq!(model.object, "model");
            assert_eq!(model.owned_by, "ollama");
        }
    }

    #[tokio::test]
    #[ignore] // 需要真实的 Ollama 服务运行，默认忽略
    async fn test_models_retrieve_ollama_integration() {
        let config =
            Config::new("ollama".to_string()).with_base_url("http://localhost:11434".to_string());
        let client = Client::new(config, LLMProvider::Ollama).unwrap();

        // 先获取模型列表
        let models = client.models_list().await.unwrap();

        if !models.is_empty() {
            // 如果列表不为空，尝试获取第一个模型
            let first_model_id = models[0].id.clone();
            let result = client.models_retrieve(first_model_id.clone()).await;

            assert!(result.is_ok(), "应该能够成功获取指定模型");
            let model = result.unwrap();
            assert_eq!(model.id, first_model_id);
            assert_eq!(model.owned_by, "ollama");
        }
    }

    #[tokio::test]
    #[ignore] // 需要真实的 Ollama 服务运行，默认忽略
    async fn test_models_retrieve_ollama_not_found() {
        let config =
            Config::new("ollama".to_string()).with_base_url("http://localhost:11434".to_string());
        let client = Client::new(config, LLMProvider::Ollama).unwrap();

        // 尝试获取一个不存在的模型
        let result = client
            .models_retrieve("non-existent-model".to_string())
            .await;
        assert!(result.is_err(), "应该返回错误当模型不存在时");

        if let Err(e) = result {
            assert!(e.to_string().contains("未找到"), "错误消息应该包含'未找到'");
        }
    }

    #[tokio::test]
    #[ignore] // 需要真实的 OpenAI API key，默认忽略
    async fn test_models_list_openai_integration() {
        let api_key = std::env::var("OPENAI_API_KEY")
            .expect("OPENAI_API_KEY environment variable must be set for integration tests");

        let config = Config::new(api_key);
        let client = Client::new(config, LLMProvider::OpenAI).unwrap();

        let result = client.models_list().await;
        assert!(result.is_ok(), "应该能够成功获取 OpenAI 模型列表");

        let models = result.unwrap();
        assert!(!models.is_empty(), "OpenAI 模型列表不应该为空");

        for model in &models {
            assert!(!model.id.is_empty(), "模型 ID 不应该为空");
            assert!(!model.object.is_empty(), "object 字段不应该为空");
        }
    }

    #[tokio::test]
    #[ignore] // 需要真实的 OpenAI API key，默认忽略
    async fn test_models_retrieve_openai_integration() {
        let api_key = std::env::var("OPENAI_API_KEY")
            .expect("OPENAI_API_KEY environment variable must be set for integration tests");

        let config = Config::new(api_key);
        let client = Client::new(config, LLMProvider::OpenAI).unwrap();

        // 尝试获取一个已知的模型（例如 gpt-3.5-turbo）
        let result = client.models_retrieve("gpt-3.5-turbo".to_string()).await;

        // 如果模型存在，应该成功；如果不存在，会返回错误
        // 这里我们主要测试不会 panic
        let _ = result;
    }

    #[test]
    fn test_ollama_base_url_trimming() {
        // 测试 base_url 处理逻辑
        // 验证 trim_end_matches 能正确处理以 / 结尾的 URL
        let test_cases = vec![
            ("http://localhost:11434", "http://localhost:11434/api/tags"),
            ("http://localhost:11434/", "http://localhost:11434/api/tags"),
            ("http://example.com", "http://example.com/api/tags"),
            ("http://example.com/", "http://example.com/api/tags"),
        ];

        for (base_url, expected) in test_cases {
            let trimmed = base_url.trim_end_matches('/');
            let url = format!("{}/api/tags", trimmed);
            assert_eq!(url, expected, "URL 应该正确格式化");
            assert!(!url.contains("//api"), "URL 不应该包含双斜杠在 /api 之前");
            assert!(url.starts_with("http"), "URL 应该以 http 开头");
        }
    }
}
