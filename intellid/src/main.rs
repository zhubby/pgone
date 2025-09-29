use intellid_mcp_server::registry::{ConnectionRegistry, ConnectionConfig, DatabaseEngine};
use intellid_mcp_server::adapters::postgres::PostgresIntrospector;
use intellid_mcp_server::core::introspector::DatabaseIntrospector;
use intellid_mcp_server::mcp;
use intellid_mcp_server::config;
use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // 在后台线程启动 MCP（STDIO 或一键自省）
    let _mcp_thread = std::thread::spawn(|| {
        let rt = match tokio::runtime::Runtime::new() { Ok(rt) => rt, Err(e) => { eprintln!("tokio runtime error: {}", e); return; } };
        rt.block_on(async move {
            let registry = ConnectionRegistry::new();

            if let Ok(path) = std::env::var("INTELLID_CONNECTIONS_PATH") {
                if let Ok(conns) = config::load_connections_from_path(&path) {
                    for c in conns { let _ = registry.register(c).await; }
                }
            }

            if std::env::var("INTELLID_MCP_STDIO").is_ok() {
                let _ = mcp::run_stdio(registry).await;
            } else if let Ok(dsn) = std::env::var("INTELLID_PG_DSN") {
                if let Err(e) = registry.register(ConnectionConfig {
                    id: "default".to_string(),
                    engine: DatabaseEngine::Postgres,
                    dsn,
                    default_schemas: None,
                    include_system: Some(false),
                }).await { eprintln!("register error: {}", e); return; }

                if let Some(handle) = registry.get("default").await {
                    let pg = PostgresIntrospector::new(handle.pool.clone());
                    match pg.introspect_database(intellid_mcp_server::core::models::IntrospectOptions {
                        schemas: None,
                        with_indexes: true,
                        with_routines: false,
                        with_types: false,
                        with_triggers: false,
                        page: None,
                        page_size: None,
                    }).await {
                        Ok(schema) => {
                            match serde_json::to_string_pretty(&schema) {
                                Ok(s) => println!("{}", s),
                                Err(e) => eprintln!("serde error: {}", e),
                            }
                        }
                        Err(e) => eprintln!("introspection error: {}", e),
                    }
                }
            } else {
                println!("intellid-mcp-server started. Set INTELLID_PG_DSN to run a quick introspection.");
            }
        });
    });

    // 在主线程启动 GUI（macOS 需要主线程运行窗口循环）
    if let Err(e) = intellid_gui::run() {
        eprintln!("GUI error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
