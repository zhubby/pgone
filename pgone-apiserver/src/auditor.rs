use anyhow::Result;
use pgone_auditor::extractor::ConnectionExtractorConfig;
use std::time::Instant;
use tokio_postgres::NoTls;
use tracing::{error, info, warn};

/// SQL 语句执行结果
#[derive(Debug, Clone)]
pub struct StatementResult {
    pub sql: String,
    pub success: bool,
    pub duration_ms: u64,
    pub rows_affected: i64,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub error: Option<String>,
}

/// 执行 SQL 语句列表
pub async fn execute_sqls(
    config: &ConnectionExtractorConfig,
    sqls: &[String],
) -> Result<Vec<StatementResult>> {
    info!(
        dsn = config.dsn,
        sql_count = sqls.len(),
        "Connecting to database"
    );

    // 连接到数据库
    // 注意：当前实现使用 NoTls，SSL 配置已解析但未使用
    // 如需支持 TLS，需要添加 postgres-native-tls 或 postgres-openssl 依赖
    let (client, connection) = tokio_postgres::connect(&config.dsn, NoTls).await?;

    // 在后台运行连接任务
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            error!(error = ?e, "Database connection error");
        }
    });

    if config.ssl.is_some() {
        warn!("SSL configuration provided but TLS is not enabled. Add TLS support to use SSL certificates.");
    }

    let mut results = Vec::new();

    // 执行每条 SQL
    for sql in sqls {
        let start_time = Instant::now();
        let sql_trimmed = sql.trim();
        
        if sql_trimmed.is_empty() {
            results.push(StatementResult {
                sql: sql.clone(),
                success: true,
                duration_ms: 0,
                rows_affected: 0,
                columns: Vec::new(),
                rows: Vec::new(),
                error: None,
            });
            continue;
        }

        // 判断查询类型
        let sql_upper = sql_trimmed.to_uppercase();
        let is_select = sql_upper.starts_with("SELECT")
            || sql_upper.starts_with("WITH")
            || sql_upper.starts_with("SHOW")
            || sql_upper.starts_with("EXPLAIN")
            || sql_upper.starts_with("DESCRIBE")
            || sql_upper.starts_with("DESC");

        let result = if is_select {
            // SELECT 类查询，返回结果集
            match client.query(sql_trimmed, &[]).await {
                Ok(rows) => {
                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    let row_count = rows.len() as i64;

                    info!(
                        sql = sql_trimmed,
                        row_count = row_count,
                        duration_ms = duration_ms,
                        "Query executed successfully"
                    );

                    // 提取列名和数据
                    let mut columns = Vec::new();
                    let mut data_rows = Vec::new();

                    if !rows.is_empty() {
                        // 从第一行获取列信息
                        let first_row = &rows[0];
                        for column in first_row.columns() {
                            columns.push(column.name().to_string());
                        }

                        // 转换数据行
                        for row in rows {
                            let mut values = Vec::new();
                            for i in 0..columns.len() {
                                let value = format_cell_value(&row, i);
                                values.push(value);
                            }
                            data_rows.push(values);
                        }
                    }

                    StatementResult {
                        sql: sql.to_string(),
                        success: true,
                        duration_ms,
                        rows_affected: row_count,
                        columns,
                        rows: data_rows,
                        error: None,
                    }
                }
                Err(e) => {
                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    let error_msg = e.to_string();
                    error!(
                        error = ?e,
                        sql = sql_trimmed,
                        duration_ms = duration_ms,
                        "Query execution failed"
                    );
                    StatementResult {
                        sql: sql.to_string(),
                        success: false,
                        duration_ms,
                        rows_affected: 0,
                        columns: Vec::new(),
                        rows: Vec::new(),
                        error: Some(error_msg),
                    }
                }
            }
        } else {
            // 非 SELECT 查询（INSERT, UPDATE, DELETE等），使用 execute 获取受影响行数
            match client.execute(sql_trimmed, &[]).await {
                Ok(rows_affected) => {
                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    info!(
                        sql = sql_trimmed,
                        rows_affected = rows_affected,
                        duration_ms = duration_ms,
                        "Command executed successfully"
                    );
                    StatementResult {
                        sql: sql.to_string(),
                        success: true,
                        duration_ms,
                        rows_affected: rows_affected as i64,
                        columns: Vec::new(),
                        rows: Vec::new(),
                        error: None,
                    }
                }
                Err(e) => {
                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    let error_msg = e.to_string();
                    error!(
                        error = ?e,
                        sql = sql_trimmed,
                        duration_ms = duration_ms,
                        "Command execution failed"
                    );
                    StatementResult {
                        sql: sql.to_string(),
                        success: false,
                        duration_ms,
                        rows_affected: 0,
                        columns: Vec::new(),
                        rows: Vec::new(),
                        error: Some(error_msg),
                    }
                }
            }
        };

        results.push(result);
    }

    Ok(results)
}

/// 格式化单元格值
fn format_cell_value(row: &tokio_postgres::Row, index: usize) -> String {
    use tokio_postgres::types::Type;
    
    let column = match row.columns().get(index) {
        Some(col) => col,
        None => return "".to_string(),
    };
    
    let pg_type = column.type_();
    
    match *pg_type {
        Type::BOOL => {
            let val: Option<bool> = row.get(index);
            val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string())
        }
        Type::INT2 => {
            let val: Option<i16> = row.get(index);
            val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string())
        }
        Type::INT4 => {
            let val: Option<i32> = row.get(index);
            val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string())
        }
        Type::INT8 => {
            let val: Option<i64> = row.get(index);
            val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string())
        }
        Type::FLOAT4 => {
            let val: Option<f32> = row.get(index);
            val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string())
        }
        Type::FLOAT8 => {
            let val: Option<f64> = row.get(index);
            val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string())
        }
        Type::TEXT | Type::VARCHAR => {
            let val: Option<String> = row.get(index);
            val.unwrap_or_else(|| "NULL".to_string())
        }
        _ => {
            // 对于其他类型，尝试作为字符串处理
            let val: Option<String> = row.get(index);
            val.unwrap_or_else(|| "NULL".to_string())
        }
    }
}

