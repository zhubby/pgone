use pgone_mcp_server::core::models::{
    Column, DatabaseSchema, ForeignKey, Index, PrimaryKey, Schema, TableDetail, ViewDetail,
};
use pgone_sql::Session;
use std::collections::{BTreeMap, HashMap};
use poll_promise::Promise;

#[derive(Default)]
pub struct DbTree {
    schema_cache: Option<DatabaseSchema>,
    expanded_schemas: HashMap<String, bool>,
    expanded_tables: HashMap<String, bool>,
    current_db_id: Option<String>,
    loading: bool,
    error: Option<String>,
    load_promise: Option<Promise<Result<DatabaseSchema, String>>>,
}

impl DbTree {
    pub fn ui(&mut self, ui: &mut egui::Ui, db_manager: &mut crate::components::DbManager) {
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
        if let Some(ref promise) = self.load_promise {
            if let Some(result) = promise.ready() {
                match result {
                    Ok(schema) => {
                        self.schema_cache = Some(schema.clone());
                        self.loading = false;
                        self.error = None;
                    }
                    Err(e) => {
                        tracing::error!("Error loading schema: {}", e);
                        self.error = Some(e.clone());
                        self.loading = false;
                    }
                }
                self.load_promise = None;
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
        
        // Get DSN from storage in the main thread
        db_manager.ensure_storage();
        let dsn = if let Some(ref storage) = db_manager.storage {
            if let Ok(Some(cfg)) = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    storage.get_db_config(&db_id).await
                })
            }) {
                cfg.dsn
            } else {
                self.loading = false;
                self.error = Some("Failed to get database config".to_string());
                return;
            }
        } else {
            self.loading = false;
            self.error = Some("Storage not available".to_string());
            return;
        };
        
        // Spawn async task to load schema using pgone-sql
        let dsn_clone = dsn.clone();
        self.load_promise = Some(Promise::spawn_thread("load_schema", move || {
            tokio::runtime::Handle::current().block_on(async move {
                // Use pgone-sql to connect and query schema
                let session = Session::new(&dsn_clone)
                    .await
                    .map_err(|e| format!("Failed to create session: {}", e))?;
                
                // Get database name
                let database = session.current_database()
                    .await
                    .map_err(|e| format!("Failed to get database name: {}", e))?;
                
                // List all tables
                let tables = session.list_tables(None)
                    .await
                    .map_err(|e| format!("Failed to list tables: {}", e))?;
                
                // List all views
                let views = session.list_views(None)
                    .await
                    .map_err(|e| format!("Failed to list views: {}", e))?;
                
                // Group tables and views by schema
                let mut schemas_map: BTreeMap<String, (Vec<TableDetail>, Vec<ViewDetail>)> = BTreeMap::new();
                
                // Process tables - get connection for each table to avoid holding it too long
                for table_info in tables {
                    let schema_name = table_info.schema.clone();
                    let table_name = table_info.name.clone();
                    
                    // Get connection for detailed queries
                    let conn = session.get_connection()
                        .await
                        .map_err(|e| format!("Failed to get connection: {}", e))?;
                    
                    // Get table details (columns, indexes, foreign keys)
                    let table_detail = Self::get_table_detail(&*conn, &schema_name, &table_name)
                        .await
                        .map_err(|e| format!("Failed to get table details for {}.{}: {}", schema_name, table_name, e))?;
                    
                    schemas_map.entry(schema_name).or_insert_with(|| (Vec::new(), Vec::new())).0.push(table_detail);
                }
                
                // Process views
                for view_info in views {
                    let view_detail = ViewDetail {
                        schema: view_info.schema,
                        name: view_info.name,
                        definition: view_info.definition,
                        comment: view_info.description,
                    };
                    
                    schemas_map.entry(view_detail.schema.clone())
                        .or_insert_with(|| (Vec::new(), Vec::new()))
                        .1.push(view_detail);
                }
                
                // Convert to Schema vec
                let schemas: Vec<Schema> = schemas_map
                    .into_iter()
                    .map(|(name, (tables, views))| Schema {
                        name,
                        tables,
                        views,
                    })
                    .collect();
                
                Ok(DatabaseSchema {
                    database,
                    schemas,
                })
            })
        }));
    }
    
    async fn get_table_detail(
        conn: &tokio_postgres::Client,
        schema: &str,
        table: &str,
    ) -> Result<TableDetail, String> {
        // Get columns
        let col_rows = conn.query(
            "SELECT c.column_name, c.is_nullable, c.data_type, c.udt_name, \
                    c.character_maximum_length, c.numeric_precision, c.numeric_scale, \
                    c.column_default, pgd.description AS column_comment \
             FROM information_schema.columns c \
             LEFT JOIN pg_class pc ON pc.relname = c.table_name \
             LEFT JOIN pg_namespace pn ON pn.nspname = c.table_schema AND pn.oid = pc.relnamespace \
             LEFT JOIN pg_attribute pa ON pa.attrelid = pc.oid AND pa.attname = c.column_name \
             LEFT JOIN pg_description pgd ON pgd.objoid = pc.oid AND pgd.objsubid = pa.attnum \
             WHERE c.table_schema = $1 AND c.table_name = $2 \
             ORDER BY c.ordinal_position",
            &[&schema, &table],
        )
        .await
        .map_err(|e| format!("Failed to query columns: {}", e))?;
        
        let columns: Vec<Column> = col_rows
            .into_iter()
            .map(|row| {
                let is_nullable: String = row.get("is_nullable");
                Column {
                    name: row.get("column_name"),
                    nullable: matches!(is_nullable.as_str(), "YES"),
                    data_type: row.get("data_type"),
                    udt_name: row.try_get("udt_name").ok(),
                    character_maximum_length: row.try_get("character_maximum_length").ok(),
                    numeric_precision: row.try_get("numeric_precision").ok(),
                    numeric_scale: row.try_get("numeric_scale").ok(),
                    default: row.try_get("column_default").ok(),
                    comment: row.try_get("column_comment").ok(),
                }
            })
            .collect();
        
        // Get table comment
        let table_comment: Option<String> = conn.query_opt(
            "SELECT obj_description(pc.oid) \
             FROM pg_class pc \
             JOIN pg_namespace pn ON pn.oid = pc.relnamespace \
             WHERE pn.nspname = $1 AND pc.relname = $2",
            &[&schema, &table],
        )
        .await
        .map_err(|e| format!("Failed to query table comment: {}", e))?
        .map(|row| row.get(0));
        
        // Get primary key
        let pk_rows = conn.query(
            "SELECT kcu.column_name \
             FROM information_schema.table_constraints tc \
             JOIN information_schema.key_column_usage kcu \
               ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema \
             WHERE tc.constraint_type = 'PRIMARY KEY' AND tc.table_schema = $1 AND tc.table_name = $2 \
             ORDER BY kcu.ordinal_position",
            &[&schema, &table],
        )
        .await
        .map_err(|e| format!("Failed to query primary key: {}", e))?;
        
        let pk_cols: Vec<String> = pk_rows.into_iter().map(|row| row.get(0)).collect();
        let primary_key = if pk_cols.is_empty() {
            None
        } else {
            Some(PrimaryKey { columns: pk_cols })
        };
        
        // Get foreign keys
        let fk_rows = conn.query(
            "SELECT kcu.constraint_name, kcu.column_name AS local_column, ccu.table_schema AS ref_schema, ccu.table_name AS ref_table, ccu.column_name AS ref_column, rc.update_rule, rc.delete_rule \
             FROM information_schema.table_constraints tc \
             JOIN information_schema.key_column_usage kcu \
               ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema \
             JOIN information_schema.referential_constraints rc \
               ON rc.constraint_name = tc.constraint_name AND rc.constraint_schema = tc.table_schema \
             JOIN information_schema.constraint_column_usage ccu \
               ON ccu.constraint_name = rc.unique_constraint_name AND ccu.constraint_schema = rc.unique_constraint_schema \
             WHERE tc.constraint_type = 'FOREIGN KEY' AND tc.table_schema = $1 AND tc.table_name = $2 \
             ORDER BY kcu.ordinal_position",
            &[&schema, &table],
        )
        .await
        .map_err(|e| format!("Failed to query foreign keys: {}", e))?;
        
        // Group by constraint_name
        let mut fk_map: BTreeMap<String, (Vec<String>, (String, Vec<String>), Option<String>, Option<String>)> = BTreeMap::new();
        for row in fk_rows {
            let cname: String = row.get("constraint_name");
            let col: String = row.get("local_column");
            let ref_schema: String = row.get("ref_schema");
            let ref_table: String = row.get("ref_table");
            let ref_col: String = row.get("ref_column");
            let on_update: Option<String> = row.try_get("update_rule").ok();
            let on_delete: Option<String> = row.try_get("delete_rule").ok();
            
            let entry = fk_map.entry(cname).or_insert_with(|| {
                (Vec::new(), (format!("{}.{}", ref_schema, ref_table), Vec::new()), None, None)
            });
            entry.0.push(col);
            entry.1.1.push(ref_col);
            if on_update.is_some() {
                entry.2 = on_update;
            }
            if on_delete.is_some() {
                entry.3 = on_delete;
            }
        }
        
        let foreign_keys: Vec<ForeignKey> = fk_map
            .into_values()
            .map(|(cols, (ref_table, ref_cols), on_update, on_delete)| ForeignKey {
                columns: cols,
                ref_table,
                ref_columns: ref_cols,
                on_update,
                on_delete,
            })
            .collect();
        
        // Get indexes
        let idx_rows = conn.query(
            "SELECT indexname, indexdef FROM pg_indexes WHERE schemaname = $1 AND tablename = $2",
            &[&schema, &table],
        )
        .await
        .map_err(|e| format!("Failed to query indexes: {}", e))?;
        
        let mut indexes: Vec<Index> = Vec::new();
        for row in idx_rows {
            let name: String = row.get("indexname");
            let def: String = row.get("indexdef");
            let upper = def.to_uppercase();
            let unique = upper.contains(" UNIQUE ");
            
            // Extract columns from parentheses
            let cols: Vec<String> = def
                .split('(')
                .nth(1)
                .and_then(|s| s.split(')').next())
                .map(|s| {
                    s.split(',')
                        .map(|c| c.trim().trim_matches('"').to_string())
                        .collect()
                })
                .unwrap_or_default();
            
            // Extract INCLUDE columns
            let include: Vec<String> = if let Some(pos) = upper.find(" INCLUDE (") {
                let rest = &def[pos..];
                rest.split('(')
                    .nth(1)
                    .and_then(|s| s.split(')').next())
                    .map(|s| {
                        s.split(',')
                            .map(|c| c.trim().trim_matches('"').to_string())
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            
            indexes.push(Index {
                name,
                unique,
                columns: cols,
                include,
                definition: Some(def),
            });
        }
        
        Ok(TableDetail {
            schema: schema.to_string(),
            name: table.to_string(),
            comment: table_comment,
            columns,
            primary_key,
            foreign_keys,
            indexes,
        })
    }
}

