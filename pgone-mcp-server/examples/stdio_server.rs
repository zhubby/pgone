use std::env;
use std::path::PathBuf;

use pgone_mcp_server::mcp::run_stdio;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志（支持 RUST_LOG 覆盖）
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init();

    // 从环境变量获取 storage 路径
    let storage_path = env::var("PGONE_STORAGE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("storage"));

    tracing::info!("PGone MCP Server 启动（stdio 模式）");
    tracing::info!("Storage 路径: {}", storage_path.display());
    run_stdio(storage_path).await
}
