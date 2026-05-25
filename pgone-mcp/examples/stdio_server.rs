use pgone_mcp::mcp::run_stdio;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志（支持 RUST_LOG 覆盖）
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init();

    let storage_path = pgone_storage::database_path();

    tracing::info!("PGone MCP Server 启动（stdio 模式）");
    tracing::info!("Storage 路径: {}", storage_path.display());
    // 示例中需要提供 dbconfig_id，这里使用环境变量或默认值
    let dbconfig_id = std::env::var("PGONE_DBCONFIG_ID").unwrap_or_else(|_| "default".to_string());
    run_stdio(dbconfig_id).await
}
