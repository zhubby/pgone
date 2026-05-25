use crate::skeletons::dock::{DockLayout, DockTab};
use anyhow::{Context, Result};
use egui_dock::DockState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const SETTINGS_FILE_NAME: &str = "settings.json";
const SETTINGS_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GuiLayoutSettings {
    version: u32,
    dock: DockState<DockTab>,
}

#[must_use]
pub fn settings_path() -> PathBuf {
    pgone_storage::app_dir().join(SETTINGS_FILE_NAME)
}

pub fn load_dock_layout() -> DockLayout {
    match try_load_dock_layout() {
        Ok(Some(layout)) => layout,
        Ok(None) => DockLayout::default(),
        Err(error) => {
            tracing::warn!("Failed to load GUI layout settings: {error:#}");
            DockLayout::default()
        }
    }
}

fn try_load_dock_layout() -> Result<Option<DockLayout>> {
    let path = settings_path();
    if !path.exists() {
        return Ok(None);
    }

    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let settings: GuiLayoutSettings = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse {}", path.display()))?;

    if settings.version != SETTINGS_VERSION {
        anyhow::bail!(
            "unsupported settings version {}, expected {}",
            settings.version,
            SETTINGS_VERSION
        );
    }

    let layout = DockLayout::from_state(settings.dock)
        .context("settings dock layout is missing required tabs")?;
    Ok(Some(layout))
}

pub fn save_dock_layout(layout: &DockLayout) -> Result<String> {
    let json = dock_layout_json(layout)?;
    save_json(&json)?;
    Ok(json)
}

pub fn dock_layout_json(layout: &DockLayout) -> Result<String> {
    let settings = GuiLayoutSettings {
        version: SETTINGS_VERSION,
        dock: layout.sanitized_state(),
    };
    Ok(serde_json::to_string_pretty(&settings)?)
}

fn save_json(json: &str) -> Result<()> {
    crate::futures::block_on_async(pgone_storage::ensure_app_dir())?;

    let path = settings_path();
    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, json)
        .with_context(|| format!("failed to write {}", tmp_path.display()))?;
    std::fs::rename(&tmp_path, &path)
        .with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_dock_layout_round_trips_through_settings_json() {
        let layout = DockLayout::default();

        let json = dock_layout_json(&layout).unwrap();
        let decoded: GuiLayoutSettings = serde_json::from_str(&json).unwrap();

        assert!(DockLayout::from_state(decoded.dock).is_some());
    }

    #[test]
    fn dock_layout_rejects_missing_required_tabs() {
        let state = DockState::new(vec![DockTab::SqlEditor]);

        assert!(DockLayout::from_state(state).is_none());
    }

    #[test]
    fn damaged_settings_json_is_rejected() {
        let result = serde_json::from_str::<GuiLayoutSettings>("{not json");

        assert!(result.is_err());
    }

    #[test]
    fn settings_path_lives_in_app_dir() {
        assert_eq!(
            settings_path(),
            pgone_storage::app_dir().join(SETTINGS_FILE_NAME)
        );
    }
}
