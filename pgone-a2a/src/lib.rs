use anyhow::Result;
use axum::{
    extract::Json,
    http::StatusCode,
    response::Json as ResponseJson,
    routing::post,
    Router,
};
use pgone_mcp_server::{
    adapter::SqlSessionIntrospector,
    core::models::{DatabaseSchema, IntrospectOptions},
};
use pgone_sql::Session;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tracing::{error, info};

/// A2A 协议请求消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaQueryRequest {
    /// PostgreSQL 数据库连接字符串 (DSN)
    pub dsn: String,
    /// 要查询的 schema 列表（可选，None 表示查询所有）
    pub schemas: Option<Vec<String>>,
    /// 是否包含索引信息
    #[serde(default = "default_true")]
    pub with_indexes: bool,
    /// 是否包含函数/存储过程信息
    #[serde(default)]
    pub with_routines: bool,
    /// 是否包含类型信息
    #[serde(default)]
    pub with_types: bool,
    /// 是否包含触发器信息
    #[serde(default)]
    pub with_triggers: bool,
}

fn default_true() -> bool {
    true
}

/// A2A 协议响应消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaQueryResponse {
    /// 是否成功
    pub success: bool,
    /// 错误信息（如果有）
    pub error: Option<String>,
    /// Schema 数据（成功时）
    pub schema: Option<DatabaseSchema>,
}

/// A2A 协议错误响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
}

/// PostgreSQL Schema 查询服务
pub struct SchemaQueryService;

impl SchemaQueryService {
    /// 查询数据库 schema
    pub async fn query_schema(req: SchemaQueryRequest) -> Result<DatabaseSchema> {
        info!(dsn = ?req.dsn, "Creating database session");
        
        let session = Session::new(&req.dsn).await
            .map_err(|e| anyhow::anyhow!("Failed to create session: {}", e))?;
        let introspector = SqlSessionIntrospector::new(session);

        let opts = IntrospectOptions {
            schemas: req.schemas,
            with_indexes: req.with_indexes,
            with_routines: req.with_routines,
            with_types: req.with_types,
            with_triggers: req.with_triggers,
            page: None,
            page_size: None,
        };

        info!("Starting database introspection");
        let schema = introspector.introspect_database(opts).await?;
        
        info!(
            database = schema.database,
            schema_count = schema.schemas.len(),
            "Database introspection completed"
        );

        Ok(schema)
    }
}

/// 处理 schema 查询请求的 HTTP 处理器
async fn handle_schema_query(
    Json(req): Json<SchemaQueryRequest>,
) -> Result<ResponseJson<SchemaQueryResponse>, (StatusCode, ResponseJson<ErrorResponse>)> {
    info!("Received schema query request");

    match SchemaQueryService::query_schema(req).await {
        Ok(schema) => Ok(ResponseJson(SchemaQueryResponse {
            success: true,
            error: None,
            schema: Some(schema),
        })),
        Err(e) => {
            error!(error = ?e, "Failed to query schema");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                ResponseJson(ErrorResponse {
                    success: false,
                    error: format!("Failed to query schema: {}", e),
                }),
            ))
        }
    }
}

/// 创建 A2A 协议的 HTTP 服务器路由
pub fn create_router() -> Router {
    Router::new().route("/schema/query", post(handle_schema_query))
}

/// 启动 A2A 协议服务器
pub async fn start_server(addr: SocketAddr) -> Result<()> {
    let app = create_router();

    info!(address = ?addr, "Starting A2A protocol server");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_schema_query_request_serialization() {
        let req = SchemaQueryRequest {
            dsn: "postgres://user:pass@localhost:5432/dbname".to_string(),
            schemas: Some(vec!["public".to_string()]),
            with_indexes: true,
            with_routines: false,
            with_types: false,
            with_triggers: false,
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("dsn"));
        assert!(json.contains("schemas"));
    }
}
