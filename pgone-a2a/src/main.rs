use anyhow::Result;
use pgone_a2a::start_server;
use std::net::SocketAddr;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // 从环境变量读取监听地址，默认为 0.0.0.0:8080
    let addr: SocketAddr = std::env::var("PGONE_A2A_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
        .parse()?;

    info!("PGone A2A Protocol Server");
    info!("Listening on: {}", addr);
    info!("Send POST requests to http://{}/schema/query", addr);
    info!("Example request body:");
    info!(r#"{{"dsn": "postgres://user:pass@localhost:5432/dbname", "schemas": ["public"]}}"#);

    start_server(addr).await?;

    Ok(())
}

