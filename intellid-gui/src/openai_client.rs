use async_openai::Client as OpenAiClient;
use async_openai::types::{ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs, ChatCompletionTool, ChatCompletionToolType, FunctionObject};
use serde_json::json;

pub async fn chat_once(api_key: String, model: String, prompt: String) -> Result<String, String> {
    let client = OpenAiClient::with_config(async_openai::config::OpenAIConfig::new().with_api_key(api_key));
    let tools = vec![
        ChatCompletionTool {
            r#type: ChatCompletionToolType::Function,
            function: FunctionObject {
                name: "introspect_all".to_string(),
                description: Some("Introspect database schema via MCP".to_string()),
                strict: None,
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "connectionId": {"type": "string"},
                        "schemas": {"type": "array", "items": {"type": "string"}},
                        "withIndexes": {"type": "boolean"},
                        "withRoutines": {"type": "boolean"},
                        "withTypes": {"type": "boolean"},
                        "withTriggers": {"type": "boolean"},
                        "format": {"type": "string"}
                    },
                    "required": ["connectionId"]
                })),
            }
        }
    ];
    let req = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages([
            ChatCompletionRequestMessage::System(
                ChatCompletionRequestSystemMessageArgs::default().content("You are a helpful assistant.").build().map_err(|e| e.to_string())?
            ),
            ChatCompletionRequestMessage::User(
                ChatCompletionRequestUserMessageArgs::default().content(prompt).build().map_err(|e| e.to_string())?
            )
        ])
        .tools(tools)
        .build().map_err(|e| e.to_string())?;
    let resp = client.chat().create(req).await.map_err(|e| e.to_string())?;
    let text = resp.choices.get(0).and_then(|c| c.message.content.clone()).unwrap_or_default();
    Ok(text)
}

pub async fn chat_with_tools(api_key: String, model: String, prompt: String) -> Result<String, String> {
    let client = OpenAiClient::with_config(async_openai::config::OpenAIConfig::new().with_api_key(api_key));
    let tools = vec![
        ChatCompletionTool {
            r#type: ChatCompletionToolType::Function,
            function: FunctionObject {
                name: "introspect_all".to_string(),
                description: Some("Introspect database schema via MCP".to_string()),
                strict: None,
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "connectionId": {"type": "string"},
                        "schemas": {"type": "array", "items": {"type": "string"}},
                        "withIndexes": {"type": "boolean"},
                        "withRoutines": {"type": "boolean"},
                        "withTypes": {"type": "boolean"},
                        "withTriggers": {"type": "boolean"},
                        "format": {"type": "string"}
                    },
                    "required": ["connectionId"]
                })),
            }
        }
    ];
    let req = CreateChatCompletionRequestArgs::default()
        .model(model.clone())
        .messages([
            ChatCompletionRequestMessage::System(
                ChatCompletionRequestSystemMessageArgs::default().content("You are a helpful assistant.").build().map_err(|e| e.to_string())?
            ),
            ChatCompletionRequestMessage::User(
                ChatCompletionRequestUserMessageArgs::default().content(prompt).build().map_err(|e| e.to_string())?
            )
        ])
        .tools(tools)
        .build().map_err(|e| e.to_string())?;
    let resp = client.chat().create(req).await.map_err(|e| e.to_string())?;
    if let Some(choice) = resp.choices.get(0) {
        if let Some(tcalls) = &choice.message.tool_calls {
            // Execute first supported tool call via MCP
            if let Some(tc) = tcalls.get(0) {
                let function = &tc.function;
                if function.name == "introspect_all" {
                    let args_str = function.arguments.clone();
                    let params: serde_json::Value = serde_json::from_str(&args_str).unwrap_or(json!({}));
                    let cli = crate::mcp_client::McpClient::spawn_with_default().await.map_err(|e| e.to_string())?;
                    let v = cli.call("introspect_all", params).await.map_err(|e| e.to_string())?;
                    if let Some(md) = v.get("markdown").and_then(|x| x.as_str()) { return Ok(md.to_string()); }
                    return Ok(v.to_string());
                }
            }
        }
        return Ok(choice.message.content.clone().unwrap_or_default());
    }
    Ok(String::new())
}


