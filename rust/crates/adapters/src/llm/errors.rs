use meeting_core::CoreError;

/// Map a non-2xx HTTP status from an LLM provider to a classified [`CoreError`].
/// The classification drives the pipeline error banners (Phase 4).
pub fn classify_http(provider: &str, status: u16, body: &str) -> CoreError {
    let msg = format!("{provider} API error {status}: {body}");
    match status {
        401 | 403 => CoreError::ApiAuth(msg),
        429 => CoreError::ApiQuota(msg),
        408 | 500..=599 => CoreError::Network(msg),
        _ => CoreError::Llm(msg),
    }
}

/// Map a transport-level `reqwest` failure to a classified [`CoreError`].
pub fn classify_transport(provider: &str, err: &reqwest::Error) -> CoreError {
    if err.is_timeout() {
        CoreError::ApiTimeout(format!("{provider}: {err}"))
    } else if err.is_connect() {
        CoreError::Network(format!("{provider}: {err}"))
    } else {
        CoreError::Llm(format!("{provider}: {err}"))
    }
}
