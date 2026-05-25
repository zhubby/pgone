use std::env;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use pgone_util::log;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "pgone")]
#[command(about = "PGone unified command-line entrypoint", long_about = None)]
pub struct Cli {
    /// Log level: trace, debug, info, warn, or error.
    #[arg(short, long, global = true)]
    pub log_level: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Launch the PGone desktop GUI.
    Gui,
    /// Run the PostgreSQL introspection MCP server.
    McpServer(McpServerArgs),
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

    /// Database config ID.
    #[arg(long)]
    pub dbconfig_id: String,
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    run_cli(cli).await
}

pub async fn run_cli(cli: Cli) -> Result<()> {
    let log_level = cli.log_level;

    match cli.command.unwrap_or(Command::Gui) {
        Command::Gui => run_gui(log_level.as_deref()),
        Command::McpServer(args) => run_mcp_server(args, log_level.as_deref()).await,
    }
}

fn run_gui(log_level: Option<&str>) -> Result<()> {
    match log_level {
        Some(log_level) => log::init_log_simple(log_level)?,
        None => log::init_log_from_env()?,
    }

    pgone_gui::run()
}

async fn run_mcp_server(args: McpServerArgs, log_level: Option<&str>) -> Result<()> {
    let protocol = resolve_protocol(args.protocol);
    log::init_log_simple(log_level.unwrap_or("info"))?;

    info!("pgone mcp-server starting");
    match protocol {
        Protocol::Stdio => pgone_mcp::mcp::run_stdio(args.dbconfig_id).await,
        Protocol::Streamable => pgone_mcp::mcp::run_streamable(&args.addr, args.dbconfig_id).await,
    }
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
    fn parses_global_log_level_before_subcommand() {
        let cli = Cli::parse_from(["pgone", "--log-level", "debug", "gui"]);
        assert_eq!(cli.log_level.as_deref(), Some("debug"));
        assert!(matches!(cli.command, Some(Command::Gui)));
    }

    #[test]
    fn parses_global_log_level_after_subcommand() {
        let cli = Cli::parse_from(["pgone", "gui", "--log-level", "warn"]);
        assert_eq!(cli.log_level.as_deref(), Some("warn"));
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
    fn clap_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
