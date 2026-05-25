use std::pin::Pin;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{AgentError, Result};

pub type ProviderChatStream = Pin<Box<dyn Stream<Item = Result<ProviderChatStreamEvent>> + Send>>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LlmProviderKind {
    #[default]
    OpenAI,
    Gemini,
    Moonshot,
    DeepSeek,
    Ollama,
    BigModel,
    OpenRouter,
}

#[derive(Clone, Debug)]
pub struct LlmConfig {
    pub api_key: String,
    pub base_url: Option<String>,
    pub proxy_enabled: bool,
    pub proxy_host: Option<String>,
    pub proxy_port: Option<u16>,
}

impl LlmConfig {
    #[must_use]
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: None,
            proxy_enabled: false,
            proxy_host: None,
            proxy_port: None,
        }
    }

    #[must_use]
    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = Some(base_url);
        self
    }

    #[must_use]
    pub fn with_proxy(mut self, host: String, port: u16) -> Self {
        self.proxy_enabled = true;
        self.proxy_host = Some(host);
        self.proxy_port = Some(port);
        self
    }

    #[must_use]
    pub fn proxy_url(&self) -> Option<String> {
        if self.proxy_enabled {
            match (&self.proxy_host, self.proxy_port) {
                (Some(host), Some(port)) => Some(format!("http://{host}:{port}")),
                _ => None,
            }
        } else {
            None
        }
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self::new(String::new())
    }
}

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub owned_by: String,
}

#[derive(Debug, Deserialize)]
struct ModelListResponse {
    data: Vec<ApiModel>,
}

#[derive(Debug, Deserialize)]
struct ApiModel {
    id: String,
    object: Option<String>,
    created: Option<u64>,
    owned_by: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
}

pub async fn list_models(config: &LlmConfig, provider: LlmProviderKind) -> Result<Vec<ModelInfo>> {
    match provider {
        LlmProviderKind::Ollama => list_ollama_models(config).await,
        LlmProviderKind::OpenRouter => list_openrouter_models(config).await,
        _ => list_openai_compatible_models(config).await,
    }
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, request: ProviderChatRequest) -> Result<ProviderChatResponse>;

    async fn chat_stream(&self, request: ProviderChatRequest) -> Result<ProviderChatStream> {
        let _ = request;
        Err(AgentError::Provider(
            "streaming chat is not implemented for this provider".to_owned(),
        ))
    }
}

#[derive(Clone, Debug)]
pub struct ProviderConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub stream: bool,
    pub proxy_enabled: bool,
    pub proxy_host: Option<String>,
    pub proxy_port: Option<u16>,
}

impl ProviderConfig {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
        stream: bool,
    ) -> Result<Self> {
        Ok(Self {
            base_url: required_setting(base_url.into(), "llm.base_url")?,
            api_key: required_setting(api_key.into(), "llm.api_key")?,
            model: required_setting(model.into(), "llm.model")?,
            stream,
            proxy_enabled: false,
            proxy_host: None,
            proxy_port: None,
        })
    }

    pub fn from_llm_config(
        config: &LlmConfig,
        model: impl Into<String>,
        stream: bool,
    ) -> Result<Self> {
        let base_url =
            required_setting(config.base_url.clone().unwrap_or_default(), "llm.base_url")?;
        let api_key = required_setting(config.api_key.clone(), "llm.api_key")?;
        let model = required_setting(model.into(), "llm.model")?;

        if config.proxy_enabled && config.proxy_url().is_none() {
            return Err(AgentError::Config(
                "proxy_host and proxy_port are required when proxy is enabled".to_owned(),
            ));
        }

        Ok(Self {
            base_url,
            api_key,
            model,
            stream,
            proxy_enabled: config.proxy_enabled,
            proxy_host: config.proxy_host.clone(),
            proxy_port: config.proxy_port,
        })
    }

    #[must_use]
    pub fn proxy_url(&self) -> Option<String> {
        if self.proxy_enabled {
            match (&self.proxy_host, self.proxy_port) {
                (Some(host), Some(port)) => Some(format!("http://{host}:{port}")),
                _ => None,
            }
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
pub struct OpenAiCompatibleProvider {
    config: ProviderConfig,
    client: reqwest::Client,
}

impl OpenAiCompatibleProvider {
    pub fn new(config: ProviderConfig) -> Result<Self> {
        let client = if let Some(proxy_url) = config.proxy_url() {
            reqwest::Client::builder()
                .proxy(
                    reqwest::Proxy::http(proxy_url)
                        .map_err(|error| AgentError::Config(error.to_string()))?,
                )
                .build()
                .map_err(|error| AgentError::Config(error.to_string()))?
        } else {
            reqwest::Client::builder()
                .no_proxy()
                .build()
                .map_err(|error| AgentError::Config(error.to_string()))?
        };
        Ok(Self { config, client })
    }

    fn endpoint(&self) -> String {
        self.config.base_url.clone()
    }
}

fn required_setting(value: String, field: &str) -> Result<String> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(AgentError::Config(format!(
            "{field} is required for agent mode"
        )));
    }
    Ok(value)
}

fn model_client(config: &LlmConfig) -> Result<reqwest::Client> {
    let builder = reqwest::Client::builder().timeout(Duration::from_secs(60));
    let builder = if let Some(proxy_url) = config.proxy_url() {
        builder.proxy(
            reqwest::Proxy::http(proxy_url).map_err(|error| AgentError::Config(error.to_string()))?,
        )
    } else {
        builder.no_proxy()
    };
    builder
        .build()
        .map_err(|error| AgentError::Config(error.to_string()))
}

fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

async fn list_openai_compatible_models(config: &LlmConfig) -> Result<Vec<ModelInfo>> {
    let base_url = required_setting(
        config.base_url.clone().unwrap_or_default(),
        "llm.base_url",
    )?;
    let api_key = required_setting(config.api_key.clone(), "llm.api_key")?;
    let url = models_endpoint(&base_url);
    let response = model_client(config)?
        .get(url)
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|error| AgentError::Provider(error.to_string()))?
        .error_for_status()
        .map_err(|error| AgentError::Provider(error.to_string()))?
        .json::<ModelListResponse>()
        .await
        .map_err(|error| AgentError::Provider(error.to_string()))?;

    Ok(response
        .data
        .into_iter()
        .map(|model| ModelInfo {
            id: model.id,
            object: model.object.unwrap_or_else(|| "model".to_owned()),
            created: model.created.unwrap_or_default(),
            owned_by: model.owned_by.or(model.name).unwrap_or_default(),
        })
        .collect())
}

async fn list_ollama_models(config: &LlmConfig) -> Result<Vec<ModelInfo>> {
    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or("http://localhost:11434")
        .trim_end_matches('/');
    let url = format!("{base_url}/api/tags");
    let response = model_client(config)?
        .get(url)
        .send()
        .await
        .map_err(|error| AgentError::Provider(error.to_string()))?
        .error_for_status()
        .map_err(|error| AgentError::Provider(error.to_string()))?
        .json::<OllamaTagsResponse>()
        .await
        .map_err(|error| AgentError::Provider(error.to_string()))?;
    let created = now_epoch_seconds();

    Ok(response
        .models
        .into_iter()
        .map(|model| ModelInfo {
            id: model.name,
            object: "model".to_owned(),
            created,
            owned_by: "ollama".to_owned(),
        })
        .collect())
}

async fn list_openrouter_models(config: &LlmConfig) -> Result<Vec<ModelInfo>> {
    let api_key = required_setting(config.api_key.clone(), "llm.api_key")?;
    let url = openrouter_models_endpoint(config.base_url.as_deref());
    let response = model_client(config)?
        .get(url)
        .bearer_auth(api_key)
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("HTTP-Referer", "https://github.com/pgone")
        .header("X-Title", "PGone")
        .send()
        .await
        .map_err(|error| AgentError::Provider(error.to_string()))?
        .error_for_status()
        .map_err(|error| AgentError::Provider(error.to_string()))?
        .json::<ModelListResponse>()
        .await
        .map_err(|error| AgentError::Provider(error.to_string()))?;

    Ok(response
        .data
        .into_iter()
        .map(|model| ModelInfo {
            id: model.id,
            object: model.object.unwrap_or_else(|| "model".to_owned()),
            created: model.created.unwrap_or_default(),
            owned_by: model.name.or(model.owned_by).unwrap_or_default(),
        })
        .collect())
}

fn models_endpoint(base_url: &str) -> String {
    let base_url = base_url.trim_end_matches('/');
    if base_url.ends_with("/models") {
        base_url.to_owned()
    } else if let Some(prefix) = base_url.strip_suffix("/chat/completions") {
        format!("{prefix}/models")
    } else {
        format!("{base_url}/models")
    }
}

fn openrouter_models_endpoint(base_url: Option<&str>) -> String {
    if let Some(base_url) = base_url {
        let base_url = base_url.trim_end_matches('/');
        if base_url.ends_with("/models") {
            base_url.to_owned()
        } else if base_url.contains("/api/v1") {
            format!("{base_url}/models")
        } else {
            format!("{base_url}/api/v1/models")
        }
    } else {
        "https://openrouter.ai/api/v1/models".to_owned()
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleProvider {
    #[tracing::instrument(name = "agent.provider.chat", skip_all, fields(model = %self.config.model))]
    async fn chat(&self, request: ProviderChatRequest) -> Result<ProviderChatResponse> {
        if self.config.stream {
            return self.chat_stream_to_response(request).await;
        }

        let body = self.chat_completion_request(request, false);
        let response = self
            .client
            .post(self.endpoint())
            .bearer_auth(&self.config.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|error| AgentError::Provider(error.to_string()))?
            .error_for_status()
            .map_err(|error| AgentError::Provider(error.to_string()))?
            .json::<ChatCompletionResponse>()
            .await
            .map_err(|error| AgentError::Provider(error.to_string()))?;

        let choice = response.choices.into_iter().next().ok_or_else(|| {
            AgentError::Provider("LLM response did not include a choice".to_owned())
        })?;
        Ok(ProviderChatResponse {
            message: choice.message,
        })
    }

    #[tracing::instrument(name = "agent.provider.chat_stream", skip_all, fields(model = %self.config.model))]
    async fn chat_stream(&self, request: ProviderChatRequest) -> Result<ProviderChatStream> {
        let body = self.chat_completion_request(request, true);
        let bytes = self
            .client
            .post(self.endpoint())
            .bearer_auth(&self.config.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|error| AgentError::Provider(error.to_string()))?
            .error_for_status()
            .map_err(|error| AgentError::Provider(error.to_string()))?
            .bytes_stream();

        Ok(futures::stream::unfold(
            (
                bytes,
                String::new(),
                Vec::<Result<ProviderChatStreamEvent>>::new(),
                ChatMessageAccumulator::default(),
                false,
            ),
            |(mut bytes, mut buffer, mut pending, mut accumulator, mut finished)| async move {
                loop {
                    if let Some(event) = pending.pop() {
                        return Some((event, (bytes, buffer, pending, accumulator, finished)));
                    }
                    if finished {
                        return None;
                    }
                    match bytes.next().await {
                        Some(Ok(chunk)) => {
                            buffer.push_str(&String::from_utf8_lossy(&chunk));
                            pending.extend(
                                parse_chat_completion_sse_events(
                                    &mut buffer,
                                    &mut accumulator,
                                    &mut finished,
                                )
                                .into_iter()
                                .rev(),
                            );
                        }
                        Some(Err(error)) => {
                            finished = true;
                            return Some((
                                Err(AgentError::Provider(error.to_string())),
                                (bytes, buffer, pending, accumulator, finished),
                            ));
                        }
                        None => {
                            finished = true;
                            if buffer.trim().is_empty() {
                                return None;
                            }
                            return Some((
                                Err(AgentError::Provider(
                                    "LLM stream ended with an incomplete SSE frame".to_owned(),
                                )),
                                (bytes, buffer, pending, accumulator, finished),
                            ));
                        }
                    }
                }
            },
        )
        .boxed())
    }
}

impl OpenAiCompatibleProvider {
    async fn chat_stream_to_response(
        &self,
        request: ProviderChatRequest,
    ) -> Result<ProviderChatResponse> {
        let mut stream = self.chat_stream(request).await?;
        while let Some(event) = stream.next().await {
            match event? {
                ProviderChatStreamEvent::Delta(_) => {}
                ProviderChatStreamEvent::Completed(response) => return Ok(response),
            }
        }
        Err(AgentError::Provider(
            "LLM stream ended before a completed response".to_owned(),
        ))
    }
}

#[derive(Clone, Debug)]
pub struct ProviderChatRequest {
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
}

#[derive(Clone, Debug)]
pub struct ProviderChatResponse {
    pub message: ChatMessage,
}

#[derive(Clone, Debug)]
pub enum ProviderChatStreamEvent {
    Delta(ChatMessageDelta),
    Completed(ProviderChatResponse),
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChatMessageDelta {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<ToolCallDelta>,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ToolCallDelta {
    pub index: usize,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub function: ToolCallFunctionDelta,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ToolCallFunctionDelta {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arguments: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
}

impl ChatMessage {
    #[must_use]
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_owned(),
            content: Some(content.into()),
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }

    #[must_use]
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_owned(),
            content: Some(content.into()),
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }

    #[must_use]
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_owned(),
            content: Some(content.into()),
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }

    #[must_use]
    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".to_owned(),
            content: Some(content.into()),
            tool_call_id: Some(tool_call_id.into()),
            tool_calls: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(default)]
    pub r#type: String,
    pub function: ToolCallFunction,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Clone, Debug)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ChatTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<&'static str>,
    stream: bool,
}

impl OpenAiCompatibleProvider {
    fn chat_completion_request(
        &self,
        request: ProviderChatRequest,
        stream: bool,
    ) -> ChatCompletionRequest {
        let tools = request
            .tools
            .into_iter()
            .map(ChatTool::from)
            .collect::<Vec<_>>();
        let tool_choice = (!tools.is_empty()).then_some("auto");
        ChatCompletionRequest {
            model: self.config.model.clone(),
            messages: request.messages,
            tools,
            tool_choice,
            stream,
        }
    }
}

#[derive(Serialize)]
struct ChatTool {
    r#type: &'static str,
    function: ChatToolFunction,
}

impl From<ToolDefinition> for ChatTool {
    fn from(value: ToolDefinition) -> Self {
        Self {
            r#type: "function",
            function: ChatToolFunction {
                name: value.name,
                description: value.description,
                parameters: value.parameters,
            },
        }
    }
}

#[derive(Serialize)]
struct ChatToolFunction {
    name: String,
    description: String,
    parameters: Value,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatCompletionChunk {
    choices: Vec<ChatChunkChoice>,
}

#[derive(Deserialize)]
struct ChatChunkChoice {
    delta: ChatMessageDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Default)]
struct ChatMessageAccumulator {
    role: Option<String>,
    content: String,
    tool_calls: Vec<ToolCallAccumulator>,
}

impl ChatMessageAccumulator {
    fn apply(&mut self, delta: &ChatMessageDelta) {
        if let Some(role) = &delta.role {
            self.role = Some(role.clone());
        }
        if let Some(content) = &delta.content {
            self.content.push_str(content);
        }
        for tool_delta in &delta.tool_calls {
            while self.tool_calls.len() <= tool_delta.index {
                self.tool_calls.push(ToolCallAccumulator::default());
            }
            self.tool_calls[tool_delta.index].apply(tool_delta);
        }
    }

    fn message(&self) -> ChatMessage {
        ChatMessage {
            role: self.role.clone().unwrap_or_else(|| "assistant".to_owned()),
            content: (!self.content.is_empty()).then(|| self.content.clone()),
            tool_call_id: None,
            tool_calls: self
                .tool_calls
                .iter()
                .map(ToolCallAccumulator::tool_call)
                .collect(),
        }
    }
}

#[derive(Default)]
struct ToolCallAccumulator {
    id: Option<String>,
    r#type: Option<String>,
    name: String,
    arguments: String,
}

impl ToolCallAccumulator {
    fn apply(&mut self, delta: &ToolCallDelta) {
        if let Some(id) = &delta.id {
            self.id = Some(id.clone());
        }
        if let Some(tool_type) = &delta.r#type {
            self.r#type = Some(tool_type.clone());
        }
        if let Some(name) = &delta.function.name {
            self.name.push_str(name);
        }
        if let Some(arguments) = &delta.function.arguments {
            self.arguments.push_str(arguments);
        }
    }

    fn tool_call(&self) -> ToolCall {
        ToolCall {
            id: self.id.clone().unwrap_or_default(),
            r#type: self.r#type.clone().unwrap_or_else(|| "function".to_owned()),
            function: ToolCallFunction {
                name: self.name.clone(),
                arguments: self.arguments.clone(),
            },
        }
    }
}

fn parse_chat_completion_sse_events(
    buffer: &mut String,
    accumulator: &mut ChatMessageAccumulator,
    finished: &mut bool,
) -> Vec<Result<ProviderChatStreamEvent>> {
    let mut events = Vec::new();
    while let Some((index, delimiter_len)) = next_sse_frame_boundary(buffer) {
        let frame = buffer[..index].to_owned();
        buffer.drain(..index + delimiter_len);
        let data = sse_frame_data(&frame);
        if data.is_empty() {
            continue;
        }
        let data = data.join("\n");
        if data.trim() == "[DONE]" {
            *finished = true;
            events.push(Ok(ProviderChatStreamEvent::Completed(
                ProviderChatResponse {
                    message: accumulator.message(),
                },
            )));
            continue;
        }
        let chunk = match serde_json::from_str::<ChatCompletionChunk>(&data) {
            Ok(chunk) => chunk,
            Err(error) => {
                events.push(Err(AgentError::Provider(error.to_string())));
                continue;
            }
        };
        for choice in chunk.choices {
            let mut delta = choice.delta;
            if delta.finish_reason.is_none() {
                delta.finish_reason = choice.finish_reason;
            }
            accumulator.apply(&delta);
            events.push(Ok(ProviderChatStreamEvent::Delta(delta)));
        }
    }
    events
}

fn sse_frame_data(frame: &str) -> Vec<String> {
    frame
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(|value| value.trim_start().to_owned())
        .collect()
}

fn next_sse_frame_boundary(buffer: &str) -> Option<(usize, usize)> {
    let lf = buffer.find("\n\n").map(|index| (index, 2));
    let crlf = buffer.find("\r\n\r\n").map(|index| (index, 4));
    match (lf, crlf) {
        (Some(lf), Some(crlf)) => Some(lf.min(crlf)),
        (Some(boundary), None) | (None, Some(boundary)) => Some(boundary),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_config_rejects_empty_settings_fields() {
        let error =
            ProviderConfig::new("https://api.openai.com/v1", " ", "gpt-5.1", false).unwrap_err();
        assert!(error.to_string().contains("llm.api_key is required"));
    }

    #[test]
    fn provider_config_trims_settings() {
        let config = ProviderConfig::new(
            " https://api.openai.com/v1 ",
            " sk-test ",
            " gpt-5.1 ",
            true,
        )
        .unwrap();
        assert_eq!(config.base_url, "https://api.openai.com/v1");
        assert_eq!(config.api_key, "sk-test");
        assert_eq!(config.model, "gpt-5.1");
        assert!(config.stream);
    }

    #[test]
    fn provider_config_builds_from_openai_compatible_settings() {
        let llm_config = LlmConfig::new(" sk-test ".to_owned())
            .with_base_url(" https://example.test/v1/chat/completions ".to_owned());
        let config = ProviderConfig::from_llm_config(&llm_config, " gpt-5.1 ", true).unwrap();

        assert_eq!(config.base_url, "https://example.test/v1/chat/completions");
        assert_eq!(config.api_key, "sk-test");
        assert_eq!(config.model, "gpt-5.1");
        assert!(config.stream);
        assert_eq!(config.proxy_url(), None);
    }

    #[test]
    fn provider_endpoint_uses_configured_url_without_appending_path() {
        let config = ProviderConfig::new(
            "https://example.test/custom/completions",
            "sk-test",
            "gpt-5.1",
            false,
        )
        .unwrap();
        let provider = OpenAiCompatibleProvider::new(config).unwrap();

        assert_eq!(
            provider.endpoint(),
            "https://example.test/custom/completions"
        );
    }

    #[test]
    fn provider_config_requires_base_url_from_llm_settings() {
        let llm_config = LlmConfig::new("sk-test".to_owned());
        let error = ProviderConfig::from_llm_config(&llm_config, "gpt-5.1", false).unwrap_err();

        assert!(error.to_string().contains("llm.base_url is required"));
    }

    #[test]
    fn provider_config_requires_api_key_from_llm_settings() {
        let llm_config = LlmConfig::default().with_base_url("https://example.test/v1".to_owned());
        let error = ProviderConfig::from_llm_config(&llm_config, "gpt-5.1", false).unwrap_err();

        assert!(error.to_string().contains("llm.api_key is required"));
    }

    #[test]
    fn provider_config_preserves_proxy_settings() {
        let llm_config = LlmConfig::new("sk-test".to_owned())
            .with_base_url("https://example.test/v1".to_owned())
            .with_proxy("127.0.0.1".to_owned(), 7890);
        let config = ProviderConfig::from_llm_config(&llm_config, "model", false).unwrap();

        assert!(config.proxy_enabled);
        assert_eq!(config.proxy_url().as_deref(), Some("http://127.0.0.1:7890"));
        assert_eq!(config.base_url, "https://example.test/v1");
    }

    #[test]
    fn parses_streamed_content_into_completed_message() {
        let mut buffer = concat!(
            "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"hel\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"lo\"},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n",
        )
        .to_owned();
        let mut accumulator = ChatMessageAccumulator::default();
        let mut finished = false;

        let events = parse_chat_completion_sse_events(&mut buffer, &mut accumulator, &mut finished);

        assert!(finished);
        assert!(buffer.is_empty());
        let Ok(ProviderChatStreamEvent::Completed(response)) = &events[3] else {
            panic!("expected completed stream event");
        };
        assert_eq!(response.message.role, "assistant");
        assert_eq!(response.message.content.as_deref(), Some("hello"));
        assert!(response.message.tool_calls.is_empty());
    }

    #[test]
    fn parses_streamed_tool_call_arguments_into_completed_message() {
        let mut buffer = concat!(
            "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"tool_calls\":[",
            "{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"get_table\",\"arguments\":\"{\\\"schema\\\"\"}}",
            "]}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[",
            "{\"index\":0,\"function\":{\"arguments\":\":\\\"public\\\",\\\"table\\\":\\\"users\\\"}\"}}",
            "]},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: [DONE]\n\n",
        )
        .to_owned();
        let mut accumulator = ChatMessageAccumulator::default();
        let mut finished = false;

        let events = parse_chat_completion_sse_events(&mut buffer, &mut accumulator, &mut finished);

        assert!(finished);
        let Ok(ProviderChatStreamEvent::Completed(response)) = events.last().unwrap() else {
            panic!("expected completed stream event");
        };
        let tool_call = &response.message.tool_calls[0];
        assert_eq!(tool_call.id, "call_1");
        assert_eq!(tool_call.r#type, "function");
        assert_eq!(tool_call.function.name, "get_table");
        assert_eq!(
            tool_call.function.arguments,
            "{\"schema\":\"public\",\"table\":\"users\"}"
        );
    }

    #[test]
    fn parses_crlf_sse_frames() {
        let mut buffer = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\r\n\r\n",
            "data: [DONE]\r\n\r\n",
        )
        .to_owned();
        let mut accumulator = ChatMessageAccumulator::default();
        let mut finished = false;

        let events = parse_chat_completion_sse_events(&mut buffer, &mut accumulator, &mut finished);

        assert!(finished);
        assert!(buffer.is_empty());
        let Ok(ProviderChatStreamEvent::Completed(response)) = events.last().unwrap() else {
            panic!("expected completed stream event");
        };
        assert_eq!(response.message.content.as_deref(), Some("ok"));
    }
}
