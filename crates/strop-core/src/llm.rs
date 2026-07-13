//! OpenAI-compatible chat-completions client (C2). One client covers Poe
//! (the primary target — Kirill's subscription), OpenAI, OpenRouter,
//! ollama, and Anthropic's compat endpoint.
//!
//! Decisions from the provider research (2026-06-11):
//! - Bearer auth everywhere (Anthropic compat accepts it; ollama requires
//!   a dummy header).
//! - `max_completion_tokens`, except base URLs that look like ollama
//!   (the one holdout still documenting only `max_tokens`).
//! - NO universal `response_format`: structured output stays a prompt-and-
//!   validate contract until an endpoint-specific shim proves otherwise.
//! - OpenRouter can put errors inside an HTTP 200 body — always check.
//! - Blocking ureq; callers run this on a background thread.

use std::time::Duration;

use serde::Deserialize;
use serde_json::json;

#[derive(Debug)]
pub enum LlmError {
    /// 401/403 — fix the key/settings; never retried.
    Auth(String),
    /// 429 — retryable upstream, surfaced after one bounded retry.
    RateLimited(String),
    /// 4xx/5xx/in-body errors; message passed through verbatim (the only
    /// cross-provider constant).
    Provider(String),
    Network(String),
    /// Response arrived but no usable content.
    Shape(String),
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auth(m) => write!(f, "authentication failed: {m}"),
            Self::RateLimited(m) => write!(f, "rate limited: {m}"),
            Self::Provider(m) => write!(f, "provider error: {m}"),
            Self::Network(m) => write!(f, "network error: {m}"),
            Self::Shape(m) => write!(f, "unexpected response: {m}"),
        }
    }
}

impl std::error::Error for LlmError {}

/// Provider-neutral completion evidence. Optional fields stay optional:
/// OpenAI-compatible gateways disagree about which metadata they return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatResult {
    pub content: String,
    pub finish_reason: Option<String>,
    pub refusal: Option<String>,
    pub usage: TokenUsage,
    pub request_id: Option<String>,
    pub retries: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct TokenUsage {
    #[serde(default, alias = "input_tokens")]
    pub prompt_tokens: Option<u64>,
    #[serde(default, alias = "output_tokens")]
    pub completion_tokens: Option<u64>,
    #[serde(default)]
    pub total_tokens: Option<u64>,
}

pub struct LlmClient {
    agent: ureq::Agent,
    base_url: String,
    api_key: String,
    pub model: String,
}

impl LlmClient {
    pub fn new(base_url: &str, api_key: &str, model: &str) -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(120)))
            .http_status_as_error(false)
            .build()
            .into();
        Self {
            agent,
            base_url: base_url.trim_end_matches('/').to_owned(),
            api_key: api_key.to_owned(),
            model: model.to_owned(),
        }
    }

    fn wants_legacy_max_tokens(&self) -> bool {
        self.base_url.contains("11434") || self.base_url.contains("ollama")
    }

    pub fn request_body(&self, system: &str, user: &str, max_tokens: u32) -> serde_json::Value {
        let mut body = json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
            "temperature": 0.3,
        });
        let key = if self.wants_legacy_max_tokens() {
            "max_tokens"
        } else {
            "max_completion_tokens"
        };
        body[key] = json!(max_tokens);
        body
    }

    /// Blocking chat-completion call. Run on a background thread. A transient
    /// network failure or 429 gets one retry; auth and response errors do not.
    pub fn chat(&self, system: &str, user: &str, max_tokens: u32) -> Result<ChatResult, LlmError> {
        let body = self.request_body(system, user, max_tokens);
        match self.chat_once(&body) {
            Ok(result) => Ok(result),
            Err(LlmError::Network(_)) | Err(LlmError::RateLimited(_)) => {
                std::thread::sleep(Duration::from_millis(300));
                self.chat_once(&body).map(|mut result| {
                    result.retries = 1;
                    result
                })
            }
            Err(error) => Err(error),
        }
    }

    fn chat_once(&self, body: &serde_json::Value) -> Result<ChatResult, LlmError> {
        let mut response = self
            .agent
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .send_json(body)
            .map_err(|e| LlmError::Network(e.to_string()))?;
        let status = response.status().as_u16();
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| LlmError::Network(e.to_string()))?;
        parse_chat_response(status, &text)
    }

    /// GET {base}/models — the answer to "model not found": list what the
    /// provider actually serves (this IS the model picker; no dropdown).
    pub fn list_models(&self) -> Result<Vec<String>, LlmError> {
        let mut response = self
            .agent
            .get(format!("{}/models", self.base_url))
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .call()
            .map_err(|e| LlmError::Network(e.to_string()))?;
        // http_status_as_error(false) means a 401/429/5xx arrives here as Ok;
        // route it through the same error mapping chat() uses, so a bad key
        // surfaces as Auth — not a misleading "empty model list".
        let status = response.status().as_u16();
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| LlmError::Network(e.to_string()))?;
        parse_models_response(status, &text)
    }
}

#[derive(Deserialize)]
struct ChatResponse {
    #[serde(default)]
    error: Option<ApiError>,
    #[serde(default)]
    choices: Vec<Choice>,
    #[serde(default)]
    usage: TokenUsage,
    #[serde(default)]
    id: Option<String>,
}

#[derive(Deserialize)]
struct ApiError {
    #[serde(default)]
    message: serde_json::Value,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    #[serde(default)]
    content: serde_json::Value,
    #[serde(default)]
    refusal: Option<String>,
}

fn parse_chat_response(status: u16, text: &str) -> Result<ChatResult, LlmError> {
    let parsed: Result<ChatResponse, _> = serde_json::from_str(text);
    let error_message = parsed
        .as_ref()
        .ok()
        .and_then(|r| r.error.as_ref())
        .map(|e| match &e.message {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        });

    match status {
        401 | 403 => {
            return Err(LlmError::Auth(
                error_message.unwrap_or_else(|| format!("HTTP {status}")),
            ));
        }
        429 => {
            return Err(LlmError::RateLimited(
                error_message.unwrap_or_else(|| "HTTP 429".into()),
            ));
        }
        s if s >= 400 => {
            return Err(LlmError::Provider(
                error_message.unwrap_or_else(|| format!("HTTP {s}: {text}")),
            ));
        }
        _ => {}
    }
    // OpenRouter can deliver errors inside a 200.
    if let Some(message) = error_message {
        return Err(LlmError::Provider(message));
    }
    let response =
        parsed.map_err(|e| LlmError::Shape(format!("{e}; body: {}", body_preview(text))))?;
    let choice = response
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| LlmError::Shape("no choices in response".into()))?;
    let content = message_text(&choice.message.content);
    if content.is_empty() && choice.message.refusal.is_none() {
        return Err(LlmError::Shape("choice contained no text".into()));
    }
    Ok(ChatResult {
        content,
        finish_reason: choice.finish_reason,
        refusal: choice.message.refusal,
        usage: response.usage,
        request_id: response.id,
        retries: 0,
    })
}

/// Compatibility endpoints return either a string or an array of typed text
/// parts. Unknown/non-text parts are ignored; an entirely non-text choice is
/// reported by the caller as a shape error.
fn message_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Array(parts) => parts
            .iter()
            .filter_map(|part| part.get("text").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

/// `GET /models` response parsing, split out so the status/error matrix is
/// unit-testable without a network. A >=400 status is mapped to the same
/// error kinds as `chat()`; a 200 with an empty/odd body still yields an
/// empty list (the settings picker reports "the provider returned an empty
/// list" for a genuinely empty 200 — that microbehaviour is preserved).
fn parse_models_response(status: u16, text: &str) -> Result<Vec<String>, LlmError> {
    #[derive(Deserialize)]
    struct Models {
        #[serde(default)]
        data: Vec<ModelEntry>,
    }
    #[derive(Deserialize)]
    struct ModelEntry {
        id: String,
    }
    let error_message = serde_json::from_str::<ChatResponse>(text)
        .ok()
        .and_then(|r| r.error)
        .map(|e| match e.message {
            serde_json::Value::String(s) => s,
            other => other.to_string(),
        });
    match status {
        401 | 403 => {
            return Err(LlmError::Auth(
                error_message.unwrap_or_else(|| format!("HTTP {status}")),
            ));
        }
        429 => {
            return Err(LlmError::RateLimited(
                error_message.unwrap_or_else(|| "HTTP 429".into()),
            ));
        }
        s if s >= 400 => {
            return Err(LlmError::Provider(
                error_message.unwrap_or_else(|| format!("HTTP {s}: {}", body_preview(text))),
            ));
        }
        _ => {}
    }
    let parsed: Models = serde_json::from_str(text)
        .map_err(|e| LlmError::Shape(format!("models list: {e}")))?;
    Ok(parsed.data.into_iter().map(|m| m.id).collect())
}

/// A char-boundary-safe prefix (<= `200` bytes) of a response body, for error
/// messages. Slicing `&text[..200]` directly panics when byte 200 lands in
/// the middle of a multibyte char (a garbage non-JSON reply with Cyrillic /
/// emoji). Snap down to the nearest boundary instead.
fn body_preview(text: &str) -> &str {
    let mut end = text.len().min(200);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_body_picks_token_field_by_provider() {
        let poe = LlmClient::new("https://api.poe.com/v1/", "k", "Claude-Sonnet-4.6");
        let body = poe.request_body("s", "u", 1000);
        assert_eq!(body["max_completion_tokens"], 1000);
        assert!(body.get("max_tokens").is_none());
        assert!(body.get("response_format").is_none());

        let ollama = LlmClient::new("http://localhost:11434/v1", "x", "llama3");
        let body = ollama.request_body("s", "u", 500);
        assert_eq!(body["max_tokens"], 500);
        assert!(body.get("max_completion_tokens").is_none());
    }

    #[test]
    fn response_parsing_covers_the_error_matrix() {
        // Happy path.
        let ok = r#"{"choices":[{"message":{"role":"assistant","content":"привет"}}]}"#;
        let parsed = parse_chat_response(200, ok).unwrap();
        assert_eq!(parsed.content, "привет");
        assert_eq!(parsed.retries, 0);
        // Auth.
        let auth = r#"{"error":{"message":"bad key","type":"auth"}}"#;
        assert!(matches!(
            parse_chat_response(401, auth),
            Err(LlmError::Auth(m)) if m == "bad key"
        ));
        // OpenRouter-style error inside a 200, with numeric code.
        let in_200 = r#"{"error":{"code":502,"message":"upstream died"},"choices":[]}"#;
        assert!(matches!(
            parse_chat_response(200, in_200),
            Err(LlmError::Provider(m)) if m == "upstream died"
        ));
        // 429.
        assert!(matches!(
            parse_chat_response(429, "{}"),
            Err(LlmError::RateLimited(_))
        ));
        // Garbage body.
        assert!(matches!(
            parse_chat_response(200, "<html>nope</html>"),
            Err(LlmError::Shape(_))
        ));
    }

    #[test]
    fn shape_error_body_preview_survives_multibyte_garbage() {
        // A non-JSON reply (status 200, no error field) longer than 200 bytes
        // whose char straddles byte 200 used to panic slicing &text[..200].
        let mut body = "x".repeat(199);
        body.push('я'); // bytes 199..=200; byte 200 is mid-char
        assert!(!body.is_char_boundary(200));
        // Must return a Shape error, never panic.
        assert!(matches!(parse_chat_response(200, &body), Err(LlmError::Shape(_))));
    }

    #[test]
    fn response_preserves_finish_usage_refusal_and_typed_content() {
        let body = r#"{
            "id":"req-7",
            "choices":[{
                "finish_reason":"length",
                "message":{"content":[{"type":"text","text":"one"},{"type":"text","text":" two"}],"refusal":"policy"}
            }],
            "usage":{"prompt_tokens":11,"completion_tokens":5,"total_tokens":16}
        }"#;
        let parsed = parse_chat_response(200, body).unwrap();
        assert_eq!(parsed.content, "one two");
        assert_eq!(parsed.finish_reason.as_deref(), Some("length"));
        assert_eq!(parsed.refusal.as_deref(), Some("policy"));
        assert_eq!(parsed.request_id.as_deref(), Some("req-7"));
        assert_eq!(parsed.usage.prompt_tokens, Some(11));
        assert_eq!(parsed.usage.completion_tokens, Some(5));
    }

    #[test]
    fn models_response_surfaces_failures_but_keeps_empty_list_ok() {
        // 401 with a provider error body must become Auth, not Ok([]).
        let bad_key = r#"{"error":{"message":"bad key"}}"#;
        assert!(matches!(
            parse_models_response(401, bad_key),
            Err(LlmError::Auth(m)) if m == "bad key"
        ));
        assert!(matches!(
            parse_models_response(429, "{}"),
            Err(LlmError::RateLimited(_))
        ));
        assert!(matches!(
            parse_models_response(500, r#"{"error":{"message":"boom"}}"#),
            Err(LlmError::Provider(m)) if m == "boom"
        ));
        // Happy path: ids extracted in order.
        let ok = r#"{"data":[{"id":"gpt-x"},{"id":"gpt-y"}]}"#;
        assert_eq!(
            parse_models_response(200, ok).unwrap(),
            vec!["gpt-x".to_string(), "gpt-y".to_string()]
        );
        // MUST stay Ok(empty): the settings panel shows "the provider returned
        // an empty list" for a genuine 200 empty list.
        assert_eq!(parse_models_response(200, r#"{"data":[]}"#).unwrap(), Vec::<String>::new());
        assert_eq!(parse_models_response(200, "{}").unwrap(), Vec::<String>::new());
        // Non-JSON 200 body -> Shape, not a silent empty list.
        assert!(matches!(
            parse_models_response(200, "<html>nope</html>"),
            Err(LlmError::Shape(_))
        ));
    }

}
