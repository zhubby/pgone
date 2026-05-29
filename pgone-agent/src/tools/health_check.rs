use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;

use super::{
    AgentContext, AgentToolServices, Result, Tool, ToolOutput, database_name_property, json_output,
    object_schema, parse_args, target_database,
};

pub(super) struct HealthCheckTool;

#[derive(Deserialize)]
struct HealthCheckArgs {
    database_name: Option<String>,
}

#[async_trait]
impl Tool for HealthCheckTool {
    fn name(&self) -> &'static str {
        "health_check"
    }

    fn description(&self) -> &'static str {
        "Check whether the selected PostgreSQL instance and target database can be reached."
    }

    fn parameters(&self) -> Value {
        object_schema(vec![database_name_property().optional()])
    }

    async fn execute(
        &self,
        args: Value,
        dbconfig_id: &str,
        context: &AgentContext,
        services: Arc<dyn AgentToolServices>,
    ) -> Result<ToolOutput> {
        let args: HealthCheckArgs = parse_args(args)?;
        json_output(
            &services
                .health_check(
                    dbconfig_id,
                    target_database(args.database_name.as_deref(), context),
                )
                .await?,
        )
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
        let output = HealthCheckTool
            .execute(json!({}), "local", &context(), services.clone())
            .await
            .unwrap();

        assert!(output.content.contains("\"ok\": true"));
        assert_eq!(
            services.seen_database_name().await,
            Some(Some("app".to_owned()))
        );
    }
}
