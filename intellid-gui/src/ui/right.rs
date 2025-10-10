use egui::WidgetText;
use egui_dock::TabViewer;

use crate::{
    components::{ChatCtx, ChatPanel, PreviewManager}, models::PersistedState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightTab {
    Chat,
}

pub struct RightViewer {
    pub preview: PreviewManager,
    pub chat: ChatPanel,
    pub state: PersistedState,
}

impl Default for RightViewer {
    fn default() -> Self {
        Self { preview: PreviewManager::default(), chat: ChatPanel::default(), state: PersistedState::default() }
    }
}


impl<'a> TabViewer for RightViewer {
    type Tab = RightTab;
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
            RightTab::Chat => self.chat.ui(&mut ctxs, ui),
        }
    }
    fn title(&mut self, tab: &mut Self::Tab) -> WidgetText {
        match tab {
            RightTab::Chat => format!("{} Chat", egui_phosphor::regular::CHAT_TEXT).into(),
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
    
    fn on_close(&mut self, _tab: &mut Self::Tab) -> egui_dock::tab_viewer::OnCloseResponse {
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
    
    fn add_popup(&mut self, _ui: &mut egui::Ui, _surface: egui_dock::SurfaceIndex, _node: egui_dock::NodeIndex) {}
    
    fn tab_style_override(&self, _tab: &Self::Tab, _global_style: &egui_dock::TabStyle) -> Option<egui_dock::TabStyle> {
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