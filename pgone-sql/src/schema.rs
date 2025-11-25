use crate::error::{Result, SqlError};
use crate::models::SchemaInfo;
use crate::session::Session;
use tracing::info;

impl Session {
    /// List all schemas in the current database
    pub async fn list_schemas(&self) -> Result<Vec<SchemaInfo>> {
        info!("Listing all schemas");
        
        let conn = self.get_connection().await?;
        let rows = conn.query(
            r#"
            SELECT 
                n.nspname AS name,
                pg_catalog.pg_get_userbyid(n.nspowner) AS owner,
                pg_catalog.obj_description(n.oid, 'pg_namespace') AS description
            FROM pg_catalog.pg_namespace n
            WHERE n.nspname NOT IN ('pg_catalog', 'pg_toast', 'information_schema')
                AND n.nspname NOT LIKE 'pg_temp_%'
                AND n.nspname NOT LIKE 'pg_toast_temp_%'
            ORDER BY n.nspname
            "#,
            &[],
        )
        .await
        .map_err(SqlError::Connection)?;

        let mut schemas = Vec::new();
        for row in rows {
            schemas.push(SchemaInfo {
                name: row.get("name"),
                owner: row.get("owner"),
                description: row.try_get("description").ok(),
            });
        }

        Ok(schemas)
    }

    /// Get detailed information about a specific schema
    pub async fn get_schema_info(&self, schema_name: &str) -> Result<SchemaInfo> {
        info!(schema_name = schema_name, "Getting schema info");
        
        let conn = self.get_connection().await?;
        let row = conn.query_opt(
            r#"
            SELECT 
                n.nspname AS name,
                pg_catalog.pg_get_userbyid(n.nspowner) AS owner,
                pg_catalog.obj_description(n.oid, 'pg_namespace') AS description
            FROM pg_catalog.pg_namespace n
            WHERE n.nspname = $1
            "#,
            &[&schema_name],
        )
        .await
        .map_err(SqlError::Connection)?
        .ok_or_else(|| SqlError::NotFound(format!("Schema '{}' not found", schema_name)))?;

        Ok(SchemaInfo {
            name: row.get("name"),
            owner: row.get("owner"),
            description: row.try_get("description").ok(),
        })
    }

    /// Create a new schema
    pub async fn create_schema(
        &self,
        schema_name: &str,
        owner: Option<&str>,
    ) -> Result<()> {
        info!(
            schema_name = schema_name,
            owner = owner,
            "Creating schema"
        );

        let mut sql = format!("CREATE SCHEMA {}", quote_ident(schema_name));

        if let Some(owner) = owner {
            sql.push_str(&format!(" AUTHORIZATION {}", quote_ident(owner)));
        }

        let conn = self.get_connection().await?;
        conn.execute(&sql, &[])
            .await
            .map_err(|e| {
                let err_str = e.to_string();
                if err_str.contains("permission denied") {
                    SqlError::PermissionDenied(format!("Creating schema requires appropriate privileges: {}", e))
                } else if err_str.contains("already exists") {
                    SqlError::InvalidInput(format!("Schema '{}' already exists", schema_name))
                } else {
                    SqlError::Execution(format!("Failed to create schema: {}", e))
                }
            })?;

        Ok(())
    }

    /// Alter schema properties
    pub async fn alter_schema(
        &self,
        schema_name: &str,
        new_name: Option<&str>,
        new_owner: Option<&str>,
    ) -> Result<()> {
        info!(
            schema_name = schema_name,
            new_name = new_name,
            new_owner = new_owner,
            "Altering schema"
        );

        let conn = self.get_connection().await?;

        // Rename schema if needed
        if let Some(new_name) = new_name {
            let sql = format!(
                "ALTER SCHEMA {} RENAME TO {}",
                quote_ident(schema_name),
                quote_ident(new_name)
            );
            conn.execute(&sql, &[])
                .await
                .map_err(|e| SqlError::Execution(format!("Failed to rename schema: {}", e)))?;
        }

        // Change owner if needed
        if let Some(new_owner) = new_owner {
            let sql = format!(
                "ALTER SCHEMA {} OWNER TO {}",
                quote_ident(schema_name),
                quote_ident(new_owner)
            );
            conn.execute(&sql, &[])
                .await
                .map_err(|e| {
                    let err_str = e.to_string();
                    if err_str.contains("permission denied") {
                        SqlError::PermissionDenied(format!("Changing schema owner requires appropriate privileges: {}", e))
                    } else {
                        SqlError::Execution(format!("Failed to change schema owner: {}", e))
                    }
                })?;
        }

        Ok(())
    }

    /// Drop a schema
    pub async fn drop_schema(
        &self,
        schema_name: &str,
        if_exists: bool,
        cascade: bool,
    ) -> Result<()> {
        info!(
            schema_name = schema_name,
            if_exists = if_exists,
            cascade = cascade,
            "Dropping schema"
        );

        let mut sql = if if_exists {
            format!("DROP SCHEMA IF EXISTS {}", quote_ident(schema_name))
        } else {
            format!("DROP SCHEMA {}", quote_ident(schema_name))
        };

        if cascade {
            sql.push_str(" CASCADE");
        }

        let conn = self.get_connection().await?;
        conn.execute(&sql, &[])
            .await
            .map_err(|e| {
                let err_str = e.to_string();
                if err_str.contains("permission denied") {
                    SqlError::PermissionDenied(format!("Dropping schema requires appropriate privileges: {}", e))
                } else if err_str.contains("does not exist") {
                    SqlError::NotFound(format!("Schema '{}' does not exist", schema_name))
                } else {
                    SqlError::Execution(format!("Failed to drop schema: {}", e))
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
        assert_eq!(quote_ident("test_schema"), "\"test_schema\"");
        assert_eq!(quote_ident("test\"schema"), "\"test\"\"schema\"");
        assert_eq!(quote_ident("my-schema"), "\"my-schema\"");
    }
}

