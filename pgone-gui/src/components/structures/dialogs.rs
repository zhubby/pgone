use super::types::{DbTree, DialogType, EditableColumn};
use super::operations;
use super::design;
use egui_data_table::{DataTable, Renderer, RowViewer};

/// 列数据的 RowViewer 实现
struct ColumnRowViewer;

impl ColumnRowViewer {
    fn new() -> Self {
        Self
    }
    
    fn column_names() -> &'static [&'static str] {
        &["名称", "类型", "长度", "精度", "小数位", "可空", "默认值", "注释"]
    }
}

impl RowViewer<EditableColumn> for ColumnRowViewer {
    fn num_columns(&mut self) -> usize {
        Self::column_names().len()
    }
    
    fn show_cell_view(&mut self, ui: &mut egui::Ui, row: &EditableColumn, column: usize) {
        let _ = match column {
            0 => ui.label(&row.name),
            1 => ui.label(&row.data_type),
            2 => ui.label(row.character_maximum_length.map(|v| v.to_string()).unwrap_or_default()),
            3 => ui.label(row.numeric_precision.map(|v| v.to_string()).unwrap_or_default()),
            4 => ui.label(row.numeric_scale.map(|v| v.to_string()).unwrap_or_default()),
            5 => ui.label(if row.nullable { "是" } else { "否" }),
            6 => ui.label(row.default.as_deref().unwrap_or("")),
            7 => ui.label(row.comment.as_deref().unwrap_or("")),
            _ => ui.label(""),
        };
    }
    
    fn show_cell_editor(
        &mut self,
        ui: &mut egui::Ui,
        row: &mut EditableColumn,
        column: usize,
    ) -> Option<egui::Response> {
        match column {
            0 => {
                let response = ui.text_edit_singleline(&mut row.name);
                // 如果名称改变且不是新列，更新 original_name
                if row.original_name.is_none() && !row.is_new {
                    row.original_name = Some(row.name.clone());
                }
                Some(response)
            }
            1 => {
                // 数据类型下拉选择
                let types = ["VARCHAR", "CHAR", "TEXT", "INTEGER", "BIGINT", "SMALLINT", 
                            "NUMERIC", "DECIMAL", "REAL", "DOUBLE PRECISION", "BOOLEAN",
                            "DATE", "TIME", "TIMESTAMP", "TIMESTAMPTZ", "JSON", "JSONB"];
                let mut selected = 0;
                for (i, t) in types.iter().enumerate() {
                    if row.data_type.to_uppercase().starts_with(t) {
                        selected = i;
                        break;
                    }
                }
                let response = egui::ComboBox::from_id_salt(("type", column))
                    .selected_text(types[selected])
                    .show_ui(ui, |ui| {
                        for (i, t) in types.iter().enumerate() {
                            if ui.selectable_label(i == selected, *t).clicked() {
                                row.data_type = t.to_string();
                            }
                        }
                    });
                Some(response.response)
            }
            2 => {
                let mut len_str = row.character_maximum_length.map(|v| v.to_string()).unwrap_or_default();
                let response = ui.text_edit_singleline(&mut len_str);
                if let Ok(len) = len_str.parse::<i32>() {
                    row.character_maximum_length = Some(len);
                } else if len_str.is_empty() {
                    row.character_maximum_length = None;
                }
                Some(response)
            }
            3 => {
                let mut prec_str = row.numeric_precision.map(|v| v.to_string()).unwrap_or_default();
                let response = ui.text_edit_singleline(&mut prec_str);
                if let Ok(prec) = prec_str.parse::<i32>() {
                    row.numeric_precision = Some(prec);
                } else if prec_str.is_empty() {
                    row.numeric_precision = None;
                }
                Some(response)
            }
            4 => {
                let mut scale_str = row.numeric_scale.map(|v| v.to_string()).unwrap_or_default();
                let response = ui.text_edit_singleline(&mut scale_str);
                if let Ok(scale) = scale_str.parse::<i32>() {
                    row.numeric_scale = Some(scale);
                } else if scale_str.is_empty() {
                    row.numeric_scale = None;
                }
                Some(response)
            }
            5 => {
                let response = ui.checkbox(&mut row.nullable, "");
                Some(response)
            }
            6 => {
                let default_str = row.default.as_deref().unwrap_or("");
                let mut default = default_str.to_string();
                let response = ui.text_edit_singleline(&mut default);
                if default.is_empty() {
                    row.default = None;
                } else {
                    row.default = Some(default);
                }
                Some(response)
            }
            7 => {
                let comment_str = row.comment.as_deref().unwrap_or("");
                let mut comment = comment_str.to_string();
                let response = ui.text_edit_singleline(&mut comment);
                if comment.is_empty() {
                    row.comment = None;
                } else {
                    row.comment = Some(comment);
                }
                Some(response)
            }
            _ => None,
        }
    }
    
    fn set_cell_value(&mut self, src: &EditableColumn, dst: &mut EditableColumn, column: usize) {
        match column {
            0 => dst.name = src.name.clone(),
            1 => dst.data_type = src.data_type.clone(),
            2 => dst.character_maximum_length = src.character_maximum_length,
            3 => dst.numeric_precision = src.numeric_precision,
            4 => dst.numeric_scale = src.numeric_scale,
            5 => dst.nullable = src.nullable,
            6 => dst.default = src.default.clone(),
            7 => dst.comment = src.comment.clone(),
            _ => {}
        }
    }
    
    fn new_empty_row(&mut self) -> EditableColumn {
        EditableColumn {
            name: String::new(),
            data_type: "VARCHAR".to_string(),
            character_maximum_length: None,
            numeric_precision: None,
            numeric_scale: None,
            nullable: true,
            default: None,
            comment: None,
            is_new: true,
            is_deleted: false,
            original_name: None,
        }
    }
    
    fn column_name(&mut self, column: usize) -> std::borrow::Cow<'static, str> {
        Self::column_names().get(column).copied().unwrap_or("").into()
    }
    
    fn is_editable_cell(&mut self, column: usize, _row: usize, _row_value: &EditableColumn) -> bool {
        column < 8 // 所有列都可编辑
    }
    
    fn allow_row_insertions(&mut self) -> bool {
        true
    }
    
    fn allow_row_deletions(&mut self) -> bool {
        true
    }
}

pub(super) fn show_dialogs(tree: &mut DbTree, ui: &mut egui::Ui, db_manager: &mut crate::components::DbManager) {
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
        
        let center = ui.ctx().screen_rect().center();
        let mut window = egui::Window::new(title)
            .open(&mut open)
            .default_pos(center)
            .pivot(egui::Align2::CENTER_CENTER)
            .collapsible(false); // 所有对话框都不可折叠
        
        // 为 DesignTable 对话框设置合适的大小
        if matches!(dialog_type, DialogType::DesignTable { .. }) {
            // 窗口高度设置为屏幕高度的一半
            let screen_height = ui.ctx().screen_rect().height();
            let window_height = screen_height * 0.5;
            
            window = window
                .default_size([900.0, window_height])
                .resizable(true)
                .max_size([1200.0, screen_height * 0.8])
                .min_size([600.0, 300.0]);
        }
        
        // 为 ShowDdl 对话框设置合适的大小
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
                    DialogType::CreateView { database: _, schema: _ } => {
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
                    DialogType::CreateMaterializedView { database: _, schema: _ } => {
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
                    DialogType::CreateFunction { database: _, schema: _ } => {
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
                    DialogType::PropertiesView { database, schema, name } => {
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
                    DialogType::PropertiesMaterializedView { database, schema, name } => {
                        // Show materialized view properties (read-only)
                        let key = format!("{}.{}", database, schema);
                        if let Some(materialized_views) = tree.materialized_views.get(&key) {
                            if let Some(matview) = materialized_views.iter().find(|mv| mv.name == *name) {
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
                    DialogType::PropertiesFunction { database, schema, name } => {
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
                    DialogType::DesignTable { database, schema, name } => {
                        // 检查当前对话框的表是否与已加载的表匹配
                        let current_table = (database.clone(), schema.clone(), name.clone());
                        if let Some(ref loaded_table) = tree.design_table_loaded {
                            if *loaded_table != current_table {
                                // 表名不匹配，清空数据并触发重新加载
                                tree.design_table_detail = None;
                                tree.design_table_columns.clear();
                                tree.design_table_promise = None;
                                tree.design_table_loaded = None;
                                // 触发重新加载
                                use super::loading;
                                loading::load_table_detail_for_design(tree, db_manager, database, schema, name);
                                ui.label("Loading table structure...");
                                return;
                            }
                        }
                        
                        // 检查异步加载的表结构详情
                        if let Some(ref promise) = tree.design_table_promise {
                            if let Some(result) = promise.ready() {
                                match result {
                                    Ok(detail) => {
                                        // 初始化可编辑列数据
                                        tree.design_table_detail = Some(detail.clone());
                                        tree.design_table_columns = detail.columns.iter().map(|col| {
                                            EditableColumn {
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
                                            }
                                        }).collect();
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
                            // 没有 promise 且没有已加载的数据，触发加载
                            use super::loading;
                            loading::load_table_detail_for_design(tree, db_manager, database, schema, name);
                            ui.label("Loading table structure...");
                            return;
                        }
                        
                        // 显示表设计界面
                        if tree.design_table_columns.is_empty() {
                            ui.label("No columns to display");
                        } else {
                            // 使用 egui-data-table 渲染可编辑表格
                            let mut data_table: DataTable<EditableColumn> = tree.design_table_columns.clone().into_iter().collect();
                            let mut viewer = ColumnRowViewer::new();
                            
                            // 使用 ScrollArea 允许滚动，固定窗口高度时内容超出会显示滚动条
                            egui::ScrollArea::vertical()
                                .auto_shrink([false; 2])
                                .show(ui, |ui| {
                                    Renderer::new(&mut data_table, &mut viewer).show(ui);
                                });
                            
                            // 更新列数据
                            tree.design_table_columns = data_table.iter().cloned().collect();
                            
                            // 右下角按钮
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
                    DialogType::ShowDdl { database: _, schema: _, name: _ } => {
                        // 检查异步加载的DDL
                        if let Some(ref promise) = tree.ddl_promise {
                            if let Some(result) = promise.ready() {
                                match result {
                                    Ok(_) => {
                                        // DDL已加载到dialog_ddl_content
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
                        
                        // 显示DDL内容，使用SQL高亮
                        let ddl_content = tree.dialog_ddl_content.clone();
                        let available_height = ui.available_height() - 60.0; // 为按钮预留空间
                        
                        egui::ScrollArea::vertical()
                            .auto_shrink([false; 2])
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.add_space(5.0);
                                    
                                    let mut ddl_text = ddl_content.clone();
                                    let ddl_text_ref = &mut ddl_text;
                                    let ddl_for_highlight = ddl_content.clone();
                                    
                                    ui.add_sized(
                                        egui::Vec2::new(ui.available_width() - 5.0, available_height.max(200.0)),
                                        egui::TextEdit::multiline(ddl_text_ref)
                                            .desired_rows((available_height.max(200.0) / 20.0) as usize)
                                            .interactive(false) // 设置为只读
                                            .layouter(&mut move |ui, _text, wrap_width| {
                                                let mut job = crate::sql::highlight_sql(&ddl_for_highlight, ui.visuals());
                                                job.wrap.max_width = wrap_width;
                                                ui.fonts(|f| f.layout_job(job))
                                            }),
                                    );
                                });
                            });
                        
                        // 关闭和复制按钮
                        ui.separator();
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Close").clicked() {
                                should_close = true;
                            }
                            ui.add_space(8.0);
                            if ui.button("Copy").clicked() {
                                // 复制 DDL 内容到剪贴板
                                let ddl_to_copy = tree.dialog_ddl_content.clone();
                                ui.ctx().copy_text(ddl_to_copy);
                            }
                        });
                    }
                    DialogType::DropTable { database: _, schema, name } => {
                        ui.colored_label(egui::Color32::RED, format!("警告：此操作将清空表 '{}.{}' 中的所有数据，此操作不可撤销。", schema, name));
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
        if should_create || should_delete || should_rename || should_save_design || should_close || should_drop {
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
                        operations::create_table(tree, db_manager, &database, &schema, &dialog_ddl_clone);
                    }
                    DialogType::CreateView { database, schema } => {
                        operations::create_view(tree, db_manager, &database, &schema, &dialog_ddl_clone);
                    }
                    DialogType::CreateMaterializedView { database, schema } => {
                        operations::create_materialized_view(tree, db_manager, &database, &schema, &dialog_ddl_clone);
                    }
                    DialogType::CreateFunction { database, schema } => {
                        operations::create_function(tree, db_manager, &database, &schema, &dialog_ddl_clone);
                    }
                    DialogType::DesignTable { .. } | DialogType::DeleteDatabase { .. } | DialogType::DeleteSchema { .. } | DialogType::DeleteTable { .. } | DialogType::RenameDatabase { .. } | DialogType::RenameSchema { .. } | DialogType::RenameTable { .. } | DialogType::PropertiesDatabase { .. } | DialogType::PropertiesSchema { .. } | DialogType::PropertiesTable { .. } | DialogType::PropertiesView { .. } | DialogType::PropertiesMaterializedView { .. } | DialogType::PropertiesFunction { .. } | DialogType::ShowDdl { .. } | DialogType::DropTable { .. } => {}
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
                    DialogType::DesignTable { .. } | DialogType::CreateDatabase | DialogType::CreateSchema { .. } | DialogType::CreateTable { .. } | DialogType::CreateView { .. } | DialogType::CreateMaterializedView { .. } | DialogType::CreateFunction { .. } | DialogType::RenameDatabase { .. } | DialogType::RenameSchema { .. } | DialogType::RenameTable { .. } | DialogType::PropertiesDatabase { .. } | DialogType::PropertiesSchema { .. } | DialogType::PropertiesTable { .. } | DialogType::PropertiesView { .. } | DialogType::PropertiesMaterializedView { .. } | DialogType::PropertiesFunction { .. } | DialogType::ShowDdl { .. } | DialogType::DropTable { .. } => {}
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
                    DialogType::DesignTable { .. } | DialogType::CreateDatabase | DialogType::CreateSchema { .. } | DialogType::CreateTable { .. } | DialogType::CreateView { .. } | DialogType::CreateMaterializedView { .. } | DialogType::CreateFunction { .. } | DialogType::DeleteDatabase { .. } | DialogType::DeleteSchema { .. } | DialogType::DeleteTable { .. } | DialogType::PropertiesDatabase { .. } | DialogType::PropertiesSchema { .. } | DialogType::PropertiesTable { .. } | DialogType::PropertiesView { .. } | DialogType::PropertiesMaterializedView { .. } | DialogType::PropertiesFunction { .. } | DialogType::ShowDdl { .. } | DialogType::DropTable { .. } => {}
                }
            } else if should_save_design {
                match dialog_type {
                    DialogType::DesignTable { database, schema, name } => {
                        if let Some(ref original_detail) = tree.design_table_detail {
                            let statements = design::generate_alter_statements(
                                &schema,
                                &name,
                                original_detail,
                                &tree.design_table_columns,
                            );
                            if !statements.is_empty() {
                                operations::design_table(tree, db_manager, &database, &schema, &name, &statements);
                            } else {
                                // 即使没有语句，也清除设计状态
                                tree.design_table_detail = None;
                                tree.design_table_columns.clear();
                            }
                        }
                        // 清空已加载表的记录，下次打开时会重新加载
                        tree.design_table_loaded = None;
                        tree.design_table_promise = None;
                        // 关闭对话框
                        tree.dialog = None;
                    }
                    DialogType::CreateDatabase | DialogType::CreateSchema { .. } | DialogType::CreateTable { .. } | DialogType::CreateView { .. } | DialogType::CreateMaterializedView { .. } | DialogType::CreateFunction { .. } | DialogType::DeleteDatabase { .. } | DialogType::DeleteSchema { .. } | DialogType::DeleteTable { .. } | DialogType::RenameDatabase { .. } | DialogType::RenameSchema { .. } | DialogType::RenameTable { .. } | DialogType::PropertiesDatabase { .. } | DialogType::PropertiesSchema { .. } | DialogType::PropertiesTable { .. } | DialogType::PropertiesView { .. } | DialogType::PropertiesMaterializedView { .. } | DialogType::PropertiesFunction { .. } | DialogType::ShowDdl { .. } | DialogType::DropTable { .. } => {}
                }
            } else if should_drop {
                match dialog_type {
                    DialogType::DropTable { database, schema, name } => {
                        operations::drop_table(tree, db_manager, &database, &schema, &name);
                    }
                    DialogType::CreateDatabase | DialogType::CreateSchema { .. } | DialogType::CreateTable { .. } | DialogType::CreateView { .. } | DialogType::CreateMaterializedView { .. } | DialogType::CreateFunction { .. } | DialogType::DeleteDatabase { .. } | DialogType::DeleteSchema { .. } | DialogType::DeleteTable { .. } | DialogType::RenameDatabase { .. } | DialogType::RenameSchema { .. } | DialogType::RenameTable { .. } | DialogType::PropertiesDatabase { .. } | DialogType::PropertiesSchema { .. } | DialogType::PropertiesTable { .. } | DialogType::PropertiesView { .. } | DialogType::PropertiesMaterializedView { .. } | DialogType::PropertiesFunction { .. } | DialogType::DesignTable { .. } | DialogType::ShowDdl { .. } => {}
                }
            }
        }
    }
}

