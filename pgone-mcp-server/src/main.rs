use clap::Parser;
use pgone_mcp_server::mcp;
use pgone_util::log::init_log_simple;
use std::env;
use std::path::PathBuf;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "pgone-mcp-server")]
#[command(about = "PostgreSQL introspection MCP server", long_about = None)]
struct Args {
    /// Storage 路径（数据库配置存储位置）
    #[arg(long)]
    storage_path: Option<String>,

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

    // 从环境变量或命令行参数获取 storage 路径
    let storage_path = args.storage_path
        .or_else(|| env::var("PGONE_STORAGE_PATH").ok())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            // 默认使用当前目录下的 storage
            PathBuf::from("storage")
        });

    let stdio = args.stdio || env::var("PGONE_MCP_STDIO").is_ok();

    // 初始化日志（使用 pgone-util 的 log 模块）
    init_log_simple(&args.log_level)?;

    info!("pgone-mcp-server 启动中...");
    info!("Storage 路径: {}", storage_path.display());

    // 如果启用了 STDIO 模式，运行 STDIO 服务器
    if stdio {
        info!("启动 STDIO 模式...");
        mcp::run_stdio(storage_path).await?;
    } else {
        info!("未启用 STDIO 模式，程序退出");
        info!("提示: 设置环境变量 PGONE_MCP_STDIO=1 或使用 --stdio 参数启用 STDIO 模式");
    }

    Ok(())
}

