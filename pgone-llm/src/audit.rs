use crate::LLMProvider;
use pgone_storage::blocking::StorageBlocking;
use pgone_storage::models::LlmAuditLog;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::warn;

/// 审计日志记录器
pub struct AuditLogger {
    storage: Option<Arc<RwLock<StorageBlocking>>>,
}

impl AuditLogger {
    /// 创建新的审计日志记录器
    pub fn new() -> Self {
        Self { storage: None }
    }

    /// 使用存储路径初始化审计日志记录器
    pub async fn with_storage_path(path: PathBuf) -> anyhow::Result<Self> {
        let storage = StorageBlocking::open_local(path.to_str().unwrap()).await?;
        Ok(Self {
            storage: Some(Arc::new(RwLock::new(storage))),
        })
    }

    /// 使用默认存储路径初始化审计日志记录器
    pub async fn with_default_path() -> anyhow::Result<Self> {
        let path = pgone_storage::database_path();
        Self::with_storage_path(path).await
    }

    /// 记录请求开始
    pub fn record_request(
        &self,
        session_id: Option<String>,
        provider: LLMProvider,
        model: String,
        request_content: Option<String>,
        request_size: Option<usize>,
    ) -> String {
        let id = Self::generate_id();
        let request_time = Self::now_ms();
        let provider_str = format!("{}", provider);

        let log = LlmAuditLog {
            id: id.clone(),
            session_id,
            provider: provider_str,
            model,
            request_time,
            response_time: None,
            request_size: request_size.map(|s| s as i64),
            response_size: None,
            request_content,
            response_content: None,
            status: "pending".to_string(),
            error_message: None,
            duration_ms: None,
            created_at: request_time,
        };

        // 异步记录，不阻塞
        if let Some(storage) = &self.storage {
            let storage_clone = storage.clone();
            let log_clone = log.clone();
            tokio::spawn(async move {
                let s = storage_clone.write().await;
                if let Err(e) = s.insert_llm_audit_log(&log_clone).await {
                    warn!("Failed to record audit log: {}", e);
                }
            });
        }

        id
    }

    /// 记录响应完成
    pub fn record_response(
        &self,
        log_id: &str,
        response_content: Option<String>,
        response_size: Option<usize>,
        status: AuditStatus,
        error_message: Option<String>,
    ) {
        let response_time = Self::now_ms();
        let status_str = match status {
            AuditStatus::Success => "success",
            AuditStatus::Error => "error",
            AuditStatus::Timeout => "timeout",
        };

        if let Some(storage) = &self.storage {
            let storage_clone = storage.clone();
            let log_id = log_id.to_string();
            let response_content = response_content.clone();
            let error_message = error_message.clone();
            tokio::spawn(async move {
                let s = storage_clone.write().await;
                // 先查询原始记录
                if let Ok(logs) = s.query_llm_audit_logs(None, Some(1000)).await
                    && let Some(mut log) = logs.into_iter().find(|l| l.id == log_id)
                {
                    log.response_time = Some(response_time);
                    log.response_size = response_size.map(|s| s as i64);
                    log.response_content = response_content;
                    log.status = status_str.to_string();
                    if let Some(rt) = log.response_time {
                        log.duration_ms = Some(rt - log.request_time);
                    }
                    if let Some(err_msg) = error_message {
                        log.error_message = Some(err_msg);
                    }

                    // 使用 INSERT OR REPLACE 更新记录
                    if let Err(e) = s.insert_llm_audit_log(&log).await {
                        warn!("Failed to update audit log: {}", e);
                    }
                }
            });
        }
    }

    /// 记录错误
    pub fn record_error(
        &self,
        log_id: &str,
        error_message: String,
        response_content: Option<String>,
    ) {
        self.record_response(
            log_id,
            response_content,
            None,
            AuditStatus::Error,
            Some(error_message),
        );
    }

    fn generate_id() -> String {
        use std::time::SystemTime;
        let t = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("audit-{}", t)
    }

    fn now_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new()
    }
}

/// 审计状态
#[derive(Debug, Clone, Copy)]
pub enum AuditStatus {
    Success,
    Error,
    Timeout,
}
