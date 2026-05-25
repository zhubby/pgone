use crate::{LlmError, Result};
use serde::Deserialize;

/// OpenRouter Models API 响应结构
#[derive(Debug, Deserialize)]
pub struct OpenRouterModelsResponse {
    pub data: Vec<OpenRouterModel>,
}

/// OpenRouter 模型对象
#[derive(Debug, Deserialize)]
pub struct OpenRouterModel {
    pub id: String,
    #[serde(rename = "canonical_slug")]
    #[allow(dead_code)]
    pub canonical_slug: Option<String>,
    pub name: String,
    pub created: u64,
    #[serde(default)]
    #[allow(dead_code)]
    pub description: Option<String>,
    #[serde(rename = "context_length")]
    #[allow(dead_code)]
    pub context_length: Option<u64>,
    #[serde(default)]
    #[allow(dead_code)]
    pub architecture: Option<serde_json::Value>,
    #[serde(default)]
    #[allow(dead_code)]
    pub pricing: Option<serde_json::Value>,
    #[serde(rename = "top_provider")]
    #[serde(default)]
    #[allow(dead_code)]
    pub top_provider: Option<serde_json::Value>,
    #[serde(rename = "per_request_limits")]
    #[serde(default)]
    #[allow(dead_code)]
    pub per_request_limits: Option<serde_json::Value>,
    #[serde(rename = "supported_parameters")]
    #[serde(default)]
    #[allow(dead_code)]
    pub supported_parameters: Option<Vec<String>>,
}

/// 获取 OpenRouter 模型列表
pub async fn list_models(
    api_key: &str,
    base_url: Option<&str>,
    proxy_url: Option<String>,
) -> Result<Vec<crate::models::ModelInfo>> {
    // OpenRouter Models API endpoint
    // If base_url is provided, use it; otherwise use the default OpenRouter API endpoint
    let url = if let Some(custom_base_url) = base_url {
        let custom_base_url = custom_base_url.trim_end_matches('/');
        // If custom base_url already includes /api/v1, use it directly
        if custom_base_url.contains("/api/v1") {
            format!("{}/models", custom_base_url)
        } else {
            format!("{}/api/v1/models", custom_base_url)
        }
    } else {
        // Default OpenRouter Models API endpoint
        "https://openrouter.ai/api/v1/models".to_string()
    };

    let mut client_builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(60));

    // Configure proxy if provided
    if let Some(proxy_url) = proxy_url {
        client_builder = client_builder.proxy(
            reqwest::Proxy::http(proxy_url)
                .map_err(|e| LlmError::Api(format!("Invalid proxy URL: {}", e)))?,
        );
    } else {
        client_builder = client_builder.no_proxy();
    }

    let client = client_builder.build().map_err(LlmError::Network)?;

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("HTTP-Referer", "https://github.com/pgone") // Optional: for analytics
        .header("X-Title", "PGone") // Optional: for analytics
        .send()
        .await
        .map_err(LlmError::Network)?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(LlmError::Api(format!(
            "OpenRouter API request failed ({}): {}",
            status, error_text
        )));
    }

    let response_text = response.text().await.map_err(LlmError::Network)?;

    // Check if response is empty
    if response_text.trim().is_empty() {
        return Err(LlmError::Api(
            "OpenRouter API returned empty response".to_string(),
        ));
    }

    // Check if response is HTML (likely an error page)
    if response_text.trim_start().starts_with("<!DOCTYPE")
        || response_text.trim_start().starts_with("<html")
    {
        return Err(LlmError::Api(format!(
            "OpenRouter API returned HTML instead of JSON. This usually indicates an authentication error or incorrect endpoint. Response preview: {}",
            if response_text.len() > 500 {
                format!("{}...", &response_text[..500])
            } else {
                response_text.clone()
            }
        )));
    }

    // Try to parse JSON with better error handling
    let openrouter_response: OpenRouterModelsResponse = serde_json::from_str(&response_text)
        .map_err(|e| {
            tracing::error!(
                "Failed to parse OpenRouter response. Response body: {}",
                if response_text.len() > 500 {
                    format!("{}...", &response_text[..500])
                } else {
                    response_text.clone()
                }
            );
            LlmError::Parse(e)
        })?;

    // 转换为统一的 ModelInfo 格式
    Ok(openrouter_response
        .data
        .into_iter()
        .map(|m| crate::models::ModelInfo {
            id: m.id,
            object: "model".to_string(),
            created: m.created,
            owned_by: m.name.clone(), // 使用 name 作为 owned_by，因为 OpenRouter 没有直接的 owned_by 字段
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openrouter_model_deserialize() {
        let json = r#"
        {
            "id": "google/gemini-2.5-pro-preview",
            "canonical_slug": "google/gemini-2.5-pro-preview",
            "name": "Gemini 2.5 Pro Preview",
            "created": 1234567890,
            "description": "A preview model",
            "context_length": 1000000,
            "architecture": {},
            "pricing": {},
            "top_provider": {},
            "supported_parameters": ["temperature", "max_tokens"]
        }
        "#;

        let model: OpenRouterModel = serde_json::from_str(json).unwrap();
        assert_eq!(model.id, "google/gemini-2.5-pro-preview");
        assert_eq!(model.name, "Gemini 2.5 Pro Preview");
        assert_eq!(model.created, 1234567890);
    }

    #[test]
    fn test_openrouter_models_response_deserialize() {
        let json = r#"
        {
            "data": [
                {
                    "id": "google/gemini-2.5-pro-preview",
                    "name": "Gemini 2.5 Pro Preview",
                    "created": 1234567890
                },
                {
                    "id": "openai/gpt-4",
                    "name": "GPT-4",
                    "created": 1234567891
                }
            ]
        }
        "#;

        let response: OpenRouterModelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.data.len(), 2);
        assert_eq!(response.data[0].id, "google/gemini-2.5-pro-preview");
        assert_eq!(response.data[1].id, "openai/gpt-4");
    }

    #[test]
    fn test_openrouter_model_minimal_fields() {
        // 测试最小字段（只有必需字段）
        let json = r#"
        {
            "id": "test-model",
            "name": "Test Model",
            "created": 1234567890
        }
        "#;

        let model: OpenRouterModel = serde_json::from_str(json).unwrap();
        assert_eq!(model.id, "test-model");
        assert_eq!(model.name, "Test Model");
        assert_eq!(model.created, 1234567890);
        assert_eq!(model.description, None);
        assert_eq!(model.context_length, None);
    }
}
