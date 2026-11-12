//! Provider transport: request bodies, HTTP, response parsing, and error text.
//!
//! Each provider is a [`TextBackend`] implementation. Adding a provider means
//! adding one backend type and one entry in [`backend_for`]; the request
//! plumbing, response parsing, and error mapping stay shared.

use super::http::check_response;
use super::provider::Provider;

/// Everything a backend needs for one generation request.
struct GenerationRequest<'a> {
    prompt: &'a str,
    api_key: &'a str,
    model: &'a str,
    max_output_tokens: u32,
    temperature: f32,
}

/// A provider's chat transport.
trait TextBackend {
    fn generate(&self, request: &GenerationRequest<'_>) -> Result<String, String>;
}

struct GeminiBackend;

impl TextBackend for GeminiBackend {
    fn generate(&self, request: &GenerationRequest<'_>) -> Result<String, String> {
        let body = serde_json::json!({
            "contents": [{ "role": "user", "parts": [{ "text": request.prompt }] }],
            "generationConfig": {
                "temperature": request.temperature,
                "maxOutputTokens": request.max_output_tokens
            }
        });
        let json = post_gemini(&body, request.api_key, request.model)?;
        response_text(&json).ok_or_else(|| "Gemini returned an empty response.".to_string())
    }
}

/// OpenAI's Responses API. Reasoning models reject a custom temperature, so it
/// is left at the model default here.
struct OpenAiBackend;

impl TextBackend for OpenAiBackend {
    fn generate(&self, request: &GenerationRequest<'_>) -> Result<String, String> {
        let body = serde_json::json!({
            "model": request.model.trim(),
            "input": request.prompt,
            "max_output_tokens": request.max_output_tokens,
            // Don't keep the document on OpenAI's servers after the reply.
            "store": false,
        });
        let resp = http_client()?
            .post("https://api.openai.com/v1/responses")
            .bearer_auth(request.api_key)
            .json(&body)
            .send()
            .map_err(|e| format!("Request failed: {e}"))?;
        let json = check_response(resp, "OpenAI")?;
        openai_text(&json).ok_or_else(|| "OpenAI returned an empty response.".to_string())
    }
}

/// Anthropic's Messages API. Temperature is omitted because the current models
/// use adaptive thinking, which rejects custom values.
struct AnthropicBackend;

impl TextBackend for AnthropicBackend {
    fn generate(&self, request: &GenerationRequest<'_>) -> Result<String, String> {
        let body = serde_json::json!({
            "model": request.model.trim(),
            "max_tokens": request.max_output_tokens,
            "messages": [{ "role": "user", "content": request.prompt }],
        });
        let resp = http_client()?
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", request.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .map_err(|e| format!("Request failed: {e}"))?;
        let json = check_response(resp, "Anthropic")?;
        anthropic_text(&json).ok_or_else(|| "Anthropic returned an empty response.".to_string())
    }
}

/// OpenRouter exposes an OpenAI-compatible chat completions endpoint.
struct OpenRouterBackend;

impl TextBackend for OpenRouterBackend {
    fn generate(&self, request: &GenerationRequest<'_>) -> Result<String, String> {
        let body = serde_json::json!({
            "model": request.model.trim(),
            "messages": [{ "role": "user", "content": request.prompt }],
            "max_tokens": request.max_output_tokens,
            "temperature": request.temperature,
        });
        let resp = http_client()?
            .post("https://openrouter.ai/api/v1/chat/completions")
            .bearer_auth(request.api_key)
            .json(&body)
            .send()
            .map_err(|e| format!("Request failed: {e}"))?;
        let json = check_response(resp, "OpenRouter")?;
        chat_completion_text(&json)
            .ok_or_else(|| "OpenRouter returned an empty response.".to_string())
    }
}

/// DeepSeek is OpenAI-compatible. The reasoner model ignores temperature, so it
/// is only sent for the chat model.
struct DeepSeekBackend;

impl TextBackend for DeepSeekBackend {
    fn generate(&self, request: &GenerationRequest<'_>) -> Result<String, String> {
        let mut body = serde_json::json!({
            "model": request.model.trim(),
            "messages": [{ "role": "user", "content": request.prompt }],
            "max_tokens": request.max_output_tokens,
        });
        if !request.model.contains("reasoner") {
            body["temperature"] = serde_json::json!(request.temperature);
        }
        let resp = http_client()?
            .post("https://api.deepseek.com/chat/completions")
            .bearer_auth(request.api_key)
            .json(&body)
            .send()
            .map_err(|e| format!("Request failed: {e}"))?;
        let json = check_response(resp, "DeepSeek")?;
        chat_completion_text(&json)
            .ok_or_else(|| "DeepSeek returned an empty response.".to_string())
    }
}

const GEMINI: &dyn TextBackend = &GeminiBackend;
const OPENAI: &dyn TextBackend = &OpenAiBackend;
const ANTHROPIC: &dyn TextBackend = &AnthropicBackend;
const OPENROUTER: &dyn TextBackend = &OpenRouterBackend;
const DEEPSEEK: &dyn TextBackend = &DeepSeekBackend;

fn backend_for(provider: Provider) -> &'static dyn TextBackend {
    match provider {
        Provider::Gemini => GEMINI,
        Provider::OpenAi => OPENAI,
        Provider::Anthropic => ANTHROPIC,
        Provider::OpenRouter => OPENROUTER,
        Provider::DeepSeek => DEEPSEEK,
    }
}

/// Generate text with the selected provider. `max_output_tokens` is a
/// best-effort cap: OpenAI's Responses API and Anthropic's Messages API both
/// accept it directly.
pub(crate) fn generate_text(
    provider: Provider,
    prompt: &str,
    api_key: &str,
    model: &str,
    max_output_tokens: u32,
    temperature: f32,
) -> Result<String, String> {
    let request = GenerationRequest {
        prompt,
        api_key,
        model,
        max_output_tokens,
        temperature,
    };
    let text = backend_for(provider).generate(&request)?;
    Ok(strip_code_fence(&text))
}

/// Strip a wrapping ```markdown fence if the model added one anyway.
fn strip_code_fence(text: &str) -> String {
    let trimmed = text.trim();
    if let Some(inner) = trimmed
        .strip_prefix("```")
        .and_then(|s| s.strip_suffix("```"))
    {
        // Drop an optional language tag on the first line (`markdown`, `md`).
        let mut lines = inner.lines();
        let first = lines.next().unwrap_or("");
        let first_trim = first.trim();
        if first_trim.eq_ignore_ascii_case("markdown") || first_trim.eq_ignore_ascii_case("md") {
            return lines.collect::<Vec<_>>().join("\n").trim().to_string();
        }
        return inner.trim().to_string();
    }
    trimmed.to_string()
}

fn http_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Request failed: {e}"))
}

fn post_gemini(
    body: &serde_json::Value,
    api_key: &str,
    model: &str,
) -> Result<serde_json::Value, String> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        model.trim()
    );
    let resp = http_client()?
        .post(&url)
        .query(&[("key", api_key)])
        .json(body)
        .send()
        .map_err(|e| format!("Request failed: {e}"))?;
    check_response(resp, "Gemini")
}

/// Text from an OpenAI Responses payload: either the convenience
/// `output_text` field or the message items inside `output`.
fn openai_text(json: &serde_json::Value) -> Option<String> {
    if let Some(text) = json["output_text"].as_str()
        && !text.trim().is_empty()
    {
        return Some(text.to_string());
    }
    let mut out = String::new();
    for item in json["output"].as_array()? {
        if item["type"].as_str() != Some("message") {
            continue;
        }
        for part in item["content"].as_array()? {
            if part["type"].as_str() == Some("output_text")
                && let Some(text) = part["text"].as_str()
            {
                out.push_str(text);
            }
        }
    }
    (!out.trim().is_empty()).then_some(out)
}

/// Text from an OpenAI-compatible chat completions payload.
fn chat_completion_text(json: &serde_json::Value) -> Option<String> {
    let text = json["choices"][0]["message"]["content"].as_str()?;
    (!text.trim().is_empty()).then(|| text.to_string())
}

/// Text from an Anthropic Messages payload.
fn anthropic_text(json: &serde_json::Value) -> Option<String> {
    let mut out = String::new();
    for part in json["content"].as_array()? {
        if part["type"].as_str() == Some("text")
            && let Some(text) = part["text"].as_str()
        {
            out.push_str(text);
        }
    }
    (!out.trim().is_empty()).then_some(out)
}

/// Turn raw provider/network errors into something a person can act on.
pub(crate) fn friendly_generation_error(raw: &str, provider: Provider) -> String {
    let name = provider.label();
    let lower = raw.to_ascii_lowercase();
    if lower.contains("api key")
        || lower.contains("api_key")
        || lower.contains("unauthenticated")
        || lower.contains("permission")
        || lower.contains("invalid_api_key")
        || lower.contains("401")
        || lower.contains("403")
    {
        return format!(
            "{name} rejected the API key. Check it in Settings > Provider and try again."
        );
    }
    if lower.contains("429")
        || lower.contains("quota")
        || lower.contains("rate limit")
        || lower.contains("resource_exhausted")
    {
        return format!("{name} is rate limiting requests right now. Wait a moment and try again.");
    }
    if lower.contains("404") || lower.contains("not found") || lower.contains("not supported") {
        return format!(
            "That model isn't available with {name}. Pick another model in Settings > Provider."
        );
    }
    if lower.contains("request failed")
        || lower.contains("sending request")
        || lower.contains("connection")
        || lower.contains("timed out")
        || lower.contains("dns")
        || lower.contains("network")
        || lower.contains("could not connect")
    {
        return format!("Could not reach {name}. Check your internet connection and try again.");
    }
    let short = raw.chars().take(220).collect::<String>();
    format!("Request failed: {short}")
}

/// Offline dictation errors are already written for people; just add a prefix
/// when the raw message is technical.
pub(crate) fn friendly_local_error(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("microphone") || lower.contains("dictation") || lower.contains("model") {
        return raw.to_string();
    }
    let short = raw.chars().take(220).collect::<String>();
    format!("Local dictation failed: {short}")
}

/// Join the text parts of the first candidate, skipping any "thought" parts.
fn response_text(json: &serde_json::Value) -> Option<String> {
    let parts = json["candidates"][0]["content"]["parts"].as_array()?;
    let text: String = parts
        .iter()
        .filter(|part| part.get("thought").and_then(|v| v.as_bool()) != Some(true))
        .filter_map(|part| part["text"].as_str())
        .collect();
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friendly_errors_point_at_the_fix() {
        let gemini = Provider::Gemini;
        assert!(friendly_generation_error("403 API key not valid", gemini).contains("API key"));
        assert!(
            friendly_generation_error("429 RESOURCE_EXHAUSTED", gemini).contains("rate limiting")
        );
        assert!(
            friendly_generation_error("Request failed: dns error", gemini).contains("internet")
        );
        assert!(friendly_generation_error("weird thing", gemini).contains("weird thing"));
    }

    #[test]
    fn strips_markdown_fence_with_language_tag() {
        let out = strip_code_fence("```markdown\n# Hello\n```");
        assert_eq!(out, "# Hello");
    }

    #[test]
    fn strips_plain_fence_and_leaves_text_alone() {
        assert_eq!(strip_code_fence("```\nfoo\n```"), "foo");
        assert_eq!(strip_code_fence("# Hello"), "# Hello");
    }

    #[test]
    fn parses_provider_responses() {
        let openai = serde_json::json!({
            "output": [{
                "type": "message",
                "content": [
                    { "type": "output_text", "text": "hello " },
                    { "type": "output_text", "text": "world" }
                ]
            }]
        });
        assert_eq!(openai_text(&openai).as_deref(), Some("hello world"));

        let anthropic = serde_json::json!({
            "content": [
                { "type": "thinking", "thinking": "hmm" },
                { "type": "text", "text": "answer" }
            ]
        });
        assert_eq!(anthropic_text(&anthropic).as_deref(), Some("answer"));

        let chat = serde_json::json!({
            "choices": [{ "message": { "content": "reply" } }]
        });
        assert_eq!(chat_completion_text(&chat).as_deref(), Some("reply"));
    }

    #[test]
    fn joins_text_parts_and_skips_thoughts() {
        let json = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [
                        {"thought": true, "text": "thinking..."},
                        {"text": "make it "},
                        {"text": "a table"}
                    ]
                }
            }]
        });
        assert_eq!(response_text(&json).as_deref(), Some("make it a table"));
    }
}
