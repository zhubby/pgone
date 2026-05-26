use super::dialogs;
use super::loading;
use super::types::DbTree;
use crate::components::ResultsTable;
use std::collections::HashSet;

impl DbTree {
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        db_manager: &mut crate::components::DbManager,
        results_table: &mut ResultsTable,
    ) {
        // Check if database config changed
        let current_db = db_manager.active_db_config_id.clone();
        if current_db != self.current_db_id {
            self.current_db_id = current_db.clone();
            self.reset();
            if current_db.is_some() {
                loading::load_databases(self, db_manager);
            }
        }

        // Check for async load results
        loading::check_promises(self);
        loading::check_result_promises(self, results_table);

        // Handle pending query table action
        if let Some((database, schema, table)) = self.pending_query_table.take() {
            loading::query_table_data(self, db_manager, results_table, &database, &schema, &table);
        }

        // Handle pending query view action
        if let Some((database, schema, view)) = self.pending_query_view.take() {
            loading::query_view_detail(self, db_manager, results_table, &database, &schema, &view);
        }

        // Handle pending query materialized view action
        if let Some((database, schema, matview)) = self.pending_query_materialized_view.take() {
            loading::query_materialized_view_detail(
                self,
                db_manager,
                results_table,
                &database,
                &schema,
                &matview,
            );
        }

        // Handle pending query function action
        if let Some((database, schema, function)) = self.pending_query_function.take() {
            loading::query_function_detail(
                self,
                db_manager,
                results_table,
                &database,
                &schema,
                &function,
            );
        }

        // Handle pending query index action
        if let Some((database, schema, table, index)) = self.pending_query_index.take() {
            loading::query_index_detail(
                self,
                db_manager,
                results_table,
                &database,
                &schema,
                &table,
                &index,
            );
        }

        // Handle pending query foreign key action
        if let Some((database, schema, table, fk_name)) = self.pending_query_foreign_key.take() {
            loading::query_foreign_key_detail(
                self,
                db_manager,
                results_table,
                &database,
                &schema,
                &table,
                &fk_name,
            );
        }

        // Handle pending query trigger action
        if let Some((database, schema, table, trigger)) = self.pending_query_trigger.take() {
            loading::query_trigger_detail(
                self,
                db_manager,
                results_table,
                &database,
                &schema,
                &table,
                &trigger,
            );
        }

        // Handle pending load DDL action
        if let Some((database, schema, table)) = self.pending_load_ddl.take() {
            loading::load_table_ddl(self, db_manager, &database, &schema, &table);
        }

        // Render tree
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
            db_manager.ensure_storage();
            let Some(storage) = db_manager.storage.as_ref() else {
                let response = ui.allocate_response(ui.available_size_before_wrap(), egui::Sense::click());
                ui.painter().text(
                    response.rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Storage not ready",
                    egui::FontId::proportional(14.0),
                    ui.visuals().text_color(),
                );
                show_blank_area_context_menu(response, db_manager);
                return;
            };

            let connection_configs = storage.list_db_configs();
            if connection_configs.is_empty() {
                let response = ui.allocate_response(ui.available_size_before_wrap(), egui::Sense::click());
                ui.painter().text(
                    response.rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "No connections configured",
                    egui::FontId::proportional(14.0),
                    ui.visuals().text_color(),
                );
                show_blank_area_context_menu(response, db_manager);
                return;
            }

            if let Some(active_id) = db_manager.active_db_config_id.clone()
                && !connection_configs.iter().any(|cfg| cfg.id == active_id)
            {
                db_manager.active_db_config_id = None;
                self.current_db_id = None;
                self.reset();
            }

            for cfg in connection_configs {
                let connection_id = cfg.id.clone();
                let is_active = db_manager.active_db_config_id.as_deref() == Some(connection_id.as_str());

                let connection_response = egui::CollapsingHeader::new(format!(
                    "{} {}",
                    egui_phosphor::regular::PLUG,
                    connection_id
                ))
                .open(Some(is_active))
                .show(ui, |ui| {
                    if let Some(err) = &self.error {
                        ui.colored_label(egui::Color32::RED, format!("Error: {}", err));
                        if ui.button("Retry").clicked() {
                            self.reset();
                            self.current_db_id = Some(connection_id.clone());
                            loading::load_databases(self, db_manager);
                        }
                        return;
                    }

                    if !self.loaded_databases && self.databases_promise.is_none() {
                        loading::load_databases(self, db_manager);
                    }

                    if !self.loaded_databases {
                        ui.label("Loading databases...");
                        return;
                    }

            // 收集需要加载表结构详情的表
            let mut pending_design_loads = Vec::new();

            let database_names = self
                .databases
                .iter()
                .map(|database| database.name.clone())
                .collect::<Vec<_>>();
            for db_name in database_names {
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

                    if !self.loaded_schemas.get(&db_name).copied().unwrap_or(false)
                        && !self.schemas_promises.contains_key(&db_name)
                    {
                        loading::load_schemas(self, db_manager, &db_name);
                    }

                    // Show schemas
                    if let Some(schemas) = schemas_clone {
                        for schema in &schemas {
                            let schema_name = schema.name.clone();
                            let is_schema_expanded = self
                                .expanded_schemas
                                .get(&db_name)
                                .map(|schemas| schemas.contains(&schema_name))
                                .unwrap_or(false);

                            let tables_key = format!("{}.{}", db_name, schema_name);

                            let schema_response = egui::CollapsingHeader::new(
                                format!("{} {}", egui_phosphor::regular::FOLDER, schema_name)
                            )
                            .default_open(is_schema_expanded)
                                        .show(ui, |ui| {
                                if !is_schema_expanded {
                                    self.expanded_schemas
                                        .entry(db_name.clone())
                                        .or_insert_with(HashSet::new)
                                        .insert(schema_name.clone());
                                }

                                // Show Tables category
                                let tables_expanded_key = format!("{}_tables", tables_key);
                                let is_tables_category_expanded = self.expanded_tables
                                    .get(&tables_key)
                                    .map(|s| s.contains(&tables_expanded_key))
                                    .unwrap_or(false);

                                let tables_category_response = egui::CollapsingHeader::new(
                                    format!("{} Tables", egui_phosphor::regular::TABLE)
                                )
                                .default_open(is_tables_category_expanded)
                                .show(ui, |ui| {
                                    if !is_tables_category_expanded {
                                        self.expanded_tables
                                            .entry(tables_key.clone())
                                            .or_insert_with(HashSet::new)
                                            .insert(tables_expanded_key.clone());
                                    }

                                    if !self.loaded_tables.get(&tables_key).copied().unwrap_or(false)
                                        && !self.tables_promises.contains_key(&tables_key)
                                    {
                                        loading::load_tables(self, db_manager, &db_name, &schema_name);
                                    }

                                    if let Some(tables) = self.tables.get(&tables_key).cloned() {
                                        for table in tables {
                                            let table_name = table.name.clone();
                                            let table_expanded_key = format!("table_{}", table_name);
                                            let is_table_expanded = self.expanded_tables
                                                .get(&tables_key)
                                                .map(|tables| tables.contains(&table_expanded_key))
                                                .unwrap_or(false);

                                        let table_response = egui::CollapsingHeader::new(
                                            format!("{} {}", egui_phosphor::regular::TABLE, table_name)
                                        )
                                        .default_open(is_table_expanded)
                                                .show(ui, |ui| {
                                            if !is_table_expanded {
                                                self.expanded_tables
                                                    .entry(tables_key.clone())
                                                    .or_insert_with(HashSet::new)
                                                    .insert(table_expanded_key.clone());
                                            }

                                            let item_key = format!("{}.{}.{}", db_name, schema_name, table_name);

                                            // Show indexes
                                            let indexes_key = format!("{}_indexes", item_key);
                                            let is_indexes_expanded = self.expanded_indexes
                                                .get(&item_key)
                                                .map(|s| s.contains(&indexes_key))
                                                .unwrap_or(false);

                                            let indexes_response = egui::CollapsingHeader::new(
                                                format!("{} Indexes", egui_phosphor::regular::LIST_BULLETS)
                                            )
                                            .default_open(is_indexes_expanded)
                                            .show(ui, |ui| {
                                                if !is_indexes_expanded {
                                                    self.expanded_indexes
                                                        .entry(item_key.clone())
                                                        .or_insert_with(HashSet::new)
                                                        .insert(indexes_key.clone());
                                                }

                                                if !self.loaded_indexes.get(&item_key).copied().unwrap_or(false)
                                                    && !self.indexes_promises.contains_key(&item_key)
                                                {
                                                    loading::load_indexes(
                                                        self,
                                                        db_manager,
                                                        &db_name,
                                                        &schema_name,
                                                        &table_name,
                                                    );
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
                                                self.expanded_indexes
                                                    .entry(item_key.clone())
                                                    .or_insert_with(HashSet::new)
                                                    .insert(indexes_key.clone());
                                            }

                                            // Show foreign keys
                                            let fks_key = format!("{}_foreign_keys", item_key);
                                            let is_fks_expanded = self.expanded_foreign_keys
                                                .get(&item_key)
                                                .map(|s| s.contains(&fks_key))
                                                .unwrap_or(false);

                                            let fks_response = egui::CollapsingHeader::new(
                                                format!("{} Foreign Keys", egui_phosphor::regular::LINK)
                                            )
                                            .default_open(is_fks_expanded)
                                            .show(ui, |ui| {
                                                if !is_fks_expanded {
                                                    self.expanded_foreign_keys
                                                        .entry(item_key.clone())
                                                        .or_insert_with(HashSet::new)
                                                        .insert(fks_key.clone());
                                                }

                                                if !self.loaded_foreign_keys.get(&item_key).copied().unwrap_or(false)
                                                    && !self.foreign_keys_promises.contains_key(&item_key)
                                                {
                                                    loading::load_foreign_keys(
                                                        self,
                                                        db_manager,
                                                        &db_name,
                                                        &schema_name,
                                                        &table_name,
                                                    );
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
                                                self.expanded_foreign_keys
                                                    .entry(item_key.clone())
                                                    .or_insert_with(HashSet::new)
                                                    .insert(fks_key.clone());
                                            }

                                            // Show triggers
                                            let triggers_key = format!("{}_triggers", item_key);
                                            let is_triggers_expanded = self.expanded_triggers
                                                .get(&item_key)
                                                .map(|s| s.contains(&triggers_key))
                                                .unwrap_or(false);

                                            let triggers_response = egui::CollapsingHeader::new(
                                                format!("{} Triggers", egui_phosphor::regular::LIGHTNING)
                                            )
                                            .default_open(is_triggers_expanded)
                                            .show(ui, |ui| {
                                                if !is_triggers_expanded {
                                                    self.expanded_triggers
                                                        .entry(item_key.clone())
                                                        .or_insert_with(HashSet::new)
                                                        .insert(triggers_key.clone());
                                                }

                                                if !self.loaded_triggers.get(&item_key).copied().unwrap_or(false)
                                                    && !self.triggers_promises.contains_key(&item_key)
                                                {
                                                    loading::load_triggers(
                                                        self,
                                                        db_manager,
                                                        &db_name,
                                                        &schema_name,
                                                        &table_name,
                                                    );
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
                                                self.expanded_triggers
                                                    .entry(item_key.clone())
                                                    .or_insert_with(HashSet::new)
                                                    .insert(triggers_key.clone());
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

                                        // 克隆需要的值以避免借用冲突
                                        let db_name_menu = db_name.clone();
                                        let schema_name_menu = schema_name.clone();
                                        let table_name_menu = table_name.clone();

                                        table_response.header_response.context_menu(|ui| {
                                            if ui.button("Query Table").clicked() {
                                                self.pending_query_table = Some((db_name_menu.clone(), schema_name_menu.clone(), table_name_menu.clone()));
                                                ui.close();
                                            }
                                            if ui.button("New Query").clicked() {
                                                self.pending_open_sql_editor = true;
                                                ui.close();
                                            }
                                            if ui.button("Show DDL").clicked() {
                                                use super::types::DialogType;
                                                self.dialog = Some(DialogType::ShowDdl {
                                                    database: db_name_menu.clone(),
                                                    schema: schema_name_menu.clone(),
                                                    name: table_name_menu.clone(),
                                                });
                                                self.dialog_ddl_content.clear();
                                                self.pending_load_ddl = Some((db_name_menu.clone(), schema_name_menu.clone(), table_name_menu.clone()));
                                                ui.close();
                                            }
                                            if ui.button("Design").clicked() {
                                                use super::types::DialogType;
                                                self.dialog = Some(DialogType::DesignTable {
                                                    database: db_name_menu.clone(),
                                                    schema: schema_name_menu.clone(),
                                                    name: table_name_menu.clone(),
                                                });
                                                ui.close();
                                            }
                                            if ui.button("Properties").clicked() {
                                                use super::types::DialogType;
                                                self.dialog = Some(DialogType::PropertiesTable {
                                                    database: db_name_menu.clone(),
                                                    schema: schema_name_menu.clone(),
                                                    name: table_name_menu.clone(),
                                                });
                                                ui.close();
                                            }
                                            if ui.button("Rename").clicked() {
                                                use super::types::DialogType;
                                                self.dialog = Some(DialogType::RenameTable {
                                                    database: db_name_menu.clone(),
                                                    schema: schema_name_menu.clone(),
                                                    old_name: table_name_menu.clone(),
                                                });
                                                self.dialog_input = table_name_menu.clone();
                                                ui.close();
                                            }
                                            if ui.button("Delete").clicked() {
                                                use super::types::DialogType;
                                                self.dialog = Some(DialogType::DeleteTable {
                                                    database: db_name_menu.clone(),
                                                    schema: schema_name_menu.clone(),
                                                    name: table_name_menu.clone(),
                                                });
                                                ui.close();
                                            }
                                            if ui.button("Drop").clicked() {
                                                use super::types::DialogType;
                                                self.dialog = Some(DialogType::DropTable {
                                                    database: db_name_menu.clone(),
                                                    schema: schema_name_menu.clone(),
                                                    name: table_name_menu.clone(),
                                                });
                                                ui.close();
                                            }
                                        });

                                        // 收集需要加载的表设计信息
                                        use super::types::DialogType;
                                        if let Some(DialogType::DesignTable { database, schema, name }) = &self.dialog {
                                            if *database == db_name && *schema == schema_name && *name == table_name {
                                                pending_design_loads.push((db_name.clone(), schema_name.clone(), table_name.clone()));
                                            }
                                        }
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

                                if !is_tables_category_expanded && tables_category_response.header_response.clicked() {
                                    self.expanded_tables
                                        .entry(tables_key.clone())
                                        .or_insert_with(HashSet::new)
                                        .insert(tables_expanded_key.clone());
                                }

                                // Show Views category
                                let views_expanded_key = format!("{}_views", tables_key);
                                let is_views_category_expanded = self.expanded_views
                                    .get(&tables_key)
                                    .map(|set| set.contains(&views_expanded_key))
                                    .unwrap_or(false);

                                let views_category_response = egui::CollapsingHeader::new(
                                    format!("{} Views", egui_phosphor::regular::EYE)
                                )
                                .default_open(is_views_category_expanded)
                                .show(ui, |ui| {
                                    if !is_views_category_expanded {
                                        self.expanded_views
                                            .entry(tables_key.clone())
                                            .or_insert_with(HashSet::new)
                                            .insert(views_expanded_key.clone());
                                    }

                                    if !self.loaded_views.get(&tables_key).copied().unwrap_or(false)
                                        && !self.views_promises.contains_key(&tables_key)
                                    {
                                        loading::load_views(self, db_manager, &db_name, &schema_name);
                                    }

                                    if let Some(views) = self.views.get(&tables_key) {
                                        for view in views {
                                            let view_name = view.name.clone();
                                            let db_name_menu = db_name.clone();
                                            let schema_name_menu = schema_name.clone();
                                            let view_name_menu = view_name.clone();

                                            let view_response = ui.selectable_label(false, &view_name);
                                            if view_response.clicked() {
                                                self.pending_query_view = Some((db_name_menu.clone(), schema_name_menu.clone(), view_name_menu.clone()));
                                            }

                                            // Context menu for view
                                            view_response.context_menu(|ui| {
                                                if ui.button("Properties").clicked() {
                                                    use super::types::DialogType;
                                                    self.dialog = Some(DialogType::PropertiesView {
                                                        database: db_name_menu.clone(),
                                                        schema: schema_name_menu.clone(),
                                                        name: view_name_menu.clone(),
                                                    });
                                                    ui.close();
                                                }
                                            });
                                        }

                                        // Add view button
                                        if ui.button(format!("{} New View", egui_phosphor::regular::PLUS)).clicked() {
                                            use super::types::DialogType;
                                            self.dialog = Some(DialogType::CreateView {
                                                database: db_name.clone(),
                                                schema: schema_name.clone(),
                                            });
                                            self.dialog_ddl = format!("CREATE VIEW {}.{} AS\nSELECT * FROM {};", schema_name, "new_view", "table_name");
                                        }
                                    } else {
                                        ui.label("Loading views...");
                                    }
                                });

                                if !is_views_category_expanded && views_category_response.header_response.clicked() {
                                    self.expanded_views
                                        .entry(tables_key.clone())
                                        .or_insert_with(HashSet::new)
                                        .insert(views_expanded_key.clone());
                                }

                                // Show Materialized Views category
                                let matviews_expanded_key = format!("{}_materialized_views", tables_key);
                                let is_matviews_category_expanded = self.expanded_materialized_views
                                    .get(&tables_key)
                                    .map(|set| set.contains(&matviews_expanded_key))
                                    .unwrap_or(false);

                                let matviews_category_response = egui::CollapsingHeader::new(
                                    format!("{} Materialized Views", egui_phosphor::regular::STACK)
                                )
                                .default_open(is_matviews_category_expanded)
                                .show(ui, |ui| {
                                    if !is_matviews_category_expanded {
                                        self.expanded_materialized_views
                                            .entry(tables_key.clone())
                                            .or_insert_with(HashSet::new)
                                            .insert(matviews_expanded_key.clone());
                                    }

                                    if !self
                                        .loaded_materialized_views
                                        .get(&tables_key)
                                        .copied()
                                        .unwrap_or(false)
                                        && !self.materialized_views_promises.contains_key(&tables_key)
                                    {
                                        loading::load_materialized_views(
                                            self,
                                            db_manager,
                                            &db_name,
                                            &schema_name,
                                        );
                                    }

                                    if let Some(materialized_views) = self.materialized_views.get(&tables_key) {
                                        for matview in materialized_views {
                                            let matview_name = matview.name.clone();
                                            let db_name_menu = db_name.clone();
                                            let schema_name_menu = schema_name.clone();
                                            let matview_name_menu = matview_name.clone();

                                            let matview_response = ui.selectable_label(false, &matview_name);
                                            if matview_response.clicked() {
                                                self.pending_query_materialized_view = Some((db_name_menu.clone(), schema_name_menu.clone(), matview_name_menu.clone()));
                                            }

                                            // Context menu for materialized view
                                            matview_response.context_menu(|ui| {
                                                if ui.button("Properties").clicked() {
                                                    use super::types::DialogType;
                                                    self.dialog = Some(DialogType::PropertiesMaterializedView {
                                                        database: db_name_menu.clone(),
                                                        schema: schema_name_menu.clone(),
                                                        name: matview_name_menu.clone(),
                                                    });
                                                    ui.close();
                                                }
                                            });
                                        }

                                        // Add materialized view button
                                        if ui.button(format!("{} New Materialized View", egui_phosphor::regular::PLUS)).clicked() {
                                            use super::types::DialogType;
                                            self.dialog = Some(DialogType::CreateMaterializedView {
                                                database: db_name.clone(),
                                                schema: schema_name.clone(),
                                            });
                                            self.dialog_ddl = format!("CREATE MATERIALIZED VIEW {}.{} AS\nSELECT * FROM {};", schema_name, "new_materialized_view", "table_name");
                                        }
                                    } else {
                                        ui.label("Loading materialized views...");
                                    }
                                });

                                if !is_matviews_category_expanded && matviews_category_response.header_response.clicked() {
                                    self.expanded_materialized_views
                                        .entry(tables_key.clone())
                                        .or_insert_with(HashSet::new)
                                        .insert(matviews_expanded_key.clone());
                                }

                                // Show Functions category
                                let functions_expanded_key = format!("{}_functions", tables_key);
                                let is_functions_category_expanded = self.expanded_functions
                                    .get(&tables_key)
                                    .map(|set| set.contains(&functions_expanded_key))
                                    .unwrap_or(false);

                                let functions_category_response = egui::CollapsingHeader::new(
                                    format!("{} Functions", egui_phosphor::regular::FUNCTION)
                                )
                                .default_open(is_functions_category_expanded)
                                .show(ui, |ui| {
                                    if !is_functions_category_expanded {
                                        self.expanded_functions
                                            .entry(tables_key.clone())
                                            .or_insert_with(HashSet::new)
                                            .insert(functions_expanded_key.clone());
                                    }

                                    if !self.loaded_functions.get(&tables_key).copied().unwrap_or(false)
                                        && !self.functions_promises.contains_key(&tables_key)
                                    {
                                        loading::load_functions(self, db_manager, &db_name, &schema_name);
                                    }

                                    if let Some(functions) = self.functions.get(&tables_key) {
                                        for function in functions {
                                            let function_name = function.name.clone();
                                            let db_name_menu = db_name.clone();
                                            let schema_name_menu = schema_name.clone();
                                            let function_name_menu = function_name.clone();

                                            let function_response = ui.selectable_label(false, &function_name);
                                            if function_response.clicked() {
                                                self.pending_query_function = Some((db_name_menu.clone(), schema_name_menu.clone(), function_name_menu.clone()));
                                            }

                                            // Context menu for function
                                            function_response.context_menu(|ui| {
                                                if ui.button("Properties").clicked() {
                                                    use super::types::DialogType;
                                                    self.dialog = Some(DialogType::PropertiesFunction {
                                                        database: db_name_menu.clone(),
                                                        schema: schema_name_menu.clone(),
                                                        name: function_name_menu.clone(),
                                                    });
                                                    ui.close();
                                                }
                                            });
                                        }

                                        // Add function button
                                        if ui.button(format!("{} New Function", egui_phosphor::regular::PLUS)).clicked() {
                                            use super::types::DialogType;
                                            self.dialog = Some(DialogType::CreateFunction {
                                                database: db_name.clone(),
                                                schema: schema_name.clone(),
                                            });
                                            self.dialog_ddl = format!("CREATE OR REPLACE FUNCTION {}.{}()\nRETURNS INTEGER AS $$\nBEGIN\n    RETURN 1;\nEND;\n$$ LANGUAGE plpgsql;", schema_name, "new_function");
                                        }
                                    } else {
                                        ui.label("Loading functions...");
                                    }
                                });

                                if !is_functions_category_expanded && functions_category_response.header_response.clicked() {
                                    self.expanded_functions
                                        .entry(tables_key.clone())
                                        .or_insert_with(HashSet::new)
                                        .insert(functions_expanded_key.clone());
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

            // 在循环外处理异步加载，避免借用冲突
            for (db_name, schema_name, table_name) in pending_design_loads {
                loading::load_table_detail_for_design(self, db_manager, &db_name, &schema_name, &table_name);
            }

                });

                if connection_response.header_response.clicked() && !is_active {
                    db_manager.select_db_config(&connection_id);
                    self.current_db_id = Some(connection_id.clone());
                    self.reset();
                    loading::load_databases(self, db_manager);
                }

                connection_response.header_response.context_menu(|ui| {
                    if ui.button("Edit Connection").clicked() {
                        if let Err(error) = db_manager.open_edit_db_config(&connection_id) {
                            crate::notify::error(format!("Failed to load: {}", error));
                        }
                        ui.close();
                    }
                    if ui.button("Delete Connection").clicked() {
                        db_manager.request_delete_db_config(&connection_id);
                        ui.close();
                    }
                });
            }

            show_blank_area_context_menu(
                ui.allocate_response(ui.available_size_before_wrap(), egui::Sense::click()),
                db_manager,
            );
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
        self.views.clear();
        self.loaded_views.clear();
        self.materialized_views.clear();
        self.loaded_materialized_views.clear();
        self.functions.clear();
        self.loaded_functions.clear();
        self.indexes.clear();
        self.loaded_indexes.clear();
        self.foreign_keys.clear();
        self.loaded_foreign_keys.clear();
        self.triggers.clear();
        self.loaded_triggers.clear();
        self.expanded_databases.clear();
        self.expanded_schemas.clear();
        self.expanded_tables.clear();
        self.expanded_views.clear();
        self.expanded_materialized_views.clear();
        self.expanded_functions.clear();
        self.expanded_indexes.clear();
        self.expanded_foreign_keys.clear();
        self.expanded_triggers.clear();
        self.selected_database = None;
        self.selected_schema = None;
        self.selected_table = None;
        self.error = None;
        self.design_table_detail = None;
        self.design_table_columns.clear();
        self.design_table_promise = None;
        self.ddl_promise = None;
        self.dialog_ddl_content.clear();
        self.pending_load_ddl = None;
    }
}

fn show_blank_area_context_menu(
    response: egui::Response,
    db_manager: &mut crate::components::DbManager,
) {
    response.context_menu(|ui| {
        if ui.button("New Connection").clicked() {
            db_manager.show_add_db = true;
            ui.close();
        }
    });
}
