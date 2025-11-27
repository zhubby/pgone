use crate::models::{Settings, SendShortcut, Theme};
use egui::{ComboBox, Ui};
use std::fs;
use std::path::Path;

pub struct SettingsPanel {
    previous_font_size: Option<f32>,
    original_settings: Option<Settings>,
    settings: Settings,
}

impl Default for SettingsPanel {
    fn default() -> Self {
        Self {
            previous_font_size: None,
            original_settings: None,
            settings: Settings::default(),
        }
    }
}

impl SettingsPanel {

    pub fn new(settings: Settings) -> Self {
        Self {
            previous_font_size: None,
            original_settings: None,
            settings,
        }
    }

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

    /// Initialize original settings (call when opening settings window)
    pub fn init_original_settings(&mut self, settings: &Settings) {
        self.original_settings = Some(settings.clone());
    }
    
    /// Check if original settings are initialized
    pub fn has_original_settings(&self) -> bool {
        self.original_settings.is_some()
    }
    
    /// Check if settings have been modified
    pub fn has_changes(&self, current_settings: &Settings) -> bool {
        if let Some(ref original) = self.original_settings {
            original != current_settings
        } else {
            false
        }
    }
    
    /// Reset original settings (call after saving)
    pub fn reset_original_settings(&mut self, settings: &Settings) {
        self.original_settings = Some(settings.clone());
    }
    
    /// Clear original settings (call when closing window)
    pub fn clear_original_settings(&mut self) {
        self.original_settings = None;
    }

    /// Render settings UI
    /// Returns true if save button was clicked
    pub fn ui(&mut self, ui: &mut Ui, settings: &mut Settings, ctx: &egui::Context) -> bool {
        ui.heading("设置");
        ui.separator();
        
        // Send shortcut selection
        ui.horizontal(|ui| {
            ui.label("发送快捷键:");
            ComboBox::from_id_salt("send_shortcut")
                .selected_text(match settings.send_shortcut {
                    SendShortcut::Enter => "Enter",
                    SendShortcut::CmdEnter => "Cmd+Enter / Ctrl+Enter",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut settings.send_shortcut,
                        SendShortcut::Enter,
                        "Enter",
                    );
                    ui.selectable_value(
                        &mut settings.send_shortcut,
                        SendShortcut::CmdEnter,
                        "Cmd+Enter / Ctrl+Enter",
                    );
                });
        });
        
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);
        
        ui.heading("OpenAI 配置");
        ui.separator();
        
        // OpenAI API Key
        ui.horizontal(|ui| {
            ui.label("API Key:");
            // Use a mutable reference to the Option<String> directly
            let mut api_key_str = settings.openai_api_key.as_deref().unwrap_or("").to_string();
            let response = ui.text_edit_singleline(&mut api_key_str);
            if response.changed() {
                settings.openai_api_key = if api_key_str.is_empty() {
                    None
                } else {
                    Some(api_key_str)
                };
            }
        });
        
        ui.add_space(5.0);
        
        // OpenAI Base URL
        ui.horizontal(|ui| {
            ui.label("Base URL:");
            // Use a mutable reference to the Option<String> directly
            let mut base_url_str = settings.openai_base_url.as_deref().unwrap_or("").to_string();
            let response = ui.text_edit_singleline(&mut base_url_str);
            if response.changed() {
                settings.openai_base_url = if base_url_str.is_empty() {
                    None
                } else {
                    Some(base_url_str)
                };
            }
        });
        
        ui.add_space(5.0);
        
        // OpenAI Model
        ui.horizontal(|ui| {
            ui.label("模型:");
            let available_models = vec![
                "gpt-4o-mini",
                "gpt-4o",
                "gpt-4-turbo",
                "gpt-4",
                "gpt-3.5-turbo",
            ];
            ComboBox::from_id_salt("openai_model")
                .selected_text(&settings.openai_model)
                .show_ui(ui, |ui| {
                    for model in &available_models {
                        let model_str = model.to_string();
                        ui.selectable_value(
                            &mut settings.openai_model,
                            model_str.clone(),
                            model_str,
                        );
                    }
                });
        });
        
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);
        
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
        
        ui.add_space(20.0);
        ui.separator();
        ui.add_space(10.0);
        
        // Save button
        let has_changes = self.has_changes(settings);
        let mut save_clicked = false;
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("保存").clicked() {
                    self.reset_original_settings(settings);
                    save_clicked = true;
                }
                if has_changes {
                    ui.label(egui::RichText::new("有未保存的更改").color(egui::Color32::YELLOW));
                }
            });
        });
        
        save_clicked
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

