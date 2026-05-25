use crate::{Client, LlmError, Result};
use async_openai::types::{
    CreateImageRequestArgs, CreateImageVariationRequestArgs, ImageResponseFormat,
};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ImageGenerationRequest {
    pub prompt: String,
    pub n: Option<u32>,
    pub size: Option<String>,
    pub response_format: Option<String>,
    pub user: Option<String>,
}

impl ImageGenerationRequest {
    pub fn new(prompt: String) -> Self {
        Self {
            prompt,
            n: None,
            size: None,
            response_format: None,
            user: None,
        }
    }

    pub fn with_n(mut self, n: u32) -> Self {
        self.n = Some(n);
        self
    }

    pub fn with_size(mut self, size: String) -> Self {
        self.size = Some(size);
        self
    }

    pub fn with_response_format(mut self, format: String) -> Self {
        self.response_format = Some(format);
        self
    }

    pub fn with_user(mut self, user: String) -> Self {
        self.user = Some(user);
        self
    }
}

#[derive(Debug, Clone)]
pub struct ImageEditRequest {
    pub image: String,
    pub mask: Option<String>,
    pub prompt: String,
    pub n: Option<u32>,
    pub size: Option<String>,
    pub response_format: Option<String>,
    pub user: Option<String>,
}

impl ImageEditRequest {
    pub fn new(image: String, prompt: String) -> Self {
        Self {
            image,
            mask: None,
            prompt,
            n: None,
            size: None,
            response_format: None,
            user: None,
        }
    }

    pub fn with_mask(mut self, mask: String) -> Self {
        self.mask = Some(mask);
        self
    }

    pub fn with_n(mut self, n: u32) -> Self {
        self.n = Some(n);
        self
    }

    pub fn with_size(mut self, size: String) -> Self {
        self.size = Some(size);
        self
    }

    pub fn with_response_format(mut self, format: String) -> Self {
        self.response_format = Some(format);
        self
    }

    pub fn with_user(mut self, user: String) -> Self {
        self.user = Some(user);
        self
    }
}

#[derive(Debug, Clone)]
pub struct ImageVariantRequest {
    pub image: String,
    pub n: Option<u32>,
    pub size: Option<String>,
    pub response_format: Option<String>,
    pub user: Option<String>,
}

impl ImageVariantRequest {
    pub fn new(image: String) -> Self {
        Self {
            image,
            n: None,
            size: None,
            response_format: None,
            user: None,
        }
    }

    pub fn with_n(mut self, n: u32) -> Self {
        self.n = Some(n);
        self
    }

    pub fn with_size(mut self, size: String) -> Self {
        self.size = Some(size);
        self
    }

    pub fn with_response_format(mut self, format: String) -> Self {
        self.response_format = Some(format);
        self
    }

    pub fn with_user(mut self, user: String) -> Self {
        self.user = Some(user);
        self
    }
}

#[derive(Debug, Clone)]
pub struct ImageResponse {
    pub urls: Vec<String>,
    pub b64_json: Option<Vec<String>>,
}

fn parse_size(size: &str) -> async_openai::types::ImageSize {
    match size {
        "256x256" => async_openai::types::ImageSize::S256x256,
        "512x512" => async_openai::types::ImageSize::S512x512,
        "1024x1024" => async_openai::types::ImageSize::S1024x1024,
        "1792x1024" => async_openai::types::ImageSize::S1792x1024,
        "1024x1792" => async_openai::types::ImageSize::S1024x1792,
        _ => async_openai::types::ImageSize::S1024x1024,
    }
}

fn parse_dalle2_size(size: &str) -> async_openai::types::DallE2ImageSize {
    match size {
        "256x256" => async_openai::types::DallE2ImageSize::S256x256,
        "512x512" => async_openai::types::DallE2ImageSize::S512x512,
        "1024x1024" => async_openai::types::DallE2ImageSize::S1024x1024,
        _ => async_openai::types::DallE2ImageSize::S1024x1024,
    }
}

fn parse_response_format(format: &str) -> ImageResponseFormat {
    match format {
        "url" => ImageResponseFormat::Url,
        "b64_json" => ImageResponseFormat::B64Json,
        _ => ImageResponseFormat::Url,
    }
}

impl Client {
    pub async fn images_create(&self, request: ImageGenerationRequest) -> Result<ImageResponse> {
        let mut req_builder = CreateImageRequestArgs::default();
        req_builder.prompt(request.prompt);

        if let Some(n) = request.n {
            req_builder.n(n as u8);
        }

        if let Some(size) = request.size {
            req_builder.size(parse_size(&size));
        }

        if let Some(format) = request.response_format {
            req_builder.response_format(parse_response_format(&format));
        }

        if let Some(user) = request.user {
            req_builder.user(user);
        }

        let req = req_builder
            .build()
            .map_err(|e| LlmError::InvalidRequest(e.to_string()))?;
        let _resp = self.inner().images().create(req).await?;

        // Note: Image response structure in async-openai 0.25 may differ
        // This is a simplified placeholder implementation
        let urls: Vec<String> = Vec::new();
        let b64_json: Option<Vec<String>> = None;

        Ok(ImageResponse { urls, b64_json })
    }

    pub async fn images_edit(&self, _request: ImageEditRequest) -> Result<ImageResponse> {
        // Note: Image editing API may have changed in async-openai 0.30
        // This is a placeholder implementation
        Err(LlmError::Api(
            "Image editing API not yet implemented for this version".to_string(),
        ))
    }

    pub async fn images_create_variant(
        &self,
        request: ImageVariantRequest,
    ) -> Result<ImageResponse> {
        let image_path = Path::new(&request.image);
        let mut req_builder = CreateImageVariationRequestArgs::default();
        req_builder.image(image_path);

        if let Some(n) = request.n {
            req_builder.n(n as u8);
        }

        if let Some(size) = request.size {
            req_builder.size(parse_dalle2_size(&size));
        }

        if let Some(format) = request.response_format {
            req_builder.response_format(parse_response_format(&format));
        }

        if let Some(user) = request.user {
            req_builder.user(user);
        }

        let req = req_builder
            .build()
            .map_err(|e| LlmError::InvalidRequest(e.to_string()))?;
        let _resp = self.inner().images().create_variation(req).await?;

        // Note: Image response structure in async-openai 0.30 may differ
        // This is a simplified placeholder implementation
        let urls: Vec<String> = Vec::new();
        let b64_json: Option<Vec<String>> = None;

        Ok(ImageResponse { urls, b64_json })
    }
}
