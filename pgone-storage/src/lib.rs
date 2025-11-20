use anyhow::Result;
#[cfg(feature = "backend-libsql")]
use libsql::{Builder, Connection, Database};
#[cfg(feature = "backend-turso")]
use turso::{Builder, Connection, Database};

pub mod blocking;
pub mod models;
pub mod schema;
pub mod storage;

pub struct Storage {
    db: Database,
}

impl Storage {
    pub async fn open_local(path: &str) -> Result<Self> {
        let db = Builder::new_local(path).build().await?;
        let s = Self { db };
        s.migrate().await?;
        Ok(s)
    }

    pub async fn conn(&self) -> Result<Connection> {
        Ok(self.db.connect()?)
    }

    async fn migrate(&self) -> Result<()> {
        let mut conn = self.conn().await?;
        schema::migrate(&mut conn).await
    }
}
