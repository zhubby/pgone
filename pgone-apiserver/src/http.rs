use axum::{extract::Json, response::IntoResponse, routing::post, Router};
use http::StatusCode;
use pgone_proxy::extractor::ConnectionExtractorConfig;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::proxy::execute_sqls;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct AuditRequest {
    pub config: ConnectionConfigRequest,
    pub sql: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ConnectionConfigRequest {
    pub dsn: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl: Option<SslConfigRequest>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct SslConfigRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cert: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ca: Option<String>,
    pub mode: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuditResponse {
    pub results: Vec<StatementResultResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct StatementResultResponse {
    pub sql: String,
    pub success: bool,
    pub duration_ms: u64,
    pub rows_affected: i64,
    pub columns: Vec<String>,
    pub rows: Vec<RowResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RowResponse {
    pub values: Vec<String>,
}

#[utoipa::path(
    post,
    path = "/api/v1/audit/execute",
    request_body = AuditRequest,
    responses(
        (status = 200, description = "Audit execution result", body = AuditResponse)
    ),
    tag = "auditor"
)]
pub async fn execute_audit(Json(req): Json<AuditRequest>) -> impl IntoResponse {
    // 转换请求配置
    let config = ConnectionExtractorConfig {
        dsn: req.config.dsn,
        sql: vec![], // SQL 从请求中获取
        ssl: req.config.ssl.map(|s| pgone_proxy::extractor::SslExtractorConfig {
            cert: s.cert.map(|p| p.into()),
            key: s.key.map(|p| p.into()),
            ca: s.ca.map(|p| p.into()),
            mode: s.mode,
        }),
        replay: None,
    };

    // 执行 SQL
    match execute_sqls(&config, &req.sql).await {
        Ok(results) => {
            let response_results: Vec<StatementResultResponse> = results
                .into_iter()
                .map(|r| StatementResultResponse {
                    sql: r.sql,
                    success: r.success,
                    duration_ms: r.duration_ms,
                    rows_affected: r.rows_affected,
                    columns: r.columns,
                    rows: r.rows
                        .into_iter()
                        .map(|values| RowResponse { values })
                        .collect(),
                    error: r.error,
                })
                .collect();

            (StatusCode::OK, Json(AuditResponse {
                results: response_results,
            }))
        }
        Err(e) => {
            tracing::error!(error = ?e, "Failed to execute audit");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuditResponse {
                    results: vec![],
                }),
            )
        }
    }
}

pub fn router() -> Router {
    Router::new().route("/api/v1/audit/execute", post(execute_audit))
}

