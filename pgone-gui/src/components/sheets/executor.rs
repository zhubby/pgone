use super::utils;
use super::{QueryResult, ResultsTable};
use crate::components::SqlCtx;
use crate::components::db_manager::PoolRegistry;
use crate::futures;
use poll_promise::Promise;
use sqlx::postgres::{PgPool, PgRow};
use sqlx::{Column, Row};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PaginatedSql {
    pub query_sql: String,
    pub count_sql: String,
}

impl ResultsTable {
    /// Execute SQL query.
    pub fn run_sql(&mut self, ctxs: &mut SqlCtx) {
        let Some((dsn, sql)) = self.query_request(ctxs, self.sql_input.clone()) else {
            return;
        };
        self.start_query(ctxs.db.pools.clone(), dsn, sql, 1);
    }

    pub fn run_sql_text(
        &mut self,
        ctxs: &mut SqlCtx,
        sql: impl Into<String>,
        database: Option<String>,
    ) {
        let previous_database = self.selected_database.clone();
        self.selected_database = database;

        let Some((dsn, sql)) = self.query_request(ctxs, sql.into()) else {
            self.selected_database = previous_database;
            return;
        };

        self.start_query(ctxs.db.pools.clone(), dsn, sql, 1);
        self.selected_database = previous_database;
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

    pub(super) fn start_query(
        &mut self,
        pools: PoolRegistry,
        dsn: String,
        sql: String,
        page: usize,
    ) {
        let (sender, promise) = Promise::new();
        self.query_promise = Some(promise);
        self.query_columns.clear();
        self.query_rows.clear();
        self.selected_result_row = None;
        self.clear_json_viewer_tabs();
        self.explain_info = None;
        self.explain_error = None;
        self.current_sql = Some(sql.clone());
        self.paged_base_sql = Some(sql.clone());
        self.current_page = page.max(1);
        self.total_rows = None;
        self.has_next_page = false;
        self.pagination_enabled = is_pageable_sql(&sql);
        let page_size = self.effective_page_size();

        futures::spawn(async move {
            let result = match pools.get_or_create_pool(&dsn).await {
                Ok(pool) => execute_query(pool, &sql, page.max(1), page_size).await,
                Err(error) => Err(error),
            };
            sender.send(result);
        });
    }

    pub(super) fn start_page_query(&mut self, ctxs: &mut SqlCtx, page: usize) {
        let Some(sql) = self
            .paged_base_sql
            .clone()
            .or_else(|| self.current_sql.clone())
        else {
            return;
        };
        let Some((dsn, sql)) = self.query_request(ctxs, sql) else {
            return;
        };
        self.start_query(ctxs.db.pools.clone(), dsn, sql, page);
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
                        self.total_rows = result.total_rows;
                        self.has_next_page = result.has_next_page;
                        self.pagination_enabled = result.pagination_enabled;
                        self.sql_error = None;
                        if let Some(rows_affected) = result.rows_affected {
                            crate::notify::sql_execute_success(rows_affected);
                        }
                    }
                    Err(error) => {
                        let error = error.clone();
                        self.sql_error = Some(error.clone());
                        self.selected_result_row = None;
                        self.clear_json_viewer_tabs();
                        self.total_rows = None;
                        self.has_next_page = false;
                        self.pagination_enabled = false;
                        crate::notify::sql_execute_error(&error);
                    }
                }
                self.query_promise = None;
            }
        }
    }
}

async fn execute_query(
    pool: PgPool,
    sql: &str,
    page: usize,
    page_size: usize,
) -> Result<QueryResult, String> {
    if !statement_returns_rows(sql) {
        let result = sqlx::query(sql)
            .execute(&pool)
            .await
            .map_err(|error| error.to_string())?;
        return Ok(QueryResult {
            columns: Vec::new(),
            rows: Vec::new(),
            primary_key_columns: HashSet::new(),
            explain_info: None,
            explain_error: None,
            total_rows: None,
            has_next_page: false,
            pagination_enabled: false,
            rows_affected: Some(result.rows_affected()),
        });
    }

    let primary_key_columns = detect_primary_keys(sql, &pool).await.unwrap_or_default();
    let explain = execute_explain(sql, &pool).await;
    let paginated_sql = build_paginated_sql(sql, page, page_size);
    let total_rows = match &paginated_sql {
        Some(paginated_sql) => execute_count(&pool, &paginated_sql.count_sql).await.ok(),
        None => None,
    };
    let query_sql = paginated_sql
        .as_ref()
        .map(|paginated_sql| paginated_sql.query_sql.as_str())
        .unwrap_or(sql);

    let rows: Vec<PgRow> = sqlx::query(query_sql)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;
    let pagination_enabled = paginated_sql.is_some();
    let has_next_page = if pagination_enabled {
        total_rows
            .map(|total_rows| page.saturating_mul(page_size) < total_rows)
            .unwrap_or_else(|| rows.len() == page_size)
    } else {
        false
    };
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
        total_rows,
        has_next_page,
        pagination_enabled,
        rows_affected: None,
    })
}

pub(super) fn build_paginated_sql(
    sql: &str,
    page: usize,
    page_size: usize,
) -> Option<PaginatedSql> {
    if !is_pageable_sql(sql) {
        return None;
    }

    let page = page.max(1);
    let page_size = page_size.max(1);
    let offset = (page - 1).saturating_mul(page_size);
    let base_sql = sql.trim().trim_end_matches(';').trim();
    Some(PaginatedSql {
        query_sql: format!(
            "SELECT * FROM ({base_sql}) pgone_page LIMIT {page_size} OFFSET {offset}"
        ),
        count_sql: format!("SELECT COUNT(*) FROM ({base_sql}) pgone_count"),
    })
}

pub(super) fn is_pageable_sql(sql: &str) -> bool {
    let dialect = sqlparser::dialect::PostgreSqlDialect {};
    let Ok(statements) = sqlparser::parser::Parser::parse_sql(&dialect, sql) else {
        return false;
    };
    matches!(statements.as_slice(), [sqlparser::ast::Statement::Query(_)])
}

fn statement_returns_rows(sql: &str) -> bool {
    let dialect = sqlparser::dialect::PostgreSqlDialect {};
    let Ok(statements) = sqlparser::parser::Parser::parse_sql(&dialect, sql) else {
        return true;
    };
    let [statement] = statements.as_slice() else {
        return true;
    };
    match statement {
        sqlparser::ast::Statement::Query(_) | sqlparser::ast::Statement::Explain { .. } => true,
        _ => sql.to_ascii_lowercase().contains(" returning "),
    }
}

async fn execute_count(pool: &sqlx::PgPool, sql: &str) -> Result<usize, String> {
    let row = sqlx::query(sql)
        .fetch_one(pool)
        .await
        .map_err(|error| error.to_string())?;
    let count = row
        .try_get::<i64, _>(0)
        .map_err(|error| error.to_string())?;
    usize::try_from(count).map_err(|error| error.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::sheets::DEFAULT_RESULTS_PAGE_SIZE;

    #[test]
    fn default_results_table_uses_default_page_size() {
        let table = ResultsTable::default();

        assert_eq!(table.page_size, DEFAULT_RESULTS_PAGE_SIZE);
        assert_eq!(table.effective_page_size(), DEFAULT_RESULTS_PAGE_SIZE);
    }

    #[test]
    fn builds_paginated_sql_for_select() {
        let paginated = build_paginated_sql("SELECT * FROM users", 1, DEFAULT_RESULTS_PAGE_SIZE)
            .expect("select should be pageable");

        assert_eq!(
            paginated.query_sql,
            "SELECT * FROM (SELECT * FROM users) pgone_page LIMIT 100 OFFSET 0"
        );
        assert_eq!(
            paginated.count_sql,
            "SELECT COUNT(*) FROM (SELECT * FROM users) pgone_count"
        );
    }

    #[test]
    fn builds_paginated_sql_for_second_page() {
        let paginated = build_paginated_sql("SELECT * FROM users", 2, DEFAULT_RESULTS_PAGE_SIZE)
            .expect("select should be pageable");

        assert!(paginated.query_sql.ends_with("LIMIT 100 OFFSET 100"));
    }

    #[test]
    fn builds_paginated_sql_for_with_and_values() {
        assert!(
            build_paginated_sql(
                "WITH recent AS (SELECT * FROM users) SELECT * FROM recent",
                1,
                DEFAULT_RESULTS_PAGE_SIZE,
            )
            .is_some()
        );
        assert!(build_paginated_sql("VALUES (1), (2)", 1, DEFAULT_RESULTS_PAGE_SIZE).is_some());
    }

    #[test]
    fn does_not_page_unsafe_or_unparseable_sql() {
        for sql in [
            "SELECT 1; SELECT 2",
            "UPDATE users SET name = 'Ada'",
            "DELETE FROM users",
            "this is not sql",
        ] {
            assert!(
                build_paginated_sql(sql, 1, DEFAULT_RESULTS_PAGE_SIZE).is_none(),
                "{sql}"
            );
        }
    }

    #[test]
    fn classifies_row_returning_statements() {
        for sql in [
            "SELECT * FROM users",
            "WITH recent AS (SELECT * FROM users) SELECT * FROM recent",
            "VALUES (1), (2)",
            "EXPLAIN SELECT * FROM users",
            "INSERT INTO users (name) VALUES ('Ada') RETURNING id",
        ] {
            assert!(statement_returns_rows(sql), "{sql}");
        }
    }

    #[test]
    fn classifies_non_returning_statements() {
        for sql in [
            "INSERT INTO users (name) VALUES ('Ada')",
            "UPDATE users SET name = 'Ada'",
            "DELETE FROM users",
            "CREATE TABLE users (id int)",
        ] {
            assert!(!statement_returns_rows(sql), "{sql}");
        }
    }
}
