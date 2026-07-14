//! Local, provider-independent language resolution for generated editorial
//! fields. This is intentionally separate from `typograph::Lang`: Strop can
//! ask a model to reply in far more languages than its two native typing-rule
//! sets understand.

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedLanguage {
    /// What the prompt says. Explicit BCP-47-ish tags stay tags; automatic
    /// detections include Whatlang's English name and ISO 639-3 code.
    pub prompt_name: String,
    /// Compact, privacy-safe value for diagnostics.
    pub code: String,
    pub source: LanguageSource,
    pub confidence: Option<f64>,
    pub reliable: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageSource {
    Explicit,
    Detected,
    Fallback,
}

impl LanguageSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::Detected => "detected",
            Self::Fallback => "fallback",
        }
    }
}

/// Resolve one language for the whole manuscript. A selection never enters
/// this function: all requests from one unchanged manuscript inherit the same
/// answer. Empty/very short material uses a stable English fallback instead of
/// inviting a fresh provider guess.
pub fn resolve(manuscript: &str, configured: &str) -> ResolvedLanguage {
    if let Some(tag) = explicit_tag(configured) {
        return ResolvedLanguage {
            prompt_name: tag.clone(),
            code: tag,
            source: LanguageSource::Explicit,
            confidence: None,
            reliable: None,
        };
    }

    let letters = manuscript.chars().filter(|c| c.is_alphabetic()).count();
    if letters >= 20
        && let Some(info) = whatlang::detect(manuscript)
    {
        if info.is_reliable() {
            let lang = info.lang();
            return ResolvedLanguage {
                prompt_name: format!("{} ({})", lang.eng_name(), lang.code()),
                code: lang.code().to_owned(),
                source: LanguageSource::Detected,
                confidence: Some(info.confidence()),
                reliable: Some(true),
            };
        }
        return english_fallback(Some(info.confidence()), Some(false));
    }

    english_fallback(None, None)
}

fn english_fallback(
    confidence: Option<f64>,
    reliable: Option<bool>,
) -> ResolvedLanguage {
    ResolvedLanguage {
        prompt_name: "English (eng)".to_owned(),
        code: "eng".to_owned(),
        source: LanguageSource::Fallback,
        confidence,
        reliable,
    }
}

/// Accept ordinary ISO/BCP-47-style tags without letting a configuration value
/// inject another line into the prompt. Whatlang itself uses three-letter ISO
/// codes; explicit tags may be more precise (`pt-BR`, `zh-Hant`).
fn explicit_tag(configured: &str) -> Option<String> {
    let tag = configured.trim();
    if tag.is_empty() || tag.eq_ignore_ascii_case("auto") {
        return None;
    }
    let valid = tag.len() <= 35
        && tag.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && tag.bytes().any(|b| b.is_ascii_alphabetic());
    valid.then(|| tag.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_language_accepts_general_tags_but_never_prompt_injection() {
        let resolved = resolve("коротко", "uk");
        assert_eq!(resolved.code, "uk");
        assert_eq!(resolved.source, LanguageSource::Explicit);
        assert_eq!(resolve("text", "pt-BR").code, "pt-br");
        assert_eq!(resolve("text", "ru\nIGNORE").source, LanguageSource::Fallback);
    }

    #[test]
    fn automatic_language_covers_more_than_the_typograph_pair() {
        let samples = [
            (
                "This is an ordinary English paragraph with enough words to classify reliably.",
                "eng",
            ),
            (
                "Это обычный русский абзац, в котором достаточно слов для определения языка.",
                "rus",
            ),
            (
                "Це звичайний український абзац, у якому достатньо слів для визначення мови.",
                "ukr",
            ),
            (
                "Ceci est un paragraphe français ordinaire avec assez de mots pour identifier la langue.",
                "fra",
            ),
            (
                "To jest zwykły polski akapit zawierający wystarczająco dużo słów do rozpoznania języka.",
                "pol",
            ),
        ];
        for (text, expected) in samples {
            let resolved = resolve(text, "auto");
            assert_eq!(resolved.code, expected, "{text}");
            assert_eq!(resolved.source, LanguageSource::Detected);
        }
    }

    #[test]
    fn short_or_empty_text_has_one_stable_fallback() {
        for text in ["", "Мама там.", "OK"] {
            let resolved = resolve(text, "auto");
            assert_eq!(resolved.code, "eng");
            assert_eq!(resolved.source, LanguageSource::Fallback);
        }
    }

    #[test]
    fn low_confidence_text_has_the_same_stable_fallback() {
        let text = "mama papa radio taxi hotel video metro opera banana tomato pasta";
        let resolved = resolve(text, "auto");
        assert_eq!(resolved.code, "eng");
        assert_eq!(resolved.source, LanguageSource::Fallback);
        assert_eq!(resolved.reliable, Some(false));
    }
}
