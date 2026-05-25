use crate::settings_store::SettingsStore;
use crate::skeletons::dock::DockLayout;
use anyhow::Result;
use std::path::PathBuf;

#[must_use]
pub fn settings_path() -> PathBuf {
    SettingsStore::path()
}

pub fn load_dock_layout() -> DockLayout {
    SettingsStore::load_dock_layout()
}

pub fn save_dock_layout(layout: &DockLayout) -> Result<String> {
    SettingsStore::save_dock_layout(layout)
}

pub fn dock_layout_json(layout: &DockLayout) -> Result<String> {
    SettingsStore::dock_layout_json(layout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_dock::DockState;

    #[test]
    fn default_dock_layout_round_trips_through_settings_json() {
        let layout = DockLayout::default();

        let json = dock_layout_json(&layout).unwrap();
        let decoded: crate::settings_store::GuiSettingsDocument =
            serde_json::from_str(&json).unwrap();

        assert!(DockLayout::from_state(decoded.layout.dock).is_some());
    }

    #[test]
    fn dock_layout_rejects_missing_required_tabs() {
        let state = DockState::new(vec![crate::skeletons::dock::DockTab::SqlEditor]);

        assert!(DockLayout::from_state(state).is_none());
    }

    #[test]
    fn damaged_settings_json_is_rejected() {
        let result =
            serde_json::from_str::<crate::settings_store::GuiSettingsDocument>("{not json");

        assert!(result.is_err());
    }

    #[test]
    fn settings_path_lives_in_app_dir() {
        assert_eq!(
            settings_path(),
            pgone_storage::app_dir().join("settings.json")
        );
    }
}
