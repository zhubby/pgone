use pgone_sql::{DatabaseInfo, SchemaInfo, Session, TableInfo};
use std::collections::{HashMap, HashSet};
use poll_promise::Promise;

use crate::components::SqlPanel;
use crate::futures;

#[derive(Clone, Debug)]
enum DialogType {
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
    current_db_id: Option<String>,
    
    // Database level
    databases: Vec<DatabaseInfo>,
    loaded_databases: bool,
    databases_promise: Option<Promise<Result<Vec<DatabaseInfo>, String>>>,
    expanded_databases: HashSet<String>,
    
    // Schema level (key: database name)
    schemas: HashMap<String, Vec<SchemaInfo>>,
    loaded_schemas: HashMap<String, bool>,
    schemas_promises: HashMap<String, Promise<Result<Vec<SchemaInfo>, String>>>,
    expanded_schemas: HashMap<String, HashSet<String>>, // key: database name
    
    // Table level (key: "database.schema")
    tables: HashMap<String, Vec<TableInfo>>,
    loaded_tables: HashMap<String, bool>,
    tables_promises: HashMap<String, Promise<Result<Vec<TableInfo>, String>>>,
    expanded_tables: HashMap<String, HashSet<String>>, // key: "database.schema"
    
    // Selected items
    selected_database: Option<String>,
    selected_schema: Option<(String, String)>, // (database, schema)
    selected_table: Option<(String, String, String)>, // (database, schema, table)
    
    // Dialog state
    dialog: Option<DialogType>,
    dialog_input: String,
    dialog_ddl: String, // For create table DDL
    dialog_cascade: bool, // For delete operations
    
    // Pending actions (to avoid borrow checker issues in context menus)
    pending_query_table: Option<(String, String, String)>, // (database, schema, table)
    pending_open_sql_editor: bool, // Flag to open SQL editor
    
    // Error state
    error: Option<String>,
}

impl DbTree {
    pub fn take_pending_open_sql_editor(&mut self) -> bool {
        let value = self.pending_open_sql_editor;
        self.pending_open_sql_editor = false;
        value
    }
    
    pub fn ui(&mut self, ui: &mut egui::Ui, db_manager: &mut crate::components::DbManager, sql_panel: &mut SqlPanel) {
        // Show database information if one is selected
        let db_id_opt = db_manager.active_db_config_id.clone();
        if let Some(db_id) = db_id_opt {
            db_manager.ensure_storage();
            if let Some(ref storage) = db_manager.storage {
                if let Ok(Some(cfg)) = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async {
                        storage.get_db_config(&db_id).await
                    })
                }) {
                    // Parse DSN to get connection details
                    if let Some(parsed) = crate::components::DbManager::parse_dsn(&cfg.dsn) {
                        egui::Frame::group(ui.style())
                            .inner_margin(egui::Vec2::splat(8.0))
                            .show(ui, |ui| {
                                ui.heading("Database Info");
                                ui.horizontal(|ui| {
                                    ui.label(egui_phosphor::regular::DATABASE);
                                    ui.label(egui::RichText::new(&cfg.id).strong());
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Engine:");
                                    ui.label(&cfg.engine);
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Host:");
                                    ui.label(&parsed.host);
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Database:");
                                    ui.label(if parsed.database.is_empty() {
                                        "<default>"
                                    } else {
                                        &parsed.database
                                    });
                                });
                            });
                        ui.add_space(10.0);
                    }
                }
            }
        }
        
        ui.heading(format!("{} Database Structure", egui_phosphor::regular::TREE_STRUCTURE));
        ui.separator();

        // Check if database config changed
        let current_db = db_manager.active_db_config_id.clone();
        if current_db != self.current_db_id {
            self.current_db_id = current_db.clone();
            self.reset();
            if current_db.is_some() {
                self.load_databases(db_manager);
            }
        }

        if let Some(err) = &self.error {
            ui.colored_label(egui::Color32::RED, format!("Error: {}", err));
            if ui.button("Retry").clicked() {
                self.reset();
                self.load_databases(db_manager);
            }
            return;
        }

        // Check for async load results
        self.check_promises();

        // Handle pending query table action
        if let Some((database, schema, table)) = self.pending_query_table.take() {
            self.query_table_data(db_manager, sql_panel, &database, &schema, &table);
        }

        // Render tree
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                // Database level
                if !self.loaded_databases {
                    // let available_size = ui.available_size();
                    // ui.allocate_space(available_size);
                    ui.centered_and_justified(|ui| {
                        ui.label("Please select a database to view the structure");
                    });
                    return;
                }

                // Pre-load schemas and tables for all databases (before rendering to avoid borrow issues)
                let databases_clone = self.databases.clone();
                for db in &databases_clone {
                    let db_name = db.name.clone();
                    let should_load_schemas = !self.loaded_schemas.get(&db_name).copied().unwrap_or(false);
                    if should_load_schemas {
                        self.load_schemas(db_manager, &db_name);
                    }
                }
                
                // Pre-load tables for expanded schemas
                let databases_clone2 = self.databases.clone();
                let mut tables_to_load = Vec::new();
                for db in &databases_clone2 {
                    let db_name = db.name.clone();
                    let schemas_clone = self.schemas.get(&db_name).cloned();
                    if let Some(ref schemas) = schemas_clone {
                        let expanded_schemas = self.expanded_schemas.get(&db_name);
                        for schema in schemas {
                            let schema_name = schema.name.clone();
                            let should_load = expanded_schemas.map(|s| s.contains(&schema_name)).unwrap_or(false);
                            if should_load {
                                let tables_key = format!("{}.{}", db_name, schema_name);
                                if !self.loaded_tables.get(&tables_key).copied().unwrap_or(false) {
                                    tables_to_load.push((db_name.clone(), schema_name));
                                }
                            }
                        }
                    }
                }
                for (db_name, schema_name) in tables_to_load {
                    self.load_tables(db_manager, &db_name, &schema_name);
                }

                for db in &self.databases {
                    let db_name = db.name.clone();
                    let is_expanded = self.expanded_databases.contains(&db_name);
                    let schemas_clone = self.schemas.get(&db_name).cloned();
                    
                    let response = egui::CollapsingHeader::new(
                        format!("{} {}", egui_phosphor::regular::DATABASE, db_name)
                    )
                    .default_open(is_expanded)
                                    .show(ui, |ui| {
                        if !is_expanded {
                            self.expanded_databases.insert(db_name.clone());
                        }
                        
                        // Show schemas
                        if let Some(schemas) = schemas_clone {
                            let expanded_schemas = self.expanded_schemas.entry(db_name.clone()).or_insert_with(HashSet::new);
                            
                            for schema in &schemas {
                                let schema_name = schema.name.clone();
                                let is_schema_expanded = expanded_schemas.contains(&schema_name);
                                
                                let tables_key = format!("{}.{}", db_name, schema_name);
                                
                                let schema_response = egui::CollapsingHeader::new(
                                    format!("{} {}", egui_phosphor::regular::FOLDER, schema_name)
                                )
                                .default_open(is_schema_expanded)
                                            .show(ui, |ui| {
                                    if !is_schema_expanded {
                                        expanded_schemas.insert(schema_name.clone());
                                    }
                                    
                                    // Show tables
                                    if let Some(tables) = self.tables.get(&tables_key) {
                                        let expanded_tables = self.expanded_tables.entry(tables_key.clone()).or_insert_with(HashSet::new);
                                        
                                        for table in tables {
                                            let table_name = table.name.clone();
                                            let is_table_expanded = expanded_tables.contains(&table_name);
                                            
                                            let table_response = egui::CollapsingHeader::new(
                                                format!("{} {}", egui_phosphor::regular::TABLE, table_name)
                                            )
                                            .default_open(is_table_expanded)
                                                    .show(ui, |ui| {
                                                if !is_table_expanded {
                                                    expanded_tables.insert(table_name.clone());
                                                }
                                                
                                                // Show table info
                                                if let Some(row_count) = table.row_count {
                                                    ui.label(format!("Rows: {}", row_count));
                                                }
                                                if let Some(size) = &table.size {
                                                    ui.label(format!("Size: {}", size));
                                                }
                                            });
                                            
                                            // Handle table selection and context menu
                                            let table_clicked = table_response.header_response.clicked();
                                            let db_name_clone = db_name.clone();
                                            let schema_name_clone = schema_name.clone();
                                            let table_name_clone = table_name.clone();
                                            
                                            if table_clicked {
                                                self.pending_query_table = Some((db_name_clone.clone(), schema_name_clone.clone(), table_name_clone.clone()));
                                            }
                                            
                                            table_response.header_response.context_menu(|ui| {
                                                if ui.button("Query Table").clicked() {
                                                    self.pending_query_table = Some((db_name.clone(), schema_name.clone(), table_name.clone()));
                                                    ui.close();
                                                }
                                                if ui.button("New Query").clicked() {
                                                    self.pending_open_sql_editor = true;
                                                    ui.close();
                                                }
                                                if ui.button("Properties").clicked() {
                                                    self.dialog = Some(DialogType::PropertiesTable {
                                                        database: db_name.clone(),
                                                        schema: schema_name.clone(),
                                                        name: table_name.clone(),
                                                    });
                                                    ui.close();
                                                }
                                                if ui.button("Rename").clicked() {
                                                    self.dialog = Some(DialogType::RenameTable {
                                                        database: db_name.clone(),
                                                        schema: schema_name.clone(),
                                                        old_name: table_name.clone(),
                                                    });
                                                    self.dialog_input = table_name.clone();
                                                    ui.close();
                                                }
                                                if ui.button("Delete").clicked() {
                                                    self.dialog = Some(DialogType::DeleteTable {
                                                        database: db_name.clone(),
                                                        schema: schema_name.clone(),
                                                        name: table_name.clone(),
                                                    });
                                                    ui.close();
                                                }
                                            });
                                        }
                                        
                                        // Add table button
                                        if ui.button(format!("{} New Table", egui_phosphor::regular::PLUS)).clicked() {
                                            self.dialog = Some(DialogType::CreateTable {
                                                database: db_name.clone(),
                                                schema: schema_name.clone(),
                                            });
                                            self.dialog_ddl = format!("CREATE TABLE {}.{} (\n    id SERIAL PRIMARY KEY\n);", schema_name, "new_table");
                                        }
                                    } else {
                                        ui.label("Loading tables...");
                                    }
                                });
                                
                                // Handle schema context menu
                                schema_response.header_response.context_menu(|ui| {
                                    if ui.button("New Schema").clicked() {
                                        self.dialog = Some(DialogType::CreateSchema {
                                            database: db_name.clone(),
                                        });
                                        self.dialog_input.clear();
                                        ui.close();
                                    }
                                    if ui.button("Properties").clicked() {
                                        self.dialog = Some(DialogType::PropertiesSchema {
                                            database: db_name.clone(),
                                            name: schema_name.clone(),
                                        });
                                        ui.close();
                                    }
                                    if ui.button("Rename").clicked() {
                                        self.dialog = Some(DialogType::RenameSchema {
                                            database: db_name.clone(),
                                            old_name: schema_name.clone(),
                                        });
                                        self.dialog_input = schema_name.clone();
                                        ui.close();
                                    }
                                    if ui.button("Delete").clicked() {
                                        self.dialog = Some(DialogType::DeleteSchema {
                                            database: db_name.clone(),
                                            name: schema_name.clone(),
                                        });
                                        ui.close();
                                    }
                                });
                            }
                            
                            // Add schema button
                            if ui.button(format!("{} New Schema", egui_phosphor::regular::PLUS)).clicked() {
                                self.dialog = Some(DialogType::CreateSchema {
                                    database: db_name.clone(),
                                });
                                self.dialog_input.clear();
                            }
                        } else {
                            ui.label("Loading schemas...");
                        }
                    });
                    
                    // Handle database context menu
                    response.header_response.context_menu(|ui| {
                        if ui.button("New Database").clicked() {
                            self.dialog = Some(DialogType::CreateDatabase);
                            self.dialog_input.clear();
                            ui.close();
                        }
                        if ui.button("Properties").clicked() {
                            self.dialog = Some(DialogType::PropertiesDatabase {
                                name: db_name.clone(),
                            });
                            ui.close();
                        }
                        if ui.button("Rename").clicked() {
                            self.dialog = Some(DialogType::RenameDatabase {
                                old_name: db_name.clone(),
                            });
                            self.dialog_input = db_name.clone();
                            ui.close();
                        }
                        if ui.button("Delete").clicked() {
                            self.dialog = Some(DialogType::DeleteDatabase {
                                name: db_name.clone(),
                            });
                            ui.close();
                        }
                    });
                }
                
                // Add database button
                if ui.button(format!("{} New Database", egui_phosphor::regular::PLUS)).clicked() {
                    self.dialog = Some(DialogType::CreateDatabase);
                    self.dialog_input.clear();
                }
            });

        // Show dialogs
        self.show_dialogs(ui, db_manager);
    }

    fn reset(&mut self) {
        self.databases.clear();
        self.loaded_databases = false;
        self.schemas.clear();
        self.loaded_schemas.clear();
        self.tables.clear();
        self.loaded_tables.clear();
        self.expanded_databases.clear();
        self.expanded_schemas.clear();
        self.expanded_tables.clear();
        self.selected_database = None;
        self.selected_schema = None;
        self.selected_table = None;
        self.error = None;
    }

    fn check_promises(&mut self) {
        // Check databases promise
        if let Some(ref promise) = self.databases_promise {
            if let Some(result) = promise.ready() {
                match result {
                    Ok(databases) => {
                        self.databases = databases.clone();
                        self.loaded_databases = true;
                    }
                    Err(e) => {
                        self.error = Some(e.clone());
                        self.loaded_databases = false;
                    }
                }
                self.databases_promise = None;
            }
        }

        // Check schemas promises
        let mut completed_schemas = Vec::new();
        for (db_name, promise) in &self.schemas_promises {
            if let Some(result) = promise.ready() {
                match result {
                    Ok(schemas) => {
                        self.schemas.insert(db_name.clone(), schemas.clone());
                        self.loaded_schemas.insert(db_name.clone(), true);
                    }
                    Err(e) => {
                        self.error = Some(format!("Failed to load schemas for {}: {}", db_name, e));
                    }
                }
                completed_schemas.push(db_name.clone());
            }
        }
        for db_name in completed_schemas {
            self.schemas_promises.remove(&db_name);
        }

        // Check tables promises
        let mut completed_tables = Vec::new();
        for (key, promise) in &self.tables_promises {
            if let Some(result) = promise.ready() {
                match result {
                    Ok(tables) => {
                        self.tables.insert(key.clone(), tables.clone());
                        self.loaded_tables.insert(key.clone(), true);
                    }
                    Err(e) => {
                        self.error = Some(format!("Failed to load tables for {}: {}", key, e));
                    }
                }
                completed_tables.push(key.clone());
            }
        }
        for key in completed_tables {
            self.tables_promises.remove(&key);
        }
    }

    fn load_databases(&mut self, db_manager: &mut crate::components::DbManager) {
        let Some(db_id) = db_manager.active_db_config_id.clone() else {
            return;
        };
        
        db_manager.ensure_storage();
        let dsn = if let Some(ref storage) = db_manager.storage {
            if let Ok(Some(cfg)) = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    storage.get_db_config(&db_id).await
                })
            }) {
                cfg.dsn
            } else {
                self.error = Some("Failed to get database config".to_string());
                return;
            }
        } else {
            self.error = Some("Storage not available".to_string());
            return;
        };
        
        let dsn_clone = dsn.clone();
        let (sender, promise) = Promise::new();
        self.databases_promise = Some(promise);
        
        futures::spawn(async move {
            let result: Result<Vec<DatabaseInfo>, String> = async {
                let session = Session::connect_to_postgres(&dsn_clone)
                    .await
                    .map_err(|e| format!("Failed to connect to postgres: {}", e))?;
                
                session.list_databases()
                    .await
                    .map_err(|e| format!("Failed to list databases: {}", e))
            }.await;
            
            sender.send(result);
        });
    }

    fn load_schemas(&mut self, db_manager: &mut crate::components::DbManager, database: &str) {
        if self.schemas_promises.contains_key(database) {
            return; // Already loading
        }
        
        let Some(db_id) = db_manager.active_db_config_id.clone() else {
            return;
        };
        
        db_manager.ensure_storage();
        let dsn = if let Some(ref storage) = db_manager.storage {
            if let Ok(Some(cfg)) = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    storage.get_db_config(&db_id).await
                })
            }) {
                // Replace database name in DSN while preserving password
                replace_database_in_dsn(&cfg.dsn, database).unwrap_or_else(|| {
                    // Fallback to manual construction if URL parsing fails
                    if let Some(parsed) = crate::components::DbManager::parse_dsn(&cfg.dsn) {
                        format!("{}://{}@{}:{}/{}", 
                            parsed.engine, parsed.user, parsed.host, parsed.port, database)
                    } else {
                        cfg.dsn.clone()
                    }
                })
            } else {
                return;
            }
        } else {
            return;
        };
        
        let dsn_clone = dsn.clone();
        let (sender, promise) = Promise::new();
        self.schemas_promises.insert(database.to_string(), promise);

        futures::spawn(async move {
            let result: Result<Vec<SchemaInfo>, String> = async {
                let session = Session::new(&dsn_clone)
                        .await
                        .map_err(|e| format!("Failed to create session: {}", e))?;
                    
                session.list_schemas()
                        .await
                    .map_err(|e| format!("Failed to list schemas: {}", e))
            }.await;
            
            sender.send(result);
        });
    }

    fn load_tables(&mut self, db_manager: &mut crate::components::DbManager, database: &str, schema: &str) {
        let key = format!("{}.{}", database, schema);
        if self.tables_promises.contains_key(&key) {
            return; // Already loading
        }
        
        let Some(db_id) = db_manager.active_db_config_id.clone() else {
            return;
        };
        
        db_manager.ensure_storage();
        let dsn = if let Some(ref storage) = db_manager.storage {
            if let Ok(Some(cfg)) = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    storage.get_db_config(&db_id).await
                })
            }) {
                // Replace database name in DSN while preserving password
                replace_database_in_dsn(&cfg.dsn, database).unwrap_or_else(|| {
                    // Fallback to manual construction if URL parsing fails
                    if let Some(parsed) = crate::components::DbManager::parse_dsn(&cfg.dsn) {
                        format!("{}://{}@{}:{}/{}", 
                            parsed.engine, parsed.user, parsed.host, parsed.port, database)
                    } else {
                        cfg.dsn.clone()
                    }
                })
            } else {
                return;
            }
        } else {
            return;
        };
        
        let dsn_clone = dsn.clone();
        let schema_clone = schema.to_string();
        let (sender, promise) = Promise::new();
        self.tables_promises.insert(key.clone(), promise);
        
        futures::spawn(async move {
            let result: Result<Vec<TableInfo>, String> = async {
                let session = Session::new(&dsn_clone)
                        .await
                    .map_err(|e| format!("Failed to create session: {}", e))?;
                    
                session.list_tables(Some(&schema_clone))
                        .await
                    .map_err(|e| format!("Failed to list tables: {}", e))
            }.await;
            
            sender.send(result);
        });
    }

    fn query_table_data(&mut self, db_manager: &mut crate::components::DbManager, sql_panel: &mut SqlPanel, database: &str, schema: &str, table: &str) {
        let Some(db_id) = db_manager.active_db_config_id.clone() else {
            return;
        };
        
        db_manager.ensure_storage();
        let dsn = if let Some(ref storage) = db_manager.storage {
            if let Ok(Some(cfg)) = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    storage.get_db_config(&db_id).await
                })
            }) {
                // Replace database name in DSN while preserving password
                replace_database_in_dsn(&cfg.dsn, database).unwrap_or_else(|| {
                    // Fallback to manual construction if URL parsing fails
                    if let Some(parsed) = crate::components::DbManager::parse_dsn(&cfg.dsn) {
                        format!("{}://{}@{}:{}/{}", 
                            parsed.engine, parsed.user, parsed.host, parsed.port, database)
                    } else {
                        cfg.dsn.clone()
                    }
                })
            } else {
                return;
            }
        } else {
            return;
        };
        
        let dsn_clone = dsn.clone();
        let schema_clone = schema.to_string();
        let table_clone = table.to_string();
        
        // Use tokio::task::block_in_place to run async code
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let session = Session::new(&dsn_clone)
                            .await
                    .map_err(|e| format!("Failed to create session: {}", e))?;
                        
                session.query_table_data(&schema_clone, &table_clone, Some(100))
                            .await
                    .map_err(|e| format!("Failed to query table: {}", e))
            })
        });
        
        match result {
            Ok((columns, rows)) => {
                sql_panel.query_columns = columns;
                sql_panel.query_rows = rows;
            }
            Err(e) => {
                self.error = Some(e);
            }
        }
    }

    fn show_dialogs(&mut self, ui: &mut egui::Ui, db_manager: &mut crate::components::DbManager) {
        if let Some(dialog_type) = self.dialog.clone() {
            let title = match &dialog_type {
                DialogType::CreateDatabase => "Create Database",
                DialogType::CreateSchema { .. } => "Create Schema",
                DialogType::CreateTable { .. } => "Create Table",
                DialogType::DeleteDatabase { .. } => "Delete Database",
                DialogType::DeleteSchema { .. } => "Delete Schema",
                DialogType::DeleteTable { .. } => "Delete Table",
                DialogType::RenameDatabase { .. } => "Rename Database",
                DialogType::RenameSchema { .. } => "Rename Schema",
                DialogType::RenameTable { .. } => "Rename Table",
                DialogType::PropertiesDatabase { .. } => "Database Properties",
                DialogType::PropertiesSchema { .. } => "Schema Properties",
                DialogType::PropertiesTable { .. } => "Table Properties",
            };
            
            let mut open = true;
            let mut should_create = false;
            let mut should_delete = false;
            let mut should_rename = false;
            let mut delete_cascade = self.dialog_cascade;
            
            let center = ui.ctx().screen_rect().center();
            egui::Window::new(title)
                .open(&mut open)
                .default_pos(center)
                .pivot(egui::Align2::CENTER_CENTER)
                .show(ui.ctx(), |ui| {
                    let dialog_input_ref = &mut self.dialog_input;
                    let dialog_ddl_ref = &mut self.dialog_ddl;
                    match &dialog_type {
                        DialogType::CreateDatabase => {
                            ui.label("Database Name:");
                            ui.text_edit_singleline(dialog_input_ref);
                            ui.horizontal(|ui| {
                                if ui.button("Create").clicked() {
                                    should_create = true;
                                }
                                if ui.button("Cancel").clicked() {
                                    // Will be handled by open = false
                                }
                            });
                        }
                        DialogType::CreateSchema { database: _ } => {
                            ui.label("Schema Name:");
                            ui.text_edit_singleline(dialog_input_ref);
                            ui.horizontal(|ui| {
                                if ui.button("Create").clicked() {
                                    should_create = true;
                                }
                                if ui.button("Cancel").clicked() {
                                    // Will be handled by open = false
                                }
                            });
                        }
                        DialogType::CreateTable { database: _, schema: _ } => {
                            ui.label("Table DDL:");
                            ui.text_edit_multiline(dialog_ddl_ref);
                            ui.horizontal(|ui| {
                                if ui.button("Create").clicked() {
                                    should_create = true;
                                }
                                if ui.button("Cancel").clicked() {
                                    // Will be handled by open = false
                                }
                            });
                        }
                        DialogType::DeleteDatabase { name } => {
                            ui.label(format!("Are you sure you want to delete database '{}'?", name));
                            ui.checkbox(&mut delete_cascade, "CASCADE");
                            ui.horizontal(|ui| {
                                if ui.button("Delete").clicked() {
                                    should_delete = true;
                                }
                                if ui.button("Cancel").clicked() {
                                    // Will be handled by open = false
                                }
                            });
                        }
                        DialogType::DeleteSchema { database: _, name } => {
                            ui.label(format!("Are you sure you want to delete schema '{}'?", name));
                            ui.checkbox(&mut delete_cascade, "CASCADE");
                            ui.horizontal(|ui| {
                                if ui.button("Delete").clicked() {
                                    should_delete = true;
                                }
                                if ui.button("Cancel").clicked() {
                                    // Will be handled by open = false
                                }
                            });
                        }
                        DialogType::DeleteTable { database: _, schema, name } => {
                            ui.label(format!("Are you sure you want to delete table '{}.{}'?", schema, name));
                            ui.checkbox(&mut delete_cascade, "CASCADE");
                            ui.horizontal(|ui| {
                                if ui.button("Delete").clicked() {
                                    should_delete = true;
                                }
                                if ui.button("Cancel").clicked() {
                                    // Will be handled by open = false
                                }
                            });
                        }
                        DialogType::RenameDatabase { old_name: _ } => {
                            ui.label("New Name:");
                            ui.text_edit_singleline(dialog_input_ref);
                            ui.horizontal(|ui| {
                                if ui.button("Rename").clicked() {
                                    should_rename = true;
                                }
                                if ui.button("Cancel").clicked() {
                                    // Will be handled by open = false
                                }
                            });
                        }
                        DialogType::RenameSchema { database: _, old_name: _ } => {
                            ui.label("New Name:");
                            ui.text_edit_singleline(dialog_input_ref);
                            ui.horizontal(|ui| {
                                if ui.button("Rename").clicked() {
                                    should_rename = true;
                                }
                                if ui.button("Cancel").clicked() {
                                    // Will be handled by open = false
                                }
                            });
                        }
                        DialogType::RenameTable { database: _, schema: _, old_name: _ } => {
                            ui.label("New Name:");
                            ui.text_edit_singleline(dialog_input_ref);
                            ui.horizontal(|ui| {
                                if ui.button("Rename").clicked() {
                                    should_rename = true;
                                }
                                if ui.button("Cancel").clicked() {
                                    // Will be handled by open = false
                                }
                            });
                        }
                        DialogType::PropertiesDatabase { name } => {
                            // Show database properties (read-only)
                            if let Some(db) = self.databases.iter().find(|d| d.name == *name) {
                                ui.label(format!("Name: {}", db.name));
                                ui.label(format!("Owner: {}", db.owner));
                                ui.label(format!("Encoding: {}", db.encoding));
                                if let Some(size) = &db.size {
                                    ui.label(format!("Size: {}", size));
                                }
                                if let Some(desc) = &db.description {
                                    ui.label(format!("Description: {}", desc));
                                }
                            }
                            if ui.button("Close").clicked() {
                                // Will be handled by open = false
                            }
                        }
                        DialogType::PropertiesSchema { database, name } => {
                            // Show schema properties (read-only)
                            if let Some(schemas) = self.schemas.get(database) {
                                if let Some(schema) = schemas.iter().find(|s| s.name == *name) {
                                    ui.label(format!("Name: {}", schema.name));
                                    ui.label(format!("Owner: {}", schema.owner));
                                    if let Some(desc) = &schema.description {
                                        ui.label(format!("Description: {}", desc));
                                    }
                                }
                            }
                            if ui.button("Close").clicked() {
                                // Will be handled by open = false
                            }
                        }
                        DialogType::PropertiesTable { database, schema, name } => {
                            // Show table properties (read-only)
                            let key = format!("{}.{}", database, schema);
                            if let Some(tables) = self.tables.get(&key) {
                                if let Some(table) = tables.iter().find(|t| t.name == *name) {
                                    ui.label(format!("Name: {}", table.name));
                                    ui.label(format!("Schema: {}", table.schema));
                                    ui.label(format!("Owner: {}", table.owner));
                                    if let Some(row_count) = table.row_count {
                                        ui.label(format!("Row Count: {}", row_count));
                                    }
                                    if let Some(size) = &table.size {
                                        ui.label(format!("Size: {}", size));
                                    }
                                    if let Some(desc) = &table.description {
                                        ui.label(format!("Description: {}", desc));
                                    }
                                }
                            }
                            if ui.button("Close").clicked() {
                                // Will be handled by open = false
                            }
                        }
                    }
                    
                    // Close window if action was triggered
                    if should_create || should_delete || should_rename {
                        // Window will close automatically when open becomes false
                    }
                });
            
            if !open {
                // Clone values before clearing dialog to avoid borrow conflicts
                let dialog_input_clone = self.dialog_input.clone();
                let dialog_ddl_clone = self.dialog_ddl.clone();
                
                // Clear dialog first to avoid borrow conflicts
                self.dialog = None;
                
                // Handle actions after dialog closes
                if should_create {
                    match dialog_type {
                        DialogType::CreateDatabase => {
                            self.create_database(db_manager, &dialog_input_clone);
                        }
                        DialogType::CreateSchema { database } => {
                            self.create_schema(db_manager, &database, &dialog_input_clone);
                        }
                        DialogType::CreateTable { database, schema } => {
                            self.create_table(db_manager, &database, &schema, &dialog_ddl_clone);
                        }
                        _ => {}
                    }
                } else if should_delete {
                    self.dialog_cascade = delete_cascade;
                    match dialog_type {
                        DialogType::DeleteDatabase { name } => {
                            self.delete_database(db_manager, &name, delete_cascade);
                        }
                        DialogType::DeleteSchema { database, name } => {
                            self.delete_schema(db_manager, &database, &name, delete_cascade);
                        }
                        DialogType::DeleteTable { database, schema, name } => {
                            self.delete_table(db_manager, &database, &schema, &name, delete_cascade);
                        }
                        _ => {}
                    }
                } else if should_rename {
                    match dialog_type {
                        DialogType::RenameDatabase { old_name } => {
                            self.rename_database(db_manager, &old_name, &dialog_input_clone);
                        }
                        DialogType::RenameSchema { database, old_name } => {
                            self.rename_schema(db_manager, &database, &old_name, &dialog_input_clone);
                        }
                        DialogType::RenameTable { database, schema, old_name } => {
                            self.rename_table(db_manager, &database, &schema, &old_name, &dialog_input_clone);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn get_dsn_for_database(&self, db_manager: &mut crate::components::DbManager, database: &str) -> Option<String> {
        let db_id = db_manager.active_db_config_id.clone()?;
        db_manager.ensure_storage();
        if let Some(ref storage) = db_manager.storage {
            if let Ok(Some(cfg)) = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    storage.get_db_config(&db_id).await
                })
            }) {
                // Replace database name in DSN while preserving password
                return replace_database_in_dsn(&cfg.dsn, database).or_else(|| {
                    // Fallback to manual construction if URL parsing fails
                    if let Some(parsed) = crate::components::DbManager::parse_dsn(&cfg.dsn) {
                        Some(format!("{}://{}@{}:{}/{}", 
                            parsed.engine, parsed.user, parsed.host, parsed.port, database))
                    } else {
                        Some(cfg.dsn.clone())
                    }
                });
            }
        }
        None
    }

    fn create_database(&mut self, db_manager: &mut crate::components::DbManager, name: &str) {
        let Some(db_id) = db_manager.active_db_config_id.clone() else {
            return;
        };
        
        db_manager.ensure_storage();
        let dsn = if let Some(ref storage) = db_manager.storage {
            if let Ok(Some(cfg)) = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    storage.get_db_config(&db_id).await
                })
            }) {
                cfg.dsn
            } else {
                return;
            }
        } else {
            return;
        };
        
        let dsn_clone = dsn.clone();
        let name_clone = name.to_string();
        
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let session = Session::connect_to_postgres(&dsn_clone)
                    .await
                    .map_err(|e| format!("Failed to connect: {}", e))?;
                
                session.create_database(&name_clone, None, None, None)
                    .await
                    .map_err(|e| format!("Failed to create database: {}", e))
            })
        });
        
        match result {
            Ok(_) => {
                // Reload databases
                self.loaded_databases = false;
                self.load_databases(db_manager);
            }
            Err(e) => {
                self.error = Some(e);
            }
        }
    }

    fn create_schema(&mut self, db_manager: &mut crate::components::DbManager, database: &str, name: &str) {
        let dsn = self.get_dsn_for_database(db_manager, database);
        let Some(dsn) = dsn else { return; };
        
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let session = Session::new(&dsn)
        .await
                    .map_err(|e| format!("Failed to create session: {}", e))?;
                
                session.create_schema(name, None)
                    .await
                    .map_err(|e| format!("Failed to create schema: {}", e))
            })
        });
        
        match result {
            Ok(_) => {
                // Reload schemas
                self.loaded_schemas.insert(database.to_string(), false);
                self.load_schemas(db_manager, database);
            }
            Err(e) => {
                self.error = Some(e);
            }
        }
    }

    fn create_table(&mut self, db_manager: &mut crate::components::DbManager, database: &str, schema: &str, ddl: &str) {
        let dsn = self.get_dsn_for_database(db_manager, database);
        let Some(dsn) = dsn else { return; };
        
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let session = Session::new(&dsn)
        .await
                    .map_err(|e| format!("Failed to create session: {}", e))?;
                
                session.create_table(ddl)
        .await
                    .map_err(|e| format!("Failed to create table: {}", e))
            })
        });
        
        match result {
            Ok(_) => {
                // Reload tables
                let key = format!("{}.{}", database, schema);
                self.loaded_tables.insert(key.clone(), false);
                self.load_tables(db_manager, database, schema);
            }
            Err(e) => {
                self.error = Some(e);
            }
        }
    }

    fn delete_database(&mut self, db_manager: &mut crate::components::DbManager, name: &str, _cascade: bool) {
        let Some(db_id) = db_manager.active_db_config_id.clone() else {
            return;
        };
        
        db_manager.ensure_storage();
        let dsn = if let Some(ref storage) = db_manager.storage {
            if let Ok(Some(cfg)) = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    storage.get_db_config(&db_id).await
                })
            }) {
                cfg.dsn
        } else {
                return;
            }
        } else {
            return;
        };
        
        let dsn_clone = dsn.clone();
        let name_clone = name.to_string();
        
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let session = Session::connect_to_postgres(&dsn_clone)
        .await
                    .map_err(|e| format!("Failed to connect: {}", e))?;
                
                session.drop_database(&name_clone, false)
                    .await
                    .map_err(|e| format!("Failed to delete database: {}", e))
            })
        });
        
        match result {
            Ok(_) => {
                // Reload databases
                self.loaded_databases = false;
                self.load_databases(db_manager);
            }
            Err(e) => {
                self.error = Some(e);
            }
        }
    }

    fn delete_schema(&mut self, db_manager: &mut crate::components::DbManager, database: &str, name: &str, cascade: bool) {
        let dsn = self.get_dsn_for_database(db_manager, database);
        let Some(dsn) = dsn else { return; };
        
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let session = Session::new(&dsn)
        .await
                    .map_err(|e| format!("Failed to create session: {}", e))?;
                
                session.drop_schema(name, false, cascade)
                    .await
                    .map_err(|e| format!("Failed to delete schema: {}", e))
            })
        });
        
        match result {
            Ok(_) => {
                // Reload schemas
                self.loaded_schemas.insert(database.to_string(), false);
                self.load_schemas(db_manager, database);
            }
            Err(e) => {
                self.error = Some(e);
            }
        }
    }

    fn delete_table(&mut self, db_manager: &mut crate::components::DbManager, database: &str, schema: &str, name: &str, cascade: bool) {
        let dsn = self.get_dsn_for_database(db_manager, database);
        let Some(dsn) = dsn else { return; };
        
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let session = Session::new(&dsn)
                    .await
                    .map_err(|e| format!("Failed to create session: {}", e))?;
                
                session.drop_table(schema, name, false, cascade)
                    .await
                    .map_err(|e| format!("Failed to delete table: {}", e))
            })
        });
        
        match result {
            Ok(_) => {
                // Reload tables
                let key = format!("{}.{}", database, schema);
                self.loaded_tables.insert(key.clone(), false);
                self.load_tables(db_manager, database, schema);
            }
            Err(e) => {
                self.error = Some(e);
            }
        }
    }

    fn rename_database(&mut self, db_manager: &mut crate::components::DbManager, old_name: &str, new_name: &str) {
        let Some(db_id) = db_manager.active_db_config_id.clone() else {
            return;
        };
        
        db_manager.ensure_storage();
        let dsn = if let Some(ref storage) = db_manager.storage {
            if let Ok(Some(cfg)) = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    storage.get_db_config(&db_id).await
                })
            }) {
                cfg.dsn
            } else {
                return;
            }
        } else {
            return;
        };
        
        let dsn_clone = dsn.clone();
        let old_name_clone = old_name.to_string();
        let new_name_clone = new_name.to_string();
        
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let session = Session::connect_to_postgres(&dsn_clone)
                    .await
                    .map_err(|e| format!("Failed to connect: {}", e))?;
                
                session.alter_database(&old_name_clone, Some(&new_name_clone), None, None)
                    .await
                    .map_err(|e| format!("Failed to rename database: {}", e))
            })
        });
        
        match result {
            Ok(_) => {
                // Reload databases
                self.loaded_databases = false;
                self.load_databases(db_manager);
            }
            Err(e) => {
                self.error = Some(e);
            }
        }
    }

    fn rename_schema(&mut self, db_manager: &mut crate::components::DbManager, database: &str, old_name: &str, new_name: &str) {
        let dsn = self.get_dsn_for_database(db_manager, database);
        let Some(dsn) = dsn else { return; };
        
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let session = Session::new(&dsn)
                    .await
                    .map_err(|e| format!("Failed to create session: {}", e))?;
                
                session.alter_schema(old_name, Some(new_name), None)
                    .await
                    .map_err(|e| format!("Failed to rename schema: {}", e))
            })
        });
        
        match result {
            Ok(_) => {
                // Reload schemas
                self.loaded_schemas.insert(database.to_string(), false);
                self.load_schemas(db_manager, database);
            }
            Err(e) => {
                self.error = Some(e);
            }
        }
    }

    fn rename_table(&mut self, db_manager: &mut crate::components::DbManager, database: &str, schema: &str, old_name: &str, new_name: &str) {
        let dsn = self.get_dsn_for_database(db_manager, database);
        let Some(dsn) = dsn else { return; };
        
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let session = Session::new(&dsn)
                    .await
                    .map_err(|e| format!("Failed to create session: {}", e))?;
                
                session.alter_table(schema, old_name, &format!("RENAME TO {}", quote_ident(new_name)))
                    .await
                    .map_err(|e| format!("Failed to rename table: {}", e))
            })
        });
        
        match result {
            Ok(_) => {
                // Reload tables
                let key = format!("{}.{}", database, schema);
                self.loaded_tables.insert(key.clone(), false);
                self.load_tables(db_manager, database, schema);
            }
            Err(e) => {
                self.error = Some(e);
            }
        }
    }
}

fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

/// Replace database name in DSN while preserving password and other parameters
fn replace_database_in_dsn(dsn: &str, new_database: &str) -> Option<String> {
    // Try to parse as URL first - this preserves password and all query parameters
    if let Ok(mut url) = url::Url::parse(dsn) {
        // Set the new database path (url::Url handles encoding automatically)
        url.set_path(&format!("/{}", new_database));
        return Some(url.to_string());
    }
    
    // Fallback: try manual parsing for postgresql:// URLs
    // This handles cases where URL parsing fails but DSN format is still valid
    if dsn.starts_with("postgresql://") || dsn.starts_with("postgres://") {
        // Find the last '/' before query parameters
        if let Some(db_start) = dsn.rfind('/') {
            if let Some(query_start) = dsn[db_start..].find('?') {
                // Has query parameters - preserve them
                let base = &dsn[..db_start];
                let query = &dsn[db_start + query_start..];
                return Some(format!("{}/{}{}", base, new_database, query));
            } else {
                // No query parameters
                return Some(format!("{}/{}", &dsn[..db_start], new_database));
            }
        }
    }
    
    None
}
