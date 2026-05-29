use std::sync::Arc;

use async_trait::async_trait;
use pgone_mcp::core::models::IntrospectOptions;
use serde::Deserialize;
use serde_json::Value;

use super::{
    AgentContext, AgentToolServices, Result, Tool, ToolOutput, array_string_property,
    boolean_property, database_name_property, default_true, integer_property, json_output,
    object_schema, parse_args, target_database,
};

pub(super) struct IntrospectDatabaseTool;

#[derive(Deserialize)]
struct IntrospectDatabaseArgs {
    database_name: Option<String>,
    schemas: Option<Vec<String>>,
    #[serde(default = "default_true")]
    with_indexes: bool,
    #[serde(default)]
    with_routines: bool,
    #[serde(default)]
    with_types: bool,
    #[serde(default)]
    with_triggers: bool,
    page: Option<u32>,
    page_size: Option<u32>,
}

#[async_trait]
impl Tool for IntrospectDatabaseTool {
    fn name(&self) -> &'static str {
        "introspect_database"
    }

    fn description(&self) -> &'static str {
        "Read a PostgreSQL database metadata overview including schemas, tables, views, columns, keys, and optional indexes."
    }

    fn parameters(&self) -> Value {
        object_schema(vec![
            database_name_property().optional(),
            array_string_property(
                "schemas",
                "Schema names to inspect; omit to inspect all user schemas",
            )
            .optional(),
            boolean_property("with_indexes", "Include index metadata; defaults to true").optional(),
            boolean_property(
                "with_routines",
                "Include routine metadata; defaults to false",
            )
            .optional(),
            boolean_property("with_types", "Include type metadata; defaults to false").optional(),
            boolean_property(
                "with_triggers",
                "Include trigger metadata; defaults to false",
            )
            .optional(),
            integer_property("page", "Page number starting at 1").optional(),
            integer_property("page_size", "Number of records per page").optional(),
        ])
    }

    async fn execute(
        &self,
        args: Value,
        dbconfig_id: &str,
        context: &AgentContext,
        services: Arc<dyn AgentToolServices>,
    ) -> Result<ToolOutput> {
        let args: IntrospectDatabaseArgs = parse_args(args)?;
        let db = services
            .introspect_database(
                dbconfig_id,
                target_database(args.database_name.as_deref(), context),
                IntrospectOptions {
                    schemas: args.schemas,
                    with_indexes: args.with_indexes,
                    with_routines: args.with_routines,
                    with_types: args.with_types,
                    with_triggers: args.with_triggers,
                    page: args.page,
                    page_size: args.page_size,
                },
            )
            .await?;
        json_output(&db)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use super::*;
    use crate::tools::test_support::{MockServices, context};

    #[tokio::test]
    async fn uses_default_index_flag_and_context_database() {
        let services = Arc::new(MockServices::default());
        let output = IntrospectDatabaseTool
            .execute(json!({}), "local", &context(), services.clone())
            .await
            .unwrap();

        assert!(output.content.contains("\"database\": \"app\""));
        assert_eq!(
            services.seen_database_name().await,
            Some(Some("app".to_owned()))
        );
    }

    #[tokio::test]
    async fn database_argument_overrides_context_database() {
        let services = Arc::new(MockServices::default());

        IntrospectDatabaseTool
            .execute(
                json!({"database_name": "doro"}),
                "local",
                &context(),
                services.clone(),
            )
            .await
            .unwrap();

        assert_eq!(
            services.seen_database_name().await,
            Some(Some("doro".to_owned()))
        );
    }
}
