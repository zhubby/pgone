use super::ResultsTable;
use crate::components::SqlCtx;
use crate::futures;
use egui_data_table::{DataTable, Renderer, RowViewer};
use sqlx::postgres::{PgPoolOptions, PgRow};
use sqlx::{Column, Row};
use std::collections::HashSet;
use super::utils;
use tracing::debug;

/// 查询结果行数据结构
/// 将动态的 Vec<String> 转换为结构化的行数据，便于 egui-data-table 使用
#[derive(Clone)]
struct QueryRow {
    cells: Vec<String>,
}

/// 查询结果表格的 RowViewer 实现
/// 负责定义如何显示和编辑表格的每一行数据
struct QueryRowViewer {
    /// 列名列表
    columns: Vec<String>,
    /// 主键列集合，用于标识主键列
    primary_keys: HashSet<String>,
}

impl QueryRowViewer {
    /// 截断单元格文本，最长12个字符，超过使用省略号显示
    /// 使用字符迭代器确保正确处理多字节字符（如中文）
    fn truncate_cell_text(text: &str) -> String {
        const MAX_LENGTH: usize = 12;
        if text.chars().count() <= MAX_LENGTH {
            text.to_string()
        } else {
            // 使用字符迭代器确保正确处理多字节字符
            let truncated: String = text.chars().take(MAX_LENGTH).collect();
            format!("{}...", truncated)
        }
    }
}

impl RowViewer<QueryRow> for QueryRowViewer {
    /// 返回列数
    fn num_columns(&mut self) -> usize {
        self.columns.len()
    }

    /// 显示单元格的只读视图
    /// 单元格内容最长显示12个字符，超过部分使用省略号
    fn show_cell_view(&mut self, ui: &mut egui::Ui, row: &QueryRow, column: usize) {
        if let Some(cell_value) = row.cells.get(column) {
            let truncated = Self::truncate_cell_text(cell_value);
            ui.label(truncated);
        } else {
            ui.label("");
        }
    }

    /// 显示单元格的编辑视图（查询结果表格为只读，不实现编辑功能）
    /// 单元格内容最长显示12个字符，超过部分使用省略号
    fn show_cell_editor(
        &mut self,
        ui: &mut egui::Ui,
        row: &mut QueryRow,
        column: usize,
    ) -> Option<egui::Response> {
        // 查询结果表格是只读的，所以直接显示只读视图
        if let Some(cell_value) = row.cells.get(column) {
            let truncated = Self::truncate_cell_text(cell_value);
            Some(ui.label(truncated))
        } else {
            Some(ui.label(""))
        }
    }

    /// 设置单元格的值（查询结果表格为只读，不实现）
    fn set_cell_value(&mut self, src: &QueryRow, dst: &mut QueryRow, column: usize) {
        if let Some(value) = src.cells.get(column) {
            if let Some(dst_cell) = dst.cells.get_mut(column) {
                *dst_cell = value.clone();
            }
        }
    }

    /// 创建新的空行
    fn new_empty_row(&mut self) -> QueryRow {
        QueryRow {
            cells: vec![String::new(); self.columns.len()],
        }
    }

    /// 返回列名
    /// 如果是主键列，会在列名前添加钥匙图标
    fn column_name(&mut self, column: usize) -> std::borrow::Cow<'static, str> {
        if let Some(col_name) = self.columns.get(column) {
            if self.primary_keys.contains(col_name) {
                // 主键列：返回带钥匙图标的列名
                format!("{} {}", egui_phosphor::regular::KEY, col_name)
                    .into()
            } else {
                col_name.clone().into()
            }
        } else {
            "".into()
        }
    }

    /// 单元格是否可编辑（查询结果表格为只读）
    fn is_editable_cell(&mut self, _column: usize, _row: usize, _row_value: &QueryRow) -> bool {
        false
    }

    /// 是否允许行插入（查询结果表格不允许）
    fn allow_row_insertions(&mut self) -> bool {
        false
    }

    /// 是否允许行删除（查询结果表格不允许）
    fn allow_row_deletions(&mut self) -> bool {
        false
    }
}

impl ResultsTable {
    /// 执行 SQL 查询并更新结果
    /// 从 SqlCtx 获取数据库连接，执行 SQL 语句，并将结果存储到表格中
    fn execute_sql(&mut self, sql: &str, ctxs: &mut SqlCtx) {
        self.sql_error = None;
        self.primary_key_columns.clear();

        // 获取数据库配置 ID
        let db_id = match &ctxs.db.active_db_config_id {
            Some(id) => id.clone(),
            None => {
                self.sql_error = Some("No database selected".into());
                return;
            }
        };

        ctxs.db.ensure_storage();
        let mut dsn = if let Some(ref storage) = ctxs.db.storage {
            match futures::block_on_async(async { storage.get_db_config(&db_id).await }) {
                Ok(Some(cfg)) => cfg.dsn,
                Ok(None) => {
                    self.sql_error = Some("Database config not found".into());
                    return;
                }
                Err(e) => {
                    self.sql_error = Some(format!("Failed to load database config: {}", e));
                    return;
                }
            }
        } else {
            self.sql_error = Some("Storage not initialized".into());
            return;
        };

        if dsn.trim().is_empty() {
            self.sql_error = Some("DSN is empty".into());
            return;
        }

        // 如果选择了不同的数据库，替换 DSN 中的数据库名
        if let Some(ref selected_db) = self.selected_database {
            if let Some(new_dsn) = utils::replace_database_in_dsn(&dsn, selected_db) {
                dsn = new_dsn;
            } else {
                self.sql_error = Some(format!(
                    "Failed to replace database in DSN: {}",
                    selected_db
                ));
                return;
            }
        }

        // 使用 DSN 的哈希值作为连接池的键
        let pool_key = utils::calculate_dsn_hash(&dsn);

        // 获取或创建连接池
        let pool = if let Some(p) = ctxs.db.pools.get(&pool_key).cloned() {
            p
        } else {
            let new_pool_result = futures::block_on_async(async {
                PgPoolOptions::new()
                    .max_connections(1)
                    .connect(&dsn)
                    .await
                    .map_err(|e| e.to_string())
            });
            match new_pool_result {
                Ok(new_pool) => {
                    ctxs.db.pools.insert(pool_key, new_pool.clone());
                    new_pool
                }
                Err(e) => {
                    self.sql_error = Some(format!("Failed to create connection pool: {}", e));
                    return;
                }
            }
        };

        // 尝试检测主键列
        let pk_cols = self.detect_primary_keys(sql, &dsn, &Some(pool.clone()));

        // 执行 SQL 查询
        let res: Result<(Vec<String>, Vec<Vec<String>>), String> =
            futures::block_on_async(async move {
                let rows: Vec<PgRow> = sqlx::query(sql)
                    .fetch_all(&pool)
                    .await
                    .map_err(|e| e.to_string())?;
                let mut cols: Vec<String> = Vec::new();
                let mut data: Vec<Vec<String>> = Vec::new();
                if let Some(first) = rows.first() {
                    for c in first.columns() {
                        cols.push(c.name().to_string());
                    }
                }
                for row in rows.into_iter().take(10000) {
                    let mut r: Vec<String> = Vec::new();
                    let n = if cols.is_empty() {
                        row.len()
                    } else {
                        cols.len()
                    };
                    for i in 0..n {
                        r.push(crate::sql::format_cell(&row, i));
                    }
                    data.push(r);
                }
                Ok((cols, data))
            });

        match res {
            Ok((cols, rows)) => {
                self.query_columns = cols;
                self.query_rows = rows;
                if let Some(pk) = pk_cols {
                    self.primary_key_columns = pk;
                }
            }
            Err(e) => {
                self.sql_error = Some(e);
            }
        }
    }

    /// 检测 SQL 查询中的主键列
    fn detect_primary_keys(
        &self,
        sql: &str,
        dsn: &str,
        pool_opt: &Option<sqlx::PgPool>,
    ) -> Option<HashSet<String>> {
        // 解析 SQL 提取表名
        let dialect = sqlparser::dialect::PostgreSqlDialect {};
        let ast = sqlparser::parser::Parser::parse_sql(&dialect, sql).ok()?;

        // 从 SELECT 语句中提取表名
        let mut table_names = Vec::new();
        for stmt in ast {
            if let sqlparser::ast::Statement::Query(query) = stmt {
                if let sqlparser::ast::SetExpr::Select(select) = &*query.body {
                    for table_with_joins in &select.from {
                        if let sqlparser::ast::TableFactor::Table { name, .. } =
                            &table_with_joins.relation
                        {
                            let schema = name.0.first().map(|i| i.value.clone());
                            let table = name.0.last().map(|i| i.value.clone());
                            match (schema, table) {
                                (Some(s), Some(t)) => {
                                    table_names.push((s, t));
                                }
                                (None, Some(t)) => {
                                    table_names.push(("public".to_string(), t));
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        if table_names.is_empty() {
            return None;
        }

        // 查询第一个表的主键信息（简单情况）
        if let Some((schema, table)) = table_names.first() {
            let pk_result = futures::block_on_async(async {
                let pool = match pool_opt {
                    Some(p) => p.clone(),
                    None => PgPoolOptions::new()
                        .max_connections(1)
                        .connect(dsn)
                        .await
                        .ok()?,
                };

                let pk_query = "SELECT kcu.column_name \
                        FROM information_schema.table_constraints tc \
                        JOIN information_schema.key_column_usage kcu \
                          ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema \
                        WHERE tc.constraint_type = 'PRIMARY KEY' AND tc.table_schema = $1 AND tc.table_name = $2 \
                        ORDER BY kcu.ordinal_position";

                let rows: Result<Vec<sqlx::postgres::PgRow>, _> = sqlx::query(pk_query)
                    .bind(schema)
                    .bind(table)
                    .fetch_all(&pool)
                    .await;

                rows.ok().map(|rows| {
                    rows.into_iter()
                        .map(|r| r.get::<String, _>(0))
                        .collect::<HashSet<String>>()
                })
            });

            pk_result
        } else {
            None
        }
    }

    /// 渲染查询结果表格
    /// 接收 SQL 语句和 SqlCtx，内部执行 SQL 并渲染结果
    /// 支持主键列标识、CSV 导出和自动刷新
    pub fn ui_results_table(
        &mut self,
        ui: &mut egui::Ui,
        sql: Option<&str>,
        ctxs: Option<&mut SqlCtx>,
        show_refresh: bool,
    ) {
        // 更新当前 SQL 语句（但不自动执行）
        if let Some(sql_str) = sql {
            // 只更新当前 SQL，不自动执行
            let sql_changed = self.current_sql.as_ref().map(|s| s != sql_str).unwrap_or(true);
            if sql_changed {
                self.current_sql = Some(sql_str.to_string());
                self.previous_sql = self.current_sql.clone();
            }
        }

        // 检查是否需要刷新
        let should_refresh = self.refresh_requested;
        if should_refresh {
            self.refresh_requested = false;
        }

        // 检查是否有执行请求（通过点击运行按钮触发）
        let should_execute_requested = self.execute_sql_requested;
        if should_execute_requested {
            self.execute_sql_requested = false;
        }

        // 执行 SQL（仅在点击运行按钮或刷新按钮时执行，不自动执行）
        if (should_refresh || should_execute_requested) && sql.is_some() {
            if let Some(ctxs) = ctxs {
                self.execute_sql(sql.unwrap(), ctxs);
            }
        }

        // 顶部工具栏：标题、刷新按钮、CSV 导出按钮
        ui.horizontal(|ui| {
            ui.heading(format!("{} Results", egui_phosphor::regular::TABLE));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if show_refresh {
                    if ui.button(egui_phosphor::regular::ARROW_CLOCKWISE).clicked() {
                        self.refresh_requested = true;
                    }
                    ui.add_space(8.0);
                }
                if ui.button("Export CSV...").clicked() {
                    self.export_csv(&self.query_columns, &self.query_rows);
                }
            });
        });
        ui.separator();

        // SQL 语句预览工具栏
        ui.horizontal(|ui| {
            if let Some(ref sql_str) = self.current_sql {
                // 只显示第一行，最多 100 个字符
                let first_line = sql_str.lines().next().unwrap_or("");
                let truncated_sql = if first_line.len() > 100 {
                    format!("{}...", &first_line[..100])
                } else {
                    first_line.to_string()
                };
                ui.label(
                    egui::RichText::new(truncated_sql)
                        .color(egui::Color32::GRAY),
                );
            } else {
                ui.label(
                    egui::RichText::new("No SQL statement")
                        .color(egui::Color32::GRAY),
                );
            }
        });
        ui.separator();

        // 显示错误信息（如果有）
        if let Some(ref error) = self.sql_error {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("{} Error: {}", egui_phosphor::regular::WARNING, error))
                        .color(egui::Color32::RED),
                );
            });
            ui.separator();
        }

        // 如果没有查询结果，显示空状态
        if self.query_columns.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(format!("{} No results", egui_phosphor::regular::EMPTY));
            });
            return;
        }

        debug!("query_columns: {:?}", self.query_columns);
        debug!("query_rows: {:?}", self.query_rows);

        // 将查询结果转换为 QueryRow 格式
        let table_data: Vec<QueryRow> = self
            .query_rows
            .iter()
            .map(|row| QueryRow {
                cells: row.clone(),
            })
            .collect();

        // 创建 DataTable 实例
        // DataTable 包装 Vec，提供表格数据管理功能
        // 使用 FromIterator trait 从 Vec 创建 DataTable
        let mut data_table: DataTable<QueryRow> = table_data.into_iter().collect();

        // 创建 RowViewer 实例
        // RowViewer 定义了如何显示和渲染表格的每一行
        let mut viewer = QueryRowViewer {
            columns: self.query_columns.clone(),
            primary_keys: self.primary_key_columns.clone(),
        };

        // 使用 egui-data-table 的 Renderer 渲染表格
        // Renderer 负责实际的 UI 渲染，包括列头、单元格、滚动等
        // Renderer::new 接受 DataTable 和 RowViewer 的引用，然后调用 show() 方法渲染
        Renderer::new(&mut data_table, &mut viewer).show(ui);
    }

    /// 导出查询结果为 CSV 文件
    /// 使用文件对话框选择保存位置，然后将查询结果写入 CSV 文件
    pub fn export_csv(&self, columns: &[String], rows: &[Vec<String>]) {
        if columns.is_empty() {
            return;
        }

        if rfd::FileDialog::new()
            .set_title("Save CSV")
            .add_filter("CSV", &["csv"])
            .save_file()
            .and_then(|path| csv::Writer::from_path(&path).ok())
            .map(|mut wtr| {
                let _ = wtr.write_record(columns);
                for row in rows {
                    let _ = wtr.write_record(row);
                }
                let _ = wtr.flush();
            })
            .is_some()
        {}
    }
}

