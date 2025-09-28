use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use moka::future::Cache;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DatabaseEngine { Postgres }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub id: String,
    pub engine: DatabaseEngine,
    pub dsn: String,
    pub default_schemas: Option<Vec<String>>, 
    pub include_system: Option<bool>,
}

#[derive(Clone)]
pub struct ConnectionHandle {
    pub engine: DatabaseEngine,
    pub pool: Pool<Postgres>,
    pub cache: Cache<String, serde_json::Value>,
}

#[derive(Clone, Default)]
pub struct ConnectionRegistry {
    inner: Arc<RwLock<HashMap<String, ConnectionHandle>>>,
}

impl ConnectionRegistry {
    pub fn new() -> Self { Self::default() }

    pub async fn register(&self, cfg: ConnectionConfig) -> anyhow::Result<()> {
        match cfg.engine {
            DatabaseEngine::Postgres => {
                let pool = sqlx::postgres::PgPoolOptions::new()
                    .max_connections(10)
                    .connect(&cfg.dsn)
                    .await?;
                let cache = Cache::builder().time_to_live(std::time::Duration::from_secs(300)).build();
                let handle = ConnectionHandle { engine: cfg.engine, pool, cache };
                self.inner.write().await.insert(cfg.id, handle);
                Ok(())
            }
        }
    }

    pub async fn get(&self, id: &str) -> Option<ConnectionHandle> {
        self.inner.read().await.get(id).cloned()
    }

    pub async fn list(&self) -> Vec<(String, DatabaseEngine)> {
        self.inner.read().await.iter().map(|(k, v)| (k.clone(), v.engine)).collect()
    }

    pub async fn remove(&self, id: &str) -> bool {
        self.inner.write().await.remove(id).is_some()
    }
}


