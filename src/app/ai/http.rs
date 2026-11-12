//! Shared HTTP response handling for provider APIs.

/// Turn a non-success response into a readable error and parse the body as JSON.
pub(crate) fn check_response(
    resp: reqwest::blocking::Response,
    provider: &str,
) -> Result<serde_json::Value, String> {
    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp.text().unwrap_or_default();
        let short = detail.chars().take(300).collect::<String>();
        return Err(format!("{provider} error {status}: {short}"));
    }
    resp.json()
        .map_err(|e| format!("Bad response from {provider}: {e}"))
}
