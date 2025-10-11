use crate::Storage;
use crate::models::*;
use anyhow::Result;

pub struct StorageBlocking {
    inner: Storage,
}

impl StorageBlocking {
    pub async fn open_local(path: &str) -> Result<Self> {
        let inner = Storage::open_local(path).await?;
        Ok(Self { inner })
    }

    pub async fn upsert_db_config(&self, cfg: &DbConfig) -> Result<()> {
        let mut conn = self.inner.conn().await?;
        crate::storage::upsert_db_config(&mut conn, cfg).await
    }

    pub async fn get_db_config(&self, id: &str) -> Result<Option<DbConfig>> {
        let mut conn = self.inner.conn().await?;
        crate::storage::get_db_config(&mut conn, id).await
    }

    pub async fn list_db_configs(&self, limit: Option<i64>) -> Result<Vec<DbConfig>> {
        let mut conn = self.inner.conn().await?;
        crate::storage::list_db_configs(&mut conn, limit).await
    }

    pub async fn delete_db_config(&self, id: &str) -> Result<()> {
        let mut conn = self.inner.conn().await?;
        crate::storage::delete_db_config(&mut conn, id).await
    }

    pub async fn create_session(&self, s: &Session) -> Result<()> {
        let mut conn = self.inner.conn().await?;
        crate::storage::create_session(&mut conn, s).await
    }

    pub async fn update_session_title(&self, id: &str, title: &str) -> Result<()> {
        let mut conn = self.inner.conn().await?;
        crate::storage::update_session_title(&mut conn, id, title).await
    }

    pub async fn list_sessions(&self, limit: i64) -> Result<Vec<Session>> {
        let mut conn = self.inner.conn().await?;
        crate::storage::list_sessions(&mut conn, limit).await
    }

    pub async fn delete_session(&self, id: &str) -> Result<()> {
        let mut conn = self.inner.conn().await?;
        crate::storage::delete_session(&mut conn, id).await
    }

    pub async fn append_markdown(&self, session_id: &str, role: Role, md: &str) -> Result<String> {
        let mut conn = self.inner.conn().await?;
        crate::storage::append_markdown(&mut conn, session_id, role, md).await
    }

    pub async fn append_image(
        &self,
        session_id: &str,
        role: Role,
        path: &str,
        w: i64,
        h: i64,
    ) -> Result<String> {
        let mut conn = self.inner.conn().await?;
        crate::storage::append_image(&mut conn, session_id, role, path, w, h).await
    }

    pub async fn append_video(
        &self,
        session_id: &str,
        role: Role,
        path: &str,
        dur_ms: Option<i64>,
    ) -> Result<String> {
        let mut conn = self.inner.conn().await?;
        crate::storage::append_video(&mut conn, session_id, role, path, dur_ms).await
    }

    pub async fn list_messages(&self, session_id: &str, limit: i64) -> Result<Vec<Message>> {
        let mut conn = self.inner.conn().await?;
        crate::storage::list_messages(&mut conn, session_id, limit).await
    }
}
