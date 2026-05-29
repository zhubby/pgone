use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;

use super::{
    AgentContext, AgentToolServices, Result, Tool, ToolOutput, database_name_property, json_output,
    object_schema, parse_args, string_property, target_database,
};

pub(super) struct GetTableTool;

#[derive(Deserialize)]
struct GetTableArgs {
    database_name: Option<String>,
    schema: String,
    table: String,
}

#[async_trait]
impl Tool for GetTableTool {
    fn name(&self) -> &'static str {
        "get_table"
    }

    fn description(&self) -> &'static str {
        "Read detailed metadata for one PostgreSQL table by schema and table name."
    }

    fn parameters(&self) -> Value {
        object_schema(vec![
            database_name_property().optional(),
            string_property("schema", "Schema name"),
            string_property("table", "Table name"),
        ])
    }

    async fn execute(
        &self,
        args: Value,
        dbconfig_id: &str,
        context: &AgentContext,
        services: Arc<dyn AgentToolServices>,
    ) -> Result<ToolOutput> {
        let args: GetTableArgs = parse_args(args)?;
        let table = services
            .get_table(
                dbconfig_id,
                target_database(args.database_name.as_deref(), context),
                &args.schema,
                &args.table,
            )
            .await?;
        json_output(&table)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use super::*;
    use crate::tools::test_support::{MockServices, context};

    #[test]
    fn schema_marks_schema_and_table_required() {
        let definition = GetTableTool.definition();

        assert_eq!(
            definition.parameters["required"],
            json!(["schema", "table"])
        );
        assert_eq!(definition.parameters["additionalProperties"], json!(false));
    }

    #[tokio::test]
    async fn executes_against_context_database() {
        let services = Arc::new(MockServices::default());
        let output = GetTableTool
            .execute(
                json!({"schema": "public", "table": "users"}),
                "local",
                &context(),
                services.clone(),
            )
            .await
            .unwrap();

        assert!(output.content.contains("\"name\": \"users\""));
        assert!(output.completion.is_none());
        assert_eq!(
            services.seen_database_name().await,
            Some(Some("app".to_owned()))
        );
    }

    #[tokio::test]
    async fn without_database_context_falls_back_to_config_dsn() {
        let services = Arc::new(MockServices::default());
        let mut context = context();
        context.database_name = None;

        GetTableTool
            .execute(
                json!({"schema": "public", "table": "users"}),
                "local",
                &context,
                services.clone(),
            )
            .await
            .unwrap();

        assert_eq!(services.seen_database_name().await, Some(None));
    }
}
