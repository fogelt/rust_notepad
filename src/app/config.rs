//! Assistant settings (API key + model) persisted across launches.
//!
//! The key is stored in plaintext, like most desktop apps store tokens:
//! readable only by the local user profile, never logged or committed
//! (`.env` and the config file both stay out of git).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct AssistantConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub model: String,
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

pub fn load() -> AssistantConfig {
    std::fs::read_to_string(config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Write settings; returns the file path on success.
pub fn save(cfg: &AssistantConfig) -> Result<PathBuf, String> {
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
        let cfg = AssistantConfig {
            api_key: "AIza-test".to_string(),
            model: "gemini-2.5-flash".to_string(),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        assert_eq!(serde_json::from_str::<AssistantConfig>(&json).unwrap(), cfg);
    }

    #[test]
    fn ignores_broken_config() {
        assert_eq!(
            serde_json::from_str::<AssistantConfig>("{oops").unwrap_or_default(),
            AssistantConfig::default()
        );
    }

    #[test]
    fn pick_prefers_stored_over_env() {
        assert_eq!(pick("stored", "DEFINITELY_UNSET_VAR_XYZ"), "stored");
        assert_eq!(pick("  ", "DEFINITELY_UNSET_VAR_XYZ"), "");
    }
}
