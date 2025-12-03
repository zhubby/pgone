use crate::components::SqlCtx;
use poll_promise::Promise;
use std::collections::HashSet;
use tracing::debug;

mod utils;
mod sql_editor;
mod executor;
mod table_view;
mod database_loader;

#[derive(Default)]
pub struct ResultsTable {
    // Refresh control
    pub refresh_requested: bool,
    pub current_sql: Option<String>,
    pub previous_sql: Option<String>,

    // SQL editor fields
    pub sql_input: String,
    pub sql_error: Option<String>,

    // SQL execution fields
    pub query_columns: Vec<String>,
    pub query_rows: Vec<Vec<String>>,
    pub primary_key_columns: HashSet<String>,

    // SQL execution flag
    pub execute_sql_requested: bool,

    // Database selection fields
    pub selected_database: Option<String>,
    pub available_databases: Vec<String>,
    pub databases_promise: Option<Promise<Result<Vec<String>, String>>>,
    pub current_db_id: Option<String>,
}

impl ResultsTable {
    pub fn new() -> Self {
        Self {
            refresh_requested: false,
            current_sql: None,
            previous_sql: None,
            sql_input: String::new(),
            sql_error: None,
            query_columns: Vec::new(),
            query_rows: Vec::new(),
            primary_key_columns: HashSet::new(),
            execute_sql_requested: false,
            selected_database: None,
            available_databases: Vec::new(),
            databases_promise: None,
            current_db_id: None,
        }
    }

    pub fn watch_ui(&mut self, _ui: &mut egui::Ui, _pipe: &mut ()) {}

    /// Main UI method - unified entry point with SQL editor and results table
    pub fn ui(&mut self, ui: &mut egui::Ui, mut ctxs: Option<&mut SqlCtx>) {
        // Check if database config changed and load databases
        if let Some(ctxs) = ctxs.as_mut() {
            let current_db_id = ctxs.db.active_db_config_id.clone();
            if current_db_id != self.current_db_id {
                self.current_db_id = current_db_id.clone();
                self.selected_database = None; // Reset selection when DB config changes
                if current_db_id.is_some() {
                    self.load_databases(ctxs);
                } else {
                    self.available_databases.clear();
                    self.databases_promise = None;
                }
            }
        }

        // Check for database list loading completion
        if let Some(promise) = &self.databases_promise {
            if let Some(result) = promise.ready() {
                match result {
                    Ok(databases) => {
                        self.available_databases = databases.clone();
                    }
                    Err(e) => {
                        debug!("Failed to load databases: {}", e);
                        self.available_databases.clear();
                    }
                }
                self.databases_promise = None;
            }
        }

        // SQL 执行现在由表格组件内部处理
        // refresh_requested 和 execute_sql_requested 会在 ui_results_table 中处理

        let has_ctxs = ctxs.is_some();

        // SQL Editor section - fixed height at 1/4 of window height, filling horizontally
        let window_height = ui.ctx().screen_rect().height();
        let editor_height = window_height / 4.0;

        ui.allocate_ui_with_layout(
            egui::Vec2::new(ui.available_width(), editor_height),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                self.ui_sql_editor(ui, has_ctxs);
            },
        );

        ui.separator();

        // Results section
        // 传递 SQL 语句和上下文，表格内部负责执行和渲染
        // 克隆 SQL 字符串以避免借用冲突
        let sql = if self.sql_input.trim().is_empty() {
            None
        } else {
            Some(self.sql_input.clone())
        };
        self.ui_results_table(ui, sql.as_deref(), ctxs, has_ctxs);
    }
}

