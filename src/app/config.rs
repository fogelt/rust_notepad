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

fn default_true() -> bool {
    true
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
    22.5
}

fn default_accent() -> [u8; 3] {
    let color = crate::app::theme::DEFAULT_ACCENT;
    [color.r(), color.g(), color.b()]
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
    /// Follow the active provider's branding color for accent chrome.
    #[serde(default = "default_true")]
    pub use_provider_color: bool,
    /// Accent used when provider colors are turned off, as sRGB.
    #[serde(default = "default_accent")]
    pub accent: [u8; 3],
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
            use_provider_color: true,
            accent: default_accent(),
        }
    }
}

impl UiSettings {
    pub const EDITOR_SIZE_RANGE: std::ops::RangeInclusive<f32> = 12.0..=24.0;
    pub const EDITOR_LINE_RANGE: std::ops::RangeInclusive<f32> = 16.0..=30.0;
    pub const PREVIEW_SIZE_RANGE: std::ops::RangeInclusive<f32> = 13.0..=22.0;
    pub const PREVIEW_LINE_RANGE: std::ops::RangeInclusive<f32> = 16.0..=32.0;

    /// Convert the preview line height in points to a multiple of the body
    /// font size for the CommonMark renderer.
    pub fn preview_line_factor(&self) -> f32 {
        (self.preview_line_spacing / self.preview_font_size.max(1.0)).clamp(0.8, 3.0)
    }

    /// Upgrade older config files that stored preview line spacing as a
    /// multiplier (for example `1.45`) instead of points.
    pub fn normalize(&mut self) {
        if self.preview_line_spacing < 4.0 {
            self.preview_line_spacing *= self.preview_font_size.max(1.0);
        }
        self.preview_line_spacing = self.preview_line_spacing.clamp(
            *Self::PREVIEW_LINE_RANGE.start(),
            *Self::PREVIEW_LINE_RANGE.end(),
        );
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    #[serde(default)]
    pub provider: String,
    /// Legacy plaintext key, read once and migrated into the OS keychain.
    /// Never written back to disk.
    #[serde(default, skip_serializing)]
    pub api_key: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub dictation_model: String,
    #[serde(default)]
    pub dictation_source: String,
    /// User-configured OpenAI-compatible server for the local provider.
    #[serde(default)]
    pub custom_endpoint: String,
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
    let mut cfg = std::fs::read_to_string(config_path())
        .ok()
        .map(|text| parse(&text))
        .unwrap_or_default();
    cfg.ui.normalize();
    cfg
}

/// Parse settings, tolerating a UTF-8 BOM added by some editors.
fn parse(text: &str) -> AppConfig {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    serde_json::from_str(text).unwrap_or_default()
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
            provider: "gemini".to_string(),
            api_key: String::new(),
            model: "gemini-3.8-flash".to_string(),
            dictation_model: "base".to_string(),
            dictation_source: "local".to_string(),
            custom_endpoint: String::new(),
            ui: UiSettings {
                editor_font: EditorFont::Monospace,
                editor_font_size: 17.0,
                editor_line_height: 24.0,
                preview_font: EditorFont::Monospace,
                preview_font_size: 18.0,
                preview_line_spacing: 24.0,
                use_provider_color: true,
                accent: [90, 160, 140],
            },
        };
        let json = serde_json::to_string(&cfg).unwrap();
        assert_eq!(serde_json::from_str::<AppConfig>(&json).unwrap(), cfg);
    }

    #[test]
    fn legacy_keys_are_read_but_never_written() {
        let cfg: AppConfig =
            serde_json::from_str(r#"{"api_key":"sk-test","model":"gpt-4o"}"#).unwrap();
        assert_eq!(cfg.api_key, "sk-test");
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(!json.contains("api_key"), "{json}");
        assert!(!json.contains("sk-test"), "{json}");
    }

    #[test]
    fn old_config_without_ui_uses_defaults() {
        let cfg: AppConfig =
            serde_json::from_str(r#"{"api_key":"AIza-test","model":"gemini-3.8-flash"}"#).unwrap();
        assert_eq!(cfg.ui, UiSettings::default());
        assert_eq!(cfg.api_key, "AIza-test");
    }

    #[test]
    fn migrates_old_preview_line_multiplier() {
        let mut ui: UiSettings = serde_json::from_str(r#"{"preview_line_spacing":1.5}"#).unwrap();
        ui.normalize();
        assert!((ui.preview_line_spacing - 23.25).abs() < 0.01);
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

    #[test]
    fn parses_config_with_utf8_bom() {
        let cfg = parse("\u{feff}{\"provider\":\"local\",\"model\":\"llama3.1\"}");
        assert_eq!(cfg.provider, "local");
        assert_eq!(cfg.model, "llama3.1");
    }
}
