use crate::error::{Result, SqlError};
use crate::models::ViewInfo;
use crate::session::Session;
use tracing::info;

impl Session {
    /// List all views in the current database
    pub async fn list_views(&self, schema: Option<&str>) -> Result<Vec<ViewInfo>> {
        info!(schema = schema, "Listing views");
        
        let conn = self.get_connection().await?;
        let rows = if let Some(s) = schema {
            conn.query(
                r#"
                SELECT 
                    v.table_schema AS schema,
                    v.table_name AS name,
                    pg_catalog.pg_get_userbyid(c.relowner) AS owner,
                    v.view_definition AS definition,
                    pg_catalog.obj_description(c.oid, 'pg_class') AS description
                FROM information_schema.views v
                JOIN pg_catalog.pg_class c ON c.relname = v.table_name
                JOIN pg_catalog.pg_namespace n ON n.nspname = v.table_schema AND n.oid = c.relnamespace
                WHERE v.table_schema = $1
                ORDER BY v.table_schema, v.table_name
                "#,
                &[&s],
            )
            .await
        } else {
            conn.query(
                r#"
                SELECT 
                    v.table_schema AS schema,
                    v.table_name AS name,
                    pg_catalog.pg_get_userbyid(c.relowner) AS owner,
                    v.view_definition AS definition,
                    pg_catalog.obj_description(c.oid, 'pg_class') AS description
                FROM information_schema.views v
                JOIN pg_catalog.pg_class c ON c.relname = v.table_name
                JOIN pg_catalog.pg_namespace n ON n.nspname = v.table_schema AND n.oid = c.relnamespace
                WHERE v.table_schema NOT IN ('pg_catalog', 'information_schema')
                ORDER BY v.table_schema, v.table_name
                "#,
                &[],
            )
            .await
        }
        .map_err(SqlError::Connection)?;

        let mut views = Vec::new();
        for row in rows {
            views.push(ViewInfo {
                schema: row.get("schema"),
                name: row.get("name"),
                owner: row.get("owner"),
                definition: row.try_get("definition").ok(),
                description: row.try_get("description").ok(),
            });
        }

        // Also include materialized views
        let mat_rows = if let Some(s) = schema {
            conn.query(
                r#"
                SELECT 
                    m.schemaname AS schema,
                    m.matviewname AS name,
                    pg_catalog.pg_get_userbyid(c.relowner) AS owner,
                    m.definition AS definition,
                    pg_catalog.obj_description(c.oid, 'pg_class') AS description
                FROM pg_catalog.pg_matviews m
                JOIN pg_catalog.pg_class c ON c.relname = m.matviewname
                JOIN pg_catalog.pg_namespace n ON n.nspname = m.schemaname AND n.oid = c.relnamespace
                WHERE m.schemaname = $1
                ORDER BY m.schemaname, m.matviewname
                "#,
                &[&s],
            )
            .await
        } else {
            conn.query(
                r#"
                SELECT 
                    m.schemaname AS schema,
                    m.matviewname AS name,
                    pg_catalog.pg_get_userbyid(c.relowner) AS owner,
                    m.definition AS definition,
                    pg_catalog.obj_description(c.oid, 'pg_class') AS description
                FROM pg_catalog.pg_matviews m
                JOIN pg_catalog.pg_class c ON c.relname = m.matviewname
                JOIN pg_catalog.pg_namespace n ON n.nspname = m.schemaname AND n.oid = c.relnamespace
                WHERE m.schemaname NOT IN ('pg_catalog', 'information_schema')
                ORDER BY m.schemaname, m.matviewname
                "#,
                &[],
            )
            .await
        }
        .map_err(SqlError::Connection)?;

        for row in mat_rows {
            views.push(ViewInfo {
                schema: row.get("schema"),
                name: row.get("name"),
                owner: row.get("owner"),
                definition: row.try_get("definition").ok(),
                description: row.try_get("description").ok(),
            });
        }

        Ok(views)
    }

    /// Get detailed information about a specific view
    pub async fn get_view_info(&self, schema: &str, view_name: &str) -> Result<ViewInfo> {
        info!(schema = schema, view_name = view_name, "Getting view info");
        
        let conn = self.get_connection().await?;
        
        // Try regular view first
        let row = conn.query_opt(
            r#"
            SELECT 
                v.table_schema AS schema,
                v.table_name AS name,
                pg_catalog.pg_get_userbyid(c.relowner) AS owner,
                v.view_definition AS definition,
                pg_catalog.obj_description(c.oid, 'pg_class') AS description
            FROM information_schema.views v
            JOIN pg_catalog.pg_class c ON c.relname = v.table_name
            JOIN pg_catalog.pg_namespace n ON n.nspname = v.table_schema AND n.oid = c.relnamespace
            WHERE v.table_schema = $1 AND v.table_name = $2
            "#,
            &[&schema, &view_name],
        )
        .await
        .map_err(SqlError::Connection)?;

        if let Some(row) = row {
            return Ok(ViewInfo {
                schema: row.get("schema"),
                name: row.get("name"),
                owner: row.get("owner"),
                definition: row.try_get("definition").ok(),
                description: row.try_get("description").ok(),
            });
        }

        // Try materialized view
        let row = conn.query_opt(
            r#"
            SELECT 
                m.schemaname AS schema,
                m.matviewname AS name,
                pg_catalog.pg_get_userbyid(c.relowner) AS owner,
                m.definition AS definition,
                pg_catalog.obj_description(c.oid, 'pg_class') AS description
            FROM pg_catalog.pg_matviews m
            JOIN pg_catalog.pg_class c ON c.relname = m.matviewname
            JOIN pg_catalog.pg_namespace n ON n.nspname = m.schemaname AND n.oid = c.relnamespace
            WHERE m.schemaname = $1 AND m.matviewname = $2
            "#,
            &[&schema, &view_name],
        )
        .await
        .map_err(SqlError::Connection)?
        .ok_or_else(|| SqlError::NotFound(format!("View '{}.{}' not found", schema, view_name)))?;

        Ok(ViewInfo {
            schema: row.get("schema"),
            name: row.get("name"),
            owner: row.get("owner"),
            definition: row.try_get("definition").ok(),
            description: row.try_get("description").ok(),
        })
    }

    /// Create a view using DDL SQL
    pub async fn create_view(&self, ddl: &str) -> Result<()> {
        info!("Creating view with DDL");
        
        let conn = self.get_connection().await?;
        conn.execute(ddl, &[])
            .await
            .map_err(|e| SqlError::Execution(format!("Failed to create view: {}", e)))?;

        Ok(())
    }

    /// Alter view definition
    /// Note: PostgreSQL doesn't support ALTER VIEW ... AS, so we need to DROP and CREATE
    pub async fn alter_view(
        &self,
        schema: &str,
        view_name: &str,
        new_definition: &str,
    ) -> Result<()> {
        info!(
            schema = schema,
            view_name = view_name,
            "Altering view"
        );

        let conn = self.get_connection().await?;

        // Drop and recreate
        let drop_sql = format!(
            "DROP VIEW IF EXISTS {}.{}",
            quote_ident(schema),
            quote_ident(view_name)
        );

        conn.execute(&drop_sql, &[])
            .await
            .map_err(|e| SqlError::Execution(format!("Failed to drop view: {}", e)))?;

        let create_sql = format!(
            "CREATE VIEW {}.{} AS {}",
            quote_ident(schema),
            quote_ident(view_name),
            new_definition
        );

        conn.execute(&create_sql, &[])
            .await
            .map_err(|e| SqlError::Execution(format!("Failed to recreate view: {}", e)))?;

        Ok(())
    }

    /// Drop a view
    pub async fn drop_view(
        &self,
        schema: &str,
        view_name: &str,
        if_exists: bool,
        cascade: bool,
    ) -> Result<()> {
        info!(
            schema = schema,
            view_name = view_name,
            if_exists = if_exists,
            cascade = cascade,
            "Dropping view"
        );

        let mut sql = if if_exists {
            format!(
                "DROP VIEW IF EXISTS {}.{}",
                quote_ident(schema),
                quote_ident(view_name)
            )
        } else {
            format!(
                "DROP VIEW {}.{}",
                quote_ident(schema),
                quote_ident(view_name)
            )
        };

        if cascade {
            sql.push_str(" CASCADE");
        }

        let conn = self.get_connection().await?;
        conn.execute(&sql, &[])
            .await
            .map_err(|e| {
                let err_str = e.to_string();
                if err_str.contains("does not exist") {
                    SqlError::NotFound(format!("View '{}.{}' does not exist", schema, view_name))
                } else {
                    SqlError::Execution(format!("Failed to drop view: {}", e))
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
        assert_eq!(quote_ident("test_view"), "\"test_view\"");
        assert_eq!(quote_ident("test\"view"), "\"test\"\"view\"");
        assert_eq!(quote_ident("my-view"), "\"my-view\"");
    }
}
