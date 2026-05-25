pub mod json;
pub mod md;
pub mod toml;
pub mod yaml;

use eframe::egui::Context;

/// 获取屏幕中心位置
pub(crate) fn screen_center(ctx: &Context) -> eframe::egui::Pos2 {
    ctx.content_rect().center()
}
