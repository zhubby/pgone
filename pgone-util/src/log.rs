use anyhow::Result;
use std::str::FromStr;
use tracing::Level;
use tracing_subscriber::fmt::time::ChronoUtc;

/// Log level enum
#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl FromStr for LogLevel {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "trace" => Ok(LogLevel::Trace),
            "debug" => Ok(LogLevel::Debug),
            "info" => Ok(LogLevel::Info),
            "warn" => Ok(LogLevel::Warn),
            "error" => Ok(LogLevel::Error),
            _ => Err(anyhow::anyhow!("invalid log level: {}", s)),
        }
    }
}

impl From<LogLevel> for Level {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => Level::TRACE,
            LogLevel::Debug => Level::DEBUG,
            LogLevel::Info => Level::INFO,
            LogLevel::Warn => Level::WARN,
            LogLevel::Error => Level::ERROR,
        }
    }
}

/// Log initialization configuration
pub struct LogConfig {
    /// Log level
    pub level: LogLevel,
    /// Whether to enable OpenTelemetry tracing
    pub enable_otel: bool,
    /// Whether to use JSON format output (for production environments)
    pub json_format: bool,
    /// Service name (for OpenTelemetry)
    pub service_name: Option<String>,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            enable_otel: false,
            json_format: false,
            service_name: None,
        }
    }
}

/// Initialize the logging system
///
/// # Parameters
/// - `config`: Log configuration
///
/// # Example
/// ```rust
/// use pgone_util::log::{init_log, LogConfig, LogLevel};
///
/// init_log(LogConfig {
///     level: LogLevel::Info,
///     enable_otel: true,
///     json_format: false,
///     service_name: Some("my-service".to_string()),
/// }).unwrap();
/// ```
pub fn init_log(config: LogConfig) -> Result<()> {
    let level_str = match config.level {
        LogLevel::Trace => "trace",
        LogLevel::Debug => "debug",
        LogLevel::Info => "info",
        LogLevel::Warn => "warn",
        LogLevel::Error => "error",
    };

    // Convert LogLevel to tracing::Level
    let level: Level = config.level.into();

    // Select formatter layer based on configuration, set log level with with_max_level
    if config.json_format {
        // JSON format (for production environments)
        tracing_subscriber::fmt()
            .with_max_level(level)
            .json()
            .with_timer(ChronoUtc::rfc_3339())
            .with_target(true)
            .with_file(true)
            .with_line_number(true)
            .init();
    } else {
        // Pretty colored format (for development environments)
        tracing_subscriber::fmt()
            .with_max_level(level)
            .with_timer(ChronoUtc::rfc_3339())
            .with_target(true)
            .with_file(true)
            .with_line_number(true)
            .with_ansi(true)
            .with_level(true)
            .pretty()
            .init();
    }

    // If OpenTelemetry is enabled, add a tracing layer
    // Note: OpenTelemetry 0.31 API may require additional configuration based on the actual exporter.
    // This provides a basic framework; a suitable exporter should be configured for actual use.
    if config.enable_otel {
        tracing::warn!(
            service_name = config.service_name.as_deref().unwrap_or("pgone-service"),
            "OpenTelemetry is enabled, but an exporter must be configured to function properly"
        );
        // TODO: Implement full OpenTelemetry integration
    }

    tracing::info!(
        level = level_str,
        "Logging system initialized, log level: {}",
        level_str
    );

    Ok(())
}

/// Convenience function: initialize logging with default configuration
///
/// # Parameters
/// - `level`: Log level string (e.g. "info", "debug", etc.)
pub fn init_log_simple(level: &str) -> Result<()> {
    let log_level = LogLevel::from_str(level)?;
    init_log(LogConfig {
        level: log_level,
        ..Default::default()
    })
}

/// Convenience function: initialize logging from environment variable
///
/// Reads the `RUST_LOG` environment variable to set the log level.
/// Defaults to `info` level if not set.
pub fn init_log_from_env() -> Result<()> {
    let level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    init_log_simple(&level)
}

/// Shut down OpenTelemetry resources
/// Call before program exit to ensure resources are properly released
pub fn shutdown_otel() {
    // OpenTelemetry SDK cleans up resources automatically
    // Add explicit cleanup code here if needed
}
