use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use once_cell::sync::Lazy;
use std::sync::Mutex;

/// 全局的 CommonMarkCache 实例，用于在多次渲染之间复用缓存
static MARKDOWN_CACHE: Lazy<Mutex<CommonMarkCache>> =
    Lazy::new(|| Mutex::new(CommonMarkCache::default()));

/// 渲染 markdown 文本到 egui UI
pub fn render_markdown(ui: &mut egui::Ui, text: &str) {
    let mut cache = MARKDOWN_CACHE.lock().unwrap();
    CommonMarkViewer::new().show(ui, &mut cache, text);
}
