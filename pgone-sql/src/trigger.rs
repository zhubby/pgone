use crate::error::{Result, SqlError};
use crate::models::TriggerInfo;
use crate::session::Session;
use tracing::info;

impl Session {
    /// List all triggers in the current database
    pub async fn list_triggers(&self, schema: Option<&str>) -> Result<Vec<TriggerInfo>> {
        info!(schema = schema, "Listing triggers");

        let conn = self.get_connection().await?;
        let rows = if let Some(s) = schema {
            conn.query(
                r#"
                SELECT 
                    t.trigger_schema AS schema,
                    t.trigger_name AS name,
                    t.event_object_schema AS table_schema,
                    t.event_object_table AS table_name,
                    t.action_timing AS timing,
                    t.event_manipulation AS event,
                    t.action_statement AS function_name,
                    t.action_condition AS condition,
                    pg_catalog.obj_description(tg.oid, 'pg_trigger') AS description
                FROM information_schema.triggers t
                JOIN pg_catalog.pg_trigger tg ON tg.tgname = t.trigger_name
                WHERE t.trigger_schema = $1
                ORDER BY t.event_object_schema, t.event_object_table, t.trigger_name
                "#,
                &[&s],
            )
            .await
        } else {
            conn.query(
                r#"
                SELECT 
                    t.trigger_schema AS schema,
                    t.trigger_name AS name,
                    t.event_object_schema AS table_schema,
                    t.event_object_table AS table_name,
                    t.action_timing AS timing,
                    t.event_manipulation AS event,
                    t.action_statement AS function_name,
                    t.action_condition AS condition,
                    pg_catalog.obj_description(tg.oid, 'pg_trigger') AS description
                FROM information_schema.triggers t
                JOIN pg_catalog.pg_trigger tg ON tg.tgname = t.trigger_name
                WHERE t.trigger_schema NOT IN ('pg_catalog', 'information_schema')
                ORDER BY t.event_object_schema, t.event_object_table, t.trigger_name
                "#,
                &[],
            )
            .await
        }
        .map_err(SqlError::Connection)?;

        // Group triggers by name and timing (same trigger can have multiple events)
        use std::collections::BTreeMap;
        let mut trigger_map: BTreeMap<(String, String, String, String), TriggerInfo> =
            BTreeMap::new();

        for row in rows {
            let schema: String = row.get("schema");
            let name: String = row.get("name");
            let table_schema: String = row.get("table_schema");
            let table_name: String = row.get("table_name");
            let timing: String = row.get("timing");
            let event: String = row.get("event");
            let function_name: Option<String> = row.try_get("function_name").ok();
            let description: Option<String> = row.try_get("description").ok();

            let key = (
                schema.clone(),
                name.clone(),
                table_schema.clone(),
                timing.clone(),
            );

            let trigger = trigger_map.entry(key).or_insert_with(|| TriggerInfo {
                schema: schema.clone(),
                name: name.clone(),
                table_schema: table_schema.clone(),
                table_name: table_name.clone(),
                timing: timing.clone(),
                events: Vec::new(),
                function_name: function_name.clone(),
                enabled: true, // Default, could query pg_trigger.tgenabled
                description: description.clone(),
            });

            if !trigger.events.contains(&event) {
                trigger.events.push(event);
            }
        }

        Ok(trigger_map.into_values().collect())
    }

    /// Get detailed information about a specific trigger
    pub async fn get_trigger_info(
        &self,
        schema: &str,
        trigger_name: &str,
    ) -> Result<Vec<TriggerInfo>> {
        info!(
            schema = schema,
            trigger_name = trigger_name,
            "Getting trigger info"
        );

        let conn = self.get_connection().await?;
        let rows = conn
            .query(
                r#"
            SELECT 
                t.trigger_schema AS schema,
                t.trigger_name AS name,
                t.event_object_schema AS table_schema,
                t.event_object_table AS table_name,
                t.action_timing AS timing,
                t.event_manipulation AS event,
                t.action_statement AS function_name,
                t.action_condition AS condition,
                pg_catalog.obj_description(tg.oid, 'pg_trigger') AS description
            FROM information_schema.triggers t
            JOIN pg_catalog.pg_trigger tg ON tg.tgname = t.trigger_name
            WHERE t.trigger_schema = $1 AND t.trigger_name = $2
            ORDER BY t.event_object_schema, t.event_object_table, t.action_timing
            "#,
                &[&schema, &trigger_name],
            )
            .await
            .map_err(SqlError::Connection)?;

        if rows.is_empty() {
            return Err(SqlError::NotFound(format!(
                "Trigger '{}.{}' not found",
                schema, trigger_name
            )));
        }

        // Group by table and timing
        use std::collections::BTreeMap;
        let mut trigger_map: BTreeMap<(String, String, String), TriggerInfo> = BTreeMap::new();

        for row in rows {
            let schema: String = row.get("schema");
            let name: String = row.get("name");
            let table_schema: String = row.get("table_schema");
            let table_name: String = row.get("table_name");
            let timing: String = row.get("timing");
            let event: String = row.get("event");
            let function_name: Option<String> = row.try_get("function_name").ok();
            let description: Option<String> = row.try_get("description").ok();

            let key = (table_schema.clone(), table_name.clone(), timing.clone());

            let trigger = trigger_map.entry(key).or_insert_with(|| TriggerInfo {
                schema: schema.clone(),
                name: name.clone(),
                table_schema: table_schema.clone(),
                table_name: table_name.clone(),
                timing: timing.clone(),
                events: Vec::new(),
                function_name: function_name.clone(),
                enabled: true,
                description: description.clone(),
            });

            if !trigger.events.contains(&event) {
                trigger.events.push(event);
            }
        }

        Ok(trigger_map.into_values().collect())
    }

    /// Create a trigger using DDL SQL
    pub async fn create_trigger(&self, ddl: &str) -> Result<()> {
        info!("Creating trigger with DDL");

        let conn = self.get_connection().await?;
        conn.execute(ddl, &[])
            .await
            .map_err(|e| SqlError::Execution(format!("Failed to create trigger: {}", e)))?;

        Ok(())
    }

    /// Alter trigger properties
    /// Supports: RENAME TO, ENABLE/DISABLE
    pub async fn alter_trigger(
        &self,
        schema: &str,
        table_name: &str,
        trigger_name: &str,
        alter_ddl: &str,
    ) -> Result<()> {
        info!(
            schema = schema,
            table_name = table_name,
            trigger_name = trigger_name,
            "Altering trigger"
        );

        let full_ddl = if alter_ddl.contains("TRIGGER") {
            alter_ddl.to_string()
        } else {
            format!(
                "ALTER TABLE {}.{} ALTER TRIGGER {} {}",
                quote_ident(schema),
                quote_ident(table_name),
                quote_ident(trigger_name),
                alter_ddl
            )
        };

        let conn = self.get_connection().await?;
        conn.execute(&full_ddl, &[])
            .await
            .map_err(|e| SqlError::Execution(format!("Failed to alter trigger: {}", e)))?;

        Ok(())
    }

    /// Drop a trigger
    pub async fn drop_trigger(
        &self,
        schema: &str,
        table_name: &str,
        trigger_name: &str,
        if_exists: bool,
    ) -> Result<()> {
        info!(
            schema = schema,
            table_name = table_name,
            trigger_name = trigger_name,
            if_exists = if_exists,
            "Dropping trigger"
        );

        let sql = if if_exists {
            format!(
                "DROP TRIGGER IF EXISTS {} ON {}.{}",
                quote_ident(trigger_name),
                quote_ident(schema),
                quote_ident(table_name)
            )
        } else {
            format!(
                "DROP TRIGGER {} ON {}.{}",
                quote_ident(trigger_name),
                quote_ident(schema),
                quote_ident(table_name)
            )
        };

        let conn = self.get_connection().await?;
        conn.execute(&sql, &[]).await.map_err(|e| {
            let err_str = e.to_string();
            if err_str.contains("does not exist") {
                SqlError::NotFound(format!(
                    "Trigger '{}.{}' on table '{}.{}' does not exist",
                    schema, trigger_name, schema, table_name
                ))
            } else {
                SqlError::Execution(format!("Failed to drop trigger: {}", e))
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
        assert_eq!(quote_ident("test_trigger"), "\"test_trigger\"");
        assert_eq!(quote_ident("test\"trigger"), "\"test\"\"trigger\"");
        assert_eq!(quote_ident("my-trigger"), "\"my-trigger\"");
    }
}
