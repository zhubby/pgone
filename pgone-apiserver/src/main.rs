use clap::Parser;
use pgone_apiserver::ApiServerConfig;

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
    let args = Args::parse();
    pgone_apiserver::run(ApiServerConfig {
        log_level: args.log_level,
        enable_otel: args.enable_otel,
        json_log: args.json_log,
        service_name: args.service_name,
        http_bind: args.http_bind,
        http_port: args.http_port,
        grpc_bind: args.grpc_bind,
        grpc_port: args.grpc_port,
    })
    .await
}
