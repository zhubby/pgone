use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlparser::ast::Statement;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

use super::{
    AgentContext, AgentError, AgentToolServices, Result, Tool, ToolOutput, database_name_property,
    integer_property, object_schema, parse_args, string_property, target_database,
};
use crate::ReadonlySqlRequest;

pub(super) struct RenderSqlResultTool;

#[derive(Deserialize)]
struct RenderSqlResultArgs {
    database_name: Option<String>,
    title: Option<String>,
    sql: String,
    max_rows: Option<u32>,
}

#[async_trait]
impl Tool for RenderSqlResultTool {
    fn name(&self) -> &'static str {
        "render_sql_result"
    }

    fn description(&self) -> &'static str {
        "Execute one read-only PostgreSQL query and render the rows in PgOne's Results panel. Returns only render status to the model."
    }

    fn parameters(&self) -> Value {
        object_schema(vec![
            database_name_property().optional(),
            string_property("title", "Short title for the result tab").optional(),
            string_property("sql", "One read-only SQL statement to execute and render"),
            integer_property("max_rows", "Maximum rows to render; defaults to 100").optional(),
        ])
    }

    async fn execute(
        &self,
        args: Value,
        dbconfig_id: &str,
        context: &AgentContext,
        services: Arc<dyn AgentToolServices>,
    ) -> Result<ToolOutput> {
        let args: RenderSqlResultArgs = parse_args(args)?;
        let sql = args.sql.trim().to_owned();
        validate_readonly_sql(&sql)?;
        let title = args
            .title
            .map(|title| title.trim().to_owned())
            .filter(|title| !title.is_empty());
        let database_name =
            target_database(args.database_name.as_deref(), context).map(ToOwned::to_owned);
        let result = services
            .execute_readonly_sql(
                dbconfig_id,
                ReadonlySqlRequest {
                    sql: sql.clone(),
                    database_name: database_name.clone(),
                    max_rows: args.max_rows.unwrap_or(100),
                    timeout_ms: 20_000,
                },
            )
            .await?;

        Ok(ToolOutput {
            content: json!({"rendered": true}).to_string(),
            completion: None,
            ui_payload: Some(json!({
                "title": title,
                "database_name": database_name,
                "sql": sql,
                "columns": result.columns,
                "rows": result.rows,
                "row_count": result.row_count,
                "truncated": result.truncated,
                "explain": result.explain,
            })),
        })
    }
}

pub fn validate_readonly_sql(sql: &str) -> Result<()> {
    let sql = sql.trim();
    if sql.is_empty() {
        return Err(AgentError::Tool("SQL is empty".to_owned()));
    }

    let dialect = PostgreSqlDialect {};
    let statements = Parser::parse_sql(&dialect, sql)
        .map_err(|error| AgentError::Tool(format!("invalid SQL: {error}")))?;
    if statements.len() != 1 {
        return Err(AgentError::Tool(
            "only one read-only SQL statement is allowed".to_owned(),
        ));
    }

    if statement_is_readonly(&statements[0]) {
        Ok(())
    } else {
        Err(AgentError::Tool(
            "only SELECT, WITH, VALUES, and EXPLAIN statements are allowed".to_owned(),
        ))
    }
}

fn statement_is_readonly(statement: &Statement) -> bool {
    match statement {
        Statement::Query(query) => query.locks.is_empty(),
        Statement::Explain {
            analyze, statement, ..
        } => !*analyze && statement_is_readonly(statement),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use super::*;
    use crate::tools::test_support::{MockServices, context};

    #[test]
    fn readonly_sql_validation_allows_safe_statements() {
        for sql in [
            "SELECT * FROM users",
            "WITH recent AS (SELECT * FROM users) SELECT * FROM recent",
            "VALUES (1), (2)",
            "EXPLAIN SELECT * FROM users",
        ] {
            validate_readonly_sql(sql).unwrap();
        }
    }

    #[test]
    fn readonly_sql_validation_rejects_mutating_statements() {
        for sql in [
            "",
            "INSERT INTO users (id) VALUES (1)",
            "UPDATE users SET id = 1",
            "DELETE FROM users",
            "CREATE TABLE users (id int)",
            "DROP TABLE users",
            "ALTER TABLE users ADD COLUMN name text",
            "COPY users TO STDOUT",
            "CALL refresh_users()",
            "SET search_path TO public",
            "SELECT 1; SELECT 2",
            "SELECT * FROM users FOR UPDATE",
            "EXPLAIN ANALYZE SELECT * FROM users",
        ] {
            assert!(validate_readonly_sql(sql).is_err(), "{sql}");
        }
    }

    #[tokio::test]
    async fn renders_against_services_without_returning_rows_to_model() {
        let services = Arc::new(MockServices::default());
        let output = RenderSqlResultTool
            .execute(
                json!({"title": "Users", "sql": "SELECT 1", "max_rows": 1}),
                "local",
                &context(),
                services.clone(),
            )
            .await
            .unwrap();

        assert_eq!(output.content, r#"{"rendered":true}"#);
        assert!(!output.content.contains("\"rows\""));

        let payload = output.ui_payload.unwrap();
        assert_eq!(payload["title"], json!("Users"));
        assert_eq!(payload["database_name"], json!("app"));
        assert_eq!(payload["sql"], json!("SELECT 1"));
        assert_eq!(payload["columns"], json!(["id"]));
        assert_eq!(payload["rows"], json!([["1"]]));
        assert_eq!(payload["row_count"], json!(1));
        assert_eq!(
            services.seen_database_name().await,
            Some(Some("app".to_owned()))
        );
    }
}
