use std::sync::Arc;

use async_trait::async_trait;
use pgone_mcp::core::models::{IntrospectOptions, RoutineKind, TypeKind};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlparser::ast::Statement;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

use crate::provider::ToolDefinition;
use crate::{
    AgentContext, AgentError, AgentEvent, AgentToolCallSummary, AgentToolServices, AgentTurnStatus,
    ReadonlySqlRequest, Result,
};

#[derive(Clone, Debug)]
pub struct ToolOutput {
    pub content: String,
    pub completion: Option<CompletionSignal>,
}

#[derive(Clone, Debug)]
pub struct CompletionSignal {
    pub status: AgentTurnStatus,
    pub summary: String,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn parameters(&self) -> Value;

    async fn execute(
        &self,
        args: Value,
        dbconfig_id: &str,
        context: &AgentContext,
        services: Arc<dyn AgentToolServices>,
    ) -> Result<ToolOutput>;

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_owned(),
            description: self.description().to_owned(),
            parameters: self.parameters(),
        }
    }
}

#[derive(Clone)]
pub struct ToolRegistry {
    tools: Vec<Arc<dyn Tool>>,
}

impl ToolRegistry {
    #[must_use]
    pub fn pgone_readonly() -> Self {
        Self {
            tools: vec![
                Arc::new(HealthCheckTool),
                Arc::new(ListDatabasesTool),
                Arc::new(IntrospectDatabaseTool),
                Arc::new(GetTableTool),
                Arc::new(ListTriggersTool),
                Arc::new(ListRoutinesTool),
                Arc::new(ListTypesTool),
                Arc::new(ExecuteReadonlySqlTool),
                Arc::new(RenderErTool),
                Arc::new(RenderDbmlTool),
                Arc::new(CompleteTaskTool),
            ],
        }
    }

    #[must_use]
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.iter().map(|tool| tool.definition()).collect()
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.iter().find(|tool| tool.name() == name).cloned()
    }
}

pub struct ToolExecutionRecord {
    pub summary: AgentToolCallSummary,
    pub event: AgentEvent,
    pub output: Option<ToolOutput>,
}

impl ToolExecutionRecord {
    pub fn success(name: String, arguments: Value, result: String, output: ToolOutput) -> Self {
        Self {
            summary: AgentToolCallSummary {
                name: name.clone(),
                arguments,
                result: Some(result.clone()),
                error: None,
            },
            event: AgentEvent::ToolFinished { name, result },
            output: Some(output),
        }
    }

    pub fn failure(name: String, arguments: Value, error: String) -> Self {
        Self {
            summary: AgentToolCallSummary {
                name: name.clone(),
                arguments,
                result: None,
                error: Some(error.clone()),
            },
            event: AgentEvent::ToolFailed { name, error },
            output: None,
        }
    }
}

struct HealthCheckTool;

#[derive(Deserialize)]
struct DatabaseTargetArgs {
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
        let args: DatabaseTargetArgs = parse_args(args)?;
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

struct ListDatabasesTool;

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

struct IntrospectDatabaseTool;

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

struct GetTableTool;

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

struct ListTriggersTool;

#[derive(Deserialize)]
struct SchemaFilterArgs {
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
        let args: SchemaFilterArgs = parse_args(args)?;
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

struct ListRoutinesTool;

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

struct ListTypesTool;

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

struct ExecuteReadonlySqlTool;

#[derive(Deserialize)]
struct ExecuteReadonlySqlArgs {
    database_name: Option<String>,
    sql: String,
    max_rows: Option<u32>,
}

#[async_trait]
impl Tool for ExecuteReadonlySqlTool {
    fn name(&self) -> &'static str {
        "execute_readonly_sql"
    }

    fn description(&self) -> &'static str {
        "Execute one read-only PostgreSQL query such as SELECT, WITH, VALUES, or EXPLAIN and return bounded rows."
    }

    fn parameters(&self) -> Value {
        object_schema(vec![
            database_name_property().optional(),
            string_property("sql", "One read-only SQL statement to execute"),
            integer_property("max_rows", "Maximum rows to return; defaults to 100").optional(),
        ])
    }

    async fn execute(
        &self,
        args: Value,
        dbconfig_id: &str,
        context: &AgentContext,
        services: Arc<dyn AgentToolServices>,
    ) -> Result<ToolOutput> {
        let args: ExecuteReadonlySqlArgs = parse_args(args)?;
        validate_readonly_sql(&args.sql)?;
        let result = services
            .execute_readonly_sql(
                dbconfig_id,
                ReadonlySqlRequest {
                    sql: args.sql,
                    database_name: target_database(args.database_name.as_deref(), context)
                        .map(ToOwned::to_owned),
                    max_rows: args.max_rows.unwrap_or(100),
                    timeout_ms: 20_000,
                },
            )
            .await?;
        json_output(&result)
    }
}

struct RenderErTool;

#[derive(Deserialize)]
struct RenderArgs {
    database_name: Option<String>,
    schemas: Option<Vec<String>>,
}

#[async_trait]
impl Tool for RenderErTool {
    fn name(&self) -> &'static str {
        "render_er"
    }

    fn description(&self) -> &'static str {
        "Render the selected PostgreSQL schemas as a Mermaid ER diagram."
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
        let args: RenderArgs = parse_args(args)?;
        let diagram = services
            .render_er(
                dbconfig_id,
                target_database(args.database_name.as_deref(), context),
                args.schemas,
            )
            .await?;
        json_output(&json!({ "mermaid": diagram.content }))
    }
}

struct RenderDbmlTool;

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
        let args: RenderArgs = parse_args(args)?;
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

struct CompleteTaskTool;

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

fn parse_args<T>(args: Value) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(args)
        .map_err(|error| AgentError::Tool(format!("invalid agent tool arguments: {error}")))
}

fn json_output<T>(value: &T) -> Result<ToolOutput>
where
    T: serde::Serialize,
{
    let content =
        serde_json::to_string_pretty(value).map_err(|error| AgentError::Tool(error.to_string()))?;
    Ok(ToolOutput {
        content,
        completion: None,
    })
}

fn target_database<'a>(
    argument_database: Option<&'a str>,
    context: &'a AgentContext,
) -> Option<&'a str> {
    argument_database
        .filter(|database| !database.trim().is_empty())
        .or(context.database_name.as_deref())
        .filter(|database| !database.trim().is_empty())
}

pub fn validate_readonly_sql(sql: &str) -> Result<()> {
    let sql = sql.trim();
    if sql.is_empty() {
        return Err(AgentError::Tool("SQL is empty".to_owned()));
    }

    let dialect = PostgreSqlDialect {};
    let statements = Parser::parse_sql(&dialect, sql)
        .map_err(|error| AgentError::Tool(format!("invalid SQL: {error}")))?;
    if statements.len() != 1 {
        return Err(AgentError::Tool(
            "only one read-only SQL statement is allowed".to_owned(),
        ));
    }

    if statement_is_readonly(&statements[0]) {
        Ok(())
    } else {
        Err(AgentError::Tool(
            "only SELECT, WITH, VALUES, and EXPLAIN statements are allowed".to_owned(),
        ))
    }
}

fn statement_is_readonly(statement: &Statement) -> bool {
    match statement {
        Statement::Query(query) => query.locks.is_empty(),
        Statement::Explain {
            analyze, statement, ..
        } => !*analyze && statement_is_readonly(statement),
        _ => false,
    }
}

fn default_true() -> bool {
    true
}

fn routine_kind(value: Option<&str>) -> Option<RoutineKind> {
    match value {
        Some("function") => Some(RoutineKind::Function),
        Some("procedure") => Some(RoutineKind::Procedure),
        Some("aggregate") => Some(RoutineKind::Aggregate),
        _ => None,
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

#[derive(Clone)]
struct SchemaProperty {
    name: &'static str,
    schema: Value,
    required: bool,
}

impl SchemaProperty {
    fn optional(mut self) -> Self {
        self.required = false;
        self
    }
}

fn string_property(name: &'static str, description: &'static str) -> SchemaProperty {
    SchemaProperty {
        name,
        schema: json!({"type": "string", "description": description}),
        required: true,
    }
}

fn database_name_property() -> SchemaProperty {
    string_property(
        "database_name",
        "Target database name on the selected PostgreSQL instance; omit to use the current UI database",
    )
}

fn string_enum_property(
    name: &'static str,
    description: &'static str,
    values: &[&'static str],
) -> SchemaProperty {
    SchemaProperty {
        name,
        schema: json!({"type": "string", "enum": values, "description": description}),
        required: true,
    }
}

fn integer_property(name: &'static str, description: &'static str) -> SchemaProperty {
    SchemaProperty {
        name,
        schema: json!({"type": "integer", "description": description}),
        required: true,
    }
}

fn boolean_property(name: &'static str, description: &'static str) -> SchemaProperty {
    SchemaProperty {
        name,
        schema: json!({"type": "boolean", "description": description}),
        required: true,
    }
}

fn array_string_property(name: &'static str, description: &'static str) -> SchemaProperty {
    SchemaProperty {
        name,
        schema: json!({
            "type": "array",
            "items": {"type": "string"},
            "description": description
        }),
        required: true,
    }
}

fn object_schema(properties: Vec<SchemaProperty>) -> Value {
    let required = properties
        .iter()
        .filter(|property| property.required)
        .map(|property| property.name)
        .collect::<Vec<_>>();
    let properties = properties
        .into_iter()
        .map(|property| (property.name.to_owned(), property.schema))
        .collect::<serde_json::Map<_, _>>();

    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use pgone_mcp::core::models::{
        Column, DatabaseSchema, ForeignKey, Index, PrimaryKey, RoutineDetail, RoutineKind, Schema,
        TableDetail, TriggerDetail, TypeDetail, TypeKind, ViewDetail,
    };
    use pgone_sql::models::DatabaseInfo;
    use serde_json::json;
    use tokio::sync::Mutex;

    use super::*;
    use crate::{RenderedDiagram, Result};

    #[derive(Default)]
    struct MockServices {
        seen_database_name: Mutex<Option<Option<String>>>,
    }

    impl MockServices {
        async fn record_database_name(&self, database_name: Option<&str>) {
            *self.seen_database_name.lock().await = Some(database_name.map(ToOwned::to_owned));
        }

        async fn seen_database_name(&self) -> Option<Option<String>> {
            self.seen_database_name.lock().await.clone()
        }
    }

    #[async_trait]
    impl AgentToolServices for MockServices {
        async fn health_check(
            &self,
            _dbconfig_id: &str,
            database_name: Option<&str>,
        ) -> Result<Value> {
            self.record_database_name(database_name).await;
            Ok(json!({"ok": true, "database": "app"}))
        }

        async fn list_databases(&self, _dbconfig_id: &str) -> Result<Vec<DatabaseInfo>> {
            Ok(vec![DatabaseInfo {
                name: "app".to_owned(),
                owner: "postgres".to_owned(),
                encoding: "UTF8".to_owned(),
                collate: None,
                ctype: None,
                size: None,
                description: None,
            }])
        }

        async fn introspect_database(
            &self,
            _dbconfig_id: &str,
            database_name: Option<&str>,
            _opts: IntrospectOptions,
        ) -> Result<DatabaseSchema> {
            self.record_database_name(database_name).await;
            Ok(DatabaseSchema {
                database: "app".to_owned(),
                schemas: vec![Schema {
                    name: "public".to_owned(),
                    tables: vec![table_detail()],
                    views: Vec::<ViewDetail>::new(),
                }],
            })
        }

        async fn get_table(
            &self,
            _dbconfig_id: &str,
            database_name: Option<&str>,
            schema: &str,
            table: &str,
        ) -> Result<TableDetail> {
            self.record_database_name(database_name).await;
            assert_eq!(schema, "public");
            assert_eq!(table, "users");
            Ok(table_detail())
        }

        async fn list_triggers(
            &self,
            _dbconfig_id: &str,
            database_name: Option<&str>,
            _schema: Option<&str>,
        ) -> Result<Vec<TriggerDetail>> {
            self.record_database_name(database_name).await;
            Ok(Vec::new())
        }

        async fn list_routines(
            &self,
            _dbconfig_id: &str,
            database_name: Option<&str>,
            _schema: Option<&str>,
            _kind: Option<RoutineKind>,
        ) -> Result<Vec<RoutineDetail>> {
            self.record_database_name(database_name).await;
            Ok(Vec::new())
        }

        async fn list_types(
            &self,
            _dbconfig_id: &str,
            database_name: Option<&str>,
            _schema: Option<&str>,
            _kind: Option<TypeKind>,
        ) -> Result<Vec<TypeDetail>> {
            self.record_database_name(database_name).await;
            Ok(Vec::new())
        }

        async fn render_er(
            &self,
            _dbconfig_id: &str,
            database_name: Option<&str>,
            _schemas: Option<Vec<String>>,
        ) -> Result<RenderedDiagram> {
            self.record_database_name(database_name).await;
            Ok(RenderedDiagram {
                content: "erDiagram".to_owned(),
            })
        }

        async fn render_dbml(
            &self,
            _dbconfig_id: &str,
            database_name: Option<&str>,
            _schemas: Option<Vec<String>>,
        ) -> Result<RenderedDiagram> {
            self.record_database_name(database_name).await;
            Ok(RenderedDiagram {
                content: "Table public.users {}".to_owned(),
            })
        }

        async fn execute_readonly_sql(
            &self,
            _dbconfig_id: &str,
            request: ReadonlySqlRequest,
        ) -> Result<crate::ReadonlySqlResult> {
            self.record_database_name(request.database_name.as_deref())
                .await;
            assert!(request.database_name.is_some());
            Ok(crate::ReadonlySqlResult {
                columns: vec!["id".to_owned()],
                rows: vec![vec!["1".to_owned()]],
                row_count: 1,
                truncated: false,
                explain: Some("Result".to_owned()),
            })
        }
    }

    fn table_detail() -> TableDetail {
        TableDetail {
            schema: "public".to_owned(),
            name: "users".to_owned(),
            comment: None,
            columns: vec![Column {
                name: "id".to_owned(),
                data_type: "integer".to_owned(),
                udt_name: None,
                nullable: false,
                default: None,
                character_maximum_length: None,
                numeric_precision: None,
                numeric_scale: None,
                comment: None,
            }],
            primary_key: Some(PrimaryKey {
                columns: vec!["id".to_owned()],
            }),
            foreign_keys: Vec::<ForeignKey>::new(),
            indexes: Vec::<Index>::new(),
        }
    }

    fn context() -> AgentContext {
        AgentContext {
            dbconfig_id: Some("local".to_owned()),
            database_name: Some("app".to_owned()),
            selected_schema: Some("public".to_owned()),
            selected_table: Some("users".to_owned()),
        }
    }

    #[test]
    fn registry_exposes_database_metadata_tools() {
        let definitions = ToolRegistry::pgone_readonly()
            .definitions()
            .into_iter()
            .map(|definition| definition.name)
            .collect::<Vec<_>>();

        assert!(definitions.contains(&"health_check".to_owned()));
        assert!(definitions.contains(&"list_databases".to_owned()));
        assert!(definitions.contains(&"introspect_database".to_owned()));
        assert!(definitions.contains(&"get_table".to_owned()));
        assert!(definitions.contains(&"list_triggers".to_owned()));
        assert!(definitions.contains(&"list_routines".to_owned()));
        assert!(definitions.contains(&"list_types".to_owned()));
        assert!(definitions.contains(&"render_er".to_owned()));
        assert!(definitions.contains(&"render_dbml".to_owned()));
        assert!(definitions.contains(&"complete_task".to_owned()));
    }

    #[test]
    fn get_table_schema_marks_schema_and_table_required() {
        let definition = ToolRegistry::pgone_readonly()
            .get("get_table")
            .unwrap()
            .definition();
        assert_eq!(
            definition.parameters["required"],
            json!(["schema", "table"])
        );
        assert_eq!(definition.parameters["additionalProperties"], json!(false));
    }

    #[tokio::test]
    async fn get_table_executes_against_services() {
        let tool = ToolRegistry::pgone_readonly().get("get_table").unwrap();
        let services = Arc::new(MockServices::default());
        let output = tool
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
    async fn introspect_database_uses_default_index_flag() {
        let tool = ToolRegistry::pgone_readonly()
            .get("introspect_database")
            .unwrap();
        let output = tool
            .execute(
                json!({}),
                "local",
                &context(),
                Arc::new(MockServices::default()),
            )
            .await
            .unwrap();

        assert!(output.content.contains("\"database\": \"app\""));
    }

    #[tokio::test]
    async fn metadata_tool_database_argument_overrides_context_database() {
        let tool = ToolRegistry::pgone_readonly()
            .get("introspect_database")
            .unwrap();
        let services = Arc::new(MockServices::default());

        tool.execute(
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

    #[tokio::test]
    async fn metadata_tool_without_database_context_falls_back_to_config_dsn() {
        let tool = ToolRegistry::pgone_readonly().get("get_table").unwrap();
        let services = Arc::new(MockServices::default());
        let mut context = context();
        context.database_name = None;

        tool.execute(
            json!({"schema": "public", "table": "users"}),
            "local",
            &context,
            services.clone(),
        )
        .await
        .unwrap();

        assert_eq!(services.seen_database_name().await, Some(None));
    }

    #[tokio::test]
    async fn list_databases_executes_against_instance_services() {
        let tool = ToolRegistry::pgone_readonly()
            .get("list_databases")
            .unwrap();
        let output = tool
            .execute(
                json!({}),
                "local",
                &context(),
                Arc::new(MockServices::default()),
            )
            .await
            .unwrap();

        assert!(output.content.contains("\"name\": \"app\""));
    }

    #[tokio::test]
    async fn render_er_returns_mermaid_key() {
        let tool = ToolRegistry::pgone_readonly().get("render_er").unwrap();
        let output = tool
            .execute(
                json!({"schemas": ["public"]}),
                "local",
                &context(),
                Arc::new(MockServices::default()),
            )
            .await
            .unwrap();

        assert!(output.content.contains("\"mermaid\": \"erDiagram\""));
    }

    #[test]
    fn readonly_sql_validation_allows_safe_statements() {
        for sql in [
            "SELECT * FROM users",
            "WITH recent AS (SELECT * FROM users) SELECT * FROM recent",
            "VALUES (1), (2)",
            "EXPLAIN SELECT * FROM users",
        ] {
            validate_readonly_sql(sql).unwrap();
        }
    }

    #[test]
    fn readonly_sql_validation_rejects_mutating_statements() {
        for sql in [
            "INSERT INTO users (id) VALUES (1)",
            "UPDATE users SET id = 1",
            "DELETE FROM users",
            "CREATE TABLE users (id int)",
            "DROP TABLE users",
            "ALTER TABLE users ADD COLUMN name text",
            "COPY users TO STDOUT",
            "CALL refresh_users()",
            "SET search_path TO public",
            "SELECT 1; SELECT 2",
            "SELECT * FROM users FOR UPDATE",
            "EXPLAIN ANALYZE SELECT * FROM users",
        ] {
            assert!(validate_readonly_sql(sql).is_err(), "{sql}");
        }
    }

    #[tokio::test]
    async fn execute_readonly_sql_executes_against_services_with_context_database() {
        let tool = ToolRegistry::pgone_readonly()
            .get("execute_readonly_sql")
            .unwrap();
        let services = Arc::new(MockServices::default());
        let output = tool
            .execute(
                json!({"sql": "SELECT 1", "max_rows": 1}),
                "local",
                &context(),
                services.clone(),
            )
            .await
            .unwrap();

        assert!(output.content.contains("\"columns\""));
        assert!(output.content.contains("\"truncated\": false"));
        assert_eq!(
            services.seen_database_name().await,
            Some(Some("app".to_owned()))
        );
    }
}
