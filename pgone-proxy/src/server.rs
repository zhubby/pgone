use std::sync::Arc;

use tokio::net::TcpListener;
use tracing::{error, info};

use pgwire::api::auth::StartupHandler;
use pgwire::api::query::SimpleQueryHandler;
use pgwire::api::PgWireServerHandlers;
use pgwire::tokio::process_socket;

use crate::processor::PostgresProxyProcessorFactory;

impl PgWireServerHandlers for PostgresProxyProcessorFactory {
    fn simple_query_handler(&self) -> Arc<impl SimpleQueryHandler> {
        self.handler.clone()
    }

    fn startup_handler(&self) -> Arc<impl StartupHandler> {
        self.handler.clone()
    }
}

/// 启动PostgreSQL代理服务器
pub async fn start() {
    let factory = Arc::new(PostgresProxyProcessorFactory::new());

    let server_addr = "127.0.0.1:5432";
    let listener = match TcpListener::bind(server_addr).await {
        Ok(listener) => {
            info!(
                address = server_addr,
                "PostgreSQL proxy server started"
            );
            listener
        }
        Err(e) => {
            error!(
                error = ?e,
                address = server_addr,
                "Failed to bind to address"
            );
            return;
        }
    };

    loop {
        match listener.accept().await {
            Ok((socket, addr)) => {
                info!(
                    client_addr = ?addr,
                    "New client connection"
                );
                let factory_ref = factory.clone();
                tokio::spawn(async move {
                    if let Err(e) = process_socket(socket, None, factory_ref).await {
                        error!(
                            error = ?e,
                            client_addr = ?addr,
                            "Error processing socket"
                        );
                    }
                });
            }
            Err(e) => {
                error!(
                    error = ?e,
                    "Failed to accept connection"
                );
            }
        }
    }
}

