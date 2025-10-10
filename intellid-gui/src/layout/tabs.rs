use egui::WidgetText;
use egui_dock::TabViewer;

use crate::{
    components::{ChatCtx, ChatPanel, PreviewManager, SessionsPanel}, models::PersistedState, AppFrame
};

#[derive(Debug, Clone)]
pub enum CenterTopTab {
    SqlEditor,
}

#[derive(Debug, Clone)]
pub enum CenterBottomTab {
    Results,
}

pub struct CenterTopViewer<'a> {
    pub app: &'a mut AppFrame,
}
impl<'a> TabViewer for CenterTopViewer<'a> {
    type Tab = CenterTopTab;
    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            CenterTopTab::SqlEditor => self.app.ui_sql_editor(ui),
        }
    }
    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match tab {
            CenterTopTab::SqlEditor => "SQL".into(),
        }
    }
}

pub struct CenterBottomViewer<'a> {
    pub app: &'a mut AppFrame,
}
impl<'a> TabViewer for CenterBottomViewer<'a> {
    type Tab = CenterBottomTab;
    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            CenterBottomTab::Results => self.app.ui_results(ui),
        }
    }
    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match tab {
            CenterBottomTab::Results => "Results".into(),
        }
    }
}
