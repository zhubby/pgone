use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    AgentContext, AgentToolServices, Result, Tool, ToolOutput, array_string_property,
    database_name_property, json_output, object_schema, parse_args, target_database,
};

pub(super) struct RenderDbmlTool;

#[derive(Deserialize)]
struct RenderDbmlArgs {
    database_name: Option<String>,
    schemas: Option<Vec<String>>,
}

#[async_trait]
impl Tool for RenderDbmlTool {
    fn name(&self) -> &'static str {
        "render_dbml"
    }

    fn description(&self) -> &'static str {
        "Render the selected PostgreSQL schemas as DBML."
    }

    fn parameters(&self) -> Value {
        object_schema(vec![
            database_name_property().optional(),
            array_string_property(
                "schemas",
                "Schema names to render; omit to render all user schemas",
            )
            .optional(),
        ])
    }

    async fn execute(
        &self,
        args: Value,
        dbconfig_id: &str,
        context: &AgentContext,
        services: Arc<dyn AgentToolServices>,
    ) -> Result<ToolOutput> {
        let args: RenderDbmlArgs = parse_args(args)?;
        let diagram = services
            .render_dbml(
                dbconfig_id,
                target_database(args.database_name.as_deref(), context),
                args.schemas,
            )
            .await?;
        json_output(&json!({ "dbml": diagram.content }))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use super::*;
    use crate::tools::test_support::{MockServices, context};

    #[tokio::test]
    async fn returns_dbml_key() {
        let services = Arc::new(MockServices::default());
        let output = RenderDbmlTool
            .execute(
                json!({"schemas": ["public"]}),
                "local",
                &context(),
                services.clone(),
            )
            .await
            .unwrap();

        assert!(
            output
                .content
                .contains("\"dbml\": \"Table public.users {}\"")
        );
        assert_eq!(
            services.seen_database_name().await,
            Some(Some("app".to_owned()))
        );
    }
}
