use crate::llm::{Client, LlmError, Result};
use async_openai::types::ResponseFormat;

#[derive(Debug, Clone)]
pub struct TranscriptionRequest {
    pub file: String,
    pub model: String,
    pub language: Option<String>,
    pub prompt: Option<String>,
    pub response_format: Option<String>,
    pub temperature: Option<f32>,
}

impl TranscriptionRequest {
    pub fn new(file: String, model: String) -> Self {
        Self {
            file,
            model,
            language: None,
            prompt: None,
            response_format: None,
            temperature: None,
        }
    }

    pub fn with_language(mut self, language: String) -> Self {
        self.language = Some(language);
        self
    }

    pub fn with_prompt(mut self, prompt: String) -> Self {
        self.prompt = Some(prompt);
        self
    }

    pub fn with_response_format(mut self, format: String) -> Self {
        self.response_format = Some(format);
        self
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }
}

#[derive(Debug, Clone)]
pub struct TranslationRequest {
    pub file: String,
    pub model: String,
    pub prompt: Option<String>,
    pub response_format: Option<String>,
    pub temperature: Option<f32>,
}

impl TranslationRequest {
    pub fn new(file: String, model: String) -> Self {
        Self {
            file,
            model,
            prompt: None,
            response_format: None,
            temperature: None,
        }
    }

    pub fn with_prompt(mut self, prompt: String) -> Self {
        self.prompt = Some(prompt);
        self
    }

    pub fn with_response_format(mut self, format: String) -> Self {
        self.response_format = Some(format);
        self
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }
}

#[derive(Debug, Clone)]
pub struct SpeechRequest {
    pub input: String,
    pub model: String,
    pub voice: Option<String>,
    pub response_format: Option<String>,
    pub speed: Option<f32>,
}

impl SpeechRequest {
    pub fn new(input: String, model: String) -> Self {
        Self {
            input,
            model,
            voice: None,
            response_format: None,
            speed: None,
        }
    }

    pub fn with_voice(mut self, voice: String) -> Self {
        self.voice = Some(voice);
        self
    }

    pub fn with_response_format(mut self, format: String) -> Self {
        self.response_format = Some(format);
        self
    }

    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = Some(speed);
        self
    }
}

fn parse_response_format(_format: &str) -> ResponseFormat {
    // In async-openai 0.30, ResponseFormat enum may have changed
    // This function is currently unused as audio APIs are placeholders
    // Always return Text format (the only available variant in 0.30)
    ResponseFormat::Text
}

fn parse_voice(voice: &str) -> String {
    // Validate voice name
    match voice {
        "alloy" | "echo" | "fable" | "onyx" | "nova" | "shimmer" => voice.to_string(),
        _ => "alloy".to_string(),
    }
}

impl Client {
    pub async fn audio_transcribe(&self, request: TranscriptionRequest) -> Result<String> {
        // Note: Audio transcription API may have changed in async-openai 0.25
        // This is a placeholder implementation
        let _ = request;
        Err(LlmError::Api("Audio transcription API not yet implemented for this version".to_string()))
    }

    pub async fn audio_translate(&self, request: TranslationRequest) -> Result<String> {
        // Note: Audio translation API may have changed in async-openai 0.25
        // This is a placeholder implementation
        let _ = request;
        Err(LlmError::Api("Audio translation API not yet implemented for this version".to_string()))
    }

    pub async fn audio_speech(&self, request: SpeechRequest) -> Result<Vec<u8>> {
        // Note: Audio speech API may have changed in async-openai 0.25
        // This is a placeholder implementation
        let _ = request;
        Err(LlmError::Api("Audio speech API not yet implemented for this version".to_string()))
    }
}

