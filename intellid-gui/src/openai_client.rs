use async_openai::Client as OpenAiClient;
use async_openai::types::{ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs};

pub async fn chat_once(api_key: String, model: String, prompt: String) -> Result<String, String> {
    let client = OpenAiClient::with_config(async_openai::config::OpenAIConfig::new().with_api_key(api_key));
    let req = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages([
            ChatCompletionRequestMessage::System(
                ChatCompletionRequestSystemMessageArgs::default().content("You are a helpful assistant.").build().map_err(|e| e.to_string())?
            ),
            ChatCompletionRequestMessage::User(
                ChatCompletionRequestUserMessageArgs::default().content(prompt).build().map_err(|e| e.to_string())?
            )
        ]).build().map_err(|e| e.to_string())?;
    let resp = client.chat().create(req).await.map_err(|e| e.to_string())?;
    let text = resp.choices.get(0).and_then(|c| c.message.content.clone()).unwrap_or_default();
    Ok(text)
}


