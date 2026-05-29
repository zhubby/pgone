use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    AgentContext, AgentError, AgentToolServices, AgentTurnStatus, CompletionSignal, Result, Tool,
    ToolOutput, database_name_property, object_schema, parse_args, string_property,
    target_database,
};

pub(super) struct PreviewSqlTool;

#[derive(Deserialize)]
struct PreviewSqlArgs {
    database_name: Option<String>,
    title: Option<String>,
    sql: String,
}

#[async_trait]
impl Tool for PreviewSqlTool {
    fn name(&self) -> &'static str {
        "preview_sql"
    }

    fn description(&self) -> &'static str {
        "Send generated SQL to PgOne's SQL panel as an editable preview tab for the user to review and execute manually. This tool does not execute SQL."
    }

    fn parameters(&self) -> Value {
        object_schema(vec![
            database_name_property().optional(),
            string_property("title", "Short title for the preview tab").optional(),
            string_property(
                "sql",
                "SQL text to preview. May be read-only or mutating because this tool only stages the SQL and never executes it.",
            ),
        ])
    }

    async fn execute(
        &self,
        args: Value,
        _dbconfig_id: &str,
        context: &AgentContext,
        _services: Arc<dyn AgentToolServices>,
    ) -> Result<ToolOutput> {
        let args: PreviewSqlArgs = parse_args(args)?;
        let sql = args.sql.trim();
        if sql.is_empty() {
            return Err(AgentError::Tool("SQL preview is empty".to_owned()));
        }

        let title = args.title.and_then(|title| {
            let title = title.trim().to_owned();
            (!title.is_empty()).then_some(title)
        });
        let database_name =
            target_database(args.database_name.as_deref(), context).map(ToOwned::to_owned);
        let content = serde_json::to_string_pretty(&json!({
            "title": title,
            "sql": sql,
            "database_name": database_name,
        }))
        .map_err(|error| AgentError::Tool(error.to_string()))?;
        let summary =
            "SQL has been sent to the SQL panel. Please review it before executing.".to_owned();

        Ok(ToolOutput {
            content,
            completion: Some(CompletionSignal {
                status: AgentTurnStatus::Completed,
                summary,
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use super::*;
    use crate::tools::test_support::{MockServices, context};

    #[tokio::test]
    async fn accepts_mutating_sql_without_executing() {
        for sql in [
            "CREATE TABLE public.audit_log (id bigint);",
            "UPDATE public.users SET active = false;",
            "DROP TABLE public.old_users;",
        ] {
            let output = PreviewSqlTool
                .execute(
                    json!({"title": "Migration", "sql": sql}),
                    "local",
                    &context(),
                    Arc::new(MockServices::default()),
                )
                .await
                .unwrap();

            assert!(output.content.contains(sql));
            assert!(output.content.contains("\"database_name\": \"app\""));
            assert_eq!(
                output
                    .completion
                    .as_ref()
                    .map(|completion| &completion.status),
                Some(&AgentTurnStatus::Completed)
            );
        }
    }

    #[tokio::test]
    async fn rejects_empty_sql() {
        let error = PreviewSqlTool
            .execute(
                json!({"sql": " \n\t "}),
                "local",
                &context(),
                Arc::new(MockServices::default()),
            )
            .await
            .unwrap_err();

        assert!(error.to_string().contains("SQL preview is empty"));
    }

    #[tokio::test]
    async fn database_argument_overrides_context_database() {
        let output = PreviewSqlTool
            .execute(
                json!({"database_name": "analytics", "sql": "SELECT 1"}),
                "local",
                &context(),
                Arc::new(MockServices::default()),
            )
            .await
            .unwrap();

        assert!(output.content.contains("\"database_name\": \"analytics\""));
    }
}
