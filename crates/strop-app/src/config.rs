//! User settings: `config.toml` in the per-user config dir (see `paths` —
//! `~/.config/strop` on Linux, the OS-native config folder elsewhere).
//! Missing file = defaults; a malformed file is reported and defaults win
//! (never crash the editor over a typo in TOML).

use std::path::{Path, PathBuf};

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
    crate::paths::config_dir().join("config.toml")
}

/// Write the [ai] provider fields through toml_edit (DESIGN §0 directive 3:
/// the UI writes through toml_edit, comments preserved; hand edits stay
/// respected). A missing file starts from the commented TEMPLATE; a
/// malformed one is refused, never overwritten. `api_key: None` leaves the
/// stored key untouched (the STROP_API_KEY path — the panel never writes a
/// key the environment is already supplying).
pub fn save_ai(base_url: &str, api_key: Option<&str>, model: &str) -> Result<PathBuf, String> {
    let path = config_path();
    save_ai_to(&path, base_url, api_key, model)?;
    Ok(path)
}

fn save_ai_to(
    path: &Path,
    base_url: &str,
    api_key: Option<&str>,
    model: &str,
) -> Result<(), String> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|_| TEMPLATE.to_owned());
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|e| format!("config.toml is not valid TOML, refusing to overwrite it: {e}"))?;
    if !doc.get("ai").is_some_and(toml_edit::Item::is_table_like) {
        doc["ai"] = toml_edit::Item::Table(toml_edit::Table::new());
    }
    doc["ai"]["base_url"] = toml_edit::value(base_url);
    if let Some(key) = api_key {
        doc["ai"]["api_key"] = toml_edit::value(key);
    }
    doc["ai"]["model"] = toml_edit::value(model);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("couldn't create {}: {e}", dir.display()))?;
    }
    std::fs::write(path, doc.to_string())
        .map_err(|e| format!("couldn't write {}: {e}", path.display()))
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

#[cfg(test)]
mod save_tests {
    use super::*;

    /// One sequential test for the whole write path. It exercises
    /// `save_ai_to` against an isolated temp config root (env vars are
    /// process-global — see files.rs `lifecycle_in_isolated_home` — so the
    /// path is injected instead of repointing XDG_CONFIG_HOME and racing
    /// that test; `save_ai` is only `config_path()` + this function).
    #[test]
    fn save_ai_writes_in_place_preserving_comments_and_unknown_keys() {
        let tmp = std::env::temp_dir().join(format!("strop-config-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let path = tmp.join("strop/config.toml");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let original = "# top comment survives\nauto_copy_selection = true\n\n[ai]\n# endpoint comment survives too\nbase_url = \"http://old\"\napi_key = \"old-key\"\nmodel = \"old-model\"\nmode = \"copy\"\nunknown_knob = 3\n";
        std::fs::write(&path, original).unwrap();

        save_ai_to(&path, "https://api.poe.com/v1", Some("sk-new"), "Claude-Sonnet-4.5").unwrap();
        let out = std::fs::read_to_string(&path).unwrap();
        // Comments and keys Strop doesn't know about survive the rewrite.
        assert!(out.contains("# top comment survives"), "{out}");
        assert!(out.contains("# endpoint comment survives too"), "{out}");
        assert!(out.contains("unknown_knob = 3"), "{out}");
        assert!(out.contains("auto_copy_selection = true"), "{out}");
        assert!(out.contains("mode = \"copy\""), "{out}");
        let parsed: Config = toml::from_str(&out).unwrap();
        assert_eq!(parsed.ai.base_url, "https://api.poe.com/v1");
        assert_eq!(parsed.ai.api_key, "sk-new");
        assert_eq!(parsed.ai.model, "Claude-Sonnet-4.5");

        // api_key: None (STROP_API_KEY active) leaves the stored key alone.
        save_ai_to(&path, "https://api.poe.com/v1", None, "second-model").unwrap();
        let parsed: Config =
            toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(parsed.ai.api_key, "sk-new");
        assert_eq!(parsed.ai.model, "second-model");

        // Missing file starts from the commented TEMPLATE (comments kept).
        let fresh = tmp.join("fresh/strop/config.toml");
        save_ai_to(&fresh, "http://localhost:11434/v1", Some("k"), "llama3").unwrap();
        let out = std::fs::read_to_string(&fresh).unwrap();
        assert!(out.contains("# Strop configuration"), "{out}");
        let parsed: Config = toml::from_str(&out).unwrap();
        assert!(parsed.ai.configured());
        assert_eq!(parsed.ai.mode, "line", "template's mode default kept");

        // A malformed file is refused, never clobbered.
        let broken = tmp.join("strop/broken.toml");
        std::fs::write(&broken, "not = [valid").unwrap();
        assert!(save_ai_to(&broken, "x", None, "y").is_err());
        assert_eq!(std::fs::read_to_string(&broken).unwrap(), "not = [valid");

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
