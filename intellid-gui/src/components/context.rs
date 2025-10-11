use crate::components::{DbManager, PreviewManager};
use crate::models::{PersistedState, SendShortcut};

#[derive(Default)]
pub struct SessionsCtx {
    pub state: PersistedState,
    pub db: DbManager,
}

pub struct ChatCtx<'a> {
    pub state: &'a mut PersistedState,
    pub preview: &'a mut PreviewManager,
    pub send_shortcut: SendShortcut,
    pub openai_api_key: Option<String>,
    pub openai_model: String,
}

#[derive(Default)]
pub struct SqlCtx {
    pub state: PersistedState,
    pub db: crate::components::DbManager,
}
