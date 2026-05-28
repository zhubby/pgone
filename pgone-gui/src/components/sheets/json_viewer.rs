use super::JsonViewerTab;
use egui_json_tree::{DefaultExpand, JsonTree};

pub fn ui(ui: &mut egui::Ui, tab: &JsonViewerTab) {
    ui.horizontal(|ui| {
        ui.strong(&tab.source_column);
        ui.label(format!("row {}", tab.source_row + 1));
    });
    ui.separator();

    egui::ScrollArea::both()
        .id_salt(("json_viewer_scroll", tab.id))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            JsonTree::new(format!("json-viewer-{}", tab.id), &tab.value)
                .default_expand(DefaultExpand::ToLevel(2))
                .show(ui);
        });
}
