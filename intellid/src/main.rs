use intellid_mcp_server::registry::{ConnectionRegistry, ConnectionConfig, DatabaseEngine};
use intellid_mcp_server::adapters::postgres::PostgresIntrospector;
use intellid_mcp_server::core::introspector::DatabaseIntrospector;
use intellid_mcp_server::mcp;
use intellid_mcp_server::config;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let registry = ConnectionRegistry::new();

    // 占位：如果存在环境变量 DSN，则注册一个默认连接，便于快速运行
    if let Ok(path) = std::env::var("INTELLID_CONNECTIONS_PATH") {
        if let Ok(conns) = config::load_connections_from_path(&path) {
            for c in conns { let _ = registry.register(c).await; }
        }
    }

    if std::env::var("INTELLID_MCP_STDIO").is_ok() {
        // MCP STDIO 模式
        mcp::run_stdio(registry).await?;
    } else if let Ok(dsn) = std::env::var("INTELLID_PG_DSN") {
        registry.register(ConnectionConfig {
            id: "default".to_string(),
            engine: DatabaseEngine::Postgres,
            dsn,
            default_schemas: None,
            include_system: Some(false),
        }).await?;

        if let Some(handle) = registry.get("default").await {
            let pg = PostgresIntrospector::new(handle.pool.clone());
            let schema = pg.introspect_database(intellid_mcp_server::core::models::IntrospectOptions {
                schemas: None,
                with_indexes: true,
                with_routines: false,
                with_types: false,
                with_triggers: false,
                page: None,
                page_size: None,
            }).await?;
            println!("{}", serde_json::to_string_pretty(&schema)?);
        }
    } else {
        println!("intellid-mcp-server started. Set INTELLID_PG_DSN to run a quick introspection.");
    }

    Ok(())
}
