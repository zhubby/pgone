use super::types::{DbTree, DialogType};
use super::operations;

pub(super) fn show_dialogs(tree: &mut DbTree, ui: &mut egui::Ui, db_manager: &mut crate::components::DbManager) {
    if let Some(dialog_type) = tree.dialog.clone() {
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
        let mut delete_cascade = tree.dialog_cascade;
        
        let center = ui.ctx().screen_rect().center();
        egui::Window::new(title)
            .open(&mut open)
            .default_pos(center)
            .pivot(egui::Align2::CENTER_CENTER)
            .show(ui.ctx(), |ui| {
                let dialog_input_ref = &mut tree.dialog_input;
                let dialog_ddl_ref = &mut tree.dialog_ddl;
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
                        if let Some(db) = tree.databases.iter().find(|d| d.name == *name) {
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
                        if let Some(schemas) = tree.schemas.get(database) {
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
                        if let Some(tables) = tree.tables.get(&key) {
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
            let dialog_input_clone = tree.dialog_input.clone();
            let dialog_ddl_clone = tree.dialog_ddl.clone();
            
            // Clear dialog first to avoid borrow conflicts
            tree.dialog = None;
            
            // Handle actions after dialog closes
            if should_create {
                match dialog_type {
                    DialogType::CreateDatabase => {
                        operations::create_database(tree, db_manager, &dialog_input_clone);
                    }
                    DialogType::CreateSchema { database } => {
                        operations::create_schema(tree, db_manager, &database, &dialog_input_clone);
                    }
                    DialogType::CreateTable { database, schema } => {
                        operations::create_table(tree, db_manager, &database, &schema, &dialog_ddl_clone);
                    }
                    _ => {}
                }
            } else if should_delete {
                tree.dialog_cascade = delete_cascade;
                match dialog_type {
                    DialogType::DeleteDatabase { name } => {
                        operations::delete_database(tree, db_manager, &name, delete_cascade);
                    }
                    DialogType::DeleteSchema { database, name } => {
                        operations::delete_schema(tree, db_manager, &database, &name, delete_cascade);
                    }
                    DialogType::DeleteTable { database, schema, name } => {
                        operations::delete_table(tree, db_manager, &database, &schema, &name, delete_cascade);
                    }
                    _ => {}
                }
            } else if should_rename {
                match dialog_type {
                    DialogType::RenameDatabase { old_name } => {
                        operations::rename_database(tree, db_manager, &old_name, &dialog_input_clone);
                    }
                    DialogType::RenameSchema { database, old_name } => {
                        operations::rename_schema(tree, db_manager, &database, &old_name, &dialog_input_clone);
                    }
                    DialogType::RenameTable { database, schema, old_name } => {
                        operations::rename_table(tree, db_manager, &database, &schema, &old_name, &dialog_input_clone);
                    }
                    _ => {}
                }
            }
        }
    }
}

