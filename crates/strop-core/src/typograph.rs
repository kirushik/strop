//! Typographic input substitution — the Birman contract:
//!
//! 1. Rules are deterministic, never "the user probably meant".
//! 2. Conventions follow the *document* language, never the keyboard layout.
//! 3. Every substitution is its own undo transaction: one undo restores
//!    exactly what was typed, and (because rules trigger only on the typed
//!    character) the restored text never re-fires.
//!
//! Deliberately NOT implemented: digit-range dashes (1941—1945). The typed
//! prefix cannot distinguish a range from an ISO date ("2024-10-…"), and a
//! rule that corrupts dates violates rule 1 harder than its absence violates
//! completeness. Needs boundary-time application with date protection, or
//! the typograph-at-output mode.

const NBSP: char = '\u{00A0}';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Ru,
}

/// Document language by letter majority over (a prefix of) the text.
/// Deterministic and stable under small edits; an explicit per-document
/// setting overrides this upstream.
pub fn detect_lang(chars: impl Iterator<Item = char>) -> Lang {
    let mut cyrillic = 0usize;
    let mut latin = 0usize;
    for c in chars.take(4000) {
        if ('\u{0400}'..='\u{04FF}').contains(&c) {
            cyrillic += 1;
        } else if c.is_ascii_alphabetic() {
            latin += 1;
        }
    }
    if cyrillic > latin { Lang::Ru } else { Lang::En }
}

/// Replace the last `span` bytes before the cursor with `text`.
#[derive(Debug, PartialEq, Eq)]
pub struct Substitution {
    pub span: usize,
    pub text: String,
}

impl Substitution {
    fn new(span: usize, text: impl Into<String>) -> Option<Self> {
        Some(Self {
            span,
            text: text.into(),
        })
    }
}

/// `prefix` is the paragraph text up to the cursor, the just-typed character
/// last; `next` is the character after the cursor, when any. Returns a
/// substitution of a suffix of `prefix`, or None.
pub fn process(prefix: &str, next: Option<char>, lang: Lang) -> Option<Substitution> {
    let typed = prefix.chars().next_back()?;
    match typed {
        '.' => ellipsis(prefix),
        '-' => double_hyphen(prefix),
        '"' => double_quote(prefix, next, lang),
        '\'' => single_quote(prefix, next, lang),
        ' ' => space_rules(prefix, lang),
        _ => None,
    }
}

/// Is a quote opening in this context (start of text, after whitespace or
/// an opening bracket/dash)?
fn opens_after(prev: Option<char>) -> bool {
    match prev {
        None => true,
        Some(c) => {
            c.is_whitespace()
                || matches!(c, '(' | '[' | '{' | '«' | '„' | '“' | '‘' | '—' | '–' | '-' | '/')
        }
    }
}

fn ellipsis(prefix: &str) -> Option<Substitution> {
    // "...." (a fourth dot) means the author wants literal dots — after the
    // third fired, "…" + "." doesn't end with "...", so no re-fire anyway.
    if prefix.ends_with("...") {
        Substitution::new(3, "…")
    } else {
        None
    }
}

fn double_hyphen(prefix: &str) -> Option<Substitution> {
    if prefix.ends_with("--") && !prefix.ends_with("---") {
        Substitution::new(2, "—")
    } else {
        None
    }
}

fn closes_empty_pair(next: Option<char>) -> bool {
    next.is_none_or(|c| {
        c.is_whitespace()
            || matches!(c, ')' | ']' | '}' | '»' | '”' | '’' | ',' | '.' | ':' | ';' | '!' | '?')
    })
}

fn double_quote(prefix: &str, next: Option<char>, lang: Lang) -> Option<Substitution> {
    let before = &prefix[..prefix.len() - 1];
    if closes_empty_pair(next) {
        match (lang, before.chars().next_back()) {
            (Lang::En, Some('“')) => return Substitution::new(1, "”"),
            (Lang::Ru, Some('„')) => return Substitution::new(1, "“"),
            _ => {}
        }
    }
    let opening = opens_after(before.chars().next_back());
    let quote = match lang {
        Lang::En => {
            if opening {
                '“'
            } else {
                '”'
            }
        }
        Lang::Ru => {
            // Nesting per Birman: «внешние „внутренние“ кавычки».
            let outer_depth = depth(before, '«', '»');
            let inner_depth = depth(before, '„', '“');
            match (opening, outer_depth > 0, inner_depth > 0) {
                (true, true, _) => '„',
                (true, false, _) => '«',
                (false, _, true) => '“',
                (false, _, false) => '»',
            }
        }
    };
    Substitution::new(1, quote.to_string())
}

/// Unmatched-opener count for a quote pair within the paragraph prefix.
fn depth(text: &str, open: char, close: char) -> usize {
    let mut depth = 0usize;
    for c in text.chars() {
        if c == open {
            depth += 1;
        } else if c == close {
            depth = depth.saturating_sub(1);
        }
    }
    depth
}

fn single_quote(prefix: &str, next: Option<char>, lang: Lang) -> Option<Substitution> {
    let before = &prefix[..prefix.len() - 1];
    let prev = before.chars().next_back();
    // Apostrophe between letters (don’t, д’Артаньян) in both languages.
    if prev.is_some_and(char::is_alphabetic) {
        return Substitution::new(1, "’");
    }
    if lang == Lang::En && prev == Some('‘') && closes_empty_pair(next) {
        return Substitution::new(1, "’");
    }
    match lang {
        Lang::En => {
            if opens_after(prev) {
                Substitution::new(1, "‘")
            } else {
                Substitution::new(1, "’")
            }
        }
        // No conventional single-quote role in Russian text; leave as typed.
        Lang::Ru => None,
    }
}

fn space_rules(prefix: &str, lang: Lang) -> Option<Substitution> {
    spaced_dash(prefix, lang).or_else(|| match lang {
        Lang::Ru => short_word_nbsp(prefix),
        Lang::En => None,
    })
}

/// "word - " -> "word — " with NBSP before the dash so it never starts a
/// line (тире не отрывается от предыдущего слова). Requires a space before
/// the hyphen, so list dashes at paragraph start are untouched.
fn spaced_dash(prefix: &str, _lang: Lang) -> Option<Substitution> {
    if !prefix.ends_with(" - ") {
        return None;
    }
    let before = &prefix[..prefix.len() - 3];
    if before.is_empty() || before.ends_with('-') {
        return None;
    }
    Substitution::new(3, format!("{NBSP}— "))
}

/// Russian short prepositions/conjunctions bind to the following word:
/// the space after them becomes NBSP («в лесу», «не так»).
const RU_SHORT_WORDS: &[&str] = &[
    "а", "и", "в", "к", "о", "с", "у", "я", "бы", "во", "до", "же", "за", "из", "ко", "ли", "на",
    "не", "ни", "но", "об", "от", "по", "со", "то", "уж",
];

fn short_word_nbsp(prefix: &str) -> Option<Substitution> {
    let before = &prefix[..prefix.len() - 1];
    let word_start = before
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_alphabetic())
        .last()
        .map(|(i, _)| i)?;
    let word = &before[word_start..];
    // The word itself must be preceded by an opening context, not mid-word.
    if !opens_after(before[..word_start].chars().next_back()) {
        return None;
    }
    let lower = word.to_lowercase();
    if RU_SHORT_WORDS.contains(&lower.as_str()) {
        Substitution::new(1, NBSP.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sub(prefix: &str, lang: Lang) -> Option<(usize, String)> {
        process(prefix, None, lang).map(|s| (s.span, s.text))
    }

    fn sub_before(prefix: &str, next: char, lang: Lang) -> Option<(usize, String)> {
        process(prefix, Some(next), lang).map(|s| (s.span, s.text))
    }

    #[test]
    fn detects_language() {
        assert_eq!(detect_lang("Plain English text.".chars()), Lang::En);
        assert_eq!(detect_lang("Просто русский текст.".chars()), Lang::Ru);
        // Majority wins for mixed documents.
        assert_eq!(
            detect_lang("Русский текст с one word".chars()),
            Lang::Ru
        );
        assert_eq!(detect_lang("".chars()), Lang::En);
    }

    #[test]
    fn ellipsis_both_langs() {
        assert_eq!(sub("так...", Lang::Ru), Some((3, "…".into())));
        assert_eq!(sub("so...", Lang::En), Some((3, "…".into())));
        assert_eq!(sub("so..", Lang::En), None);
        // After substitution, "…" + "." cannot re-fire.
        assert_eq!(sub("so….", Lang::En), None);
    }

    #[test]
    fn double_hyphen_to_em_dash() {
        assert_eq!(sub("word--", Lang::En), Some((2, "—".into())));
        assert_eq!(sub("word---", Lang::En), None); // deliberate ASCII art
    }

    #[test]
    fn english_double_quotes() {
        assert_eq!(sub("\"", Lang::En), Some((1, "“".into())));
        assert_eq!(sub("He said \"", Lang::En), Some((1, "“".into())));
        assert_eq!(sub("He said “hi\"", Lang::En), Some((1, "”".into())));
        assert_eq!(sub("(\"", Lang::En), Some((1, "“".into())));
        assert_eq!(sub("“\"", Lang::En), Some((1, "”".into())));
        assert_eq!(sub_before("“\"", 'x', Lang::En), Some((1, "“".into())));
    }

    #[test]
    fn russian_quotes_nest() {
        assert_eq!(sub("Он сказал \"", Lang::Ru), Some((1, "«".into())));
        assert_eq!(sub("«Фильм \"", Lang::Ru), Some((1, "„".into())));
        assert_eq!(sub("«Фильм „Ирония\"", Lang::Ru), Some((1, "“".into())));
        assert_eq!(sub("«Фильм „Ирония“ хорош\"", Lang::Ru), Some((1, "»".into())));
        assert_eq!(sub("«\"", Lang::Ru), Some((1, "„".into())));
        assert_eq!(sub("«„\"", Lang::Ru), Some((1, "“".into())));
    }

    #[test]
    fn apostrophes_and_singles() {
        assert_eq!(sub("don'", Lang::En), Some((1, "’".into())));
        assert_eq!(sub("д'", Lang::Ru), Some((1, "’".into())));
        assert_eq!(sub("'", Lang::En), Some((1, "‘".into())));
        assert_eq!(sub("said ‘hi'", Lang::En), Some((1, "’".into())));
        assert_eq!(sub("‘'", Lang::En), Some((1, "’".into())));
        assert_eq!(sub_before("‘'", 'x', Lang::En), Some((1, "‘".into())));
        assert_eq!(sub("сказал '", Lang::Ru), None);
    }

    #[test]
    fn spaced_hyphen_becomes_dash_with_nbsp() {
        assert_eq!(
            sub("слово - ", Lang::Ru),
            Some((3, "\u{00A0}— ".into()))
        );
        assert_eq!(sub("word - ", Lang::En), Some((3, "\u{00A0}— ".into())));
        // List dash at paragraph start is not a dash-between-words.
        assert_eq!(sub("- ", Lang::En), None);
        // "-- " already became an em dash on the second hyphen.
        assert_eq!(sub("word-- ", Lang::En), None);
    }

    #[test]
    fn russian_short_words_get_nbsp() {
        assert_eq!(sub("в ", Lang::Ru), Some((1, "\u{00A0}".into())));
        assert_eq!(sub("Я иду в ", Lang::Ru), Some((1, "\u{00A0}".into())));
        assert_eq!(sub("Не ", Lang::Ru), Some((1, "\u{00A0}".into())));
        // Longer words keep their ordinary space.
        assert_eq!(sub("лесу ", Lang::Ru), None);
        // Mid-word fragments don't count ("остров" ends with "ов").
        assert_eq!(sub("остров ", Lang::Ru), None);
        // English documents are untouched.
        assert_eq!(sub("a ", Lang::En), None);
    }

    #[test]
    fn ordinary_typing_passes_through() {
        assert_eq!(sub("обычное слово", Lang::Ru), None);
        assert_eq!(sub("word ", Lang::En), None);
        assert_eq!(sub("word-", Lang::En), None);
    }
}
