use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use once_cell::sync::Lazy;
use std::sync::Mutex;

/// Global CommonMarkCache instance, reused across multiple renders
static MARKDOWN_CACHE: Lazy<Mutex<CommonMarkCache>> =
    Lazy::new(|| Mutex::new(CommonMarkCache::default()));

/// Render markdown text to egui UI
pub fn render_markdown(ui: &mut egui::Ui, text: &str) {
    render_markdown_with_id(ui, text, egui::Id::new("markdown"));
}

/// Render markdown text with a stable parent id for widgets created by the renderer.
pub fn render_markdown_with_id(ui: &mut egui::Ui, text: &str, id: egui::Id) {
    let mut cache = MARKDOWN_CACHE.lock().unwrap();
    ui.push_id(id, |ui| {
        CommonMarkViewer::new().show(ui, &mut cache, text);
    });
}
