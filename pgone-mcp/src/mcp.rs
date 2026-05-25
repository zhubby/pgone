use crate::adapter::SqlSessionIntrospector;
use crate::core::models::{IntrospectOptions, RoutineKind, TypeKind};
use crate::formatters::{dbml, markdown, mermaid};
use pgone_sql::Session;
use pgone_storage::service::StorageService;
use rmcp::handler::server::ServerHandler;
use rmcp::model::ErrorData as McpError;
use rmcp::model::{CallToolRequestParam, CallToolResult, ListToolsResult, Tool};
use rmcp::service::{RequestContext, RoleServer};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut sigterm = match signal(SignalKind::terminate()) {
            Ok(sigterm) => Some(sigterm),
            Err(e) => {
                tracing::warn!("注册 SIGTERM 关闭信号失败: {}", e);
                None
            }
        };

        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                if let Err(e) = result {
                    tracing::warn!("监听 Ctrl+C 关闭信号失败: {}", e);
                }
            }
            _ = async {
                if let Some(sigterm) = sigterm.as_mut() {
                    sigterm.recv().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {}
        }
    }

    #[cfg(not(unix))]
    {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::warn!("监听 Ctrl+C 关闭信号失败: {}", e);
        }
    }

    tracing::info!("收到关闭信号");
}

/// MCP 服务器上下文
#[derive(Clone)]
pub struct McpContext {
    storage: Arc<RwLock<StorageService>>,
}

impl McpContext {
    pub async fn new(storage_path: PathBuf) -> anyhow::Result<Self> {
        let storage = StorageService::open_local(storage_path.to_str().unwrap()).await?;
        Ok(Self {
            storage: Arc::new(RwLock::new(storage)),
        })
    }

    async fn get_session(&self, dbconfig_id: &str) -> anyhow::Result<Session> {
        let storage = self.storage.read().await;
        let config = storage
            .get_db_config(dbconfig_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Database config not found: {}", dbconfig_id))?;
        Session::new(&config.dsn)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create session: {}", e))
    }
}

/// MCP 服务器实现
#[derive(Clone)]
pub struct PgoneMcpServer {
    context: McpContext,
    dbconfig_id: String,
}

impl PgoneMcpServer {
    pub async fn new(dbconfig_id: String) -> anyhow::Result<Self> {
        let storage_path = pgone_storage::database_path();
        Self::with_path(storage_path, dbconfig_id).await
    }

    pub async fn with_path(storage_path: PathBuf, dbconfig_id: String) -> anyhow::Result<Self> {
        Ok(Self {
            context: McpContext::new(storage_path).await?,
            dbconfig_id,
        })
    }

    fn create_tool(name: &'static str, description: &'static str, schema: Value) -> Tool {
        let schema_obj = schema
            .as_object()
            .expect("Schema must be an object")
            .clone();
        Tool {
            name: name.into(),
            description: Some(description.into()),
            title: None,
            input_schema: Arc::new(schema_obj),
            output_schema: None,
            annotations: Default::default(),
            icons: Default::default(),
            meta: Default::default(),
        }
    }
}

pub fn list_tools() -> Vec<Tool> {
    vec![
        PgoneMcpServer::create_tool(
            "introspect_all",
            "自省整个数据库，返回表、视图、触发器、例程、类型等信息",
            json!({
                "type": "object",
                "properties": {
                    "schemas": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "要查询的 schema 列表，为空则查询所有"
                    },
                    "with_indexes": {
                        "type": "boolean",
                        "description": "是否包含索引信息",
                        "default": true
                    },
                    "with_routines": {
                        "type": "boolean",
                        "description": "是否包含例程（函数/过程）",
                        "default": false
                    },
                    "with_types": {
                        "type": "boolean",
                        "description": "是否包含类型信息",
                        "default": false
                    },
                    "with_triggers": {
                        "type": "boolean",
                        "description": "是否包含触发器",
                        "default": false
                    },
                    "page": {
                        "type": "number",
                        "description": "分页页码（从1开始）"
                    },
                    "page_size": {
                        "type": "number",
                        "description": "每页大小"
                    },
                    "format": {
                        "type": "string",
                        "enum": ["json", "markdown"],
                        "description": "输出格式",
                        "default": "json"
                    }
                }
            }),
        ),
        PgoneMcpServer::create_tool(
            "get_table",
            "获取指定表的详细信息",
            json!({
                "type": "object",
                "properties": {
                    "schema": {
                        "type": "string",
                        "description": "Schema 名称"
                    },
                    "table": {
                        "type": "string",
                        "description": "表名"
                    },
                    "format": {
                        "type": "string",
                        "enum": ["json", "markdown"],
                        "description": "输出格式",
                        "default": "json"
                    }
                },
                "required": ["schema", "table"]
            }),
        ),
        PgoneMcpServer::create_tool(
            "list_triggers",
            "列出触发器",
            json!({
                "type": "object",
                "properties": {
                    "schema": {
                        "type": "string",
                        "description": "Schema 名称，为空则查询所有"
                    }
                }
            }),
        ),
        PgoneMcpServer::create_tool(
            "list_routines",
            "列出例程（函数/过程/聚合）",
            json!({
                "type": "object",
                "properties": {
                    "schema": {
                        "type": "string",
                        "description": "Schema 名称，为空则查询所有"
                    },
                    "kind": {
                        "type": "string",
                        "enum": ["function", "procedure", "aggregate"],
                        "description": "例程类型"
                    }
                }
            }),
        ),
        PgoneMcpServer::create_tool(
            "list_types",
            "列出类型（枚举/域/复合）",
            json!({
                "type": "object",
                "properties": {
                    "schema": {
                        "type": "string",
                        "description": "Schema 名称，为空则查询所有"
                    },
                    "kind": {
                        "type": "string",
                        "enum": ["enum", "domain", "composite", "base"],
                        "description": "类型"
                    }
                }
            }),
        ),
        PgoneMcpServer::create_tool(
            "render_er",
            "生成 ER 图（Mermaid 格式）",
            json!({
                "type": "object",
                "properties": {
                    "schemas": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "要查询的 schema 列表，为空则查询所有"
                    }
                }
            }),
        ),
        PgoneMcpServer::create_tool(
            "render_dbml",
            "生成 DBML 格式",
            json!({
                "type": "object",
                "properties": {
                    "schemas": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "要查询的 schema 列表，为空则查询所有"
                    }
                }
            }),
        ),
        PgoneMcpServer::create_tool("health_check", "检查数据库连接健康状态", json!({})),
    ]
}

impl ServerHandler for PgoneMcpServer {
    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: list_tools(),
            next_cursor: None,
        })
    }

    fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        let context = self.context.clone();
        async move {
            let tool_name = request.name.as_ref();
            let args_value = request
                .arguments
                .map(serde_json::Value::Object)
                .unwrap_or_default();

            let result = match tool_name {
                "introspect_all" => {
                    PgoneMcpServer {
                        context: context.clone(),
                        dbconfig_id: self.dbconfig_id.clone(),
                    }
                    .handle_introspect_all(args_value.clone())
                    .await
                }
                "get_table" => {
                    PgoneMcpServer {
                        context: context.clone(),
                        dbconfig_id: self.dbconfig_id.clone(),
                    }
                    .handle_get_table(args_value.clone())
                    .await
                }
                "list_triggers" => {
                    PgoneMcpServer {
                        context: context.clone(),
                        dbconfig_id: self.dbconfig_id.clone(),
                    }
                    .handle_list_triggers(args_value.clone())
                    .await
                }
                "list_routines" => {
                    PgoneMcpServer {
                        context: context.clone(),
                        dbconfig_id: self.dbconfig_id.clone(),
                    }
                    .handle_list_routines(args_value.clone())
                    .await
                }
                "list_types" => {
                    PgoneMcpServer {
                        context: context.clone(),
                        dbconfig_id: self.dbconfig_id.clone(),
                    }
                    .handle_list_types(args_value.clone())
                    .await
                }
                "render_er" => {
                    PgoneMcpServer {
                        context: context.clone(),
                        dbconfig_id: self.dbconfig_id.clone(),
                    }
                    .handle_render_er(args_value.clone())
                    .await
                }
                "render_dbml" => {
                    PgoneMcpServer {
                        context: context.clone(),
                        dbconfig_id: self.dbconfig_id.clone(),
                    }
                    .handle_render_dbml(args_value.clone())
                    .await
                }
                "health_check" => {
                    PgoneMcpServer {
                        context: context.clone(),
                        dbconfig_id: self.dbconfig_id.clone(),
                    }
                    .handle_health_check(args_value)
                    .await
                }
                _ => Err(anyhow::anyhow!("Unknown tool: {}", tool_name)),
            };

            match result {
                Ok(value) => {
                    let text = serde_json::to_string(&value).map_err(|e| {
                        let msg = e.to_string();
                        McpError::internal_error(msg, None)
                    })?;
                    Ok(CallToolResult {
                        content: vec![rmcp::model::Annotated {
                            raw: rmcp::model::RawContent::Text(rmcp::model::RawTextContent {
                                text,
                                meta: Default::default(),
                            }),
                            annotations: Default::default(),
                        }],
                        is_error: Some(false),
                        meta: Default::default(),
                        structured_content: Default::default(),
                    })
                }
                Err(e) => Ok(CallToolResult {
                    content: vec![rmcp::model::Annotated {
                        raw: rmcp::model::RawContent::Text(rmcp::model::RawTextContent {
                            text: format!("Error: {}", e),
                            meta: Default::default(),
                        }),
                        annotations: Default::default(),
                    }],
                    is_error: Some(true),
                    meta: Default::default(),
                    structured_content: Default::default(),
                }),
            }
        }
    }
}

impl PgoneMcpServer {
    async fn handle_introspect_all(&self, args: Value) -> anyhow::Result<Value> {
        #[derive(Deserialize)]
        struct Params {
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
            format: Option<String>,
        }

        fn default_true() -> bool {
            true
        }

        let params: Params = serde_json::from_value(args)?;
        let session = self.context.get_session(&self.dbconfig_id).await?;
        let introspector = SqlSessionIntrospector::new(session);

        let opts = IntrospectOptions {
            schemas: params.schemas,
            with_indexes: params.with_indexes,
            with_routines: params.with_routines,
            with_types: params.with_types,
            with_triggers: params.with_triggers,
            page: params.page,
            page_size: params.page_size,
        };

        let db = introspector.introspect_database(opts).await?;
        let want_markdown = matches!(params.format.as_deref(), Some("markdown"));

        if want_markdown {
            let mut md = markdown::render_overview(&db);
            if params.with_triggers {
                let triggers = introspector.list_triggers(None).await?;
                if !triggers.is_empty() {
                    md.push_str("\n触发器：\n");
                    for t in triggers {
                        md.push_str(&format!(
                            "- {}.{} [{} {}]\n",
                            t.schema,
                            t.name,
                            t.timing,
                            t.events.join("/")
                        ));
                    }
                }
            }
            if params.with_routines {
                let routines = introspector.list_routines(None, None).await?;
                if !routines.is_empty() {
                    md.push_str("\n例程：\n");
                    for r in routines {
                        md.push_str(&format!("- {}.{} ({:?})\n", r.schema, r.name, r.kind));
                    }
                }
            }
            if params.with_types {
                let types = introspector.list_types(None, None).await?;
                if !types.is_empty() {
                    md.push_str("\n类型：\n");
                    for t in types {
                        md.push_str(&format!("- {}.{} ({:?})\n", t.schema, t.name, t.kind));
                    }
                }
            }
            Ok(json!({"markdown": md}))
        } else {
            Ok(serde_json::to_value(&db)?)
        }
    }

    async fn handle_get_table(&self, args: Value) -> anyhow::Result<Value> {
        #[derive(Deserialize)]
        struct Params {
            schema: String,
            table: String,
            format: Option<String>,
        }

        let params: Params = serde_json::from_value(args)?;
        let session = self.context.get_session(&self.dbconfig_id).await?;
        let introspector = SqlSessionIntrospector::new(session);

        let table = introspector
            .get_table(&params.schema, &params.table)
            .await?;
        let want_markdown = matches!(params.format.as_deref(), Some("markdown"));

        if want_markdown {
            Ok(json!({"markdown": markdown::render_table_detail(&table)}))
        } else {
            Ok(serde_json::to_value(&table)?)
        }
    }

    async fn handle_list_triggers(&self, args: Value) -> anyhow::Result<Value> {
        #[derive(Deserialize)]
        struct Params {
            schema: Option<String>,
        }

        let params: Params = serde_json::from_value(args)?;
        let session = self.context.get_session(&self.dbconfig_id).await?;
        let introspector = SqlSessionIntrospector::new(session);

        let triggers = introspector.list_triggers(params.schema.as_deref()).await?;
        Ok(serde_json::to_value(triggers)?)
    }

    async fn handle_list_routines(&self, args: Value) -> anyhow::Result<Value> {
        #[derive(Deserialize)]
        struct Params {
            schema: Option<String>,
            kind: Option<String>,
        }

        let params: Params = serde_json::from_value(args)?;
        let session = self.context.get_session(&self.dbconfig_id).await?;
        let introspector = SqlSessionIntrospector::new(session);

        let kind = match params.kind.as_deref() {
            Some("function") => Some(RoutineKind::Function),
            Some("procedure") => Some(RoutineKind::Procedure),
            Some("aggregate") => Some(RoutineKind::Aggregate),
            _ => None,
        };

        let routines = introspector
            .list_routines(params.schema.as_deref(), kind)
            .await?;
        Ok(serde_json::to_value(routines)?)
    }

    async fn handle_list_types(&self, args: Value) -> anyhow::Result<Value> {
        #[derive(Deserialize)]
        struct Params {
            schema: Option<String>,
            kind: Option<String>,
        }

        let params: Params = serde_json::from_value(args)?;
        let session = self.context.get_session(&self.dbconfig_id).await?;
        let introspector = SqlSessionIntrospector::new(session);

        let kind = match params.kind.as_deref() {
            Some("enum") => Some(TypeKind::Enum),
            Some("domain") => Some(TypeKind::Domain),
            Some("composite") => Some(TypeKind::Composite),
            Some("base") => Some(TypeKind::Base),
            _ => None,
        };

        let types = introspector
            .list_types(params.schema.as_deref(), kind)
            .await?;
        Ok(serde_json::to_value(types)?)
    }

    async fn handle_render_er(&self, args: Value) -> anyhow::Result<Value> {
        #[derive(Deserialize)]
        struct Params {
            schemas: Option<Vec<String>>,
        }

        let params: Params = serde_json::from_value(args)?;
        let session = self.context.get_session(&self.dbconfig_id).await?;
        let introspector = SqlSessionIntrospector::new(session);

        let opts = IntrospectOptions {
            schemas: params.schemas,
            with_indexes: false,
            with_routines: false,
            with_types: false,
            with_triggers: false,
            page: None,
            page_size: None,
        };

        let db = introspector.introspect_database(opts).await?;
        Ok(json!({"mermaid": mermaid::render_er(&db)}))
    }

    async fn handle_render_dbml(&self, args: Value) -> anyhow::Result<Value> {
        #[derive(Deserialize)]
        struct Params {
            schemas: Option<Vec<String>>,
        }

        let params: Params = serde_json::from_value(args)?;
        let session = self.context.get_session(&self.dbconfig_id).await?;
        let introspector = SqlSessionIntrospector::new(session);

        let opts = IntrospectOptions {
            schemas: params.schemas,
            with_indexes: false,
            with_routines: false,
            with_types: false,
            with_triggers: false,
            page: None,
            page_size: None,
        };

        let db = introspector.introspect_database(opts).await?;
        Ok(json!({"dbml": dbml::render_dbml(&db)}))
    }

    async fn handle_health_check(&self, _args: Value) -> anyhow::Result<Value> {
        let session = self.context.get_session(&self.dbconfig_id).await?;

        // 执行简单查询来检查连接
        let _: String = session
            .current_database()
            .await
            .map_err(|e| anyhow::anyhow!("Health check failed: {}", e))?;
        Ok(json!({"ok": true}))
    }

    /// 直接调用工具（用于 GUI 等非 MCP 协议场景）
    pub async fn call_tool_direct(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        match tool_name {
            "introspect_all" => self.handle_introspect_all(arguments).await,
            "get_table" => self.handle_get_table(arguments).await,
            "list_triggers" => self.handle_list_triggers(arguments).await,
            "list_routines" => self.handle_list_routines(arguments).await,
            "list_types" => self.handle_list_types(arguments).await,
            "render_er" => self.handle_render_er(arguments).await,
            "render_dbml" => self.handle_render_dbml(arguments).await,
            "health_check" => self.handle_health_check(arguments).await,
            _ => Err(anyhow::anyhow!("Unknown tool: {}", tool_name)),
        }
    }
}

/// 运行 STDIO 模式的 MCP 服务器
pub async fn run_stdio(dbconfig_id: String) -> anyhow::Result<()> {
    use rmcp::handler::server::router::Router;
    use rmcp::service::serve_server;
    use rmcp::transport::async_rw::AsyncRwTransport;
    use tokio::io;

    let handler = PgoneMcpServer::new(dbconfig_id).await?;
    let service = Router::new(handler);
    let transport = AsyncRwTransport::new_server(io::stdin(), io::stdout());

    tracing::info!("MCP 服务器启动（STDIO 模式）");
    tokio::select! {
        result = serve_server(service, transport) => {
            result?;
            tracing::info!("MCP STDIO 服务已结束");
        }
        _ = wait_for_shutdown_signal() => {
            tracing::info!("开始关闭 MCP STDIO 服务");
        }
    }

    tracing::info!("MCP STDIO 服务已关闭");
    Ok(())
}

/// 运行 Streamable HTTP 模式的 MCP 服务器
pub async fn run_streamable(addr: &str, dbconfig_id: String) -> anyhow::Result<()> {
    use rmcp::transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    };
    use std::sync::Arc;

    // 先创建 handler（async 初始化）
    let handler = PgoneMcpServer::new(dbconfig_id).await?;
    let handler = Arc::new(handler);

    // 创建工厂函数，返回克隆的 handler
    let handler_clone = handler.clone();
    let factory =
        move || -> Result<PgoneMcpServer, std::io::Error> { Ok((*handler_clone).clone()) };

    // 创建 session manager 并包装在 Arc 中
    let session_manager = Arc::new(LocalSessionManager::default());

    // 创建 StreamableHttpService
    let service = StreamableHttpService::new(
        factory,
        session_manager,
        StreamableHttpServerConfig {
            stateful_mode: true,
            sse_keep_alive: None,
        },
    );

    // 创建 axum router
    let router = axum::Router::new().nest_service("/mcp", service);

    // 绑定地址
    let tcp_listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("MCP 服务器启动（Streamable HTTP 模式）");
    tracing::info!("监听地址: {}", addr);

    // 启动服务器
    let _ = axum::serve(tcp_listener, router)
        .with_graceful_shutdown(wait_for_shutdown_signal())
        .await;

    tracing::info!("MCP Streamable HTTP 服务已关闭");
    Ok(())
}
