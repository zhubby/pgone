use super::{QueryResult, ResultsTable};
use super::utils;
use crate::components::SqlCtx;
use crate::futures;
use poll_promise::Promise;
use sqlx::postgres::{PgPoolOptions, PgRow};
use sqlx::{Column, Row};
use std::collections::HashSet;

impl ResultsTable {
    /// Execute SQL query.
    pub fn run_sql(&mut self, ctxs: &mut SqlCtx) {
        let Some((dsn, sql)) = self.query_request(ctxs, self.sql_input.clone()) else {
            return;
        };
        self.start_query(dsn, sql);
    }

    pub(super) fn query_request(
        &mut self,
        ctxs: &mut SqlCtx,
        sql: String,
    ) -> Option<(String, String)> {
        self.sql_error = None;
        self.explain_error = None;
        self.primary_key_columns.clear();

        let mut dsn = match ctxs.db.active_dsn() {
            Some(dsn) => dsn,
            None => {
                self.sql_error = Some("Database config not found".into());
                return None;
            }
        };

        if dsn.trim().is_empty() {
            self.sql_error = Some("DSN is empty".into());
            return None;
        }

        if let Some(ref selected_db) = self.selected_database {
            if let Some(new_dsn) = utils::replace_database_in_dsn(&dsn, selected_db) {
                dsn = new_dsn;
            } else {
                self.sql_error = Some(format!(
                    "Failed to replace database in DSN: {}",
                    selected_db
                ));
                return None;
            }
        }

        Some((dsn, sql))
    }

    pub(super) fn start_query(&mut self, dsn: String, sql: String) {
        let (sender, promise) = Promise::new();
        self.query_promise = Some(promise);
        self.query_columns.clear();
        self.query_rows.clear();
        self.explain_info = None;
        self.explain_error = None;
        self.current_sql = Some(sql.clone());

        futures::spawn(async move {
            let result = execute_query(&dsn, &sql).await;
            sender.send(result);
        });
    }

    pub(super) fn poll_query_promise(&mut self) {
        if let Some(promise) = &self.query_promise {
            if let Some(result) = promise.ready() {
                match result {
                    Ok(result) => {
                        self.query_columns = result.columns.clone();
                        self.query_rows = result.rows.clone();
                        self.primary_key_columns = result.primary_key_columns.clone();
                        self.explain_info = result.explain_info.clone();
                        self.explain_error = result.explain_error.clone();
                        self.sql_error = None;
                    }
                    Err(error) => {
                        self.sql_error = Some(error.clone());
                    }
                }
                self.query_promise = None;
            }
        }
    }
}

async fn execute_query(dsn: &str, sql: &str) -> Result<QueryResult, String> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(dsn)
        .await
        .map_err(|e| format!("Failed to create connection pool: {}", e))?;

    let primary_key_columns = detect_primary_keys(sql, &pool).await.unwrap_or_default();
    let explain = execute_explain(sql, &pool).await;

    let rows: Vec<PgRow> = sqlx::query(sql)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;
    let mut columns = Vec::new();
    let mut data = Vec::new();
    if let Some(first) = rows.first() {
        for column in first.columns() {
            columns.push(column.name().to_string());
        }
    }
    for row in rows.into_iter().take(10000) {
        let mut values = Vec::new();
        let len = if columns.is_empty() {
            row.len()
        } else {
            columns.len()
        };
        for index in 0..len {
            values.push(crate::sql::format_cell(&row, index));
        }
        data.push(values);
    }

    let (explain_info, explain_error) = match explain {
        Some(Ok(output)) => (
            parse_explain_output(&output),
            parse_explain_output(&output)
                .is_none()
                .then(|| "Failed to parse EXPLAIN output".to_string()),
        ),
        Some(Err(error)) => (None, Some(error)),
        None => (None, None),
    };

    Ok(QueryResult {
        columns,
        rows: data,
        primary_key_columns,
        explain_info,
        explain_error,
    })
}

fn parse_explain_output(output: &str) -> Option<super::ExplainInfo> {
    let first_line = output.lines().next()?;
    Some(super::ExplainInfo {
        scan_type: extract_scan_type(first_line),
        cost: extract_cost(first_line),
        rows: extract_rows(first_line),
    })
}

fn extract_scan_type(line: &str) -> String {
    let patterns = [
        "Seq Scan",
        "Index Scan",
        "Index Only Scan",
        "Bitmap Index Scan",
        "Bitmap Heap Scan",
        "Hash Join",
        "Nested Loop",
        "Merge Join",
        "Sort",
        "Aggregate",
        "Group",
        "Limit",
        "Subquery Scan",
        "CTE Scan",
        "Function Scan",
        "Materialize",
    ];

    for pattern in patterns {
        if line.contains(pattern) {
            return pattern.to_string();
        }
    }

    if let Some(start) = line.find(|c: char| c.is_uppercase()) {
        let end = line[start..]
            .find(|c: char| c.is_whitespace() || c == '(')
            .unwrap_or(line.len() - start);
        return line[start..start + end].to_string();
    }

    "Unknown".to_string()
}

fn extract_cost(line: &str) -> String {
    if let Some(start) = line.find("cost=") {
        let cost_start = start + 5;
        if let Some(end) = line[cost_start..].find(|c: char| c == ' ' || c == ')') {
            return line[cost_start..cost_start + end].to_string();
        }
        return line[cost_start..].trim().to_string();
    }
    "N/A".to_string()
}

fn extract_rows(line: &str) -> String {
    if let Some(start) = line.find("rows=") {
        let rows_start = start + 5;
        if let Some(end) = line[rows_start..].find(|c: char| c == ' ' || c == ')') {
            return line[rows_start..rows_start + end].to_string();
        }
        return line[rows_start..].trim().to_string();
    }
    "N/A".to_string()
}

async fn execute_explain(sql: &str, pool: &sqlx::PgPool) -> Option<Result<String, String>> {
    let sql_trimmed = sql.trim();
    let sql_upper = sql_trimmed.to_uppercase();
    if !sql_upper.starts_with("SELECT")
        && !sql_upper.starts_with("WITH")
        && !sql_upper.starts_with("VALUES")
    {
        return None;
    }

    let explain_sql = format!("EXPLAIN (FORMAT TEXT) {}", sql_trimmed);
    let result = async {
        let rows: Vec<PgRow> = sqlx::query(&explain_sql)
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;

        let mut output = String::new();
        for row in rows {
            if let Ok(text) = row.try_get::<String, _>(0) {
                output.push_str(&text);
                output.push('\n');
            }
        }
        Ok(output)
    }
    .await;

    Some(result)
}

async fn detect_primary_keys(sql: &str, pool: &sqlx::PgPool) -> Option<HashSet<String>> {
    let dialect = sqlparser::dialect::PostgreSqlDialect {};
    let ast = sqlparser::parser::Parser::parse_sql(&dialect, sql).ok()?;

    let mut table_names = Vec::new();
    for stmt in ast {
        if let sqlparser::ast::Statement::Query(query) = stmt {
            if let sqlparser::ast::SetExpr::Select(select) = &*query.body {
                for table_with_joins in &select.from {
                    if let sqlparser::ast::TableFactor::Table { name, .. } =
                        &table_with_joins.relation
                    {
                        let schema = name.0.first().map(|identifier| identifier.value.clone());
                        let table = name.0.last().map(|identifier| identifier.value.clone());
                        match (schema, table) {
                            (Some(schema), Some(table)) => table_names.push((schema, table)),
                            (None, Some(table)) => table_names.push(("public".to_string(), table)),
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    let (schema, table) = table_names.first()?;
    let pk_query = "SELECT kcu.column_name \
            FROM information_schema.table_constraints tc \
            JOIN information_schema.key_column_usage kcu \
              ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema \
            WHERE tc.constraint_type = 'PRIMARY KEY' AND tc.table_schema = $1 AND tc.table_name = $2 \
            ORDER BY kcu.ordinal_position";

    let rows = sqlx::query(pk_query)
        .bind(schema)
        .bind(table)
        .fetch_all(pool)
        .await
        .ok()?;

    Some(
        rows.into_iter()
            .map(|row| row.get::<String, _>(0))
            .collect(),
    )
}
