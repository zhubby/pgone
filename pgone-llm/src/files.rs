use crate::{Client, LlmError, Result};
use async_openai::types::{CreateFileRequestArgs, FilePurpose};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub id: String,
    pub object: String,
    pub bytes: u64,
    pub created_at: u64,
    pub filename: String,
    pub purpose: String,
}

#[derive(Debug, Clone)]
pub struct FileUploadRequest {
    pub file: String,
    pub purpose: String,
}

impl FileUploadRequest {
    pub fn new(file: String, purpose: String) -> Self {
        Self { file, purpose }
    }
}

fn parse_purpose(purpose: &str) -> FilePurpose {
    match purpose {
        "fine-tune" => FilePurpose::FineTune,
        "assistants" => FilePurpose::Assistants,
        "batch" => FilePurpose::Batch,
        _ => FilePurpose::FineTune,
    }
}

impl Client {
    pub async fn files_upload(&self, request: FileUploadRequest) -> Result<FileInfo> {
        let req = CreateFileRequestArgs::default()
            .file(Path::new(&request.file))
            .purpose(parse_purpose(&request.purpose))
            .build()
            .map_err(|e| LlmError::InvalidRequest(e.to_string()))?;

        let resp = self.inner().files().create(req).await?;

        Ok(FileInfo {
            id: resp.id,
            object: resp.object,
            bytes: resp.bytes as u64,
            created_at: resp.created_at as u64,
            filename: resp.filename,
            purpose: format!("{:?}", resp.purpose),
        })
    }

    pub async fn files_list(&self) -> Result<Vec<FileInfo>> {
        let resp = self.inner().files().list(&()).await?;

        Ok(resp.data.into_iter().map(|f| FileInfo {
            id: f.id,
            object: f.object,
            bytes: f.bytes as u64,
            created_at: f.created_at as u64,
            filename: f.filename,
            purpose: format!("{:?}", f.purpose),
        }).collect())
    }

    pub async fn files_retrieve(&self, file_id: String) -> Result<FileInfo> {
        let resp = self.inner().files().retrieve(&file_id).await?;

        Ok(FileInfo {
            id: resp.id,
            object: resp.object,
            bytes: resp.bytes as u64,
            created_at: resp.created_at as u64,
            filename: resp.filename,
            purpose: format!("{:?}", resp.purpose),
        })
    }

    pub async fn files_delete(&self, file_id: String) -> Result<bool> {
        let resp = self.inner().files().delete(&file_id).await?;
        Ok(resp.deleted)
    }

    pub async fn files_retrieve_content(&self, _file_id: String) -> Result<Vec<u8>> {
        // In async-openai 0.30, file content is retrieved differently
        // This is a placeholder - may need to use a different API endpoint
        Err(LlmError::Api("File content retrieval not yet implemented for async-openai 0.30".to_string()))
    }
}

