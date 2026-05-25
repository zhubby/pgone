use clap::Parser;
use pgone_proxy::server;
use pgone_util::log;
use std::str::FromStr;

/// PostgreSQL 代理服务器
#[derive(Parser, Debug)]
#[command(name = "pgone-proxy")]
#[command(about = "PostgreSQL 代理服务器，提供数据库连接代理功能", long_about = None)]
struct Args {
    /// 日志级别 (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info")]
    log_level: String,

    /// 启用 OpenTelemetry 追踪
    #[arg(long)]
    enable_otel: bool,

    /// 使用 JSON 格式输出日志（用于生产环境）
    #[arg(long)]
    json_log: bool,

    /// 服务名称（用于 OpenTelemetry）
    #[arg(long, default_value = "pgone-proxy")]
    service_name: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 解析命令行参数
    let args = Args::parse();

    // 初始化日志系统
    log::init_log(log::LogConfig {
        level: log::LogLevel::from_str(&args.log_level)?,
        enable_otel: args.enable_otel,
        json_format: args.json_log,
        service_name: Some(args.service_name.clone()),
    })?;

    // 启动代理服务器
    server::start().await;

    // 清理 OpenTelemetry 资源
    if args.enable_otel {
        log::shutdown_otel();
    }

    Ok(())
}
