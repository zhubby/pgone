use crate::components::{DbManager, PreviewManager};
use crate::models::{PersistedState, SendShortcut};

pub struct SessionsCtx {
    pub state: PersistedState,
    pub db: DbManager,
}

impl Default for SessionsCtx {
    fn default() -> Self {
        Self { state: PersistedState::default(), db: DbManager::default() }
    }
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
