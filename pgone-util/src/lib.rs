pub mod log;
pub mod llm;

// Re-export LLM module for convenience
pub use llm::{
    Client, Config, LlmError, Result as LlmResult,
    chat_once, chat_with_tools,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_parsing() {
        use log::LogLevel;
        use std::str::FromStr;

        assert!(LogLevel::from_str("info").is_ok());
        assert!(LogLevel::from_str("DEBUG").is_ok());
        assert!(LogLevel::from_str("trace").is_ok());
        assert!(LogLevel::from_str("invalid").is_err());
    }
}
