//! Local, manually-shareable AI diagnostics. Entries contain operational
//! metadata only: never manuscript text, quotes, prompts, raw completions, or
//! credentials. Support conversations decide whether a writer shares it.

use std::io::Write as _;
use std::path::Path;

const AI_LOG_MAX_BYTES: u64 = 2 * 1024 * 1024;

#[derive(serde::Serialize)]
pub struct PassLog {
    pub t_unix_ms: i64,
    pub provider: String,
    pub model: String,
    pub pass: String,
    pub scope: String,
    pub target_chars: usize,
    pub finish_reason: Option<String>,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub retries: u8,
    pub latency_ms: u64,
    pub request_id: Option<String>,
    pub accepted: usize,
    pub rejected: usize,
    pub language: String,
    pub language_source: String,
    pub language_confidence: Option<f64>,
    pub language_reliable: Option<bool>,
}

#[derive(serde::Serialize)]
pub struct FailureLog<'a> {
    pub t_unix_ms: i64,
    pub provider: String,
    pub model: &'a str,
    pub pass: &'a str,
    pub failure: &'a str,
}

pub fn record(entry: &PassLog) {
    append(entry);
}

pub fn record_failure(entry: &FailureLog<'_>) {
    append(entry);
}

fn append(entry: &impl serde::Serialize) {
    let path = crate::paths::state_dir().join("ai-diagnostics.jsonl");
    append_to(&path, entry, AI_LOG_MAX_BYTES);
}

fn append_to(path: &Path, entry: &impl serde::Serialize, max_bytes: u64) {
    if let Some(dir) = path.parent()
        && std::fs::create_dir_all(dir).is_err()
    {
        return;
    }
    let Ok(mut line) = serde_json::to_vec(entry) else { return };
    line.push(b'\n');
    let current = std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    if current.saturating_add(line.len() as u64) > max_bytes {
        let previous = path.with_file_name("ai-diagnostics.previous.jsonl");
        let _ = std::fs::remove_file(&previous);
        let _ = std::fs::rename(path, previous);
    }
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    let _ = file.write_all(&line);
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

pub fn provider_host(base_url: &str) -> String {
    let authority = base_url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("custom");
    authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_log_value_drops_paths_queries_and_credentials() {
        assert_eq!(
            provider_host("https://example.test/v1?key=secret"),
            "example.test"
        );
        assert_eq!(provider_host("http://localhost:11434/v1"), "localhost:11434");
        assert_eq!(
            provider_host("https://user:sk-live-abc@example.test/v1"),
            "example.test"
        );
    }

    #[test]
    fn diagnostics_log_rotates_to_one_bounded_predecessor() {
        let root = std::env::temp_dir().join(format!(
            "strop-ai-log-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("ai-diagnostics.jsonl");
        std::fs::write(&path, "old row that fills the tiny test budget\n").unwrap();

        append_to(&path, &serde_json::json!({"new":"row"}), 16);

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"new\":\"row\"}\n");
        assert!(
            std::fs::read_to_string(root.join("ai-diagnostics.previous.jsonl"))
                .unwrap()
                .starts_with("old row")
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
