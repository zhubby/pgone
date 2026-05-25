use crate::error::{Result, SqlError};
use crate::models::UserInfo;
use crate::session::Session;
use tracing::info;

#[derive(Debug, Clone, Default)]
pub struct CreateUserOptions<'a> {
    pub password: Option<&'a str>,
    pub superuser: bool,
    pub createdb: bool,
    pub createrole: bool,
    pub can_login: bool,
    pub replication: bool,
    pub valid_until: Option<&'a str>,
}

#[derive(Debug, Clone, Default)]
pub struct AlterUserOptions<'a> {
    pub new_name: Option<&'a str>,
    pub password: Option<&'a str>,
    pub superuser: Option<bool>,
    pub createdb: Option<bool>,
    pub createrole: Option<bool>,
    pub can_login: Option<bool>,
    pub replication: Option<bool>,
    pub valid_until: Option<Option<&'a str>>,
}

impl Session {
    /// List all users/roles in the PostgreSQL instance
    pub async fn list_users(&self) -> Result<Vec<UserInfo>> {
        info!("Listing all users/roles");

        let conn = self.get_connection().await?;
        let rows = conn.query(
            r#"
            SELECT 
                r.rolname AS name,
                r.rolsuper AS superuser,
                r.rolcreatedb AS createdb,
                r.rolcreaterole AS createrole,
                r.rolcanlogin AS can_login,
                r.rolreplication AS replication,
                CASE WHEN r.rolvaliduntil IS NULL THEN NULL ELSE r.rolvaliduntil::text END AS valid_until,
                pg_catalog.shobj_description(r.oid, 'pg_authid') AS description
            FROM pg_catalog.pg_roles r
            ORDER BY r.rolname
            "#,
            &[],
        )
        .await
        .map_err(SqlError::Connection)?;

        let mut users = Vec::new();
        for row in rows {
            users.push(UserInfo {
                name: row.get("name"),
                superuser: row.get("superuser"),
                createdb: row.get("createdb"),
                createrole: row.get("createrole"),
                can_login: row.get("can_login"),
                replication: row.get("replication"),
                valid_until: row.try_get("valid_until").ok(),
                description: row.try_get("description").ok(),
            });
        }

        Ok(users)
    }

    /// Get detailed information about a specific user/role
    pub async fn get_user_info(&self, user_name: &str) -> Result<UserInfo> {
        info!(user_name = user_name, "Getting user info");

        let conn = self.get_connection().await?;
        let row = conn.query_opt(
            r#"
            SELECT 
                r.rolname AS name,
                r.rolsuper AS superuser,
                r.rolcreatedb AS createdb,
                r.rolcreaterole AS createrole,
                r.rolcanlogin AS can_login,
                r.rolreplication AS replication,
                CASE WHEN r.rolvaliduntil IS NULL THEN NULL ELSE r.rolvaliduntil::text END AS valid_until,
                pg_catalog.shobj_description(r.oid, 'pg_authid') AS description
            FROM pg_catalog.pg_roles r
            WHERE r.rolname = $1
            "#,
            &[&user_name],
        )
        .await
        .map_err(SqlError::Connection)?
        .ok_or_else(|| SqlError::NotFound(format!("User '{}' not found", user_name)))?;

        Ok(UserInfo {
            name: row.get("name"),
            superuser: row.get("superuser"),
            createdb: row.get("createdb"),
            createrole: row.get("createrole"),
            can_login: row.get("can_login"),
            replication: row.get("replication"),
            valid_until: row.try_get("valid_until").ok(),
            description: row.try_get("description").ok(),
        })
    }

    /// Create a new user/role
    /// Requires CREATEROLE privilege
    pub async fn create_user(&self, user_name: &str, options: CreateUserOptions<'_>) -> Result<()> {
        info!(
            user_name = user_name,
            superuser = options.superuser,
            createdb = options.createdb,
            createrole = options.createrole,
            can_login = options.can_login,
            replication = options.replication,
            "Creating user"
        );

        let mut sql = format!("CREATE ROLE {}", quote_ident(user_name));

        if let Some(password) = options.password {
            sql.push_str(&format!(" PASSWORD '{}'", password.replace('\'', "''")));
        }

        if options.superuser {
            sql.push_str(" SUPERUSER");
        } else {
            sql.push_str(" NOSUPERUSER");
        }

        if options.createdb {
            sql.push_str(" CREATEDB");
        } else {
            sql.push_str(" NOCREATEDB");
        }

        if options.createrole {
            sql.push_str(" CREATEROLE");
        } else {
            sql.push_str(" NOCREATEROLE");
        }

        if options.can_login {
            sql.push_str(" LOGIN");
        } else {
            sql.push_str(" NOLOGIN");
        }

        if options.replication {
            sql.push_str(" REPLICATION");
        } else {
            sql.push_str(" NOREPLICATION");
        }

        if let Some(valid_until) = options.valid_until {
            sql.push_str(&format!(
                " VALID UNTIL '{}'",
                valid_until.replace('\'', "''")
            ));
        }

        let conn = self.get_connection().await?;
        conn.execute(&sql, &[]).await.map_err(|e| {
            let err_str = e.to_string();
            if err_str.contains("permission denied") || err_str.contains("must have CREATEROLE") {
                SqlError::PermissionDenied(format!(
                    "Creating user requires CREATEROLE privilege: {}",
                    e
                ))
            } else {
                SqlError::Execution(format!("Failed to create user: {}", e))
            }
        })?;

        Ok(())
    }

    /// Alter user/role properties
    pub async fn alter_user(&self, user_name: &str, options: AlterUserOptions<'_>) -> Result<()> {
        info!(
            user_name = user_name,
            new_name = options.new_name,
            "Altering user"
        );

        let conn = self.get_connection().await?;

        // Rename user if needed
        if let Some(new_name) = options.new_name {
            let sql = format!(
                "ALTER ROLE {} RENAME TO {}",
                quote_ident(user_name),
                quote_ident(new_name)
            );
            conn.execute(&sql, &[])
                .await
                .map_err(|e| SqlError::Execution(format!("Failed to rename user: {}", e)))?;
        }

        // Build ALTER ROLE statement for other properties
        let mut alter_parts = Vec::new();

        if let Some(password) = options.password {
            alter_parts.push(format!("PASSWORD '{}'", password.replace('\'', "''")));
        }

        if let Some(superuser) = options.superuser {
            alter_parts.push(if superuser {
                "SUPERUSER".to_string()
            } else {
                "NOSUPERUSER".to_string()
            });
        }

        if let Some(createdb) = options.createdb {
            alter_parts.push(if createdb {
                "CREATEDB".to_string()
            } else {
                "NOCREATEDB".to_string()
            });
        }

        if let Some(createrole) = options.createrole {
            alter_parts.push(if createrole {
                "CREATEROLE".to_string()
            } else {
                "NOCREATEROLE".to_string()
            });
        }

        if let Some(can_login) = options.can_login {
            alter_parts.push(if can_login {
                "LOGIN".to_string()
            } else {
                "NOLOGIN".to_string()
            });
        }

        if let Some(replication) = options.replication {
            alter_parts.push(if replication {
                "REPLICATION".to_string()
            } else {
                "NOREPLICATION".to_string()
            });
        }

        if let Some(valid_until) = options.valid_until {
            match valid_until {
                Some(until) => {
                    alter_parts.push(format!("VALID UNTIL '{}'", until.replace('\'', "''")))
                }
                None => alter_parts.push("VALID UNTIL 'infinity'".to_string()),
            }
        }

        if !alter_parts.is_empty() {
            let sql = format!(
                "ALTER ROLE {} {}",
                quote_ident(user_name),
                alter_parts.join(" ")
            );
            conn.execute(&sql, &[]).await.map_err(|e| {
                let err_str = e.to_string();
                if err_str.contains("permission denied") {
                    SqlError::PermissionDenied(format!(
                        "Altering user requires appropriate privileges: {}",
                        e
                    ))
                } else {
                    SqlError::Execution(format!("Failed to alter user: {}", e))
                }
            })?;
        }

        Ok(())
    }

    /// Drop a user/role
    pub async fn drop_user(&self, user_name: &str, if_exists: bool) -> Result<()> {
        info!(
            user_name = user_name,
            if_exists = if_exists,
            "Dropping user"
        );

        let sql = if if_exists {
            format!("DROP ROLE IF EXISTS {}", quote_ident(user_name))
        } else {
            format!("DROP ROLE {}", quote_ident(user_name))
        };

        let conn = self.get_connection().await?;
        conn.execute(&sql, &[]).await.map_err(|e| {
            let err_str = e.to_string();
            if err_str.contains("permission denied") {
                SqlError::PermissionDenied(format!(
                    "Dropping user requires appropriate privileges: {}",
                    e
                ))
            } else if err_str.contains("does not exist") {
                SqlError::NotFound(format!("User '{}' does not exist", user_name))
            } else {
                SqlError::Execution(format!("Failed to drop user: {}", e))
            }
        })?;

        Ok(())
    }

    /// Grant privileges to a user/role
    pub async fn grant_privileges(
        &self,
        user_name: &str,
        privileges: &[&str],
        object_type: &str,
        object_name: Option<&str>,
        schema: Option<&str>,
    ) -> Result<()> {
        info!(
            user_name = user_name,
            privileges = ?privileges,
            object_type = object_type,
            object_name = object_name,
            schema = schema,
            "Granting privileges"
        );

        let privs = privileges.join(", ");
        let mut sql = format!("GRANT {} ON {}", privs, object_type.to_uppercase());

        if let Some(schema) = schema {
            if let Some(obj_name) = object_name {
                sql.push_str(&format!(
                    " {}.{}",
                    quote_ident(schema),
                    quote_ident(obj_name)
                ));
            } else {
                sql.push_str(&format!(" SCHEMA {}", quote_ident(schema)));
            }
        } else if let Some(obj_name) = object_name {
            sql.push_str(&format!(" {}", quote_ident(obj_name)));
        } else {
            sql.push_str(" ALL");
        }

        sql.push_str(&format!(" TO {}", quote_ident(user_name)));

        let conn = self.get_connection().await?;
        conn.execute(&sql, &[])
            .await
            .map_err(|e| SqlError::Execution(format!("Failed to grant privileges: {}", e)))?;

        Ok(())
    }

    /// Revoke privileges from a user/role
    pub async fn revoke_privileges(
        &self,
        user_name: &str,
        privileges: &[&str],
        object_type: &str,
        object_name: Option<&str>,
        schema: Option<&str>,
    ) -> Result<()> {
        info!(
            user_name = user_name,
            privileges = ?privileges,
            object_type = object_type,
            object_name = object_name,
            schema = schema,
            "Revoking privileges"
        );

        let privs = privileges.join(", ");
        let mut sql = format!("REVOKE {} ON {}", privs, object_type.to_uppercase());

        if let Some(schema) = schema {
            if let Some(obj_name) = object_name {
                sql.push_str(&format!(
                    " {}.{}",
                    quote_ident(schema),
                    quote_ident(obj_name)
                ));
            } else {
                sql.push_str(&format!(" SCHEMA {}", quote_ident(schema)));
            }
        } else if let Some(obj_name) = object_name {
            sql.push_str(&format!(" {}", quote_ident(obj_name)));
        } else {
            sql.push_str(" ALL");
        }

        sql.push_str(&format!(" FROM {}", quote_ident(user_name)));

        let conn = self.get_connection().await?;
        conn.execute(&sql, &[])
            .await
            .map_err(|e| SqlError::Execution(format!("Failed to revoke privileges: {}", e)))?;

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
        assert_eq!(quote_ident("user-name"), "\"user-name\"");
        assert_eq!(quote_ident("123table"), "\"123table\"");
        assert_eq!(quote_ident("table name"), "\"table name\"");
    }
}
