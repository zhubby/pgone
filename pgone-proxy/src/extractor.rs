use crate::replay::StorageType;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 数据库连接配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionExtractorConfig {
    /// 数据库连接字符串 (DSN)
    pub dsn: String,
    /// SQL 语句列表
    pub sql: Vec<String>,
    /// SSL/TLS 配置
    #[serde(default)]
    pub ssl: Option<SslExtractorConfig>,
    /// 回放配置
    #[serde(default)]
    pub replay: Option<ReplayExtractorConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReplayExtractorConfig {
    pub storage_type: StorageType,
    /// 数据库连接字符串 (DSN)
    pub dsn: Option<String>,
    pub sqlite_path: Option<PathBuf>,
}

/// SSL/TLS 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslExtractorConfig {
    /// SSL 证书文件路径
    #[serde(default)]
    pub cert: Option<PathBuf>,
    /// SSL 私钥文件路径
    #[serde(default)]
    pub key: Option<PathBuf>,
    /// CA 证书文件路径
    #[serde(default)]
    pub ca: Option<PathBuf>,
    /// SSL 模式: disable, allow, prefer, require, verify-ca, verify-full
    #[serde(default = "default_ssl_mode")]
    pub mode: String,
}

fn default_ssl_mode() -> String {
    "prefer".to_string()
}

impl Default for SslExtractorConfig {
    fn default() -> Self {
        Self {
            cert: None,
            key: None,
            ca: None,
            mode: default_ssl_mode(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_config_with_sql() {
        let config = ConnectionExtractorConfig {
            dsn: "postgres://user:pass@localhost:5432/db".to_string(),
            sql: vec!["SELECT * FROM users;".to_string(), "SELECT 1;".to_string()],
            ssl: None,
            replay: None,
        };
        assert_eq!(config.dsn, "postgres://user:pass@localhost:5432/db");
        assert_eq!(config.sql.len(), 2);
        assert_eq!(config.sql[0], "SELECT * FROM users;");
    }

    #[test]
    fn test_connection_config_with_ssl() {
        let config = ConnectionExtractorConfig {
            dsn: "postgres://user:pass@localhost:5432/db".to_string(),
            sql: vec!["SELECT * FROM users;".to_string()],
            ssl: Some(SslExtractorConfig {
                cert: Some("/path/to/cert.pem".into()),
                key: Some("/path/to/key.pem".into()),
                ca: Some("/path/to/ca.pem".into()),
                mode: "require".to_string(),
            }),
            replay: None,
        };
        assert!(config.ssl.is_some());
        assert_eq!(config.ssl.as_ref().unwrap().mode, "require");
    }
}
