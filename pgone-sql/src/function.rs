use crate::error::{Result, SqlError};
use crate::models::FunctionInfo;
use crate::session::Session;
use tracing::info;

impl Session {
    /// List all functions/routines in the current database
    pub async fn list_functions(&self, schema: Option<&str>) -> Result<Vec<FunctionInfo>> {
        info!(schema = schema, "Listing functions");

        let conn = self.get_connection().await?;
        let rows = if let Some(s) = schema {
            conn.query(
                r#"
                SELECT 
                    n.nspname AS schema,
                    p.proname AS name,
                    pg_catalog.pg_get_userbyid(p.proowner) AS owner,
                    l.lanname AS language,
                    pg_catalog.pg_get_function_result(p.oid) AS return_type,
                    pg_catalog.pg_get_functiondef(p.oid) AS definition,
                    pg_catalog.obj_description(p.oid, 'pg_proc') AS description
                FROM pg_catalog.pg_proc p
                JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace
                JOIN pg_catalog.pg_language l ON l.oid = p.prolang
                WHERE n.nspname = $1
                    AND n.nspname NOT IN ('pg_catalog', 'information_schema')
                ORDER BY n.nspname, p.proname
                "#,
                &[&s],
            )
            .await
        } else {
            conn.query(
                r#"
                SELECT 
                    n.nspname AS schema,
                    p.proname AS name,
                    pg_catalog.pg_get_userbyid(p.proowner) AS owner,
                    l.lanname AS language,
                    pg_catalog.pg_get_function_result(p.oid) AS return_type,
                    pg_catalog.pg_get_functiondef(p.oid) AS definition,
                    pg_catalog.obj_description(p.oid, 'pg_proc') AS description
                FROM pg_catalog.pg_proc p
                JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace
                JOIN pg_catalog.pg_language l ON l.oid = p.prolang
                WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
                ORDER BY n.nspname, p.proname
                "#,
                &[],
            )
            .await
        }
        .map_err(SqlError::Connection)?;

        let mut functions = Vec::new();
        for row in rows {
            functions.push(FunctionInfo {
                schema: row.get("schema"),
                name: row.get("name"),
                owner: row.get("owner"),
                language: row.try_get("language").ok(),
                return_type: row.try_get("return_type").ok(),
                definition: row.try_get("definition").ok(),
                description: row.try_get("description").ok(),
            });
        }

        Ok(functions)
    }

    /// Get detailed information about a specific function
    /// Note: Functions are identified by name and argument types, so this may return multiple results
    pub async fn get_function_info(
        &self,
        schema: &str,
        function_name: &str,
    ) -> Result<Vec<FunctionInfo>> {
        info!(
            schema = schema,
            function_name = function_name,
            "Getting function info"
        );

        let conn = self.get_connection().await?;
        let rows = conn
            .query(
                r#"
            SELECT 
                n.nspname AS schema,
                p.proname AS name,
                pg_catalog.pg_get_userbyid(p.proowner) AS owner,
                l.lanname AS language,
                pg_catalog.pg_get_function_result(p.oid) AS return_type,
                pg_catalog.pg_get_functiondef(p.oid) AS definition,
                pg_catalog.obj_description(p.oid, 'pg_proc') AS description
            FROM pg_catalog.pg_proc p
            JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace
            JOIN pg_catalog.pg_language l ON l.oid = p.prolang
            WHERE n.nspname = $1 AND p.proname = $2
            ORDER BY p.oid
            "#,
                &[&schema, &function_name],
            )
            .await
            .map_err(SqlError::Connection)?;

        if rows.is_empty() {
            return Err(SqlError::NotFound(format!(
                "Function '{}.{}' not found",
                schema, function_name
            )));
        }

        let mut functions = Vec::new();
        for row in rows {
            functions.push(FunctionInfo {
                schema: row.get("schema"),
                name: row.get("name"),
                owner: row.get("owner"),
                language: row.try_get("language").ok(),
                return_type: row.try_get("return_type").ok(),
                definition: row.try_get("definition").ok(),
                description: row.try_get("description").ok(),
            });
        }

        Ok(functions)
    }

    /// Create a function using DDL SQL
    pub async fn create_function(&self, ddl: &str) -> Result<()> {
        info!("Creating function with DDL");

        let conn = self.get_connection().await?;
        conn.execute(ddl, &[])
            .await
            .map_err(|e| SqlError::Execution(format!("Failed to create function: {}", e)))?;

        Ok(())
    }

    /// Alter function properties
    /// Supports: OWNER TO, SET SCHEMA, RENAME TO, SET configuration parameters
    pub async fn alter_function(
        &self,
        schema: &str,
        function_name: &str,
        alter_ddl: &str,
    ) -> Result<()> {
        info!(
            schema = schema,
            function_name = function_name,
            "Altering function"
        );

        // Note: ALTER FUNCTION requires the full function signature
        // For simplicity, we'll use the alter_ddl parameter which should include the signature
        let full_ddl = if alter_ddl.contains("FUNCTION") {
            alter_ddl.to_string()
        } else {
            format!(
                "ALTER FUNCTION {}.{} {}",
                quote_ident(schema),
                quote_ident(function_name),
                alter_ddl
            )
        };

        let conn = self.get_connection().await?;
        conn.execute(&full_ddl, &[])
            .await
            .map_err(|e| SqlError::Execution(format!("Failed to alter function: {}", e)))?;

        Ok(())
    }

    /// Drop a function
    /// Note: Requires the full function signature for overloaded functions
    pub async fn drop_function(
        &self,
        schema: &str,
        function_name: &str,
        if_exists: bool,
        cascade: bool,
        signature: Option<&str>,
    ) -> Result<()> {
        info!(
            schema = schema,
            function_name = function_name,
            if_exists = if_exists,
            cascade = cascade,
            signature = signature,
            "Dropping function"
        );

        let mut sql = if let Some(sig) = signature {
            let if_exists_str = if if_exists { "IF EXISTS " } else { "" };
            format!(
                "DROP FUNCTION {} {}.{}({})",
                if_exists_str,
                quote_ident(schema),
                quote_ident(function_name),
                sig
            )
        } else if if_exists {
            format!(
                "DROP FUNCTION IF EXISTS {}.{}",
                quote_ident(schema),
                quote_ident(function_name)
            )
        } else {
            format!(
                "DROP FUNCTION {}.{}",
                quote_ident(schema),
                quote_ident(function_name)
            )
        };

        if cascade {
            sql.push_str(" CASCADE");
        }

        let conn = self.get_connection().await?;
        conn.execute(&sql, &[]).await.map_err(|e| {
            let err_str = e.to_string();
            if err_str.contains("does not exist") {
                SqlError::NotFound(format!(
                    "Function '{}.{}' does not exist",
                    schema, function_name
                ))
            } else {
                SqlError::Execution(format!("Failed to drop function: {}", e))
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
        assert_eq!(quote_ident("test_func"), "\"test_func\"");
        assert_eq!(quote_ident("test\"func"), "\"test\"\"func\"");
        assert_eq!(quote_ident("my-function"), "\"my-function\"");
    }
}
