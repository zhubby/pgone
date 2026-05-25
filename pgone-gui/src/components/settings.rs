use crate::futures;
use crate::models::{SendShortcut, Settings};
use crate::styles::toggle::toggle;
use egui::{ComboBox, Ui};
use egui_dock::{DockArea, DockState, Style, TabViewer};
use pgone_llm::LLMProvider;
use tokio::sync::mpsc;

const SETTINGS_DOCK_HEIGHT: f32 = 360.0;

pub struct SettingsPanel {
    dock_state: DockState<SettingsTab>,
    previous_font_size: Option<f32>,
    original_settings: Option<Settings>,
    available_models: Vec<String>,
    models_receiver: Option<mpsc::Receiver<Result<Vec<String>, String>>>,
    models_loaded: bool,
    last_api_key: Option<String>,
    last_base_url: Option<String>,
}

impl Default for SettingsPanel {
    fn default() -> Self {
        Self {
            dock_state: default_dock_state(),
            previous_font_size: None,
            original_settings: None,
            available_models: Vec::new(),
            models_receiver: None,
            models_loaded: false,
            last_api_key: None,
            last_base_url: None,
        }
    }
}

impl SettingsPanel {
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
        self.original_settings
            .as_ref()
            .is_some_and(|original| original != current_settings)
    }

    /// Reset original settings (call after saving)
    pub fn reset_original_settings(&mut self, settings: &Settings) {
        self.original_settings = Some(settings.clone());
    }

    /// Clear original settings (call when closing window)
    pub fn clear_original_settings(&mut self) {
        self.original_settings = None;
    }

    /// Render settings UI.
    /// Returns true if save button was clicked.
    pub fn ui(&mut self, ui: &mut Ui, settings: &mut Settings, ctx: &egui::Context) -> bool {
        self.update_model_loader(settings);

        let has_changes = self.has_changes(settings);
        let mut dock_state = std::mem::replace(&mut self.dock_state, default_dock_state());
        let mut viewer = SettingsTabViewer {
            panel: self,
            settings,
            ctx,
        };

        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), SETTINGS_DOCK_HEIGHT),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                DockArea::new(&mut dock_state)
                    .show_leaf_collapse_buttons(false)
                    .show_leaf_close_all_buttons(false)
                    .show_close_buttons(false)
                    .tab_context_menus(false)
                    .style(Style::from_egui(ui.style().as_ref()))
                    .show_inside(ui, &mut viewer);
            },
        );
        viewer.panel.dock_state = dock_state;

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        let mut save_clicked = false;
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("保存").clicked() {
                    viewer.panel.reset_original_settings(viewer.settings);
                    save_clicked = true;
                }
                if has_changes {
                    ui.label(egui::RichText::new("有未保存的更改").color(egui::Color32::YELLOW));
                }
            });
        });

        save_clicked
    }

    #[cfg(test)]
    pub fn tab_titles(&self) -> Vec<&'static str> {
        self.dock_state
            .iter_all_tabs()
            .map(|(_, tab)| tab.title())
            .collect()
    }

    #[cfg(test)]
    pub fn tabs_are_fixed(&self) -> bool {
        let viewer = SettingsTabViewerReadOnly;
        self.dock_state.iter_all_tabs().all(|(_, tab)| {
            let mut tab = tab.clone();
            !viewer.is_closeable(&tab) && !viewer.allowed_in_windows(&mut tab)
        })
    }

    fn update_model_loader(&mut self, settings: &Settings) {
        let api_key_changed = self.last_api_key != settings.openai_api_key;
        let base_url_changed = self.last_base_url != settings.openai_base_url;

        if (api_key_changed || base_url_changed) && settings.openai_api_key.is_some() {
            self.last_api_key = settings.openai_api_key.clone();
            self.last_base_url = settings.openai_base_url.clone();
            self.models_loaded = false;
            self.models_receiver = None;
            self.load_models(settings);
        }

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
                            self.available_models = default_models();
                            self.models_loaded = true;
                        }
                    }
                    self.models_receiver = None;
                }
                Err(mpsc::error::TryRecvError::Empty) => {}
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    self.models_receiver = None;
                }
            }
        }

        if !self.models_loaded
            && settings.openai_api_key.is_some()
            && self.models_receiver.is_none()
        {
            self.load_models(settings);
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
            if proxy_enabled && let (Some(host), Some(port)) = (proxy_host, proxy_port) {
                config = config.with_proxy(host, port);
            }

            let result = match pgone_llm::Client::new(config, provider) {
                Ok(client) => match client.models_list().await {
                    Ok(models) => Ok(models.into_iter().map(|m| m.id).collect()),
                    Err(e) => Err(e.to_string()),
                },
                Err(e) => Err(e.to_string()),
            };

            let _ = sender.send(result).await;
        });
    }
}

fn default_dock_state() -> DockState<SettingsTab> {
    DockState::new(vec![
        SettingsTab::General,
        SettingsTab::Llm,
        SettingsTab::Network,
        SettingsTab::Appearance,
    ])
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SettingsTab {
    General,
    Llm,
    Network,
    Appearance,
}

impl SettingsTab {
    fn title(&self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Llm => "LLM",
            Self::Network => "Network",
            Self::Appearance => "Appearance",
        }
    }
}

struct SettingsTabViewer<'a> {
    panel: &'a mut SettingsPanel,
    settings: &'a mut Settings,
    ctx: &'a egui::Context,
}

impl TabViewer for SettingsTabViewer<'_> {
    type Tab = SettingsTab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.title().into()
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        match tab {
            SettingsTab::General => self.show_general(ui),
            SettingsTab::Llm => self.show_llm(ui),
            SettingsTab::Network => self.show_network(ui),
            SettingsTab::Appearance => self.show_appearance(ui),
        }
    }

    fn is_closeable(&self, _tab: &Self::Tab) -> bool {
        false
    }

    fn allowed_in_windows(&self, _tab: &mut Self::Tab) -> bool {
        false
    }
}

impl SettingsTabViewer<'_> {
    fn show_general(&mut self, ui: &mut Ui) {
        ui.heading("General");
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("发送快捷键:");
            ComboBox::from_id_salt("send_shortcut")
                .selected_text(match self.settings.send_shortcut {
                    SendShortcut::Enter => "Enter",
                    SendShortcut::CmdEnter => "Cmd+Enter / Ctrl+Enter",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.settings.send_shortcut,
                        SendShortcut::Enter,
                        "Enter",
                    );
                    ui.selectable_value(
                        &mut self.settings.send_shortcut,
                        SendShortcut::CmdEnter,
                        "Cmd+Enter / Ctrl+Enter",
                    );
                });
        });

        ui.add_space(12.0);
        ui.checkbox(&mut self.settings.enable_monitor, "启用系统监控");
        ui.label("在状态栏显示当前进程的 CPU、内存和网络使用情况");
    }

    fn show_llm(&mut self, ui: &mut Ui) {
        ui.heading("LLM");
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("模型供应商:");
            ComboBox::from_id_salt("llm_provider")
                .selected_text(SettingsPanel::llm_provider_display_name(
                    &self.settings.llm_provider,
                ))
                .show_ui(ui, |ui| {
                    for provider in SettingsPanel::all_llm_providers() {
                        ui.selectable_value(
                            &mut self.settings.llm_provider,
                            *provider,
                            SettingsPanel::llm_provider_display_name(provider),
                        );
                    }
                });
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("API Key:");
            let mut api_key_str = self
                .settings
                .openai_api_key
                .as_deref()
                .unwrap_or("")
                .to_string();
            if ui.text_edit_singleline(&mut api_key_str).changed() {
                self.settings.openai_api_key = (!api_key_str.is_empty()).then_some(api_key_str);
            }
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("Base URL:");
            let mut base_url_str = self
                .settings
                .openai_base_url
                .as_deref()
                .unwrap_or("")
                .to_string();
            if ui.text_edit_singleline(&mut base_url_str).changed() {
                self.settings.openai_base_url = (!base_url_str.is_empty()).then_some(base_url_str);
            }
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("启用流式 API:");
            ui.add(toggle(&mut self.settings.enable_stream_api));
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("模型:");
            let available_models = if self.panel.available_models.is_empty() {
                default_models()
            } else {
                self.panel.available_models.clone()
            };
            let display_text = if self.panel.models_receiver.is_some() {
                format!("{} (加载中...)", self.settings.openai_model)
            } else {
                self.settings.openai_model.clone()
            };

            ComboBox::from_id_salt("openai_model")
                .selected_text(&display_text)
                .show_ui(ui, |ui| {
                    for model in &available_models {
                        ui.selectable_value(&mut self.settings.openai_model, model.clone(), model);
                    }
                });
        });
    }

    fn show_network(&mut self, ui: &mut Ui) {
        ui.heading("Network");
        ui.separator();

        ui.checkbox(&mut self.settings.proxy_enabled, "启用网络代理");
        ui.add_enabled_ui(self.settings.proxy_enabled, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("代理地址:");
                let mut proxy_host_str = self
                    .settings
                    .proxy_host
                    .as_deref()
                    .unwrap_or("")
                    .to_string();
                if ui.text_edit_singleline(&mut proxy_host_str).changed() {
                    self.settings.proxy_host =
                        (!proxy_host_str.is_empty()).then_some(proxy_host_str);
                }
            });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("代理端口:");
                let mut proxy_port_str = self
                    .settings
                    .proxy_port
                    .map(|p| p.to_string())
                    .unwrap_or_default();
                if ui.text_edit_singleline(&mut proxy_port_str).changed() {
                    self.settings.proxy_port = if proxy_port_str.is_empty() {
                        None
                    } else {
                        proxy_port_str.parse::<u16>().ok()
                    };
                }
            });
        });
    }

    fn show_appearance(&mut self, ui: &mut Ui) {
        ui.heading("Appearance");
        ui.separator();

        let mut size_changed = false;
        ui.horizontal(|ui| {
            ui.label("字号:");
            let old_size = self.settings.font_size;
            ComboBox::from_id_salt("font_size")
                .selected_text(format!("{:.0}", self.settings.font_size))
                .show_ui(ui, |ui| {
                    for size in SettingsPanel::get_available_font_sizes() {
                        ui.selectable_value(
                            &mut self.settings.font_size,
                            size,
                            format!("{:.0}", size),
                        );
                    }
                });
            size_changed = old_size != self.settings.font_size;
        });

        if size_changed || self.panel.previous_font_size != Some(self.settings.font_size) {
            let mut style = (*self.ctx.style()).clone();
            for text_style in style.text_styles.values_mut() {
                text_style.size = self.settings.font_size;
            }
            self.ctx.set_style(style);
            self.panel.previous_font_size = Some(self.settings.font_size);
        }
    }
}

#[cfg(test)]
struct SettingsTabViewerReadOnly;

#[cfg(test)]
impl TabViewer for SettingsTabViewerReadOnly {
    type Tab = SettingsTab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.title().into()
    }

    fn ui(&mut self, _ui: &mut Ui, _tab: &mut Self::Tab) {}

    fn is_closeable(&self, _tab: &Self::Tab) -> bool {
        false
    }

    fn allowed_in_windows(&self, _tab: &mut Self::Tab) -> bool {
        false
    }
}

fn default_models() -> Vec<String> {
    vec![
        "gpt-4o-mini".to_string(),
        "gpt-4o".to_string(),
        "gpt-4-turbo".to_string(),
        "gpt-4".to_string(),
        "gpt-3.5-turbo".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_panel_has_four_expected_tabs() {
        let panel = SettingsPanel::default();

        assert_eq!(
            panel.tab_titles(),
            vec!["General", "LLM", "Network", "Appearance"]
        );
    }

    #[test]
    fn settings_tabs_are_fixed() {
        let panel = SettingsPanel::default();

        assert!(panel.tabs_are_fixed());
    }
}
