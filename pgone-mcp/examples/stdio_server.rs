use pgone_mcp::mcp::run_stdio;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging (supports RUST_LOG override)
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init();

    let storage_path = pgone_storage::database_path();

    tracing::info!("PGone MCP Server starting (stdio mode)");
    tracing::info!("Storage path: {}", storage_path.display());
    // dbconfig_id is required in the example, use env var or default here
    let dbconfig_id = std::env::var("PGONE_DBCONFIG_ID").unwrap_or_else(|_| "default".to_string());
    run_stdio(dbconfig_id).await
}
