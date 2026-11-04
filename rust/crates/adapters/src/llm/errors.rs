use meeting_core::CoreError;

/// Map a non-2xx HTTP status from an LLM provider to a classified [`CoreError`].
/// The classification drives the pipeline error banners (Phase 4), so it must
/// tell apart "the key is wrong" (401) from "the key is fine but this model is
/// not yours" (403/404) — conflating them sends the user to re-enter a working
/// key. The provider's own message is kept (not the raw JSON body) so the UI
/// can show it verbatim as the error detail.
pub fn classify_http(provider: &str, status: u16, body: &str) -> CoreError {
    let msg = format!("{provider} API error {status}: {}", provider_message(body));
    match status {
        401 => CoreError::ApiAuth(msg),
        // Gemini reports a bad key as 400 INVALID_ARGUMENT, not 401.
        400 if is_invalid_key(body) => CoreError::ApiAuth(msg),
        403 | 404 => CoreError::LlmModelUnavailable(msg),
        // Mistral reports an unknown model id as 400 `invalid_model`.
        400 | 422 if is_invalid_model(body) => CoreError::LlmModelUnavailable(msg),
        // Anthropic reports an exhausted balance as 400, not 429.
        400 if is_out_of_credit(body) => CoreError::ApiQuota(msg),
        429 => CoreError::ApiQuota(msg),
        408 | 500..=599 => CoreError::Network(msg),
        400..=499 => CoreError::LlmRejected(msg),
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

/// Longest raw (non-JSON) body kept in an error message.
const MAX_RAW_BODY_CHARS: usize = 300;

/// The human-readable message out of a provider error body. Shapes seen:
/// `{"error":{"message":..}}` (OpenAI, Anthropic, Gemini), `{"message":..}`
/// (Mistral), `{"error":".."}` (Ollama), `{"detail":".."}` or
/// `{"detail":[{"msg":..}]}` (Mistral validation). Anything else falls back to
/// the trimmed, length-capped body.
fn provider_message(body: &str) -> String {
    let parsed = serde_json::from_str::<serde_json::Value>(body).ok();
    let from_json = parsed.as_ref().and_then(|v| {
        let text = |v: Option<&serde_json::Value>| {
            v.and_then(|m| m.as_str())
                .map(str::trim)
                .filter(|m| !m.is_empty())
                .map(str::to_owned)
        };
        text(v.pointer("/error/message"))
            .or_else(|| text(v.get("message")))
            .or_else(|| text(v.get("error")))
            .or_else(|| text(v.get("detail")))
            .or_else(|| {
                let msgs: Vec<&str> = v
                    .get("detail")?
                    .as_array()?
                    .iter()
                    .filter_map(|d| d.get("msg")?.as_str())
                    .collect();
                (!msgs.is_empty()).then(|| msgs.join("; "))
            })
    });
    if let Some(m) = from_json {
        return m;
    }

    let raw = body.trim();
    if raw.is_empty() {
        return "(empty response)".to_string();
    }
    if raw.chars().count() <= MAX_RAW_BODY_CHARS {
        return raw.to_string();
    }
    let cut: String = raw.chars().take(MAX_RAW_BODY_CHARS).collect();
    format!("{cut}…")
}

fn is_invalid_key(body: &str) -> bool {
    body.contains("API_KEY_INVALID") || body.contains("API key not valid")
}

fn is_invalid_model(body: &str) -> bool {
    let b = body.to_lowercase();
    b.contains("invalid_model") || b.contains("model_not_found") || b.contains("invalid model")
}

fn is_out_of_credit(body: &str) -> bool {
    body.to_lowercase().contains("credit balance")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact body Mistral returned for `mistral-large-latest` on a tier
    /// without access to it — the case that used to surface as "API-ключ не
    /// задан или неверен" although the key was valid.
    const MISTRAL_TIER_403: &str = r#"{"object":"error","message":"This model is not available in your subscription tier","type":"tier_not_allowed","param":null,"code":"1910","raw_status_code":403}"#;

    #[test]
    fn forbidden_model_is_not_reported_as_a_bad_key() {
        let err = classify_http("mistral", 403, MISTRAL_TIER_403);
        assert!(
            matches!(err, CoreError::LlmModelUnavailable(_)),
            "got {err:?}"
        );
        assert_eq!(
            err.to_string(),
            "model unavailable: mistral API error 403: \
             This model is not available in your subscription tier"
        );
    }

    #[test]
    fn unauthorized_is_a_bad_key() {
        let body =
            r#"{"error":{"message":"Incorrect API key provided","type":"invalid_request_error"}}"#;
        let err = classify_http("openai", 401, body);
        assert!(matches!(err, CoreError::ApiAuth(_)), "got {err:?}");
        assert!(err.to_string().ends_with(": Incorrect API key provided"));
    }

    #[test]
    fn gemini_invalid_key_400_is_a_bad_key() {
        let body = r#"{"error":{"code":400,"message":"API key not valid. Please pass a valid API key.","status":"INVALID_ARGUMENT","details":[{"reason":"API_KEY_INVALID"}]}}"#;
        let err = classify_http("Gemini", 400, body);
        assert!(matches!(err, CoreError::ApiAuth(_)), "got {err:?}");
    }

    #[test]
    fn unknown_model_is_model_unavailable() {
        let mistral = r#"{"object":"error","message":"Invalid model: mistral-hugest","type":"invalid_model","param":null,"code":"1500"}"#;
        assert!(matches!(
            classify_http("mistral", 400, mistral),
            CoreError::LlmModelUnavailable(_)
        ));
        let openai = r#"{"error":{"message":"The model `gpt-9` does not exist or you do not have access to it.","type":"invalid_request_error","code":"model_not_found"}}"#;
        assert!(matches!(
            classify_http("openai", 404, openai),
            CoreError::LlmModelUnavailable(_)
        ));
        let ollama = r#"{"error":"model 'llama9' not found"}"#;
        let err = classify_http("ollama", 404, ollama);
        assert!(matches!(err, CoreError::LlmModelUnavailable(_)));
        assert!(err.to_string().ends_with(": model 'llama9' not found"));
    }

    #[test]
    fn exhausted_anthropic_balance_is_quota() {
        let body = r#"{"type":"error","error":{"type":"invalid_request_error","message":"Your credit balance is too low to access the Anthropic API."}}"#;
        assert!(matches!(
            classify_http("Anthropic", 400, body),
            CoreError::ApiQuota(_)
        ));
    }

    #[test]
    fn other_client_errors_are_rejected_not_retried_as_unknown() {
        let body = r#"{"detail":[{"msg":"Input should be less than or equal to 32768","loc":["body","max_tokens"]}]}"#;
        let err = classify_http("mistral", 422, body);
        assert!(matches!(err, CoreError::LlmRejected(_)), "got {err:?}");
        assert!(err
            .to_string()
            .ends_with(": Input should be less than or equal to 32768"));
    }

    #[test]
    fn transient_statuses_keep_their_classes() {
        assert!(matches!(
            classify_http("x", 429, ""),
            CoreError::ApiQuota(_)
        ));
        assert!(matches!(classify_http("x", 503, ""), CoreError::Network(_)));
        assert!(matches!(classify_http("x", 408, ""), CoreError::Network(_)));
    }

    #[test]
    fn non_json_body_is_trimmed_and_capped() {
        assert_eq!(provider_message("  \n"), "(empty response)");
        assert_eq!(
            provider_message(" 404 page not found\n"),
            "404 page not found"
        );
        let long = "x".repeat(MAX_RAW_BODY_CHARS + 50);
        let m = provider_message(&long);
        assert_eq!(m.chars().count(), MAX_RAW_BODY_CHARS + 1);
        assert!(m.ends_with('…'));
    }
}
