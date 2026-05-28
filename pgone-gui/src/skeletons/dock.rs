use crate::components::{
    ChatCtx, ChatPanel, DbManager, DbTree, PreviewManager, ResultsTable, SqlCtx,
};
use crate::models::PersistedState;
use crate::storage::SessionStorage;
use eframe::egui::{Rect, Ui, WidgetText};
use egui_dock::{DockArea, DockState, Node, NodeIndex, Style, TabViewer};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DockTab {
    DatabaseStructure,
    SqlEditor,
    Results,
    #[serde(skip)]
    JsonViewer {
        id: u64,
        title: String,
    },
    #[serde(skip)]
    DdlViewer {
        id: u64,
        title: String,
    },
    #[serde(skip)]
    GraphViewer {
        id: u64,
        title: String,
    },
    Chat,
}

impl DockTab {
    fn title(&self) -> String {
        match self {
            Self::DatabaseStructure => {
                format!("{} Structure", egui_phosphor::regular::TREE_STRUCTURE)
            }
            Self::SqlEditor => format!("{} SQL", egui_phosphor::regular::CODE),
            Self::Results => format!("{} Results", egui_phosphor::regular::TABLE),
            Self::JsonViewer { title, .. } => {
                format!("{} {}", egui_phosphor::regular::BRACKETS_CURLY, title)
            }
            Self::DdlViewer { title, .. } => {
                format!("{} {}", egui_phosphor::regular::CODE, title)
            }
            Self::GraphViewer { title, .. } => {
                format!("{} {}", egui_phosphor::regular::GRAPH, title)
            }
            Self::Chat => format!("{} Agent", egui_phosphor::regular::SPARKLE),
        }
    }

    fn is_temporary_viewer(&self) -> bool {
        matches!(
            self,
            Self::JsonViewer { .. } | Self::DdlViewer { .. } | Self::GraphViewer { .. }
        )
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
    pub fn from_state(state: DockState<DockTab>) -> Option<Self> {
        Self::has_required_tabs(&state).then_some(Self { state })
    }

    pub fn state(&self) -> &DockState<DockTab> {
        &self.state
    }

    pub fn sanitized_state(&self) -> DockState<DockTab> {
        let mut state = self.state.filter_tabs(|tab| !tab.is_temporary_viewer());
        for surface in state.iter_surfaces_mut() {
            let Some(tree) = surface.node_tree_mut() else {
                continue;
            };

            for node in tree.iter_mut() {
                match node {
                    Node::Leaf(leaf) => {
                        if !leaf.rect.is_finite() {
                            leaf.rect = Rect::ZERO;
                        }
                        if !leaf.viewport.is_finite() {
                            leaf.viewport = Rect::ZERO;
                        }
                    }
                    Node::Horizontal(split) | Node::Vertical(split) => {
                        if !split.rect.is_finite() {
                            split.rect = Rect::ZERO;
                        }
                    }
                    Node::Empty => {}
                }
            }
        }
        state
    }

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
            .show_leaf_collapse_buttons(false)
            .show_inside(ui, &mut viewer);

        for tab in results_table.take_pending_json_viewer_tabs() {
            self.push_json_viewer_tab(DockTab::JsonViewer {
                id: tab.id,
                title: tab.title,
            });
        }
        for tab in results_table.take_pending_ddl_viewer_tabs() {
            self.push_ddl_viewer_tab(DockTab::DdlViewer {
                id: tab.id,
                title: tab.title,
            });
        }
        for tab in results_table.take_pending_graph_viewer_tabs() {
            self.push_results_viewer_tab(DockTab::GraphViewer {
                id: tab.id,
                title: tab.title,
            });
        }

        self.retain_live_json_viewer_tabs(results_table);
        self.retain_live_ddl_viewer_tabs(results_table);
        self.retain_live_graph_viewer_tabs(results_table);
    }

    fn retain_live_json_viewer_tabs(&mut self, results_table: &mut ResultsTable) {
        self.state.retain_tabs(|tab| match tab {
            DockTab::JsonViewer { id, .. } => results_table.json_viewer_tab(*id).is_some(),
            _ => true,
        });

        let keep_ids = self
            .state
            .iter_all_tabs()
            .filter_map(|(_, tab)| match tab {
                DockTab::JsonViewer { id, .. } => Some(*id),
                _ => None,
            })
            .collect::<HashSet<_>>();
        results_table.retain_json_viewer_tabs(&keep_ids);
    }

    fn push_json_viewer_tab(&mut self, tab: DockTab) {
        self.push_results_viewer_tab(tab);
    }

    fn push_results_viewer_tab(&mut self, tab: DockTab) {
        if let Some(results_path) = self.state.find_tab(&DockTab::Results) {
            if let Ok(leaf) = self.state.leaf_mut(results_path.node_path()) {
                let active = if let Some(index) = leaf.tabs.iter().position(|existing| {
                    matches!(
                        (&tab, existing),
                        (
                            DockTab::JsonViewer { id: new_id, .. },
                            DockTab::JsonViewer {
                                id: existing_id,
                                ..
                            }
                        ) if new_id == existing_id
                    ) || matches!(
                        (&tab, existing),
                        (
                            DockTab::GraphViewer { id: new_id, .. },
                            DockTab::GraphViewer {
                                id: existing_id,
                                ..
                            }
                        ) if new_id == existing_id
                    )
                }) {
                    index
                } else {
                    leaf.append_tab(tab);
                    leaf.tabs.len().saturating_sub(1)
                };
                let _ = leaf.set_active_tab(active);
                self.state
                    .set_focused_node_and_surface(results_path.node_path());
                return;
            }
        }

        self.state.push_to_focused_leaf(tab);
    }

    fn retain_live_ddl_viewer_tabs(&mut self, results_table: &mut ResultsTable) {
        self.state.retain_tabs(|tab| match tab {
            DockTab::DdlViewer { id, .. } => results_table.ddl_viewer_tab(*id).is_some(),
            _ => true,
        });

        let keep_ids = self
            .state
            .iter_all_tabs()
            .filter_map(|(_, tab)| match tab {
                DockTab::DdlViewer { id, .. } => Some(*id),
                _ => None,
            })
            .collect::<HashSet<_>>();
        results_table.retain_ddl_viewer_tabs(&keep_ids);
    }

    fn push_ddl_viewer_tab(&mut self, tab: DockTab) {
        if let Some(sql_path) = self.state.find_tab(&DockTab::SqlEditor) {
            if let Ok(leaf) = self.state.leaf_mut(sql_path.node_path()) {
                leaf.append_tab(tab);
                let active = leaf.tabs.len().saturating_sub(1);
                let _ = leaf.set_active_tab(active);
                self.state
                    .set_focused_node_and_surface(sql_path.node_path());
                return;
            }
        }

        self.state.push_to_focused_leaf(tab);
    }

    fn retain_live_graph_viewer_tabs(&mut self, results_table: &mut ResultsTable) {
        self.state.retain_tabs(|tab| match tab {
            DockTab::GraphViewer { id, .. } => results_table.graph_viewer_tab(*id).is_some(),
            _ => true,
        });

        let keep_ids = self
            .state
            .iter_all_tabs()
            .filter_map(|(_, tab)| match tab {
                DockTab::GraphViewer { id, .. } => Some(*id),
                _ => None,
            })
            .collect::<HashSet<_>>();
        results_table.retain_graph_viewer_tabs(&keep_ids);
    }

    fn default_state() -> DockState<DockTab> {
        let mut state = DockState::new(vec![DockTab::SqlEditor]);
        let surface = state.main_surface_mut();
        surface.split_below(NodeIndex::root(), 0.45, vec![DockTab::Results]);
        let [center_node, _database_node] =
            surface.split_left(NodeIndex::root(), 0.78, vec![DockTab::DatabaseStructure]);
        surface.split_right(center_node, 0.70, vec![DockTab::Chat]);
        state
    }

    fn has_required_tabs(state: &DockState<DockTab>) -> bool {
        [
            DockTab::DatabaseStructure,
            DockTab::SqlEditor,
            DockTab::Results,
            DockTab::Chat,
        ]
        .into_iter()
        .all(|required| state.iter_all_tabs().any(|(_, tab)| *tab == required))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn pending_json_viewer_tab_survives_live_tab_retention() {
        let mut layout = DockLayout::default();
        let mut results_table = ResultsTable::new();
        let id = results_table.open_json_viewer(0, "payload", json!({ "ok": true }));

        for tab in results_table.take_pending_json_viewer_tabs() {
            layout.push_json_viewer_tab(DockTab::JsonViewer {
                id: tab.id,
                title: tab.title,
            });
        }
        layout.retain_live_json_viewer_tabs(&mut results_table);

        assert!(results_table.json_viewer_tab(id).is_some());
        assert!(layout.state.iter_all_tabs().any(
            |(_, tab)| matches!(tab, DockTab::JsonViewer { id: tab_id, .. } if *tab_id == id)
        ));
    }

    #[test]
    fn pending_ddl_viewer_tab_opens_next_to_sql_editor() {
        let mut layout = DockLayout::default();
        let mut results_table = ResultsTable::new();
        let id = results_table.open_ddl_viewer("DDL public.users", "CREATE TABLE public.users ();");

        for tab in results_table.take_pending_ddl_viewer_tabs() {
            layout.push_ddl_viewer_tab(DockTab::DdlViewer {
                id: tab.id,
                title: tab.title,
            });
        }

        let sql_path = layout.state.find_tab(&DockTab::SqlEditor).unwrap();
        let leaf = layout.state.leaf(sql_path.node_path()).unwrap();

        assert!(
            leaf.tabs
                .iter()
                .any(|tab| matches!(tab, DockTab::DdlViewer { id: tab_id, .. } if *tab_id == id))
        );
        assert!(
            matches!(&leaf[leaf.active], DockTab::DdlViewer { id: tab_id, .. } if *tab_id == id)
        );
    }

    #[test]
    fn sanitized_state_removes_ddl_viewer_tabs() {
        let mut layout = DockLayout::default();
        let mut results_table = ResultsTable::new();
        let id = results_table.open_ddl_viewer("DDL public.users", "CREATE TABLE public.users ();");
        for tab in results_table.take_pending_ddl_viewer_tabs() {
            layout.push_ddl_viewer_tab(DockTab::DdlViewer {
                id: tab.id,
                title: tab.title,
            });
        }

        let sanitized = layout.sanitized_state();

        assert!(
            !sanitized.iter_all_tabs().any(
                |(_, tab)| matches!(tab, DockTab::DdlViewer { id: tab_id, .. } if *tab_id == id)
            )
        );
    }

    #[test]
    fn ddl_viewer_tab_survives_live_tab_retention() {
        let mut layout = DockLayout::default();
        let mut results_table = ResultsTable::new();
        let id = results_table.open_ddl_viewer("DDL public.users", "CREATE TABLE public.users ();");

        for tab in results_table.take_pending_ddl_viewer_tabs() {
            layout.push_ddl_viewer_tab(DockTab::DdlViewer {
                id: tab.id,
                title: tab.title,
            });
        }
        layout.retain_live_ddl_viewer_tabs(&mut results_table);

        assert!(results_table.ddl_viewer_tab(id).is_some());
        assert!(
            layout.state.iter_all_tabs().any(
                |(_, tab)| matches!(tab, DockTab::DdlViewer { id: tab_id, .. } if *tab_id == id)
            )
        );
    }

    #[test]
    fn pending_graph_viewer_tab_opens_in_results_leaf() {
        let mut layout = DockLayout::default();
        let mut results_table = ResultsTable::new();
        let id = results_table.open_graph_viewer(
            "analytics",
            "public",
            "postgresql://localhost/analytics",
        );

        for tab in results_table.take_pending_graph_viewer_tabs() {
            layout.push_results_viewer_tab(DockTab::GraphViewer {
                id: tab.id,
                title: tab.title,
            });
        }

        let results_path = layout.state.find_tab(&DockTab::Results).unwrap();
        let leaf = layout.state.leaf(results_path.node_path()).unwrap();

        assert!(
            leaf.tabs
                .iter()
                .any(|tab| matches!(tab, DockTab::GraphViewer { id: tab_id, .. } if *tab_id == id))
        );
        assert!(
            matches!(&leaf[leaf.active], DockTab::GraphViewer { id: tab_id, .. } if *tab_id == id)
        );
    }

    #[test]
    fn graph_viewer_tab_survives_live_tab_retention() {
        let mut layout = DockLayout::default();
        let mut results_table = ResultsTable::new();
        let id = results_table.open_graph_viewer(
            "analytics",
            "public",
            "postgresql://localhost/analytics",
        );

        for tab in results_table.take_pending_graph_viewer_tabs() {
            layout.push_results_viewer_tab(DockTab::GraphViewer {
                id: tab.id,
                title: tab.title,
            });
        }
        layout.retain_live_graph_viewer_tabs(&mut results_table);

        assert!(results_table.graph_viewer_tab(id).is_some());
        assert!(layout.state.iter_all_tabs().any(
            |(_, tab)| matches!(tab, DockTab::GraphViewer { id: tab_id, .. } if *tab_id == id)
        ));
    }

    #[test]
    fn sanitized_state_removes_graph_viewer_tabs() {
        let mut layout = DockLayout::default();
        let mut results_table = ResultsTable::new();
        let id = results_table.open_graph_viewer(
            "analytics",
            "public",
            "postgresql://localhost/analytics",
        );
        for tab in results_table.take_pending_graph_viewer_tabs() {
            layout.push_results_viewer_tab(DockTab::GraphViewer {
                id: tab.id,
                title: tab.title,
            });
        }

        let sanitized = layout.sanitized_state();

        assert!(!sanitized.iter_all_tabs().any(
            |(_, tab)| matches!(tab, DockTab::GraphViewer { id: tab_id, .. } if *tab_id == id)
        ));
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

    fn make_sql_ctx(&mut self) -> SqlCtx {
        self.db.ensure_storage();
        let mut sql_ctx = SqlCtx {
            state: self.state.clone(),
            db: self.db.sql_context_copy(),
        };

        sql_ctx
    }

    fn show_sql_editor(&mut self, ui: &mut Ui) {
        let mut sql_ctx = self.make_sql_ctx();
        self.results_table
            .sync_database_selection(Some(&mut sql_ctx));
        self.results_table.ui_sql_editor(ui, true);
    }

    fn show_results(&mut self, ui: &mut Ui) {
        let mut sql_ctx = self.make_sql_ctx();
        self.results_table
            .sync_database_selection(Some(&mut sql_ctx));
        let sql = if self.results_table.sql_input.trim().is_empty() {
            None
        } else {
            Some(self.results_table.sql_input.clone())
        };

        self.results_table
            .ui_results_table(ui, sql.as_deref(), Some(&mut sql_ctx), true);
    }

    fn show_json_viewer(&mut self, ui: &mut Ui, id: u64) {
        self.results_table.ui_json_viewer(ui, id);
    }

    fn show_ddl_viewer(&mut self, ui: &mut Ui, id: u64) {
        self.results_table.ui_ddl_viewer(ui, id);
    }

    fn show_graph_viewer(&mut self, ui: &mut Ui, id: u64) {
        let pools = self.db.pools.clone();
        self.results_table.ui_graph_viewer(ui, id, pools);
    }

    fn show_chat(&mut self, ui: &mut Ui) {
        let settings = self.state.settings.clone();
        let active_db_label = self
            .db
            .active_db_config()
            .map(|config| config.id)
            .or_else(|| self.db.active_db_config_id.clone());
        let mut chat_ctx = ChatCtx {
            state: self.state,
            preview: self.preview,
            send_shortcut: settings.send_shortcut,
            openai_api_key: settings.openai_api_key.clone(),
            openai_model: settings.openai_model.clone(),
            storage: self.storage,
            should_scroll_to_bottom: false,
            active_db_config_id: self.db.active_db_config_id.clone(),
            active_db_label,
            selected_database: self.results_table.selected_database.clone(),
            selected_schema: self.db_tree.selected_schema_name(),
            selected_table: self.db_tree.selected_table_name(),
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
            DockTab::SqlEditor => self.show_sql_editor(ui),
            DockTab::Results => self.show_results(ui),
            DockTab::JsonViewer { id, .. } => self.show_json_viewer(ui, *id),
            DockTab::DdlViewer { id, .. } => self.show_ddl_viewer(ui, *id),
            DockTab::GraphViewer { id, .. } => self.show_graph_viewer(ui, *id),
            DockTab::Chat => self.show_chat(ui),
        }
    }

    fn is_closeable(&self, tab: &Self::Tab) -> bool {
        tab.is_temporary_viewer()
    }

    fn allowed_in_windows(&self, _tab: &mut Self::Tab) -> bool {
        true
    }

    fn scroll_bars(&self, tab: &Self::Tab) -> [bool; 2] {
        match tab {
            DockTab::SqlEditor
            | DockTab::Results
            | DockTab::JsonViewer { .. }
            | DockTab::DdlViewer { .. }
            | DockTab::GraphViewer { .. } => [false, false],
            DockTab::DatabaseStructure | DockTab::Chat => [true, true],
        }
    }

    fn clear_background(&self, _tab: &Self::Tab) -> bool {
        true
    }
}
