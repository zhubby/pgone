use crate::components::{
    ChatCtx, ChatPanel, DbManager, DbTree, PreviewManager, ResultsTable, SqlCtx,
};
use crate::models::PersistedState;
use crate::storage::SessionStorage;
use eframe::egui::{Ui, WidgetText};
use egui_dock::{DockArea, DockState, NodeIndex, Style, TabViewer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DockTab {
    DatabaseStructure,
    SqlWorkspace,
    Chat,
}

impl DockTab {
    fn title(&self) -> &'static str {
        match self {
            Self::DatabaseStructure => "Database Structure",
            Self::SqlWorkspace => "SQL Workspace",
            Self::Chat => "Chat",
        }
    }
}

pub struct DockLayout {
    state: DockState<DockTab>,
}

impl Default for DockLayout {
    fn default() -> Self {
        Self {
            state: Self::default_state(),
        }
    }
}

impl DockLayout {
    pub fn reset(&mut self) {
        self.state = Self::default_state();
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        db_tree: &mut DbTree,
        db: &mut DbManager,
        results_table: &mut ResultsTable,
        chat: &mut ChatPanel,
        state: &mut PersistedState,
        preview: &mut PreviewManager,
        storage: &mut SessionStorage,
    ) {
        let mut viewer = DockTabViewer {
            db_tree,
            db,
            results_table,
            chat,
            state,
            preview,
            storage,
        };

        DockArea::new(&mut self.state)
            .style(Style::from_egui(ui.style().as_ref()))
            .show_inside(ui, &mut viewer);
    }

    fn default_state() -> DockState<DockTab> {
        let mut state = DockState::new(vec![DockTab::SqlWorkspace]);
        let surface = state.main_surface_mut();
        let [sql_node, _database_node] =
            surface.split_left(NodeIndex::root(), 0.78, vec![DockTab::DatabaseStructure]);
        surface.split_right(sql_node, 0.70, vec![DockTab::Chat]);
        state
    }
}

struct DockTabViewer<'a> {
    db_tree: &'a mut DbTree,
    db: &'a mut DbManager,
    results_table: &'a mut ResultsTable,
    chat: &'a mut ChatPanel,
    state: &'a mut PersistedState,
    preview: &'a mut PreviewManager,
    storage: &'a mut SessionStorage,
}

impl DockTabViewer<'_> {
    fn show_database_structure(&mut self, ui: &mut Ui) {
        self.db_tree.ui(ui, self.db, self.results_table);
    }

    fn show_sql_workspace(&mut self, ui: &mut Ui) {
        self.db.ensure_storage();
        let mut sql_ctx = SqlCtx {
            state: self.state.clone(),
            db: DbManager {
                active_db_config_id: self.db.active_db_config_id.clone(),
                pools: self.db.pools.clone(),
                storage: None,
                ..Default::default()
            },
        };

        if self.db.storage.is_some() {
            sql_ctx.db.ensure_storage();
        }

        self.results_table.ui(ui, Some(&mut sql_ctx));
        self.db.pools = sql_ctx.db.pools;
    }

    fn show_chat(&mut self, ui: &mut Ui) {
        let settings = self.state.settings.clone();
        let mut chat_ctx = ChatCtx {
            state: self.state,
            preview: self.preview,
            send_shortcut: settings.send_shortcut,
            openai_api_key: settings.openai_api_key.clone(),
            openai_model: settings.openai_model.clone(),
            storage: self.storage,
            should_scroll_to_bottom: false,
            active_db_config_id: self.db.active_db_config_id.clone(),
        };
        self.chat.ui(&mut chat_ctx, ui);
    }
}

impl TabViewer for DockTabViewer<'_> {
    type Tab = DockTab;

    fn title(&mut self, tab: &mut Self::Tab) -> WidgetText {
        tab.title().into()
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        match tab {
            DockTab::DatabaseStructure => self.show_database_structure(ui),
            DockTab::SqlWorkspace => self.show_sql_workspace(ui),
            DockTab::Chat => self.show_chat(ui),
        }
    }

    fn is_closeable(&self, _tab: &Self::Tab) -> bool {
        false
    }

    fn allowed_in_windows(&self, _tab: &mut Self::Tab) -> bool {
        true
    }

    fn scroll_bars(&self, tab: &Self::Tab) -> [bool; 2] {
        match tab {
            DockTab::SqlWorkspace => [false, false],
            DockTab::DatabaseStructure | DockTab::Chat => [true, true],
        }
    }

    fn clear_background(&self, _tab: &Self::Tab) -> bool {
        true
    }
}
