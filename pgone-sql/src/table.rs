use crate::error::{Result, SqlError};
use crate::models::{
    ColumnDetail, ForeignKeyDetail, IndexInfo, PrimaryKeyDetail, TableDetail, TableInfo,
};
use crate::session::Session;
use sqlx::postgres::PgRow;
use sqlx::{Column, Row, TypeInfo, ValueRef};
use std::collections::BTreeMap;
use tracing::info;

type ForeignKeyColumns = (
    Vec<String>,
    (String, Vec<String>),
    Option<String>,
    Option<String>,
);

impl Session {
    /// List all tables in the current database
    pub async fn list_tables(&self, schema: Option<&str>) -> Result<Vec<TableInfo>> {
        info!(schema = schema, "Listing tables");

        let pool = self.pool();
        let rows = if let Some(s) = schema {
            sqlx::query(
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
            )
            .bind(s)
            .fetch_all(pool)
            .await
        } else {
            sqlx::query(
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
            )
            .fetch_all(pool)
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
        info!(
            schema = schema,
            table_name = table_name,
            "Getting table info"
        );

        let pool = self.pool();
        let row = sqlx::query(
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
        )
        .bind(schema)
        .bind(table_name)
        .fetch_optional(pool)
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

        let pool = self.pool();
        sqlx::query(ddl)
            .execute(pool)
            .await
            .map_err(|e| SqlError::Execution(format!("Failed to create table: {}", e)))?;

        Ok(())
    }

    /// Alter table structure
    /// Supports: ADD COLUMN, DROP COLUMN, ALTER COLUMN, RENAME COLUMN, RENAME TABLE
    pub async fn alter_table(&self, schema: &str, table_name: &str, alter_ddl: &str) -> Result<()> {
        info!(schema = schema, table_name = table_name, "Altering table");

        let full_ddl = format!(
            "ALTER TABLE {}.{} {}",
            quote_ident(schema),
            quote_ident(table_name),
            alter_ddl
        );

        let pool = self.pool();
        sqlx::query(&full_ddl)
            .execute(pool)
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
            format!(
                "DROP TABLE IF EXISTS {}.{}",
                quote_ident(schema),
                quote_ident(table_name)
            )
        } else {
            format!(
                "DROP TABLE {}.{}",
                quote_ident(schema),
                quote_ident(table_name)
            )
        };

        if cascade {
            sql.push_str(" CASCADE");
        }

        let pool = self.pool();
        sqlx::query(&sql).execute(pool).await.map_err(|e| {
            let err_str = e.to_string();
            if err_str.contains("does not exist") {
                SqlError::NotFound(format!("Table '{}.{}' does not exist", schema, table_name))
            } else {
                SqlError::Execution(format!("Failed to drop table: {}", e))
            }
        })?;

        Ok(())
    }

    /// Truncate a table (clear all data)
    pub async fn truncate_table(&self, schema: &str, table_name: &str) -> Result<()> {
        info!(schema = schema, table_name = table_name, "Truncating table");

        let sql = format!(
            "TRUNCATE TABLE {}.{}",
            quote_ident(schema),
            quote_ident(table_name)
        );

        let pool = self.pool();
        sqlx::query(&sql).execute(pool).await.map_err(|e| {
            let err_str = e.to_string();
            if err_str.contains("does not exist") {
                SqlError::NotFound(format!("Table '{}.{}' does not exist", schema, table_name))
            } else {
                SqlError::Execution(format!("Failed to truncate table: {}", e))
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

        let pool = self.pool();

        // Build query
        let mut query = format!(
            "SELECT * FROM {}.{}",
            quote_ident(schema),
            quote_ident(table_name)
        );

        if let Some(lim) = limit {
            query.push_str(&format!(" LIMIT {}", lim));
        }

        let rows = sqlx::query(&query)
            .fetch_all(pool)
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

    /// Get detailed information about a table including columns, primary keys, and foreign keys
    pub async fn get_table_detail(&self, schema: &str, table_name: &str) -> Result<TableDetail> {
        info!(
            schema = schema,
            table_name = table_name,
            "Getting table detail"
        );

        let pool = self.pool();

        // Get columns with comments
        let col_rows = sqlx::query(
            r#"
                SELECT c.column_name, c.is_nullable, c.data_type, c.udt_name,
                       c.character_maximum_length, c.numeric_precision, c.numeric_scale,
                       c.column_default, pgd.description AS column_comment
                FROM information_schema.columns c
                LEFT JOIN pg_class pc ON pc.relname = c.table_name
                LEFT JOIN pg_namespace pn ON pn.nspname = c.table_schema AND pn.oid = pc.relnamespace
                LEFT JOIN pg_attribute pa ON pa.attrelid = pc.oid AND pa.attname = c.column_name
                LEFT JOIN pg_description pgd ON pgd.objoid = pc.oid AND pgd.objsubid = pa.attnum
                WHERE c.table_schema = $1 AND c.table_name = $2
                ORDER BY c.ordinal_position
                "#,
        )
        .bind(schema)
        .bind(table_name)
        .fetch_all(pool)
        .await
        .map_err(SqlError::Connection)?;

        let columns: Vec<ColumnDetail> = col_rows
            .iter()
            .map(|row| ColumnDetail {
                name: row.get("column_name"),
                nullable: matches!(row.get::<String, _>("is_nullable").as_str(), "YES"),
                data_type: row.get("data_type"),
                udt_name: row.try_get("udt_name").ok(),
                character_maximum_length: row.try_get("character_maximum_length").ok(),
                numeric_precision: row.try_get("numeric_precision").ok(),
                numeric_scale: row.try_get("numeric_scale").ok(),
                default: row.try_get("column_default").ok(),
                comment: row.try_get("column_comment").ok(),
            })
            .collect();

        // Get table comment
        let table_comment: Option<String> = sqlx::query(
            r#"
                SELECT obj_description(pc.oid)
                FROM pg_class pc
                JOIN pg_namespace pn ON pn.oid = pc.relnamespace
                WHERE pn.nspname = $1 AND pc.relname = $2
                "#,
        )
        .bind(schema)
        .bind(table_name)
        .fetch_optional(pool)
        .await
        .map_err(SqlError::Connection)?
        .and_then(|row| row.try_get(0).ok());

        // Get primary key
        let pk_rows = sqlx::query(
            r#"
                SELECT kcu.column_name
                FROM information_schema.table_constraints tc
                JOIN information_schema.key_column_usage kcu
                  ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema
                WHERE tc.constraint_type = 'PRIMARY KEY' AND tc.table_schema = $1 AND tc.table_name = $2
                ORDER BY kcu.ordinal_position
                "#,
        )
        .bind(schema)
        .bind(table_name)
        .fetch_all(pool)
        .await
        .map_err(SqlError::Connection)?;

        let pk_cols: Vec<String> = pk_rows.iter().map(|row| row.get(0)).collect();
        let primary_key = if pk_cols.is_empty() {
            None
        } else {
            Some(PrimaryKeyDetail { columns: pk_cols })
        };

        // Get foreign keys
        let fk_rows = sqlx::query(
            r#"
                SELECT kcu.constraint_name, kcu.column_name, ccu.table_schema, ccu.table_name,
                       ccu.column_name AS ref_column, rc.update_rule, rc.delete_rule
                FROM information_schema.table_constraints tc
                JOIN information_schema.key_column_usage kcu
                  ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema
                JOIN information_schema.referential_constraints rc
                  ON rc.constraint_name = tc.constraint_name AND rc.constraint_schema = tc.table_schema
                JOIN information_schema.constraint_column_usage ccu
                  ON ccu.constraint_name = rc.unique_constraint_name
                  AND ccu.constraint_schema = rc.unique_constraint_schema
                WHERE tc.constraint_type = 'FOREIGN KEY' AND tc.table_schema = $1 AND tc.table_name = $2
                ORDER BY kcu.ordinal_position
                "#,
        )
        .bind(schema)
        .bind(table_name)
        .fetch_all(pool)
        .await
        .map_err(SqlError::Connection)?;

        // Group by constraint_name
        let mut fk_map: BTreeMap<String, ForeignKeyColumns> = BTreeMap::new();

        for row in fk_rows {
            let constraint_name: String = row.get("constraint_name");
            let column: String = row.get("column_name");
            let ref_schema: String = row.get("table_schema");
            let ref_table: String = row.get("table_name");
            let ref_column: String = row.get("ref_column");
            let on_update: Option<String> = row.try_get("update_rule").ok();
            let on_delete: Option<String> = row.try_get("delete_rule").ok();

            let entry = fk_map.entry(constraint_name).or_insert((
                Vec::new(),
                (format!("{}.{}", ref_schema, ref_table), Vec::new()),
                None,
                None,
            ));
            entry.0.push(column);
            entry.1.1.push(ref_column);
            entry.2 = on_update;
            entry.3 = on_delete;
        }

        let foreign_keys: Vec<ForeignKeyDetail> = fk_map
            .into_values()
            .map(
                |(cols, (ref_table, ref_cols), on_update, on_delete)| ForeignKeyDetail {
                    columns: cols,
                    ref_table,
                    ref_columns: ref_cols,
                    on_update,
                    on_delete,
                },
            )
            .collect();

        Ok(TableDetail {
            schema: schema.to_string(),
            name: table_name.to_string(),
            comment: table_comment,
            columns,
            primary_key,
            foreign_keys,
        })
    }

    /// List all tables with detailed information in a schema
    pub async fn list_table_details(&self, schema: &str) -> Result<Vec<TableDetail>> {
        info!(schema = schema, "Listing table details");

        // First get list of tables
        let tables = self.list_tables(Some(schema)).await?;

        // Then get details for each table
        let mut details = Vec::new();
        for table in tables {
            match self.get_table_detail(&table.schema, &table.name).await {
                Ok(detail) => details.push(detail),
                Err(e) => {
                    tracing::warn!(
                        schema = table.schema,
                        table = table.name,
                        error = %e,
                        "Failed to get table detail"
                    );
                }
            }
        }

        Ok(details)
    }

    /// List all indexes for a specific table
    pub async fn list_table_indexes(
        &self,
        schema: &str,
        table_name: &str,
    ) -> Result<Vec<IndexInfo>> {
        info!(
            schema = schema,
            table_name = table_name,
            "Listing table indexes"
        );

        let pool = self.pool();

        // Query indexes using pg_indexes view
        let rows = sqlx::query(
            r#"
            SELECT 
                i.indexname AS name,
                i.indexdef AS definition,
                pg_catalog.obj_description(c.oid, 'pg_class') AS description
            FROM pg_indexes i
            JOIN pg_class c ON c.relname = i.indexname
            JOIN pg_namespace n ON n.nspname = i.schemaname AND n.oid = c.relnamespace
            WHERE i.schemaname = $1 AND i.tablename = $2
            ORDER BY i.indexname
            "#,
        )
        .bind(schema)
        .bind(table_name)
        .fetch_all(pool)
        .await
        .map_err(SqlError::Connection)?;

        let mut indexes = Vec::new();
        for row in rows {
            let name: String = row.get("name");
            let definition: String = row.get("definition");
            let description: Option<String> = row.try_get("description").ok();

            // Check if index is unique (UNIQUE keyword in definition)
            let unique = definition.to_uppercase().contains(" UNIQUE ");

            // Extract columns from definition
            // Format: CREATE [UNIQUE] INDEX ... ON ... USING ... (column1, column2, ...)
            let columns = if let Some(start_pos) = definition.rfind('(') {
                if let Some(end_pos) = definition[start_pos..].find(')') {
                    definition[start_pos + 1..start_pos + end_pos]
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };

            indexes.push(IndexInfo {
                name,
                unique,
                columns,
                definition: Some(definition),
                description,
            });
        }

        Ok(indexes)
    }
}

/// Format a cell value to string
fn format_cell(row: &PgRow, idx: usize) -> String {
    if row
        .try_get_raw(idx)
        .map(|raw| raw.is_null())
        .unwrap_or(true)
    {
        return "NULL".to_string();
    }

    let type_name = row.column(idx).type_info().name().to_ascii_lowercase();
    match type_name.as_str() {
        "text" | "varchar" | "bpchar" | "name" => row.try_get::<String, _>(idx).unwrap_or_default(),
        "int2" => row
            .try_get::<i16, _>(idx)
            .map(|v| v.to_string())
            .unwrap_or_default(),
        "int4" => row
            .try_get::<i32, _>(idx)
            .map(|v| v.to_string())
            .unwrap_or_default(),
        "int8" => row
            .try_get::<i64, _>(idx)
            .map(|v| v.to_string())
            .unwrap_or_default(),
        "float4" => row
            .try_get::<f32, _>(idx)
            .map(|v| v.to_string())
            .unwrap_or_default(),
        "float8" => row
            .try_get::<f64, _>(idx)
            .map(|v| v.to_string())
            .unwrap_or_default(),
        "bool" => row
            .try_get::<bool, _>(idx)
            .map(|v| v.to_string())
            .unwrap_or_default(),
        "json" | "jsonb" => format_json_value(row, idx),
        "timestamptz" => format_timestamptz(row, idx),
        "timestamp" => format_timestamp(row, idx),
        "date" => format_date(row, idx),
        "time" => format_time(row, idx),
        "uuid" => row
            .try_get::<uuid::Uuid, _>(idx)
            .map(|v| v.to_string())
            .unwrap_or_default(),
        "bytea" => format_bytes_fallback(row, idx),
        _ => row
            .try_get::<String, _>(idx)
            .unwrap_or_else(|_| format_bytes_fallback(row, idx)),
    }
}

/// Format JSON/JSONB value with pretty printing
fn format_json_value(row: &PgRow, idx: usize) -> String {
    row.try_get::<String, _>(idx)
        .map(|v| {
            serde_json::from_str::<serde_json::Value>(&v)
                .ok()
                .and_then(|json_val| serde_json::to_string_pretty(&json_val).ok())
                .unwrap_or(v)
        })
        .unwrap_or_else(|_| "<unformatted>".to_string())
}

/// Format TIMESTAMPTZ value
fn format_timestamptz(row: &PgRow, idx: usize) -> String {
    if let Ok(v) = row.try_get::<String, _>(idx) {
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
fn format_timestamp(row: &PgRow, idx: usize) -> String {
    if let Ok(v) = row.try_get::<String, _>(idx) {
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
fn format_date(row: &PgRow, idx: usize) -> String {
    if let Ok(v) = row.try_get::<String, _>(idx) {
        chrono::NaiveDate::parse_from_str(&v, "%Y-%m-%d")
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or(v)
    } else {
        "<unformatted>".to_string()
    }
}

/// Format TIME value
fn format_time(row: &PgRow, idx: usize) -> String {
    if let Ok(v) = row.try_get::<String, _>(idx) {
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
fn format_bytes_fallback(row: &PgRow, idx: usize) -> String {
    row.try_get::<Vec<u8>, _>(idx)
        .map(|v| {
            format!(
                "\\x{}",
                v.iter().map(|b| format!("{:02x}", b)).collect::<String>()
            )
        })
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
