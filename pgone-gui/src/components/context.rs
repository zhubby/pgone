use crate::components::{DbManager, PreviewManager};
use crate::models::{PersistedState, SendShortcut};
use crate::storage::SessionStorage;

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
    pub storage: &'a mut SessionStorage,
    pub should_scroll_to_bottom: bool,
    pub active_db_config_id: Option<String>,
}

#[derive(Default)]
pub struct SqlCtx {
    pub state: PersistedState,
    pub db: DbManager,
}
