use crate::error::{Result, SqlError};
use crate::models::TableInfo;
use crate::session::Session;
use tracing::info;

impl Session {
    /// List all tables in the current database
    pub async fn list_tables(&self, schema: Option<&str>) -> Result<Vec<TableInfo>> {
        info!(schema = schema, "Listing tables");
        
        let conn = self.get_connection().await?;
        let rows = if let Some(s) = schema {
            conn.query(
                r#"
                SELECT 
                    t.table_schema AS schema,
                    t.table_name AS name,
                    pg_catalog.pg_get_userbyid(c.relowner) AS owner,
                    ts.spcname AS tablespace,
                    (SELECT COUNT(*) FROM pg_catalog.pg_stat_user_tables WHERE schemaname = t.table_schema AND relname = t.table_name) AS row_count,
                    pg_size_pretty(pg_total_relation_size(c.oid)) AS size,
                    pg_catalog.obj_description(c.oid, 'pg_class') AS description
                FROM information_schema.tables t
                JOIN pg_catalog.pg_class c ON c.relname = t.table_name
                JOIN pg_catalog.pg_namespace n ON n.nspname = t.table_schema AND n.oid = c.relnamespace
                LEFT JOIN pg_catalog.pg_tablespace ts ON ts.oid = c.reltablespace
                WHERE t.table_type = 'BASE TABLE' 
                    AND t.table_schema = $1
                    AND t.table_schema NOT IN ('pg_catalog', 'information_schema')
                ORDER BY t.table_schema, t.table_name
                "#,
                &[&s],
            )
            .await
        } else {
            conn.query(
                r#"
                SELECT 
                    t.table_schema AS schema,
                    t.table_name AS name,
                    pg_catalog.pg_get_userbyid(c.relowner) AS owner,
                    ts.spcname AS tablespace,
                    (SELECT COUNT(*) FROM pg_catalog.pg_stat_user_tables WHERE schemaname = t.table_schema AND relname = t.table_name) AS row_count,
                    pg_size_pretty(pg_total_relation_size(c.oid)) AS size,
                    pg_catalog.obj_description(c.oid, 'pg_class') AS description
                FROM information_schema.tables t
                JOIN pg_catalog.pg_class c ON c.relname = t.table_name
                JOIN pg_catalog.pg_namespace n ON n.nspname = t.table_schema AND n.oid = c.relnamespace
                LEFT JOIN pg_catalog.pg_tablespace ts ON ts.oid = c.reltablespace
                WHERE t.table_type = 'BASE TABLE' 
                    AND t.table_schema NOT IN ('pg_catalog', 'information_schema')
                ORDER BY t.table_schema, t.table_name
                "#,
                &[],
            )
            .await
        }
        .map_err(SqlError::Connection)?;

        let mut tables = Vec::new();
        for row in rows {
            tables.push(TableInfo {
                schema: row.get("schema"),
                name: row.get("name"),
                owner: row.get("owner"),
                tablespace: row.try_get("tablespace").ok(),
                row_count: row.try_get("row_count").ok(),
                size: row.try_get("size").ok(),
                description: row.try_get("description").ok(),
            });
        }

        Ok(tables)
    }

    /// Get detailed information about a specific table
    pub async fn get_table_info(&self, schema: &str, table_name: &str) -> Result<TableInfo> {
        info!(schema = schema, table_name = table_name, "Getting table info");
        
        let conn = self.get_connection().await?;
        let row = conn.query_opt(
            r#"
            SELECT 
                t.table_schema AS schema,
                t.table_name AS name,
                pg_catalog.pg_get_userbyid(c.relowner) AS owner,
                ts.spcname AS tablespace,
                (SELECT COUNT(*) FROM pg_catalog.pg_stat_user_tables WHERE schemaname = t.table_schema AND relname = t.table_name) AS row_count,
                pg_size_pretty(pg_total_relation_size(c.oid)) AS size,
                pg_catalog.obj_description(c.oid, 'pg_class') AS description
            FROM information_schema.tables t
            JOIN pg_catalog.pg_class c ON c.relname = t.table_name
            JOIN pg_catalog.pg_namespace n ON n.nspname = t.table_schema AND n.oid = c.relnamespace
            LEFT JOIN pg_catalog.pg_tablespace ts ON ts.oid = c.reltablespace
            WHERE t.table_type = 'BASE TABLE' 
                AND t.table_schema = $1
                AND t.table_name = $2
            "#,
            &[&schema, &table_name],
        )
        .await
        .map_err(SqlError::Connection)?
        .ok_or_else(|| SqlError::NotFound(format!("Table '{}.{}' not found", schema, table_name)))?;

        Ok(TableInfo {
            schema: row.get("schema"),
            name: row.get("name"),
            owner: row.get("owner"),
            tablespace: row.try_get("tablespace").ok(),
            row_count: row.try_get("row_count").ok(),
            size: row.try_get("size").ok(),
            description: row.try_get("description").ok(),
        })
    }

    /// Create a table using DDL SQL
    pub async fn create_table(&self, ddl: &str) -> Result<()> {
        info!("Creating table with DDL");
        
        let conn = self.get_connection().await?;
        conn.execute(ddl, &[])
            .await
            .map_err(|e| SqlError::Execution(format!("Failed to create table: {}", e)))?;

        Ok(())
    }

    /// Alter table structure
    /// Supports: ADD COLUMN, DROP COLUMN, ALTER COLUMN, RENAME COLUMN, RENAME TABLE
    pub async fn alter_table(
        &self,
        schema: &str,
        table_name: &str,
        alter_ddl: &str,
    ) -> Result<()> {
        info!(
            schema = schema,
            table_name = table_name,
            "Altering table"
        );

        let full_ddl = format!(
            "ALTER TABLE {}.{} {}",
            quote_ident(schema),
            quote_ident(table_name),
            alter_ddl
        );

        let conn = self.get_connection().await?;
        conn.execute(&full_ddl, &[])
            .await
            .map_err(|e| SqlError::Execution(format!("Failed to alter table: {}", e)))?;

        Ok(())
    }

    /// Drop a table
    pub async fn drop_table(
        &self,
        schema: &str,
        table_name: &str,
        if_exists: bool,
        cascade: bool,
    ) -> Result<()> {
        info!(
            schema = schema,
            table_name = table_name,
            if_exists = if_exists,
            cascade = cascade,
            "Dropping table"
        );

        let mut sql = if if_exists {
            format!("DROP TABLE IF EXISTS {}.{}", quote_ident(schema), quote_ident(table_name))
        } else {
            format!("DROP TABLE {}.{}", quote_ident(schema), quote_ident(table_name))
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
                    SqlError::NotFound(format!("Table '{}.{}' does not exist", schema, table_name))
                } else {
                    SqlError::Execution(format!("Failed to drop table: {}", e))
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
        assert_eq!(quote_ident("public.users"), "\"public.users\"");
        assert_eq!(quote_ident("my-table"), "\"my-table\"");
    }
}
