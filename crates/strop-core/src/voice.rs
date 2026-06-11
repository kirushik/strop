//! Voice fingerprint v0 — deterministic stylometry, no model, no logprobs
//! (research 2026-06-11). Honest scope per Eder's sample-size floor: at
//! essay length this detects *drift in coarse statistics* relative to the
//! writer's own text, never identity. Sentence-rhythm flattening (falling
//! CV/burstiness) gets top billing: it is the single most LLM-
//! characteristic deterministic signal. The real surprisal-based metric
//! remains post-MVP.

use crate::typograph::Lang;

const EN_FUNCTION_WORDS: &[&str] = &[
    "the", "of", "and", "a", "to", "in", "is", "it", "that", "was", "for", "on", "are", "as",
    "with", "his", "her", "they", "at", "be", "this", "have", "from", "or", "had", "by", "not",
    "but", "what", "all", "were", "we", "when", "your", "can", "said", "there", "an", "which",
    "their", "if", "do", "will", "each", "about", "how", "up", "out", "them", "then", "she",
    "many", "some", "so", "would", "into", "has", "more", "could", "no", "than", "been", "who",
    "its", "now", "did", "down", "only", "over", "just", "also", "after", "very", "any",
];

const RU_FUNCTION_WORDS: &[&str] = &[
    "и", "в", "не", "на", "я", "что", "он", "с", "это", "как", "а", "то", "все", "она", "так",
    "его", "но", "да", "ты", "к", "у", "же", "вы", "за", "бы", "по", "ее", "мне", "было", "вот",
    "от", "меня", "еще", "нет", "о", "из", "ему", "уж", "вам", "ведь", "там", "потом", "себя",
    "ничего", "ей", "может", "они", "тут", "где", "есть", "надо", "ней", "для", "мы", "тебя",
    "их", "чем", "была", "сам", "чтоб", "без", "будто", "чего", "раз", "тоже", "себе", "под",
    "будет", "тогда", "кто", "этот", "лишь", "разве", "хотя", "даже", "ли", "если", "или",
];

#[derive(Debug, Clone, PartialEq)]
pub struct Signature {
    /// Relative frequencies over the per-language function-word list.
    pub function_words: Vec<f32>,
    pub sentence_mean: f32,
    pub sentence_sd: f32,
    /// Coefficient of variation — rhythm. Flattening = LLM smell.
    pub sentence_cv: f32,
    /// Goh–Barabási burstiness B = (σ−μ)/(σ+μ) ∈ (−1, 1).
    pub burstiness: f32,
    /// Per-1000-word rates: em-dash, semicolon, colon, parens, !, ?, comma.
    pub punct: [f32; 7],
    /// MATTR, window 100 — length-robust lexical diversity.
    pub mattr: f32,
    pub mean_word_len: f32,
    pub words: usize,
    pub sentences: usize,
}

fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for c in text.chars() {
        if c.is_alphabetic() {
            for lc in c.to_lowercase() {
                current.push(lc);
            }
        } else if !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn sentence_lengths(text: &str) -> Vec<usize> {
    let mut lengths = Vec::new();
    let mut count = 0usize;
    let mut in_word = false;
    for c in text.chars() {
        if c.is_alphabetic() {
            if !in_word {
                count += 1;
                in_word = true;
            }
        } else {
            in_word = false;
            if matches!(c, '.' | '!' | '?' | '…') && count > 0 {
                lengths.push(count);
                count = 0;
            }
        }
    }
    if count > 0 {
        lengths.push(count);
    }
    lengths
}

pub fn signature(text: &str, lang: Lang) -> Signature {
    let toks = tokens(text);
    let words = toks.len().max(1);
    let list = match lang {
        Lang::Ru => RU_FUNCTION_WORDS,
        Lang::En => EN_FUNCTION_WORDS,
    };
    let function_words = list
        .iter()
        .map(|w| toks.iter().filter(|t| t == w).count() as f32 / words as f32)
        .collect();

    let lens = sentence_lengths(text);
    let n = lens.len().max(1) as f32;
    let mean = lens.iter().sum::<usize>() as f32 / n;
    let var = lens.iter().map(|&l| (l as f32 - mean).powi(2)).sum::<f32>() / n;
    let sd = var.sqrt();
    let cv = if mean > 0. { sd / mean } else { 0. };
    let burstiness = if sd + mean > 0. {
        (sd - mean) / (sd + mean)
    } else {
        0.
    };

    let per_k = |c: usize| c as f32 * 1000. / words as f32;
    let count = |chs: &[char]| text.chars().filter(|c| chs.contains(c)).count();
    let punct = [
        per_k(count(&['—'])),
        per_k(count(&[';'])),
        per_k(count(&[':'])),
        per_k(count(&['(', ')'])),
        per_k(count(&['!'])),
        per_k(count(&['?'])),
        per_k(count(&[','])),
    ];

    // MATTR, window 100 (Covington & McFall): sliding-window mean TTR.
    let window = 100usize;
    let mattr = if toks.len() < window {
        let unique: std::collections::HashSet<&String> = toks.iter().collect();
        unique.len() as f32 / words as f32
    } else {
        let mut counts: std::collections::HashMap<&str, usize> = Default::default();
        let mut distinct = 0usize;
        let mut sum = 0f32;
        let mut windows = 0usize;
        for i in 0..toks.len() {
            let entry = counts.entry(toks[i].as_str()).or_insert(0);
            if *entry == 0 {
                distinct += 1;
            }
            *entry += 1;
            if i >= window {
                let out = counts.get_mut(toks[i - window].as_str()).unwrap();
                *out -= 1;
                if *out == 0 {
                    distinct -= 1;
                }
            }
            if i + 1 >= window {
                sum += distinct as f32 / window as f32;
                windows += 1;
            }
        }
        sum / windows.max(1) as f32
    };

    let mean_word_len =
        toks.iter().map(|t| t.chars().count()).sum::<usize>() as f32 / words as f32;

    Signature {
        function_words,
        sentence_mean: mean,
        sentence_sd: sd,
        sentence_cv: cv,
        burstiness,
        punct,
        mattr,
        mean_word_len,
        words: toks.len(),
        sentences: lens.len(),
    }
}

/// Descriptive per-feature drift between two texts of the same document —
/// no baseline corpus, so no σ-flagging (Eder): observations, not verdicts.
/// Ordered by salience; rhythm first (the LLM-characteristic block).
pub fn describe_drift(from: &Signature, to: &Signature, lang: Lang) -> Vec<String> {
    let mut out = Vec::new();
    let ru = lang == Lang::Ru;
    let pct = |a: f32, b: f32| {
        if a.abs() < 1e-6 {
            0.
        } else {
            (b - a) / a * 100.
        }
    };
    let cv_pct = pct(from.sentence_cv, to.sentence_cv);
    if cv_pct.abs() >= 15. {
        out.push(if ru {
            format!(
                "ритм фраз: разброс длины {} на {:.0}%",
                if cv_pct < 0. { "сузился" } else { "вырос" },
                cv_pct.abs()
            )
        } else {
            format!(
                "sentence rhythm: length variance {} {:.0}%",
                if cv_pct < 0. { "narrowed" } else { "widened" },
                cv_pct.abs()
            )
        });
    }
    let mean_pct = pct(from.sentence_mean, to.sentence_mean);
    if mean_pct.abs() >= 20. {
        out.push(if ru {
            format!(
                "средняя длина фразы: {:.0} → {:.0} слов",
                from.sentence_mean, to.sentence_mean
            )
        } else {
            format!(
                "mean sentence length: {:.0} → {:.0} words",
                from.sentence_mean, to.sentence_mean
            )
        });
    }
    let punct_names_en = ["em-dash", "semicolon", "colon", "parens", "!", "?", "comma"];
    let punct_names_ru = ["тире", "точка с запятой", "двоеточие", "скобки", "!", "?", "запятая"];
    for i in 0..7 {
        let (a, b) = (from.punct[i], to.punct[i]);
        if (a - b).abs() >= 1.5 && a.max(b) >= 2. {
            let name = if ru { punct_names_ru[i] } else { punct_names_en[i] };
            out.push(if ru {
                format!("{name}: {a:.1} → {b:.1} на 1000 слов")
            } else {
                format!("{name}: {a:.1} → {b:.1} per 1000 words")
            });
        }
    }
    let mattr_pct = pct(from.mattr, to.mattr);
    if mattr_pct.abs() >= 8. {
        out.push(if ru {
            format!("лексическое разнообразие (MATTR): {:.0}%", mattr_pct)
        } else {
            format!("lexical diversity (MATTR): {:+.0}%", mattr_pct)
        });
    }
    out.truncate(4);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_text_drifts_nowhere() {
        let text = "Первое предложение здесь. Второе — длиннее и с тире, конечно. Третье?";
        let s = signature(text, Lang::Ru);
        assert!(describe_drift(&s, &s, Lang::Ru).is_empty());
        assert_eq!(s.sentences, 3);
    }

    #[test]
    fn rhythm_flattening_is_loudest() {
        // Varied rhythm vs metronome prose of similar length.
        let varied = "Да. А вот это предложение тянется, петляет и никуда не спешит, набирая слова. Хватит. Теперь снова длинное, с придаточными, которые цепляются друг за друга до самого конца.";
        let flat = "Это предложение ровно средней длины и формы. Это предложение ровно средней длины и формы. Это предложение ровно средней длины и формы. Это предложение ровно средней длины опять.";
        let a = signature(varied, Lang::Ru);
        let b = signature(flat, Lang::Ru);
        assert!(a.sentence_cv > b.sentence_cv);
        let drift = describe_drift(&a, &b, Lang::Ru);
        assert!(!drift.is_empty());
        assert!(drift[0].contains("ритм"), "rhythm leads: {drift:?}");
    }

    #[test]
    fn function_words_and_punct_count() {
        let s = signature("кот и пёс, и ещё кот — и всё.", Lang::Ru);
        // "и" appears 3 times of 8 words.
        let i_ix = RU_FUNCTION_WORDS.iter().position(|w| *w == "и").unwrap();
        assert!((s.function_words[i_ix] - 3. / 8.).abs() < 1e-4);
        assert!(s.punct[0] > 0.); // em-dash counted
    }
}
