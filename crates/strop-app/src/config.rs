//! User settings: ~/.config/strop/config.toml (XDG_CONFIG_HOME honored).
//! Missing file = defaults; a malformed file is reported and defaults win
//! (never crash the editor over a typo in TOML).

use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    /// Kirill's habit: any selection is also copied to the clipboard
    /// (beyond the PRIMARY selection, which always works).
    pub auto_copy_selection: bool,
    /// Typograph language override: "auto" (default), "ru", "en".
    pub language: Language,
    /// Body text size in px (line height scales at 1.4, rhythm-rounded).
    pub font_size: Option<f32>,
    pub ai: AiConfig,
    pub voice: VoiceConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct VoiceConfig {
    /// Globs of the writer's own past texts (.md/.txt/.strop) — the
    /// self-baseline corpus for voice-drift flagging (needs >= 3 files).
    pub corpus: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    Auto,
    Ru,
    En,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct AiConfig {
    /// OpenAI-compatible chat-completions base URL; one client covers
    /// Poe / OpenAI / OpenRouter / ollama / Anthropic-compat.
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    /// Default levels-of-edit depth: "developmental" | "line" | "copy".
    pub mode: String,
}

impl AiConfig {
    pub fn configured(&self) -> bool {
        !self.base_url.is_empty() && !self.model.is_empty()
    }

    /// STROP_API_KEY wins over the file (the plaintext-averse path).
    pub fn resolved_api_key(&self) -> String {
        std::env::var("STROP_API_KEY")
            .ok()
            .filter(|k| !k.is_empty())
            .unwrap_or_else(|| self.api_key.clone())
    }
}

/// Commented starter config; written once, then the user's file is never
/// touched again (comments survive because Strop only ever reads it).
const TEMPLATE: &str = r#"# Strop configuration. Edit and save — Strop re-reads this file before
# every AI pass, so no restart is needed.

[ai]
# Any OpenAI-compatible chat-completions endpoint works. Examples:
#   Poe:        base_url = "https://api.poe.com/v1"        model = "Claude-Sonnet-4.5"
#   OpenRouter: base_url = "https://openrouter.ai/api/v1"  model = "anthropic/claude-sonnet-4.5"
#   Ollama:     base_url = "http://localhost:11434/v1"     model = "llama3.3"
base_url = ""
# The key sits in plain text here — chmod 600 this file, or leave it empty
# and export STROP_API_KEY instead (the environment variable wins).
api_key = ""
model = ""
# Default depth of the editorial pass: "developmental" | "line" | "copy".
mode = "line"

# [voice]
# corpus = ["~/essays/*.md"]   # your own texts; >= 3 enable drift flags
"#;

/// Ensure a starter config exists; returns its path either way.
pub fn write_template_if_missing() -> PathBuf {
    let path = config_path();
    if !path.exists() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, TEMPLATE);
    }
    path
}

pub fn config_path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").expect("HOME not set")).join(".config")
        })
        .join("strop/config.toml")
}

pub fn load() -> Config {
    match std::fs::read_to_string(config_path()) {
        Ok(text) => match toml::from_str(&text) {
            Ok(config) => config,
            Err(e) => {
                eprintln!("strop: config.toml ignored: {e}");
                Config::default()
            }
        },
        Err(_) => Config::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_partial_config() {
        let config: Config = toml::from_str(
            "auto_copy_selection = true\nlanguage = \"ru\"\n\n[ai]\nbase_url = \"https://api.poe.com/v1\"\nmodel = \"Claude-Sonnet-4.5\"\n",
        )
        .unwrap();
        assert!(config.auto_copy_selection);
        assert!(config.voice.corpus.is_empty());
        assert_eq!(config.language, Language::Ru);
        assert_eq!(config.ai.base_url, "https://api.poe.com/v1");
        assert!(config.ai.api_key.is_empty());
        // Empty/missing input is fine too.
        let default: Config = toml::from_str("").unwrap();
        assert!(!default.auto_copy_selection);
    }
}

#[cfg(test)]
mod template_tests {
    use super::*;

    #[test]
    fn template_parses_as_valid_config() {
        let config: Config = toml::from_str(TEMPLATE).expect("template must stay valid TOML");
        assert!(!config.ai.configured(), "template starts unconfigured");
        assert_eq!(config.ai.mode, "line");
    }
}
