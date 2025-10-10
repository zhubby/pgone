use crate::components::{DbManager, SessionsCtx};
use egui::WidgetText;
use egui_dock::TabViewer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftTab {
    Sessions,
    DbConfig,
}


use crate::{
    components::{ChatCtx, ChatPanel, PreviewManager, SessionsPanel}, models::PersistedState
};

pub struct LeftViewer {
    // pub ctx: &'a mut SessionsCtx,
}

impl<'a> TabViewer for LeftViewer {
    type Tab = LeftTab;
    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            LeftTab::Sessions => {
                let mut ctx = SessionsCtx::default();
                SessionsPanel::default().ui(&mut ctx, ui);
            },
            LeftTab::DbConfig =>  {
                let mut ctx = DbManager::default();
                
            },
        }
    }
    fn title(&mut self, tab: &mut Self::Tab) -> WidgetText {
        match tab {
            LeftTab::Sessions => format!("{} Sessions", egui_phosphor::regular::CHAT_TEXT).into(),
            LeftTab::DbConfig => format!("{} DB Config", egui_phosphor::regular::DATABASE).into(),
        }
    }
}
