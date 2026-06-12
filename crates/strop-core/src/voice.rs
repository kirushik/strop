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

/// Flatten a signature into one feature vector. Layout: function words,
/// then [cv, burstiness, mean sentence length], punct rates, [mattr,
/// mean word length].
fn flatten(s: &Signature) -> Vec<f32> {
    let mut v = s.function_words.clone();
    v.extend([s.sentence_cv, s.burstiness, s.sentence_mean]);
    v.extend(s.punct);
    v.extend([s.mattr, s.mean_word_len]);
    v
}

fn feature_name(ix: usize, lang: Lang) -> String {
    let fw = match lang {
        Lang::Ru => RU_FUNCTION_WORDS,
        Lang::En => EN_FUNCTION_WORDS,
    };
    let n = fw.len();
    let ru = lang == Lang::Ru;
    match ix {
        i if i < n => {
            if ru {
                format!("частота «{}»", fw[i])
            } else {
                format!("frequency of \"{}\"", fw[i])
            }
        }
        i if i == n => if ru { "разброс длины фраз".into() } else { "sentence-length variance".into() },
        i if i == n + 1 => if ru { "ритмическая неровность".into() } else { "rhythm burstiness".into() },
        i if i == n + 2 => if ru { "средняя длина фразы".into() } else { "mean sentence length".into() },
        i if i < n + 10 => {
            let names_en = ["em-dash", "semicolon", "colon", "parens", "!", "?", "comma"];
            let names_ru = ["тире", "точка с запятой", "двоеточие", "скобки", "!", "?", "запятая"];
            let p = ix - n - 3;
            if ru {
                format!("частота: {}", names_ru[p])
            } else {
                format!("{} rate", names_en[p])
            }
        }
        i if i == n + 10 => if ru { "лексическое разнообразие".into() } else { "lexical diversity".into() },
        _ => if ru { "длина слов".into() } else { "word length".into() },
    }
}

/// Self-baseline over the writer's own corpus (>=3 documents), with
/// leave-one-out calibration of normal self-variation — the research
/// recipe. Flags are nameable per-feature observations, never identity
/// verdicts (Eder's floor).
pub struct Baseline {
    pub docs: usize,
    mean: Vec<f32>,
    sd: Vec<f32>,
    delta_mean: f32,
    delta_sd: f32,
    lang: Lang,
}

const SIGMA_FLOOR: f32 = 1e-4;

fn mean_sd(rows: &[Vec<f32>]) -> (Vec<f32>, Vec<f32>) {
    let n = rows.len() as f32;
    let dims = rows[0].len();
    let mut mean = vec![0f32; dims];
    for r in rows {
        for (m, v) in mean.iter_mut().zip(r) {
            *m += v / n;
        }
    }
    let mut sd = vec![0f32; dims];
    for r in rows {
        for ((s, v), m) in sd.iter_mut().zip(r).zip(&mean) {
            *s += (v - m).powi(2) / n;
        }
    }
    for s in &mut sd {
        *s = s.sqrt().max(SIGMA_FLOOR);
    }
    (mean, sd)
}

/// Mean |z| over the function-word block — the Burrows-Delta-style
/// distance against a baseline.
fn fw_delta(v: &[f32], mean: &[f32], sd: &[f32], fw_len: usize) -> f32 {
    (0..fw_len)
        .map(|i| ((v[i] - mean[i]) / sd[i]).abs())
        .sum::<f32>()
        / fw_len as f32
}

pub fn baseline(texts: &[String], lang: Lang) -> Option<Baseline> {
    if texts.len() < 3 {
        return None;
    }
    let rows: Vec<Vec<f32>> = texts
        .iter()
        .map(|t| flatten(&signature(t, lang)))
        .collect();
    let fw_len = match lang {
        Lang::Ru => RU_FUNCTION_WORDS.len(),
        Lang::En => EN_FUNCTION_WORDS.len(),
    };
    let (mean, sd) = mean_sd(&rows);
    // Leave-one-out: each doc's distance to the baseline of the others.
    let mut deltas = Vec::new();
    for i in 0..rows.len() {
        let others: Vec<Vec<f32>> = rows
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, r)| r.clone())
            .collect();
        let (m, s) = mean_sd(&others);
        deltas.push(fw_delta(&rows[i], &m, &s, fw_len));
    }
    let dn = deltas.len() as f32;
    let delta_mean = deltas.iter().sum::<f32>() / dn;
    let delta_sd = (deltas
        .iter()
        .map(|d| (d - delta_mean).powi(2))
        .sum::<f32>()
        / dn)
        .sqrt()
        .max(SIGMA_FLOOR);
    Some(Baseline {
        docs: texts.len(),
        mean,
        sd,
        delta_mean,
        delta_sd,
        lang,
    })
}

pub struct VoiceReport {
    /// How far outside normal self-variation the draft sits, in LOO sigmas.
    pub overall_sigma: f32,
    /// Per-feature |z|>2 observations, loudest first, max 5.
    pub flags: Vec<String>,
}

impl Baseline {
    /// The corpus language. Signatures fed to `assess` MUST be built with
    /// it: the function-word vectors are per-language and differently
    /// sized — mixing languages would index out of bounds.
    pub fn lang(&self) -> Lang {
        self.lang
    }

    pub fn assess(&self, draft: &Signature) -> VoiceReport {
        let v = flatten(draft);
        let fw_len = match self.lang {
            Lang::Ru => RU_FUNCTION_WORDS.len(),
            Lang::En => EN_FUNCTION_WORDS.len(),
        };
        let delta = fw_delta(&v, &self.mean, &self.sd, fw_len);
        let overall_sigma = (delta - self.delta_mean) / self.delta_sd;
        let ru = self.lang == Lang::Ru;
        let mut scored: Vec<(f32, String)> = Vec::new();
        for (i, value) in v.iter().enumerate() {
            let z = (value - self.mean[i]) / self.sd[i];
            if z.abs() > 2. {
                let name = feature_name(i, self.lang);
                scored.push((
                    z.abs(),
                    if ru {
                        format!("{name}: {:+.1}σ от вашей нормы", z)
                    } else {
                        format!("{name}: {:+.1}σ from your baseline", z)
                    },
                ));
            }
        }
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        VoiceReport {
            overall_sigma,
            flags: scored.into_iter().take(5).map(|(_, f)| f).collect(),
        }
    }
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
    fn baseline_needs_three_docs_and_calibrates() {
        let style_a = |n: usize| {
            (0..n)
                .map(|i| format!("Я думаю, что это решает дело номер {i}. Коротко. А потом — длинная фраза, которая петляет и не спешит к точке, потому что так дышит автор."))
                .collect::<Vec<_>>()
                .join(" ")
        };
        let corpus: Vec<String> = (3..7).map(|k| style_a(k * 4)).collect();
        assert!(baseline(&corpus[..2], Lang::Ru).is_none());
        let b = baseline(&corpus, Lang::Ru).unwrap();
        // A corpus member sits within normal self-variation.
        let member = signature(&corpus[0], Lang::Ru);
        let r = b.assess(&member);
        assert!(r.overall_sigma < 2., "member flagged: {}", r.overall_sigma);
        // A metronome draft with alien punctuation drifts measurably.
        let alien = "Это предложение средней длины без украшений; точно так же; и снова так; всегда одинаково; без вариаций; механически; ровно; гладко; одинаково; совсем одинаково."
            .repeat(8);
        let r2 = b.assess(&signature(&alien, Lang::Ru));
        assert!(
            r2.overall_sigma > r.overall_sigma,
            "alien {} <= member {}",
            r2.overall_sigma,
            r.overall_sigma
        );
        assert!(!r2.flags.is_empty(), "no flags for alien draft");
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
