# pgone-llm

LLM client library for the PGone project, providing a unified interface to access multiple LLM providers.

## Feature Modules

- **chat**: Chat conversation functionality
- **models**: Model listing and querying
- **embeddings**: Text embedding vectors
- **images**: Image generation and editing
- **audio**: Audio transcription and speech synthesis
- **files**: File upload and management
- **tools**: Function calling (Function Calling)
- **audit**: Request audit logging

## Provider Support Status

### OpenAI (Default)

Official OpenAI API, implemented via the `async-openai` library.

| Feature | Status | Notes |
|---------|--------|-------|
| Chat | ✅ | Full support, including streaming responses |
| Chat Stream | ✅ | Supports Server-Sent Events (SSE) streaming responses |
| Models | ✅ | Supports model listing and querying |
| Embeddings | ✅ | Supports text embedding vector generation |
| Images | ✅ | Supports image generation, editing, and variations |
| Audio | ⚠️ | Placeholder implementation, API not fully adapted |
| Files | ✅ | Supports file upload, listing, querying, and deletion |
| Tools | ✅ | Supports function calling (Function Calling) |

**Configuration Example**:
```rust
use pgone_llm::{Client, Config, LLMProvider};

let config = Config::new("sk-...".to_string());
let client = Client::new(config, LLMProvider::OpenAI)?;
```

### Gemini

Google Gemini API, implemented via custom implementation.

| Feature | Status | Notes |
|---------|--------|-------|
| Chat | ✅ | Supports basic chat functionality |
| Chat Stream | ❌ | Not implemented |
| Models | ❌ | Not implemented |
| Embeddings | ❌ | Not implemented |
| Images | ❌ | Not implemented |
| Audio | ❌ | Not implemented |
| Files | ❌ | Not implemented |
| Tools | ❌ | Function calling not supported |

**Limitations**:
- Function calling (Function Calls) not supported
- Streaming responses not supported
- Only text messages supported, images not supported

**Configuration Example**:
```rust
use pgone_llm::{Client, Config, LLMProvider};

let config = Config::new("your-gemini-api-key".to_string());
let client = Client::new(config, LLMProvider::Gemini)?;
```

### Ollama

Local Ollama service, implemented via custom implementation.

| Feature | Status | Notes |
|---------|--------|-------|
| Chat | ✅ | Supports basic chat functionality |
| Chat Stream | ✅ | Supports streaming responses |
| Models | ✅ | Supports model listing and querying |
| Embeddings | ❌ | Not implemented |
| Images | ❌ | Image URLs or base64 images not supported |
| Audio | ❌ | Not implemented |
| Files | ❌ | Not implemented |
| Tools | ✅ | Supports function calling (Function Calling) |

**Limitations**:
- Image messages (ImageUrl and ImageBase64) not supported
- Default connection address: `http://localhost:11434`
- Custom address can be configured via `base_url`

**Configuration Example**:
```rust
use pgone_llm::{Client, Config, LLMProvider};

let config = Config::new("ollama".to_string())
    .with_base_url("http://localhost:11434".to_string());
let client = Client::new(config, LLMProvider::Ollama)?;
```

### OpenRouter

OpenRouter unified API, supporting access to multiple LLM providers.

| Feature | Status | Notes |
|---------|--------|-------|
| Chat | ✅ | Via OpenAI-compatible API |
| Chat Stream | ✅ | Via OpenAI-compatible API |
| Models | ✅ | Custom implementation, supports 400+ models |
| Embeddings | ✅ | Via OpenAI-compatible API |
| Images | ✅ | Via OpenAI-compatible API |
| Audio | ⚠️ | Placeholder implementation, API not fully adapted |
| Files | ✅ | Via OpenAI-compatible API |
| Tools | ✅ | Via OpenAI-compatible API |

**Features**:
- Supports access to 400+ models (GPT-4, Claude, Gemini, Llama, etc.)
- Unified API interface, no need to configure each provider separately
- Default API address: `https://openrouter.ai`

**Configuration Example**:
```rust
use pgone_llm::{Client, Config, LLMProvider};

let config = Config::new("sk-or-...".to_string())
    .with_base_url("https://openrouter.ai".to_string());
let client = Client::new(config, LLMProvider::OpenRouter)?;
```

### Moonshot / DeepSeek / BigModel

These providers are implemented via OpenAI-compatible API.

| Feature | Status | Notes |
|---------|--------|-------|
| Chat | ✅ | Via OpenAI-compatible API |
| Chat Stream | ✅ | Via OpenAI-compatible API |
| Models | ✅ | Via OpenAI-compatible API |
| Embeddings | ✅ | Via OpenAI-compatible API |
| Images | ✅ | Via OpenAI-compatible API |
| Audio | ⚠️ | Placeholder implementation, API not fully adapted |
| Files | ✅ | Via OpenAI-compatible API |
| Tools | ✅ | Via OpenAI-compatible API |

**Configuration Example**:
```rust
use pgone_llm::{Client, Config, LLMProvider};

// Moonshot
let config = Config::new("sk-...".to_string())
    .with_base_url("https://api.moonshot.cn/v1".to_string());
let client = Client::new(config, LLMProvider::Moonshot)?;

// DeepSeek
let config = Config::new("sk-...".to_string())
    .with_base_url("https://api.deepseek.com/v1".to_string());
let client = Client::new(config, LLMProvider::DeepSeek)?;
```

## Usage Examples

### Chat Conversation

```rust
use pgone_llm::{Client, Config, LLMProvider};
use pgone_llm::chat::{ChatRequest, ChatMessage};

let client = Client::new(Config::new("api-key".to_string()), LLMProvider::OpenAI)?;

let request = ChatRequest::new("gpt-4".to_string())
    .with_messages(vec![
        ChatMessage::user("Hello, world!".to_string()),
    ])
    .with_temperature(0.7);

let response = client.chat_create(request).await?;
println!("Response: {}", response.content);
```

### Streaming Response

```rust
use futures::StreamExt;

let request = ChatRequest::new("gpt-4".to_string())
    .with_messages(vec![
        ChatMessage::user("Tell me a story".to_string()),
    ]);

let mut stream = client.chat_create_stream(request);
while let Some(chunk) = stream.next().await {
    match chunk {
        Ok(response) => {
            // Handle streaming response
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            break;
        }
    }
}
```

### Model Listing

```rust
let models = client.models_list().await?;
for model in models {
    println!("Model: {} (owned by: {})", model.id, model.owned_by);
}
```

### Text Embeddings

```rust
use pgone_llm::embeddings::EmbeddingRequest;

let request = EmbeddingRequest::new(
    "text-embedding-ada-002".to_string(),
    vec!["Hello, world!".to_string()],
);

let response = client.embeddings_create(request).await?;
println!("Embedding dimensions: {}", response.embeddings[0].len());
```

### Function Calling

```rust
use pgone_llm::tools::{Tool, Function};

let function = Function::new("get_weather".to_string())
    .with_description("Get the current weather".to_string())
    .with_parameters(serde_json::json!({
        "type": "object",
        "properties": {
            "location": {
                "type": "string",
                "description": "The city and state"
            }
        },
        "required": ["location"]
    }));

let tool = Tool::new(function);
let request = ChatRequest::new("gpt-4".to_string())
    .with_messages(vec![
        ChatMessage::user("What's the weather in Beijing?".to_string()),
    ])
    .with_tools(Some(vec![tool]));

let response = client.chat_create(request).await?;
if let Some(tool_calls) = response.tool_calls {
    for call in tool_calls {
        println!("Function: {}, Arguments: {}", call.name, call.arguments);
    }
}
```

## Configuration Options

The `Config` struct supports the following options:

- `api_key`: API key (required)
- `base_url`: Custom API base URL
- `timeout`: Request timeout (default: 60 seconds)
- `max_retries`: Maximum retry count (default: 3)
- `default_model`: Default model name
- `default_temperature`: Default temperature parameter
- `default_top_p`: Default top_p parameter
- `default_max_tokens`: Default maximum token count
- `proxy_enabled`: Whether to enable proxy
- `proxy_host`: Proxy host
- `proxy_port`: Proxy port

## Error Handling

All API calls return `Result<T, LlmError>`, with error types including:

- `Network`: Network errors
- `Api`: API errors
- `Parse`: JSON parsing errors
- `Config`: Configuration errors
- `InvalidApiKey`: Invalid API key
- `InvalidModel`: Invalid model
- `InvalidRequest`: Invalid request
- `Stream`: Streaming response errors
- `File`: File operation errors
- `Unknown`: Unknown errors

## Audit Logging

All requests and responses are automatically logged to audit logs, including:

- Request ID
- Session ID
- Provider type
- Model name
- Request content
- Response content
- Error information
- Timestamp

## Dependencies

Main dependencies:

- `async-openai`: OpenAI API client
- `reqwest`: HTTP client
- `serde`: Serialization/deserialization
- `tokio`: Async runtime
- `futures`: Async stream processing

## License

See the LICENSE file in the project root directory.
