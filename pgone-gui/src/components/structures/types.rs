use pgone_sql::{
    DatabaseInfo, ForeignKeyDetail, FunctionInfo, IndexInfo, MaterializedViewInfo, SchemaInfo,
    TableDetail, TableInfo, TriggerInfo, ViewInfo,
};
use poll_promise::Promise;
use std::collections::{HashMap, HashSet};
use std::mem;

#[derive(Clone, Debug)]
pub(super) enum DialogType {
    CreateDatabase,
    CreateSchema {
        database: String,
    },
    CreateTable {
        database: String,
        schema: String,
    },
    CreateView {
        database: String,
        schema: String,
    },
    CreateMaterializedView {
        database: String,
        schema: String,
    },
    CreateFunction {
        database: String,
        schema: String,
    },
    DeleteDatabase {
        name: String,
    },
    DeleteSchema {
        database: String,
        name: String,
    },
    DeleteTable {
        database: String,
        schema: String,
        name: String,
    },
    RenameDatabase {
        old_name: String,
    },
    RenameSchema {
        database: String,
        old_name: String,
    },
    RenameTable {
        database: String,
        schema: String,
        old_name: String,
    },
    PropertiesDatabase {
        name: String,
    },
    PropertiesSchema {
        database: String,
        name: String,
    },
    PropertiesTable {
        database: String,
        schema: String,
        name: String,
    },
    PropertiesView {
        database: String,
        schema: String,
        name: String,
    },
    PropertiesMaterializedView {
        database: String,
        schema: String,
        name: String,
    },
    PropertiesFunction {
        database: String,
        schema: String,
        name: String,
    },
    DesignTable {
        database: String,
        schema: String,
        name: String,
    },
    ShowDdl {
        database: String,
        schema: String,
        name: String,
    },
    DropTable {
        database: String,
        schema: String,
        name: String,
    },
}

/// 可编辑的列数据结构
#[derive(Clone, Debug)]
pub(super) struct EditableColumn {
    pub name: String,
    pub data_type: String,
    pub character_maximum_length: Option<i32>,
    pub numeric_precision: Option<i32>,
    pub numeric_scale: Option<i32>,
    pub nullable: bool,
    pub default: Option<String>,
    pub comment: Option<String>,
    pub is_new: bool,                  // 标记是否为新增列
    pub is_deleted: bool,              // 标记是否为删除列
    pub original_name: Option<String>, // 原始列名（用于重命名检测）
}

#[derive(Default)]
pub(super) struct ConnectionTreeState {
    pub(super) databases: Vec<DatabaseInfo>,
    pub(super) loaded_databases: bool,
    pub(super) databases_promise: Option<Promise<Result<Vec<DatabaseInfo>, String>>>,
    pub(super) expanded_databases: HashSet<String>,
    pub(super) schemas: HashMap<String, Vec<SchemaInfo>>,
    pub(super) loaded_schemas: HashMap<String, bool>,
    pub(super) schemas_promises: HashMap<String, Promise<Result<Vec<SchemaInfo>, String>>>,
    pub(super) expanded_schemas: HashMap<String, HashSet<String>>,
    pub(super) tables: HashMap<String, Vec<TableInfo>>,
    pub(super) loaded_tables: HashMap<String, bool>,
    pub(super) tables_promises: HashMap<String, Promise<Result<Vec<TableInfo>, String>>>,
    pub(super) expanded_tables: HashMap<String, HashSet<String>>,
    pub(super) views: HashMap<String, Vec<ViewInfo>>,
    pub(super) loaded_views: HashMap<String, bool>,
    pub(super) views_promises: HashMap<String, Promise<Result<Vec<ViewInfo>, String>>>,
    pub(super) expanded_views: HashMap<String, HashSet<String>>,
    pub(super) materialized_views: HashMap<String, Vec<MaterializedViewInfo>>,
    pub(super) loaded_materialized_views: HashMap<String, bool>,
    pub(super) materialized_views_promises:
        HashMap<String, Promise<Result<Vec<MaterializedViewInfo>, String>>>,
    pub(super) expanded_materialized_views: HashMap<String, HashSet<String>>,
    pub(super) functions: HashMap<String, Vec<FunctionInfo>>,
    pub(super) loaded_functions: HashMap<String, bool>,
    pub(super) functions_promises: HashMap<String, Promise<Result<Vec<FunctionInfo>, String>>>,
    pub(super) expanded_functions: HashMap<String, HashSet<String>>,
    pub(super) indexes: HashMap<String, Vec<IndexInfo>>,
    pub(super) loaded_indexes: HashMap<String, bool>,
    pub(super) indexes_promises: HashMap<String, Promise<Result<Vec<IndexInfo>, String>>>,
    pub(super) expanded_indexes: HashMap<String, HashSet<String>>,
    pub(super) foreign_keys: HashMap<String, Vec<ForeignKeyDetail>>,
    pub(super) loaded_foreign_keys: HashMap<String, bool>,
    pub(super) foreign_keys_promises:
        HashMap<String, Promise<Result<Vec<ForeignKeyDetail>, String>>>,
    pub(super) expanded_foreign_keys: HashMap<String, HashSet<String>>,
    pub(super) triggers: HashMap<String, Vec<TriggerInfo>>,
    pub(super) loaded_triggers: HashMap<String, bool>,
    pub(super) triggers_promises: HashMap<String, Promise<Result<Vec<TriggerInfo>, String>>>,
    pub(super) expanded_triggers: HashMap<String, HashSet<String>>,
    pub(super) selected_database: Option<String>,
    pub(super) selected_schema: Option<(String, String)>,
    pub(super) selected_table: Option<(String, String, String)>,
    pub(super) dialog: Option<DialogType>,
    pub(super) dialog_input: String,
    pub(super) dialog_ddl: String,
    pub(super) dialog_ddl_content: String,
    pub(super) dialog_cascade: bool,
    pub(super) design_table_detail: Option<TableDetail>,
    pub(super) design_table_columns: Vec<EditableColumn>,
    pub(super) design_table_promise: Option<Promise<Result<TableDetail, String>>>,
    pub(super) design_table_loaded: Option<(String, String, String)>,
    pub(super) ddl_promise: Option<Promise<Result<String, String>>>,
    pub(super) results_promise: Option<Promise<Result<(Vec<String>, Vec<Vec<String>>), String>>>,
    pub(super) pending_query_table: Option<(String, String, String)>,
    pub(super) pending_query_view: Option<(String, String, String)>,
    pub(super) pending_query_materialized_view: Option<(String, String, String)>,
    pub(super) pending_query_function: Option<(String, String, String)>,
    pub(super) pending_query_index: Option<(String, String, String, String)>,
    pub(super) pending_query_foreign_key: Option<(String, String, String, String)>,
    pub(super) pending_query_trigger: Option<(String, String, String, String)>,
    pub(super) pending_open_sql_editor: bool,
    pub(super) pending_open_graph: Option<(String, String)>,
    pub(super) pending_load_ddl: Option<(String, String, String)>,
    pub(super) error: Option<String>,
}

#[derive(Default)]
pub struct DbTree {
    pub(super) connection_states: HashMap<String, ConnectionTreeState>,
    pub(super) expanded_connections: HashSet<String>,

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

    // View level (key: "database.schema")
    pub(super) views: HashMap<String, Vec<ViewInfo>>,
    pub(super) loaded_views: HashMap<String, bool>,
    pub(super) views_promises: HashMap<String, Promise<Result<Vec<ViewInfo>, String>>>,
    pub(super) expanded_views: HashMap<String, HashSet<String>>, // key: "database.schema"

    // Materialized view level (key: "database.schema")
    pub(super) materialized_views: HashMap<String, Vec<MaterializedViewInfo>>,
    pub(super) loaded_materialized_views: HashMap<String, bool>,
    pub(super) materialized_views_promises:
        HashMap<String, Promise<Result<Vec<MaterializedViewInfo>, String>>>,
    pub(super) expanded_materialized_views: HashMap<String, HashSet<String>>, // key: "database.schema"

    // Function level (key: "database.schema")
    pub(super) functions: HashMap<String, Vec<FunctionInfo>>,
    pub(super) loaded_functions: HashMap<String, bool>,
    pub(super) functions_promises: HashMap<String, Promise<Result<Vec<FunctionInfo>, String>>>,
    pub(super) expanded_functions: HashMap<String, HashSet<String>>, // key: "database.schema"

    // Index level (key: "database.schema.table")
    pub(super) indexes: HashMap<String, Vec<IndexInfo>>,
    pub(super) loaded_indexes: HashMap<String, bool>,
    pub(super) indexes_promises: HashMap<String, Promise<Result<Vec<IndexInfo>, String>>>,
    pub(super) expanded_indexes: HashMap<String, HashSet<String>>, // key: "database.schema.table"

    // Foreign key level (key: "database.schema.table")
    pub(super) foreign_keys: HashMap<String, Vec<ForeignKeyDetail>>,
    pub(super) loaded_foreign_keys: HashMap<String, bool>,
    pub(super) foreign_keys_promises:
        HashMap<String, Promise<Result<Vec<ForeignKeyDetail>, String>>>,
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
    pub(super) dialog_ddl: String,         // For create table DDL
    pub(super) dialog_ddl_content: String, // For show DDL content
    pub(super) dialog_cascade: bool,       // For delete operations

    // Table design state
    pub(super) design_table_detail: Option<TableDetail>, // 原始表结构
    pub(super) design_table_columns: Vec<EditableColumn>, // 可编辑的列数据
    pub(super) design_table_promise: Option<Promise<Result<TableDetail, String>>>, // 异步加载表结构的 Promise
    pub(super) design_table_loaded: Option<(String, String, String)>, // 当前已加载的表 (database, schema, name)

    // DDL state
    pub(super) ddl_promise: Option<Promise<Result<String, String>>>, // 异步加载DDL的 Promise
    pub(super) results_promise: Option<Promise<Result<(Vec<String>, Vec<Vec<String>>), String>>>,

    // Pending actions (to avoid borrow checker issues in context menus)
    pub(super) pending_query_table: Option<(String, String, String)>, // (database, schema, table)
    pub(super) pending_query_view: Option<(String, String, String)>,  // (database, schema, view)
    pub(super) pending_query_materialized_view: Option<(String, String, String)>, // (database, schema, materialized_view)
    pub(super) pending_query_function: Option<(String, String, String)>, // (database, schema, function)
    pub(super) pending_query_index: Option<(String, String, String, String)>, // (database, schema, table, index)
    pub(super) pending_query_foreign_key: Option<(String, String, String, String)>, // (database, schema, table, fk_name)
    pub(super) pending_query_trigger: Option<(String, String, String, String)>, // (database, schema, table, trigger)
    pub(super) pending_open_sql_editor: bool, // Flag to open SQL editor
    pub(super) pending_open_graph: Option<(String, String)>, // (database, schema) - Flag to open graph window
    pub(super) pending_load_ddl: Option<(String, String, String)>, // (database, schema, table) - Flag to load DDL

    // Error state
    pub(super) error: Option<String>,
}

impl DbTree {
    pub(super) fn load_connection_state(
        &mut self,
        connection_id: String,
        state: ConnectionTreeState,
    ) {
        self.current_db_id = Some(connection_id);
        self.databases = state.databases;
        self.loaded_databases = state.loaded_databases;
        self.databases_promise = state.databases_promise;
        self.expanded_databases = state.expanded_databases;
        self.schemas = state.schemas;
        self.loaded_schemas = state.loaded_schemas;
        self.schemas_promises = state.schemas_promises;
        self.expanded_schemas = state.expanded_schemas;
        self.tables = state.tables;
        self.loaded_tables = state.loaded_tables;
        self.tables_promises = state.tables_promises;
        self.expanded_tables = state.expanded_tables;
        self.views = state.views;
        self.loaded_views = state.loaded_views;
        self.views_promises = state.views_promises;
        self.expanded_views = state.expanded_views;
        self.materialized_views = state.materialized_views;
        self.loaded_materialized_views = state.loaded_materialized_views;
        self.materialized_views_promises = state.materialized_views_promises;
        self.expanded_materialized_views = state.expanded_materialized_views;
        self.functions = state.functions;
        self.loaded_functions = state.loaded_functions;
        self.functions_promises = state.functions_promises;
        self.expanded_functions = state.expanded_functions;
        self.indexes = state.indexes;
        self.loaded_indexes = state.loaded_indexes;
        self.indexes_promises = state.indexes_promises;
        self.expanded_indexes = state.expanded_indexes;
        self.foreign_keys = state.foreign_keys;
        self.loaded_foreign_keys = state.loaded_foreign_keys;
        self.foreign_keys_promises = state.foreign_keys_promises;
        self.expanded_foreign_keys = state.expanded_foreign_keys;
        self.triggers = state.triggers;
        self.loaded_triggers = state.loaded_triggers;
        self.triggers_promises = state.triggers_promises;
        self.expanded_triggers = state.expanded_triggers;
        self.selected_database = state.selected_database;
        self.selected_schema = state.selected_schema;
        self.selected_table = state.selected_table;
        self.dialog = state.dialog;
        self.dialog_input = state.dialog_input;
        self.dialog_ddl = state.dialog_ddl;
        self.dialog_ddl_content = state.dialog_ddl_content;
        self.dialog_cascade = state.dialog_cascade;
        self.design_table_detail = state.design_table_detail;
        self.design_table_columns = state.design_table_columns;
        self.design_table_promise = state.design_table_promise;
        self.design_table_loaded = state.design_table_loaded;
        self.ddl_promise = state.ddl_promise;
        self.results_promise = state.results_promise;
        self.pending_query_table = state.pending_query_table;
        self.pending_query_view = state.pending_query_view;
        self.pending_query_materialized_view = state.pending_query_materialized_view;
        self.pending_query_function = state.pending_query_function;
        self.pending_query_index = state.pending_query_index;
        self.pending_query_foreign_key = state.pending_query_foreign_key;
        self.pending_query_trigger = state.pending_query_trigger;
        self.pending_open_sql_editor = state.pending_open_sql_editor;
        self.pending_open_graph = state.pending_open_graph;
        self.pending_load_ddl = state.pending_load_ddl;
        self.error = state.error;
    }

    pub(super) fn take_connection_state(&mut self) -> ConnectionTreeState {
        ConnectionTreeState {
            databases: mem::take(&mut self.databases),
            loaded_databases: mem::take(&mut self.loaded_databases),
            databases_promise: mem::take(&mut self.databases_promise),
            expanded_databases: mem::take(&mut self.expanded_databases),
            schemas: mem::take(&mut self.schemas),
            loaded_schemas: mem::take(&mut self.loaded_schemas),
            schemas_promises: mem::take(&mut self.schemas_promises),
            expanded_schemas: mem::take(&mut self.expanded_schemas),
            tables: mem::take(&mut self.tables),
            loaded_tables: mem::take(&mut self.loaded_tables),
            tables_promises: mem::take(&mut self.tables_promises),
            expanded_tables: mem::take(&mut self.expanded_tables),
            views: mem::take(&mut self.views),
            loaded_views: mem::take(&mut self.loaded_views),
            views_promises: mem::take(&mut self.views_promises),
            expanded_views: mem::take(&mut self.expanded_views),
            materialized_views: mem::take(&mut self.materialized_views),
            loaded_materialized_views: mem::take(&mut self.loaded_materialized_views),
            materialized_views_promises: mem::take(&mut self.materialized_views_promises),
            expanded_materialized_views: mem::take(&mut self.expanded_materialized_views),
            functions: mem::take(&mut self.functions),
            loaded_functions: mem::take(&mut self.loaded_functions),
            functions_promises: mem::take(&mut self.functions_promises),
            expanded_functions: mem::take(&mut self.expanded_functions),
            indexes: mem::take(&mut self.indexes),
            loaded_indexes: mem::take(&mut self.loaded_indexes),
            indexes_promises: mem::take(&mut self.indexes_promises),
            expanded_indexes: mem::take(&mut self.expanded_indexes),
            foreign_keys: mem::take(&mut self.foreign_keys),
            loaded_foreign_keys: mem::take(&mut self.loaded_foreign_keys),
            foreign_keys_promises: mem::take(&mut self.foreign_keys_promises),
            expanded_foreign_keys: mem::take(&mut self.expanded_foreign_keys),
            triggers: mem::take(&mut self.triggers),
            loaded_triggers: mem::take(&mut self.loaded_triggers),
            triggers_promises: mem::take(&mut self.triggers_promises),
            expanded_triggers: mem::take(&mut self.expanded_triggers),
            selected_database: mem::take(&mut self.selected_database),
            selected_schema: mem::take(&mut self.selected_schema),
            selected_table: mem::take(&mut self.selected_table),
            dialog: mem::take(&mut self.dialog),
            dialog_input: mem::take(&mut self.dialog_input),
            dialog_ddl: mem::take(&mut self.dialog_ddl),
            dialog_ddl_content: mem::take(&mut self.dialog_ddl_content),
            dialog_cascade: mem::take(&mut self.dialog_cascade),
            design_table_detail: mem::take(&mut self.design_table_detail),
            design_table_columns: mem::take(&mut self.design_table_columns),
            design_table_promise: mem::take(&mut self.design_table_promise),
            design_table_loaded: mem::take(&mut self.design_table_loaded),
            ddl_promise: mem::take(&mut self.ddl_promise),
            results_promise: mem::take(&mut self.results_promise),
            pending_query_table: mem::take(&mut self.pending_query_table),
            pending_query_view: mem::take(&mut self.pending_query_view),
            pending_query_materialized_view: mem::take(&mut self.pending_query_materialized_view),
            pending_query_function: mem::take(&mut self.pending_query_function),
            pending_query_index: mem::take(&mut self.pending_query_index),
            pending_query_foreign_key: mem::take(&mut self.pending_query_foreign_key),
            pending_query_trigger: mem::take(&mut self.pending_query_trigger),
            pending_open_sql_editor: mem::take(&mut self.pending_open_sql_editor),
            pending_open_graph: mem::take(&mut self.pending_open_graph),
            pending_load_ddl: mem::take(&mut self.pending_load_ddl),
            error: mem::take(&mut self.error),
        }
    }

    pub fn take_pending_open_sql_editor(&mut self) -> bool {
        if let Some((_, state)) = self
            .connection_states
            .iter_mut()
            .find(|(_, state)| state.pending_open_sql_editor)
        {
            state.pending_open_sql_editor = false;
            return true;
        }

        let value = self.pending_open_sql_editor;
        self.pending_open_sql_editor = false;
        value
    }

    pub fn take_pending_open_graph(&mut self) -> Option<(String, String)> {
        for state in self.connection_states.values_mut() {
            if let Some(pending) = state.pending_open_graph.take() {
                return Some(pending);
            }
        }

        self.pending_open_graph.take()
    }

    pub fn selected_schema_name(&self) -> Option<String> {
        self.selected_schema
            .as_ref()
            .map(|(_, schema)| schema.clone())
            .or_else(|| {
                self.selected_table
                    .as_ref()
                    .map(|(_, schema, _)| schema.clone())
            })
    }

    pub fn selected_table_name(&self) -> Option<String> {
        self.selected_table
            .as_ref()
            .map(|(_, _, table)| table.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database(name: &str) -> DatabaseInfo {
        DatabaseInfo {
            name: name.to_string(),
            owner: "owner".to_string(),
            encoding: "UTF8".to_string(),
            collate: None,
            ctype: None,
            size: None,
            description: None,
        }
    }

    #[test]
    fn connection_states_restore_independent_database_cache() {
        let mut tree = DbTree::default();

        tree.load_connection_state("first".to_string(), ConnectionTreeState::default());
        tree.databases.push(database("first_db"));
        tree.loaded_databases = true;
        tree.expanded_databases.insert("first_db".to_string());
        let first_state = tree.take_connection_state();
        tree.connection_states
            .insert("first".to_string(), first_state);

        tree.load_connection_state("second".to_string(), ConnectionTreeState::default());
        tree.databases.push(database("second_db"));
        tree.loaded_databases = true;
        tree.expanded_databases.insert("second_db".to_string());
        let second_state = tree.take_connection_state();
        tree.connection_states
            .insert("second".to_string(), second_state);

        let first_state = tree.connection_states.remove("first").unwrap();
        tree.load_connection_state("first".to_string(), first_state);

        assert_eq!(tree.current_db_id.as_deref(), Some("first"));
        assert_eq!(tree.databases[0].name, "first_db");
        assert!(tree.expanded_databases.contains("first_db"));
        assert!(!tree.expanded_databases.contains("second_db"));
    }
}
