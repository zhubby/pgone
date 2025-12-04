use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseInfo {
    pub name: String,
    pub owner: String,
    pub encoding: String,
    pub collate: Option<String>,
    pub ctype: Option<String>,
    pub size: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub name: String,
    pub superuser: bool,
    pub createdb: bool,
    pub createrole: bool,
    pub can_login: bool,
    pub replication: bool,
    pub valid_until: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub schema: String,
    pub name: String,
    pub owner: String,
    pub tablespace: Option<String>,
    pub row_count: Option<i64>,
    pub size: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewInfo {
    pub schema: String,
    pub name: String,
    pub owner: String,
    pub definition: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInfo {
    pub schema: String,
    pub name: String,
    pub owner: String,
    pub language: Option<String>,
    pub return_type: Option<String>,
    pub definition: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerInfo {
    pub schema: String,
    pub name: String,
    pub table_schema: String,
    pub table_name: String,
    pub timing: String,
    pub events: Vec<String>,
    pub function_name: Option<String>,
    pub enabled: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaInfo {
    pub name: String,
    pub owner: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableDetail {
    pub schema: String,
    pub name: String,
    pub comment: Option<String>,
    pub columns: Vec<ColumnDetail>,
    pub primary_key: Option<PrimaryKeyDetail>,
    pub foreign_keys: Vec<ForeignKeyDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDetail {
    pub name: String,
    pub data_type: String,
    pub udt_name: Option<String>,
    pub nullable: bool,
    pub default: Option<String>,
    pub character_maximum_length: Option<i32>,
    pub numeric_precision: Option<i32>,
    pub numeric_scale: Option<i32>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimaryKeyDetail {
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKeyDetail {
    pub columns: Vec<String>,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
    pub on_update: Option<String>,
    pub on_delete: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub unique: bool,
    pub columns: Vec<String>,
    pub definition: Option<String>,
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_info_serialization() {
        let db_info = DatabaseInfo {
            name: "testdb".to_string(),
            owner: "postgres".to_string(),
            encoding: "UTF8".to_string(),
            collate: Some("en_US.utf8".to_string()),
            ctype: Some("en_US.utf8".to_string()),
            size: Some("10 MB".to_string()),
            description: Some("Test database".to_string()),
        };

        let json = serde_json::to_string(&db_info).unwrap();
        assert!(json.contains("testdb"));
        assert!(json.contains("postgres"));
        assert!(json.contains("UTF8"));
    }

    #[test]
    fn test_user_info_serialization() {
        let user_info = UserInfo {
            name: "testuser".to_string(),
            superuser: false,
            createdb: true,
            createrole: false,
            can_login: true,
            replication: false,
            valid_until: Some("2025-12-31 23:59:59".to_string()),
            description: Some("Test user".to_string()),
        };

        let json = serde_json::to_string(&user_info).unwrap();
        assert!(json.contains("testuser"));
        assert!(json.contains("true"));
        assert!(json.contains("false"));
    }

    #[test]
    fn test_table_info_serialization() {
        let table_info = TableInfo {
            schema: "public".to_string(),
            name: "users".to_string(),
            owner: "postgres".to_string(),
            tablespace: Some("pg_default".to_string()),
            row_count: Some(1000),
            size: Some("1 MB".to_string()),
            description: Some("User table".to_string()),
        };

        let json = serde_json::to_string(&table_info).unwrap();
        assert!(json.contains("public"));
        assert!(json.contains("users"));
        assert!(json.contains("1000"));
    }

    #[test]
    fn test_view_info_serialization() {
        let view_info = ViewInfo {
            schema: "public".to_string(),
            name: "user_view".to_string(),
            owner: "postgres".to_string(),
            definition: Some("SELECT * FROM users".to_string()),
            description: Some("User view".to_string()),
        };

        let json = serde_json::to_string(&view_info).unwrap();
        assert!(json.contains("user_view"));
        assert!(json.contains("SELECT"));
    }

    #[test]
    fn test_function_info_serialization() {
        let func_info = FunctionInfo {
            schema: "public".to_string(),
            name: "test_func".to_string(),
            owner: "postgres".to_string(),
            language: Some("plpgsql".to_string()),
            return_type: Some("integer".to_string()),
            definition: Some("CREATE FUNCTION...".to_string()),
            description: Some("Test function".to_string()),
        };

        let json = serde_json::to_string(&func_info).unwrap();
        assert!(json.contains("test_func"));
        assert!(json.contains("plpgsql"));
    }

    #[test]
    fn test_trigger_info_serialization() {
        let trigger_info = TriggerInfo {
            schema: "public".to_string(),
            name: "test_trigger".to_string(),
            table_schema: "public".to_string(),
            table_name: "users".to_string(),
            timing: "BEFORE".to_string(),
            events: vec!["INSERT".to_string(), "UPDATE".to_string()],
            function_name: Some("test_func".to_string()),
            enabled: true,
            description: Some("Test trigger".to_string()),
        };

        let json = serde_json::to_string(&trigger_info).unwrap();
        assert!(json.contains("test_trigger"));
        assert!(json.contains("BEFORE"));
        assert!(json.contains("INSERT"));
    }

    #[test]
    fn test_models_deserialization() {
        let json = r#"{"name":"testdb","owner":"postgres","encoding":"UTF8","collate":"en_US.utf8","ctype":"en_US.utf8","size":"10 MB","description":"Test database"}"#;
        let db_info: DatabaseInfo = serde_json::from_str(json).unwrap();
        assert_eq!(db_info.name, "testdb");
        assert_eq!(db_info.owner, "postgres");
    }
}

