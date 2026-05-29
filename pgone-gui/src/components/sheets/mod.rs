use super::db_manager::PoolRegistry;
use super::graph::SchemaGraph;
use crate::components::SqlCtx;
use poll_promise::Promise;
use serde_json::Value;
use std::collections::BTreeMap;
use std::collections::HashSet;
use tracing::debug;

mod database_loader;
mod ddl_viewer;
mod executor;
mod json_viewer;
mod sql_draft;
mod sql_editor;
mod table_view;
mod utils;

/// SQL execution plan information
#[derive(Clone, Default)]
pub struct ExplainInfo {
    /// Scan type (e.g., "Seq Scan", "Index Scan", "Hash Join")
    pub scan_type: String,
    /// Cost information (e.g., "0.00..1234.56")
    pub cost: String,
    /// Row count (e.g., "10000")
    pub rows: String,
}

pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub primary_key_columns: HashSet<String>,
    pub explain_info: Option<ExplainInfo>,
    pub explain_error: Option<String>,
    pub total_rows: Option<usize>,
    pub has_next_page: bool,
    pub pagination_enabled: bool,
}

pub const DEFAULT_RESULTS_PAGE_SIZE: usize = 100;

#[derive(Clone)]
pub struct JsonViewerTab {
    pub id: u64,
    pub title: String,
    pub value: Value,
    pub source_column: String,
    pub source_row: usize,
}

#[derive(Clone)]
pub struct DdlViewerTab {
    pub id: u64,
    pub title: String,
    pub ddl: String,
}

#[derive(Clone)]
pub struct SqlDraftTab {
    pub id: u64,
    pub title: String,
    pub sql: String,
    pub database: String,
}

#[derive(Clone)]
pub struct GraphViewerTabInfo {
    pub id: u64,
    pub title: String,
}

pub struct GraphViewerTab {
    pub id: u64,
    pub title: String,
    pub database: String,
    pub schema: String,
    pub dsn: String,
    pub graph: SchemaGraph,
}

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
    pub query_promise: Option<Promise<Result<QueryResult, String>>>,
    pub selected_result_row: Option<usize>,
    pub json_viewer_tabs: BTreeMap<u64, JsonViewerTab>,
    pub pending_json_viewer_tabs: Vec<JsonViewerTab>,
    pub next_json_viewer_tab_id: u64,
    pub ddl_viewer_tabs: BTreeMap<u64, DdlViewerTab>,
    pub pending_ddl_viewer_tabs: Vec<DdlViewerTab>,
    pub next_ddl_viewer_tab_id: u64,
    pub sql_draft_tabs: BTreeMap<u64, SqlDraftTab>,
    pub pending_sql_draft_tabs: Vec<SqlDraftTab>,
    pub next_sql_draft_tab_id: u64,
    pub graph_viewer_tabs: BTreeMap<u64, GraphViewerTab>,
    pub pending_graph_viewer_tabs: Vec<GraphViewerTabInfo>,
    pub next_graph_viewer_tab_id: u64,
    pub current_page: usize,
    pub page_size: usize,
    pub total_rows: Option<usize>,
    pub has_next_page: bool,
    pub pagination_enabled: bool,
    pub paged_base_sql: Option<String>,

    // SQL execution flag
    pub execute_sql_requested: bool,

    // EXPLAIN information
    pub explain_info: Option<ExplainInfo>,
    pub explain_error: Option<String>,

    // Database selection fields
    pub selected_database: Option<String>,
    pub available_databases: Vec<String>,
    pub databases_promise: Option<Promise<Result<Vec<String>, String>>>,
    pub current_db_id: Option<String>,

    // Auto-completion fields
    pub completion_suggestions: Vec<String>,
    pub completion_selected_index: usize,
    pub show_completion: bool,
    pub completion_prefix: String,
    pub completion_cursor_pos: usize,
    pub completion_word_start: usize,
    pub completion_word_end: usize,
    pub previous_sql_input: String, // Used to detect text change position
    pub pending_cursor_pos: Option<usize>, // Pending cursor position to set
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
            query_promise: None,
            selected_result_row: None,
            json_viewer_tabs: BTreeMap::new(),
            pending_json_viewer_tabs: Vec::new(),
            next_json_viewer_tab_id: 1,
            ddl_viewer_tabs: BTreeMap::new(),
            pending_ddl_viewer_tabs: Vec::new(),
            next_ddl_viewer_tab_id: 1,
            sql_draft_tabs: BTreeMap::new(),
            pending_sql_draft_tabs: Vec::new(),
            next_sql_draft_tab_id: 1,
            graph_viewer_tabs: BTreeMap::new(),
            pending_graph_viewer_tabs: Vec::new(),
            next_graph_viewer_tab_id: 1,
            current_page: 1,
            page_size: DEFAULT_RESULTS_PAGE_SIZE,
            total_rows: None,
            has_next_page: false,
            pagination_enabled: false,
            paged_base_sql: None,
            execute_sql_requested: false,
            explain_info: None,
            explain_error: None,
            selected_database: None,
            available_databases: Vec::new(),
            databases_promise: None,
            current_db_id: None,
            completion_suggestions: Vec::new(),
            completion_selected_index: 0,
            show_completion: false,
            completion_prefix: String::new(),
            completion_cursor_pos: 0,
            completion_word_start: 0,
            completion_word_end: 0,
            previous_sql_input: String::new(),
            pending_cursor_pos: None,
        }
    }

    pub fn watch_ui(&mut self, _ui: &mut egui::Ui, _pipe: &mut ()) {}

    pub fn open_json_viewer(
        &mut self,
        source_row: usize,
        source_column: &str,
        value: Value,
    ) -> u64 {
        let id = self.next_json_viewer_tab_id;
        self.next_json_viewer_tab_id = self.next_json_viewer_tab_id.saturating_add(1);
        let title = format!("JSON {}.{}", source_row + 1, source_column);
        let tab = JsonViewerTab {
            id,
            title,
            value,
            source_column: source_column.to_string(),
            source_row,
        };
        self.json_viewer_tabs.insert(id, tab.clone());
        self.pending_json_viewer_tabs.push(tab);
        id
    }

    pub fn take_pending_json_viewer_tabs(&mut self) -> Vec<JsonViewerTab> {
        std::mem::take(&mut self.pending_json_viewer_tabs)
    }

    pub fn open_ddl_viewer(&mut self, title: impl Into<String>, ddl: impl Into<String>) -> u64 {
        let id = self.next_ddl_viewer_tab_id;
        self.next_ddl_viewer_tab_id = self.next_ddl_viewer_tab_id.saturating_add(1);
        let tab = DdlViewerTab {
            id,
            title: title.into(),
            ddl: ddl.into(),
        };
        self.ddl_viewer_tabs.insert(id, tab.clone());
        self.pending_ddl_viewer_tabs.push(tab);
        id
    }

    pub fn take_pending_ddl_viewer_tabs(&mut self) -> Vec<DdlViewerTab> {
        std::mem::take(&mut self.pending_ddl_viewer_tabs)
    }

    pub fn open_sql_draft(
        &mut self,
        title: impl Into<String>,
        sql: impl Into<String>,
        database: impl Into<String>,
    ) -> u64 {
        let id = self.next_sql_draft_tab_id;
        self.next_sql_draft_tab_id = self.next_sql_draft_tab_id.saturating_add(1);
        let tab = SqlDraftTab {
            id,
            title: title.into(),
            sql: sql.into(),
            database: database.into(),
        };
        self.sql_draft_tabs.insert(id, tab.clone());
        self.pending_sql_draft_tabs.push(tab);
        id
    }

    pub fn take_pending_sql_draft_tabs(&mut self) -> Vec<SqlDraftTab> {
        std::mem::take(&mut self.pending_sql_draft_tabs)
    }

    pub fn open_graph_viewer(
        &mut self,
        database: impl Into<String>,
        schema: impl Into<String>,
        dsn: impl Into<String>,
    ) -> u64 {
        let database = database.into();
        let schema = schema.into();
        if let Some(tab) = self
            .graph_viewer_tabs
            .values()
            .find(|tab| tab.database == database && tab.schema == schema)
        {
            self.pending_graph_viewer_tabs.push(GraphViewerTabInfo {
                id: tab.id,
                title: tab.title.clone(),
            });
            return tab.id;
        }

        let id = self.next_graph_viewer_tab_id;
        self.next_graph_viewer_tab_id = self.next_graph_viewer_tab_id.saturating_add(1);
        let title = format!("Graph {}.{}", database, schema);
        let tab = GraphViewerTab {
            id,
            title: title.clone(),
            database: database.clone(),
            schema: schema.clone(),
            dsn: dsn.into(),
            graph: SchemaGraph::new(database, schema),
        };
        self.graph_viewer_tabs.insert(id, tab);
        self.pending_graph_viewer_tabs
            .push(GraphViewerTabInfo { id, title });
        id
    }

    pub fn take_pending_graph_viewer_tabs(&mut self) -> Vec<GraphViewerTabInfo> {
        std::mem::take(&mut self.pending_graph_viewer_tabs)
    }

    pub fn ddl_viewer_tab(&self, id: u64) -> Option<&DdlViewerTab> {
        self.ddl_viewer_tabs.get(&id)
    }

    pub fn sql_draft_tab(&self, id: u64) -> Option<&SqlDraftTab> {
        self.sql_draft_tabs.get(&id)
    }

    pub fn graph_viewer_tab(&self, id: u64) -> Option<&GraphViewerTab> {
        self.graph_viewer_tabs.get(&id)
    }

    pub fn json_viewer_tab(&self, id: u64) -> Option<&JsonViewerTab> {
        self.json_viewer_tabs.get(&id)
    }

    pub fn clear_json_viewer_tabs(&mut self) {
        self.json_viewer_tabs.clear();
        self.pending_json_viewer_tabs.clear();
    }

    pub fn retain_json_viewer_tabs(&mut self, keep_ids: &HashSet<u64>) {
        self.json_viewer_tabs.retain(|id, _| keep_ids.contains(id));
    }

    pub fn retain_ddl_viewer_tabs(&mut self, keep_ids: &HashSet<u64>) {
        self.ddl_viewer_tabs.retain(|id, _| keep_ids.contains(id));
    }

    pub fn retain_sql_draft_tabs(&mut self, keep_ids: &HashSet<u64>) {
        self.sql_draft_tabs.retain(|id, _| keep_ids.contains(id));
    }

    pub fn retain_graph_viewer_tabs(&mut self, keep_ids: &HashSet<u64>) {
        self.graph_viewer_tabs.retain(|id, _| keep_ids.contains(id));
    }

    pub fn ui_json_viewer(&self, ui: &mut egui::Ui, id: u64) {
        if let Some(tab) = self.json_viewer_tab(id) {
            json_viewer::ui(ui, tab);
        } else {
            ui.centered_and_justified(|ui| {
                ui.label("JSON viewer content is no longer available");
            });
        }
    }

    pub fn ui_ddl_viewer(&self, ui: &mut egui::Ui, id: u64) {
        if let Some(tab) = self.ddl_viewer_tab(id) {
            ddl_viewer::ui(ui, tab);
        } else {
            ui.centered_and_justified(|ui| {
                ui.label("DDL viewer content is no longer available");
            });
        }
    }

    pub fn ui_sql_draft(&mut self, ui: &mut egui::Ui, id: u64) {
        if self.sql_draft_tab(id).is_some() {
            sql_draft::ui(ui, self, id);
        } else {
            ui.centered_and_justified(|ui| {
                ui.label("SQL draft content is no longer available");
            });
        }
    }

    pub fn ui_graph_viewer(&mut self, ui: &mut egui::Ui, id: u64, pools: PoolRegistry) {
        if let Some(tab) = self.graph_viewer_tabs.get_mut(&id) {
            tab.graph.ui(ui, pools, Some(&tab.dsn));
        } else {
            ui.centered_and_justified(|ui| {
                ui.label("Graph content is no longer available");
            });
        }
    }

    pub fn sync_database_selection(&mut self, mut ctxs: Option<&mut SqlCtx>) {
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
    }

    /// Main UI method - unified entry point with SQL editor and results table
    pub fn ui(&mut self, ui: &mut egui::Ui, mut ctxs: Option<&mut SqlCtx>) {
        self.sync_database_selection(ctxs.as_deref_mut());

        // SQL execution is now handled internally by the table component
        // refresh_requested and execute_sql_requested will be handled in ui_results_table

        let has_ctxs = ctxs.is_some();

        // SQL Editor section - fixed height at 1/4 of window height, filling horizontally
        let window_height = ui.ctx().content_rect().height();
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
        // Pass SQL statement and context; the table handles execution and rendering internally
        // Clone SQL string to avoid borrow conflicts
        let sql = if self.sql_input.trim().is_empty() {
            None
        } else {
            Some(self.sql_input.clone())
        };
        self.ui_results_table(ui, sql.as_deref(), ctxs, has_ctxs);
    }
}
