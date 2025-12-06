use crate::styles::media::MediaCache;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct PreviewState {
    pub path: PathBuf,
    pub zoom: f32,
}

#[derive(Clone, Default)]
pub struct PreviewManager {
    pub media: MediaCache,
    pub preview: Option<PreviewState>,
}

impl PreviewManager {
    pub fn ensure_texture(
        &mut self,
        ctx: &egui::Context,
        path: &PathBuf,
    ) -> Option<eframe::egui::TextureHandle> {
        self.media.ensure_texture(ctx, path)
    }

    pub fn open(&mut self, path: PathBuf) {
        self.preview = Some(PreviewState { path, zoom: 1.0 });
    }

    pub fn ui_window(&mut self, ctx: &egui::Context) {
        if self.preview.is_none() {
            return;
        }
        let (path, mut zoom) = {
            let p = self.preview.as_ref().unwrap();
            (p.path.clone(), p.zoom)
        };
        let mut open = true;
        let center = ctx.screen_rect().center();
        egui::Window::new("Image Preview")
            .open(&mut open)
            .default_pos(center)
            .pivot(egui::Align2::CENTER_CENTER)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("{}", path.display()));
                    ui.add(egui::Slider::new(&mut zoom, 0.1..=5.0).text("Zoom"));
                });
                if let Some(handle) = self.media.ensure_texture(ui.ctx(), &path.clone()) {
                    let tex_size = handle.size_vec2();
                    let size = tex_size * zoom;
                    ui.add(egui::widgets::Image::new(&handle).fit_to_exact_size(size));
                } else {
                    ui.label("[image not available]");
                }
            });
        if open {
            if let Some(p) = &mut self.preview {
                p.zoom = zoom;
            }
        } else {
            self.preview = None;
        }
    }
}
