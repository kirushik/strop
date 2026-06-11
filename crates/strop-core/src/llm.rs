//! OpenAI-compatible chat-completions client (C2). One client covers Poe
//! (the primary target — Kirill's subscription), OpenAI, OpenRouter,
//! ollama, and Anthropic's compat endpoint.
//!
//! Decisions from the provider research (2026-06-11):
//! - Bearer auth everywhere (Anthropic compat accepts it; ollama requires
//!   a dummy header).
//! - `max_completion_tokens`, except base URLs that look like ollama
//!   (the one holdout still documenting only `max_tokens`).
//! - NO `response_format`: Poe and Anthropic-compat silently ignore it,
//!   so structured output is prompt-and-parse with a lenient extractor.
//! - OpenRouter can put errors inside an HTTP 200 body — always check.
//! - Blocking ureq; callers run this on a background thread.

use std::time::Duration;

use serde::Deserialize;
use serde_json::json;

#[derive(Debug)]
pub enum LlmError {
    /// 401/403 — fix the key/settings; never retried.
    Auth(String),
    /// 429 — retryable upstream, surfaced after our single retry.
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

    /// Blocking chat-completion call. Run on a background thread.
    pub fn chat(&self, system: &str, user: &str, max_tokens: u32) -> Result<String, LlmError> {
        let body = self.request_body(system, user, max_tokens);
        let mut response = self
            .agent
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .send_json(&body)
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
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| LlmError::Network(e.to_string()))?;
        #[derive(Deserialize)]
        struct Models {
            #[serde(default)]
            data: Vec<ModelEntry>,
        }
        #[derive(Deserialize)]
        struct ModelEntry {
            id: String,
        }
        let parsed: Models = serde_json::from_str(&text)
            .map_err(|e| LlmError::Shape(format!("models list: {e}")))?;
        Ok(parsed.data.into_iter().map(|m| m.id).collect())
    }
}

#[derive(Deserialize)]
struct ChatResponse {
    #[serde(default)]
    error: Option<ApiError>,
    #[serde(default)]
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct ApiError {
    #[serde(default)]
    message: serde_json::Value,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    #[serde(default)]
    content: String,
}

fn parse_chat_response(status: u16, text: &str) -> Result<String, LlmError> {
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
        parsed.map_err(|e| LlmError::Shape(format!("{e}; body: {}", &text[..text.len().min(200)])))?;
    response
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .filter(|c| !c.is_empty())
        .ok_or_else(|| LlmError::Shape("no choices in response".into()))
}

/// Lenient JSON-array extraction for prompt-and-parse structured output:
/// strips markdown fences, takes first '[' .. last ']'.
pub fn extract_json_array(text: &str) -> Option<&str> {
    let start = text.find('[')?;
    let end = text.rfind(']')?;
    (end > start).then(|| &text[start..=end])
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
        assert_eq!(parse_chat_response(200, ok).unwrap(), "привет");
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
    fn json_array_extraction_is_lenient() {
        assert_eq!(
            extract_json_array("```json\n[{\"a\":1}]\n```"),
            Some("[{\"a\":1}]")
        );
        assert_eq!(extract_json_array("Вот:\n[1, 2]\nготово"), Some("[1, 2]"));
        assert_eq!(extract_json_array("no array"), None);
    }
}
