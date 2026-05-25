use crate::Storage;
use crate::models::*;
use anyhow::Result;

pub struct StorageBlocking {
    inner: Storage,
}

impl StorageBlocking {
    pub async fn open_default() -> Result<Self> {
        let inner = Storage::open_default().await?;
        Ok(Self { inner })
    }

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

    pub async fn get_default_db_config(&self) -> Result<Option<DbConfig>> {
        let mut conn = self.inner.conn().await?;
        crate::storage::get_default_db_config(&mut conn).await
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

    pub async fn query_messages_by_session(&self, session_id: &str) -> Result<Vec<Message>> {
        let mut conn = self.inner.conn().await?;
        crate::storage::query_messages_by_session(&mut conn, session_id).await
    }

    // Auth helpers
    pub async fn upsert_auth_user(&self, u: &AuthUser) -> Result<()> {
        let mut conn = self.inner.conn().await?;
        crate::storage::upsert_auth_user(&mut conn, u).await
    }

    pub async fn insert_auth_token(&self, t: &AuthToken) -> Result<()> {
        let mut conn = self.inner.conn().await?;
        crate::storage::insert_auth_token(&mut conn, t).await
    }

    pub async fn get_current_user(&self) -> Result<Option<AuthUser>> {
        let mut conn = self.inner.conn().await?;
        crate::storage::get_current_user(&mut conn).await
    }

    // Settings helpers
    pub async fn upsert_setting(&self, key: &str, value: &str) -> Result<()> {
        let mut conn = self.inner.conn().await?;
        crate::storage::upsert_setting(&mut conn, key, value).await
    }

    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.inner.conn().await?;
        crate::storage::get_setting(&mut conn, key).await
    }

    pub async fn get_all_settings(&self) -> Result<std::collections::HashMap<String, String>> {
        let mut conn = self.inner.conn().await?;
        crate::storage::get_all_settings(&mut conn).await
    }

    pub async fn delete_setting(&self, key: &str) -> Result<()> {
        let mut conn = self.inner.conn().await?;
        crate::storage::delete_setting(&mut conn, key).await
    }

    pub async fn clear_settings(&self) -> Result<()> {
        let mut conn = self.inner.conn().await?;
        crate::storage::clear_settings(&mut conn).await
    }

    // LLM Audit Log helpers
    pub async fn insert_llm_audit_log(&self, log: &crate::models::LlmAuditLog) -> Result<()> {
        let mut conn = self.inner.conn().await?;
        crate::storage::insert_llm_audit_log(&mut conn, log).await
    }

    pub async fn query_llm_audit_logs(
        &self,
        session_id: Option<&str>,
        limit: Option<i64>,
    ) -> Result<Vec<crate::models::LlmAuditLog>> {
        let mut conn = self.inner.conn().await?;
        crate::storage::query_llm_audit_logs(&mut conn, session_id, limit).await
    }

    // File index helpers
    pub async fn list_files(&self) -> Result<Vec<FileIndex>> {
        let mut conn = self.inner.conn().await?;
        crate::file_sys::query_files_by_date_range(&mut conn, None, None).await
    }

    pub async fn get_file(&self, id: &str) -> Result<Option<FileIndex>> {
        let mut conn = self.inner.conn().await?;
        crate::file_sys::get_file(&mut conn, id).await
    }

    pub async fn query_files_by_type(&self, mime_type: &str) -> Result<Vec<FileIndex>> {
        let mut conn = self.inner.conn().await?;
        crate::file_sys::query_files_by_type(&mut conn, mime_type).await
    }

    pub async fn copy_file_to_index(&self, source_path: &str) -> Result<FileIndex> {
        let mut conn = self.inner.conn().await?;
        crate::file_sys::copy_file_to_index(&mut conn, source_path).await
    }
}
