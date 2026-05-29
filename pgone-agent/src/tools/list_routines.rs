use std::sync::Arc;

use async_trait::async_trait;
use pgone_mcp::core::models::RoutineKind;
use serde::Deserialize;
use serde_json::Value;

use super::{
    AgentContext, AgentToolServices, Result, Tool, ToolOutput, database_name_property, json_output,
    object_schema, parse_args, string_enum_property, string_property, target_database,
};

pub(super) struct ListRoutinesTool;

#[derive(Deserialize)]
struct ListRoutinesArgs {
    database_name: Option<String>,
    schema: Option<String>,
    kind: Option<String>,
}

#[async_trait]
impl Tool for ListRoutinesTool {
    fn name(&self) -> &'static str {
        "list_routines"
    }

    fn description(&self) -> &'static str {
        "List PostgreSQL routines such as functions, procedures, or aggregates."
    }

    fn parameters(&self) -> Value {
        object_schema(vec![
            database_name_property().optional(),
            string_property("schema", "Schema name").optional(),
            string_enum_property(
                "kind",
                "Routine kind",
                &["function", "procedure", "aggregate"],
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
        let args: ListRoutinesArgs = parse_args(args)?;
        let routines = services
            .list_routines(
                dbconfig_id,
                target_database(args.database_name.as_deref(), context),
                args.schema.as_deref(),
                routine_kind(args.kind.as_deref()),
            )
            .await?;
        json_output(&routines)
    }
}

fn routine_kind(value: Option<&str>) -> Option<RoutineKind> {
    match value {
        Some("function") => Some(RoutineKind::Function),
        Some("procedure") => Some(RoutineKind::Procedure),
        Some("aggregate") => Some(RoutineKind::Aggregate),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use super::*;
    use crate::tools::test_support::{MockServices, context};

    #[test]
    fn converts_supported_routine_kinds() {
        assert_eq!(routine_kind(Some("function")), Some(RoutineKind::Function));
        assert_eq!(
            routine_kind(Some("procedure")),
            Some(RoutineKind::Procedure)
        );
        assert_eq!(
            routine_kind(Some("aggregate")),
            Some(RoutineKind::Aggregate)
        );
        assert_eq!(routine_kind(Some("unknown")), None);
        assert_eq!(routine_kind(None), None);
    }

    #[tokio::test]
    async fn executes_against_context_database() {
        let services = Arc::new(MockServices::default());
        let output = ListRoutinesTool
            .execute(
                json!({"schema": "public", "kind": "function"}),
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
