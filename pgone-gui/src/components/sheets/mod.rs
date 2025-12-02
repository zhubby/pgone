use crate::components::SqlCtx;
use poll_promise::Promise;
use std::collections::{HashMap, HashSet};
use tracing::debug;

mod utils;
mod sql_editor;
mod executor;
mod table_view;
mod database_loader;

#[derive(Default)]
pub struct ResultsTable {
    // Filter and pagination
    // pub filter_values: HashMap<usize, String>,
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

    // Database selection fields
    pub selected_database: Option<String>,
    pub available_databases: Vec<String>,
    pub databases_promise: Option<Promise<Result<Vec<String>, String>>>,
    pub current_db_id: Option<String>,
}

impl ResultsTable {
    pub fn new() -> Self {
        Self {
            // filter_values: HashMap::new(),
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
            },
        );

        ui.separator();

        // Results section
        self.ui_results_table(ui, has_ctxs);
    }
}

