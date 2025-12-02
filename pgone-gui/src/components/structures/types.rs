use pgone_sql::{DatabaseInfo, SchemaInfo, TableInfo, IndexInfo, ForeignKeyDetail, TriggerInfo};
use std::collections::{HashMap, HashSet};
use poll_promise::Promise;

#[derive(Clone, Debug)]
pub(super) enum DialogType {
    CreateDatabase,
    CreateSchema { database: String },
    CreateTable { database: String, schema: String },
    DeleteDatabase { name: String },
    DeleteSchema { database: String, name: String },
    DeleteTable { database: String, schema: String, name: String },
    RenameDatabase { old_name: String },
    RenameSchema { database: String, old_name: String },
    RenameTable { database: String, schema: String, old_name: String },
    PropertiesDatabase { name: String },
    PropertiesSchema { database: String, name: String },
    PropertiesTable { database: String, schema: String, name: String },
}

#[derive(Default)]
pub struct DbTree {
    // Current database config ID
    pub(super) current_db_id: Option<String>,
    
    // Database level
    pub(super) databases: Vec<DatabaseInfo>,
    pub(super) loaded_databases: bool,
    pub(super) databases_promise: Option<Promise<Result<Vec<DatabaseInfo>, String>>>,
    pub(super) expanded_databases: HashSet<String>,
    
    // Schema level (key: database name)
    pub(super) schemas: HashMap<String, Vec<SchemaInfo>>,
    pub(super) loaded_schemas: HashMap<String, bool>,
    pub(super) schemas_promises: HashMap<String, Promise<Result<Vec<SchemaInfo>, String>>>,
    pub(super) expanded_schemas: HashMap<String, HashSet<String>>, // key: database name
    
    // Table level (key: "database.schema")
    pub(super) tables: HashMap<String, Vec<TableInfo>>,
    pub(super) loaded_tables: HashMap<String, bool>,
    pub(super) tables_promises: HashMap<String, Promise<Result<Vec<TableInfo>, String>>>,
    pub(super) expanded_tables: HashMap<String, HashSet<String>>, // key: "database.schema"
    
    // Index level (key: "database.schema.table")
    pub(super) indexes: HashMap<String, Vec<IndexInfo>>,
    pub(super) loaded_indexes: HashMap<String, bool>,
    pub(super) indexes_promises: HashMap<String, Promise<Result<Vec<IndexInfo>, String>>>,
    pub(super) expanded_indexes: HashMap<String, HashSet<String>>, // key: "database.schema.table"
    
    // Foreign key level (key: "database.schema.table")
    pub(super) foreign_keys: HashMap<String, Vec<ForeignKeyDetail>>,
    pub(super) loaded_foreign_keys: HashMap<String, bool>,
    pub(super) foreign_keys_promises: HashMap<String, Promise<Result<Vec<ForeignKeyDetail>, String>>>,
    pub(super) expanded_foreign_keys: HashMap<String, HashSet<String>>, // key: "database.schema.table"
    
    // Trigger level (key: "database.schema.table")
    pub(super) triggers: HashMap<String, Vec<TriggerInfo>>,
    pub(super) loaded_triggers: HashMap<String, bool>,
    pub(super) triggers_promises: HashMap<String, Promise<Result<Vec<TriggerInfo>, String>>>,
    pub(super) expanded_triggers: HashMap<String, HashSet<String>>, // key: "database.schema.table"
    
    // Selected items
    pub(super) selected_database: Option<String>,
    pub(super) selected_schema: Option<(String, String)>, // (database, schema)
    pub(super) selected_table: Option<(String, String, String)>, // (database, schema, table)
    
    // Dialog state
    pub(super) dialog: Option<DialogType>,
    pub(super) dialog_input: String,
    pub(super) dialog_ddl: String, // For create table DDL
    pub(super) dialog_cascade: bool, // For delete operations
    
    // Pending actions (to avoid borrow checker issues in context menus)
    pub(super) pending_query_table: Option<(String, String, String)>, // (database, schema, table)
    pub(super) pending_query_index: Option<(String, String, String, String)>, // (database, schema, table, index)
    pub(super) pending_query_foreign_key: Option<(String, String, String, String)>, // (database, schema, table, fk_name)
    pub(super) pending_query_trigger: Option<(String, String, String, String)>, // (database, schema, table, trigger)
    pub(super) pending_open_sql_editor: bool, // Flag to open SQL editor
    pub(super) pending_open_graph: Option<(String, String)>, // (database, schema) - Flag to open graph window
    
    // Error state
    pub(super) error: Option<String>,
}

impl DbTree {
    pub fn take_pending_open_sql_editor(&mut self) -> bool {
        let value = self.pending_open_sql_editor;
        self.pending_open_sql_editor = false;
        value
    }
    
    pub fn take_pending_open_graph(&mut self) -> Option<(String, String)> {
        self.pending_open_graph.take()
    }
}

