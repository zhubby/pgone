use crate::models::Settings;
use crate::skeletons::dock::{DockLayout, DockTab};
use anyhow::{Context, Result};
use egui_dock::DockState;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const SETTINGS_FILE_NAME: &str = "settings.json";
const SETTINGS_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiSettingsDocument {
    pub version: u32,
    #[serde(default)]
    pub app: Settings,
    #[serde(default)]
    pub layout: LayoutSettings,
    #[serde(default)]
    pub window: WindowSettings,
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

impl Default for GuiSettingsDocument {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            app: Settings::default(),
            layout: LayoutSettings::default(),
            window: WindowSettings::default(),
            unknown: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutSettings {
    #[serde(default = "default_dock_state")]
    pub dock: DockState<DockTab>,
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

impl Default for LayoutSettings {
    fn default() -> Self {
        Self {
            dock: default_dock_state(),
            unknown: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowSettings {
    #[serde(flatten)]
    pub unknown: BTreeMap<String, Value>,
}

pub struct SettingsStore;

impl SettingsStore {
    #[must_use]
    pub fn path() -> PathBuf {
        pgone_storage::app_dir().join(SETTINGS_FILE_NAME)
    }

    pub fn load() -> Result<Option<GuiSettingsDocument>> {
        load_from_path(&Self::path())
    }

    pub fn load_app_settings() -> Settings {
        match Self::load() {
            Ok(Some(document)) => document.app,
            Ok(None) => Settings::default(),
            Err(error) => {
                tracing::warn!("Failed to load GUI app settings: {error:#}");
                Settings::default()
            }
        }
    }

    pub fn load_dock_layout() -> DockLayout {
        match Self::load() {
            Ok(Some(document)) => {
                DockLayout::from_state(document.layout.dock).unwrap_or_else(DockLayout::default)
            }
            Ok(None) => DockLayout::default(),
            Err(error) => {
                tracing::warn!("Failed to load GUI dock layout: {error:#}");
                DockLayout::default()
            }
        }
    }

    pub fn save_app_settings(settings: &Settings) -> Result<String> {
        let mut document = Self::load()?.unwrap_or_default();
        document.version = SETTINGS_VERSION;
        document.app = settings.clone();
        save_document_to_path(&Self::path(), &document)
    }

    pub fn save_dock_layout(layout: &DockLayout) -> Result<String> {
        let mut document = Self::load()?.unwrap_or_default();
        document.version = SETTINGS_VERSION;
        document.layout.dock = layout.sanitized_state();
        save_document_to_path(&Self::path(), &document)
    }

    pub fn dock_layout_json(layout: &DockLayout) -> Result<String> {
        let mut document = GuiSettingsDocument::default();
        document.layout.dock = layout.sanitized_state();
        Ok(serde_json::to_string_pretty(&document)?)
    }
}

fn default_dock_state() -> DockState<DockTab> {
    DockLayout::default().sanitized_state()
}

fn load_from_path(path: &Path) -> Result<Option<GuiSettingsDocument>> {
    if !path.exists() {
        return Ok(None);
    }

    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let value: Value = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(Some(document_from_value(value)?))
}

fn document_from_value(mut value: Value) -> Result<GuiSettingsDocument> {
    normalize_legacy_layout_document(&mut value)?;
    let mut document: GuiSettingsDocument = serde_json::from_value(value)?;
    if document.version != SETTINGS_VERSION {
        anyhow::bail!(
            "unsupported settings version {}, expected {}",
            document.version,
            SETTINGS_VERSION
        );
    }
    Ok(document)
}

fn normalize_legacy_layout_document(value: &mut Value) -> Result<()> {
    let Some(object) = value.as_object_mut() else {
        anyhow::bail!("settings document must be a JSON object");
    };

    if object.contains_key("layout") {
        return Ok(());
    }

    if let Some(dock) = object.remove("dock") {
        let mut layout = Map::new();
        layout.insert("dock".to_string(), dock);
        object.insert("layout".to_string(), Value::Object(layout));
    }

    Ok(())
}

fn save_document_to_path(path: &Path, document: &GuiSettingsDocument) -> Result<String> {
    crate::futures::block_on_async(pgone_storage::ensure_app_dir())?;
    save_document_to_path_without_app_dir(path, document)
}

fn save_document_to_path_without_app_dir(
    path: &Path,
    document: &GuiSettingsDocument,
) -> Result<String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let json = serde_json::to_string_pretty(document)?;
    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, &json)
        .with_context(|| format!("failed to write {}", tmp_path.display()))?;
    std::fs::rename(&tmp_path, path)
        .with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_settings_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pgone-settings-store-{name}-{}-{}.json",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        path
    }

    fn valid_dock_value() -> Value {
        serde_json::to_value(DockLayout::default().sanitized_state()).unwrap()
    }

    #[test]
    fn new_settings_document_round_trips() {
        let mut document = GuiSettingsDocument::default();
        document.app.openai_model = "gpt-4o".to_string();
        document
            .unknown
            .insert("future".to_string(), serde_json::json!({"enabled": true}));

        let json = serde_json::to_string_pretty(&document).unwrap();
        let decoded = document_from_value(serde_json::from_str(&json).unwrap()).unwrap();

        assert_eq!(decoded.app.openai_model, "gpt-4o");
        assert!(DockLayout::from_state(decoded.layout.dock).is_some());
        assert_eq!(decoded.unknown["future"]["enabled"], true);
    }

    #[test]
    fn legacy_layout_only_document_loads_with_default_app_settings() {
        let legacy = serde_json::json!({
            "version": SETTINGS_VERSION,
            "dock": valid_dock_value()
        });

        let decoded = document_from_value(legacy).unwrap();

        assert_eq!(decoded.app, Settings::default());
        assert!(DockLayout::from_state(decoded.layout.dock).is_some());
    }

    #[test]
    fn saving_app_settings_preserves_layout_and_unknown_fields() {
        let path = temp_settings_path("app");
        let mut document = GuiSettingsDocument::default();
        document.app.openai_model = "old-model".to_string();
        document
            .unknown
            .insert("future".to_string(), serde_json::json!({"x": 1}));
        save_document_to_path_without_app_dir(&path, &document).unwrap();

        let mut loaded = load_from_path(&path).unwrap().unwrap();
        loaded.app.openai_model = "new-model".to_string();
        save_document_to_path_without_app_dir(&path, &loaded).unwrap();
        let reloaded = load_from_path(&path).unwrap().unwrap();

        assert_eq!(reloaded.app.openai_model, "new-model");
        assert!(DockLayout::from_state(reloaded.layout.dock).is_some());
        assert_eq!(reloaded.unknown["future"]["x"], 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn saving_dock_layout_preserves_app_settings() {
        let path = temp_settings_path("layout");
        let mut document = GuiSettingsDocument::default();
        document.app.openai_model = "keep-me".to_string();
        save_document_to_path_without_app_dir(&path, &document).unwrap();

        let mut loaded = load_from_path(&path).unwrap().unwrap();
        loaded.layout.dock = DockLayout::default().sanitized_state();
        save_document_to_path_without_app_dir(&path, &loaded).unwrap();
        let reloaded = load_from_path(&path).unwrap().unwrap();

        assert_eq!(reloaded.app.openai_model, "keep-me");
        assert!(DockLayout::from_state(reloaded.layout.dock).is_some());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn damaged_settings_json_is_rejected() {
        let result = serde_json::from_str::<Value>("{not json");

        assert!(result.is_err());
    }

    #[test]
    fn settings_path_lives_in_app_dir() {
        assert_eq!(
            SettingsStore::path(),
            pgone_storage::app_dir().join(SETTINGS_FILE_NAME)
        );
    }
}
