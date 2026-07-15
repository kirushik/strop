//! OpenAI-compatible chat-completions client (C2). One client covers Poe
//! (the primary target — Kirill's subscription), OpenAI, OpenRouter,
//! ollama, and Anthropic's compat endpoint.
//!
//! Decisions from the provider research (2026-06-11):
//! - Bearer auth everywhere (Anthropic compat accepts it; ollama requires
//!   a dummy header).
//! - `max_completion_tokens`, except base URLs that look like ollama
//!   (the one holdout still documenting only `max_tokens`).
//! - JSON Schema output on the three endpoints that document it (OpenAI,
//!   OpenRouter, Ollama); prompt-and-validate remains authoritative everywhere.
//! - OpenRouter can put errors inside an HTTP 200 body — always check.
//! - Blocking ureq; callers run this on a background thread.

use std::time::Duration;
use std::time::Instant;

use serde::Deserialize;
use serde_json::json;

#[derive(Debug)]
pub enum LlmError {
    /// 401/403 — fix the key/settings; never retried.
    Auth(String),
    /// 429 — retryable upstream, surfaced after one bounded retry.
    RateLimited(String),
    /// Stable 4xx request errors — model/settings must change before retrying.
    Request(String),
    /// 5xx/in-body errors; message passed through verbatim (the only
    /// cross-provider constant). These may be transient.
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
            Self::Request(m) => write!(f, "request rejected: {m}"),
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
    pub latency_ms: u64,
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

    fn supports_json_schema(&self) -> bool {
        self.base_url.contains("api.openai.com")
            || self.base_url.contains("openrouter.ai")
            || self.base_url.contains("11434")
            || self.base_url.contains("ollama")
    }

    pub fn structured_request_body(
        &self,
        system: &str,
        user: &str,
        max_tokens: u32,
        schema: &serde_json::Value,
    ) -> serde_json::Value {
        let mut body = self.request_body(system, user, max_tokens);
        if self.supports_json_schema() {
            body["response_format"] = json!({
                "type": "json_schema",
                "json_schema": {
                    "name": "strop_diagnoses",
                    "strict": true,
                    "schema": schema,
                },
            });
        }
        body
    }

    /// Blocking chat-completion call. Run on a background thread. A transient
    /// network failure, 429, or selected 5xx gets one bounded retry; auth and
    /// stable request errors do not.
    pub fn chat(&self, system: &str, user: &str, max_tokens: u32) -> Result<ChatResult, LlmError> {
        let body = self.request_body(system, user, max_tokens);
        self.chat_body(&body)
    }

    pub fn chat_structured(
        &self,
        system: &str,
        user: &str,
        max_tokens: u32,
        schema: &serde_json::Value,
    ) -> Result<ChatResult, LlmError> {
        let body = self.structured_request_body(system, user, max_tokens, schema);
        let has_schema = body.get("response_format").is_some();
        let started = Instant::now();
        match self.chat_body(&body) {
            Err(LlmError::Request(_)) if has_schema => {
                // Schema output is prevention, never the portable contract. A
                // host/model pair may reject it even when another model on the
                // same endpoint accepts it; retry once with prompt + local
                // validation rather than turning a capability miss into a
                // settings loop.
                let fallback = self.request_body(system, user, max_tokens);
                self.chat_body(&fallback).map(|mut result| {
                    result.retries = result.retries.saturating_add(1);
                    result.latency_ms = started.elapsed().as_millis() as u64;
                    result
                })
            }
            other => other,
        }
    }

    fn chat_body(&self, body: &serde_json::Value) -> Result<ChatResult, LlmError> {
        let started = Instant::now();
        let first = self.chat_once(body);
        let (result, retries) = if first.retryable {
            std::thread::sleep(first.retry_after.unwrap_or_else(fallback_retry_delay));
            (self.chat_once(body).result, 1)
        } else {
            (first.result, 0)
        };
        result.map(|mut result| {
            result.retries = retries;
            result.latency_ms = started.elapsed().as_millis() as u64;
            result
        })
    }

    fn chat_once(&self, body: &serde_json::Value) -> ChatAttempt {
        let response = self
            .agent
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .send_json(body);
        let mut response = match response {
            Ok(response) => response,
            Err(error) => {
                let retryable = retryable_network_error(&error);
                return ChatAttempt {
                    result: Err(LlmError::Network(error.to_string())),
                    retryable,
                    retry_after: None,
                };
            }
        };
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(retry_after_delay);
        let text = match response.body_mut().read_to_string() {
            Ok(text) => text,
            Err(error) => {
                return ChatAttempt {
                    result: Err(LlmError::Network(error.to_string())),
                    retryable: retryable_network_error(&error),
                    retry_after,
                };
            }
        };
        ChatAttempt {
            result: parse_chat_response(status, &text),
            retryable: retryable_status(status),
            retry_after,
        }
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

struct ChatAttempt {
    result: Result<ChatResult, LlmError>,
    retryable: bool,
    retry_after: Option<Duration>,
}

fn retryable_status(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504)
}

fn retryable_network_error(error: &ureq::Error) -> bool {
    !matches!(error, ureq::Error::Timeout(_))
}

/// LLM providers normally send delta-seconds. Clamp an upstream value so a
/// malformed or hostile header cannot park the one background request forever.
fn retry_after_delay(value: &str) -> Option<Duration> {
    value
        .trim()
        .parse::<u64>()
        .ok()
        .map(|seconds| Duration::from_secs(seconds.min(30)))
}

fn fallback_retry_delay() -> Duration {
    let jitter = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.subsec_millis() as u64 % 200)
        .unwrap_or(0);
    Duration::from_millis(500 + jitter)
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
        s @ 400..=499 => {
            return Err(LlmError::Request(
                error_message.unwrap_or_else(|| format!("HTTP {s}: {text}")),
            ));
        }
        s if s >= 500 => {
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
    if content.is_empty()
        && choice.message.refusal.is_none()
        && matches!(choice.finish_reason.as_deref(), None | Some("stop"))
    {
        return Err(LlmError::Shape("choice contained no text".into()));
    }
    Ok(ChatResult {
        content,
        finish_reason: choice.finish_reason,
        refusal: choice.message.refusal,
        usage: response.usage,
        request_id: response.id,
        retries: 0,
        latency_ms: 0,
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
        s @ 400..=499 => {
            return Err(LlmError::Request(
                error_message.unwrap_or_else(|| format!("HTTP {s}: {}", body_preview(text))),
            ));
        }
        s if s >= 500 => {
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
    use std::io::{BufRead as _, BufReader, Read as _, Write as _};
    use std::net::TcpListener;
    use serde_json::Value;

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
    fn structured_output_is_an_opportunistic_host_shim() {
        let schema = json!({"type":"object"});
        for base_url in [
            "https://api.openai.com/v1",
            "https://openrouter.ai/api/v1",
            "http://localhost:11434/v1",
        ] {
            let client = LlmClient::new(base_url, "k", "model");
            let body = client.structured_request_body("s", "u", 100, &schema);
            assert_eq!(body["response_format"]["type"], "json_schema");
            assert_eq!(body["response_format"]["json_schema"]["strict"], true);
        }

        let poe = LlmClient::new("https://api.poe.com/v1", "k", "model");
        assert!(poe
            .structured_request_body("s", "u", 100, &schema)
            .get("response_format")
            .is_none());
    }

    #[test]
    fn structured_output_downgrades_once_after_a_request_rejection() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let mut bodies = Vec::new();
            for index in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut content_length = 0usize;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    if let Some(value) = line
                        .to_ascii_lowercase()
                        .strip_prefix("content-length:")
                    {
                        content_length = value.trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; content_length];
                reader.read_exact(&mut body).unwrap();
                bodies.push(serde_json::from_slice::<Value>(&body).unwrap());
                let (status, response) = if index == 0 {
                    ("400 Bad Request", r#"{"error":{"message":"unsupported response_format"}}"#)
                } else {
                    (
                        "200 OK",
                        r#"{"choices":[{"finish_reason":"stop","message":{"content":"[]"}}]}"#,
                    )
                };
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                    response.len()
                )
                .unwrap();
            }
            bodies
        });
        let client = LlmClient::new(
            &format!("http://{address}/ollama"),
            "key",
            "model",
        );

        let result = client
            .chat_structured("system", "user", 100, &json!({"type":"array"}))
            .unwrap();
        let bodies = server.join().unwrap();

        assert_eq!(result.content, "[]");
        assert_eq!(result.retries, 1);
        assert!(bodies[0].get("response_format").is_some());
        assert!(bodies[1].get("response_format").is_none());
    }

    #[test]
    fn retry_policy_covers_transient_status_and_bounds_provider_delay() {
        for status in [429, 500, 502, 503, 504] {
            assert!(retryable_status(status));
        }
        for status in [400, 401, 403, 404, 501] {
            assert!(!retryable_status(status));
        }
        assert_eq!(retry_after_delay("7"), Some(Duration::from_secs(7)));
        assert_eq!(retry_after_delay("999"), Some(Duration::from_secs(30)));
        assert_eq!(retry_after_delay("tomorrow"), None);
        assert!(!retryable_network_error(&ureq::Error::Timeout(
            ureq::Timeout::Global
        )));
        assert!(retryable_network_error(&ureq::Error::HostNotFound));
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
        // Stable request error, distinct from a retryable upstream failure.
        assert!(matches!(
            parse_chat_response(404, r#"{"error":{"message":"model not found"}}"#),
            Err(LlmError::Request(m)) if m == "model not found"
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
    fn empty_completion_preserves_actionable_finish_reason() {
        for reason in ["length", "content_filter"] {
            let body = format!(
                "{{\"choices\":[{{\"finish_reason\":\"{reason}\",\"message\":{{\"content\":\"\"}}}}]}}"
            );
            let parsed = parse_chat_response(200, &body).unwrap();
            assert!(parsed.content.is_empty());
            assert_eq!(parsed.finish_reason.as_deref(), Some(reason));
        }
        let stopped = r#"{"choices":[{"finish_reason":"stop","message":{"content":""}}]}"#;
        assert!(matches!(
            parse_chat_response(200, stopped),
            Err(LlmError::Shape(_))
        ));
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
        assert!(matches!(
            parse_models_response(404, r#"{"error":{"message":"missing"}}"#),
            Err(LlmError::Request(m)) if m == "missing"
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
