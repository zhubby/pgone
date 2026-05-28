pub mod json;
pub mod md;
pub mod toml;
pub mod yaml;

use eframe::egui::Context;

/// Get screen center position
pub(crate) fn screen_center(ctx: &Context) -> eframe::egui::Pos2 {
    ctx.content_rect().center()
}
