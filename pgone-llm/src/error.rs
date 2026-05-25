use thiserror::Error;

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("网络错误: {0}")]
    Network(#[from] reqwest::Error),

    #[error("API 错误: {0}")]
    Api(String),

    #[error("解析错误: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("配置错误: {0}")]
    Config(String),

    #[error("无效的 API key")]
    InvalidApiKey,

    #[error("无效的模型: {0}")]
    InvalidModel(String),

    #[error("无效的请求: {0}")]
    InvalidRequest(String),

    #[error("流式响应错误: {0}")]
    Stream(String),

    #[error("文件操作错误: {0}")]
    File(String),

    #[error("未知错误: {0}")]
    Unknown(String),
}

impl From<async_openai::error::OpenAIError> for LlmError {
    fn from(err: async_openai::error::OpenAIError) -> Self {
        // In async-openai 0.30, error structure may have changed
        // Convert to string representation
        let err_str = err.to_string();

        // Try to match common error patterns
        if err_str.contains("json") || err_str.contains("parse") || err_str.contains("deserialize")
        {
            // Try to create a parse error
            match serde_json::from_str::<serde_json::Value>("invalid") {
                Err(e) => LlmError::Parse(e),
                Ok(_) => LlmError::Parse(serde_json::from_str::<String>("").unwrap_err()),
            }
        } else if err_str.contains("network")
            || err_str.contains("connection")
            || err_str.contains("timeout")
        {
            // For network errors, we can't easily create a reqwest::Error from scratch
            // So we'll use Unknown error type instead
            LlmError::Unknown(format!("Network error: {}", err_str))
        } else {
            // Default to API error
            LlmError::Api(err_str)
        }
    }
}

pub type Result<T> = std::result::Result<T, LlmError>;
