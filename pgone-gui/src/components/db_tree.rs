use pgone_mcp_server::core::models::DatabaseSchema;
use std::collections::HashMap;
use std::sync::mpsc;

#[derive(Default)]
pub struct DbTree {
    schema_cache: Option<DatabaseSchema>,
    expanded_schemas: HashMap<String, bool>,
    expanded_tables: HashMap<String, bool>,
    current_db_id: Option<String>,
    loading: bool,
    error: Option<String>,
    load_receiver: Option<mpsc::Receiver<Result<DatabaseSchema, String>>>,
}

impl DbTree {
    pub fn ui(&mut self, ui: &mut egui::Ui, db_manager: &mut crate::components::DbManager) {
        // Show database information if one is selected
        let db_id_opt = db_manager.active_db_config_id.clone();
        if let Some(db_id) = db_id_opt {
            db_manager.ensure_storage();
            if let Some(ref storage) = db_manager.storage {
                let rt = &db_manager.rt;
                if let Ok(Some(cfg)) = rt.block_on(async {
                    storage.get_db_config(&db_id).await
                }) {
                    // Parse DSN to get connection details
                    if let Some(parsed) = crate::components::DbManager::parse_dsn(&cfg.dsn) {
                        ui.group(|ui| {
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
        
        ui.heading("Database Structure");
        ui.separator();

        // Check if database config changed
        let current_db = db_manager.active_db_config_id.clone();
        if current_db != self.current_db_id {
            self.current_db_id = current_db.clone();
            self.schema_cache = None;
            self.error = None;
            if current_db.is_some() {
                self.load_schema(db_manager);
            }
        }

        if self.loading {
            ui.spinner();
            ui.label("Loading schema...");
            return;
        }

        if let Some(err) = &self.error {
            ui.colored_label(egui::Color32::RED, format!("Error: {}", err));
            if ui.button("Retry").clicked() {
                self.load_schema(db_manager);
            }
            return;
        }

        // Check for async load result
        if let Some(ref receiver) = self.load_receiver {
            if let Ok(result) = receiver.try_recv() {
                match result {
                    Ok(schema) => {
                        self.schema_cache = Some(schema);
                        self.loading = false;
                        self.error = None;
                    }
                    Err(e) => {
                        self.error = Some(e);
                        self.loading = false;
                    }
                }
                self.load_receiver = None;
            }
        }

        let schema = self.schema_cache.as_ref();

        if let Some(schema) = schema {
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    // Database level - get expanded state first
                    let db_key = "__database__".to_string();
                    let db_expanded = *self.expanded_schemas.get(&db_key).unwrap_or(&true);
                    let db_expanded = egui::CollapsingHeader::new(
                        format!("{} {}", egui_phosphor::regular::DATABASE, schema.database)
                    )
                    .default_open(db_expanded)
                    .show(ui, |ui| {
                        self.expanded_schemas.insert(db_key.clone(), true);
                        // Schema level
                        for schema_item in &schema.schemas {
                            let schema_key = format!("schema_{}", schema_item.name);
                            let schema_expanded = *self.expanded_schemas.get(&schema_key).unwrap_or(&true);
                            
                            egui::CollapsingHeader::new(
                                format!("{} {}", egui_phosphor::regular::FOLDER, schema_item.name)
                            )
                            .default_open(schema_expanded)
                            .show(ui, |ui| {
                                self.expanded_schemas.insert(schema_key.clone(), true);
                                
                                // Tables
                                if !schema_item.tables.is_empty() {
                                    let tables_key = format!("{}_tables", schema_key);
                                    let tables_expanded = *self.expanded_tables.get(&tables_key).unwrap_or(&true);
                                    
                                    egui::CollapsingHeader::new(
                                        format!("{} Tables", egui_phosphor::regular::TABLE)
                                    )
                                    .default_open(tables_expanded)
                                    .show(ui, |ui| {
                                        self.expanded_tables.insert(tables_key.clone(), true);
                                        for table in &schema_item.tables {
                                            let table_key = format!("{}_{}", schema_key, table.name);
                                            let table_expanded = *self.expanded_tables.get(&table_key).unwrap_or(&false);
                                            
                                            egui::CollapsingHeader::new(
                                                format!("{} {}", egui_phosphor::regular::TABLE, table.name)
                                            )
                                            .default_open(table_expanded)
                                            .show(ui, |ui| {
                                                self.expanded_tables.insert(table_key.clone(), true);
                                                
                                                // Columns
                                                if !table.columns.is_empty() {
                                                    let cols_key = format!("{}_columns", table_key);
                                                    let cols_expanded = *self.expanded_tables.get(&cols_key).unwrap_or(&true);
                                                    
                                                    egui::CollapsingHeader::new(
                                                        format!("{} Columns", egui_phosphor::regular::LIST_BULLETS)
                                                    )
                                                    .default_open(cols_expanded)
                                                    .show(ui, |ui| {
                                                        self.expanded_tables.insert(cols_key.clone(), true);
                                                        for col in &table.columns {
                                                            let nullable_str = if col.nullable { "NULL" } else { "NOT NULL" };
                                                            ui.label(format!(
                                                                "{} {} ({}) {}",
                                                                egui_phosphor::regular::LIST_BULLETS,
                                                                col.name,
                                                                col.data_type,
                                                                nullable_str
                                                            ));
                                                        }
                                                    });
                                                }
                                                
                                                // Indexes
                                                if !table.indexes.is_empty() {
                                                    let idx_key = format!("{}_indexes", table_key);
                                                    let idx_expanded = *self.expanded_tables.get(&idx_key).unwrap_or(&false);
                                                    
                                                    egui::CollapsingHeader::new(
                                                        format!("{} Indexes", egui_phosphor::regular::LIST)
                                                    )
                                                    .default_open(idx_expanded)
                                                    .show(ui, |ui| {
                                                        self.expanded_tables.insert(idx_key.clone(), true);
                                                        for idx in &table.indexes {
                                                            let unique_str = if idx.unique { "UNIQUE " } else { "" };
                                                            ui.label(format!(
                                                                "{} {}{} ({})",
                                                                egui_phosphor::regular::LIST,
                                                                unique_str,
                                                                idx.name,
                                                                idx.columns.join(", ")
                                                            ));
                                                        }
                                                    });
                                                }
                                                
                                                // Foreign Keys
                                                if !table.foreign_keys.is_empty() {
                                                    let fk_key = format!("{}_fkeys", table_key);
                                                    let fk_expanded = *self.expanded_tables.get(&fk_key).unwrap_or(&false);
                                                    
                                                    egui::CollapsingHeader::new(
                                                        format!("{} Foreign Keys", egui_phosphor::regular::LINK)
                                                    )
                                                    .default_open(fk_expanded)
                                                    .show(ui, |ui| {
                                                        self.expanded_tables.insert(fk_key.clone(), true);
                                                        for fk in &table.foreign_keys {
                                                            ui.label(format!(
                                                                "{} {} -> {}.{}",
                                                                egui_phosphor::regular::LINK,
                                                                fk.columns.join(", "),
                                                                fk.ref_table,
                                                                fk.ref_columns.join(", ")
                                                            ));
                                                        }
                                                    });
                                                }
                                            });
                                        }
                                    });
                                }
                                
                                // Views
                                if !schema_item.views.is_empty() {
                                    let views_key = format!("{}_views", schema_key);
                                    let views_expanded = *self.expanded_tables.get(&views_key).unwrap_or(&false);
                                    
                                    egui::CollapsingHeader::new(
                                        format!("{} Views", egui_phosphor::regular::EYE)
                                    )
                                    .default_open(views_expanded)
                                    .show(ui, |ui| {
                                        self.expanded_tables.insert(views_key.clone(), true);
                                        for view in &schema_item.views {
                                            ui.label(format!(
                                                "{} {}",
                                                egui_phosphor::regular::EYE,
                                                view.name
                                            ));
                                        }
                                    });
                                }
                            });
                        }
                    });
                    // Update db_expanded state based on header response
                    if !db_expanded.header_response.clicked() {
                        // Header was not clicked, so state remains
                    }
                });
        } else {
            ui.label("No database selected");
        }
    }

    fn load_schema(&mut self, db_manager: &mut crate::components::DbManager) {
        self.loading = true;
        self.error = None;
        
        let Some(db_id) = db_manager.active_db_config_id.clone() else {
            self.loading = false;
            return;
        };
        
        let db_id_clone = db_id.clone();
        let (tx, rx) = mpsc::channel();
        self.load_receiver = Some(rx);
        
        // Spawn async task to load schema
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                // Get DSN from storage - we need to recreate storage in the thread
                let storage = pgone_storage::blocking::StorageBlocking::open_local("pgone.db").await
                    .map_err(|e| format!("Failed to open storage: {}", e))?;
                let configs = storage.list_db_configs(None).await
                    .map_err(|e| format!("Failed to list configs: {}", e))?;
                let config = configs.iter()
                    .find(|c| c.id == db_id_clone)
                    .ok_or_else(|| "Config not found".to_string())?;
                
                // Call API server to get schema
                let client = reqwest::Client::new();
                let request = pgone_a2a::SchemaQueryRequest {
                    dsn: config.dsn.clone(),
                    schemas: None,
                    with_indexes: true,
                    with_routines: false,
                    with_types: false,
                    with_triggers: false,
                };
                
                let response = client
                    .post("http://127.0.0.1:8765/schema/query")
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| format!("Request failed: {}", e))?;
                
                let response_data: pgone_a2a::SchemaQueryResponse = response.json().await
                    .map_err(|e| format!("Failed to parse response: {}", e))?;
                
                if response_data.success {
                    response_data.schema
                        .ok_or_else(|| "Schema is None".to_string())
                } else {
                    Err(response_data.error.unwrap_or_else(|| "Unknown error".to_string()))
                }
            });
            
            let _ = tx.send(result);
        });
    }
}

