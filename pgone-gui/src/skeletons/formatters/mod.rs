pub mod json;
pub mod md;
pub mod yaml;
pub mod toml;

use eframe::egui::Context;

/// 获取屏幕中心位置
pub(crate) fn screen_center(ctx: &Context) -> eframe::egui::Pos2 {
    ctx.screen_rect().center()
}

