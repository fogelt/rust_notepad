//! Provider metadata: names, key handling, and curated model lists.
//!
//! The provider is stored as a stable id in the config file, and an API key
//! prefix gives a best-effort hint when a key is pasted.

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Provider {
    #[default]
    Gemini,
    OpenAi,
    Anthropic,
    OpenRouter,
    DeepSeek,
}

impl Provider {
    pub const ALL: [Provider; 5] = [
        Provider::Gemini,
        Provider::OpenAi,
        Provider::Anthropic,
        Provider::OpenRouter,
        Provider::DeepSeek,
    ];

    /// Stable value stored in the config file.
    pub fn id(self) -> &'static str {
        match self {
            Provider::Gemini => "gemini",
            Provider::OpenAi => "openai",
            Provider::Anthropic => "anthropic",
            Provider::OpenRouter => "openrouter",
            Provider::DeepSeek => "deepseek",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id.trim().to_ascii_lowercase().as_str() {
            "gemini" => Some(Provider::Gemini),
            "openai" | "open-ai" => Some(Provider::OpenAi),
            "anthropic" | "claude" => Some(Provider::Anthropic),
            "openrouter" | "open-router" => Some(Provider::OpenRouter),
            "deepseek" | "deep-seek" => Some(Provider::DeepSeek),
            _ => None,
        }
    }

    /// Name shown in the interface.
    pub fn label(self) -> &'static str {
        match self {
            Provider::Gemini => "Google Gemini",
            Provider::OpenAi => "OpenAI",
            Provider::Anthropic => "Anthropic",
            Provider::OpenRouter => "OpenRouter",
            Provider::DeepSeek => "DeepSeek",
        }
    }

    /// Best-effort guess from the API key prefix. Returns `None` when the key
    /// doesn't look like any provider we know about.
    ///
    /// Note that `sk-` is shared by OpenAI and DeepSeek, so it maps to OpenAI
    /// here. The settings screen keeps an existing DeepSeek choice instead of
    /// switching it back.
    pub fn detect(key: &str) -> Option<Self> {
        let key = key.trim();
        // Google keys use the classic AIza prefix or the newer AQ. prefix.
        if key.starts_with("AIza") || key.starts_with("AQ.") {
            return Some(Provider::Gemini);
        }
        if key.starts_with("sk-ant-") {
            return Some(Provider::Anthropic);
        }
        if key.starts_with("sk-or-") {
            return Some(Provider::OpenRouter);
        }
        if key.starts_with("sk-") {
            return Some(Provider::OpenAi);
        }
        None
    }

    /// Environment variable used as a fallback when the field is empty.
    pub fn env_var(self) -> &'static str {
        match self {
            Provider::Gemini => "GEMINI_API_KEY",
            Provider::OpenAi => "OPENAI_API_KEY",
            Provider::Anthropic => "ANTHROPIC_API_KEY",
            Provider::OpenRouter => "OPENROUTER_API_KEY",
            Provider::DeepSeek => "DEEPSEEK_API_KEY",
        }
    }

    /// Environment variable that overrides the default model.
    pub fn env_model_var(self) -> &'static str {
        match self {
            Provider::Gemini => "GEMINI_MODEL",
            Provider::OpenAi => "OPENAI_MODEL",
            Provider::Anthropic => "ANTHROPIC_MODEL",
            Provider::OpenRouter => "OPENROUTER_MODEL",
            Provider::DeepSeek => "DEEPSEEK_MODEL",
        }
    }

    /// Placeholder for the key field.
    pub fn key_hint(self) -> &'static str {
        match self {
            Provider::Gemini => "Paste your Gemini API key…",
            Provider::OpenAi => "Paste your OpenAI API key…",
            Provider::Anthropic => "Paste your Anthropic API key…",
            Provider::OpenRouter => "Paste your OpenRouter API key…",
            Provider::DeepSeek => "Paste your DeepSeek API key…",
        }
    }

    /// Page where a new API key can be created.
    pub fn key_url(self) -> &'static str {
        match self {
            Provider::Gemini => "https://aistudio.google.com/api-keys",
            Provider::OpenAi => "https://platform.openai.com/api-keys",
            Provider::Anthropic => "https://console.anthropic.com/settings/keys",
            Provider::OpenRouter => "https://openrouter.ai/keys",
            Provider::DeepSeek => "https://platform.deepseek.com/api_keys",
        }
    }

    pub fn models(self) -> &'static [ModelChoice] {
        match self {
            Provider::Gemini => GEMINI_MODELS,
            Provider::OpenAi => OPENAI_MODELS,
            Provider::Anthropic => ANTHROPIC_MODELS,
            Provider::DeepSeek => DEEPSEEK_MODELS,
            // OpenRouter has a huge, fast-moving catalog, so the model is
            // entered by hand instead of guessed at.
            Provider::OpenRouter => &[],
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
        assert_eq!(Provider::detect("sk-or-v1-x"), Some(Provider::OpenRouter));
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
}
