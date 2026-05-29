use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;

use super::{
    AgentContext, AgentToolServices, AgentTurnStatus, CompletionSignal, Result, Tool, ToolOutput,
    object_schema, parse_args, string_enum_property, string_property,
};

pub(super) struct CompleteTaskTool;

#[derive(Deserialize)]
struct CompleteTaskArgs {
    summary: String,
    status: Option<String>,
}

#[async_trait]
impl Tool for CompleteTaskTool {
    fn name(&self) -> &'static str {
        "complete_task"
    }

    fn description(&self) -> &'static str {
        "Signal that the agent has completed, partially completed, or is blocked on the task."
    }

    fn parameters(&self) -> Value {
        object_schema(vec![
            string_property("summary", "Summary of what was accomplished"),
            string_enum_property(
                "status",
                "success, partial, or blocked",
                &["success", "partial", "blocked"],
            )
            .optional(),
        ])
    }

    async fn execute(
        &self,
        args: Value,
        _dbconfig_id: &str,
        _context: &AgentContext,
        _services: Arc<dyn AgentToolServices>,
    ) -> Result<ToolOutput> {
        let args: CompleteTaskArgs = parse_args(args)?;
        let status = match args.status.as_deref() {
            Some("partial") => AgentTurnStatus::Partial,
            Some("blocked") => AgentTurnStatus::Blocked,
            _ => AgentTurnStatus::Completed,
        };
        Ok(ToolOutput {
            content: args.summary.clone(),
            completion: Some(CompletionSignal {
                status,
                summary: args.summary,
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
    async fn completes_success_by_default() {
        let output = CompleteTaskTool
            .execute(
                json!({"summary": "done"}),
                "local",
                &context(),
                Arc::new(MockServices::default()),
            )
            .await
            .unwrap();

        let completion = output.completion.unwrap();
        assert_eq!(output.content, "done");
        assert_eq!(completion.status, AgentTurnStatus::Completed);
        assert_eq!(completion.summary, "done");
    }

    #[tokio::test]
    async fn maps_partial_and_blocked_statuses() {
        for (status, expected) in [
            ("partial", AgentTurnStatus::Partial),
            ("blocked", AgentTurnStatus::Blocked),
        ] {
            let output = CompleteTaskTool
                .execute(
                    json!({"summary": status, "status": status}),
                    "local",
                    &context(),
                    Arc::new(MockServices::default()),
                )
                .await
                .unwrap();

            assert_eq!(output.completion.unwrap().status, expected);
        }
    }
}
