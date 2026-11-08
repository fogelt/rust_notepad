//! App settings (assistant credentials + appearance) persisted across launches.
//!
//! The key is stored in plaintext, like most desktop apps store tokens:
//! readable only by the local user profile, never logged or committed
//! (`.env` and the config file both stay out of git).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Editor font families offered in Settings.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EditorFont {
    #[default]
    Sans,
    Monospace,
}

fn default_editor_font_size() -> f32 {
    14.5
}

fn default_editor_line_height() -> f32 {
    20.0
}

fn default_preview_font_size() -> f32 {
    15.5
}

fn default_preview_line_spacing() -> f32 {
    1.45
}

/// Appearance settings, all optional so older config files keep loading.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UiSettings {
    #[serde(default)]
    pub editor_font: EditorFont,
    #[serde(default = "default_editor_font_size")]
    pub editor_font_size: f32,
    #[serde(default = "default_editor_line_height")]
    pub editor_line_height: f32,
    #[serde(default)]
    pub preview_font: EditorFont,
    #[serde(default = "default_preview_font_size")]
    pub preview_font_size: f32,
    #[serde(default = "default_preview_line_spacing")]
    pub preview_line_spacing: f32,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            editor_font: EditorFont::Sans,
            editor_font_size: default_editor_font_size(),
            editor_line_height: default_editor_line_height(),
            preview_font: EditorFont::Sans,
            preview_font_size: default_preview_font_size(),
            preview_line_spacing: default_preview_line_spacing(),
        }
    }
}

impl UiSettings {
    pub const EDITOR_SIZE_RANGE: std::ops::RangeInclusive<f32> = 12.0..=24.0;
    pub const EDITOR_LINE_RANGE: std::ops::RangeInclusive<f32> = 16.0..=30.0;
    pub const PREVIEW_SIZE_RANGE: std::ops::RangeInclusive<f32> = 13.0..=22.0;
    pub const PREVIEW_LINE_RANGE: std::ops::RangeInclusive<f32> = 1.1..=2.0;
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub ui: UiSettings,
}

/// First non-empty value wins: stored setting, then environment variable.
pub fn pick(stored: &str, env_name: &str) -> String {
    if !stored.trim().is_empty() {
        return stored.trim().to_string();
    }
    std::env::var(env_name)
        .map(|v| v.trim().to_string())
        .unwrap_or_default()
}

pub fn config_path() -> PathBuf {
    if let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) {
        return appdata.join("rust_notepad").join("config.json");
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        return home
            .join(".config")
            .join("rust_notepad")
            .join("config.json");
    }
    PathBuf::from(".rust_notepad.json")
}

pub fn load() -> AppConfig {
    std::fs::read_to_string(config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Write settings; returns the file path on success.
pub fn save(cfg: &AppConfig) -> Result<PathBuf, String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Could not create {parent:?}: {e}"))?;
    }
    let json =
        serde_json::to_string_pretty(cfg).map_err(|e| format!("Could not encode settings: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("Could not write {path:?}: {e}"))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_through_json() {
        let cfg = AppConfig {
            api_key: "AIza-test".to_string(),
            model: "gemini-2.5-flash".to_string(),
            ui: UiSettings {
                editor_font: EditorFont::Monospace,
                editor_font_size: 17.0,
                editor_line_height: 24.0,
                preview_font: EditorFont::Monospace,
                preview_font_size: 18.0,
                preview_line_spacing: 1.6,
            },
        };
        let json = serde_json::to_string(&cfg).unwrap();
        assert_eq!(serde_json::from_str::<AppConfig>(&json).unwrap(), cfg);
    }

    #[test]
    fn old_config_without_ui_uses_defaults() {
        let cfg: AppConfig =
            serde_json::from_str(r#"{"api_key":"AIza-test","model":"gemini-2.5-flash"}"#).unwrap();
        assert_eq!(cfg.ui, UiSettings::default());
        assert_eq!(cfg.api_key, "AIza-test");
    }

    #[test]
    fn ignores_broken_config() {
        assert_eq!(
            serde_json::from_str::<AppConfig>("{oops").unwrap_or_default(),
            AppConfig::default()
        );
    }

    #[test]
    fn pick_prefers_stored_over_env() {
        assert_eq!(pick("stored", "DEFINITELY_UNSET_VAR_XYZ"), "stored");
        assert_eq!(pick("  ", "DEFINITELY_UNSET_VAR_XYZ"), "");
    }
}
