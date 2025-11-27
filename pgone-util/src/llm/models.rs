use crate::llm::{Client, Result};

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub owned_by: String,
}

impl Client {
    pub async fn models_list(&self) -> Result<Vec<ModelInfo>> {
        let resp = self.inner().models().list().await?;

        Ok(resp.data.into_iter().map(|m| ModelInfo {
            id: m.id,
            object: m.object,
            created: m.created as u64,
            owned_by: m.owned_by,
        }).collect())
    }

    pub async fn models_retrieve(&self, model_id: String) -> Result<ModelInfo> {
        let resp = self.inner().models().retrieve(&model_id).await?;

        Ok(ModelInfo {
            id: resp.id,
            object: resp.object,
            created: resp.created as u64,
            owned_by: resp.owned_by,
        })
    }
}

