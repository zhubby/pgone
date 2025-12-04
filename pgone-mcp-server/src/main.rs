use clap::{Parser, ValueEnum};
use pgone_mcp_server::mcp;
use pgone_util::log::init_log_simple;
use std::env;
use tracing::info;

#[derive(ValueEnum, Clone, Debug)]
enum Protocol {
    /// STDIO 模式：通过标准输入输出进行通信
    Stdio,
    /// Streamable HTTP 模式：通过 HTTP 服务器提供 MCP 服务
    Streamable,
}

#[derive(Parser, Debug)]
#[command(name = "pgone-mcp-server")]
#[command(about = "PostgreSQL introspection MCP server", long_about = None)]
struct Args {
    /// 协议类型：stdio 或 streamable
    #[arg(long, value_enum)]
    protocol: Option<Protocol>,

    /// Streamable HTTP 服务器绑定地址（仅在 streamable 模式下有效）
    #[arg(long, default_value = "127.0.0.1:3000")]
    addr: String,

    /// 日志级别 (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// 数据库配置 ID（必填）
    #[arg(long)]
    dbconfig_id: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // 确定协议类型
    // 优先级：命令行参数 > 环境变量 > 默认 streamable
    let protocol = if let Some(protocol) = args.protocol {
        protocol
    } else if let Ok(protocol_str) = env::var("PGONE_MCP_PROTOCOL") {
        match protocol_str.as_str() {
            "stdio" => Protocol::Stdio,
            "streamable" => Protocol::Streamable,
            _ => {
                eprintln!("警告: 无效的协议类型 '{}'，使用默认值 streamable", protocol_str);
                Protocol::Streamable
            }
        }
    } else {
        Protocol::Streamable
    };

    let addr = args.addr.clone();

    // 初始化日志（使用 pgone-util 的 log 模块）
    init_log_simple(&args.log_level)?;

    info!("pgone-mcp-server 启动中...");

    let dbconfig_id = args.dbconfig_id.clone();

    // 根据协议类型启动相应的服务器
    match protocol {
        Protocol::Stdio => {
            info!("启动 STDIO 模式...");
            info!("使用数据库配置 ID: {}", dbconfig_id);
            mcp::run_stdio(dbconfig_id).await?;
        }
        Protocol::Streamable => {
            info!("启动 Streamable HTTP 模式...");
            info!("监听地址: {}", addr);
            info!("使用数据库配置 ID: {}", dbconfig_id);
            mcp::run_streamable(&addr, dbconfig_id).await?;
        }
    }

    Ok(())
}

