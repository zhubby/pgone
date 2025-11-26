use crate::models::{Settings, Theme};
use egui::{ComboBox, Ui};
use std::fs;
use std::path::Path;

pub struct SettingsPanel {
    previous_font_size: Option<f32>,
}

impl Default for SettingsPanel {
    fn default() -> Self {
        Self {
            previous_font_size: None,
        }
    }
}

impl SettingsPanel {
    /// Get available font names from assets/fonts directory
    pub fn get_available_fonts() -> Vec<String> {
        let fonts_dir = Path::new("assets/fonts");
        let mut fonts = Vec::new();
        
        if let Ok(entries) = fs::read_dir(fonts_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("ttf") {
                    if let Some(font_name) = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                    {
                        fonts.push(font_name);
                    }
                }
            }
        }
        
        // Sort fonts for consistent display
        fonts.sort();
        fonts
    }

    /// Get available font sizes
    pub fn get_available_font_sizes() -> Vec<f32> {
        vec![10.0, 12.0, 14.0, 16.0, 18.0, 20.0, 24.0]
    }

    /// Render settings UI
    pub fn ui(&mut self, ui: &mut Ui, settings: &mut Settings, ctx: &egui::Context) {
        ui.heading("设置");
        ui.separator();
        
        // Theme selection
        ui.horizontal(|ui| {
            ui.label("主题:");
            let old_theme = settings.theme;
            ComboBox::from_id_salt("theme")
                .selected_text(settings.theme.display_name())
                .show_ui(ui, |ui| {
                    for theme in Theme::all() {
                        ui.selectable_value(
                            &mut settings.theme,
                            *theme,
                            theme.display_name(),
                        );
                    }
                });
            if old_theme != settings.theme {
                Self::apply_theme(ctx, settings.theme);
            }
        });
        
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);
        
        ui.heading("字体设置");
        ui.separator();
        
        // Font family selection
        ui.horizontal(|ui| {
            ui.label("字体:");
            let available_fonts = Self::get_available_fonts();
            ComboBox::from_id_salt("font_family")
                .selected_text(&settings.font_family)
                .show_ui(ui, |ui| {
                    for font in &available_fonts {
                        ui.selectable_value(
                            &mut settings.font_family,
                            font.clone(),
                            font,
                        );
                    }
                });
        });
        
        ui.add_space(10.0);
        
        // Font size selection
        let mut size_changed = false;
        ui.horizontal(|ui| {
            ui.label("字号:");
            let available_sizes = Self::get_available_font_sizes();
            let old_size = settings.font_size;
            ComboBox::from_id_salt("font_size")
                .selected_text(format!("{:.0}", settings.font_size))
                .show_ui(ui, |ui| {
                    for size in &available_sizes {
                        ui.selectable_value(
                            &mut settings.font_size,
                            *size,
                            format!("{:.0}", size),
                        );
                    }
                });
            if old_size != settings.font_size {
                size_changed = true;
            }
        });
        
        // Apply font size changes immediately
        if size_changed || self.previous_font_size != Some(settings.font_size) {
            let mut style = (*ctx.style()).clone();
            for text_style in style.text_styles.values_mut() {
                text_style.size = settings.font_size;
            }
            ctx.set_style(style);
            self.previous_font_size = Some(settings.font_size);
        }
        
        // Note: Font family changes require font reloading which is complex
        // For now, we'll save the preference and it will be applied on next startup
        ui.add_space(10.0);
        ui.label("提示: 字体更改将在下次启动时生效");
    }

    /// Apply theme to the context
    pub fn apply_theme(ctx: &egui::Context, theme: Theme) {
        match theme {
            Theme::System => {
                // Follow system theme - use egui's default behavior
                // egui will automatically detect system theme preference
                // We'll use dark mode as default fallback
                ctx.set_visuals(egui::Visuals::dark());
            }
            Theme::Latte => {
                catppuccin_egui::set_theme(ctx, catppuccin_egui::LATTE);
            }
            Theme::Frappe => {
                catppuccin_egui::set_theme(ctx, catppuccin_egui::FRAPPE);
            }
            Theme::Macchiato => {
                catppuccin_egui::set_theme(ctx, catppuccin_egui::MACCHIATO);
            }
            Theme::Mocha => {
                catppuccin_egui::set_theme(ctx, catppuccin_egui::MOCHA);
            }
        }
    }
}

