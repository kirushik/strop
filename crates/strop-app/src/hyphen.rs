//! Runtime hyphenation dictionaries: lazy loading, per-word language
//! routing, and the F2 NFC-skip. The engine (bookpage.rs) asks through the
//! `Hyphenate` trait; this module answers from the TeX patterns.
//!
//! The dictionaries are RUNTIME-LOADED loose files
//! (`assets/hyphenation/{en-us,ru}.standard.bincode`, located via
//! `paths::asset_file`), NEVER `include_bytes!`: hyph-ru is LPPL 1.2+,
//! GPL-incompatible — the mere-aggregation posture keeps it an intact
//! independent work on disk (impl 05 §2.4; research-linebreak §1.4). A
//! missing file degrades to justify-without-hyphenation for that script,
//! with one eprintln.

use std::sync::OnceLock;

use hyphenation::{Hyphenator, Language, Load, Standard};
use unicode_segmentation::UnicodeSegmentation;

use crate::bookpage::Hyphenate;
use crate::paths;

static EN: OnceLock<Option<Standard>> = OnceLock::new();
static RU: OnceLock<Option<Standard>> = OnceLock::new();

fn load(lang: Language, file: &str) -> Option<Standard> {
    let Some(path) = paths::asset_file(&format!("hyphenation/{file}")) else {
        eprintln!(
            "strop: hyphenation dictionary {file} not found — justifying without hyphenation"
        );
        return None;
    };
    match Standard::from_path(lang, &path) {
        Ok(dict) => Some(dict),
        Err(e) => {
            eprintln!(
                "strop: cannot load {}: {e} — justifying without hyphenation",
                path.display()
            );
            None
        }
    }
}

fn en() -> Option<&'static Standard> {
    EN.get_or_init(|| load(Language::EnglishUS, "en-us.standard.bincode")).as_ref()
}

fn ru() -> Option<&'static Standard> {
    RU.get_or_init(|| load(Language::Russian, "ru.standard.bincode")).as_ref()
}

/// Per-word language routing without language tags (research §2): the
/// script of the first alphabetic char decides — Cyrillic → ru, Basic
/// Latin/Latin-1 letters → en-US, anything else → no hyphenation. A word
/// is atomic: halves of one word never route to different dictionaries
/// (compounds were already split per part by the tokenizer).
fn route(word: &str) -> Option<&'static Standard> {
    let first = word.chars().find(|c| c.is_alphabetic())?;
    match first {
        'a'..='z' | 'A'..='Z' | '\u{C0}'..='\u{FF}' => en(),
        '\u{0400}'..='\u{04FF}' | '\u{0500}'..='\u{052F}' => ru(),
        _ => None,
    }
}

/// Byte offsets into `word` where a hyphen may break; empty when no
/// dictionary answers. The crate returns offsets realigned to the ORIGINAL
/// string (capitalized words fold internally), enforces each dictionary's
/// own edge minima (en-US 2/3, ru 2/2), and lets an author's soft hyphens
/// override the patterns outright — their byte indices become the only
/// breaks. F2: the lookup is skipped entirely when the word is not already
/// NFC (an NFC copy would not map offsets back onto the slice string, which
/// is never transformed). Every returned offset is guaranteed a char AND
/// grapheme boundary of the original word.
pub fn breaks(word: &str) -> Vec<usize> {
    let Some(dict) = route(word) else {
        return vec![];
    };
    // F2: NFC-copy rule — a word whose NFC form differs from the raw form
    // (NFD input) skips hyphenation, the same degradation as a missing
    // dictionary.
    if !unicode_normalization::is_nfc(word) {
        return vec![];
    }
    let hyphenated = dict.hyphenate(word);
    let graphemes: Vec<usize> = word.grapheme_indices(true).map(|(i, _)| i).collect();
    hyphenated
        .breaks
        .into_iter()
        .filter(|&b| {
            let ok = b > 0
                && b < word.len()
                && word.is_char_boundary(b)
                && graphemes.binary_search(&b).is_ok();
            debug_assert!(ok, "hyphenation break {b} is not a boundary of {word:?}");
            ok
        })
        .collect()
}

/// The engine-facing router: `paginate(…, &mut DictHyphenator)`.
pub struct DictHyphenator;

impl Hyphenate for DictHyphenator {
    fn breaks(&mut self, word: &str) -> Vec<usize> {
        breaks(word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_normalization::UnicodeNormalization;

    fn assert_boundaries(word: &str, bs: &[usize]) {
        let graphemes: Vec<usize> = word.grapheme_indices(true).map(|(i, _)| i).collect();
        for &b in bs {
            assert!(word.is_char_boundary(b), "{b} not a char boundary of {word:?}");
            assert!(graphemes.contains(&b), "{b} not a grapheme boundary of {word:?}");
        }
    }

    /// research §9.2: mixed case folds internally and the computed
    /// opportunities realign to the ORIGINAL string's bytes — a
    /// capitalized word hyphenates exactly like its lowercase form.
    #[test]
    fn capitalized_words_hyphenate_with_realigned_offsets() {
        let b = breaks("Anfractuous");
        assert!(!b.is_empty(), "the en-US dictionary must load from assets/");
        assert_boundaries("Anfractuous", &b);
        assert_eq!(b, breaks("anfractuous"));
    }

    /// «Ёлка»: NFC ё is a single scalar (U+0451) and the ru patterns carry
    /// explicit ё entries; offsets land on char/grapheme boundaries even
    /// past the two-byte Cyrillic capitals.
    #[test]
    fn nfc_yo_routes_to_russian() {
        assert_boundaries("Ёлка", &breaks("Ёлка"));
        let b = breaks("Ёлочка");
        assert!(!b.is_empty(), "the ru dictionary must load and know ё");
        assert_boundaries("Ёлочка", &b);
        let b = breaks("переносами");
        assert!(!b.is_empty());
        assert_boundaries("переносами", &b);
    }

    /// Soft hyphens take priority over the dictionary: their byte indices
    /// are returned as the ONLY breaks (crate behavior, research §1.2).
    #[test]
    fn soft_hyphens_override_the_dictionary() {
        let w = "hy\u{ad}phenation";
        assert_eq!(breaks(w), vec![2], "the shy position is the only break");
        assert_ne!(breaks("hyphenation"), vec![2], "the dictionary disagrees");
    }

    /// F2: a word whose NFC form differs from its raw form (NFD input) is
    /// skipped whole — offsets into a normalized copy would not map back
    /// onto the untransformed slice string.
    #[test]
    fn nfd_words_skip_hyphenation() {
        let nfd: String = "Ёлочка".nfd().collect();
        assert_ne!(nfd, "Ёлочка", "the fixture must actually decompose");
        assert!(breaks(&nfd).is_empty(), "NFD form differs from raw -> skipped");
    }

    /// Routing: Cyrillic → ru, Latin → en-US, anything else → none; words
    /// with no alphabetic core route nowhere.
    #[test]
    fn routing_by_the_first_alphabetic_script() {
        assert!(!breaks("hyphenation").is_empty());
        assert!(!breaks("типографика").is_empty());
        assert!(breaks("ελληνικά").is_empty(), "Greek routes to no dictionary");
        assert!(breaks("12345").is_empty());
        assert!(breaks("——").is_empty());
    }

    /// Apostrophe words pass through whole: the apostrophe never matches a
    /// pattern char, so nearby opportunities are merely suppressed — safe,
    /// just conservative (research §1.2).
    #[test]
    fn apostrophe_words_stay_safe() {
        assert_boundaries("don't", &breaks("don't"));
        let b = breaks("mademoiselle's");
        assert_boundaries("mademoiselle's", &b);
    }
}
