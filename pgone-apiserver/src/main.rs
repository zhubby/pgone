use clap::Parser;
use pgone_apiserver::{grpc, serve};
use pgone_util::log;
use std::net::SocketAddr;
use std::str::FromStr;
use tokio::signal;
use tokio::sync::broadcast;
use tracing::{info, warn};

/// PostgreSQL API 服务器
#[derive(Parser, Debug)]
#[command(name = "pgone-apiserver")]
#[command(about = "PostgreSQL API 服务器，提供 HTTP 和 gRPC 接口", long_about = None)]
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
    #[arg(long, default_value = "pgone-apiserver")]
    service_name: String,

    /// HTTP 服务器绑定地址
    #[arg(long, default_value = "127.0.0.1")]
    http_bind: String,

    /// HTTP 服务器监听端口
    #[arg(long, default_value = "8765")]
    http_port: u16,

    /// gRPC 服务器绑定地址
    #[arg(long, default_value = "127.0.0.1")]
    grpc_bind: String,

    /// gRPC 服务器监听端口
    #[arg(long, default_value = "50051")]
    grpc_port: u16,
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

    info!("pgone-apiserver 启动中...");

    // 解析 HTTP 服务器地址
    let http_addr: SocketAddr = format!("{}:{}", args.http_bind, args.http_port)
        .parse()
        .map_err(|e| anyhow::anyhow!("无效的 HTTP 地址: {}", e))?;

    // 解析 gRPC 服务器地址
    let grpc_addr: SocketAddr = format!("{}:{}", args.grpc_bind, args.grpc_port)
        .parse()
        .map_err(|e| anyhow::anyhow!("无效的 gRPC 地址: {}", e))?;

    info!(
        http_addr = %http_addr,
        grpc_addr = %grpc_addr,
        "服务器地址配置完成"
    );

    // 创建广播 channel 用于关闭信号
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    // 创建可以被多次等待的关闭信号 future
    let create_shutdown_signal = || {
        let mut shutdown_rx = shutdown_tx.subscribe();
        async move {
            let _ = shutdown_rx.recv().await;
        }
    };

    // 启动信号监听任务
    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        // 监听 Ctrl+C 和 SIGTERM 信号
        let ctrl_c = async {
            signal::ctrl_c()
                .await
                .expect("failed to install Ctrl+C handler");
            warn!("收到 Ctrl+C 信号，开始优雅关闭...");
        };

        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("failed to install signal handler")
                .recv()
                .await;
            warn!("收到 SIGTERM 信号，开始优雅关闭...");
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }

        // 发送关闭信号
        let _ = shutdown_tx_clone.send(());
    });

    info!("启动 HTTP 和 gRPC 服务器...");

    // 使用 tokio::select! 同时运行 HTTP 和 gRPC 服务器
    tokio::select! {
        result = serve(http_addr, create_shutdown_signal()) => {
            match result {
                Ok(_) => {
                    info!("HTTP 服务器已关闭");
                }
                Err(e) => {
                    tracing::error!(error = ?e, "HTTP 服务器错误");
                    return Err(e);
                }
            }
        }
        result = grpc::serve_grpc(grpc_addr, create_shutdown_signal()) => {
            match result {
                Ok(_) => {
                    info!("gRPC 服务器已关闭");
                }
                Err(e) => {
                    tracing::error!(error = ?e, "gRPC 服务器错误");
                    return Err(e);
                }
            }
        }
    }

    info!("所有服务器已关闭，正在清理资源...");

    // 清理 OpenTelemetry 资源
    if args.enable_otel {
        log::shutdown_otel();
    }

    Ok(())
}

