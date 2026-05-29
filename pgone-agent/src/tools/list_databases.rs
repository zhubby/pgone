use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use super::{
    AgentContext, AgentToolServices, Result, Tool, ToolOutput, json_output, object_schema,
};

pub(super) struct ListDatabasesTool;

#[async_trait]
impl Tool for ListDatabasesTool {
    fn name(&self) -> &'static str {
        "list_databases"
    }

    fn description(&self) -> &'static str {
        "List databases available on the selected PostgreSQL instance."
    }

    fn parameters(&self) -> Value {
        object_schema(vec![])
    }

    async fn execute(
        &self,
        _args: Value,
        dbconfig_id: &str,
        _context: &AgentContext,
        services: Arc<dyn AgentToolServices>,
    ) -> Result<ToolOutput> {
        json_output(&services.list_databases(dbconfig_id).await?)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use super::*;
    use crate::tools::test_support::{MockServices, context};

    #[tokio::test]
    async fn returns_available_databases() {
        let output = ListDatabasesTool
            .execute(
                json!({}),
                "local",
                &context(),
                Arc::new(MockServices::default()),
            )
            .await
            .unwrap();

        assert!(output.content.contains("\"name\": \"app\""));
        assert!(output.completion.is_none());
    }
}
