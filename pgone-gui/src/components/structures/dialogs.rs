use super::design;
use super::operations;
use super::types::{DbTree, DialogType, EditableColumn};

fn column_names() -> &'static [&'static str] {
    &[
        "Name",
        "Type",
        "Length",
        "Precision",
        "Scale",
        "Nullable",
        "Default",
        "Comment",
    ]
}

fn show_editable_column_row(ui: &mut egui::Ui, row: &mut EditableColumn, row_index: usize) {
    let response = ui.text_edit_singleline(&mut row.name);
    if response.changed() && row.original_name.is_none() && !row.is_new {
        row.original_name = Some(row.name.clone());
    }

    let types = [
        "VARCHAR",
        "CHAR",
        "TEXT",
        "INTEGER",
        "BIGINT",
        "SMALLINT",
        "NUMERIC",
        "DECIMAL",
        "REAL",
        "DOUBLE PRECISION",
        "BOOLEAN",
        "DATE",
        "TIME",
        "TIMESTAMP",
        "TIMESTAMPTZ",
        "JSON",
        "JSONB",
    ];
    let mut selected = types
        .iter()
        .position(|value| row.data_type.to_uppercase().starts_with(value))
        .unwrap_or(0);
    egui::ComboBox::from_id_salt(("design_type", row_index))
        .selected_text(types[selected])
        .show_ui(ui, |ui| {
            for (index, value) in types.iter().enumerate() {
                if ui.selectable_label(index == selected, *value).clicked() {
                    selected = index;
                    row.data_type = value.to_string();
                }
            }
        });

    edit_optional_i32(ui, &mut row.character_maximum_length);
    edit_optional_i32(ui, &mut row.numeric_precision);
    edit_optional_i32(ui, &mut row.numeric_scale);
    ui.checkbox(&mut row.nullable, "");
    edit_optional_string(ui, &mut row.default);
    edit_optional_string(ui, &mut row.comment);
    ui.end_row();
}

fn edit_optional_i32(ui: &mut egui::Ui, value: &mut Option<i32>) {
    let mut text = value.map(|value| value.to_string()).unwrap_or_default();
    if ui.text_edit_singleline(&mut text).changed() {
        *value = if text.trim().is_empty() {
            None
        } else {
            text.trim().parse().ok()
        };
    }
}

fn edit_optional_string(ui: &mut egui::Ui, value: &mut Option<String>) {
    let mut text = value.clone().unwrap_or_default();
    if ui.text_edit_singleline(&mut text).changed() {
        *value = if text.is_empty() { None } else { Some(text) };
    }
}

pub(super) fn show_dialogs(
    tree: &mut DbTree,
    ui: &mut egui::Ui,
    db_manager: &mut crate::components::DbManager,
) {
    if let Some(dialog_type) = tree.dialog.clone() {
        let title = match &dialog_type {
            DialogType::CreateDatabase => "Create Database",
            DialogType::CreateSchema { .. } => "Create Schema",
            DialogType::CreateTable { .. } => "Create Table",
            DialogType::CreateView { .. } => "Create View",
            DialogType::CreateMaterializedView { .. } => "Create Materialized View",
            DialogType::CreateFunction { .. } => "Create Function",
            DialogType::DeleteDatabase { .. } => "Delete Database",
            DialogType::DeleteSchema { .. } => "Delete Schema",
            DialogType::DeleteTable { .. } => "Delete Table",
            DialogType::RenameDatabase { .. } => "Rename Database",
            DialogType::RenameSchema { .. } => "Rename Schema",
            DialogType::RenameTable { .. } => "Rename Table",
            DialogType::PropertiesDatabase { .. } => "Database Properties",
            DialogType::PropertiesSchema { .. } => "Schema Properties",
            DialogType::PropertiesTable { .. } => "Table Properties",
            DialogType::PropertiesView { .. } => "View Properties",
            DialogType::PropertiesMaterializedView { .. } => "Materialized View Properties",
            DialogType::PropertiesFunction { .. } => "Function Properties",
            DialogType::DesignTable { .. } => "Design Table",
            DialogType::ShowDdl { .. } => "Show DDL",
            DialogType::DropTable { .. } => "Drop Table",
        };

        let mut open = true;
        let mut should_create = false;
        let mut should_delete = false;
        let mut should_rename = false;
        let mut should_save_design = false;
        let mut should_close = false;
        let mut should_drop = false;
        let mut delete_cascade = tree.dialog_cascade;

        let center = ui.ctx().content_rect().center();
        let mut window = egui::Window::new(title)
            .id(egui::Id::new(("structure_dialog", title)))
            .open(&mut open)
            .default_pos(center)
            .pivot(egui::Align2::CENTER_CENTER)
            .collapsible(false); // All dialogs are not collapsible

        // Set appropriate size for DesignTable dialog
        if matches!(dialog_type, DialogType::DesignTable { .. }) {
            // Window height set to half the screen height
            let screen_height = ui.ctx().content_rect().height();
            let window_height = screen_height * 0.5;

            window = window
                .default_size([900.0, window_height])
                .resizable(true)
                .max_size([1200.0, screen_height * 0.8])
                .min_size([600.0, 300.0]);
        }

        // Set appropriate size for ShowDdl dialog
        if matches!(dialog_type, DialogType::ShowDdl { .. }) {
            window = window
                .default_size([800.0, 300.0])
                .resizable(true)
                .max_size([1200.0, 600.0])
                .min_size([600.0, 250.0]);
        }

        window.show(ui.ctx(), |ui| {
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
                DialogType::CreateTable {
                    database: _,
                    schema: _,
                } => {
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
                DialogType::CreateView {
                    database: _,
                    schema: _,
                } => {
                    ui.label("View DDL:");
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
                DialogType::CreateMaterializedView {
                    database: _,
                    schema: _,
                } => {
                    ui.label("Materialized View DDL:");
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
                DialogType::CreateFunction {
                    database: _,
                    schema: _,
                } => {
                    ui.label("Function DDL:");
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
                    ui.label(format!(
                        "Are you sure you want to delete database '{}'?",
                        name
                    ));
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
                    ui.label(format!(
                        "Are you sure you want to delete schema '{}'?",
                        name
                    ));
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
                DialogType::DeleteTable {
                    database: _,
                    schema,
                    name,
                } => {
                    ui.label(format!(
                        "Are you sure you want to delete table '{}.{}'?",
                        schema, name
                    ));
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
                DialogType::RenameSchema {
                    database: _,
                    old_name: _,
                } => {
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
                DialogType::RenameTable {
                    database: _,
                    schema: _,
                    old_name: _,
                } => {
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
                DialogType::PropertiesTable {
                    database,
                    schema,
                    name,
                } => {
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
                DialogType::PropertiesView {
                    database,
                    schema,
                    name,
                } => {
                    // Show view properties (read-only)
                    let key = format!("{}.{}", database, schema);
                    if let Some(views) = tree.views.get(&key) {
                        if let Some(view) = views.iter().find(|v| v.name == *name) {
                            ui.label(format!("Name: {}", view.name));
                            ui.label(format!("Schema: {}", view.schema));
                            ui.label(format!("Owner: {}", view.owner));
                            if let Some(def) = &view.definition {
                                ui.separator();
                                ui.label("Definition:");
                                let mut def_text = def.clone();
                                ui.text_edit_multiline(&mut def_text);
                            }
                            if let Some(desc) = &view.description {
                                ui.separator();
                                ui.label(format!("Description: {}", desc));
                            }
                        }
                    }
                    if ui.button("Close").clicked() {
                        // Will be handled by open = false
                    }
                }
                DialogType::PropertiesMaterializedView {
                    database,
                    schema,
                    name,
                } => {
                    // Show materialized view properties (read-only)
                    let key = format!("{}.{}", database, schema);
                    if let Some(materialized_views) = tree.materialized_views.get(&key) {
                        if let Some(matview) = materialized_views.iter().find(|mv| mv.name == *name)
                        {
                            ui.label(format!("Name: {}", matview.name));
                            ui.label(format!("Schema: {}", matview.schema));
                            ui.label(format!("Owner: {}", matview.owner));
                            if let Some(def) = &matview.definition {
                                ui.separator();
                                ui.label("Definition:");
                                let mut def_text = def.clone();
                                ui.text_edit_multiline(&mut def_text);
                            }
                            if let Some(desc) = &matview.description {
                                ui.separator();
                                ui.label(format!("Description: {}", desc));
                            }
                        }
                    }
                    if ui.button("Close").clicked() {
                        // Will be handled by open = false
                    }
                }
                DialogType::PropertiesFunction {
                    database,
                    schema,
                    name,
                } => {
                    // Show function properties (read-only)
                    let key = format!("{}.{}", database, schema);
                    if let Some(functions) = tree.functions.get(&key) {
                        if let Some(function) = functions.iter().find(|f| f.name == *name) {
                            ui.label(format!("Name: {}", function.name));
                            ui.label(format!("Schema: {}", function.schema));
                            ui.label(format!("Owner: {}", function.owner));
                            if let Some(lang) = &function.language {
                                ui.label(format!("Language: {}", lang));
                            }
                            if let Some(ret_type) = &function.return_type {
                                ui.label(format!("Return Type: {}", ret_type));
                            }
                            if let Some(def) = &function.definition {
                                ui.separator();
                                ui.label("Definition:");
                                let mut def_text = def.clone();
                                ui.text_edit_multiline(&mut def_text);
                            }
                            if let Some(desc) = &function.description {
                                ui.separator();
                                ui.label(format!("Description: {}", desc));
                            }
                        }
                    }
                    if ui.button("Close").clicked() {
                        // Will be handled by open = false
                    }
                }
                DialogType::DesignTable {
                    database,
                    schema,
                    name,
                } => {
                    // Check if the current dialog's table matches the loaded table
                    let current_table = (database.clone(), schema.clone(), name.clone());
                    if let Some(ref loaded_table) = tree.design_table_loaded {
                        if *loaded_table != current_table {
                            // Table name mismatch, clear data and trigger reload
                            tree.design_table_detail = None;
                            tree.design_table_columns.clear();
                            tree.design_table_promise = None;
                            tree.design_table_loaded = None;
                            // Trigger reload
                            use super::loading;
                            loading::load_table_detail_for_design(
                                tree, db_manager, database, schema, name,
                            );
                            ui.label("Loading table structure...");
                            return;
                        }
                    }

                    // Check async-loaded table structure details
                    if let Some(ref promise) = tree.design_table_promise {
                        if let Some(result) = promise.ready() {
                            match result {
                                Ok(detail) => {
                                    // Initialize editable column data
                                    tree.design_table_detail = Some(detail.clone());
                                    tree.design_table_columns = detail
                                        .columns
                                        .iter()
                                        .map(|col| EditableColumn {
                                            name: col.name.clone(),
                                            data_type: col.data_type.clone(),
                                            character_maximum_length: col.character_maximum_length,
                                            numeric_precision: col.numeric_precision,
                                            numeric_scale: col.numeric_scale,
                                            nullable: col.nullable,
                                            default: col.default.clone(),
                                            comment: col.comment.clone(),
                                            is_new: false,
                                            is_deleted: false,
                                            original_name: Some(col.name.clone()),
                                        })
                                        .collect();
                                }
                                Err(e) => {
                                    ui.colored_label(egui::Color32::RED, format!("Error: {}", e));
                                    tree.design_table_promise = None;
                                    tree.design_table_loaded = None;
                                }
                            }
                        } else {
                            ui.label("Loading table structure...");
                            return;
                        }
                    } else if tree.design_table_detail.is_none() {
                        // No promise and no loaded data, trigger loading
                        use super::loading;
                        loading::load_table_detail_for_design(
                            tree, db_manager, database, schema, name,
                        );
                        ui.label("Loading table structure...");
                        return;
                    }

                    // Display table design interface
                    if tree.design_table_columns.is_empty() {
                        ui.label("No columns to display");
                    } else {
                        egui::ScrollArea::both()
                            .auto_shrink([false; 2])
                            .show(ui, |ui| {
                                egui::Grid::new("design_table_columns")
                                    .striped(true)
                                    .num_columns(column_names().len())
                                    .show(ui, |ui| {
                                        for name in column_names() {
                                            ui.strong(*name);
                                        }
                                        ui.end_row();

                                        for (row_index, column) in
                                            tree.design_table_columns.iter_mut().enumerate()
                                        {
                                            show_editable_column_row(ui, column, row_index);
                                        }
                                    });
                            });

                        // Bottom-right buttons
                        ui.separator();
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Cancel").clicked() {
                                // Will be handled by open = false outside
                            }
                            ui.add_space(8.0);
                            if ui.button("Save").clicked() {
                                should_save_design = true;
                            }
                        });
                    }
                }
                DialogType::ShowDdl {
                    database: _,
                    schema: _,
                    name: _,
                } => {
                    // Check async-loaded DDL
                    if let Some(ref promise) = tree.ddl_promise {
                        if let Some(result) = promise.ready() {
                            match result {
                                Ok(_) => {
                                    // DDL has been loaded into dialog_ddl_content
                                }
                                Err(e) => {
                                    ui.colored_label(egui::Color32::RED, format!("Error: {}", e));
                                    tree.ddl_promise = None;
                                }
                            }
                        } else {
                            ui.label("Loading DDL...");
                            return;
                        }
                    }

                    // Display DDL content with SQL highlighting
                    let ddl_content = tree.dialog_ddl_content.clone();
                    let available_height = ui.available_height() - 60.0; // Reserve space for buttons

                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.add_space(5.0);

                                let mut ddl_text = ddl_content.clone();
                                let ddl_text_ref = &mut ddl_text;
                                let ddl_for_highlight = ddl_content.clone();

                                ui.add_sized(
                                    egui::Vec2::new(
                                        ui.available_width() - 5.0,
                                        available_height.max(200.0),
                                    ),
                                    egui::TextEdit::multiline(ddl_text_ref)
                                        .desired_rows((available_height.max(200.0) / 20.0) as usize)
                                        .interactive(false) // Set as read-only
                                        .layouter(&mut move |ui, _text, wrap_width| {
                                            let mut job = crate::sql::highlight_sql(
                                                &ddl_for_highlight,
                                                ui.visuals(),
                                            );
                                            job.wrap.max_width = wrap_width;
                                            ui.fonts_mut(|f| f.layout_job(job))
                                        }),
                                );
                            });
                        });

                    // Close and copy buttons
                    ui.separator();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            should_close = true;
                        }
                        ui.add_space(8.0);
                        if ui.button("Copy").clicked() {
                            // Copy DDL content to clipboard
                            let ddl_to_copy = tree.dialog_ddl_content.clone();
                            ui.ctx().copy_text(ddl_to_copy);
                        }
                    });
                }
                DialogType::DropTable {
                    database: _,
                    schema,
                    name,
                } => {
                    ui.colored_label(
                        egui::Color32::RED,
                        format!(
                            "Warning: This operation will clear all data in table '{}.{}'. This operation cannot be undone.",
                            schema, name
                        ),
                    );
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Drop").clicked() {
                            should_drop = true;
                        }
                        if ui.button("Cancel").clicked() {
                            // Will be handled by open = false
                        }
                    });
                }
            }
        });

        // Close window if action was triggered
        if should_create
            || should_delete
            || should_rename
            || should_save_design
            || should_close
            || should_drop
        {
            open = false; // Close window to trigger action execution
        }

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
                        operations::create_table(
                            tree,
                            db_manager,
                            &database,
                            &schema,
                            &dialog_ddl_clone,
                        );
                    }
                    DialogType::CreateView { database, schema } => {
                        operations::create_view(
                            tree,
                            db_manager,
                            &database,
                            &schema,
                            &dialog_ddl_clone,
                        );
                    }
                    DialogType::CreateMaterializedView { database, schema } => {
                        operations::create_materialized_view(
                            tree,
                            db_manager,
                            &database,
                            &schema,
                            &dialog_ddl_clone,
                        );
                    }
                    DialogType::CreateFunction { database, schema } => {
                        operations::create_function(
                            tree,
                            db_manager,
                            &database,
                            &schema,
                            &dialog_ddl_clone,
                        );
                    }
                    DialogType::DesignTable { .. }
                    | DialogType::DeleteDatabase { .. }
                    | DialogType::DeleteSchema { .. }
                    | DialogType::DeleteTable { .. }
                    | DialogType::RenameDatabase { .. }
                    | DialogType::RenameSchema { .. }
                    | DialogType::RenameTable { .. }
                    | DialogType::PropertiesDatabase { .. }
                    | DialogType::PropertiesSchema { .. }
                    | DialogType::PropertiesTable { .. }
                    | DialogType::PropertiesView { .. }
                    | DialogType::PropertiesMaterializedView { .. }
                    | DialogType::PropertiesFunction { .. }
                    | DialogType::ShowDdl { .. }
                    | DialogType::DropTable { .. } => {}
                }
            } else if should_delete {
                tree.dialog_cascade = delete_cascade;
                match dialog_type {
                    DialogType::DeleteDatabase { name } => {
                        operations::delete_database(tree, db_manager, &name, delete_cascade);
                    }
                    DialogType::DeleteSchema { database, name } => {
                        operations::delete_schema(
                            tree,
                            db_manager,
                            &database,
                            &name,
                            delete_cascade,
                        );
                    }
                    DialogType::DeleteTable {
                        database,
                        schema,
                        name,
                    } => {
                        operations::delete_table(
                            tree,
                            db_manager,
                            &database,
                            &schema,
                            &name,
                            delete_cascade,
                        );
                    }
                    DialogType::DesignTable { .. }
                    | DialogType::CreateDatabase
                    | DialogType::CreateSchema { .. }
                    | DialogType::CreateTable { .. }
                    | DialogType::CreateView { .. }
                    | DialogType::CreateMaterializedView { .. }
                    | DialogType::CreateFunction { .. }
                    | DialogType::RenameDatabase { .. }
                    | DialogType::RenameSchema { .. }
                    | DialogType::RenameTable { .. }
                    | DialogType::PropertiesDatabase { .. }
                    | DialogType::PropertiesSchema { .. }
                    | DialogType::PropertiesTable { .. }
                    | DialogType::PropertiesView { .. }
                    | DialogType::PropertiesMaterializedView { .. }
                    | DialogType::PropertiesFunction { .. }
                    | DialogType::ShowDdl { .. }
                    | DialogType::DropTable { .. } => {}
                }
            } else if should_rename {
                match dialog_type {
                    DialogType::RenameDatabase { old_name } => {
                        operations::rename_database(
                            tree,
                            db_manager,
                            &old_name,
                            &dialog_input_clone,
                        );
                    }
                    DialogType::RenameSchema { database, old_name } => {
                        operations::rename_schema(
                            tree,
                            db_manager,
                            &database,
                            &old_name,
                            &dialog_input_clone,
                        );
                    }
                    DialogType::RenameTable {
                        database,
                        schema,
                        old_name,
                    } => {
                        operations::rename_table(
                            tree,
                            db_manager,
                            &database,
                            &schema,
                            &old_name,
                            &dialog_input_clone,
                        );
                    }
                    DialogType::DesignTable { .. }
                    | DialogType::CreateDatabase
                    | DialogType::CreateSchema { .. }
                    | DialogType::CreateTable { .. }
                    | DialogType::CreateView { .. }
                    | DialogType::CreateMaterializedView { .. }
                    | DialogType::CreateFunction { .. }
                    | DialogType::DeleteDatabase { .. }
                    | DialogType::DeleteSchema { .. }
                    | DialogType::DeleteTable { .. }
                    | DialogType::PropertiesDatabase { .. }
                    | DialogType::PropertiesSchema { .. }
                    | DialogType::PropertiesTable { .. }
                    | DialogType::PropertiesView { .. }
                    | DialogType::PropertiesMaterializedView { .. }
                    | DialogType::PropertiesFunction { .. }
                    | DialogType::ShowDdl { .. }
                    | DialogType::DropTable { .. } => {}
                }
            } else if should_save_design {
                match dialog_type {
                    DialogType::DesignTable {
                        database,
                        schema,
                        name,
                    } => {
                        if let Some(ref original_detail) = tree.design_table_detail {
                            let statements = design::generate_alter_statements(
                                &schema,
                                &name,
                                original_detail,
                                &tree.design_table_columns,
                            );
                            if !statements.is_empty() {
                                operations::design_table(
                                    tree,
                                    db_manager,
                                    &database,
                                    &schema,
                                    &name,
                                    &statements,
                                );
                            } else {
                                // Clear design state even if there are no statements
                                tree.design_table_detail = None;
                                tree.design_table_columns.clear();
                            }
                        }
                        // Clear loaded table record so it reloads on next open
                        tree.design_table_loaded = None;
                        tree.design_table_promise = None;
                        // Close dialog
                        tree.dialog = None;
                    }
                    DialogType::CreateDatabase
                    | DialogType::CreateSchema { .. }
                    | DialogType::CreateTable { .. }
                    | DialogType::CreateView { .. }
                    | DialogType::CreateMaterializedView { .. }
                    | DialogType::CreateFunction { .. }
                    | DialogType::DeleteDatabase { .. }
                    | DialogType::DeleteSchema { .. }
                    | DialogType::DeleteTable { .. }
                    | DialogType::RenameDatabase { .. }
                    | DialogType::RenameSchema { .. }
                    | DialogType::RenameTable { .. }
                    | DialogType::PropertiesDatabase { .. }
                    | DialogType::PropertiesSchema { .. }
                    | DialogType::PropertiesTable { .. }
                    | DialogType::PropertiesView { .. }
                    | DialogType::PropertiesMaterializedView { .. }
                    | DialogType::PropertiesFunction { .. }
                    | DialogType::ShowDdl { .. }
                    | DialogType::DropTable { .. } => {}
                }
            } else if should_drop {
                match dialog_type {
                    DialogType::DropTable {
                        database,
                        schema,
                        name,
                    } => {
                        operations::drop_table(tree, db_manager, &database, &schema, &name);
                    }
                    DialogType::CreateDatabase
                    | DialogType::CreateSchema { .. }
                    | DialogType::CreateTable { .. }
                    | DialogType::CreateView { .. }
                    | DialogType::CreateMaterializedView { .. }
                    | DialogType::CreateFunction { .. }
                    | DialogType::DeleteDatabase { .. }
                    | DialogType::DeleteSchema { .. }
                    | DialogType::DeleteTable { .. }
                    | DialogType::RenameDatabase { .. }
                    | DialogType::RenameSchema { .. }
                    | DialogType::RenameTable { .. }
                    | DialogType::PropertiesDatabase { .. }
                    | DialogType::PropertiesSchema { .. }
                    | DialogType::PropertiesTable { .. }
                    | DialogType::PropertiesView { .. }
                    | DialogType::PropertiesMaterializedView { .. }
                    | DialogType::PropertiesFunction { .. }
                    | DialogType::DesignTable { .. }
                    | DialogType::ShowDdl { .. } => {}
                }
            }
        }
    }
}
