use crate::futures;
use crate::models::{SendShortcut, Settings, Theme};
use crate::styles::toggle::toggle;
use egui::{ComboBox, Ui};
use pgone_llm::LLMProvider;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

pub struct SettingsPanel {
    previous_font_size: Option<f32>,
    original_settings: Option<Settings>,
    settings: Settings,
    available_models: Vec<String>,
    models_receiver: Option<mpsc::Receiver<Result<Vec<String>, String>>>,
    models_loaded: bool,
    last_api_key: Option<String>,
    last_base_url: Option<String>,
}

impl Default for SettingsPanel {
    fn default() -> Self {
        Self {
            previous_font_size: None,
            original_settings: None,
            settings: Settings::default(),
            available_models: Vec::new(),
            models_receiver: None,
            models_loaded: false,
            last_api_key: None,
            last_base_url: None,
        }
    }
}

impl SettingsPanel {
    pub fn new(settings: Settings) -> Self {
        Self {
            previous_font_size: None,
            original_settings: None,
            settings: settings.clone(),
            available_models: Vec::new(),
            models_receiver: None,
            models_loaded: false,
            last_api_key: settings.openai_api_key.clone(),
            last_base_url: settings.openai_base_url.clone(),
        }
    }

    fn asset_path(path: impl AsRef<Path>) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join(path)
    }

    fn font_dirs() -> [PathBuf; 2] {
        [Self::asset_path("fonts"), Self::asset_path("")]
    }

    /// Get available font names from crate assets directories.
    pub fn get_available_fonts() -> Vec<String> {
        let mut fonts = Vec::new();

        for fonts_dir in Self::font_dirs() {
            if let Ok(entries) = fs::read_dir(fonts_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                        if ext == "ttf" || ext == "otf" {
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

    /// Get all available LLM providers
    pub fn all_llm_providers() -> &'static [LLMProvider] {
        &[
            LLMProvider::OpenAI,
            LLMProvider::Gemini,
            LLMProvider::Moonshot,
            LLMProvider::DeepSeek,
            LLMProvider::Ollama,
            LLMProvider::BigModel,
            LLMProvider::OpenRouter,
        ]
    }

    /// Get display name for LLM provider
    pub fn llm_provider_display_name(provider: &LLMProvider) -> &'static str {
        match provider {
            LLMProvider::OpenAI => "OpenAI",
            LLMProvider::Gemini => "Google Gemini",
            LLMProvider::Moonshot => "Moonshot",
            LLMProvider::DeepSeek => "DeepSeek",
            LLMProvider::Ollama => "Ollama",
            LLMProvider::BigModel => "BigModel",
            LLMProvider::OpenRouter => "OpenRouter",
        }
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
        // 检查 API key 或 base_url 是否改变，如果改变则重新加载模型列表
        let api_key_changed = self.last_api_key != settings.openai_api_key;
        let base_url_changed = self.last_base_url != settings.openai_base_url;

        if (api_key_changed || base_url_changed) && settings.openai_api_key.is_some() {
            self.last_api_key = settings.openai_api_key.clone();
            self.last_base_url = settings.openai_base_url.clone();
            self.models_loaded = false;
            self.models_receiver = None;
            self.load_models(settings);
        }

        // 检查模型加载结果
        if let Some(ref mut receiver) = self.models_receiver {
            match receiver.try_recv() {
                Ok(result) => {
                    match result {
                        Ok(models) => {
                            self.available_models = models;
                            self.models_loaded = true;
                        }
                        Err(e) => {
                            let message = format!("加载模型列表失败: {}", e);
                            crate::notify::error(&message);
                            tracing::error!("{}", message);
                            // 如果加载失败，使用默认模型列表
                            self.available_models = vec![
                                "gpt-4o-mini".to_string(),
                                "gpt-4o".to_string(),
                                "gpt-4-turbo".to_string(),
                                "gpt-4".to_string(),
                                "gpt-3.5-turbo".to_string(),
                            ];
                            self.models_loaded = true;
                        }
                    }
                    self.models_receiver = None;
                }
                Err(mpsc::error::TryRecvError::Empty) => {
                    // 还没有结果，继续等待
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    // Channel已断开，清理
                    self.models_receiver = None;
                }
            }
        }

        // 如果还没有加载过且有 API key，则开始加载
        if !self.models_loaded
            && settings.openai_api_key.is_some()
            && self.models_receiver.is_none()
        {
            self.load_models(settings);
        }

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
                    ui.selectable_value(&mut settings.send_shortcut, SendShortcut::Enter, "Enter");
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

        ui.heading("LLM 配置");
        ui.separator();

        // LLM Provider selection
        ui.horizontal(|ui| {
            ui.label("模型供应商:");
            ComboBox::from_id_salt("llm_provider")
                .selected_text(Self::llm_provider_display_name(&settings.llm_provider))
                .show_ui(ui, |ui| {
                    for provider in Self::all_llm_providers() {
                        ui.selectable_value(
                            &mut settings.llm_provider,
                            *provider,
                            Self::llm_provider_display_name(provider),
                        );
                    }
                });
        });

        ui.add_space(5.0);

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
            let mut base_url_str = settings
                .openai_base_url
                .as_deref()
                .unwrap_or("")
                .to_string();
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

        // Proxy Configuration
        ui.checkbox(&mut settings.proxy_enabled, "启用网络代理");
        if settings.proxy_enabled {
            ui.group(|ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label("代理地址:");
                    let mut proxy_host_str =
                        settings.proxy_host.as_deref().unwrap_or("").to_string();
                    let response = ui.text_edit_singleline(&mut proxy_host_str);
                    if response.changed() {
                        settings.proxy_host = if proxy_host_str.is_empty() {
                            None
                        } else {
                            Some(proxy_host_str)
                        };
                    }
                });
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.label("代理端口:");
                    let mut proxy_port_str = settings
                        .proxy_port
                        .map(|p| p.to_string())
                        .unwrap_or_default();
                    let response = ui.text_edit_singleline(&mut proxy_port_str);
                    if response.changed() {
                        settings.proxy_port = if proxy_port_str.is_empty() {
                            None
                        } else {
                            proxy_port_str.parse::<u16>().ok()
                        };
                    }
                });
            });
        }

        ui.add_space(5.0);

        // Stream API toggle
        ui.horizontal(|ui| {
            ui.label("启用流式 API:");
            ui.add(toggle(&mut settings.enable_stream_api));
        });

        ui.add_space(5.0);

        // OpenAI Model
        ui.horizontal(|ui| {
            ui.label("模型:");
            let available_models = if self.available_models.is_empty() {
                // 如果还没有加载，显示默认模型列表
                vec![
                    "gpt-4o-mini".to_string(),
                    "gpt-4o".to_string(),
                    "gpt-4-turbo".to_string(),
                    "gpt-4".to_string(),
                    "gpt-3.5-turbo".to_string(),
                ]
            } else {
                self.available_models.clone()
            };

            // 如果正在加载，显示加载状态
            let display_text = if self.models_receiver.is_some() {
                format!("{} (加载中...)", settings.openai_model)
            } else {
                settings.openai_model.clone()
            };

            ComboBox::from_id_salt("openai_model")
                .selected_text(&display_text)
                .show_ui(ui, |ui| {
                    for model in &available_models {
                        let model_str = model.clone();
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
                        ui.selectable_value(&mut settings.theme, *theme, theme.display_name());
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
                        ui.selectable_value(&mut settings.font_family, font.clone(), font);
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
                        ui.selectable_value(&mut settings.font_size, *size, format!("{:.0}", size));
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

        ui.heading("系统选项");
        ui.separator();

        // Enable monitor checkbox
        ui.checkbox(&mut settings.enable_monitor, "启用系统监控");
        ui.label("在状态栏显示当前进程的 CPU、内存和网络使用情况");

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
                apply_catppuccin_visuals(ctx, CatppuccinPalette::latte());
            }
            Theme::Frappe => {
                apply_catppuccin_visuals(ctx, CatppuccinPalette::frappe());
            }
            Theme::Macchiato => {
                apply_catppuccin_visuals(ctx, CatppuccinPalette::macchiato());
            }
            Theme::Mocha => {
                apply_catppuccin_visuals(ctx, CatppuccinPalette::mocha());
            }
        }
    }

    fn load_models(&mut self, settings: &Settings) {
        let Some(api_key) = settings.openai_api_key.clone() else {
            return;
        };

        let provider = settings.llm_provider;
        let base_url = settings.openai_base_url.clone();
        let proxy_enabled = settings.proxy_enabled;
        let proxy_host = settings.proxy_host.clone();
        let proxy_port = settings.proxy_port;
        let (sender, receiver) = mpsc::channel(1);
        self.models_receiver = Some(receiver);

        futures::spawn(async move {
            let mut config = pgone_llm::Config::new(api_key);
            if let Some(url) = base_url {
                config = config.with_base_url(url);
            }
            if proxy_enabled {
                if let (Some(host), Some(port)) = (proxy_host, proxy_port) {
                    config = config.with_proxy(host, port);
                }
            }

            let result = match pgone_llm::Client::new(config, provider) {
                Ok(client) => match client.models_list().await {
                    Ok(models) => {
                        let model_ids: Vec<String> = models.into_iter().map(|m| m.id).collect();
                        Ok(model_ids)
                    }
                    Err(e) => Err(e.to_string()),
                },
                Err(e) => Err(e.to_string()),
            };

            let _ = sender.send(result).await;
        });
    }
}

struct CatppuccinPalette {
    base: egui::Color32,
    surface0: egui::Color32,
    surface1: egui::Color32,
    text: egui::Color32,
    subtext: egui::Color32,
    accent: egui::Color32,
}

impl CatppuccinPalette {
    fn latte() -> Self {
        Self {
            base: egui::Color32::from_rgb(239, 241, 245),
            surface0: egui::Color32::from_rgb(204, 208, 218),
            surface1: egui::Color32::from_rgb(188, 192, 204),
            text: egui::Color32::from_rgb(76, 79, 105),
            subtext: egui::Color32::from_rgb(108, 111, 133),
            accent: egui::Color32::from_rgb(30, 102, 245),
        }
    }

    fn frappe() -> Self {
        Self {
            base: egui::Color32::from_rgb(48, 52, 70),
            surface0: egui::Color32::from_rgb(65, 69, 89),
            surface1: egui::Color32::from_rgb(81, 87, 109),
            text: egui::Color32::from_rgb(198, 208, 245),
            subtext: egui::Color32::from_rgb(181, 191, 226),
            accent: egui::Color32::from_rgb(140, 170, 238),
        }
    }

    fn macchiato() -> Self {
        Self {
            base: egui::Color32::from_rgb(36, 39, 58),
            surface0: egui::Color32::from_rgb(54, 58, 79),
            surface1: egui::Color32::from_rgb(73, 77, 100),
            text: egui::Color32::from_rgb(202, 211, 245),
            subtext: egui::Color32::from_rgb(184, 192, 224),
            accent: egui::Color32::from_rgb(138, 173, 244),
        }
    }

    fn mocha() -> Self {
        Self {
            base: egui::Color32::from_rgb(30, 30, 46),
            surface0: egui::Color32::from_rgb(49, 50, 68),
            surface1: egui::Color32::from_rgb(69, 71, 90),
            text: egui::Color32::from_rgb(205, 214, 244),
            subtext: egui::Color32::from_rgb(186, 194, 222),
            accent: egui::Color32::from_rgb(137, 180, 250),
        }
    }
}

fn apply_catppuccin_visuals(ctx: &egui::Context, palette: CatppuccinPalette) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = palette.base;
    visuals.window_fill = palette.base;
    visuals.extreme_bg_color = palette.base;
    visuals.faint_bg_color = palette.surface0;
    visuals.widgets.noninteractive.bg_fill = palette.surface0;
    visuals.widgets.noninteractive.fg_stroke.color = palette.text;
    visuals.widgets.inactive.bg_fill = palette.surface0;
    visuals.widgets.inactive.fg_stroke.color = palette.subtext;
    visuals.widgets.hovered.bg_fill = palette.surface1;
    visuals.widgets.hovered.fg_stroke.color = palette.text;
    visuals.widgets.active.bg_fill = palette.accent;
    visuals.widgets.active.fg_stroke.color = palette.base;
    visuals.selection.bg_fill = palette.accent;
    visuals.selection.stroke.color = palette.text;
    visuals.override_text_color = Some(palette.text);
    ctx.set_visuals(visuals);
}
