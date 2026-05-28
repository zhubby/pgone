use clap::{Parser, ValueEnum};
use pgone_mcp::mcp;
use pgone_util::log::init_log_simple;
use std::env;
use tracing::info;

#[derive(ValueEnum, Clone, Debug)]
enum Protocol {
    /// STDIO mode: communication via standard input/output
    Stdio,
    /// Streamable HTTP mode: provides MCP service via HTTP server
    Streamable,
}

#[derive(Parser, Debug)]
#[command(name = "pgone-mcp-server")]
#[command(about = "PostgreSQL introspection MCP server", long_about = None)]
struct Args {
    /// Protocol type: stdio or streamable
    #[arg(long, value_enum)]
    protocol: Option<Protocol>,

    /// Streamable HTTP server bind address (only effective in streamable mode)
    #[arg(long, default_value = "127.0.0.1:3000")]
    addr: String,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Database config ID (required)
    #[arg(long)]
    dbconfig_id: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Determine protocol type
    // Priority: CLI args > env var > default streamable
    let protocol = if let Some(protocol) = args.protocol {
        protocol
    } else if let Ok(protocol_str) = env::var("PGONE_MCP_PROTOCOL") {
        match protocol_str.as_str() {
            "stdio" => Protocol::Stdio,
            "streamable" => Protocol::Streamable,
            _ => {
                eprintln!(
                    "Warning: invalid protocol type '{}', using default streamable",
                    protocol_str
                );
                Protocol::Streamable
            }
        }
    } else {
        Protocol::Streamable
    };

    let addr = args.addr.clone();

    // Initialize logging (using pgone-util log module)
    init_log_simple(&args.log_level)?;

    info!("pgone-mcp-server starting...");

    let dbconfig_id = args.dbconfig_id.clone();

    // Start the appropriate server based on protocol type
    match protocol {
        Protocol::Stdio => {
            info!("Starting STDIO mode...");
            info!("Using database config ID: {}", dbconfig_id);
            mcp::run_stdio(dbconfig_id).await?;
        }
        Protocol::Streamable => {
            info!("Starting Streamable HTTP mode...");
            info!("Listening address: {}", addr);
            info!("Using database config ID: {}", dbconfig_id);
            mcp::run_streamable(&addr, dbconfig_id).await?;
        }
    }

    Ok(())
}
