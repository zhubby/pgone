use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;

use super::{
    AgentContext, AgentToolServices, Result, Tool, ToolOutput, database_name_property, json_output,
    object_schema, parse_args, string_property, target_database,
};

pub(super) struct ListTriggersTool;

#[derive(Deserialize)]
struct ListTriggersArgs {
    database_name: Option<String>,
    schema: Option<String>,
}

#[async_trait]
impl Tool for ListTriggersTool {
    fn name(&self) -> &'static str {
        "list_triggers"
    }

    fn description(&self) -> &'static str {
        "List PostgreSQL trigger metadata, optionally filtered by schema."
    }

    fn parameters(&self) -> Value {
        object_schema(vec![
            database_name_property().optional(),
            string_property("schema", "Schema name").optional(),
        ])
    }

    async fn execute(
        &self,
        args: Value,
        dbconfig_id: &str,
        context: &AgentContext,
        services: Arc<dyn AgentToolServices>,
    ) -> Result<ToolOutput> {
        let args: ListTriggersArgs = parse_args(args)?;
        let triggers = services
            .list_triggers(
                dbconfig_id,
                target_database(args.database_name.as_deref(), context),
                args.schema.as_deref(),
            )
            .await?;
        json_output(&triggers)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use super::*;
    use crate::tools::test_support::{MockServices, context};

    #[tokio::test]
    async fn executes_against_context_database() {
        let services = Arc::new(MockServices::default());
        let output = ListTriggersTool
            .execute(
                json!({"schema": "public"}),
                "local",
                &context(),
                services.clone(),
            )
            .await
            .unwrap();

        assert_eq!(output.content, "[]");
        assert_eq!(
            services.seen_database_name().await,
            Some(Some("app".to_owned()))
        );
    }
}
