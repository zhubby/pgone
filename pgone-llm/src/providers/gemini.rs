use crate::chat::{ChatRequest, ChatResponse, ChatRole, ChatMessageContent};
use crate::{LlmError, Result};
use gemini_rust::Gemini;

/// Gemini 客户端实现
pub struct GeminiClient {
    inner: Gemini,
}

impl GeminiClient {
    pub fn new(api_key: String) -> crate::Result<Self> {
        let gemini = Gemini::new(api_key)
            .map_err(|e| LlmError::Api(format!("Failed to create Gemini client: {}", e)))?;
        Ok(Self { inner: gemini })
    }

    /// 将 ChatRequest 转换为 Gemini API 调用
    pub async fn chat_create(&self, request: ChatRequest) -> Result<ChatResponse> {
        // 提取系统消息和用户消息
        let mut system_prompt = String::new();
        let mut user_prompt = String::new();
        
        for msg in &request.messages {
            match &msg.role {
                ChatRole::System => {
                    // 提取系统消息的文本内容
                    for content in &msg.content {
                        if let ChatMessageContent::Text(text) = content {
                            if !system_prompt.is_empty() {
                                system_prompt.push_str("\n");
                            }
                            system_prompt.push_str(text);
                        }
                    }
                }
                ChatRole::User => {
                    // 提取用户消息的文本内容
                    for content in &msg.content {
                        if let ChatMessageContent::Text(text) = content {
                            if !user_prompt.is_empty() {
                                user_prompt.push_str("\n");
                            }
                            user_prompt.push_str(text);
                        }
                    }
                }
                ChatRole::Assistant => {
                    // Gemini API 可能需要处理助手消息，但当前实现主要关注用户输入
                    // 可以在这里添加对话历史处理
                }
                ChatRole::Function => {
                    // Function 消息在 Gemini 中可能需要特殊处理
                    return Err(LlmError::Api("Function calls not yet supported for Gemini".to_string()));
                }
            }
        }

        // 调用 Gemini API - 使用 generate_content 构建器
        let mut builder = self.inner.generate_content();
        
        // 设置系统提示（如果有）
        if !system_prompt.is_empty() {
            builder = builder.with_system_prompt(system_prompt);
        }
        
        // 设置用户消息
        builder = builder.with_user_message(user_prompt);
        
        // 设置温度和其他参数（如果请求中有）
        if let Some(temp) = request.temperature {
            builder = builder.with_temperature(temp);
        }
        if let Some(top_p) = request.top_p {
            builder = builder.with_top_p(top_p);
        }
        if let Some(max_tokens) = request.max_tokens {
            builder = builder.with_max_output_tokens(max_tokens as i32);
        }
        
        // 执行请求
        let response = builder
            .execute()
            .await
            .map_err(|e| LlmError::Api(format!("Gemini API error: {}", e)))?;

        // 提取响应文本内容
        let content = response.text();
        
        Ok(ChatResponse {
            content,
            role: "assistant".to_string(),
            finish_reason: Some("stop".to_string()),
            function_call: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::{ChatMessage, ChatRequest};

    #[test]
    fn test_gemini_client_new_with_valid_api_key() {
        // 使用一个有效的 API key 格式（至少不是空字符串）
        let api_key = "test_api_key_12345".to_string();
        let result = GeminiClient::new(api_key);
        assert!(result.is_ok(), "应该能够使用有效的 API key 创建客户端");
    }

    #[test]
    fn test_gemini_client_new_with_empty_api_key() {
        // gemini-rust 可能会拒绝空字符串
        let api_key = String::new();
        let result = GeminiClient::new(api_key);
        // 根据 gemini-rust 的实现，可能会失败
        // 这里我们只测试不会 panic
        let _ = result;
    }

    #[tokio::test]
    #[ignore] // 需要真实的 API key，默认忽略
    async fn test_chat_create_with_simple_request() {
        let api_key = std::env::var("GEMINI_API_KEY")
            .expect("GEMINI_API_KEY environment variable must be set for integration tests");
        
        let client = GeminiClient::new(api_key).unwrap();
        
        let request = ChatRequest::new("gemini-pro".to_string())
            .with_messages(vec![
                ChatMessage::user("Hello, how are you?".to_string()),
            ]);
        
        let result = client.chat_create(request).await;
        assert!(result.is_ok(), "应该能够成功调用 Gemini API");
        
        let response = result.unwrap();
        assert!(!response.content.is_empty(), "响应内容不应该为空");
        assert_eq!(response.role, "assistant");
    }

    #[tokio::test]
    #[ignore]
    async fn test_chat_create_with_system_prompt() {
        let api_key = std::env::var("GEMINI_API_KEY")
            .expect("GEMINI_API_KEY environment variable must be set for integration tests");
        
        let client = GeminiClient::new(api_key).unwrap();
        
        let request = ChatRequest::new("gemini-pro".to_string())
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
        let api_key = std::env::var("GEMINI_API_KEY")
            .expect("GEMINI_API_KEY environment variable must be set for integration tests");
        
        let client = GeminiClient::new(api_key).unwrap();
        
        let request = ChatRequest::new("gemini-pro".to_string())
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
        let api_key = "test_api_key".to_string();
        let client = GeminiClient::new(api_key).unwrap();
        
        let request = ChatRequest::new("gemini-pro".to_string())
            .with_messages(vec![
                ChatMessage::function("test_function".to_string(), "test_content".to_string()),
            ]);
        
        let result = client.chat_create(request).await;
        assert!(result.is_err(), "应该拒绝 Function 消息");
        
        if let Err(e) = result {
            assert!(e.to_string().contains("Function calls not yet supported"), 
                "错误消息应该说明不支持 Function calls");
        }
    }

    #[tokio::test]
    async fn test_chat_create_with_multiple_user_messages() {
        let api_key = "test_api_key".to_string();
        let client = GeminiClient::new(api_key).unwrap();
        
        let request = ChatRequest::new("gemini-pro".to_string())
            .with_messages(vec![
                ChatMessage::user("First message".to_string()),
                ChatMessage::user("Second message".to_string()),
            ]);
        
        // 这个测试主要验证不会 panic，实际 API 调用会失败因为没有真实的 key
        // 但我们可以测试消息提取逻辑不会出错
        let _ = client.chat_create(request).await;
    }

    #[tokio::test]
    async fn test_chat_create_with_assistant_message() {
        let api_key = "test_api_key".to_string();
        let client = GeminiClient::new(api_key).unwrap();
        
        let request = ChatRequest::new("gemini-pro".to_string())
            .with_messages(vec![
                ChatMessage::assistant("Previous response".to_string()),
                ChatMessage::user("Follow up question".to_string()),
            ]);
        
        // Assistant 消息应该被忽略（当前实现），但不会导致错误
        let _ = client.chat_create(request).await;
    }

    #[test]
    fn test_message_extraction_logic() {
        // 测试消息提取逻辑（不涉及实际 API 调用）
        let request = ChatRequest::new("gemini-pro".to_string())
            .with_messages(vec![
                ChatMessage::system("System prompt 1".to_string()),
                ChatMessage::system("System prompt 2".to_string()),
                ChatMessage::user("User message 1".to_string()),
                ChatMessage::user("User message 2".to_string()),
            ]);
        
        // 验证消息结构正确
        assert_eq!(request.messages.len(), 4);
        assert!(matches!(request.messages[0].role, ChatRole::System));
        assert!(matches!(request.messages[2].role, ChatRole::User));
    }
}

