use crate::components::DbManager;
use crate::components::structures;
use crate::futures;
use pgone_sql::{DatabaseInfo, SchemaInfo, Session, TableInfo};
use poll_promise::Promise;
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

#[derive(Default)]
pub struct ExportWindow {
    // 选择状态
    selected_database: Option<String>,
    selected_schema: Option<String>,
    selected_tables: HashSet<String>,

    // 导出选项
    export_ddl: bool,
    export_dml: bool,

    // 文件路径
    file_path: Option<PathBuf>,

    // 数据加载
    databases: Vec<DatabaseInfo>,
    databases_promise: Option<Promise<Result<Vec<DatabaseInfo>, String>>>,
    schemas: Vec<SchemaInfo>,
    schemas_promise: Option<Promise<Result<Vec<SchemaInfo>, String>>>,
    tables: Vec<TableInfo>,
    tables_promise: Option<Promise<Result<Vec<TableInfo>, String>>>,
    tables_loaded: bool, // 标记表是否已加载（即使结果为空）

    // 导出状态
    export_promise: Option<Promise<Result<(), String>>>,
    export_progress: f32, // 0.0 - 1.0
    export_status: String,
    is_exporting: bool,
}

impl ExportWindow {
    pub fn ui(&mut self, ui: &mut egui::Ui, db_manager: &mut DbManager) {
        ui.vertical(|ui| {
            ui.set_width(500.0);

            // 数据库选择
            ui.horizontal(|ui| {
                ui.label("数据库:");
                self.load_databases_if_needed(db_manager);

                // 检查数据库加载状态
                if let Some(ref promise) = self.databases_promise {
                    if let Some(result) = promise.ready() {
                        match result {
                            Ok(databases) => {
                                self.databases = databases.clone();
                            }
                            Err(e) => {
                                ui.colored_label(egui::Color32::RED, format!("错误: {}", e));
                            }
                        }
                        self.databases_promise = None;
                    } else {
                        ui.spinner();
                        ui.label("加载中...");
                    }
                }

                egui::ComboBox::from_id_salt("export_database")
                    .width(300.0)
                    .selected_text(
                        self.selected_database
                            .as_ref()
                            .map(|s| s.as_str())
                            .unwrap_or("请选择数据库"),
                    )
                    .show_ui(ui, |ui| {
                        if self.databases.is_empty() && self.databases_promise.is_none() {
                            ui.label("没有可用的数据库");
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
                                    // 重置 schema 和 tables
                                    self.selected_schema = None;
                                    self.selected_tables.clear();
                                    self.schemas.clear();
                                    self.tables.clear();
                                    self.tables_loaded = false; // 重置加载状态
                                }
                            }
                        }
                    });
            });

            // Schema 选择
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
                                    ui.colored_label(egui::Color32::RED, format!("错误: {}", e));
                                }
                            }
                            self.schemas_promise = None;
                        } else {
                            ui.spinner();
                            ui.label("加载中...");
                        }
                    }

                    egui::ComboBox::from_id_salt("export_schema")
                        .width(300.0)
                        .selected_text(
                            self.selected_schema
                                .as_ref()
                                .map(|s| s.as_str())
                                .unwrap_or("请选择 Schema"),
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
                                    // 重置 tables
                                    self.selected_tables.clear();
                                    self.tables.clear();
                                    self.tables_loaded = false; // 重置加载状态
                                }
                            }
                        });
                });
            }

            // 表选择（多选）
            let schema_name = self.selected_schema.clone();
            let db_name = self.selected_database.clone();
            if let (Some(ref schema_name), Some(ref db_name)) = (schema_name, db_name) {
                ui.horizontal(|ui| {
                    ui.label("表:");
                    self.load_tables_if_needed(db_manager, db_name, schema_name);

                    if let Some(ref promise) = self.tables_promise {
                        if let Some(result) = promise.ready() {
                            match result {
                                Ok(tables) => {
                                    self.tables = tables.clone();
                                    self.tables_loaded = true; // 标记为已加载
                                }
                                Err(e) => {
                                    ui.colored_label(egui::Color32::RED, format!("错误: {}", e));
                                    self.tables_loaded = true; // 即使出错也标记为已加载，避免重复请求
                                }
                            }
                            self.tables_promise = None;
                        } else {
                            ui.spinner();
                            ui.label("加载中...");
                        }
                    }
                });

                // 表多选列表
                egui::ScrollArea::vertical()
                    .max_height(150.0)
                    .show(ui, |ui| {
                        if self.tables_loaded && self.tables.is_empty() {
                            ui.label("该 Schema 中没有表");
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

            // 导出类型选择
            ui.horizontal(|ui| {
                ui.label("导出类型:");
                ui.checkbox(&mut self.export_ddl, "DDL");
                ui.checkbox(&mut self.export_dml, "DML");
            });

            ui.separator();

            // 文件路径选择
            ui.horizontal(|ui| {
                ui.label("保存路径:");
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

                if ui.button("浏览...").clicked() {
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

            // 进度条和状态（导出中或已完成时都显示）
            if self.is_exporting || self.export_progress > 0.0 {
                ui.horizontal(|ui| {
                    if self.is_exporting {
                        ui.spinner();
                    }
                    ui.label(&self.export_status);
                });
                ui.add(egui::ProgressBar::new(self.export_progress));
            }

            // 按钮
            ui.horizontal(|ui| {
                let can_export = self.selected_database.is_some()
                    && self.selected_schema.is_some()
                    && !self.selected_tables.is_empty()
                    && (self.export_ddl || self.export_dml)
                    && self.file_path.is_some()
                    && !self.is_exporting;

                if ui
                    .add_enabled(can_export, egui::Button::new("导出"))
                    .clicked()
                {
                    self.start_export(db_manager);
                }

                if ui
                    .button(if self.is_exporting {
                        "取消"
                    } else {
                        "关闭"
                    })
                    .clicked()
                {
                    if !self.is_exporting {
                        // 如果不在导出中，重置所有状态
                        *self = ExportWindow::default();
                    }
                    // 如果正在导出，取消按钮的行为可以在这里处理（目前只是关闭窗口）
                }
            });
        });
    }

    fn load_databases_if_needed(&mut self, db_manager: &mut DbManager) {
        if !self.databases.is_empty() || self.databases_promise.is_some() {
            return;
        }

        let Some(db_id) = db_manager.active_db_config_id.clone() else {
            return;
        };

        db_manager.ensure_storage();
        let dsn = if let Some(ref storage) = db_manager.storage {
            match futures::block_on_async(async { storage.get_db_config(&db_id).await }) {
                Ok(Some(cfg)) => cfg.dsn,
                _ => return,
            }
        } else {
            return;
        };

        let dsn_clone = dsn.clone();
        let (sender, promise) = Promise::new();
        self.databases_promise = Some(promise);

        futures::spawn(async move {
            let result: Result<Vec<DatabaseInfo>, String> = async {
                let session = Session::connect_to_postgres(&dsn_clone)
                    .await
                    .map_err(|e| format!("Failed to connect: {}", e))?;

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

        let Some(db_id) = db_manager.active_db_config_id.clone() else {
            return;
        };

        db_manager.ensure_storage();
        let dsn = if let Some(ref storage) = db_manager.storage {
            match futures::block_on_async(async { storage.get_db_config(&db_id).await }) {
                Ok(Some(cfg)) => structures::utils::replace_database_in_dsn(&cfg.dsn, database)
                    .unwrap_or_else(|| cfg.dsn.clone()),
                _ => return,
            }
        } else {
            return;
        };

        let dsn_clone = dsn.clone();
        let (sender, promise) = Promise::new();
        self.schemas_promise = Some(promise);

        futures::spawn(async move {
            let result: Result<Vec<SchemaInfo>, String> = async {
                let session = Session::new(&dsn_clone)
                    .await
                    .map_err(|e| format!("Failed to create session: {}", e))?;

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
        // 如果已经加载过或者正在加载，则不再加载
        if self.tables_loaded || self.tables_promise.is_some() {
            return;
        }

        let Some(db_id) = db_manager.active_db_config_id.clone() else {
            return;
        };

        db_manager.ensure_storage();
        let dsn = if let Some(ref storage) = db_manager.storage {
            match futures::block_on_async(async { storage.get_db_config(&db_id).await }) {
                Ok(Some(cfg)) => structures::utils::replace_database_in_dsn(&cfg.dsn, database)
                    .unwrap_or_else(|| cfg.dsn.clone()),
                _ => return,
            }
        } else {
            return;
        };

        let dsn_clone = dsn.clone();
        let schema_clone = schema.to_string();
        let (sender, promise) = Promise::new();
        self.tables_promise = Some(promise);

        futures::spawn(async move {
            let result: Result<Vec<TableInfo>, String> = async {
                let session = Session::new(&dsn_clone)
                    .await
                    .map_err(|e| format!("Failed to create session: {}", e))?;

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

        let Some(db_id) = db_manager.active_db_config_id.clone() else {
            return;
        };

        db_manager.ensure_storage();
        let dsn = if let Some(ref storage) = db_manager.storage {
            match futures::block_on_async(async { storage.get_db_config(&db_id).await }) {
                Ok(Some(cfg)) => structures::utils::replace_database_in_dsn(&cfg.dsn, &db_name)
                    .unwrap_or_else(|| cfg.dsn.clone()),
                _ => return,
            }
        } else {
            return;
        };

        let dsn_clone = dsn.clone();
        let schema_clone = schema_name.clone();
        let db_name_clone = db_name.clone();
        let tables_clone: Vec<String> = self.selected_tables.iter().cloned().collect();
        let export_ddl = self.export_ddl;
        let export_dml = self.export_dml;
        let file_path_clone = file_path.clone();

        self.is_exporting = true;
        self.export_progress = 0.0;
        self.export_status = "准备导出...".to_string();

        let (sender, promise) = Promise::new();
        self.export_promise = Some(promise);

        futures::spawn(async move {
            let result: Result<(), String> = async {
                // 创建文件
                let mut file = OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .write(true)
                    .open(&file_path_clone)
                    .map_err(|e| format!("Failed to create file: {}", e))?;

                // 写入文件头
                writeln!(file, "-- Export generated by PGone")
                    .map_err(|e| format!("Failed to write: {}", e))?;
                writeln!(file, "-- Database: {}", db_name_clone)
                    .map_err(|e| format!("Failed to write: {}", e))?;
                writeln!(file, "-- Schema: {}", schema_clone)
                    .map_err(|e| format!("Failed to write: {}", e))?;
                writeln!(file, "-- Tables: {}", tables_clone.join(", "))
                    .map_err(|e| format!("Failed to write: {}", e))?;
                writeln!(file, "").map_err(|e| format!("Failed to write: {}", e))?;

                let session = Session::new(&dsn_clone)
                    .await
                    .map_err(|e| format!("Failed to create session: {}", e))?;

                let total_tables = tables_clone.len();

                for (table_idx, table_name) in tables_clone.iter().enumerate() {
                    // 更新进度（注意：这里无法直接更新 UI，需要在 UI 线程中检查 promise）
                    let _progress = (table_idx as f32) / (total_tables as f32);

                    // 导出 DDL
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

                    // 导出 DML
                    if export_dml {
                        writeln!(file, "-- DML for table {}.{}", schema_clone, table_name)
                            .map_err(|e| format!("Failed to write: {}", e))?;

                        // 分页查询数据
                        const PAGE_SIZE: usize = 100;
                        let mut offset = 0;
                        let mut has_more = true;
                        let mut columns_loaded = false;
                        let mut column_names = Vec::new();

                        while has_more {
                            // 构建查询 SQL
                            let query = format!(
                                "SELECT * FROM {}.{} LIMIT {} OFFSET {}",
                                structures::utils::quote_ident(&schema_clone),
                                structures::utils::quote_ident(table_name),
                                PAGE_SIZE,
                                offset
                            );

                            // 直接执行 SQL 查询
                            let conn = session
                                .get_connection()
                                .await
                                .map_err(|e| format!("Failed to get connection: {}", e))?;

                            let rows = conn
                                .query(&query, &[])
                                .await
                                .map_err(|e| format!("Failed to query data: {}", e))?;

                            if rows.is_empty() {
                                has_more = false;
                            } else {
                                // 获取列名（只在第一次）
                                if !columns_loaded {
                                    if let Some(first_row) = rows.first() {
                                        for col in first_row.columns() {
                                            column_names.push(col.name().to_string());
                                        }
                                        columns_loaded = true;
                                    }
                                }

                                // 转换行数据
                                let mut row_data = Vec::new();
                                for row in rows {
                                    let mut row_values = Vec::new();
                                    for i in 0..column_names.len() {
                                        // 格式化单元格值
                                        let value = if row.try_get::<_, String>(i).is_ok() {
                                            row.get::<_, String>(i)
                                        } else if row.try_get::<_, i64>(i).is_ok() {
                                            row.get::<_, i64>(i).to_string()
                                        } else if row.try_get::<_, f64>(i).is_ok() {
                                            row.get::<_, f64>(i).to_string()
                                        } else if row.try_get::<_, bool>(i).is_ok() {
                                            row.get::<_, bool>(i).to_string()
                                        } else {
                                            // 尝试作为字符串获取，如果失败则使用 NULL
                                            row.try_get::<_, Option<String>>(i)
                                                .ok()
                                                .flatten()
                                                .unwrap_or_else(|| "NULL".to_string())
                                        };
                                        row_values.push(value);
                                    }
                                    row_data.push(row_values);
                                }

                                // 生成 DML
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
                        self.export_status = "导出完成！".to_string();
                        self.export_promise = None;
                    }
                    Err(e) => {
                        self.is_exporting = false;
                        self.export_status = format!("导出失败: {}", e);
                        self.export_promise = None;
                    }
                }
            } else {
                // 更新进度（简化版本，实际应该从异步任务中获取）
                if self.export_progress < 0.9 {
                    self.export_progress += 0.01;
                }
            }
        }
    }

    pub fn is_exporting(&self) -> bool {
        self.is_exporting
    }
}
