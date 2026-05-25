use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use futures::{Sink, StreamExt, stream};

use pgwire::api::ClientInfo;
use pgwire::api::auth::noop::NoopStartupHandler;
use pgwire::api::query::SimpleQueryHandler;
use pgwire::api::results::{FieldFormat, FieldInfo, QueryResponse, Response, Tag};
use pgwire::error::{ErrorInfo, PgWireError, PgWireResult};
use pgwire::messages::{PgWireBackendMessage, PgWireFrontendMessage};
use tokio_postgres::NoTls;
use tracing::{error, info, warn};

use crate::dsn_extractor::extract_dsn_from_sql;
use crate::row_converter::convert_row_to_data_row;
use crate::sql_parser::parse_and_log_sql;
use crate::type_converter::convert_pg_type;

/// PostgreSQL代理处理器
pub struct PostgresProxyProcessor;

#[async_trait]
impl NoopStartupHandler for PostgresProxyProcessor {
    async fn post_startup<C>(
        &self,
        client: &mut C,
        _message: PgWireFrontendMessage,
    ) -> PgWireResult<()>
    where
        C: ClientInfo + Sink<PgWireBackendMessage> + Unpin + Send,
        C::Error: Debug,
        PgWireError: From<<C as Sink<PgWireBackendMessage>>::Error>,
    {
        info!(
            socket_addr = ?client.socket_addr(),
            is_secure = client.is_secure(),
            protocol_version = ?client.protocol_version(),
            pid_and_secret_key = ?client.pid_and_secret_key(),
            metadata = ?client.metadata(),
            "Client connected"
        );
        Ok(())
    }
}

#[async_trait]
impl SimpleQueryHandler for PostgresProxyProcessor {
    async fn do_query<C>(&self, client: &mut C, query: &str) -> PgWireResult<Vec<Response>>
    where
        C: ClientInfo + Unpin + Send + Sync,
    {
        info!(
            socket_addr = ?client.socket_addr(),
            query = query,
            "Received query"
        );

        // 提取DSN和实际SQL
        let (dsn, actual_sql) = match extract_dsn_from_sql(query) {
            Some((dsn, sql)) => {
                info!(dsn = dsn, "Extracted DSN from SQL comment");
                (dsn, sql)
            }
            None => {
                warn!(
                    query = query,
                    "No DSN found in SQL comment, using query as-is"
                );
                (String::new(), query.to_string())
            }
        };

        if dsn.is_empty() {
            return Err(PgWireError::UserError(Box::new(ErrorInfo::new(
                "ERROR".to_owned(),
                "08000".to_owned(),
                "No DSN specified in SQL comment. Please use format: -- DSN: postgres://..."
                    .to_string(),
            ))));
        }

        // 解析SQL AST并记录日志
        parse_and_log_sql(&actual_sql);

        // 连接到后端数据库
        let backend_client = match tokio_postgres::connect(&dsn, NoTls).await {
            Ok((client, conn)) => {
                info!(dsn = dsn, "Connected to backend database");
                // 在后台运行连接任务
                tokio::spawn(async move {
                    if let Err(e) = conn.await {
                        error!(
                            error = ?e,
                            "Backend connection error"
                        );
                    }
                });
                client
            }
            Err(e) => {
                error!(
                    error = ?e,
                    dsn = dsn,
                    "Failed to connect to backend database"
                );
                return Err(PgWireError::UserError(Box::new(ErrorInfo::new(
                    "FATAL".to_owned(),
                    "08006".to_owned(),
                    format!("Failed to connect to backend database: {}", e),
                ))));
            }
        };

        // 判断查询类型
        let sql_upper = actual_sql.trim().to_uppercase();
        let is_select = sql_upper.starts_with("SELECT")
            || sql_upper.starts_with("WITH")
            || sql_upper.starts_with("SHOW")
            || sql_upper.starts_with("EXPLAIN")
            || sql_upper.starts_with("DESCRIBE")
            || sql_upper.starts_with("DESC");

        if is_select {
            // SELECT类查询，返回结果集
            match backend_client.query(&actual_sql, &[]).await {
                Ok(rows) => {
                    info!(row_count = rows.len(), "Query executed successfully");

                    if rows.is_empty() {
                        // SELECT查询但没有结果，返回空结果集
                        return Ok(vec![Response::Query(QueryResponse::new(
                            Arc::new(vec![]),
                            stream::empty(),
                        ))]);
                    }

                    // 构建schema
                    let first_row = &rows[0];
                    let mut fields = Vec::new();
                    for column in first_row.columns() {
                        let pg_type = convert_pg_type(column.type_());
                        let field_info = FieldInfo::new(
                            column.name().into(),
                            None,
                            None,
                            pg_type,
                            FieldFormat::Text,
                        );
                        fields.push(field_info);
                    }
                    let schema = Arc::new(fields);

                    // 转换数据行
                    let schema_ref = schema.clone();
                    let data_row_stream = stream::iter(rows).map(move |row| {
                        match convert_row_to_data_row(row, &schema_ref) {
                            Ok(encoder) => encoder.finish(),
                            Err(e) => {
                                error!(
                                    error = ?e,
                                    "Failed to convert row"
                                );
                                Err(e)
                            }
                        }
                    });

                    Ok(vec![Response::Query(QueryResponse::new(
                        schema,
                        data_row_stream,
                    ))])
                }
                Err(e) => {
                    error!(
                        error = ?e,
                        sql = actual_sql,
                        "Query execution failed"
                    );
                    Err(PgWireError::UserError(Box::new(ErrorInfo::new(
                        "ERROR".to_owned(),
                        "XX000".to_owned(),
                        format!("Query execution failed: {}", e),
                    ))))
                }
            }
        } else {
            // 非SELECT查询（INSERT, UPDATE, DELETE等），使用execute获取受影响行数
            match backend_client.execute(&actual_sql, &[]).await {
                Ok(rows_affected) => {
                    info!(
                        rows_affected = rows_affected,
                        "Command executed successfully"
                    );
                    Ok(vec![Response::Execution(
                        Tag::new("OK").with_rows(rows_affected as usize),
                    )])
                }
                Err(e) => {
                    error!(
                        error = ?e,
                        sql = actual_sql,
                        "Command execution failed"
                    );
                    Err(PgWireError::UserError(Box::new(ErrorInfo::new(
                        "ERROR".to_owned(),
                        "XX000".to_owned(),
                        format!("Command execution failed: {}", e),
                    ))))
                }
            }
        }
    }
}

/// PostgreSQL代理处理器工厂
pub struct PostgresProxyProcessorFactory {
    pub handler: Arc<PostgresProxyProcessor>,
}

impl Default for PostgresProxyProcessorFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl PostgresProxyProcessorFactory {
    pub fn new() -> PostgresProxyProcessorFactory {
        Self {
            handler: Arc::new(PostgresProxyProcessor),
        }
    }
}
