use crate::models::{PersistedState, SendShortcut};
use crate::components::PreviewManager;

pub struct SessionsCtx<'a> {
    pub state: &'a mut PersistedState,
    pub db: &'a mut crate::components::DbManager,
}

pub struct ChatCtx<'a> {
    pub state: &'a mut PersistedState,
    pub preview: &'a mut PreviewManager,
    pub send_shortcut: SendShortcut,
    pub openai_api_key: Option<String>,
    pub openai_model: String,
}

pub struct SqlCtx<'a> {
    pub state: &'a mut PersistedState,
    pub db: &'a mut crate::components::DbManager,
}


