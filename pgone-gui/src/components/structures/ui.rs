use super::types::DbTree;
use super::loading;
use super::dialogs;
use crate::components::ResultsTable;
use std::collections::HashSet;

impl DbTree {
    pub fn ui(&mut self, ui: &mut egui::Ui, db_manager: &mut crate::components::DbManager, results_table: &mut ResultsTable) {
        ui.horizontal(|ui| {
            ui.heading(format!("{} Structure", egui_phosphor::regular::TREE_STRUCTURE));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(format!("{} Open", egui_phosphor::regular::FOLDER_OPEN)).clicked() {
                    db_manager.show_manage_db = true;
                }
            });
        });
        ui.separator();

        // Check if database config changed
        let current_db = db_manager.active_db_config_id.clone();
        if current_db != self.current_db_id {
            self.current_db_id = current_db.clone();
            self.reset();
            if current_db.is_some() {
                loading::load_databases(self, db_manager);
            }
        }

        if let Some(err) = &self.error {
            ui.colored_label(egui::Color32::RED, format!("Error: {}", err));
            if ui.button("Retry").clicked() {
                self.reset();
                loading::load_databases(self, db_manager);
            }
            return;
        }

        // Check for async load results
        loading::check_promises(self);

        // Handle pending query table action
        if let Some((database, schema, table)) = self.pending_query_table.take() {
            loading::query_table_data(self, db_manager, results_table, &database, &schema, &table);
        }

        // Handle pending query index action
        if let Some((database, schema, table, index)) = self.pending_query_index.take() {
            loading::query_index_detail(self, db_manager, results_table, &database, &schema, &table, &index);
        }

        // Handle pending query foreign key action
        if let Some((database, schema, table, fk_name)) = self.pending_query_foreign_key.take() {
            loading::query_foreign_key_detail(self, db_manager, results_table, &database, &schema, &table, &fk_name);
        }

        // Handle pending query trigger action
        if let Some((database, schema, table, trigger)) = self.pending_query_trigger.take() {
            loading::query_trigger_detail(self, db_manager, results_table, &database, &schema, &table, &trigger);
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
                    loading::load_schemas(self, db_manager, &db_name);
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
                loading::load_tables(self, db_manager, &db_name, &schema_name);
            }

            // Pre-load indexes, foreign keys, and triggers for expanded tables
            let databases_clone3 = self.databases.clone();
            let mut items_to_load = Vec::new();
            for db in &databases_clone3 {
                let db_name = db.name.clone();
                let schemas_clone = self.schemas.get(&db_name).cloned();
                if let Some(ref schemas) = schemas_clone {
                    let _expanded_schemas = self.expanded_schemas.get(&db_name);
                    for schema in schemas {
                        let schema_name = schema.name.clone();
                        let tables_key = format!("{}.{}", db_name, schema_name);
                        let expanded_tables = self.expanded_tables.get(&tables_key);
                        if let Some(tables) = self.tables.get(&tables_key) {
                            if let Some(expanded) = expanded_tables {
                                for table in tables {
                                    let table_name = table.name.clone();
                                    if expanded.contains(&table_name) {
                                        items_to_load.push((db_name.clone(), schema_name.clone(), table_name));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            for (db_name, schema_name, table_name) in items_to_load {
                let item_key = format!("{}.{}.{}", db_name, schema_name, table_name);
                if !self.loaded_indexes.get(&item_key).copied().unwrap_or(false) {
                    loading::load_indexes(self, db_manager, &db_name, &schema_name, &table_name);
                }
                if !self.loaded_foreign_keys.get(&item_key).copied().unwrap_or(false) {
                    loading::load_foreign_keys(self, db_manager, &db_name, &schema_name, &table_name);
                }
                if !self.loaded_triggers.get(&item_key).copied().unwrap_or(false) {
                    loading::load_triggers(self, db_manager, &db_name, &schema_name, &table_name);
                }
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
                                            
                                            let item_key = format!("{}.{}.{}", db_name, schema_name, table_name);
                                            
                                            // Show indexes
                                            let indexes_key = format!("{}_indexes", item_key);
                                            let expanded_indexes = self.expanded_indexes.entry(item_key.clone()).or_insert_with(HashSet::new);
                                            let is_indexes_expanded = expanded_indexes.contains(&indexes_key);
                                            
                                            let indexes_response = egui::CollapsingHeader::new(
                                                format!("{} Indexes", egui_phosphor::regular::LIST_BULLETS)
                                            )
                                            .default_open(is_indexes_expanded)
                                            .show(ui, |ui| {
                                                if !is_indexes_expanded {
                                                    expanded_indexes.insert(indexes_key.clone());
                                                }
                                                
                                                if let Some(indexes) = self.indexes.get(&item_key) {
                                                    for index in indexes {
                                                        let index_name = index.name.clone();
                                                        if ui.selectable_label(false, &index_name).clicked() {
                                                            self.pending_query_index = Some((db_name.clone(), schema_name.clone(), table_name.clone(), index_name.clone()));
                                                        }
                                                    }
                                                } else {
                                                    ui.label("Loading indexes...");
                                                }
                                            });
                                            
                                            if !is_indexes_expanded && indexes_response.header_response.clicked() {
                                                expanded_indexes.insert(indexes_key.clone());
                                            }
                                            
                                            // Show foreign keys
                                            let fks_key = format!("{}_foreign_keys", item_key);
                                            let expanded_foreign_keys = self.expanded_foreign_keys.entry(item_key.clone()).or_insert_with(HashSet::new);
                                            let is_fks_expanded = expanded_foreign_keys.contains(&fks_key);
                                            
                                            let fks_response = egui::CollapsingHeader::new(
                                                format!("{} Foreign Keys", egui_phosphor::regular::LINK)
                                            )
                                            .default_open(is_fks_expanded)
                                            .show(ui, |ui| {
                                                if !is_fks_expanded {
                                                    expanded_foreign_keys.insert(fks_key.clone());
                                                }
                                                
                                                if let Some(foreign_keys) = self.foreign_keys.get(&item_key) {
                                                    for (idx, fk) in foreign_keys.iter().enumerate() {
                                                        // Generate a display name for the foreign key
                                                        let fk_display = format!("{} -> {}", fk.columns.join(", "), fk.ref_table);
                                                        if ui.selectable_label(false, &fk_display).clicked() {
                                                            // Store foreign key info as JSON string to pass to query function
                                                            // The query function will use columns and ref_table to find the constraint name
                                                            let fk_info = format!("{}|{}|{}", fk.columns.join(","), fk.ref_table, idx);
                                                            self.pending_query_foreign_key = Some((db_name.clone(), schema_name.clone(), table_name.clone(), fk_info));
                                                        }
                                                    }
                                                } else {
                                                    ui.label("Loading foreign keys...");
                                                }
                                            });
                                            
                                            if !is_fks_expanded && fks_response.header_response.clicked() {
                                                expanded_foreign_keys.insert(fks_key.clone());
                                            }
                                            
                                            // Show triggers
                                            let triggers_key = format!("{}_triggers", item_key);
                                            let expanded_triggers = self.expanded_triggers.entry(item_key.clone()).or_insert_with(HashSet::new);
                                            let is_triggers_expanded = expanded_triggers.contains(&triggers_key);
                                            
                                            let triggers_response = egui::CollapsingHeader::new(
                                                format!("{} Triggers", egui_phosphor::regular::LIGHTNING)
                                            )
                                            .default_open(is_triggers_expanded)
                                            .show(ui, |ui| {
                                                if !is_triggers_expanded {
                                                    expanded_triggers.insert(triggers_key.clone());
                                                }
                                                
                                                if let Some(triggers) = self.triggers.get(&item_key) {
                                                    for trigger in triggers {
                                                        let trigger_name = trigger.name.clone();
                                                        if ui.selectable_label(false, &trigger_name).clicked() {
                                                            self.pending_query_trigger = Some((db_name.clone(), schema_name.clone(), table_name.clone(), trigger_name.clone()));
                                                        }
                                                    }
                                                } else {
                                                    ui.label("Loading triggers...");
                                                }
                                            });
                                            
                                            if !is_triggers_expanded && triggers_response.header_response.clicked() {
                                                expanded_triggers.insert(triggers_key.clone());
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
                                                use super::types::DialogType;
                                                self.dialog = Some(DialogType::PropertiesTable {
                                                    database: db_name.clone(),
                                                    schema: schema_name.clone(),
                                                    name: table_name.clone(),
                                                });
                                                ui.close();
                                            }
                                            if ui.button("Rename").clicked() {
                                                use super::types::DialogType;
                                                self.dialog = Some(DialogType::RenameTable {
                                                    database: db_name.clone(),
                                                    schema: schema_name.clone(),
                                                    old_name: table_name.clone(),
                                                });
                                                self.dialog_input = table_name.clone();
                                                ui.close();
                                            }
                                            if ui.button("Delete").clicked() {
                                                use super::types::DialogType;
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
                                        use super::types::DialogType;
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
                                if ui.button("Graph").clicked() {
                                    self.pending_open_graph = Some((db_name.clone(), schema_name.clone()));
                                    ui.close();
                                }
                                if ui.button("New Schema").clicked() {
                                    use super::types::DialogType;
                                    self.dialog = Some(DialogType::CreateSchema {
                                        database: db_name.clone(),
                                    });
                                    self.dialog_input.clear();
                                    ui.close();
                                }
                                if ui.button("Properties").clicked() {
                                    use super::types::DialogType;
                                    self.dialog = Some(DialogType::PropertiesSchema {
                                        database: db_name.clone(),
                                        name: schema_name.clone(),
                                    });
                                    ui.close();
                                }
                                if ui.button("Rename").clicked() {
                                    use super::types::DialogType;
                                    self.dialog = Some(DialogType::RenameSchema {
                                        database: db_name.clone(),
                                        old_name: schema_name.clone(),
                                    });
                                    self.dialog_input = schema_name.clone();
                                    ui.close();
                                }
                                if ui.button("Delete").clicked() {
                                    use super::types::DialogType;
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
                            use super::types::DialogType;
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
                        use super::types::DialogType;
                        self.dialog = Some(DialogType::CreateDatabase);
                        self.dialog_input.clear();
                        ui.close();
                    }
                    if ui.button("Properties").clicked() {
                        use super::types::DialogType;
                        self.dialog = Some(DialogType::PropertiesDatabase {
                            name: db_name.clone(),
                        });
                        ui.close();
                    }
                    if ui.button("Rename").clicked() {
                        use super::types::DialogType;
                        self.dialog = Some(DialogType::RenameDatabase {
                            old_name: db_name.clone(),
                        });
                        self.dialog_input = db_name.clone();
                        ui.close();
                    }
                    if ui.button("Delete").clicked() {
                        use super::types::DialogType;
                        self.dialog = Some(DialogType::DeleteDatabase {
                            name: db_name.clone(),
                        });
                        ui.close();
                    }
                });
            }
            
            // Add database button
            if ui.button(format!("{} New Database", egui_phosphor::regular::PLUS)).clicked() {
                use super::types::DialogType;
                self.dialog = Some(DialogType::CreateDatabase);
                self.dialog_input.clear();
            }
        });

        // Show dialogs
        dialogs::show_dialogs(self, ui, db_manager);
    }

    fn reset(&mut self) {
        self.databases.clear();
        self.loaded_databases = false;
        self.schemas.clear();
        self.loaded_schemas.clear();
        self.tables.clear();
        self.loaded_tables.clear();
        self.indexes.clear();
        self.loaded_indexes.clear();
        self.foreign_keys.clear();
        self.loaded_foreign_keys.clear();
        self.triggers.clear();
        self.loaded_triggers.clear();
        self.expanded_databases.clear();
        self.expanded_schemas.clear();
        self.expanded_tables.clear();
        self.expanded_indexes.clear();
        self.expanded_foreign_keys.clear();
        self.expanded_triggers.clear();
        self.selected_database = None;
        self.selected_schema = None;
        self.selected_table = None;
        self.error = None;
    }
}

