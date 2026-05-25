use std::env;
use std::net::SocketAddr;
use std::str::FromStr;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use pgone_apiserver::ApiServerConfig;
use pgone_util::log::{self, LogConfig, LogLevel};
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "pgone")]
#[command(about = "PGone unified command-line entrypoint", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Launch the PGone desktop GUI.
    Gui,
    /// Run the PostgreSQL introspection MCP server.
    McpServer(McpServerArgs),
    /// Run the HTTP/gRPC API server.
    Apiserver(ApiServerArgs),
    /// Run the PostgreSQL proxy server.
    Proxy(ServiceLogArgs),
    /// Run the A2A schema query server.
    A2a(A2aArgs),
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
    /// STDIO mode for agent integrations.
    Stdio,
    /// Streamable HTTP mode.
    Streamable,
}

#[derive(Parser, Debug)]
pub struct McpServerArgs {
    /// Protocol type: stdio or streamable.
    #[arg(long, value_enum)]
    pub protocol: Option<Protocol>,

    /// Streamable HTTP bind address.
    #[arg(long, default_value = "127.0.0.1:3000")]
    pub addr: String,

    /// Log level: trace, debug, info, warn, or error.
    #[arg(long, default_value = "info")]
    pub log_level: String,

    /// Database config ID.
    #[arg(long)]
    pub dbconfig_id: String,
}

#[derive(Parser, Debug)]
pub struct ApiServerArgs {
    /// Log level: trace, debug, info, warn, or error.
    #[arg(short, long, default_value = "info")]
    pub log_level: String,

    /// Enable OpenTelemetry tracing.
    #[arg(long)]
    pub enable_otel: bool,

    /// Use JSON formatted logs.
    #[arg(long)]
    pub json_log: bool,

    /// Service name used for OpenTelemetry.
    #[arg(long, default_value = "pgone-apiserver")]
    pub service_name: String,

    /// HTTP server bind address.
    #[arg(long, default_value = "127.0.0.1")]
    pub http_bind: String,

    /// HTTP server port.
    #[arg(long, default_value = "8765")]
    pub http_port: u16,

    /// gRPC server bind address.
    #[arg(long, default_value = "127.0.0.1")]
    pub grpc_bind: String,

    /// gRPC server port.
    #[arg(long, default_value = "50051")]
    pub grpc_port: u16,
}

#[derive(Parser, Debug)]
pub struct ServiceLogArgs {
    /// Log level: trace, debug, info, warn, or error.
    #[arg(short, long, default_value = "info")]
    pub log_level: String,

    /// Enable OpenTelemetry tracing.
    #[arg(long)]
    pub enable_otel: bool,

    /// Use JSON formatted logs.
    #[arg(long)]
    pub json_log: bool,

    /// Service name used for OpenTelemetry.
    #[arg(long, default_value = "pgone-proxy")]
    pub service_name: String,
}

#[derive(Parser, Debug)]
pub struct A2aArgs {
    /// A2A server bind address.
    #[arg(long)]
    pub addr: Option<SocketAddr>,
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    run_cli(cli).await
}

pub async fn run_cli(cli: Cli) -> Result<()> {
    match cli.command.unwrap_or(Command::Gui) {
        Command::Gui => run_gui(),
        Command::McpServer(args) => run_mcp_server(args).await,
        Command::Apiserver(args) => run_apiserver(args).await,
        Command::Proxy(args) => run_proxy(args).await,
        Command::A2a(args) => run_a2a(args).await,
    }
}

fn run_gui() -> Result<()> {
    log::init_log_from_env()?;
    pgone_gui::run()
}

async fn run_mcp_server(args: McpServerArgs) -> Result<()> {
    let protocol = resolve_protocol(args.protocol);
    log::init_log_simple(&args.log_level)?;

    info!("pgone mcp-server starting");
    match protocol {
        Protocol::Stdio => pgone_mcp::mcp::run_stdio(args.dbconfig_id).await,
        Protocol::Streamable => pgone_mcp::mcp::run_streamable(&args.addr, args.dbconfig_id).await,
    }
}

async fn run_apiserver(args: ApiServerArgs) -> Result<()> {
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

async fn run_proxy(args: ServiceLogArgs) -> Result<()> {
    log::init_log(LogConfig {
        level: LogLevel::from_str(&args.log_level)?,
        enable_otel: args.enable_otel,
        json_format: args.json_log,
        service_name: Some(args.service_name.clone()),
    })?;

    pgone_proxy::server::start().await;

    if args.enable_otel {
        log::shutdown_otel();
    }

    Ok(())
}

async fn run_a2a(args: A2aArgs) -> Result<()> {
    log::init_log_simple("info")?;
    let addr = match args.addr {
        Some(addr) => addr,
        None => env::var("PGONE_A2A_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
            .parse()?,
    };

    pgone_a2a::start_server(addr).await
}

fn resolve_protocol(protocol: Option<Protocol>) -> Protocol {
    if let Some(protocol) = protocol {
        return protocol;
    }

    match env::var("PGONE_MCP_PROTOCOL") {
        Ok(value) if value == "stdio" => Protocol::Stdio,
        Ok(value) if value == "streamable" => Protocol::Streamable,
        Ok(value) => {
            eprintln!("警告: 无效的协议类型 '{}'，使用默认值 streamable", value);
            Protocol::Streamable
        }
        Err(_) => Protocol::Streamable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn empty_args_default_to_gui() {
        let cli = Cli::parse_from(["pgone"]);
        assert!(cli.command.is_none());
    }

    #[test]
    fn parses_gui_command() {
        let cli = Cli::parse_from(["pgone", "gui"]);
        assert!(matches!(cli.command, Some(Command::Gui)));
    }

    #[test]
    fn parses_mcp_server_stdio_protocol() {
        let cli = Cli::parse_from([
            "pgone",
            "mcp-server",
            "--dbconfig-id",
            "local",
            "--protocol",
            "stdio",
        ]);

        let Some(Command::McpServer(args)) = cli.command else {
            panic!("expected mcp-server command");
        };
        assert_eq!(args.dbconfig_id, "local");
        assert_eq!(args.protocol, Some(Protocol::Stdio));
    }

    #[test]
    fn apiserver_defaults_to_existing_ports() {
        let cli = Cli::parse_from(["pgone", "apiserver"]);

        let Some(Command::Apiserver(args)) = cli.command else {
            panic!("expected apiserver command");
        };
        assert_eq!(args.http_port, 8765);
        assert_eq!(args.grpc_port, 50051);
    }

    #[test]
    fn a2a_addr_can_be_overridden() {
        let cli = Cli::parse_from(["pgone", "a2a", "--addr", "127.0.0.1:9000"]);

        let Some(Command::A2a(args)) = cli.command else {
            panic!("expected a2a command");
        };
        assert_eq!(
            args.addr,
            Some("127.0.0.1:9000".parse::<SocketAddr>().unwrap())
        );
    }

    #[test]
    fn clap_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
