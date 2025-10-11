use crate::registry::{ConnectionConfig, DatabaseEngine};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ConnectionsFile {
    pub connections: Vec<FileConn>,
}

#[derive(Debug, Deserialize)]
pub struct FileConn {
    pub id: String,
    pub engine: String,
    pub dsn: String,
}

pub fn load_connections_from_path(path: &str) -> anyhow::Result<Vec<ConnectionConfig>> {
    let text = std::fs::read_to_string(path)?;
    let cfg: ConnectionsFile = serde_yaml::from_str(&text)?;
    let mut out = Vec::new();
    for c in cfg.connections {
        let engine = match c.engine.as_str() {
            "postgres" | "pg" | "postgresql" => DatabaseEngine::Postgres,
            _ => anyhow::bail!("Unsupported engine: {}", c.engine),
        };
        out.push(ConnectionConfig {
            id: c.id,
            engine,
            dsn: c.dsn,
            default_schemas: None,
            include_system: Some(false),
        });
    }
    Ok(out)
}
