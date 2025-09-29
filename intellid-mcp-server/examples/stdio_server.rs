use std::env;

use intellid_mcp_server::config::load_connections_from_path;
use intellid_mcp_server::mcp::run_stdio;
use intellid_mcp_server::registry::ConnectionRegistry;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志（支持 RUST_LOG 覆盖）
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .try_init();

    // 读取 --connections <path> 或环境变量 INTELLID_CONNECTIONS
    let mut args = env::args().collect::<Vec<String>>();
    let mut connections_path: Option<String> = None;
    if let Some(pos) = args.iter().position(|a| a == "--connections") {
        if let Some(p) = args.get(pos + 1) {
            connections_path = Some(p.clone());
            // 移除已消费参数，避免误传给其他解析（这里纯示例，无其他解析）
            args.drain(pos..=pos + 1);
        }
    }
    if connections_path.is_none() {
        connections_path = env::var("INTELLID_CONNECTIONS").ok();
    }

    let registry = ConnectionRegistry::new();

    // 预加载连接（可选）
    if let Some(path) = connections_path {
        match load_connections_from_path(&path) {
            Ok(conns) => {
                for c in conns {
                    if let Err(e) = registry.register(c).await {
                        tracing::warn!(error = %e, "注册数据库连接失败");
                    }
                }
                tracing::info!("已从配置文件加载连接: {}", path);
            }
            Err(e) => {
                tracing::warn!(error = %e, "读取连接配置失败，继续以空注册表启动");
            }
        }
    }

    tracing::info!("IntelliD MCP Server 启动（stdio 模式）");
    run_stdio(registry).await
}


