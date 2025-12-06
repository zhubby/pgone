use anyhow::Result;
use pgone_proxy::extractor::ConnectionExtractorConfig;
use std::net::SocketAddr;
use tonic::{transport::Server, Request, Response, Status};
use tracing::info;

pub mod proto {
    tonic::include_proto!("pgone.proxy.v1");
}

use proto::proxy_service_server::{ProxyService, ProxyServiceServer};
use proto::{ProxyRequest, ProxyResponse, ConnectionConfig, Row, StatementResult};

use crate::proxy::execute_sqls;

#[derive(Debug, Default)]
pub struct ProxyServiceImpl;

#[tonic::async_trait]
impl ProxyService for ProxyServiceImpl {
    async fn execute(
        &self,
        request: Request<ProxyRequest>,
    ) -> Result<Response<ProxyResponse>, Status> {
        let req = request.into_inner();
        
        info!(
            sql_count = req.sql.len(),
            "Received gRPC proxy request"
        );

        // 转换 ConnectionConfig 为 ConnectionExtractorConfig
        let config = req.config
            .ok_or_else(|| Status::invalid_argument("Missing connection config"))?;
        let config = convert_connection_config(&config)?;

        // 执行 SQL
        let results = execute_sqls(&config, &req.sql)
            .await
            .map_err(|e| Status::internal(format!("Failed to execute SQL: {}", e)))?;

        // 转换为 protobuf 消息
        let proto_results: Vec<StatementResult> = results
            .into_iter()
            .map(|r| StatementResult {
                sql: r.sql,
                success: r.success,
                duration_ms: r.duration_ms,
                rows_affected: r.rows_affected,
                columns: r.columns,
                rows: r.rows
                    .into_iter()
                    .map(|values| Row { values })
                    .collect(),
                error: r.error,
            })
            .collect();

        Ok(Response::new(ProxyResponse {
            results: proto_results,
        }))
    }
}

fn convert_connection_config(config: &ConnectionConfig) -> Result<ConnectionExtractorConfig, Status> {
    let ssl = if let Some(s) = &config.ssl {
        Some(pgone_proxy::extractor::SslExtractorConfig {
            cert: s.cert.as_ref()
                .and_then(|path| path.parse::<std::path::PathBuf>().ok()),
            key: s.key.as_ref()
                .and_then(|path| path.parse::<std::path::PathBuf>().ok()),
            ca: s.ca.as_ref()
                .and_then(|path| path.parse::<std::path::PathBuf>().ok()),
            mode: s.mode.clone(),
        })
    } else {
        None
    };

    Ok(ConnectionExtractorConfig {
        dsn: config.dsn.clone(),
        sql: vec![], // SQL 从请求中获取
        ssl,
        replay: None,
    })
}

pub async fn serve_grpc(addr: SocketAddr, shutdown: impl std::future::Future<Output = ()> + Send + 'static) -> Result<()> {
    info!("Starting gRPC server on {}", addr);

    let proxy_service = ProxyServiceImpl::default();

    Server::builder()
        .add_service(ProxyServiceServer::new(proxy_service))
        .serve_with_shutdown(addr, shutdown)
        .await?;

    Ok(())
}

