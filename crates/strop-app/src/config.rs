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
        assert_eq!(config.language, Language::Ru);
        assert_eq!(config.ai.base_url, "https://api.poe.com/v1");
        assert!(config.ai.api_key.is_empty());
        // Empty/missing input is fine too.
        let default: Config = toml::from_str("").unwrap();
        assert!(!default.auto_copy_selection);
    }
}
