use pgone_mcp_server::adapters::postgres::PostgresIntrospector;
use pgone_mcp_server::config;
use pgone_mcp_server::core::introspector::DatabaseIntrospector;
use pgone_mcp_server::mcp;
use pgone_mcp_server::registry::{ConnectionConfig, ConnectionRegistry, DatabaseEngine};
use tracing_subscriber::EnvFilter;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // 在后台启动 MCP（STDIO 或一键自省）
    tokio::spawn(async move {
        let registry = ConnectionRegistry::new();

        if let Ok(path) = std::env::var("PGONE_CONNECTIONS_PATH")
            && let Ok(conns) = config::load_connections_from_path(&path)
        {
            for c in conns {
                let _ = registry.register(c).await;
            }
        }

        if std::env::var("PGONE_MCP_STDIO").is_ok() {
            let _ = mcp::run_stdio(registry).await;
        } else if let Ok(dsn) = std::env::var("PGONE_PG_DSN") {
            if let Err(e) = registry
                .register(ConnectionConfig {
                    id: "default".to_string(),
                    engine: DatabaseEngine::Postgres,
                    dsn,
                    default_schemas: None,
                    include_system: Some(false),
                })
                .await
            {
                eprintln!("register error: {}", e);
                return;
            }

            if let Some(handle) = registry.get("default").await {
                let pg = PostgresIntrospector::new(handle.pool.clone());
                match pg
                    .introspect_database(pgone_mcp_server::core::models::IntrospectOptions {
                        schemas: None,
                        with_indexes: true,
                        with_routines: false,
                        with_types: false,
                        with_triggers: false,
                        page: None,
                        page_size: None,
                    })
                    .await
                {
                    Ok(schema) => match serde_json::to_string_pretty(&schema) {
                        Ok(s) => println!("{}", s),
                        Err(e) => eprintln!("serde error: {}", e),
                    },
                    Err(e) => eprintln!("introspection error: {}", e),
                }
            }
        } else {
            println!(
                "pgone-mcp-server started. Set PGONE_PG_DSN to run a quick introspection."
            );
        }
    });

    // 在主线程启动 GUI（macOS 需要主线程运行窗口循环）
    // GUI 会阻塞主线程，但 tokio runtime 的其他工作线程可以继续运行异步任务
    // 注意：必须在主线程直接调用，不能使用 spawn_blocking
    pgone_gui::run()
}
