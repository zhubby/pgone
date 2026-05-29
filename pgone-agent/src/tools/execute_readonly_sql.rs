use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use sqlparser::ast::Statement;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

use super::{
    AgentContext, AgentError, AgentToolServices, Result, Tool, ToolOutput, database_name_property,
    integer_property, json_output, object_schema, parse_args, string_property, target_database,
};
use crate::ReadonlySqlRequest;

pub(super) struct ExecuteReadonlySqlTool;

#[derive(Deserialize)]
struct ExecuteReadonlySqlArgs {
    database_name: Option<String>,
    sql: String,
    max_rows: Option<u32>,
}

#[async_trait]
impl Tool for ExecuteReadonlySqlTool {
    fn name(&self) -> &'static str {
        "execute_readonly_sql"
    }

    fn description(&self) -> &'static str {
        "Execute one read-only PostgreSQL query such as SELECT, WITH, VALUES, or EXPLAIN and return bounded rows."
    }

    fn parameters(&self) -> Value {
        object_schema(vec![
            database_name_property().optional(),
            string_property("sql", "One read-only SQL statement to execute"),
            integer_property("max_rows", "Maximum rows to return; defaults to 100").optional(),
        ])
    }

    async fn execute(
        &self,
        args: Value,
        dbconfig_id: &str,
        context: &AgentContext,
        services: Arc<dyn AgentToolServices>,
    ) -> Result<ToolOutput> {
        let args: ExecuteReadonlySqlArgs = parse_args(args)?;
        validate_readonly_sql(&args.sql)?;
        let result = services
            .execute_readonly_sql(
                dbconfig_id,
                ReadonlySqlRequest {
                    sql: args.sql,
                    database_name: target_database(args.database_name.as_deref(), context)
                        .map(ToOwned::to_owned),
                    max_rows: args.max_rows.unwrap_or(100),
                    timeout_ms: 20_000,
                },
            )
            .await?;
        json_output(&result)
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
    async fn executes_against_services_with_context_database() {
        let services = Arc::new(MockServices::default());
        let output = ExecuteReadonlySqlTool
            .execute(
                json!({"sql": "SELECT 1", "max_rows": 1}),
                "local",
                &context(),
                services.clone(),
            )
            .await
            .unwrap();

        assert!(output.content.contains("\"columns\""));
        assert!(output.content.contains("\"truncated\": false"));
        assert_eq!(
            services.seen_database_name().await,
            Some(Some("app".to_owned()))
        );
    }
}
