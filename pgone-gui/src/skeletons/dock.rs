use crate::components::{
    ChatCtx, ChatPanel, DbManager, DbTree, PreviewManager, ResultsTable, SqlCtx,
};
use crate::models::{ChatSession, PersistedState};
use crate::storage::SessionStorage;
use eframe::egui::{Rect, Ui, WidgetText};
use egui_dock::{DockArea, DockState, Node, NodeIndex, NodePath, Split, Style, TabViewer};
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
    SqlDraft {
        id: u64,
        title: String,
    },
    #[serde(skip)]
    GraphViewer {
        id: u64,
        title: String,
    },
    #[serde(skip)]
    SqlResult {
        id: u64,
        title: String,
    },
    Agent {
        session_id: String,
    },
    Chat,
}

#[derive(Clone, Copy)]
pub enum DockPanel {
    Structure,
    Agent,
    Sql,
    Results,
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
            Self::SqlDraft { title, .. } => {
                format!("{} {}", egui_phosphor::regular::NOTE_PENCIL, title)
            }
            Self::GraphViewer { title, .. } => {
                format!("{} {}", egui_phosphor::regular::GRAPH, title)
            }
            Self::SqlResult { title, .. } => {
                format!("{} {}", egui_phosphor::regular::TABLE, title)
            }
            Self::Agent { session_id } => {
                format!("{} Agent {}", egui_phosphor::regular::SPARKLE, session_id)
            }
            Self::Chat => format!("{} Agent", egui_phosphor::regular::SPARKLE),
        }
    }

    fn is_temporary_viewer(&self) -> bool {
        matches!(
            self,
            Self::JsonViewer { .. }
                | Self::DdlViewer { .. }
                | Self::SqlDraft { .. }
                | Self::GraphViewer { .. }
                | Self::SqlResult { .. }
        )
    }
}

pub struct DockLayout {
    state: DockState<DockTab>,
    agent_explicitly_hidden: bool,
}

impl Default for DockLayout {
    fn default() -> Self {
        Self {
            state: Self::default_state(),
            agent_explicitly_hidden: false,
        }
    }
}

impl DockLayout {
    pub fn from_state(state: DockState<DockTab>) -> Option<Self> {
        Self::has_required_tabs(&state).then_some(Self {
            state,
            agent_explicitly_hidden: false,
        })
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
        self.agent_explicitly_hidden = false;
    }

    pub fn is_panel_visible(&self, panel: DockPanel) -> bool {
        match panel {
            DockPanel::Structure => self.state.find_tab(&DockTab::DatabaseStructure).is_some(),
            DockPanel::Sql => self.state.find_tab(&DockTab::SqlEditor).is_some(),
            DockPanel::Results => self.state.find_tab(&DockTab::Results).is_some(),
            DockPanel::Agent => self.has_agent_tab(),
        }
    }

    pub fn toggle_panel(&mut self, panel: DockPanel, state: &mut PersistedState) {
        if self.is_panel_visible(panel) {
            self.hide_panel(panel);
        } else {
            self.show_panel(panel, state);
        }
    }

    fn hide_panel(&mut self, panel: DockPanel) {
        if matches!(panel, DockPanel::Agent) {
            self.agent_explicitly_hidden = true;
        }

        self.state.retain_tabs(|tab| match panel {
            DockPanel::Structure => !matches!(tab, DockTab::DatabaseStructure),
            DockPanel::Sql => !matches!(
                tab,
                DockTab::SqlEditor | DockTab::DdlViewer { .. } | DockTab::SqlDraft { .. }
            ),
            DockPanel::Results => !matches!(
                tab,
                DockTab::Results | DockTab::JsonViewer { .. } | DockTab::GraphViewer { .. }
            ),
            DockPanel::Agent => !matches!(tab, DockTab::Agent { .. } | DockTab::Chat),
        });
    }

    fn show_panel(&mut self, panel: DockPanel, state: &mut PersistedState) {
        match panel {
            DockPanel::Structure => {
                self.show_static_panel(DockTab::DatabaseStructure, Split::Left, 0.78)
            }
            DockPanel::Sql => self.show_static_panel(DockTab::SqlEditor, Split::Above, 0.55),
            DockPanel::Results => self.show_static_panel(DockTab::Results, Split::Below, 0.55),
            DockPanel::Agent => {
                self.agent_explicitly_hidden = false;
                normalize_current_session_index(state);
                let session_id = current_session_id(state)
                    .unwrap_or_else(|| create_replacement_agent_session(state, None));
                self.push_agent_tab_to_sidebar(session_id);
            }
        }
    }

    fn show_static_panel(&mut self, tab: DockTab, split: Split, fraction: f32) {
        if self.state.find_tab(&tab).is_some() {
            return;
        }

        self.state.split(
            NodePath::MAIN_ROOT,
            split,
            fraction,
            Node::leaf_with(vec![tab]),
        );
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
        self.reconcile_agent_tabs(state, Some(storage));

        {
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
        }

        for session_id in chat.take_pending_agent_tab_requests() {
            self.push_or_focus_agent_tab(session_id);
        }
        for request in chat.take_pending_sql_preview_requests() {
            results_table.open_sql_draft(request.title, request.sql, request.database);
        }
        for request in chat.take_pending_sql_result_requests() {
            results_table.open_sql_result(
                request.title,
                request.sql,
                request.database,
                request.columns,
                request.rows,
                request.row_count,
                request.truncated,
                request.explain,
            );
        }

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
        for tab in results_table.take_pending_sql_draft_tabs() {
            self.push_sql_panel_tab(DockTab::SqlDraft {
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
        for tab in results_table.take_pending_sql_result_tabs() {
            self.push_results_viewer_tab(DockTab::SqlResult {
                id: tab.id,
                title: tab.title,
            });
        }

        self.retain_live_json_viewer_tabs(results_table);
        self.retain_live_ddl_viewer_tabs(results_table);
        self.retain_live_sql_draft_tabs(results_table);
        self.retain_live_graph_viewer_tabs(results_table);
        self.retain_live_sql_result_tabs(results_table);
        self.reconcile_agent_tabs(state, Some(storage));
    }

    fn reconcile_agent_tabs(
        &mut self,
        state: &mut PersistedState,
        mut storage: Option<&mut SessionStorage>,
    ) {
        normalize_current_session_index(state);
        let current_session_id = current_session_id(state);

        for (_, tab) in self.state.iter_all_tabs_mut() {
            if matches!(tab, DockTab::Chat) {
                if let Some(session_id) = &current_session_id {
                    *tab = DockTab::Agent {
                        session_id: session_id.clone(),
                    };
                }
            }
        }

        let session_ids = state
            .sessions
            .iter()
            .map(|session| session.id.as_str())
            .collect::<HashSet<_>>();
        self.state.retain_tabs(|tab| match tab {
            DockTab::Agent { session_id } => session_ids.contains(session_id.as_str()),
            DockTab::Chat => false,
            _ => true,
        });

        if self.has_agent_tab() || self.agent_explicitly_hidden {
            return;
        }

        let session_id = create_replacement_agent_session(state, storage.as_deref_mut());
        self.push_agent_tab_to_sidebar(session_id);
    }

    fn has_agent_tab(&self) -> bool {
        self.state
            .iter_all_tabs()
            .any(|(_, tab)| matches!(tab, DockTab::Agent { .. }))
    }

    fn push_or_focus_agent_tab(&mut self, session_id: String) {
        if let Some(path) = self.state.find_tab_from(|tab| {
            matches!(tab, DockTab::Agent { session_id: tab_session_id } if tab_session_id == &session_id)
        }) {
            let _ = self.state.set_active_tab(path);
            self.state.set_focused_node_and_surface(path.node_path());
            return;
        }

        let tab = DockTab::Agent { session_id };
        if let Some(agent_path) = self
            .state
            .find_tab_from(|tab| matches!(tab, DockTab::Agent { .. } | DockTab::Chat))
        {
            if let Ok(leaf) = self.state.leaf_mut(agent_path.node_path()) {
                leaf.append_tab(tab);
                let active = leaf.tabs.len().saturating_sub(1);
                let _ = leaf.set_active_tab(active);
                self.state
                    .set_focused_node_and_surface(agent_path.node_path());
                return;
            }
        }

        self.state.push_to_focused_leaf(tab);
    }

    fn push_agent_tab_to_sidebar(&mut self, session_id: String) {
        if let Some(path) = self.state.find_tab_from(|tab| {
            matches!(tab, DockTab::Agent { session_id: tab_session_id } if tab_session_id == &session_id)
        }) {
            let _ = self.state.set_active_tab(path);
            self.state.set_focused_node_and_surface(path.node_path());
            return;
        }

        let tab = DockTab::Agent { session_id };
        if let Some(agent_path) = self
            .state
            .find_tab_from(|tab| matches!(tab, DockTab::Agent { .. } | DockTab::Chat))
        {
            if let Ok(leaf) = self.state.leaf_mut(agent_path.node_path()) {
                leaf.append_tab(tab);
                let active = leaf.tabs.len().saturating_sub(1);
                let _ = leaf.set_active_tab(active);
                self.state
                    .set_focused_node_and_surface(agent_path.node_path());
                return;
            }
        }

        self.state
            .main_surface_mut()
            .split_right(NodeIndex::root(), 0.70, vec![tab]);
        self.state
            .set_focused_node_and_surface(NodePath::MAIN_ROOT.right_node());
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
                    ) || matches!(
                        (&tab, existing),
                        (
                            DockTab::SqlResult { id: new_id, .. },
                            DockTab::SqlResult {
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
        self.push_sql_panel_tab(tab);
    }

    fn push_sql_panel_tab(&mut self, tab: DockTab) {
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

    fn retain_live_sql_draft_tabs(&mut self, results_table: &mut ResultsTable) {
        self.state.retain_tabs(|tab| match tab {
            DockTab::SqlDraft { id, .. } => results_table.sql_draft_tab(*id).is_some(),
            _ => true,
        });

        let keep_ids = self
            .state
            .iter_all_tabs()
            .filter_map(|(_, tab)| match tab {
                DockTab::SqlDraft { id, .. } => Some(*id),
                _ => None,
            })
            .collect::<HashSet<_>>();
        results_table.retain_sql_draft_tabs(&keep_ids);
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

    fn retain_live_sql_result_tabs(&mut self, results_table: &mut ResultsTable) {
        self.state.retain_tabs(|tab| match tab {
            DockTab::SqlResult { id, .. } => results_table.sql_result_tab(*id).is_some(),
            _ => true,
        });

        let keep_ids = self
            .state
            .iter_all_tabs()
            .filter_map(|(_, tab)| match tab {
                DockTab::SqlResult { id, .. } => Some(*id),
                _ => None,
            })
            .collect::<HashSet<_>>();
        results_table.retain_sql_result_tabs(&keep_ids);
    }

    fn default_state() -> DockState<DockTab> {
        let mut state = DockState::new(vec![DockTab::SqlEditor]);
        let surface = state.main_surface_mut();
        surface.split_below(NodeIndex::root(), 0.45, vec![DockTab::Results]);
        let [center_node, _database_node] =
            surface.split_left(NodeIndex::root(), 0.78, vec![DockTab::DatabaseStructure]);
        surface.split_right(
            center_node,
            0.70,
            vec![DockTab::Agent {
                session_id: "1".to_owned(),
            }],
        );
        state
    }

    fn has_required_tabs(state: &DockState<DockTab>) -> bool {
        [
            DockTab::DatabaseStructure,
            DockTab::SqlEditor,
            DockTab::Results,
        ]
        .into_iter()
        .all(|required| state.iter_all_tabs().any(|(_, tab)| *tab == required))
            && state
                .iter_all_tabs()
                .any(|(_, tab)| matches!(tab, DockTab::Agent { .. } | DockTab::Chat))
    }
}

fn normalize_current_session_index(state: &mut PersistedState) {
    if state.sessions.is_empty() {
        state.current_index = 0;
    } else if state.current_index >= state.sessions.len() {
        state.current_index = state.sessions.len() - 1;
    }
}

fn current_session_id(state: &PersistedState) -> Option<String> {
    state
        .sessions
        .get(state.current_index)
        .map(|session| session.id.clone())
}

fn create_replacement_agent_session(
    state: &mut PersistedState,
    storage: Option<&mut SessionStorage>,
) -> String {
    let new_id = state.next_session_id.to_string();
    state.next_session_id = state.next_session_id.saturating_add(1);

    let new_session = ChatSession::default_with_timestamp(new_id.clone());
    state.sessions.push(new_session.clone());
    state.current_index = state.sessions.len().saturating_sub(1);

    if let Some(storage) = storage
        && let Err(error) = storage.save_session(&new_session)
    {
        tracing::error!("Failed to save replacement Agent session: {error}");
    }

    new_id
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn agent_tab_count(layout: &DockLayout, session_id: &str) -> usize {
        layout
            .state
            .iter_all_tabs()
            .filter(|(_, tab)| {
                matches!(tab, DockTab::Agent { session_id: tab_session_id } if tab_session_id == session_id)
            })
            .count()
    }

    #[test]
    fn default_layout_contains_agent_session_tab() {
        let layout = DockLayout::default();

        assert_eq!(agent_tab_count(&layout, "1"), 1);
    }

    #[test]
    fn legacy_chat_tab_reconciles_to_current_agent_tab() {
        let mut layout = DockLayout {
            state: DockState::new(vec![DockTab::Chat]),
            agent_explicitly_hidden: false,
        };
        let mut state = PersistedState::default();

        layout.reconcile_agent_tabs(&mut state, None);

        assert_eq!(agent_tab_count(&layout, "1"), 1);
        assert!(
            !layout
                .state
                .iter_all_tabs()
                .any(|(_, tab)| matches!(tab, DockTab::Chat))
        );
    }

    #[test]
    fn push_or_focus_agent_tab_does_not_duplicate_existing_session_tab() {
        let mut layout = DockLayout::default();

        layout.push_or_focus_agent_tab("1".to_owned());
        layout.push_or_focus_agent_tab("1".to_owned());

        assert_eq!(agent_tab_count(&layout, "1"), 1);
    }

    #[test]
    fn reconcile_agent_tabs_removes_orphan_session_tabs() {
        let mut layout = DockLayout::default();
        layout.push_or_focus_agent_tab("2".to_owned());
        let mut state = PersistedState {
            sessions: vec![ChatSession::default_with_timestamp("1".to_owned())],
            ..PersistedState::default()
        };

        layout.reconcile_agent_tabs(&mut state, None);

        assert_eq!(agent_tab_count(&layout, "1"), 1);
        assert_eq!(agent_tab_count(&layout, "2"), 0);
    }

    #[test]
    fn reconcile_agent_tabs_creates_new_session_when_last_agent_tab_is_closed() {
        let mut layout = DockLayout::default();
        layout
            .state
            .retain_tabs(|tab| !matches!(tab, DockTab::Agent { .. } | DockTab::Chat));
        let mut state = PersistedState::default();
        let initial_session_count = state.sessions.len();
        let next_session_id = state.next_session_id;

        layout.reconcile_agent_tabs(&mut state, None);

        let new_session_id = next_session_id.to_string();
        assert_eq!(state.sessions.len(), initial_session_count + 1);
        assert_eq!(state.current_index, state.sessions.len() - 1);
        assert_eq!(state.sessions[state.current_index].id, new_session_id);
        assert_eq!(state.next_session_id, next_session_id + 1);
        assert_eq!(agent_tab_count(&layout, &new_session_id), 1);
        assert!(DockLayout::has_required_tabs(&layout.state));
    }

    #[test]
    fn reconcile_agent_tabs_keeps_existing_agent_tab_without_creating_session() {
        let mut layout = DockLayout::default();
        let mut state = PersistedState::default();
        let initial_session_count = state.sessions.len();
        let next_session_id = state.next_session_id;

        layout.reconcile_agent_tabs(&mut state, None);

        assert_eq!(state.sessions.len(), initial_session_count);
        assert_eq!(state.next_session_id, next_session_id);
        assert_eq!(agent_tab_count(&layout, "1"), 1);
    }

    #[test]
    fn toggle_panel_hides_and_restores_structure() {
        let mut layout = DockLayout::default();
        let mut state = PersistedState::default();

        layout.toggle_panel(DockPanel::Structure, &mut state);
        assert!(!layout.is_panel_visible(DockPanel::Structure));

        layout.toggle_panel(DockPanel::Structure, &mut state);
        assert!(layout.is_panel_visible(DockPanel::Structure));
        assert!(DockLayout::has_required_tabs(&layout.state));
    }

    #[test]
    fn toggle_panel_hides_agent_without_auto_reconcile() {
        let mut layout = DockLayout::default();
        let mut state = PersistedState::default();
        let initial_session_count = state.sessions.len();

        layout.toggle_panel(DockPanel::Agent, &mut state);
        layout.reconcile_agent_tabs(&mut state, None);

        assert!(!layout.is_panel_visible(DockPanel::Agent));
        assert_eq!(state.sessions.len(), initial_session_count);
    }

    #[test]
    fn toggle_panel_restores_agent_after_explicit_hide() {
        let mut layout = DockLayout::default();
        let mut state = PersistedState::default();

        layout.toggle_panel(DockPanel::Agent, &mut state);
        layout.toggle_panel(DockPanel::Agent, &mut state);

        assert!(layout.is_panel_visible(DockPanel::Agent));
        assert_eq!(agent_tab_count(&layout, "1"), 1);
    }

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
    fn pending_sql_draft_tab_opens_next_to_sql_editor() {
        let mut layout = DockLayout::default();
        let mut results_table = ResultsTable::new();
        let id = results_table.open_sql_draft(
            "New Table",
            "CREATE TABLE public.new_table (id SERIAL PRIMARY KEY);",
            "postgres",
        );

        for tab in results_table.take_pending_sql_draft_tabs() {
            layout.push_sql_panel_tab(DockTab::SqlDraft {
                id: tab.id,
                title: tab.title,
            });
        }

        let sql_path = layout.state.find_tab(&DockTab::SqlEditor).unwrap();
        let leaf = layout.state.leaf(sql_path.node_path()).unwrap();

        assert!(
            leaf.tabs
                .iter()
                .any(|tab| matches!(tab, DockTab::SqlDraft { id: tab_id, .. } if *tab_id == id))
        );
        assert!(
            matches!(&leaf[leaf.active], DockTab::SqlDraft { id: tab_id, .. } if *tab_id == id)
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
    fn pending_sql_result_tab_opens_in_results_leaf() {
        let mut layout = DockLayout::default();
        let mut results_table = ResultsTable::new();
        let id = results_table.open_sql_result(
            "Query Result",
            "SELECT 1",
            "app",
            vec!["?column?".to_owned()],
            vec![vec!["1".to_owned()]],
            1,
            false,
            None,
        );

        for tab in results_table.take_pending_sql_result_tabs() {
            layout.push_results_viewer_tab(DockTab::SqlResult {
                id: tab.id,
                title: tab.title,
            });
        }

        let results_path = layout.state.find_tab(&DockTab::Results).unwrap();
        let leaf = layout.state.leaf(results_path.node_path()).unwrap();

        assert!(
            leaf.tabs
                .iter()
                .any(|tab| matches!(tab, DockTab::SqlResult { id: tab_id, .. } if *tab_id == id))
        );
        assert!(
            matches!(&leaf[leaf.active], DockTab::SqlResult { id: tab_id, .. } if *tab_id == id)
        );
    }

    #[test]
    fn sql_result_tab_survives_live_tab_retention() {
        let mut layout = DockLayout::default();
        let mut results_table = ResultsTable::new();
        let id = results_table.open_sql_result(
            "Query Result",
            "SELECT 1",
            "app",
            vec!["?column?".to_owned()],
            vec![vec!["1".to_owned()]],
            1,
            false,
            None,
        );

        for tab in results_table.take_pending_sql_result_tabs() {
            layout.push_results_viewer_tab(DockTab::SqlResult {
                id: tab.id,
                title: tab.title,
            });
        }
        layout.retain_live_sql_result_tabs(&mut results_table);

        assert!(results_table.sql_result_tab(id).is_some());
        assert!(
            layout.state.iter_all_tabs().any(
                |(_, tab)| matches!(tab, DockTab::SqlResult { id: tab_id, .. } if *tab_id == id)
            )
        );
    }

    #[test]
    fn sanitized_state_removes_sql_result_tabs() {
        let mut layout = DockLayout::default();
        let mut results_table = ResultsTable::new();
        let id = results_table.open_sql_result(
            "Query Result",
            "SELECT 1",
            "app",
            vec!["?column?".to_owned()],
            vec![vec!["1".to_owned()]],
            1,
            false,
            None,
        );

        for tab in results_table.take_pending_sql_result_tabs() {
            layout.push_results_viewer_tab(DockTab::SqlResult {
                id: tab.id,
                title: tab.title,
            });
        }

        let sanitized = layout.sanitized_state();

        assert!(
            !sanitized.iter_all_tabs().any(
                |(_, tab)| matches!(tab, DockTab::SqlResult { id: tab_id, .. } if *tab_id == id)
            )
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

    fn show_sql_draft(&mut self, ui: &mut Ui, id: u64) {
        let mut sql_ctx = self.make_sql_ctx();
        self.results_table.ui_sql_draft(ui, id, &mut sql_ctx);
    }

    fn show_graph_viewer(&mut self, ui: &mut Ui, id: u64) {
        let pools = self.db.pools.clone();
        self.results_table.ui_graph_viewer(ui, id, pools);
    }

    fn show_sql_result(&mut self, ui: &mut Ui, id: u64) {
        self.results_table.ui_sql_result(ui, id);
    }

    fn show_chat(&mut self, ui: &mut Ui, session_id: Option<String>) {
        if let Some(session_id) = session_id {
            if let Some(index) = self
                .state
                .sessions
                .iter()
                .position(|session| session.id == session_id)
            {
                self.state.current_index = index;
            } else {
                ui.label(format!(
                    "{} Agent session no longer exists",
                    egui_phosphor::regular::WARNING
                ));
                return;
            }
        }

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
        match tab {
            DockTab::Agent { session_id } => {
                let title = self
                    .state
                    .sessions
                    .iter()
                    .find(|session| session.id == *session_id)
                    .map(|session| session.title.as_str())
                    .unwrap_or(session_id.as_str());
                format!("{} {}", egui_phosphor::regular::SPARKLE, title).into()
            }
            _ => tab.title().into(),
        }
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        match tab {
            DockTab::DatabaseStructure => self.show_database_structure(ui),
            DockTab::SqlEditor => self.show_sql_editor(ui),
            DockTab::Results => self.show_results(ui),
            DockTab::JsonViewer { id, .. } => self.show_json_viewer(ui, *id),
            DockTab::DdlViewer { id, .. } => self.show_ddl_viewer(ui, *id),
            DockTab::SqlDraft { id, .. } => self.show_sql_draft(ui, *id),
            DockTab::GraphViewer { id, .. } => self.show_graph_viewer(ui, *id),
            DockTab::SqlResult { id, .. } => self.show_sql_result(ui, *id),
            DockTab::Agent { session_id } => self.show_chat(ui, Some(session_id.clone())),
            DockTab::Chat => self.show_chat(ui, None),
        }
    }

    fn is_closeable(&self, tab: &Self::Tab) -> bool {
        tab.is_temporary_viewer() || matches!(tab, DockTab::Agent { .. })
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
            | DockTab::SqlDraft { .. }
            | DockTab::GraphViewer { .. }
            | DockTab::SqlResult { .. } => [false, false],
            DockTab::DatabaseStructure => [true, true],
            DockTab::Agent { .. } | DockTab::Chat => [false, false],
        }
    }

    fn clear_background(&self, _tab: &Self::Tab) -> bool {
        true
    }
}
