use egui::WidgetText;
use egui_dock::TabViewer;

use crate::{
    components::{ChatCtx, ChatPanel, PreviewManager, SessionsPanel}, models::PersistedState, AppFrame
};



#[derive(Debug, Clone, PartialEq, Eq, Hash, strum::Display, strum::EnumString)]
pub enum Tab {
    Chat,
    SqlEditor,
    Results,
    Sessions,
    DbConfig,
}