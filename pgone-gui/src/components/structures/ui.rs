use super::dialogs;
use super::loading;
use super::types::DbTree;
use crate::components::ResultsTable;
use std::collections::HashSet;

impl DbTree {
    fn handle_connection_state(
        &mut self,
        db_manager: &mut crate::components::DbManager,
        results_table: &mut ResultsTable,
    ) {
        loading::check_promises(self);
        loading::check_result_promises(self, results_table);

        if let Some((database, schema, table)) = self.pending_query_table.take() {
            loading::query_table_data(self, db_manager, results_table, &database, &schema, &table);
        }

        if let Some((database, schema, view)) = self.pending_query_view.take() {
            loading::query_view_detail(self, db_manager, results_table, &database, &schema, &view);
        }

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

        if let Some((database, schema, table)) = self.pending_load_ddl.take() {
            loading::load_table_ddl(self, db_manager, &database, &schema, &table);
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        db_manager: &mut crate::components::DbManager,
        results_table: &mut ResultsTable,
    ) {
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
            }

            let known_connection_ids = connection_configs
                .iter()
                .map(|cfg| cfg.id.clone())
                .collect::<HashSet<_>>();
            self.connection_states
                .retain(|connection_id, _| known_connection_ids.contains(connection_id));
            self.expanded_connections
                .retain(|connection_id| known_connection_ids.contains(connection_id));

            for cfg in connection_configs {
                let connection_id = cfg.id.clone();
                let state = self
                    .connection_states
                    .remove(&connection_id)
                    .unwrap_or_default();
                self.load_connection_state(connection_id.clone(), state);
                self.handle_connection_state(db_manager, results_table);
                let is_active =
                    db_manager.active_db_config_id.as_deref() == Some(connection_id.as_str());
                let connection_label = if is_active {
                    format!(
                        "{} {}  [Active]",
                        egui_phosphor::regular::PLUG,
                        connection_id
                    )
                } else {
                    format!("{} {}", egui_phosphor::regular::PLUG, connection_id)
                };
                let mut connection_title = egui::RichText::new(connection_label);
                if is_active {
                    connection_title = connection_title
                        .strong()
                        .color(ui.visuals().selection.stroke.color);
                }

                let connection_response = egui::CollapsingHeader::new(connection_title)
                    .default_open(self.expanded_connections.contains(&connection_id))
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

            // Collect tables that need to load structure details
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
                        if schemas.is_empty() {
                            empty_placeholder(ui, "No schemas");
                        }

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
                                        if tables.is_empty() {
                                            empty_placeholder(ui, "No tables");
                                        }

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
                                                    if indexes.is_empty() {
                                                        empty_placeholder(ui, "No indexes");
                                                    }

                                                    for index in indexes {
                                                        let index_name = index.name.clone();
                                                        let db_name_menu = db_name.clone();
                                                        let schema_name_menu = schema_name.clone();
                                                        let table_name_menu = table_name.clone();
                                                        let index_name_menu = index_name.clone();
                                                        let index_response =
                                                            ui.selectable_label(false, &index_name);
                                                        if index_response.clicked() {
                                                            self.pending_query_index = Some((db_name_menu.clone(), schema_name_menu.clone(), table_name_menu.clone(), index_name_menu.clone()));
                                                        }

                                                        index_response.context_menu(|ui| {
                                                            if refresh_menu_button(ui).clicked() {
                                                                self.pending_query_index = Some((db_name_menu.clone(), schema_name_menu.clone(), table_name_menu.clone(), index_name_menu.clone()));
                                                                ui.close();
                                                            }
                                                        });
                                                    }
                                                } else {
                                                    ui.label("Loading indexes...");
                                                }
                                            });

                                            indexes_response.header_response.context_menu(|ui| {
                                                if refresh_menu_button(ui).clicked() {
                                                    loading::refresh_indexes(
                                                        self,
                                                        db_manager,
                                                        &db_name,
                                                        &schema_name,
                                                        &table_name,
                                                    );
                                                    ui.close();
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
                                                    if foreign_keys.is_empty() {
                                                        empty_placeholder(ui, "No foreign keys");
                                                    }

                                                    for (idx, fk) in foreign_keys.iter().enumerate() {
                                                        // Generate a display name for the foreign key
                                                        let fk_display = format!("{} -> {}", fk.columns.join(", "), fk.ref_table);
                                                        let fk_info = format!("{}|{}|{}", fk.columns.join(","), fk.ref_table, idx);
                                                        let db_name_menu = db_name.clone();
                                                        let schema_name_menu = schema_name.clone();
                                                        let table_name_menu = table_name.clone();
                                                        let fk_info_menu = fk_info.clone();
                                                        let fk_response =
                                                            ui.selectable_label(false, &fk_display);
                                                        if fk_response.clicked() {
                                                            self.pending_query_foreign_key = Some((db_name_menu.clone(), schema_name_menu.clone(), table_name_menu.clone(), fk_info_menu.clone()));
                                                        }

                                                        fk_response.context_menu(|ui| {
                                                            if refresh_menu_button(ui).clicked() {
                                                                self.pending_query_foreign_key = Some((db_name_menu.clone(), schema_name_menu.clone(), table_name_menu.clone(), fk_info_menu.clone()));
                                                                ui.close();
                                                            }
                                                        });
                                                    }
                                                } else {
                                                    ui.label("Loading foreign keys...");
                                                }
                                            });

                                            fks_response.header_response.context_menu(|ui| {
                                                if refresh_menu_button(ui).clicked() {
                                                    loading::refresh_foreign_keys(
                                                        self,
                                                        db_manager,
                                                        &db_name,
                                                        &schema_name,
                                                        &table_name,
                                                    );
                                                    ui.close();
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
                                                    if triggers.is_empty() {
                                                        empty_placeholder(ui, "No triggers");
                                                    }

                                                    for trigger in triggers {
                                                        let trigger_name = trigger.name.clone();
                                                        let db_name_menu = db_name.clone();
                                                        let schema_name_menu = schema_name.clone();
                                                        let table_name_menu = table_name.clone();
                                                        let trigger_name_menu = trigger_name.clone();
                                                        let trigger_response =
                                                            ui.selectable_label(false, &trigger_name);
                                                        if trigger_response.clicked() {
                                                            self.pending_query_trigger = Some((db_name_menu.clone(), schema_name_menu.clone(), table_name_menu.clone(), trigger_name_menu.clone()));
                                                        }

                                                        trigger_response.context_menu(|ui| {
                                                            if refresh_menu_button(ui).clicked() {
                                                                self.pending_query_trigger = Some((db_name_menu.clone(), schema_name_menu.clone(), table_name_menu.clone(), trigger_name_menu.clone()));
                                                                ui.close();
                                                            }
                                                        });
                                                    }
                                                } else {
                                                    ui.label("Loading triggers...");
                                                }
                                            });

                                            triggers_response.header_response.context_menu(|ui| {
                                                if refresh_menu_button(ui).clicked() {
                                                    loading::refresh_triggers(
                                                        self,
                                                        db_manager,
                                                        &db_name,
                                                        &schema_name,
                                                        &table_name,
                                                    );
                                                    ui.close();
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

                                        // Clone needed values to avoid borrow conflicts
                                        let db_name_menu = db_name.clone();
                                        let schema_name_menu = schema_name.clone();
                                        let table_name_menu = table_name.clone();

                                        table_response.header_response.context_menu(|ui| {
                                            if refresh_menu_button(ui).clicked() {
                                                self.pending_query_table = Some((db_name_menu.clone(), schema_name_menu.clone(), table_name_menu.clone()));
                                                loading::refresh_table_children(
                                                    self,
                                                    db_manager,
                                                    &db_name_menu,
                                                    &schema_name_menu,
                                                    &table_name_menu,
                                                );
                                                ui.close();
                                            }
                                            if menu_button(
                                                ui,
                                                egui_phosphor::regular::MAGNIFYING_GLASS,
                                                "Query Table",
                                            )
                                            .clicked()
                                            {
                                                self.pending_query_table = Some((db_name_menu.clone(), schema_name_menu.clone(), table_name_menu.clone()));
                                                ui.close();
                                            }
                                            if menu_button(ui, egui_phosphor::regular::FILE_SQL, "New Query")
                                                .clicked()
                                            {
                                                self.pending_open_sql_editor = true;
                                                ui.close();
                                            }
                                            if menu_button(ui, egui_phosphor::regular::TABLE, "New Table")
                                                .clicked()
                                            {
                                                use super::types::DialogType;
                                                self.dialog = Some(DialogType::CreateTable {
                                                    database: db_name_menu.clone(),
                                                    schema: schema_name_menu.clone(),
                                                });
                                                self.dialog_ddl = format!(
                                                    "CREATE TABLE {}.{} (\n    id SERIAL PRIMARY KEY\n);",
                                                    schema_name_menu, "new_table"
                                                );
                                                ui.close();
                                            }
                                            if menu_button(ui, egui_phosphor::regular::CODE, "Show DDL")
                                                .clicked()
                                            {
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
                                            if menu_button(ui, egui_phosphor::regular::WRENCH, "Design")
                                                .clicked()
                                            {
                                                use super::types::DialogType;
                                                self.dialog = Some(DialogType::DesignTable {
                                                    database: db_name_menu.clone(),
                                                    schema: schema_name_menu.clone(),
                                                    name: table_name_menu.clone(),
                                                });
                                                ui.close();
                                            }
                                            if menu_button(ui, egui_phosphor::regular::GEAR, "Properties")
                                                .clicked()
                                            {
                                                use super::types::DialogType;
                                                self.dialog = Some(DialogType::PropertiesTable {
                                                    database: db_name_menu.clone(),
                                                    schema: schema_name_menu.clone(),
                                                    name: table_name_menu.clone(),
                                                });
                                                ui.close();
                                            }
                                            if menu_button(ui, egui_phosphor::regular::PENCIL, "Rename")
                                                .clicked()
                                            {
                                                use super::types::DialogType;
                                                self.dialog = Some(DialogType::RenameTable {
                                                    database: db_name_menu.clone(),
                                                    schema: schema_name_menu.clone(),
                                                    old_name: table_name_menu.clone(),
                                                });
                                                self.dialog_input = table_name_menu.clone();
                                                ui.close();
                                            }
                                            if danger_menu_button(ui, egui_phosphor::regular::TRASH, "Delete")
                                                .clicked()
                                            {
                                                use super::types::DialogType;
                                                self.dialog = Some(DialogType::DeleteTable {
                                                    database: db_name_menu.clone(),
                                                    schema: schema_name_menu.clone(),
                                                    name: table_name_menu.clone(),
                                                });
                                                ui.close();
                                            }
                                            if danger_menu_button(ui, egui_phosphor::regular::TRASH, "Drop")
                                                .clicked()
                                            {
                                                use super::types::DialogType;
                                                self.dialog = Some(DialogType::DropTable {
                                                    database: db_name_menu.clone(),
                                                    schema: schema_name_menu.clone(),
                                                    name: table_name_menu.clone(),
                                                });
                                                ui.close();
                                            }
                                        });

                                        // Collect table design info that needs to be loaded
                                        use super::types::DialogType;
                                        if let Some(DialogType::DesignTable { database, schema, name }) = &self.dialog {
                                            if *database == db_name && *schema == schema_name && *name == table_name {
                                                pending_design_loads.push((db_name.clone(), schema_name.clone(), table_name.clone()));
                                            }
                                        }
                                    }

                                    } else {
                                        ui.label("Loading tables...");
                                    }
                                });

                                tables_category_response.header_response.context_menu(|ui| {
                                    if refresh_menu_button(ui).clicked() {
                                        loading::refresh_tables(self, db_manager, &db_name, &schema_name);
                                        ui.close();
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
                                        if views.is_empty() {
                                            empty_placeholder(ui, "No views");
                                        }

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
                                                if refresh_menu_button(ui).clicked() {
                                                    self.pending_query_view = Some((db_name_menu.clone(), schema_name_menu.clone(), view_name_menu.clone()));
                                                    ui.close();
                                                }
                                                if menu_button(
                                                    ui,
                                                    egui_phosphor::regular::GEAR,
                                                    "Properties",
                                                )
                                                .clicked()
                                                {
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

                                    } else {
                                        ui.label("Loading views...");
                                    }
                                });

                                views_category_response.header_response.context_menu(|ui| {
                                    if refresh_menu_button(ui).clicked() {
                                        loading::refresh_views(self, db_manager, &db_name, &schema_name);
                                        ui.close();
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
                                        if materialized_views.is_empty() {
                                            empty_placeholder(ui, "No materialized views");
                                        }

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
                                                if refresh_menu_button(ui).clicked() {
                                                    self.pending_query_materialized_view = Some((db_name_menu.clone(), schema_name_menu.clone(), matview_name_menu.clone()));
                                                    ui.close();
                                                }
                                                if menu_button(
                                                    ui,
                                                    egui_phosphor::regular::GEAR,
                                                    "Properties",
                                                )
                                                .clicked()
                                                {
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

                                    } else {
                                        ui.label("Loading materialized views...");
                                    }
                                });

                                matviews_category_response.header_response.context_menu(|ui| {
                                    if refresh_menu_button(ui).clicked() {
                                        loading::refresh_materialized_views(
                                            self,
                                            db_manager,
                                            &db_name,
                                            &schema_name,
                                        );
                                        ui.close();
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
                                        if functions.is_empty() {
                                            empty_placeholder(ui, "No functions");
                                        }

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
                                                if refresh_menu_button(ui).clicked() {
                                                    self.pending_query_function = Some((db_name_menu.clone(), schema_name_menu.clone(), function_name_menu.clone()));
                                                    ui.close();
                                                }
                                                if menu_button(
                                                    ui,
                                                    egui_phosphor::regular::GEAR,
                                                    "Properties",
                                                )
                                                .clicked()
                                                {
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

                                    } else {
                                        ui.label("Loading functions...");
                                    }
                                });

                                functions_category_response.header_response.context_menu(|ui| {
                                    if refresh_menu_button(ui).clicked() {
                                        loading::refresh_functions(
                                            self,
                                            db_manager,
                                            &db_name,
                                            &schema_name,
                                        );
                                        ui.close();
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
                                if refresh_menu_button(ui).clicked() {
                                    loading::refresh_schema_children(
                                        self,
                                        db_manager,
                                        &db_name,
                                        &schema_name,
                                    );
                                    ui.close();
                                }
                                if menu_button(ui, egui_phosphor::regular::GRAPH, "Graph").clicked() {
                                    self.pending_open_graph = Some((db_name.clone(), schema_name.clone()));
                                    ui.close();
                                }
                                if menu_button(ui, egui_phosphor::regular::TABLE, "New Table")
                                    .clicked()
                                {
                                    use super::types::DialogType;
                                    self.dialog = Some(DialogType::CreateTable {
                                        database: db_name.clone(),
                                        schema: schema_name.clone(),
                                    });
                                    self.dialog_ddl = format!(
                                        "CREATE TABLE {}.{} (\n    id SERIAL PRIMARY KEY\n);",
                                        schema_name, "new_table"
                                    );
                                    ui.close();
                                }
                                if menu_button(ui, egui_phosphor::regular::EYE, "New View")
                                    .clicked()
                                {
                                    use super::types::DialogType;
                                    self.dialog = Some(DialogType::CreateView {
                                        database: db_name.clone(),
                                        schema: schema_name.clone(),
                                    });
                                    self.dialog_ddl = format!(
                                        "CREATE VIEW {}.{} AS\nSELECT * FROM {};",
                                        schema_name, "new_view", "table_name"
                                    );
                                    ui.close();
                                }
                                if menu_button(
                                    ui,
                                    egui_phosphor::regular::STACK,
                                    "New Materialized View",
                                )
                                .clicked()
                                {
                                    use super::types::DialogType;
                                    self.dialog = Some(DialogType::CreateMaterializedView {
                                        database: db_name.clone(),
                                        schema: schema_name.clone(),
                                    });
                                    self.dialog_ddl = format!(
                                        "CREATE MATERIALIZED VIEW {}.{} AS\nSELECT * FROM {};",
                                        schema_name, "new_materialized_view", "table_name"
                                    );
                                    ui.close();
                                }
                                if menu_button(ui, egui_phosphor::regular::FUNCTION, "New Function")
                                    .clicked()
                                {
                                    use super::types::DialogType;
                                    self.dialog = Some(DialogType::CreateFunction {
                                        database: db_name.clone(),
                                        schema: schema_name.clone(),
                                    });
                                    self.dialog_ddl = format!(
                                        "CREATE OR REPLACE FUNCTION {}.{}()\nRETURNS INTEGER AS $$\nBEGIN\n    RETURN 1;\nEND;\n$$ LANGUAGE plpgsql;",
                                        schema_name, "new_function"
                                    );
                                    ui.close();
                                }
                                if menu_button(ui, egui_phosphor::regular::GEAR, "Properties")
                                    .clicked()
                                {
                                    use super::types::DialogType;
                                    self.dialog = Some(DialogType::PropertiesSchema {
                                        database: db_name.clone(),
                                        name: schema_name.clone(),
                                    });
                                    ui.close();
                                }
                                if menu_button(ui, egui_phosphor::regular::PENCIL, "Rename")
                                    .clicked()
                                {
                                    use super::types::DialogType;
                                    self.dialog = Some(DialogType::RenameSchema {
                                        database: db_name.clone(),
                                        old_name: schema_name.clone(),
                                    });
                                    self.dialog_input = schema_name.clone();
                                    ui.close();
                                }
                                if danger_menu_button(ui, egui_phosphor::regular::TRASH, "Delete")
                                    .clicked()
                                {
                                    use super::types::DialogType;
                                    self.dialog = Some(DialogType::DeleteSchema {
                                        database: db_name.clone(),
                                        name: schema_name.clone(),
                                    });
                                    ui.close();
                                }
                            });
                        }

                    } else {
                        ui.label("Loading schemas...");
                    }
                });

                // Handle database context menu
                response.header_response.context_menu(|ui| {
                    if refresh_menu_button(ui).clicked() {
                        loading::refresh_schemas(self, db_manager, &db_name);
                        ui.close();
                    }
                    if menu_button(ui, egui_phosphor::regular::FOLDER_PLUS, "New Schema").clicked() {
                        use super::types::DialogType;
                        self.dialog = Some(DialogType::CreateSchema {
                            database: db_name.clone(),
                        });
                        self.dialog_input.clear();
                        ui.close();
                    }
                    if menu_button(ui, egui_phosphor::regular::GEAR, "Properties").clicked() {
                        use super::types::DialogType;
                        self.dialog = Some(DialogType::PropertiesDatabase {
                            name: db_name.clone(),
                        });
                        ui.close();
                    }
                    if menu_button(ui, egui_phosphor::regular::PENCIL, "Rename").clicked() {
                        use super::types::DialogType;
                        self.dialog = Some(DialogType::RenameDatabase {
                            old_name: db_name.clone(),
                        });
                        self.dialog_input = db_name.clone();
                        ui.close();
                    }
                    if danger_menu_button(ui, egui_phosphor::regular::TRASH, "Delete").clicked() {
                        use super::types::DialogType;
                        self.dialog = Some(DialogType::DeleteDatabase {
                            name: db_name.clone(),
                        });
                        ui.close();
                    }
                });
            }

            // Process async loads outside the loop to avoid borrow conflicts
            for (db_name, schema_name, table_name) in pending_design_loads {
                loading::load_table_detail_for_design(self, db_manager, &db_name, &schema_name, &table_name);
            }

                });

                if connection_response.header_response.clicked() && !is_active {
                    db_manager.select_db_config(&connection_id);
                }

                if connection_response.fully_open() {
                    self.expanded_connections.insert(connection_id.clone());
                } else if connection_response.fully_closed() {
                    self.expanded_connections.remove(&connection_id);
                }

                connection_response.header_response.context_menu(|ui| {
                    if refresh_menu_button(ui).clicked() {
                        if !is_active {
                            db_manager.select_db_config(&connection_id);
                        }
                        self.reset();
                        loading::refresh_databases(self, db_manager);
                        ui.close();
                    }
                    if menu_button(ui, egui_phosphor::regular::DATABASE, "New Database").clicked() {
                        use super::types::DialogType;
                        self.dialog = Some(DialogType::CreateDatabase);
                        self.dialog_input.clear();
                        ui.close();
                    }
                    if menu_button(ui, egui_phosphor::regular::PENCIL, "Edit Connection").clicked() {
                        if let Err(error) = db_manager.open_edit_db_config(&connection_id) {
                            crate::notify::error(format!("Failed to load: {}", error));
                        }
                        ui.close();
                    }
                    if danger_menu_button(ui, egui_phosphor::regular::TRASH, "Delete").clicked() {
                        db_manager.request_delete_db_config(&connection_id);
                        ui.close();
                    }
                });

                dialogs::show_dialogs(self, ui, db_manager);
                let state = self.take_connection_state();
                self.connection_states.insert(connection_id.clone(), state);
            }

            show_blank_area_context_menu(
                ui.allocate_response(ui.available_size_before_wrap(), egui::Sense::click()),
                db_manager,
            );
        });

        self.current_db_id = db_manager.active_db_config_id.clone();
    }

    fn reset(&mut self) {
        self.databases.clear();
        self.loaded_databases = false;
        self.databases_promise = None;
        self.schemas.clear();
        self.loaded_schemas.clear();
        self.schemas_promises.clear();
        self.tables.clear();
        self.loaded_tables.clear();
        self.tables_promises.clear();
        self.views.clear();
        self.loaded_views.clear();
        self.views_promises.clear();
        self.materialized_views.clear();
        self.loaded_materialized_views.clear();
        self.materialized_views_promises.clear();
        self.functions.clear();
        self.loaded_functions.clear();
        self.functions_promises.clear();
        self.indexes.clear();
        self.loaded_indexes.clear();
        self.indexes_promises.clear();
        self.foreign_keys.clear();
        self.loaded_foreign_keys.clear();
        self.foreign_keys_promises.clear();
        self.triggers.clear();
        self.loaded_triggers.clear();
        self.triggers_promises.clear();
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
        self.results_promise = None;
        self.dialog_ddl_content.clear();
        self.pending_load_ddl = None;
    }
}

fn show_blank_area_context_menu(
    response: egui::Response,
    db_manager: &mut crate::components::DbManager,
) {
    response.context_menu(|ui| {
        if menu_button(ui, egui_phosphor::regular::PLUG, "New Connection").clicked() {
            db_manager.show_add_db = true;
            ui.close();
        }
    });
}

fn menu_button(ui: &mut egui::Ui, icon: &str, label: &str) -> egui::Response {
    ui.button(format!("{} {}", icon, label))
}

fn refresh_menu_button(ui: &mut egui::Ui) -> egui::Response {
    menu_button(ui, egui_phosphor::regular::ARROW_CLOCKWISE, "Refresh")
}

fn danger_menu_button(ui: &mut egui::Ui, icon: &str, label: &str) -> egui::Response {
    ui.button(egui::RichText::new(format!("{} {}", icon, label)).color(ui.visuals().error_fg_color))
}

fn empty_placeholder(ui: &mut egui::Ui, label: &str) {
    ui.label(egui::RichText::new(label).weak().small());
}
