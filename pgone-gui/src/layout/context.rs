use egui::{WidgetText};
use egui_dock::{Style, TabViewer};
use std::collections::HashSet;

use crate::{
    components::{
        ChatCtx, ChatPanel, PreviewManager, ResultsTable, SessionsCtx, SessionsPanel, SqlCtx,
    },
    layout::Tab,
    models::{DbConfig, PersistedState},
};

#[derive(Default)]
pub struct Context {
    pub preview: PreviewManager,
    pub chat: ChatPanel,
    pub results_table: ResultsTable,
    pub sessions: SessionsPanel,
    pub db_config: DbConfig,
    pub state: PersistedState,
    pub style: Option<Style>,
    pub open_tabs: HashSet<Tab>,
}

// Default is derived

impl TabViewer for Context {
    type Tab = Tab;
    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        let settings = self.state.settings.clone();
        let mut ctxs = ChatCtx {
            state: &mut self.state,
            preview: &mut self.preview,
            send_shortcut: settings.send_shortcut,
            openai_api_key: settings.openai_api_key.clone(),
            openai_model: settings.openai_model.clone(),
        };
        match tab {
            Tab::Chat => self.chat.ui(&mut ctxs, ui),
            Tab::SqlEditor | Tab::Results => {
                let mut sql_ctx = SqlCtx {
                    state: self.state.clone(),
                    db: crate::components::DbManager::default(),
                };
                self.results_table.ui(ui, Some(&mut sql_ctx));
            }
            Tab::Sessions => self.sessions.ui(&mut SessionsCtx::default(), ui),
            Tab::DbConfig => self.db_config.ui(ui),
        }
        // ui.heading("My egui Application");
    }
    fn title(&mut self, tab: &mut Self::Tab) -> WidgetText {
        match tab {
            Tab::Chat => format!("{} Chat", egui_phosphor::regular::CHAT_TEXT).into(),
            Tab::SqlEditor => format!("{} SQL Editor", egui_phosphor::regular::CODE).into(),
            Tab::Results => format!("{} Results", egui_phosphor::regular::TABLE).into(),
            Tab::Sessions => format!("{} Sessions", egui_phosphor::regular::CHAT_TEXT).into(),
            Tab::DbConfig => format!("{} DB Config", egui_phosphor::regular::DATABASE).into(),
        }
    }

    fn context_menu(
        &mut self,
        _ui: &mut egui::Ui,
        _tab: &mut Self::Tab,
        _surface: egui_dock::SurfaceIndex,
        _node: egui_dock::NodeIndex,
    ) {
    }

    fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
        egui::Id::new(self.title(tab).text())
    }

    fn on_tab_button(&mut self, _tab: &mut Self::Tab, _response: &egui::Response) {}

    fn on_close(&mut self, tab: &mut Self::Tab) -> egui_dock::tab_viewer::OnCloseResponse {
        self.open_tabs.remove(tab);
        egui_dock::tab_viewer::OnCloseResponse::Close
    }

    fn is_closeable(&self, _tab: &Self::Tab) -> bool {
        true
    }

    fn closeable(&mut self, _tab: &mut Self::Tab) -> bool {
        true
    }

    fn force_close(&mut self, _tab: &mut Self::Tab) -> bool {
        false
    }

    fn on_add(&mut self, _surface: egui_dock::SurfaceIndex, _node: egui_dock::NodeIndex) {}

    fn on_rect_changed(&mut self, _tab: &mut Self::Tab) {}

    fn add_popup(
        &mut self,
        _ui: &mut egui::Ui,
        _surface: egui_dock::SurfaceIndex,
        _node: egui_dock::NodeIndex,
    ) {
    }

    fn tab_style_override(
        &self,
        _tab: &Self::Tab,
        _global_style: &egui_dock::TabStyle,
    ) -> Option<egui_dock::TabStyle> {
        None
    }

    fn allowed_in_windows(&self, _tab: &mut Self::Tab) -> bool {
        true
    }

    fn clear_background(&self, _tab: &Self::Tab) -> bool {
        true
    }

    fn scroll_bars(&self, _tab: &Self::Tab) -> [bool; 2] {
        [true, true]
    }
}
