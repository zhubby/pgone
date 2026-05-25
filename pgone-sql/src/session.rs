use crate::error::{Result, SqlError};
use bb8_postgres::{PostgresConnectionManager, bb8::Pool};
use std::sync::Arc;
use tokio_postgres::NoTls;
use tracing::info;

#[derive(Clone)]
pub struct Session {
    pool: Arc<Pool<PostgresConnectionManager<NoTls>>>,
}

impl Session {
    /// Create a new session with a DSN connection string
    pub async fn new(dsn: &str) -> Result<Self> {
        info!(dsn = dsn, "Creating database session");

        let config = dsn
            .parse::<tokio_postgres::Config>()
            .map_err(SqlError::Connection)?;

        let manager = PostgresConnectionManager::new(config, NoTls);
        let pool = Pool::builder().max_size(10).build(manager).await?;

        Ok(Self {
            pool: Arc::new(pool),
        })
    }

    /// Create a session connected to the 'postgres' system database
    /// This is useful for querying database lists and managing databases
    pub async fn connect_to_postgres(dsn: &str) -> Result<Self> {
        // Parse DSN and replace database name with 'postgres'
        let dsn = replace_database_in_dsn(dsn, "postgres");
        Self::new(&dsn).await
    }

    /// Get a connection from the pool
    pub async fn get_connection(
        &self,
    ) -> Result<bb8_postgres::bb8::PooledConnection<'_, PostgresConnectionManager<NoTls>>> {
        self.pool.get().await.map_err(SqlError::Pool)
    }

    /// Get the current database name
    pub async fn current_database(&self) -> Result<String> {
        let conn = self.get_connection().await?;
        let row = conn
            .query_one("SELECT current_database()", &[])
            .await
            .map_err(SqlError::Connection)?;
        let db: String = row.get(0);
        Ok(db)
    }
}

/// Replace the database name in a PostgreSQL DSN
fn replace_database_in_dsn(dsn: &str, new_db: &str) -> String {
    // Simple approach: find the last '/' and replace everything after it (before query params)
    if let Some(query_pos) = dsn.find('?') {
        let (base, query) = dsn.split_at(query_pos);
        if let Some(slash_pos) = base.rfind('/') {
            format!("{}{}{}", &base[..=slash_pos], new_db, query)
        } else {
            format!("{}/{}{}", base, new_db, query)
        }
    } else if let Some(slash_pos) = dsn.rfind('/') {
        format!("{}{}", &dsn[..=slash_pos], new_db)
    } else {
        format!("{}/{}", dsn, new_db)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_database_in_dsn() {
        assert_eq!(
            replace_database_in_dsn("postgresql://user:pass@host:5432/mydb", "postgres"),
            "postgresql://user:pass@host:5432/postgres"
        );
        assert_eq!(
            replace_database_in_dsn(
                "postgresql://user:pass@host:5432/mydb?sslmode=require",
                "postgres"
            ),
            "postgresql://user:pass@host:5432/postgres?sslmode=require"
        );
        assert_eq!(
            replace_database_in_dsn(
                "postgresql://user:pass@host:5432/mydb?sslmode=require&connect_timeout=10",
                "postgres"
            ),
            "postgresql://user:pass@host:5432/postgres?sslmode=require&connect_timeout=10"
        );
        assert_eq!(
            replace_database_in_dsn("postgresql://localhost/testdb", "postgres"),
            "postgresql://localhost/postgres"
        );
    }
}
