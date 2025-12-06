use eframe::egui::TextureHandle;
use eframe::epaint::ColorImage;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Default)]
pub struct MediaCache {
    pub textures: HashMap<PathBuf, TextureHandle>,
}
// Default is derived

impl MediaCache {
    pub fn ensure_texture(&mut self, ctx: &egui::Context, path: &PathBuf) -> Option<TextureHandle> {
        if let Some(handle) = self.textures.get(path) {
            return Some(handle.clone());
        }
        let img = image::open(path).ok()?;
        let size = [img.width() as usize, img.height() as usize];
        let rgba = img.to_rgba8();
        let color_image = ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
        let handle = ctx.load_texture(
            "img:".to_owned() + &path.to_string_lossy(),
            color_image,
            egui::TextureOptions::LINEAR,
        );
        self.textures.insert(path.clone(), handle.clone());
        Some(handle)
    }
}
