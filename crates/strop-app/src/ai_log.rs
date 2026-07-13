//! Local, manually-shareable AI diagnostics. Entries contain operational
//! metadata only: never manuscript text, quotes, prompts, raw completions, or
//! credentials. Support conversations decide whether a writer shares it.

use std::io::Write as _;

#[derive(serde::Serialize)]
pub struct PassLog<'a> {
    pub t_unix_ms: i64,
    pub provider: String,
    pub model: &'a str,
    pub pass: &'a str,
    pub scope: &'a str,
    pub target_chars: usize,
    pub finish_reason: Option<&'a str>,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub retries: u8,
    pub repair_attempted: bool,
    pub accepted: usize,
    pub rejected: usize,
}

#[derive(serde::Serialize)]
pub struct FailureLog<'a> {
    pub t_unix_ms: i64,
    pub provider: String,
    pub model: &'a str,
    pub pass: &'a str,
    pub failure: &'a str,
}

pub fn record(entry: &PassLog<'_>) {
    append(entry);
}

pub fn record_failure(entry: &FailureLog<'_>) {
    append(entry);
}

fn append(entry: &impl serde::Serialize) {
    let path = crate::paths::state_dir().join("ai-diagnostics.jsonl");
    if let Some(dir) = path.parent()
        && std::fs::create_dir_all(dir).is_err()
    {
        return;
    }
    let Ok(mut line) = serde_json::to_vec(entry) else { return };
    line.push(b'\n');
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
    base_url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("custom")
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
    }
}
