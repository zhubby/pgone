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

    /// Query table data
    /// Returns (columns, rows) where rows are Vec<Vec<String>>
    pub async fn query_table_data(
        &self,
        schema: &str,
        table_name: &str,
        limit: Option<usize>,
    ) -> Result<(Vec<String>, Vec<Vec<String>>)> {
        info!(
            schema = schema,
            table_name = table_name,
            limit = limit,
            "Querying table data"
        );

        let conn = self.get_connection().await?;
        
        // Build query
        let mut query = format!(
            "SELECT * FROM {}.{}",
            quote_ident(schema),
            quote_ident(table_name)
        );
        
        if let Some(lim) = limit {
            query.push_str(&format!(" LIMIT {}", lim));
        }

        let rows = conn.query(&query, &[])
            .await
            .map_err(|e| SqlError::Execution(format!("Failed to query table data: {}", e)))?;

        // Get column names from first row
        let mut columns = Vec::new();
        let mut data = Vec::new();

        if let Some(first_row) = rows.first() {
            for col in first_row.columns() {
                columns.push(col.name().to_string());
            }

            // Convert rows to Vec<Vec<String>>
            for row in rows {
                let mut row_data = Vec::new();
                for i in 0..columns.len() {
                    let value = format_cell(&row, i);
                    row_data.push(value);
                }
                data.push(row_data);
            }
        }

        Ok((columns, data))
    }
}

/// Format a cell value to string
fn format_cell(row: &tokio_postgres::Row, idx: usize) -> String {
    use tokio_postgres::types::Type;
    
    let col_type = row.columns().get(idx).map(|c| c.type_());
    
    match col_type {
        Some(t) => match *t {
            Type::TEXT | Type::VARCHAR => {
                row.try_get::<_, String>(idx).unwrap_or_default()
            }
            Type::INT4 => {
                row.try_get::<_, i32>(idx)
                    .map(|v| v.to_string())
                    .unwrap_or_default()
            }
            Type::INT8 => {
                row.try_get::<_, i64>(idx)
                    .map(|v| v.to_string())
                    .unwrap_or_default()
            }
            Type::FLOAT4 => {
                row.try_get::<_, f32>(idx)
                    .map(|v| v.to_string())
                    .unwrap_or_default()
            }
            Type::FLOAT8 => {
                row.try_get::<_, f64>(idx)
                    .map(|v| v.to_string())
                    .unwrap_or_default()
            }
            Type::BOOL => {
                row.try_get::<_, bool>(idx)
                    .map(|v| v.to_string())
                    .unwrap_or_default()
            }
            Type::JSON | Type::JSONB => {
                format_json_value(row, idx)
            }
            Type::TIMESTAMPTZ => {
                format_timestamptz(row, idx)
            }
            Type::TIMESTAMP => {
                format_timestamp(row, idx)
            }
            Type::DATE => {
                format_date(row, idx)
            }
            Type::TIME => {
                format_time(row, idx)
            }
            _ => {
                // Fallback: try as string
                row.try_get::<_, String>(idx)
                    .unwrap_or_else(|_| format_bytes_fallback(row, idx))
            }
        },
        None => {
            // No type info, try common fallbacks
            row.try_get::<_, String>(idx)
                .unwrap_or_else(|_| format_bytes_fallback(row, idx))
        }
    }
}

/// Format JSON/JSONB value with pretty printing
fn format_json_value(row: &tokio_postgres::Row, idx: usize) -> String {
    row.try_get::<_, String>(idx)
        .map(|v| {
            serde_json::from_str::<serde_json::Value>(&v)
                .ok()
                .and_then(|json_val| serde_json::to_string_pretty(&json_val).ok())
                .unwrap_or(v)
        })
        .unwrap_or_else(|_| "<unformatted>".to_string())
}

/// Format TIMESTAMPTZ value
fn format_timestamptz(row: &tokio_postgres::Row, idx: usize) -> String {
    if let Ok(v) = row.try_get::<_, String>(idx) {
        chrono::DateTime::parse_from_rfc3339(&v)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S%.f %z").to_string())
            .or_else(|_| {
                chrono::NaiveDateTime::parse_from_str(&v, "%Y-%m-%d %H:%M:%S%.f")
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S%.f").to_string())
            })
            .unwrap_or(v)
    } else {
        "<unformatted>".to_string()
    }
}

/// Format TIMESTAMP value
fn format_timestamp(row: &tokio_postgres::Row, idx: usize) -> String {
    if let Ok(v) = row.try_get::<_, String>(idx) {
        chrono::NaiveDateTime::parse_from_str(&v, "%Y-%m-%d %H:%M:%S%.f")
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S%.f").to_string())
            .or_else(|_| {
                chrono::NaiveDateTime::parse_from_str(&v, "%Y-%m-%dT%H:%M:%S%.f")
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S%.f").to_string())
            })
            .unwrap_or(v)
    } else {
        "<unformatted>".to_string()
    }
}

/// Format DATE value
fn format_date(row: &tokio_postgres::Row, idx: usize) -> String {
    if let Ok(v) = row.try_get::<_, String>(idx) {
        chrono::NaiveDate::parse_from_str(&v, "%Y-%m-%d")
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or(v)
    } else {
        "<unformatted>".to_string()
    }
}

/// Format TIME value
fn format_time(row: &tokio_postgres::Row, idx: usize) -> String {
    if let Ok(v) = row.try_get::<_, String>(idx) {
        chrono::NaiveTime::parse_from_str(&v, "%H:%M:%S%.f")
            .map(|t| t.format("%H:%M:%S%.f").to_string())
            .or_else(|_| {
                chrono::NaiveTime::parse_from_str(&v, "%H:%M:%S")
                    .map(|t| t.format("%H:%M:%S").to_string())
            })
            .unwrap_or(v)
    } else {
        "<unformatted>".to_string()
    }
}

/// Fallback: try to format as bytes (hex)
fn format_bytes_fallback(row: &tokio_postgres::Row, idx: usize) -> String {
    row.try_get::<_, Vec<u8>>(idx)
        .map(|v| format!("\\x{}", v.iter().map(|b| format!("{:02x}", b)).collect::<String>()))
        .unwrap_or_else(|_| "<unformatted>".to_string())
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
