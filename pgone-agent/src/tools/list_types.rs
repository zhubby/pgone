use std::sync::Arc;

use async_trait::async_trait;
use pgone_mcp::core::models::TypeKind;
use serde::Deserialize;
use serde_json::Value;

use super::{
    AgentContext, AgentToolServices, Result, Tool, ToolOutput, database_name_property, json_output,
    object_schema, parse_args, string_enum_property, string_property, target_database,
};

pub(super) struct ListTypesTool;

#[derive(Deserialize)]
struct ListTypesArgs {
    database_name: Option<String>,
    schema: Option<String>,
    kind: Option<String>,
}

#[async_trait]
impl Tool for ListTypesTool {
    fn name(&self) -> &'static str {
        "list_types"
    }

    fn description(&self) -> &'static str {
        "List PostgreSQL type metadata such as enum, domain, composite, or base types."
    }

    fn parameters(&self) -> Value {
        object_schema(vec![
            database_name_property().optional(),
            string_property("schema", "Schema name").optional(),
            string_enum_property(
                "kind",
                "Type kind",
                &["enum", "domain", "composite", "base"],
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
        let args: ListTypesArgs = parse_args(args)?;
        let types = services
            .list_types(
                dbconfig_id,
                target_database(args.database_name.as_deref(), context),
                args.schema.as_deref(),
                type_kind(args.kind.as_deref()),
            )
            .await?;
        json_output(&types)
    }
}

fn type_kind(value: Option<&str>) -> Option<TypeKind> {
    match value {
        Some("enum") => Some(TypeKind::Enum),
        Some("domain") => Some(TypeKind::Domain),
        Some("composite") => Some(TypeKind::Composite),
        Some("base") => Some(TypeKind::Base),
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
    fn converts_supported_type_kinds() {
        assert_eq!(type_kind(Some("enum")), Some(TypeKind::Enum));
        assert_eq!(type_kind(Some("domain")), Some(TypeKind::Domain));
        assert_eq!(type_kind(Some("composite")), Some(TypeKind::Composite));
        assert_eq!(type_kind(Some("base")), Some(TypeKind::Base));
        assert_eq!(type_kind(Some("unknown")), None);
        assert_eq!(type_kind(None), None);
    }

    #[tokio::test]
    async fn executes_against_context_database() {
        let services = Arc::new(MockServices::default());
        let output = ListTypesTool
            .execute(
                json!({"schema": "public", "kind": "enum"}),
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
