use crate::{Client, LlmError, Result};
use async_openai::types::CreateEmbeddingRequestArgs;

#[derive(Debug, Clone)]
pub struct EmbeddingRequest {
    pub model: String,
    pub input: Vec<String>,
    pub user: Option<String>,
}

impl EmbeddingRequest {
    pub fn new(model: String, input: Vec<String>) -> Self {
        Self {
            model,
            input,
            user: None,
        }
    }

    pub fn with_user(mut self, user: String) -> Self {
        self.user = Some(user);
        self
    }
}

#[derive(Debug, Clone)]
pub struct EmbeddingResponse {
    pub embeddings: Vec<Vec<f32>>,
    pub model: String,
    pub usage: EmbeddingUsage,
}

#[derive(Debug, Clone)]
pub struct EmbeddingUsage {
    pub prompt_tokens: u32,
    pub total_tokens: u32,
}

impl Client {
    pub async fn embeddings_create(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse> {
        let mut req_builder = CreateEmbeddingRequestArgs::default();
        req_builder.model(request.model.clone());
        req_builder.input(request.input.clone());
        
        if let Some(user) = request.user {
            req_builder.user(user);
        }

        let req = req_builder.build().map_err(|e| LlmError::InvalidRequest(e.to_string()))?;
        let resp = self.inner().embeddings().create(req).await?;

        let embeddings: Vec<Vec<f32>> = resp.data.into_iter().map(|e| e.embedding).collect();
        let usage = EmbeddingUsage {
            prompt_tokens: resp.usage.prompt_tokens,
            total_tokens: resp.usage.total_tokens,
        };

        Ok(EmbeddingResponse {
            embeddings,
            model: resp.model,
            usage,
        })
    }

    pub async fn embedding_create(&self, model: String, text: String) -> Result<Vec<f32>> {
        let request = EmbeddingRequest::new(model, vec![text]);
        let response = self.embeddings_create(request).await?;
        response.embeddings.into_iter().next().ok_or_else(|| {
            LlmError::Api("No embedding in response".to_string())
        })
    }
}

