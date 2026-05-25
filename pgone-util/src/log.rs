use anyhow::Result;
use std::str::FromStr;
use tracing::Level;
use tracing_subscriber::fmt::time::ChronoUtc;

/// 日志级别枚举
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
            _ => Err(anyhow::anyhow!("无效的日志级别: {}", s)),
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

/// 日志初始化配置
pub struct LogConfig {
    /// 日志级别
    pub level: LogLevel,
    /// 是否启用 OpenTelemetry 追踪
    pub enable_otel: bool,
    /// 是否使用 JSON 格式输出（用于生产环境）
    pub json_format: bool,
    /// 服务名称（用于 OpenTelemetry）
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

/// 初始化日志系统
///
/// # 参数
/// - `config`: 日志配置
///
/// # 示例
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

    // 将 LogLevel 转换为 tracing::Level
    let level: Level = config.level.into();

    // 根据配置选择格式化层，使用 with_max_level 设置日志级别
    if config.json_format {
        // JSON 格式（用于生产环境）
        tracing_subscriber::fmt()
            .with_max_level(level)
            .json()
            .with_timer(ChronoUtc::rfc_3339())
            .with_target(true)
            .with_file(true)
            .with_line_number(true)
            .init();
    } else {
        // 美观的彩色格式（用于开发环境）
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

    // 如果启用 OpenTelemetry，添加追踪层
    // 注意：OpenTelemetry 0.31 版本的 API 可能需要根据实际 exporter 配置
    // 这里提供一个基础框架，实际使用时需要配置合适的 exporter
    if config.enable_otel {
        tracing::warn!(
            service_name = config.service_name.as_deref().unwrap_or("pgone-service"),
            "OpenTelemetry 功能已启用，但需要配置 exporter 才能正常工作"
        );
        // TODO: 实现完整的 OpenTelemetry 集成
    }

    tracing::info!(
        level = level_str,
        "日志系统已初始化，日志级别: {}",
        level_str
    );

    Ok(())
}

/// 便捷函数：使用默认配置初始化日志
///
/// # 参数
/// - `level`: 日志级别字符串（如 "info", "debug" 等）
pub fn init_log_simple(level: &str) -> Result<()> {
    let log_level = LogLevel::from_str(level)?;
    init_log(LogConfig {
        level: log_level,
        ..Default::default()
    })
}

/// 便捷函数：从环境变量初始化日志
///
/// 读取环境变量 `RUST_LOG` 来设置日志级别
/// 如果未设置，默认使用 `info` 级别
pub fn init_log_from_env() -> Result<()> {
    let level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    init_log_simple(&level)
}

/// 清理 OpenTelemetry 资源
/// 在程序退出前调用以确保资源正确释放
pub fn shutdown_otel() {
    // OpenTelemetry SDK 会自动清理资源
    // 如果需要显式清理，可以在这里添加相应的清理代码
}
