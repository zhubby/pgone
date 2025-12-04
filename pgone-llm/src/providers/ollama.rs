use crate::chat::{ChatRequest, ChatResponse, ChatRole, ChatMessageContent};
use crate::{LlmError, Result};
use serde::{Deserialize, Serialize};
use futures::Stream;
use async_openai::types::CreateChatCompletionStreamResponse;

/// Ollama 客户端实现
pub struct OllamaClient {
    base_url: String,
    http_client: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    options: Option<OllamaOptions>,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    #[allow(dead_code)]
    model: String,
    message: OllamaResponseMessage,
    #[allow(dead_code)]
    done: bool,
    #[serde(rename = "done_reason")]
    done_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaResponseMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OllamaStreamResponse {
    model: String,
    message: OllamaStreamMessage,
    done: bool,
    #[serde(rename = "done_reason")]
    done_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OllamaStreamMessage {
    role: String,
    content: String,
}

impl OllamaClient {
    pub fn new(base_url: Option<String>) -> crate::Result<Self> {
        let base_url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
        let http_client = reqwest::Client::builder().no_proxy()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| LlmError::Api(format!("Failed to create HTTP client: {}", e)))?;
        
        Ok(Self {
            base_url,
            http_client,
        })
    }

    /// 将 ChatRequest 转换为 Ollama API 调用
    pub async fn chat_create(&self, request: ChatRequest) -> Result<ChatResponse> {
        // 转换消息格式
        let mut ollama_messages = Vec::new();
        
        for msg in &request.messages {
            // 提取文本内容
            let mut content_parts = Vec::new();
            for content in &msg.content {
                match content {
                    ChatMessageContent::Text(text) => {
                        content_parts.push(text.clone());
                    }
                    ChatMessageContent::ImageUrl { url: _ } => {
                        return Err(LlmError::Api("Ollama does not support image URLs in current implementation".to_string()));
                    }
                    ChatMessageContent::ImageBase64 { data: _ } => {
                        return Err(LlmError::Api("Ollama does not support base64 images in current implementation".to_string()));
                    }
                }
            }
            
            let content = content_parts.join("\n");
            if content.is_empty() {
                continue;
            }
            
            let role = match msg.role {
                ChatRole::System => "system".to_string(),
                ChatRole::User => "user".to_string(),
                ChatRole::Assistant => "assistant".to_string(),
                ChatRole::Function => {
                    return Err(LlmError::Api("Ollama does not support function calls".to_string()));
                }
            };
            
            ollama_messages.push(OllamaMessage {
                role,
                content,
            });
        }

        // 构建选项
        let mut options = OllamaOptions {
            temperature: None,
            top_p: None,
            num_predict: None,
        };
        
        if let Some(temp) = request.temperature {
            options.temperature = Some(temp);
        }
        if let Some(top_p) = request.top_p {
            options.top_p = Some(top_p);
        }
        if let Some(max_tokens) = request.max_tokens {
            options.num_predict = Some(max_tokens);
        }

        // 构建请求
        let ollama_request = OllamaRequest {
            model: request.model.clone(),
            messages: ollama_messages,
            stream: false,
            options: Some(options),
        };

        // 发送请求
        let url = format!("{}/api/chat", self.base_url.trim_end_matches('/'));
        let response = self.http_client
            .post(&url)
            .json(&ollama_request)
            .send()
            .await
            .map_err(|e| LlmError::Network(e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(LlmError::Api(format!("Ollama API error ({}): {}", status, error_text)));
        }

        let ollama_response: OllamaResponse = response
            .json()
            .await
            .map_err(|e| LlmError::Api(format!("Failed to parse Ollama response: {}", e)))?;

        // 转换响应
        Ok(ChatResponse {
            content: ollama_response.message.content,
            role: ollama_response.message.role,
            finish_reason: ollama_response.done_reason,
            tool_calls: None,
        })
    }

    /// 流式聊天创建
    pub fn chat_create_stream(
        &self,
        request: ChatRequest,
    ) -> Box<dyn Stream<Item = Result<CreateChatCompletionStreamResponse>> + Send> {
        let base_url = self.base_url.clone();
        let http_client = self.http_client.clone();
        
        Box::new(async_stream::stream! {
            // 转换消息格式
            let mut ollama_messages = Vec::new();
            
            for msg in &request.messages {
                // 提取文本内容
                let mut content_parts = Vec::new();
                for content in &msg.content {
                    match content {
                        ChatMessageContent::Text(text) => {
                            content_parts.push(text.clone());
                        }
                        ChatMessageContent::ImageUrl { url: _ } => {
                            yield Err(LlmError::Api("Ollama does not support image URLs in current implementation".to_string()).into());
                            return;
                        }
                        ChatMessageContent::ImageBase64 { data: _ } => {
                            yield Err(LlmError::Api("Ollama does not support base64 images in current implementation".to_string()).into());
                            return;
                        }
                    }
                }
                
                let content = content_parts.join("\n");
                if content.is_empty() {
                    continue;
                }
                
                let role = match msg.role {
                    ChatRole::System => "system".to_string(),
                    ChatRole::User => "user".to_string(),
                    ChatRole::Assistant => "assistant".to_string(),
                    ChatRole::Function => {
                        yield Err(LlmError::Api("Ollama does not support function calls".to_string()).into());
                        return;
                    }
                };
                
                ollama_messages.push(OllamaMessage {
                    role,
                    content,
                });
            }

            // 构建选项
            let mut options = OllamaOptions {
                temperature: None,
                top_p: None,
                num_predict: None,
            };
            
            if let Some(temp) = request.temperature {
                options.temperature = Some(temp);
            }
            if let Some(top_p) = request.top_p {
                options.top_p = Some(top_p);
            }
            if let Some(max_tokens) = request.max_tokens {
                options.num_predict = Some(max_tokens);
            }

            // 构建请求
            let ollama_request = OllamaRequest {
                model: request.model.clone(),
                messages: ollama_messages,
                stream: true,
                options: Some(options),
            };

            // 发送请求
            let url = format!("{}/api/chat", base_url.trim_end_matches('/'));
            let response = match http_client
                .post(&url)
                .json(&ollama_request)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    yield Err(LlmError::Network(e).into());
                    return;
                }
            };

            if !response.status().is_success() {
                let status = response.status();
                let error_text = match response.text().await {
                    Ok(text) => text,
                    Err(_) => "Unknown error".to_string(),
                };
                yield Err(LlmError::Api(format!("Ollama API error ({}): {}", status, error_text)).into());
                return;
            }

            // 解析 SSE 流
            let mut stream = response.bytes_stream();
            use futures::StreamExt;
            let mut buffer = String::new();
            let mut index = 0u32;

            while let Some(chunk_result) = stream.next().await {
                let chunk = match chunk_result {
                    Ok(c) => c,
                    Err(e) => {
                        yield Err(LlmError::Network(e).into());
                        return;
                    }
                };

                // 将字节转换为字符串
                let text = match String::from_utf8(chunk.to_vec()) {
                    Ok(t) => t,
                    Err(e) => {
                        yield Err(LlmError::Api(format!("Invalid UTF-8 in stream: {}", e)).into());
                        return;
                    }
                };

                buffer.push_str(&text);

                // 处理完整的行
                while let Some(newline_pos) = buffer.find('\n') {
                    let line = buffer[..newline_pos].trim().to_string();
                    buffer = buffer[newline_pos + 1..].to_string();

                    if line.is_empty() {
                        continue;
                    }

                    // 解析 JSON
                    let stream_response: OllamaStreamResponse = match serde_json::from_str(&line) {
                        Ok(r) => r,
                        Err(e) => {
                            yield Err(LlmError::Api(format!("Failed to parse Ollama stream response: {}", e)).into());
                            continue;
                        }
                    };

                    // 转换为 OpenAI 格式
                    let delta_content = stream_response.message.content;
                    let finish_reason = if stream_response.done {
                        stream_response.done_reason.clone().map(|r| {
                            match r.as_str() {
                                "stop" => async_openai::types::FinishReason::Stop,
                                "length" => async_openai::types::FinishReason::Length,
                                _ => async_openai::types::FinishReason::Stop,
                            }
                        })
                    } else {
                        None
                    };

                    // 创建 delta 消息
                    let delta = async_openai::types::ChatCompletionStreamResponseDelta {
                        role: if stream_response.done && finish_reason.is_some() {
                            Some(async_openai::types::Role::Assistant)
                        } else {
                            None
                        },
                        content: Some(delta_content),
                        #[allow(deprecated)]
                        function_call: None,
                        tool_calls: None,
                        refusal: None,
                    };

                    // 创建 OpenAI 兼容的流式响应
                    let choice = async_openai::types::ChatChoiceStream {
                        index: index as u32,
                        delta,
                        finish_reason: finish_reason.clone(),
                        logprobs: None,
                    };

                    let openai_response = CreateChatCompletionStreamResponse {
                        id: format!("chatcmpl-{}", index),
                        object: "chat.completion.chunk".to_string(),
                        created: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs() as u32,
                        model: request.model.clone(),
                        system_fingerprint: None,
                        choices: vec![choice],
                        service_tier: None,
                        usage: None,
                    };

                    index += 1;
                    yield Ok(openai_response);

                    if stream_response.done {
                        return;
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::{ChatMessage, ChatRequest};

    #[test]
    fn test_ollama_client_new_with_default_url() {
        let client = OllamaClient::new(None);
        assert!(client.is_ok(), "应该能够使用默认 URL 创建客户端");
    }

    #[test]
    fn test_ollama_client_new_with_custom_url() {
        let client = OllamaClient::new(Some("http://localhost:11434".to_string()));
        assert!(client.is_ok(), "应该能够使用自定义 URL 创建客户端");
    }

    #[tokio::test]
    #[ignore] // 需要运行中的 Ollama 服务，默认忽略
    async fn test_chat_create_with_simple_request() {
        let client = OllamaClient::new(Some("http://localhost:11434".to_string())).unwrap();
        
        let request = ChatRequest::new("llama2".to_string())
            .with_messages(vec![
                ChatMessage::user("Hello, how are you?".to_string()),
            ]);
        
        let result = client.chat_create(request).await;
        assert!(result.is_ok(), "应该能够成功调用 Ollama API");
        
        let response = result.unwrap();
        assert!(!response.content.is_empty(), "响应内容不应该为空");
        assert_eq!(response.role, "assistant");
    }

    #[tokio::test]
    #[ignore]
    async fn test_chat_create_with_system_prompt() {
        let client = OllamaClient::new(Some("http://localhost:11434".to_string())).unwrap();
        
        let request = ChatRequest::new("llama2".to_string())
            .with_messages(vec![
                ChatMessage::system("You are a helpful assistant.".to_string()),
                ChatMessage::user("What is 2+2?".to_string()),
            ]);
        
        let result = client.chat_create(request).await;
        assert!(result.is_ok(), "应该能够成功调用带系统提示的请求");
        
        let response = result.unwrap();
        assert!(!response.content.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn test_chat_create_with_parameters() {
        let client = OllamaClient::new(Some("http://localhost:11434".to_string())).unwrap();
        
        let request = ChatRequest::new("llama2".to_string())
            .with_messages(vec![
                ChatMessage::user("Tell me a short joke.".to_string()),
            ])
            .with_temperature(0.7)
            .with_top_p(0.9)
            .with_max_tokens(100);
        
        let result = client.chat_create(request).await;
        assert!(result.is_ok(), "应该能够成功调用带参数的请求");
        
        let response = result.unwrap();
        assert!(!response.content.is_empty());
    }

    #[tokio::test]
    async fn test_chat_create_rejects_function_calls() {
        let client = OllamaClient::new(None).unwrap();
        
        let request = ChatRequest::new("llama2".to_string())
            .with_messages(vec![
                ChatMessage::function("test_function".to_string(), "test_content".to_string()),
            ]);
        
        let result = client.chat_create(request).await;
        assert!(result.is_err(), "应该拒绝 Function 消息");
        
        if let Err(e) = result {
            assert!(e.to_string().contains("function calls"), 
                "错误消息应该说明不支持 function calls");
        }
    }

    #[tokio::test]
    async fn test_chat_create_rejects_image_urls() {
        let client = OllamaClient::new(None).unwrap();
        
        let request = ChatRequest::new("llama2".to_string())
            .with_messages(vec![
                ChatMessage::user("test".to_string())
                    .with_image_url("http://example.com/image.jpg".to_string()),
            ]);
        
        let result = client.chat_create(request).await;
        assert!(result.is_err(), "应该拒绝包含图片 URL 的消息");
    }
}

