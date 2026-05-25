use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum StorageType {
    #[default]
    #[serde(rename = "sqlite")]
    SQLite,
    #[serde(rename = "postgresql")]
    PostgreSQL,
}
