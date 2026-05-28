use crate::components::DbManager;
use crate::futures;
use pgone_sql::{DatabaseInfo, SchemaInfo, Session};
use poll_promise::Promise;
use sqlx::Row;
use std::fs;
use std::path::PathBuf;

#[derive(Clone)]
pub struct ImportResult {
    pub sql: String,
    pub success: bool,
    pub error: Option<String>,
    pub rows_affected: Option<u64>,
}

#[derive(Default)]
pub struct ImportWindow {
    // 选择状态
    selected_database: Option<String>,
    selected_schema: Option<String>,

    // 文件路径
    file_path: Option<PathBuf>,

    // 数据加载
    databases: Vec<DatabaseInfo>,
    databases_promise: Option<Promise<Result<Vec<DatabaseInfo>, String>>>,
    schemas: Vec<SchemaInfo>,
    schemas_promise: Option<Promise<Result<Vec<SchemaInfo>, String>>>,

    // 导入状态
    import_promise: Option<Promise<Result<Vec<ImportResult>, String>>>,
    import_progress: f32, // 0.0 - 1.0
    import_status: String,
    is_importing: bool,
    results: Vec<ImportResult>,
    show_results: bool,
}

impl ImportWindow {
    pub fn ui(&mut self, ui: &mut egui::Ui, db_manager: &mut DbManager) {
        ui.vertical(|ui| {
            ui.set_width(600.0);

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

                egui::ComboBox::from_id_salt("import_database")
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
                                    // 重置 schema
                                    self.selected_schema = None;
                                    self.schemas.clear();
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

                    egui::ComboBox::from_id_salt("import_schema")
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
                                    // Schema selected
                                }
                            }
                        });
                });
            }

            ui.separator();

            // 文件路径选择
            ui.horizontal(|ui| {
                ui.label("SQL文件:");
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
                        .add_filter("SQL Files", &["sql"])
                        .add_filter("All Files", &["*"])
                        .pick_file()
                    {
                        self.file_path = Some(path);
                    }
                }
            });

            ui.separator();

            // 进度条和状态（导入中或已完成时都显示）
            if self.is_importing || self.import_progress > 0.0 {
                ui.horizontal(|ui| {
                    if self.is_importing {
                        ui.spinner();
                    }
                    ui.label(&self.import_status);
                });
                ui.add(egui::ProgressBar::new(self.import_progress));
            }

            // 结果显示
            if !self.results.is_empty() {
                ui.separator();
                ui.checkbox(&mut self.show_results, "显示详细结果");

                if self.show_results {
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .show(ui, |ui| {
                            let success_count = self.results.iter().filter(|r| r.success).count();
                            let fail_count = self.results.len() - success_count;

                            ui.label(format!("成功: {}, 失败: {}", success_count, fail_count));
                            ui.separator();

                            for (idx, result) in self.results.iter().enumerate() {
                                let color = if result.success {
                                    egui::Color32::GREEN
                                } else {
                                    egui::Color32::RED
                                };

                                ui.horizontal(|ui| {
                                    ui.colored_label(color, format!("[{}]", idx + 1));
                                    if result.success {
                                        ui.label(format!(
                                            "✓ {}",
                                            result
                                                .rows_affected
                                                .map(|r| format!("影响 {} 行", r))
                                                .unwrap_or_else(|| "成功".to_string())
                                        ));
                                    } else {
                                        ui.label(format!(
                                            "✗ {}",
                                            result
                                                .error
                                                .as_ref()
                                                .unwrap_or(&"未知错误".to_string())
                                        ));
                                    }
                                });

                                if ui.small_button("查看SQL").clicked() {
                                    // 可以显示SQL内容
                                }
                            }
                        });
                }
            }

            // 按钮
            ui.horizontal(|ui| {
                let can_import = self.selected_database.is_some()
                    && self.selected_schema.is_some()
                    && self.file_path.is_some()
                    && !self.is_importing;

                if ui
                    .add_enabled(can_import, egui::Button::new("导入"))
                    .clicked()
                {
                    self.start_import(db_manager);
                }

                if ui
                    .button(if self.is_importing {
                        "取消"
                    } else {
                        "关闭"
                    })
                    .clicked()
                {
                    if !self.is_importing {
                        // 如果不在导入中，重置所有状态
                        *self = ImportWindow::default();
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

    fn parse_sql_file(&self, content: &str) -> (Vec<String>, Vec<String>) {
        let mut ddl_statements = Vec::new();
        let mut dml_statements = Vec::new();

        let mut current_section: Option<&str> = None; // "DDL" or "DML"
        let mut current_statement = String::new();
        let mut in_string = false;
        let mut string_char = '\0';
        let mut in_comment = false;

        let lines: Vec<&str> = content.lines().collect();

        for line in lines {
            let trimmed = line.trim();

            // 检查是否是DDL或DML标记
            if trimmed.starts_with("-- DDL for table") {
                // 如果之前有未完成的语句，先保存
                if !current_statement.trim().is_empty() {
                    match current_section {
                        Some("DDL") => ddl_statements.push(current_statement.trim().to_string()),
                        Some("DML") => dml_statements.push(current_statement.trim().to_string()),
                        _ => {}
                    }
                }
                current_statement.clear();
                current_section = Some("DDL");
                continue;
            } else if trimmed.starts_with("-- DML for table") {
                // 如果之前有未完成的语句，先保存
                if !current_statement.trim().is_empty() {
                    match current_section {
                        Some("DDL") => ddl_statements.push(current_statement.trim().to_string()),
                        Some("DML") => dml_statements.push(current_statement.trim().to_string()),
                        _ => {}
                    }
                }
                current_statement.clear();
                current_section = Some("DML");
                continue;
            }

            // 跳过注释行（但不是字符串中的）
            if trimmed.starts_with("--") && !in_string {
                continue;
            }

            // 处理多行注释
            if trimmed.contains("/*") {
                in_comment = true;
            }
            if trimmed.contains("*/") {
                in_comment = false;
                continue;
            }
            if in_comment {
                continue;
            }

            // 处理字符串
            for ch in line.chars() {
                if !in_string && (ch == '\'' || ch == '"') {
                    in_string = true;
                    string_char = ch;
                    current_statement.push(ch);
                } else if in_string && ch == string_char {
                    // 检查是否是转义的引号
                    if current_statement.ends_with('\\') {
                        current_statement.push(ch);
                    } else {
                        in_string = false;
                        current_statement.push(ch);
                    }
                } else if !in_string && ch == ';' {
                    current_statement.push(ch);
                    // 语句结束
                    let stmt = current_statement.trim().to_string();
                    if !stmt.is_empty() && !stmt.starts_with("--") {
                        match current_section {
                            Some("DDL") => ddl_statements.push(stmt),
                            Some("DML") => dml_statements.push(stmt),
                            _ => {
                                // 如果没有明确的标记，根据语句类型判断
                                let upper = stmt.to_uppercase();
                                if upper.starts_with("CREATE")
                                    || upper.starts_with("ALTER")
                                    || upper.starts_with("DROP")
                                    || upper.starts_with("COMMENT")
                                {
                                    ddl_statements.push(stmt);
                                } else if upper.starts_with("INSERT")
                                    || upper.starts_with("UPDATE")
                                    || upper.starts_with("DELETE")
                                {
                                    dml_statements.push(stmt);
                                }
                            }
                        }
                    }
                    current_statement.clear();
                } else {
                    current_statement.push(ch);
                }
            }

            // 添加换行符（如果不在字符串中）
            if !in_string {
                current_statement.push('\n');
            }
        }

        // 处理最后一个语句
        if !current_statement.trim().is_empty() {
            let stmt = current_statement.trim().to_string();
            if !stmt.is_empty() && !stmt.starts_with("--") {
                match current_section {
                    Some("DDL") => ddl_statements.push(stmt),
                    Some("DML") => dml_statements.push(stmt),
                    _ => {
                        let upper = stmt.to_uppercase();
                        if upper.starts_with("CREATE")
                            || upper.starts_with("ALTER")
                            || upper.starts_with("DROP")
                            || upper.starts_with("COMMENT")
                        {
                            ddl_statements.push(stmt);
                        } else if upper.starts_with("INSERT")
                            || upper.starts_with("UPDATE")
                            || upper.starts_with("DELETE")
                        {
                            dml_statements.push(stmt);
                        }
                    }
                }
            }
        }

        (ddl_statements, dml_statements)
    }

    fn start_import(&mut self, db_manager: &mut DbManager) {
        if self.is_importing {
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

        let Some(_db_id) = db_manager.active_db_config_id.clone() else {
            return;
        };

        let Some(dsn) = db_manager.dsn_for_database(&db_name) else {
            return;
        };

        // 读取文件内容
        let file_content = match fs::read_to_string(&file_path) {
            Ok(content) => content,
            Err(e) => {
                self.import_status = format!("读取文件失败: {}", e);
                return;
            }
        };

        // 解析SQL文件
        let (ddl_statements, dml_statements) = self.parse_sql_file(&file_content);

        if ddl_statements.is_empty() && dml_statements.is_empty() {
            self.import_status = "SQL文件中没有找到有效的语句".to_string();
            return;
        }

        let pools = db_manager.pools.clone();
        let dsn_clone = dsn.clone();
        let schema_clone = schema_name.clone();

        self.is_importing = true;
        self.import_progress = 0.0;
        self.import_status = format!(
            "准备导入... (DDL: {}, DML: {})",
            ddl_statements.len(),
            dml_statements.len()
        );
        self.results.clear();

        let (sender, promise) = Promise::new();
        self.import_promise = Some(promise);

        futures::spawn(async move {
            let result: Result<Vec<ImportResult>, String> = async {
                let pool = pools.get_or_create_pool(&dsn_clone).await?;

                let mut results = Vec::new();

                // 先执行DDL语句
                for (idx, sql) in ddl_statements.iter().enumerate() {
                    let _progress_msg = format!("执行DDL ({}/{})...", idx + 1, ddl_statements.len());

                    // 检查是否是CREATE TABLE语句，如果是，检查表是否已存在
                    let sql_upper = sql.to_uppercase().trim().to_string();
                    if sql_upper.starts_with("CREATE TABLE") {
                        // 提取表名
                        if let Some(table_name) = extract_table_name_from_create(&sql_upper) {
                            // 检查表是否存在
                            let check_sql = format!(
                                "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_schema = '{}' AND table_name = '{}')",
                                schema_clone, table_name
                            );

                            match sqlx::query(&check_sql).fetch_one(&pool).await {
                                Ok(row) => {
                                    let exists: bool = row.get(0);
                                    if exists {
                                        results.push(ImportResult {
                                            sql: sql.clone(),
                                            success: true,
                                            error: Some("表已存在，跳过".to_string()),
                                            rows_affected: None,
                                        });
                                        continue;
                                    }
                                }
                                Err(_) => {
                                    // 如果检查失败，继续执行
                                }
                            }
                        }
                    }

                    // 执行SQL语句
                    match sqlx::query(sql).execute(&pool).await {
                        Ok(result) => {
                            results.push(ImportResult {
                                sql: sql.clone(),
                                success: true,
                                error: None,
                                rows_affected: Some(result.rows_affected()),
                            });
                        }
                        Err(e) => {
                            // 如果是"表已存在"错误，跳过
                            let error_msg = e.to_string();
                            if error_msg.contains("already exists") || error_msg.contains("已存在") {
                                results.push(ImportResult {
                                    sql: sql.clone(),
                                    success: true,
                                    error: Some("表已存在，跳过".to_string()),
                                    rows_affected: None,
                                });
                            } else {
                                results.push(ImportResult {
                                    sql: sql.clone(),
                                    success: false,
                                    error: Some(error_msg),
                                    rows_affected: None,
                                });
                            }
                        }
                    }
                }

                // 然后执行DML语句
                for (idx, sql) in dml_statements.iter().enumerate() {
                    let _progress_msg = format!("执行DML ({}/{})...", idx + 1, dml_statements.len());

                    match sqlx::query(sql).execute(&pool).await {
                        Ok(result) => {
                            results.push(ImportResult {
                                sql: sql.clone(),
                                success: true,
                                error: None,
                                rows_affected: Some(result.rows_affected()),
                            });
                        }
                        Err(e) => {
                            results.push(ImportResult {
                                sql: sql.clone(),
                                success: false,
                                error: Some(e.to_string()),
                                rows_affected: None,
                            });
                        }
                    }
                }

                Ok(results)
            }.await;

            sender.send(result);
        });
    }

    pub fn check_import_progress(&mut self) {
        if let Some(ref promise) = self.import_promise {
            if let Some(result) = promise.ready() {
                match result {
                    Ok(results) => {
                        self.is_importing = false;
                        self.import_progress = 1.0;
                        self.results = results.clone();
                        let success_count = results.iter().filter(|r| r.success).count();
                        let fail_count = results.len() - success_count;
                        self.import_status =
                            format!("导入完成！成功: {}, 失败: {}", success_count, fail_count);
                        self.import_promise = None;
                        self.show_results = true;
                    }
                    Err(e) => {
                        self.is_importing = false;
                        self.import_status = format!("导入失败: {}", e);
                        self.import_promise = None;
                    }
                }
            } else {
                // 更新进度（简化版本）
                if self.import_progress < 0.9 {
                    self.import_progress += 0.01;
                }
            }
        }
    }

    pub fn is_importing(&self) -> bool {
        self.is_importing
    }
}

// 从CREATE TABLE语句中提取表名
fn extract_table_name_from_create(sql: &str) -> Option<String> {
    // 简单的正则匹配：CREATE TABLE schema.table_name 或 CREATE TABLE table_name
    // 处理可能的引号：CREATE TABLE "schema"."table_name" 或 CREATE TABLE schema.table_name
    let parts: Vec<&str> = sql.split_whitespace().collect();
    if parts.len() >= 3 && parts[0].to_uppercase() == "CREATE" && parts[1].to_uppercase() == "TABLE"
    {
        let mut table_part = parts[2];
        // 移除可能的引号和分号
        let mut table_name = table_part
            .trim_matches('"')
            .trim_matches('\'')
            .trim_end_matches(';')
            .trim_end_matches('(')
            .to_string();

        // 如果下一个部分是点号，说明是schema.table格式
        if parts.len() > 3 && parts[3] == "." {
            if parts.len() > 4 {
                table_name = parts[4]
                    .trim_matches('"')
                    .trim_matches('\'')
                    .trim_end_matches(';')
                    .trim_end_matches('(')
                    .to_string();
            }
        } else if table_name.contains('.') {
            // 如果包含点号，取最后一部分
            if let Some(dot_pos) = table_name.rfind('.') {
                table_name = table_name[dot_pos + 1..].to_string();
            }
        }

        Some(table_name)
    } else {
        None
    }
}
