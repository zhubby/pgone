use egui_dock::TabViewer;

use crate::IntelliGuiApp;

#[derive(Debug, Clone)]
pub enum LeftTab { Sessions, DbConfig }

#[derive(Debug, Clone)]
pub enum RightTab { Chat }

#[derive(Debug, Clone)]
pub enum CenterTopTab { SqlEditor }

#[derive(Debug, Clone)]
pub enum CenterBottomTab { Results }

pub struct LeftViewer<'a> { pub app: &'a mut IntelliGuiApp }
impl<'a> TabViewer for LeftViewer<'a> {
    type Tab = LeftTab;
    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            LeftTab::Sessions => self.app.ui_sessions(ui),
            LeftTab::DbConfig => self.app.ui_db_config(ui),
        }
    }
    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match tab { LeftTab::Sessions => "Sessions".into(), LeftTab::DbConfig => "DB Config".into() }
    }
}

pub struct RightViewer<'a> { pub app: &'a mut IntelliGuiApp }
impl<'a> TabViewer for RightViewer<'a> {
    type Tab = RightTab;
    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab { RightTab::Chat => self.app.ui_chat(ui) }
    }
    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match tab { RightTab::Chat => "Chat".into() }
    }
}

pub struct CenterTopViewer<'a> { pub app: &'a mut IntelliGuiApp }
impl<'a> TabViewer for CenterTopViewer<'a> {
    type Tab = CenterTopTab;
    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab { CenterTopTab::SqlEditor => self.app.ui_sql_editor(ui) }
    }
    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match tab { CenterTopTab::SqlEditor => "SQL".into() }
    }
}

pub struct CenterBottomViewer<'a> { pub app: &'a mut IntelliGuiApp }
impl<'a> TabViewer for CenterBottomViewer<'a> {
    type Tab = CenterBottomTab;
    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab { CenterBottomTab::Results => self.app.ui_results(ui) }
    }
    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match tab { CenterBottomTab::Results => "Results".into() }
    }
}


