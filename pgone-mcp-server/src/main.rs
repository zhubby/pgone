use clap::Parser;
use pgone_mcp_server::{config, mcp, registry::ConnectionRegistry};
use pgone_util::log::init_log_simple;
use std::env;
use tracing::{info, error};

#[derive(Parser, Debug)]
#[command(name = "pgone-mcp-server")]
#[command(about = "PostgreSQL introspection MCP server", long_about = None)]
struct Args {
    /// 连接配置文件路径
    #[arg(long)]
    connections_path: Option<String>,

    /// 启用 STDIO 模式
    #[arg(long)]
    stdio: bool,

    /// 日志级别 (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // 从环境变量或命令行参数获取配置
    let connections_path = args.connections_path
        .or_else(|| env::var("PGONE_CONNECTIONS_PATH").ok());
    let stdio = args.stdio || env::var("PGONE_MCP_STDIO").is_ok();

    // 初始化日志（使用 pgone-util 的 log 模块）
    init_log_simple(&args.log_level)?;

    info!("pgone-mcp-server 启动中...");

    // 创建连接注册表
    let registry = ConnectionRegistry::new();

    // 如果提供了连接配置文件路径，加载连接
    if let Some(path) = connections_path {
        info!("从文件加载连接配置: {}", path);
        match config::load_connections_from_path(&path) {
            Ok(connections) => {
                for conn in connections {
                    match registry.register(conn.clone()).await {
                        Ok(_) => info!("已注册连接: {}", conn.id),
                        Err(e) => error!("注册连接失败 {}: {}", conn.id, e),
                    }
                }
            }
            Err(e) => {
                error!("加载连接配置失败: {}", e);
                return Err(e);
            }
        }
    }

    // 如果启用了 STDIO 模式，运行 STDIO 服务器
    if stdio {
        info!("启动 STDIO 模式...");
        mcp::run_stdio(registry).await?;
    } else {
        info!("未启用 STDIO 模式，程序退出");
        info!("提示: 设置环境变量 PGONE_MCP_STDIO=1 或使用 --stdio 参数启用 STDIO 模式");
    }

    Ok(())
}

