use crate::components::DbManager;
use crate::components::structures;
use crate::futures;
use pgone_sql::{DatabaseInfo, SchemaInfo, Session, TableInfo};
use poll_promise::Promise;
use sqlx::{Column, Row};
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
pub struct ExportWindow {
    // Selection state
    selected_database: Option<String>,
    selected_schema: Option<String>,
    selected_tables: HashSet<String>,

    // Export options
    export_ddl: bool,
    export_dml: bool,

    // File path
    file_path: Option<PathBuf>,

    // Data loading
    databases: Vec<DatabaseInfo>,
    databases_promise: Option<Promise<Result<Vec<DatabaseInfo>, String>>>,
    schemas: Vec<SchemaInfo>,
    schemas_promise: Option<Promise<Result<Vec<SchemaInfo>, String>>>,
    tables: Vec<TableInfo>,
    tables_promise: Option<Promise<Result<Vec<TableInfo>, String>>>,
    tables_loaded: bool, // Flag to indicate if tables are loaded (even if empty)

    // Export state
    export_promise: Option<Promise<Result<(), String>>>,
    export_cancel: Option<Arc<AtomicBool>>,
    export_progress: f32, // 0.0 - 1.0
    export_status: String,
    is_exporting: bool,
}

impl ExportWindow {
    pub fn ui(&mut self, ui: &mut egui::Ui, db_manager: &mut DbManager) {
        ui.vertical(|ui| {
            ui.set_width(500.0);

            // Database selection
            ui.horizontal(|ui| {
                ui.label("Database:");
                self.load_databases_if_needed(db_manager);

                // Check database loading state
                if let Some(ref promise) = self.databases_promise {
                    if let Some(result) = promise.ready() {
                        match result {
                            Ok(databases) => {
                                self.databases = databases.clone();
                            }
                            Err(e) => {
                                ui.colored_label(egui::Color32::RED, format!("Error: {}", e));
                            }
                        }
                        self.databases_promise = None;
                    } else {
                        ui.spinner();
                        ui.label("Loading...");
                    }
                }

                egui::ComboBox::from_id_salt("export_database")
                    .width(300.0)
                    .selected_text(
                        self.selected_database
                            .as_ref()
                            .map(|s| s.as_str())
                            .unwrap_or("Please select a database"),
                    )
                    .show_ui(ui, |ui| {
                        if self.databases.is_empty() && self.databases_promise.is_none() {
                            ui.label("No databases available");
                        } else {
                            for db in self.databases.iter() {
                                if ui
                                    .selectable_value(
                                        &mut self.selected_database,
                                        Some(db.name.clone()),
                                        &db.name,
                                    )
                                    .clicked()
                                {
                                    // Reset schema and tables
                                    self.selected_schema = None;
                                    self.selected_tables.clear();
                                    self.schemas.clear();
                                    self.tables.clear();
                                    self.tables_loaded = false; // Reset loading state
                                }
                            }
                        }
                    });
            });

            // Schema selection
            let db_name = self.selected_database.clone();
            if let Some(ref db_name) = db_name {
                ui.horizontal(|ui| {
                    ui.label("Schema:");
                    self.load_schemas_if_needed(db_manager, db_name);

                    if let Some(ref promise) = self.schemas_promise {
                        if let Some(result) = promise.ready() {
                            match result {
                                Ok(schemas) => {
                                    self.schemas = schemas.clone();
                                }
                                Err(e) => {
                                    ui.colored_label(egui::Color32::RED, format!("Error: {}", e));
                                }
                            }
                            self.schemas_promise = None;
                        } else {
                            ui.spinner();
                            ui.label("Loading...");
                        }
                    }

                    egui::ComboBox::from_id_salt("export_schema")
                        .width(300.0)
                        .selected_text(
                            self.selected_schema
                                .as_ref()
                                .map(|s| s.as_str())
                                .unwrap_or("Please select schema"),
                        )
                        .show_ui(ui, |ui| {
                            for schema in self.schemas.iter() {
                                if ui
                                    .selectable_value(
                                        &mut self.selected_schema,
                                        Some(schema.name.clone()),
                                        &schema.name,
                                    )
                                    .clicked()
                                {
                                    // Reset tables
                                    self.selected_tables.clear();
                                    self.tables.clear();
                                    self.tables_loaded = false; // Reset loading state
                                }
                            }
                        });
                });
            }

            // Table selection (multiple)
            let schema_name = self.selected_schema.clone();
            let db_name = self.selected_database.clone();
            if let (Some(ref schema_name), Some(ref db_name)) = (schema_name, db_name) {
                ui.horizontal(|ui| {
                    ui.label("Tables:");
                    self.load_tables_if_needed(db_manager, db_name, schema_name);

                    if let Some(ref promise) = self.tables_promise {
                        if let Some(result) = promise.ready() {
                            match result {
                                Ok(tables) => {
                                    self.tables = tables.clone();
                                    self.tables_loaded = true; // Mark as loaded
                                }
                                Err(e) => {
                                    ui.colored_label(egui::Color32::RED, format!("Error: {}", e));
                                    self.tables_loaded = true; // Mark as loaded even on error to avoid repeated requests
                                }
                            }
                            self.tables_promise = None;
                        } else {
                            ui.spinner();
                            ui.label("Loading...");
                        }
                    }
                });

                // Table multiple selection list
                egui::ScrollArea::vertical()
                    .max_height(150.0)
                    .show(ui, |ui| {
                        if self.tables_loaded && self.tables.is_empty() {
                            ui.label("No tables in this schema");
                        } else {
                            for table in self.tables.iter() {
                                let mut is_selected = self.selected_tables.contains(&table.name);
                                if ui.checkbox(&mut is_selected, &table.name).changed() {
                                    if is_selected {
                                        self.selected_tables.insert(table.name.clone());
                                    } else {
                                        self.selected_tables.remove(&table.name);
                                    }
                                }
                            }
                        }
                    });
            }

            ui.separator();

            // Export type selection
            ui.horizontal(|ui| {
                ui.label("Export type:");
                ui.checkbox(&mut self.export_ddl, "DDL");
                ui.checkbox(&mut self.export_dml, "DML");
            });

            ui.separator();

            // File path selection
            ui.horizontal(|ui| {
                ui.label("Save path:");
                let mut path_text = self
                    .file_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                if ui.text_edit_singleline(&mut path_text).changed() {
                    if !path_text.is_empty() {
                        self.file_path = Some(PathBuf::from(path_text));
                    } else {
                        self.file_path = None;
                    }
                }

                if ui.button("Browse...").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_file_name("export.sql")
                        .add_filter("SQL Files", &["sql"])
                        .add_filter("All Files", &["*"])
                        .save_file()
                    {
                        self.file_path = Some(path);
                    }
                }
            });

            ui.separator();

            // Progress bar and status (show during export or when completed)
            if self.is_exporting || self.export_progress > 0.0 {
                ui.horizontal(|ui| {
                    if self.is_exporting {
                        ui.spinner();
                    }
                    ui.label(&self.export_status);
                });
                ui.add(egui::ProgressBar::new(self.export_progress));
            }

            // Buttons
            ui.horizontal(|ui| {
                let can_export = self.selected_database.is_some()
                    && self.selected_schema.is_some()
                    && !self.selected_tables.is_empty()
                    && (self.export_ddl || self.export_dml)
                    && self.file_path.is_some()
                    && !self.is_exporting;

                if ui
                    .add_enabled(can_export, egui::Button::new("Export"))
                    .clicked()
                {
                    self.start_export(db_manager);
                }

                if ui
                    .button(if self.is_exporting {
                        "Cancel"
                    } else {
                        "Close"
                    })
                    .clicked()
                {
                    if !self.is_exporting {
                        // If not exporting, reset all state
                        *self = ExportWindow::default();
                    } else {
                        self.cancel_export();
                    }
                }
            });
        });
    }

    fn load_databases_if_needed(&mut self, db_manager: &mut DbManager) {
        if !self.databases.is_empty() || self.databases_promise.is_some() {
            return;
        }

        let Some(_db_id) = db_manager.active_db_config_id.clone() else {
            return;
        };

        let Some(dsn) = db_manager.active_dsn() else {
            return;
        };

        let pools = db_manager.pools.clone();
        let dsn_clone =
            crate::components::structures::utils::replace_database_in_dsn(&dsn, "postgres")
                .unwrap_or(dsn);
        let (sender, promise) = Promise::new();
        self.databases_promise = Some(promise);

        futures::spawn(async move {
            let result: Result<Vec<DatabaseInfo>, String> = async {
                let pool = pools.get_or_create_pool(&dsn_clone).await?;
                let session = Session::from_pool(pool);

                session
                    .list_databases()
                    .await
                    .map_err(|e| format!("Failed to list databases: {}", e))
            }
            .await;

            sender.send(result);
        });
    }

    fn load_schemas_if_needed(&mut self, db_manager: &mut DbManager, database: &str) {
        if !self.schemas.is_empty() || self.schemas_promise.is_some() {
            return;
        }

        let Some(_db_id) = db_manager.active_db_config_id.clone() else {
            return;
        };

        let Some(dsn) = db_manager.dsn_for_database(database) else {
            return;
        };

        let pools = db_manager.pools.clone();
        let dsn_clone = dsn.clone();
        let (sender, promise) = Promise::new();
        self.schemas_promise = Some(promise);

        futures::spawn(async move {
            let result: Result<Vec<SchemaInfo>, String> = async {
                let pool = pools.get_or_create_pool(&dsn_clone).await?;
                let session = Session::from_pool(pool);

                session
                    .list_schemas()
                    .await
                    .map_err(|e| format!("Failed to list schemas: {}", e))
            }
            .await;

            sender.send(result);
        });
    }

    fn load_tables_if_needed(&mut self, db_manager: &mut DbManager, database: &str, schema: &str) {
        // If already loaded or loading, don't load again
        if self.tables_loaded || self.tables_promise.is_some() {
            return;
        }

        let Some(_db_id) = db_manager.active_db_config_id.clone() else {
            return;
        };

        let Some(dsn) = db_manager.dsn_for_database(database) else {
            return;
        };

        let pools = db_manager.pools.clone();
        let dsn_clone = dsn.clone();
        let schema_clone = schema.to_string();
        let (sender, promise) = Promise::new();
        self.tables_promise = Some(promise);

        futures::spawn(async move {
            let result: Result<Vec<TableInfo>, String> = async {
                let pool = pools.get_or_create_pool(&dsn_clone).await?;
                let session = Session::from_pool(pool);

                session
                    .list_tables(Some(&schema_clone))
                    .await
                    .map_err(|e| format!("Failed to list tables: {}", e))
            }
            .await;

            sender.send(result);
        });
    }

    fn start_export(&mut self, db_manager: &mut DbManager) {
        if self.is_exporting {
            return;
        }

        let Some(db_name) = self.selected_database.clone() else {
            return;
        };
        let Some(schema_name) = self.selected_schema.clone() else {
            return;
        };
        let Some(file_path) = self.file_path.clone() else {
            return;
        };

        if self.selected_tables.is_empty() {
            return;
        }

        let Some(_db_id) = db_manager.active_db_config_id.clone() else {
            return;
        };

        let Some(dsn) = db_manager.dsn_for_database(&db_name) else {
            return;
        };

        let pools = db_manager.pools.clone();
        let dsn_clone = dsn.clone();
        let schema_clone = schema_name.clone();
        let db_name_clone = db_name.clone();
        let tables_clone: Vec<String> = self.selected_tables.iter().cloned().collect();
        let export_ddl = self.export_ddl;
        let export_dml = self.export_dml;
        let file_path_clone = file_path.clone();

        self.is_exporting = true;
        self.export_progress = 0.0;
        self.export_status = "Preparing to export...".to_string();

        let (sender, promise) = Promise::new();
        let cancel_token = Arc::new(AtomicBool::new(false));
        let worker_cancel_token = Arc::clone(&cancel_token);
        self.export_cancel = Some(cancel_token);
        self.export_promise = Some(promise);

        futures::spawn(async move {
            let result: Result<(), String> = async {
                // Create file
                let mut file = OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .write(true)
                    .open(&file_path_clone)
                    .map_err(|e| format!("Failed to create file: {}", e))?;

                // Write file header
                writeln!(file, "-- Export generated by PGone")
                    .map_err(|e| format!("Failed to write: {}", e))?;
                writeln!(file, "-- Database: {}", db_name_clone)
                    .map_err(|e| format!("Failed to write: {}", e))?;
                writeln!(file, "-- Schema: {}", schema_clone)
                    .map_err(|e| format!("Failed to write: {}", e))?;
                writeln!(file, "-- Tables: {}", tables_clone.join(", "))
                    .map_err(|e| format!("Failed to write: {}", e))?;
                writeln!(file, "").map_err(|e| format!("Failed to write: {}", e))?;

                let pool = pools.get_or_create_pool(&dsn_clone).await?;
                let session = Session::from_pool(pool.clone());

                let total_tables = tables_clone.len();

                for (table_idx, table_name) in tables_clone.iter().enumerate() {
                    if worker_cancel_token.load(Ordering::Relaxed) {
                        return Err("Export canceled".to_string());
                    }
                    // Update progress (note: cannot directly update UI here, need to check promise in UI thread)
                    let _progress = (table_idx as f32) / (total_tables as f32);

                    // Export DDL
                    if export_ddl {
                        match session.get_table_detail(&schema_clone, table_name).await {
                            Ok(table_detail) => {
                                match session.list_table_indexes(&schema_clone, table_name).await {
                                    Ok(indexes) => {
                                        let ddl = structures::utils::generate_table_ddl(
                                            &schema_clone,
                                            table_name,
                                            &table_detail,
                                            &indexes,
                                        );
                                        writeln!(
                                            file,
                                            "-- DDL for table {}.{}",
                                            schema_clone, table_name
                                        )
                                        .map_err(|e| format!("Failed to write: {}", e))?;
                                        writeln!(file, "{}", ddl)
                                            .map_err(|e| format!("Failed to write: {}", e))?;
                                        writeln!(file, "")
                                            .map_err(|e| format!("Failed to write: {}", e))?;
                                    }
                                    Err(e) => {
                                        return Err(format!(
                                            "Failed to get indexes for {}: {}",
                                            table_name, e
                                        ));
                                    }
                                }
                            }
                            Err(e) => {
                                return Err(format!(
                                    "Failed to get table detail for {}: {}",
                                    table_name, e
                                ));
                            }
                        }
                    }

                    // Export DML
                    if export_dml {
                        writeln!(file, "-- DML for table {}.{}", schema_clone, table_name)
                            .map_err(|e| format!("Failed to write: {}", e))?;

                        // Paginated data query
                        const PAGE_SIZE: usize = 100;
                        let mut offset = 0;
                        let mut has_more = true;
                        let mut columns_loaded = false;
                        let mut column_names = Vec::new();

                        while has_more {
                            if worker_cancel_token.load(Ordering::Relaxed) {
                                return Err("Export canceled".to_string());
                            }
                            // Build query SQL
                            let query = format!(
                                "SELECT * FROM {}.{} LIMIT {} OFFSET {}",
                                structures::utils::quote_ident(&schema_clone),
                                structures::utils::quote_ident(table_name),
                                PAGE_SIZE,
                                offset
                            );

                            let rows = sqlx::query(&query)
                                .fetch_all(&pool)
                                .await
                                .map_err(|e| format!("Failed to query data: {}", e))?;

                            if rows.is_empty() {
                                has_more = false;
                            } else {
                                // Get column names (only on first time)
                                if !columns_loaded {
                                    if let Some(first_row) = rows.first() {
                                        for col in first_row.columns() {
                                            column_names.push(col.name().to_string());
                                        }
                                        columns_loaded = true;
                                    }
                                }

                                // Convert row data
                                let mut row_data = Vec::new();
                                for row in rows {
                                    let mut row_values = Vec::new();
                                    for i in 0..column_names.len() {
                                        // Format cell value
                                        let value = if row.try_get::<String, _>(i).is_ok() {
                                            row.get::<String, _>(i)
                                        } else if row.try_get::<i64, _>(i).is_ok() {
                                            row.get::<i64, _>(i).to_string()
                                        } else if row.try_get::<f64, _>(i).is_ok() {
                                            row.get::<f64, _>(i).to_string()
                                        } else if row.try_get::<bool, _>(i).is_ok() {
                                            row.get::<bool, _>(i).to_string()
                                        } else {
                                            // Try to get as string, use NULL if it fails
                                            row.try_get::<Option<String>, _>(i)
                                                .ok()
                                                .flatten()
                                                .unwrap_or_else(|| "NULL".to_string())
                                        };
                                        row_values.push(value);
                                    }
                                    row_data.push(row_values);
                                }

                                // Generate DML
                                let dml = structures::utils::generate_table_dml(
                                    &schema_clone,
                                    table_name,
                                    &column_names,
                                    &row_data,
                                );

                                if !dml.is_empty() {
                                    file.write_all(dml.as_bytes())
                                        .map_err(|e| format!("Failed to write: {}", e))?;
                                    writeln!(file, "")
                                        .map_err(|e| format!("Failed to write: {}", e))?;
                                }

                                if row_data.len() < PAGE_SIZE {
                                    has_more = false;
                                } else {
                                    offset += PAGE_SIZE;
                                }
                            }
                        }

                        writeln!(file, "").map_err(|e| format!("Failed to write: {}", e))?;
                    }
                }

                Ok(())
            }
            .await;

            sender.send(result);
        });
    }

    pub fn check_export_progress(&mut self) {
        if let Some(ref promise) = self.export_promise {
            if let Some(result) = promise.ready() {
                match result {
                    Ok(()) => {
                        self.is_exporting = false;
                        self.export_progress = 1.0;
                        self.export_status = "Export completed!".to_string();
                        self.export_promise = None;
                        self.export_cancel = None;
                    }
                    Err(e) => {
                        self.is_exporting = false;
                        self.export_status = format!("Export failed: {}", e);
                        self.export_promise = None;
                        self.export_cancel = None;
                    }
                }
            } else {
                // Update progress (simplified version, should actually get from async task)
                if self.export_progress < 0.9 {
                    self.export_progress += 0.01;
                }
            }
        }
    }

    pub fn is_exporting(&self) -> bool {
        self.is_exporting
    }

    pub fn cancel_export(&mut self) {
        if let Some(cancel_token) = &self.export_cancel {
            cancel_token.store(true, Ordering::Relaxed);
        }
        self.is_exporting = false;
        self.export_progress = 0.0;
        self.export_status = "Export canceled.".to_string();
        self.export_promise = None;
        self.export_cancel = None;
    }
}
