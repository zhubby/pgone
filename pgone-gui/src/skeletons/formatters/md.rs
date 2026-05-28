use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use once_cell::sync::Lazy;
use std::sync::Mutex;

/// Global CommonMarkCache instance, reused across multiple renders
static MARKDOWN_CACHE: Lazy<Mutex<CommonMarkCache>> =
    Lazy::new(|| Mutex::new(CommonMarkCache::default()));

/// Render markdown text to egui UI
pub fn render_markdown(ui: &mut egui::Ui, text: &str) {
    let mut cache = MARKDOWN_CACHE.lock().unwrap();
    CommonMarkViewer::new().show(ui, &mut cache, text);
}
