use crate::{Client, LlmError, Result, LLMProvider};
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
    ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestFunctionMessageArgs,
};
use futures::Stream;
use crate::tools::{Tool, FunctionCall};

#[derive(Debug, Clone)]
pub enum ChatMessageContent {
    Text(String),
    ImageUrl { url: String },
    ImageBase64 { data: String },
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: Vec<ChatMessageContent>,
    pub name: Option<String>,
    pub function_call: Option<FunctionCall>,
}

#[derive(Debug, Clone)]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Function,
}

impl ChatMessage {
    pub fn system(content: String) -> Self {
        Self {
            role: ChatRole::System,
            content: vec![ChatMessageContent::Text(content)],
            name: None,
            function_call: None,
        }
    }

    pub fn user(content: String) -> Self {
        Self {
            role: ChatRole::User,
            content: vec![ChatMessageContent::Text(content)],
            name: None,
            function_call: None,
        }
    }

    pub fn assistant(content: String) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: vec![ChatMessageContent::Text(content)],
            name: None,
            function_call: None,
        }
    }

    pub fn function(name: String, content: String) -> Self {
        Self {
            role: ChatRole::Function,
            content: vec![ChatMessageContent::Text(content)],
            name: Some(name),
            function_call: None,
        }
    }

    pub fn with_image_url(mut self, url: String) -> Self {
        self.content.push(ChatMessageContent::ImageUrl { url });
        self
    }

    pub fn with_image_base64(mut self, data: String) -> Self {
        self.content.push(ChatMessageContent::ImageBase64 { data });
        self
    }
}

#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Option<Vec<Tool>>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stream: bool,
}

impl ChatRequest {
    pub fn new(model: String) -> Self {
        Self {
            model,
            messages: Vec::new(),
            tools: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: false,
        }
    }

    pub fn with_messages(mut self, messages: Vec<ChatMessage>) -> Self {
        self.messages = messages;
        self
    }

    pub fn with_tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = Some(tools);
        self
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p);
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn with_stream(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }
}

#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub content: String,
    pub role: String,
    pub finish_reason: Option<String>,
    pub function_call: Option<FunctionCall>,
}

impl Client {
    pub async fn chat_create(&self, request: ChatRequest) -> Result<ChatResponse> {
        match self.provider() {
            LLMProvider::Gemini => {
                // 使用 Gemini 客户端
                let gemini_client = crate::providers::gemini::GeminiClient::new(
                    self.config().api_key.clone()
                )?;
                gemini_client.chat_create(request).await
            }
            LLMProvider::Ollama => {
                // 使用 Ollama 客户端
                let ollama_client = crate::providers::ollama::OllamaClient::new(
                    self.config().base_url.clone()
                )?;
                ollama_client.chat_create(request).await
            }
            _ => {
                // 使用 OpenAI 兼容的 API
                let messages = self.convert_messages(&request.messages)?;
                let mut req_builder = CreateChatCompletionRequestArgs::default();
                req_builder.model(request.model);
                req_builder.messages(messages);

                if let Some(tools) = request.tools {
                    let tools: Vec<async_openai::types::ChatCompletionTool> = tools
                        .into_iter()
                        .map(|t| async_openai::types::ChatCompletionTool {
                            r#type: async_openai::types::ChatCompletionToolType::Function,
                            function: async_openai::types::FunctionObject {
                                name: t.function.name,
                                description: t.function.description,
                                parameters: Some(t.function.parameters),
                                strict: None,
                            },
                        })
                        .collect();
                    req_builder.tools(tools);
                }

                if let Some(temp) = request.temperature {
                    req_builder.temperature(temp);
                }

                if let Some(top_p) = request.top_p {
                    req_builder.top_p(top_p);
                }

                if let Some(max_tokens) = request.max_tokens {
                    req_builder.max_tokens(max_tokens as u16);
                }

                let req = req_builder.build().map_err(|e| LlmError::InvalidRequest(e.to_string()))?;
                let resp = self.inner().chat().create(req).await?;

                let choice = resp.choices.first().ok_or_else(|| {
                    LlmError::Api("No choices in response".to_string())
                })?;

                let content = choice.message.content.clone().unwrap_or_default();
                let role = choice.message.role.to_string();
                let finish_reason = choice.finish_reason.as_ref().map(|r| format!("{:?}", r));

                let function_call = choice.message.tool_calls.as_ref()
                    .and_then(|calls| calls.first())
                    .map(|call| FunctionCall {
                        name: call.function.name.clone(),
                        arguments: call.function.arguments.clone(),
                    });

                Ok(ChatResponse {
                    content,
                    role,
                    finish_reason,
                    function_call,
                })
            }
        }
    }

    pub fn chat_create_stream(
        &self,
        request: ChatRequest,
    ) -> Box<dyn Stream<Item = Result<async_openai::types::CreateChatCompletionStreamResponse>> + Send> {
        match self.provider() {
            LLMProvider::Ollama => {
                // 使用 Ollama 客户端
                let ollama_client = match crate::providers::ollama::OllamaClient::new(
                    self.config().base_url.clone()
                ) {
                    Ok(c) => c,
                    Err(e) => {
                        let stream: Box<dyn Stream<Item = Result<async_openai::types::CreateChatCompletionStreamResponse>> + Send> = 
                            Box::new(futures::stream::once(futures::future::ready(Err(e.into()))));
                        return stream;
                    }
                };
                ollama_client.chat_create_stream(request)
            }
            _ => {
                // 使用 OpenAI 兼容的 API
                let client = self.inner().clone();
                let messages = match self.convert_messages(&request.messages) {
                    Ok(m) => m,
                    Err(e) => {
                        let stream: Box<dyn Stream<Item = Result<async_openai::types::CreateChatCompletionStreamResponse>> + Send> = 
                            Box::new(futures::stream::once(futures::future::ready(Err(e))));
                        return stream;
                    }
                };
                let mut req_builder = CreateChatCompletionRequestArgs::default();
                req_builder.model(request.model.clone());
                req_builder.messages(messages);

                if let Some(tools) = request.tools {
                    let tools: Vec<async_openai::types::ChatCompletionTool> = tools
                        .into_iter()
                        .map(|t| async_openai::types::ChatCompletionTool {
                            r#type: async_openai::types::ChatCompletionToolType::Function,
                            function: async_openai::types::FunctionObject {
                                name: t.function.name,
                                description: t.function.description,
                                parameters: Some(t.function.parameters),
                                strict: None,
                            },
                        })
                        .collect();
                    req_builder.tools(tools);
                }

                if let Some(temp) = request.temperature {
                    req_builder.temperature(temp);
                }

                if let Some(top_p) = request.top_p {
                    req_builder.top_p(top_p);
                }

                if let Some(max_tokens) = request.max_tokens {
                    req_builder.max_tokens(max_tokens as u16);
                }

                req_builder.stream(true);

                let req = match req_builder.build() {
                    Ok(r) => r,
                    Err(e) => {
                        let stream: Box<dyn Stream<Item = Result<async_openai::types::CreateChatCompletionStreamResponse>> + Send> = 
                            Box::new(futures::stream::once(futures::future::ready(
                                Err(LlmError::InvalidRequest(e.to_string()))
                            )));
                        return stream;
                    }
                };

                Box::new(async_stream::stream! {
                    let req_clone = req;
                    match client.chat().create_stream(req_clone).await {
                        Ok(stream) => {
                            use futures::StreamExt;
                            let mut stream = Box::pin(stream);
                            while let Some(result) = stream.next().await {
                                match result {
                                    Ok(chunk) => yield Ok(chunk),
                                    Err(e) => yield Err(e.into()),
                                }
                            }
                        }
                        Err(e) => yield Err(e.into()),
                    }
                })
            }
        }
    }

    fn convert_messages(
        &self,
        messages: &[ChatMessage],
    ) -> Result<Vec<ChatCompletionRequestMessage>> {
        let mut result = Vec::new();

        for msg in messages {
            let converted = match msg.role {
                ChatRole::System => {
                    let content = msg.content
                        .iter()
                        .find_map(|c| match c {
                            ChatMessageContent::Text(t) => Some(t.clone()),
                            _ => None,
                        })
                        .ok_or_else(|| LlmError::InvalidRequest("System message must have text content".to_string()))?;
                    
                    ChatCompletionRequestMessage::System(
                        ChatCompletionRequestSystemMessageArgs::default()
                            .content(content)
                            .build()
                            .map_err(|e| LlmError::InvalidRequest(e.to_string()))?,
                    )
                }
                ChatRole::User => {
                    if msg.content.len() == 1 {
                        if let ChatMessageContent::Text(text) = &msg.content[0] {
                            ChatCompletionRequestMessage::User(
                                ChatCompletionRequestUserMessageArgs::default()
                                    .content(text.clone())
                                    .build()
                                    .map_err(|e| LlmError::InvalidRequest(e.to_string()))?,
                            )
                        } else {
                            return Err(LlmError::InvalidRequest("Multi-modal user messages not yet fully supported".to_string()));
                        }
                    } else {
                        return Err(LlmError::InvalidRequest("Multi-modal user messages not yet fully supported".to_string()));
                    }
                }
                ChatRole::Assistant => {
                    let content = msg.content
                        .iter()
                        .find_map(|c| match c {
                            ChatMessageContent::Text(t) => Some(t.clone()),
                            _ => None,
                        })
                        .unwrap_or_default();
                    
                    ChatCompletionRequestMessage::Assistant(
                        ChatCompletionRequestAssistantMessageArgs::default()
                            .content(content)
                            .build()
                            .map_err(|e| LlmError::InvalidRequest(e.to_string()))?,
                    )
                }
                ChatRole::Function => {
                    let name = msg.name.as_ref().ok_or_else(|| {
                        LlmError::InvalidRequest("Function message must have a name".to_string())
                    })?;
                    let content = msg.content
                        .iter()
                        .find_map(|c| match c {
                            ChatMessageContent::Text(t) => Some(t.clone()),
                            _ => None,
                        })
                        .ok_or_else(|| LlmError::InvalidRequest("Function message must have text content".to_string()))?;
                    
                    ChatCompletionRequestMessage::Function(
                        ChatCompletionRequestFunctionMessageArgs::default()
                            .name(name.clone())
                            .content(content)
                            .build()
                            .map_err(|e| LlmError::InvalidRequest(e.to_string()))?,
                    )
                }
            };
            result.push(converted);
        }

        Ok(result)
    }
}

