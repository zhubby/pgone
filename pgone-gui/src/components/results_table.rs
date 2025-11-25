use crate::components::SqlCtx;
use sqlx::postgres::{PgPoolOptions, PgRow};
use sqlx::{Column, Row};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct ResultsTable {
    // Filter and pagination
    pub filter_values: HashMap<usize, String>,
    pub refresh_requested: bool,
    pub current_sql: Option<String>,
    pub previous_sql: Option<String>,
    pub current_page: usize,
    pub rows_per_page: usize,
    
    // SQL editor fields
    pub sql_input: String,
    pub sql_error: Option<String>,
    
    // SQL execution fields
    pub query_columns: Vec<String>,
    pub query_rows: Vec<Vec<String>>,
    pub primary_key_columns: HashSet<String>,
    
    // Pagination enhancement fields
    pub page_size_options: Vec<usize>,
    pub page_jump_input: String,
    
    // Field display enhancement fields
    pub column_widths: HashMap<String, f32>,
    pub sort_column: Option<String>,
    pub sort_ascending: bool,
    
    // SQL execution flag
    pub execute_sql_requested: bool,
}

impl ResultsTable {
    pub fn new() -> Self {
        Self {
            filter_values: HashMap::new(),
            refresh_requested: false,
            current_sql: None,
            previous_sql: None,
            current_page: 1,
            rows_per_page: 100,
            sql_input: String::new(),
            sql_error: None,
            query_columns: Vec::new(),
            query_rows: Vec::new(),
            primary_key_columns: HashSet::new(),
            page_size_options: vec![10, 25, 50, 100, 200, 500],
            page_jump_input: String::new(),
            column_widths: HashMap::new(),
            sort_column: None,
            sort_ascending: true,
            execute_sql_requested: false,
        }
    }

    pub fn watch_ui(&mut self, _ui: &mut egui::Ui, _pipe: &mut ()) {
    }

    /// Render SQL editor with syntax highlighting
    pub fn ui_sql_editor(&mut self, ui: &mut egui::Ui, show_execute: bool) {
        ui.horizontal(|ui| {
            ui.heading(format!("{} SQL Editor", egui_phosphor::regular::CODE));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if show_execute {
                    if ui
                        .add(
                            egui::Button::new(egui_phosphor::regular::PLAY)
                                .min_size(egui::vec2(28.0, 28.0)),
                        )
                        .clicked()
                    {
                        self.execute_sql_requested = true;
                    }
                    ui.add_space(8.0);
                }
                if ui
                    .add(
                        egui::Button::new(egui_phosphor::regular::CHECK)
                            .min_size(egui::vec2(28.0, 28.0)),
                    )
                    .clicked()
                {
                    self.check_sql();
                }
            });
        });
        ui.separator();
        
        let current_sql = self.sql_input.clone();
        // Use available height minus header and separator space
        let available_height = ui.available_height() - 10.0;
        let editor = ui.add_sized(
            egui::Vec2::new(ui.available_width(), available_height),
            egui::TextEdit::multiline(&mut self.sql_input)
                .desired_rows((available_height / 20.0) as usize)
                .layouter(&mut move |ui, _text, wrap_width| {
                    let mut job = crate::sql::highlight_sql(&current_sql, ui.visuals());
                    job.wrap.max_width = wrap_width;
                    ui.fonts(|f| f.layout_job(job))
                }),
        );
        
        if let Some(err) = &self.sql_error {
            ui.colored_label(egui::Color32::RED, err);
        }
        
        if editor.changed() {
            self.sql_error = None;
        }
    }

    /// Check SQL syntax
    pub fn check_sql(&mut self) {
        self.sql_error = None;
        let dialect = sqlparser::dialect::PostgreSqlDialect {};
        match sqlparser::parser::Parser::parse_sql(&dialect, &self.sql_input) {
            Ok(_) => {
                self.sql_error = None;
            }
            Err(e) => {
                self.sql_error = Some(format!("{}", e));
            }
        }
    }

    /// Execute SQL query
    pub fn run_sql(&mut self, ctxs: &mut SqlCtx) {
        self.sql_error = None;
        self.primary_key_columns.clear();
        
        // Get DSN from active database config instead of session
        let db_id = match &ctxs.db.active_db_config_id {
            Some(id) => id.clone(),
            None => {
                self.sql_error = Some("No database selected".into());
                return;
            }
        };
        
        ctxs.db.ensure_storage();
        let dsn = if let Some(ref storage) = ctxs.db.storage {
            match tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    storage.get_db_config(&db_id).await
                })
            }) {
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
        
        let sql = self.sql_input.clone();
        // Use a hash of the db_id as the pool key
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        db_id.hash(&mut hasher);
        let pool_key = hasher.finish();
        let pool_opt = ctxs.db.pools.get(&pool_key).cloned();
        
        // Try to detect primary key columns from SQL query
        let pk_cols = self.detect_primary_keys(&sql, &dsn, &pool_opt);
        
        let res: Result<(Vec<String>, Vec<Vec<String>>), String> = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let pool = match pool_opt {
                    Some(p) => p,
                    None => PgPoolOptions::new()
                        .max_connections(1)
                        .connect(&dsn)
                        .await
                        .map_err(|e| e.to_string())?,
                };
                let rows: Vec<PgRow> = sqlx::query(&sql)
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
            })
        });
        
        match res {
            Ok((cols, rows)) => {
                self.query_columns = cols;
                self.query_rows = rows;
                // Update primary key columns if detected
                if let Some(pk) = pk_cols {
                    self.primary_key_columns = pk;
                }
                // Reset to first page after new query
                self.current_page = 1;
                self.current_sql = Some(self.sql_input.clone());
            }
            Err(e) => {
                self.sql_error = Some(e);
            }
        }
    }

    /// Detect primary key columns from SQL query
    fn detect_primary_keys(&self, sql: &str, dsn: &str, pool_opt: &Option<sqlx::PgPool>) -> Option<HashSet<String>> {
        // Parse SQL to extract table names
        let dialect = sqlparser::dialect::PostgreSqlDialect {};
        let ast = sqlparser::parser::Parser::parse_sql(&dialect, sql).ok()?;
        
        // Extract table names from SELECT statements
        let mut table_names = Vec::new();
        for stmt in ast {
            if let sqlparser::ast::Statement::Query(query) = stmt {
                // Extract from SetExpr::Select
                if let sqlparser::ast::SetExpr::Select(select) = &*query.body {
                    for table_with_joins in &select.from {
                        if let sqlparser::ast::TableFactor::Table { name, .. } = &table_with_joins.relation {
                            let schema = name.0.first().map(|i| i.value.clone());
                            let table = name.0.last().map(|i| i.value.clone());
                            match (schema, table) {
                                (Some(s), Some(t)) => {
                                    table_names.push((s, t));
                                }
                                (None, Some(t)) => {
                                    // Default to public schema if no schema specified
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
        
        // Query primary key information for the first table (simple case)
        // For JOIN queries, we only check the first table
        if let Some((schema, table)) = table_names.first() {
            let pk_result = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
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
                })
            });
            
            pk_result
        } else {
            None
        }
    }

    /// Sort rows by column
    fn sort_rows(&mut self, column: &str) {
        if let Some(col_idx) = self.query_columns.iter().position(|c| c == column) {
            let ascending = if self.sort_column.as_ref().map(|s| s == column).unwrap_or(false) {
                !self.sort_ascending
            } else {
                true
            };
            
            self.sort_column = Some(column.to_string());
            self.sort_ascending = ascending;
            
            self.query_rows.sort_by(|a, b| {
                let a_val = a.get(col_idx).map(|s| s.as_str()).unwrap_or("");
                let b_val = b.get(col_idx).map(|s| s.as_str()).unwrap_or("");
                let cmp = a_val.cmp(b_val);
                if ascending {
                    cmp
                } else {
                    cmp.reverse()
                }
            });
            
            // Reset to first page after sorting
            self.current_page = 1;
        }
    }

    fn truncate_text(ui: &egui::Ui, text: &str, max_width: f32) -> String {
        let font_id = egui::TextStyle::Body.resolve(ui.style());
        let text_width = ui.fonts(|f| {
            let galley = f.layout_no_wrap(text.to_string(), font_id.clone(), egui::Color32::GRAY);
            galley.size().x
        });
        
        if text_width <= max_width {
            text.to_string()
        } else {
            let ellipsis = "...";
            let ellipsis_width = ui.fonts(|f| {
                let galley = f.layout_no_wrap(ellipsis.to_string(), font_id.clone(), egui::Color32::GRAY);
                galley.size().x
            });
            let available_width = max_width - ellipsis_width;
            
            // Binary search for the right truncation point
            let mut low = 0;
            let mut high = text.len();
            while low < high {
                let mid = (low + high + 1) / 2;
                let truncated = &text[..mid];
                let width = ui.fonts(|f| {
                    let galley = f.layout_no_wrap(truncated.to_string(), font_id.clone(), egui::Color32::GRAY);
                    galley.size().x
                });
                if width <= available_width {
                    low = mid;
                } else {
                    high = mid - 1;
                }
            }
            format!("{}...", &text[..low])
        }
    }

    /// Main UI method - unified entry point with SQL editor and results table
    pub fn ui(&mut self, ui: &mut egui::Ui, mut ctxs: Option<&mut SqlCtx>) {
        // Check if refresh was requested
        if self.refresh_requested {
            self.refresh_requested = false;
            if let Some(ctxs) = ctxs.as_mut() {
                self.run_sql(ctxs);
            }
        }

        // Check if SQL execution was requested
        if self.execute_sql_requested {
            self.execute_sql_requested = false;
            if let Some(ctxs) = ctxs.as_mut() {
                self.run_sql(ctxs);
            }
        }

        let has_ctxs = ctxs.is_some();

        // SQL Editor section - fixed height at 1/4 of window height, filling horizontally
        let window_height = ui.ctx().screen_rect().height();
        let editor_height = window_height / 4.0;
        
        ui.allocate_ui_with_layout(
            egui::Vec2::new(ui.available_width(), editor_height),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                self.ui_sql_editor(ui, has_ctxs);
            }
        );
        
        ui.separator();

        // Results section
        self.ui_results_table(ui, has_ctxs);
    }

    /// Render results table with enhanced pagination and field display
    fn ui_results_table(&mut self, ui: &mut egui::Ui, show_refresh: bool) {
        // Update current SQL statement
        let new_sql = Some(self.sql_input.clone());
        
        // Reset to first page if SQL statement changed
        if self.previous_sql != new_sql {
            self.current_page = 1;
            self.previous_sql = new_sql.clone();
        }
        
        self.current_sql = new_sql;
        
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
        
        // Toolbar with SQL statement and pagination
        ui.horizontal(|ui| {
            // Display SQL statement (truncated if too long)
            if let Some(ref sql) = self.current_sql {
                let available_width = ui.available_width() - 400.0; // Reserve space for pagination controls
                let truncated_sql = Self::truncate_text(ui, sql, available_width.max(100.0));
                ui.label(egui::RichText::new(truncated_sql).color(egui::Color32::GRAY).small());
            } else {
                ui.label(egui::RichText::new("No SQL statement").color(egui::Color32::GRAY).small());
            }
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Enhanced pagination controls
                let total_rows = self.query_rows.len();
                let rows_per_page = self.rows_per_page.max(1);
                let total_pages = if total_rows == 0 { 1 } else { (total_rows + rows_per_page - 1) / rows_per_page };
                
                // Ensure current_page is valid
                if total_pages > 0 {
                    if self.current_page > total_pages {
                        self.current_page = total_pages;
                    }
                    if self.current_page < 1 {
                        self.current_page = 1;
                    }
                } else {
                    self.current_page = 1;
                }
                
                // Page size selector
                egui::ComboBox::from_id_salt("page_size")
                    .selected_text(format!("{} / 页", self.rows_per_page))
                    .show_ui(ui, |ui| {
                        for &size in &self.page_size_options {
                            if ui.selectable_value(&mut self.rows_per_page, size, format!("{} / 页", size)).clicked() {
                                self.current_page = 1; // Reset to first page when changing page size
                            }
                        }
                    });
                ui.add_space(8.0);
                
                // Page jump input
                ui.add(egui::TextEdit::singleline(&mut self.page_jump_input)
                    .desired_width(50.0)
                    .hint_text("页码"));
                if ui.button("跳转").clicked() {
                    if let Ok(page_num) = self.page_jump_input.parse::<usize>() {
                        if page_num >= 1 && page_num <= total_pages {
                            self.current_page = page_num;
                            self.page_jump_input.clear();
                        }
                    }
                }
                ui.add_space(8.0);
                
                // Page info
                if total_rows > 0 {
                    let start_row = (self.current_page - 1) * rows_per_page + 1;
                    let end_row = (start_row + rows_per_page - 1).min(total_rows);
                    ui.label(format!("{} - {} / {}", start_row, end_row, total_rows));
                    ui.add_space(8.0);
                    
                    // Next page button
                    if ui.add_enabled(self.current_page < total_pages, egui::Button::new(egui_phosphor::regular::CARET_RIGHT)).clicked() {
                        if self.current_page < total_pages {
                            self.current_page += 1;
                        }
                    }
                    
                    // Previous page button
                    if ui.add_enabled(self.current_page > 1, egui::Button::new(egui_phosphor::regular::CARET_LEFT)).clicked() {
                        if self.current_page > 1 {
                            self.current_page -= 1;
                        }
                    }
                    
                    ui.add_space(4.0);
                    
                    // First page button
                    if ui.add_enabled(self.current_page > 1, egui::Button::new(egui_phosphor::regular::CARET_DOUBLE_LEFT)).clicked() {
                        self.current_page = 1;
                    }
                    
                    // Last page button
                    if ui.add_enabled(self.current_page < total_pages, egui::Button::new(egui_phosphor::regular::CARET_DOUBLE_RIGHT)).clicked() {
                        self.current_page = total_pages;
                    }
                } else {
                    ui.label("0 / 0");
                }
            });
        });
        ui.separator();
        
        if self.query_columns.is_empty() {
            ui.label("No results");
            return;
        }

        // Calculate pagination
        let total_rows = self.query_rows.len();
        let total_pages = if self.rows_per_page > 0 {
            (total_rows + self.rows_per_page - 1) / self.rows_per_page
        } else {
            1
        };
        
        // Ensure current_page is valid
        if self.current_page > total_pages.max(1) {
            self.current_page = total_pages.max(1);
        }
        if self.current_page < 1 {
            self.current_page = 1;
        }
        
        // Get current page rows
        let start_idx = if total_rows == 0 { 0 } else { (self.current_page - 1) * self.rows_per_page };
        let end_idx = (start_idx + self.rows_per_page).min(total_rows);
        let page_rows = if start_idx < total_rows {
            &self.query_rows[start_idx..end_idx]
        } else {
            &[]
        };
        
        let available_height = ui.available_height() - 40.0;
        let row_height = 20.0;
        let max_visible_rows = (available_height / row_height).floor() as usize;
        
        let pk_cols: Vec<String> = self.primary_key_columns.iter().cloned().collect();
        let sort_column = self.sort_column.clone();
        let sort_ascending = self.sort_ascending;
        let query_columns = self.query_columns.clone();
        
        // Track which column was clicked for sorting
        let mut clicked_column: Option<String> = None;
        
        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                egui::Grid::new("results_table")
                    .striped(true)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        // Header row - add blank column at the beginning
                        ui.label(""); // Blank column
                        for col in &query_columns {
                            let is_sorted = sort_column.as_ref().map(|s| s == col).unwrap_or(false);
                            let sort_indicator = if is_sorted {
                                if sort_ascending {
                                    egui_phosphor::regular::CARET_UP
                                } else {
                                    egui_phosphor::regular::CARET_DOWN
                                }
                            } else {
                                ""
                            };
                            
                            let response = ui.horizontal(|ui| {
                                // Show key icon for primary key columns
                                if pk_cols.contains(col) {
                                    ui.label(egui_phosphor::regular::KEY);
                                    ui.add_space(4.0);
                                }
                                ui.strong(col);
                                if is_sorted {
                                    ui.label(sort_indicator);
                                }
                            }).response;
                            
                            // Track clicked column for sorting (outside closure)
                            if response.clicked() {
                                clicked_column = Some(col.clone());
                            }
                        }
                        ui.end_row();
                        
                        // Data rows - add blank cell at the beginning of each row
                        for row in page_rows {
                            ui.label(""); // Blank cell
                            for cell in row {
                                ui.label(cell);
                            }
                            ui.end_row();
                        }
                        
                        // Empty rows for better visibility
                        let data_rows = page_rows.len();
                        if data_rows < max_visible_rows {
                            let empty_rows_needed = max_visible_rows - data_rows;
                            for _ in 0..empty_rows_needed {
                                ui.label(""); // Blank cell
                                for _ in &query_columns {
                                    ui.label("");
                                }
                                ui.end_row();
                            }
                        }
                    });
            });
        
        // Apply sorting outside the closure
        if let Some(col) = clicked_column {
            self.sort_rows(&col);
        }
    }

    /// Legacy UI method for backward compatibility
    pub fn ui_legacy(&mut self, ui: &mut egui::Ui, columns: &[String], rows: &[Vec<String>], primary_key_columns: Option<&HashSet<String>>, show_refresh: bool, sql: Option<&str>) {
        // Update current SQL statement
        let new_sql = sql.map(|s| s.to_string());
        
        // Reset to first page if SQL statement changed
        if self.previous_sql != new_sql {
            self.current_page = 1;
            self.previous_sql = new_sql.clone();
        }
        
        self.current_sql = new_sql;
        ui.horizontal(|ui| {
            ui.heading("Results");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if show_refresh {
                    if ui.button(egui_phosphor::regular::ARROW_CLOCKWISE).clicked() {
                        self.refresh_requested = true;
                    }
                    ui.add_space(8.0);
                }
                if ui.button("Export CSV...").clicked() {
                    self.export_csv(columns, rows);
                }
            });
        });
        ui.separator();
        
        // Toolbar with SQL statement and pagination
        ui.horizontal(|ui| {
            // Display SQL statement (truncated if too long)
            if let Some(ref sql) = self.current_sql {
                let available_width = ui.available_width() - 200.0; // Reserve space for pagination buttons
                let truncated_sql = Self::truncate_text(ui, sql, available_width.max(100.0));
                ui.label(egui::RichText::new(truncated_sql).color(egui::Color32::GRAY).small());
            } else {
                ui.label(egui::RichText::new("No SQL statement").color(egui::Color32::GRAY).small());
            }
            
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Pagination controls
                let total_rows = rows.len();
                let rows_per_page = self.rows_per_page.max(1);
                let total_pages = if total_rows == 0 { 1 } else { (total_rows + rows_per_page - 1) / rows_per_page };
                
                // Ensure current_page is valid
                if total_pages > 0 {
                    if self.current_page > total_pages {
                        self.current_page = total_pages;
                    }
                    if self.current_page < 1 {
                        self.current_page = 1;
                    }
                } else {
                    self.current_page = 1;
                }
                
                // Page info
                if total_rows > 0 {
                    let start_row = (self.current_page - 1) * rows_per_page + 1;
                    let end_row = (start_row + rows_per_page - 1).min(total_rows);
                    ui.label(format!("{} - {} / {}", start_row, end_row, total_rows));
                    ui.add_space(8.0);
                    
                    // Next page button
                    if ui.add_enabled(self.current_page < total_pages, egui::Button::new(egui_phosphor::regular::CARET_RIGHT)).clicked() {
                        if self.current_page < total_pages {
                            self.current_page += 1;
                        }
                    }
                    
                    // Previous page button
                    if ui.add_enabled(self.current_page > 1, egui::Button::new(egui_phosphor::regular::CARET_LEFT)).clicked() {
                        if self.current_page > 1 {
                            self.current_page -= 1;
                        }
                    }
                    
                    ui.add_space(4.0);
                    
                    // First page button
                    if ui.add_enabled(self.current_page > 1, egui::Button::new(egui_phosphor::regular::CARET_DOUBLE_LEFT)).clicked() {
                        self.current_page = 1;
                    }
                    
                    // Last page button
                    if ui.add_enabled(self.current_page < total_pages, egui::Button::new(egui_phosphor::regular::CARET_DOUBLE_RIGHT)).clicked() {
                        self.current_page = total_pages;
                    }
                } else {
                    ui.label("0 / 0");
                }
            });
        });
        ui.separator();
        
        if columns.is_empty() {
            ui.label("No results");
            return;
        }

        // Calculate pagination
        let total_rows = rows.len();
        let total_pages = if self.rows_per_page > 0 {
            (total_rows + self.rows_per_page - 1) / self.rows_per_page
        } else {
            1
        };
        
        // Ensure current_page is valid
        if self.current_page > total_pages.max(1) {
            self.current_page = total_pages.max(1);
        }
        if self.current_page < 1 {
            self.current_page = 1;
        }
        
        // Get current page rows
        let start_idx = if total_rows == 0 { 0 } else { (self.current_page - 1) * self.rows_per_page };
        let end_idx = (start_idx + self.rows_per_page).min(total_rows);
        let page_rows = if start_idx < total_rows {
            &rows[start_idx..end_idx]
        } else {
            &[]
        };
        
        let available_height = ui.available_height() - 40.0;
        let row_height = 20.0;
        let max_visible_rows = (available_height / row_height).floor() as usize;
        
        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                egui::Grid::new("results_table")
                    .striped(true)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        // Header row - add blank column at the beginning
                        ui.label(""); // Blank column
                        for col in columns {
                            ui.horizontal(|ui| {
                                // Show key icon for primary key columns
                                if let Some(pk_cols) = primary_key_columns {
                                    if pk_cols.contains(col) {
                                        ui.label(egui_phosphor::regular::KEY);
                                        ui.add_space(4.0);
                                    }
                                }
                                ui.strong(col);
                            });
                        }
                        ui.end_row();
                        
                        // Data rows - add blank cell at the beginning of each row
                        for row in page_rows {
                            ui.label(""); // Blank cell
                            for cell in row {
                                ui.label(cell);
                            }
                            ui.end_row();
                        }
                        
                        // Empty rows for better visibility
                        let data_rows = page_rows.len();
                        if data_rows < max_visible_rows {
                            let empty_rows_needed = max_visible_rows - data_rows;
                            for _ in 0..empty_rows_needed {
                                ui.label(""); // Blank cell
                                for _ in columns {
                                    ui.label("");
                                }
                                ui.end_row();
                            }
                        }
                    });
            });
    }

    fn export_csv(&self, columns: &[String], rows: &[Vec<String>]) {
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
