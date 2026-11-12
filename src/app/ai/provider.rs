//! Provider metadata: names, key handling, and curated model lists.
//!
//! All per-provider metadata lives in [`SPECS`] so adding a provider is one
//! table entry (plus its transport in `ai::text`). The provider is stored as a
//! stable id in the config file, and an API key prefix gives a best-effort hint
//! when a key is pasted.

/// A curated model entry shown in the Settings → Provider dropdown.
pub struct ModelChoice {
    pub id: &'static str,
    pub label: &'static str,
    pub note: &'static str,
}

const GEMINI_MODELS: &[ModelChoice] = &[
    ModelChoice {
        id: "gemini-3.8-flash",
        label: "Gemini 3.8 Flash",
        note: "Recommended",
    },
    ModelChoice {
        id: "gemini-3.5-flash-lite",
        label: "Gemini 3.5 Flash Lite",
        note: "Fastest",
    },
    ModelChoice {
        id: "gemini-4-argon",
        label: "Gemini 4 Argon",
        note: "Most capable",
    },
];

const OPENAI_MODELS: &[ModelChoice] = &[
    ModelChoice {
        id: "gpt-6.1-sol",
        label: "GPT-6.1 Sol",
        note: "Recommended",
    },
    ModelChoice {
        id: "gpt-6-luna",
        label: "GPT-6 Luna",
        note: "Fastest",
    },
    ModelChoice {
        id: "gpt-6.1-terra",
        label: "GPT-6.1 Terra",
        note: "Balanced",
    },
    ModelChoice {
        id: "gpt-6-astra",
        label: "GPT-6 Astra",
        note: "Most capable",
    },
];

const ANTHROPIC_MODELS: &[ModelChoice] = &[
    ModelChoice {
        id: "claude-sonnet-5-5",
        label: "Claude Sonnet 5.5",
        note: "Recommended",
    },
    ModelChoice {
        id: "claude-haiku-5-5",
        label: "Claude Haiku 5.5",
        note: "Fastest",
    },
    ModelChoice {
        id: "claude-opus-5-5",
        label: "Claude Opus 5.5",
        note: "Most capable",
    },
];

const DEEPSEEK_MODELS: &[ModelChoice] = &[
    ModelChoice {
        id: "deepseek-chat",
        label: "DeepSeek Chat",
        note: "Recommended",
    },
    ModelChoice {
        id: "deepseek-reasoner",
        label: "DeepSeek Reasoner",
        note: "Reasoning",
    },
];

/// Everything the app knows about a provider except how to talk to it.
///
/// Transport lives in `ai::text`; keeping metadata here makes provider work
/// data-driven and leaves room for a custom OpenAI-compatible endpoint
/// with its own spec.
pub struct ProviderSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub env_key: &'static str,
    pub env_model: &'static str,
    pub key_hint: &'static str,
    pub key_url: &'static str,
    pub models: &'static [ModelChoice],
    /// False for providers that run on the user's machine and need no key.
    pub requires_api_key: bool,
}

const SPECS: [ProviderSpec; 5] = [
    ProviderSpec {
        id: "gemini",
        label: "Google Gemini",
        env_key: "GEMINI_API_KEY",
        env_model: "GEMINI_MODEL",
        key_hint: "Paste your Gemini API key…",
        key_url: "https://aistudio.google.com/api-keys",
        models: GEMINI_MODELS,
        requires_api_key: true,
    },
    ProviderSpec {
        id: "openai",
        label: "OpenAI",
        env_key: "OPENAI_API_KEY",
        env_model: "OPENAI_MODEL",
        key_hint: "Paste your OpenAI API key…",
        key_url: "https://platform.openai.com/api-keys",
        models: OPENAI_MODELS,
        requires_api_key: true,
    },
    ProviderSpec {
        id: "anthropic",
        label: "Anthropic",
        env_key: "ANTHROPIC_API_KEY",
        env_model: "ANTHROPIC_MODEL",
        key_hint: "Paste your Anthropic API key…",
        key_url: "https://console.anthropic.com/settings/keys",
        models: ANTHROPIC_MODELS,
        requires_api_key: true,
    },
    ProviderSpec {
        id: "deepseek",
        label: "DeepSeek",
        env_key: "DEEPSEEK_API_KEY",
        env_model: "DEEPSEEK_MODEL",
        key_hint: "Paste your DeepSeek API key…",
        key_url: "https://platform.deepseek.com/api_keys",
        models: DEEPSEEK_MODELS,
        requires_api_key: true,
    },
    ProviderSpec {
        id: "custom",
        label: "Custom endpoint",
        env_key: "CUSTOM_LLM_API_KEY",
        env_model: "CUSTOM_LLM_MODEL",
        key_hint: "Optional API key…",
        // No provider key page: the endpoint can be anything OpenAI-compatible.
        key_url: "",
        models: &[],
        requires_api_key: false,
    },
];

/// API key prefixes, ordered so more specific prefixes win (`sk-ant-` before
/// `sk-`).
const KEY_PREFIXES: &[(&str, Provider)] = &[
    ("AIza", Provider::Gemini),
    ("AQ.", Provider::Gemini),
    ("sk-ant-", Provider::Anthropic),
    ("sk-", Provider::OpenAi),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum Provider {
    #[default]
    Gemini = 0,
    OpenAi = 1,
    Anthropic = 2,
    DeepSeek = 3,
    Custom = 4,
}

impl Provider {
    pub const ALL: [Provider; 5] = [
        Provider::Gemini,
        Provider::OpenAi,
        Provider::Anthropic,
        Provider::DeepSeek,
        Provider::Custom,
    ];

    pub fn spec(self) -> &'static ProviderSpec {
        &SPECS[self as usize]
    }

    /// Stable value stored in the config file.
    pub fn id(self) -> &'static str {
        self.spec().id
    }

    pub fn from_id(id: &str) -> Option<Self> {
        let id = id.trim().to_ascii_lowercase();
        Provider::ALL
            .into_iter()
            .find(|provider| provider.id() == id)
    }

    /// Name shown in the interface.
    pub fn label(self) -> &'static str {
        self.spec().label
    }

    /// Best-effort guess from the API key prefix. Returns `None` when the key
    /// doesn't look like any provider we know about.
    ///
    /// Note that `sk-` is shared by OpenAI and DeepSeek, so it maps to OpenAI
    /// here. The settings screen keeps an existing DeepSeek choice instead of
    /// switching it back.
    pub fn detect(key: &str) -> Option<Self> {
        let key = key.trim();
        KEY_PREFIXES
            .iter()
            .find(|(prefix, _)| key.starts_with(prefix))
            .map(|(_, provider)| *provider)
    }

    /// Whether this provider needs an API key at all.
    pub fn requires_api_key(self) -> bool {
        self.spec().requires_api_key
    }

    /// Whether this provider accepts an optional key. Cloud providers require
    /// one; a custom endpoint may or may not need it.
    pub fn allows_api_key(self) -> bool {
        self.requires_api_key() || self == Provider::Custom
    }

    /// Whether the provider uses a user-configured server URL.
    pub fn uses_custom_endpoint(self) -> bool {
        self == Provider::Custom
    }

    /// Environment variable used as a fallback when the field is empty.
    pub fn env_var(self) -> &'static str {
        self.spec().env_key
    }

    /// Environment variable that overrides the default model.
    pub fn env_model_var(self) -> &'static str {
        self.spec().env_model
    }

    /// Placeholder for the key field.
    pub fn key_hint(self) -> &'static str {
        self.spec().key_hint
    }

    /// Page where a new API key can be created.
    pub fn key_url(self) -> &'static str {
        self.spec().key_url
    }

    pub fn models(self) -> &'static [ModelChoice] {
        self.spec().models
    }

    /// Branding color used when the accent follows the active provider.
    /// Chosen to stay legible on the dark chrome.
    pub fn accent_rgb(self) -> [u8; 3] {
        match self {
            Provider::Gemini => [138, 180, 248],
            Provider::OpenAi => [52, 197, 155],
            Provider::Anthropic => [221, 170, 136],
            Provider::DeepSeek => [123, 143, 255],
            // Punk yellow for custom OpenAI-compatible endpoints.
            Provider::Custom => [255, 230, 0],
        }
    }

    /// Output token budget for a request. Visual reports need more room;
    /// Gemini and custom endpoints get the smaller caps.
    pub fn max_output_tokens(self, report: bool) -> u32 {
        match (self, report) {
            (Provider::Gemini | Provider::Custom, false) => 2048,
            (Provider::Gemini | Provider::Custom, true) => 8192,
            (_, false) => 4096,
            (_, true) => 16384,
        }
    }

    pub fn default_model(self) -> &'static str {
        self.models().first().map(|model| model.id).unwrap_or("")
    }

    /// Human-readable label for a model id, with a fallback for models that
    /// aren't in the curated list.
    pub fn model_label(self, id: &str) -> String {
        self.models()
            .iter()
            .find(|model| model.id == id)
            .map(|model| format!("{} · {}", model.label, model.note))
            .unwrap_or_else(|| id.to_string())
    }

    /// True when the model is one of the curated entries for this provider.
    pub fn has_model(self, id: &str) -> bool {
        self.models().iter().any(|model| model.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_provider_from_key_prefixes() {
        assert_eq!(Provider::detect("AIzaSyabc"), Some(Provider::Gemini));
        assert_eq!(Provider::detect("AQ.Ab8abc"), Some(Provider::Gemini));
        assert_eq!(
            Provider::detect("sk-ant-api03-x"),
            Some(Provider::Anthropic)
        );
        assert_eq!(Provider::detect("sk-proj-x"), Some(Provider::OpenAi));
        assert_eq!(Provider::detect("nonsense"), None);
    }

    #[test]
    fn provider_ids_roundtrip() {
        for provider in Provider::ALL {
            assert_eq!(Provider::from_id(provider.id()), Some(provider));
            assert!(!provider.label().is_empty());
        }
    }

    #[test]
    fn every_provider_has_key_metadata() {
        for provider in Provider::ALL {
            assert!(!provider.env_model_var().is_empty());
            if provider.requires_api_key() {
                assert!(!provider.env_var().is_empty());
                assert!(!provider.key_hint().is_empty());
                assert!(provider.key_url().starts_with("https://"));
            } else if provider.allows_api_key() {
                // Optional key: an environment fallback exists, but there is
                // no provider key page to link to.
                assert!(!provider.env_var().is_empty());
                assert!(!provider.key_hint().is_empty());
                assert!(provider.key_url().is_empty());
            }
        }
    }

    #[test]
    fn custom_provider_accepts_an_optional_key() {
        let custom = Provider::Custom;
        assert!(!custom.requires_api_key());
        assert!(custom.allows_api_key());
        assert!(custom.uses_custom_endpoint());
        assert!(custom.models().is_empty());
        assert_eq!(Provider::from_id("custom"), Some(custom));
        assert_eq!(custom.env_model_var(), "CUSTOM_LLM_MODEL");
        for provider in Provider::ALL {
            if provider != Provider::Custom {
                assert!(provider.requires_api_key());
                assert!(provider.allows_api_key());
                assert!(!provider.uses_custom_endpoint());
            }
        }
    }

    #[test]
    fn curated_models_are_unique_and_usable() {
        for provider in Provider::ALL {
            let models = provider.models();
            if models.is_empty() {
                continue;
            }
            assert!(models.iter().any(|m| m.id == provider.default_model()));
            let mut ids: Vec<_> = models.iter().map(|m| m.id).collect();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), models.len(), "duplicate model for {provider:?}");
            for model in models {
                assert!(!model.label.is_empty());
                assert!(!model.note.is_empty());
            }
        }
    }

    #[test]
    fn model_labels_are_friendly() {
        assert!(
            Provider::Gemini
                .model_label("gemini-3.8-flash")
                .contains("Recommended")
        );
        assert_eq!(Provider::Gemini.model_label("custom-model"), "custom-model");
        assert!(Provider::OpenAi.has_model(Provider::OpenAi.default_model()));
    }

    #[test]
    fn providers_have_distinct_brand_colors() {
        let mut seen = Vec::new();
        for provider in Provider::ALL {
            let rgb = provider.accent_rgb();
            assert!(rgb.iter().any(|channel| *channel > 0));
            assert!(!seen.contains(&rgb), "duplicate color for {provider:?}");
            seen.push(rgb);
        }
    }
}
