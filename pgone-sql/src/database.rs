use crate::error::{Result, SqlError};
use crate::models::DatabaseInfo;
use crate::session::Session;
use sqlx::Row;
use tracing::info;

impl Session {
    /// List all databases in the PostgreSQL instance
    /// Requires connection to the 'postgres' system database
    pub async fn list_databases(&self) -> Result<Vec<DatabaseInfo>> {
        info!("Listing all databases");

        let pool = self.pool();
        let rows = sqlx::query(
            r#"
            SELECT 
                d.datname AS name,
                pg_catalog.pg_get_userbyid(d.datdba) AS owner,
                pg_catalog.pg_encoding_to_char(d.encoding) AS encoding,
                d.datcollate AS collate,
                d.datctype AS ctype,
                pg_size_pretty(pg_database_size(d.datname)) AS size,
                pg_catalog.shobj_description(d.oid, 'pg_database') AS description
            FROM pg_catalog.pg_database d
            WHERE d.datistemplate = false
            ORDER BY d.datname
            "#,
        )
        .fetch_all(pool)
        .await
        .map_err(SqlError::Connection)?;

        let mut databases = Vec::new();
        for row in rows {
            databases.push(DatabaseInfo {
                name: row.get("name"),
                owner: row.get("owner"),
                encoding: row.get("encoding"),
                collate: row.try_get("collate").ok(),
                ctype: row.try_get("ctype").ok(),
                size: row.try_get("size").ok(),
                description: row.try_get("description").ok(),
            });
        }

        Ok(databases)
    }

    /// Get detailed information about a specific database
    pub async fn get_database_info(&self, db_name: &str) -> Result<DatabaseInfo> {
        info!(db_name = db_name, "Getting database info");

        let pool = self.pool();
        let row = sqlx::query(
            r#"
            SELECT 
                d.datname AS name,
                pg_catalog.pg_get_userbyid(d.datdba) AS owner,
                pg_catalog.pg_encoding_to_char(d.encoding) AS encoding,
                d.datcollate AS collate,
                d.datctype AS ctype,
                pg_size_pretty(pg_database_size(d.datname)) AS size,
                pg_catalog.shobj_description(d.oid, 'pg_database') AS description
            FROM pg_catalog.pg_database d
            WHERE d.datname = $1 AND d.datistemplate = false
            "#,
        )
        .bind(db_name)
        .fetch_optional(pool)
        .await
        .map_err(SqlError::Connection)?
        .ok_or_else(|| SqlError::NotFound(format!("Database '{}' not found", db_name)))?;

        Ok(DatabaseInfo {
            name: row.get("name"),
            owner: row.get("owner"),
            encoding: row.get("encoding"),
            collate: row.try_get("collate").ok(),
            ctype: row.try_get("ctype").ok(),
            size: row.try_get("size").ok(),
            description: row.try_get("description").ok(),
        })
    }

    /// Create a new database
    /// Requires superuser privileges
    pub async fn create_database(
        &self,
        db_name: &str,
        owner: Option<&str>,
        encoding: Option<&str>,
        template: Option<&str>,
    ) -> Result<()> {
        info!(
            db_name = db_name,
            owner = owner,
            encoding = encoding,
            template = template,
            "Creating database"
        );

        let mut sql = format!("CREATE DATABASE {}", quote_ident(db_name));

        if let Some(owner) = owner {
            sql.push_str(&format!(" OWNER {}", quote_ident(owner)));
        }

        if let Some(encoding) = encoding {
            sql.push_str(&format!(" ENCODING '{}'", encoding.replace('\'', "''")));
        }

        if let Some(template) = template {
            sql.push_str(&format!(" TEMPLATE {}", quote_ident(template)));
        }

        let pool = self.pool();
        sqlx::query(&sql).execute(pool).await.map_err(|e| {
            let err_str = e.to_string();
            if err_str.contains("permission denied") || err_str.contains("must be superuser") {
                SqlError::PermissionDenied(format!(
                    "Creating database requires superuser privileges: {}",
                    e
                ))
            } else {
                SqlError::Execution(format!("Failed to create database: {}", e))
            }
        })?;

        Ok(())
    }

    /// Alter database properties
    pub async fn alter_database(
        &self,
        db_name: &str,
        new_name: Option<&str>,
        new_owner: Option<&str>,
        new_encoding: Option<&str>,
    ) -> Result<()> {
        info!(
            db_name = db_name,
            new_name = new_name,
            new_owner = new_owner,
            new_encoding = new_encoding,
            "Altering database"
        );

        let pool = self.pool();

        // Rename database if needed
        if let Some(new_name) = new_name {
            let sql = format!(
                "ALTER DATABASE {} RENAME TO {}",
                quote_ident(db_name),
                quote_ident(new_name)
            );
            sqlx::query(&sql)
                .execute(pool)
                .await
                .map_err(|e| SqlError::Execution(format!("Failed to rename database: {}", e)))?;
        }

        // Change owner if needed
        if let Some(new_owner) = new_owner {
            let sql = format!(
                "ALTER DATABASE {} OWNER TO {}",
                quote_ident(db_name),
                quote_ident(new_owner)
            );
            sqlx::query(&sql).execute(pool).await.map_err(|e| {
                let err_str = e.to_string();
                if err_str.contains("permission denied") {
                    SqlError::PermissionDenied(format!(
                        "Changing database owner requires appropriate privileges: {}",
                        e
                    ))
                } else {
                    SqlError::Execution(format!("Failed to change database owner: {}", e))
                }
            })?;
        }

        // Note: PostgreSQL doesn't support ALTER DATABASE ... SET ENCODING
        // Encoding can only be set at creation time
        if new_encoding.is_some() {
            return Err(SqlError::InvalidInput(
                "Database encoding cannot be changed after creation".to_string(),
            ));
        }

        Ok(())
    }

    /// Drop a database
    /// Requires superuser privileges or ownership of the database
    pub async fn drop_database(&self, db_name: &str, if_exists: bool) -> Result<()> {
        info!(
            db_name = db_name,
            if_exists = if_exists,
            "Dropping database"
        );

        let sql = if if_exists {
            format!("DROP DATABASE IF EXISTS {}", quote_ident(db_name))
        } else {
            format!("DROP DATABASE {}", quote_ident(db_name))
        };

        let pool = self.pool();
        sqlx::query(&sql).execute(pool).await.map_err(|e| {
            let err_str = e.to_string();
            if err_str.contains("permission denied") {
                SqlError::PermissionDenied(format!(
                    "Dropping database requires appropriate privileges: {}",
                    e
                ))
            } else if err_str.contains("does not exist") {
                SqlError::NotFound(format!("Database '{}' does not exist", db_name))
            } else {
                SqlError::Execution(format!("Failed to drop database: {}", e))
            }
        })?;

        Ok(())
    }
}

/// Quote an identifier for use in SQL
fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quote_ident() {
        assert_eq!(quote_ident("test"), "\"test\"");
        assert_eq!(quote_ident("test_db"), "\"test_db\"");
        assert_eq!(quote_ident("test\"db"), "\"test\"\"db\"");
        assert_eq!(quote_ident("my-database"), "\"my-database\"");
        assert_eq!(quote_ident("database123"), "\"database123\"");
    }
}
