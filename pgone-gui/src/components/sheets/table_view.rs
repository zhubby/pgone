use super::ResultsTable;
use egui_data_table::{DataTable, Renderer, RowViewer};
use std::collections::HashSet;
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
    /// 渲染查询结果表格
    /// 使用 egui-data-table 组件显示查询结果，支持主键列标识和 CSV 导出
    pub fn ui_results_table(&mut self, ui: &mut egui::Ui, show_refresh: bool) {
        // 更新当前 SQL 语句
        let new_sql = Some(self.sql_input.clone());
        self.current_sql = new_sql.clone();
        self.previous_sql = new_sql;

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
            if let Some(ref sql) = self.current_sql {
                // 只显示第一行，最多 100 个字符
                let first_line = sql.lines().next().unwrap_or("");
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

