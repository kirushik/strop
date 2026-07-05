//! The diagnosis run (C3): Strop's editorial theory (docs/editorial-foundations.md) as a prompt and
//! a parser. Named problems as queries to the author; zero rewrites; the
//! Gaiman guardrail; voice is never a defect.

use std::ops::Range;

use serde::Deserialize;

use crate::document::{Annotation, NoteKind, NoteStatus};
use crate::llm::extract_json_array;

#[derive(Debug, Deserialize, PartialEq)]
pub struct Diagnosis {
    pub quote: String,
    pub problem: String,
    pub query: String,
    #[serde(default)]
    pub level: String,
}

pub fn system_prompt(mode: &str) -> String {
    let mode_clause = match mode {
        "developmental" => "Confine yourself to DEVELOPMENTAL editing: structure, argument, what the piece is and wants to be. Ignore sentence-level matters.",
        "copy" => "Confine yourself to COPY editing: consistency, usage, factual slips, mechanical problems. Do not question structure or style.",
        _ => "Work at the LINE level by default — clarity, momentum, dead weight — but flag a structural problem if it overshadows everything else.",
    };
    format!(
        "You are a literary editor in the tradition of Maxwell Perkins: you diagnose, \
you never rewrite. The author's voice is not yours to average away — never flag \
voice, style, rhythm, or unusual phrasing as a problem unless it fails the \
author's own apparent intent. Readers are right that something is wrong and \
almost always wrong about how to fix it; you name what is wrong and stop.\n\
{mode_clause}\n\
Respond with ONLY a JSON array, no prose, no markdown fences:\n\
[{{\"quote\": \"exact verbatim excerpt from the text, under 120 characters, \
unique enough to locate\", \"problem\": \"the named problem, a few words\", \
\"query\": \"ONE question to the author in the manuscript-query tradition, at \
most two sentences so it is actually read; never replacement text. Prefer the \
canonical form — 'this passage is doing X — is that intentional?' or 'a reader \
might [effect] here — is that what you want?': name the effect, presuppose the \
author's competence, stay open-ended. It is one editor's reading, not a verdict\", \"level\": \"developmental|line|copy\"}}]\n\
At most 7 items, most important first. An empty array is an acceptable and \
honorable answer. Write problem and query in the language of the manuscript."
    )
}

/// Elbow's believing game as a pass (research 2026-06-11): named, quoted
/// strengths with mechanisms — a discipline, not a mood. No shipping tool
/// does this; Strop's margin idiom (observations as queries) is exactly
/// Elbow's "Sharing and Responding" register.
pub fn believing_system_prompt() -> String {
    "You are playing Peter Elbow's believing game. Your only job is to find \
what is true, alive, and working in this draft, and to name it precisely \
enough that the writer can do it again on purpose. You are diagnosing \
strengths, not judging quality. Report reader experience (\"I leaned in \
at...\", \"the energy pools around...\"), never verdicts.\n\
Respond with ONLY a JSON array, no prose, no markdown fences:\n\
[{\"quote\": \"exact verbatim excerpt, under 120 characters\", \"problem\": \
\"the named observation kind + the move's name\", \"query\": \"one sentence: \
the MECHANISM (what it does to the reader and why), then optionally one \
extension question pointing where else the move might serve\", \"level\": \
\"move|center|alive|almost\"}]\n\
Exactly these kinds: 2-3 items of level \"move\" (a working craft move, \
named with a craft term); exactly 1 of level \"center\" (Elbow's center of \
gravity — the source of energy, the generative center, which may NOT be \
the thesis: say what the piece seems secretly about and ask if that is \
the essay being written); exactly 1 of level \"alive\" (the single sentence \
where the voice clicks on, and what changes there); at most 1 of level \
\"almost\" (something gestured at twice but never stated, asked as a \
question). Maximum 5 items total.\n\
RULES: every item must contain a verbatim quote. Evaluative adjectives \
(great, beautiful, strong, powerful) are BANNED unless followed by the \
craft mechanism. Advice verbs (cut, add, rewrite, change, consider) are \
BANNED — observations and questions only. No summary praise. If the piece \
is weak, return fewer, truer items — scarcity is the credibility signal. \
Write in the language of the manuscript."
        .to_owned()
}

/// The believing game's mirror (impl 04 §0, review H34): the strongest case
/// AGAINST the draft, named the same disciplined way — quoted, mechanism-first,
/// as questions to the author, never rewrites. Form-NEUTRAL by mandate: no
/// form primitive exists yet, so it must not assert the piece is an "argument"
/// (a novelist opening this row would meet a thesis her manuscript doesn't
/// have). It mirrors `believing_system_prompt`'s shape exactly — a small fixed
/// set of levels, max 5, scarcity as the credibility signal — and returns the
/// SAME parse format, so `to_annotations` anchors it unchanged. The level words
/// are deliberately NOT "developmental"/"copy": those two drive the altitude
/// gate (`suppressed_copy`), and a doubting card is a whole-piece objection,
/// not a copy nit — it should surface like any open diagnosis, never be held
/// back under a developmental one.
pub fn doubting_system_prompt() -> String {
    "You are playing the doubting game — Peter Elbow's disciplined opposite of \
the believing game. Your only job is to build the strongest honest case \
AGAINST this draft on its own terms: where a fair, attentive reader stops \
trusting it, loses the thread, or puts it down. You are diagnosing weakness, \
not scoring quality, and never rewriting. Report reader experience (\"I \
stopped believing at...\", \"my attention slid off here...\"), never verdicts \
or fixes.\n\
Respond with ONLY a JSON array, no prose, no markdown fences:\n\
[{\"quote\": \"exact verbatim excerpt, under 120 characters\", \"problem\": \
\"the named weakness kind + what gives way\", \"query\": \"one sentence: the \
MECHANISM (what it does to the reader and why it costs trust), asked as a \
question that presupposes the author's competence — never replacement text\", \
\"level\": \"doubt|weakest|flat|unearned\"}]\n\
Exactly these kinds: 2-3 items of level \"doubt\" (a specific place a \
skeptical reader pushes back, named with a craft term); exactly 1 of level \
\"weakest\" (the single most load-bearing weakness — the one thing most \
likely to make the piece fail on its OWN terms, which may NOT be the obvious \
flaw); at most 1 of level \"flat\" (the single sentence where the piece goes \
inert and the reader disengages); at most 1 of level \"unearned\" (something \
the piece assumes or asserts but has not yet earned, asked as a question). \
Maximum 5 items total.\n\
RULES: every item must contain a verbatim quote. This is the DOUBTING game — \
find fault honestly, but voice is never a defect: never flag rhythm, register, \
or unusual phrasing unless it fails the author's own apparent intent. Advice \
verbs (cut, add, rewrite, change, consider) are BANNED — objections and \
questions only. If the piece is strong, return fewer, truer doubts — scarcity \
is the credibility signal. Write in the language of the manuscript."
        .to_owned()
}

pub fn user_prompt(text: &str) -> String {
    format!("The manuscript:\n\n{text}")
}

pub fn parse(response: &str) -> Result<Vec<Diagnosis>, String> {
    let json = extract_json_array(response).ok_or("no JSON array in response")?;
    serde_json::from_str(json).map_err(|e| e.to_string())
}

/// Locate a quote in the text as a char range, searching from `after_char`
/// (sequential anchoring handles repeated quotes).
pub fn anchor(text: &str, quote: &str, after_char: usize) -> Option<Range<usize>> {
    if quote.is_empty() {
        return None;
    }
    let after_byte = text
        .char_indices()
        .nth(after_char)
        .map(|(b, _)| b)
        .unwrap_or(text.len());
    let found = text[after_byte..]
        .find(quote)
        .map(|b| b + after_byte)
        .or_else(|| text.find(quote))?;
    let start_char = text[..found].chars().count();
    let end_char = start_char + quote.chars().count();
    Some(start_char..end_char)
}

/// Diagnoses -> annotations, anchored sequentially, skipping anything the
/// author has already dismissed (same named problem, overlapping span).
pub fn to_annotations(
    text: &str,
    diagnoses: Vec<Diagnosis>,
    existing: &crate::document::Annotations,
    created_unix: i64,
    pass_id: u64,
) -> Vec<Annotation> {
    let mut out = Vec::new();
    let mut cursor = 0usize;
    for d in diagnoses {
        let Some(range) = anchor(text, &d.quote, cursor) else {
            continue; // model hallucinated the quote; drop silently
        };
        // Advance PAST this match so a repeated identical quote anchors to its
        // NEXT occurrence (anchor() wraps around via its or_else fallback when
        // the tail no longer contains the quote, so single/out-of-order quotes
        // still resolve). range.start would re-find the same occurrence.
        cursor = range.end;
        // Skip what the writer already dismissed (don't re-nag) AND what an open
        // card already covers (don't stack a duplicate on a re-run).
        if existing.is_suppressed(&range, &d.problem) {
            continue;
        }
        out.push(Annotation {
            id: 0, // assigned by Annotations::push
            range,
            body: d.query,
            status: NoteStatus::Open,
            created_unix,
            kind: NoteKind::Diagnosis,
            title: d.problem,
            level: d.level,
            orphaned: false,
            pass_id,
            unverified: false,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Annotations;

    #[test]
    fn parses_fenced_response_and_anchors_cyrillic() {
        let response = "Вот разбор:\n```json\n[{\"quote\": \"зарыта мысль\", \
\"problem\": \"похороненный лид\", \"query\": \"Почему главное — в третьем абзаце?\", \
\"level\": \"developmental\"}]\n```";
        let diagnoses = parse(response).unwrap();
        assert_eq!(diagnoses.len(), 1);
        let text = "Здесь зарыта мысль, и зарыта мысль глубоко.";
        let range = anchor(text, &diagnoses[0].quote, 0).unwrap();
        assert_eq!(range, 6..18);
        // Sequential anchoring finds the SECOND occurrence next.
        let second = anchor(text, "зарыта мысль", range.start + 1).unwrap();
        assert_eq!(second.start, 22);
    }

    #[test]
    fn believing_prompt_carries_the_rules() {
        let p = believing_system_prompt();
        assert!(p.contains("believing game"));
        assert!(p.contains("BANNED"));
        assert!(p.contains("center of gravity"));
        assert!(p.contains("JSON array"));
    }

    #[test]
    fn doubting_prompt_mirrors_believing_form_neutrally() {
        let p = doubting_system_prompt();
        // The doubting game, its own discipline (not the believing one).
        assert!(p.contains("doubting game"));
        assert!(p.contains("case AGAINST"));
        // Form-neutral (review H34): never asserts the piece is an "argument".
        assert!(!p.to_lowercase().contains("argument"));
        // Same parser contract as believing: JSON array of quote/level items,
        // advice verbs banned (queries, not rewrites), voice never a defect.
        assert!(p.contains("JSON array"));
        assert!(p.contains("BANNED"));
        assert!(p.contains("voice is never a defect"));
        // Its level words avoid the altitude-gate strings so a doubting card
        // is never held back as "copy" under a "developmental" one.
        assert!(p.contains("doubt|weakest|flat|unearned"));
        assert!(!p.contains("developmental"));
        assert!(!p.contains("\"copy\""));
    }

    #[test]
    fn diagnosis_prompt_carries_the_query_grammar() {
        // The constrained Socratic card (core-loop research, deep-dive 4c):
        // canonical "doing X — intentional?", short enough to be read,
        // non-authoritative voice.
        let p = system_prompt("line");
        assert!(p.contains("is that intentional?"), "canonical query form");
        assert!(p.contains("two sentences"), "length discipline");
        assert!(p.contains("not a verdict"), "non-authoritative voice");
    }

    #[test]
    fn repeated_quote_anchors_to_successive_occurrences() {
        // Two diagnoses quoting the same phrase must land on the first AND the
        // second occurrence, not stack on the first (cursor advances by
        // range.end). With cursor=range.start the second re-found 6..18.
        let text = "Здесь зарыта мысль, и зарыта мысль глубоко.";
        let mk = |problem: &str| Diagnosis {
            quote: "зарыта мысль".into(),
            problem: problem.into(),
            query: "?".into(),
            level: "line".into(),
        };
        let existing = Annotations::default();
        let out = to_annotations(text, vec![mk("p1"), mk("p2")], &existing, 1, 1);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].range, 6..18);
        assert_eq!(out[1].range, 22..34);
    }

    #[test]
    fn dismissed_diagnoses_stay_dismissed() {
        let text = "слабое место здесь";
        let mut existing = Annotations::default();
        // A previously-DISMISSED diagnosis (empty title) covering the span.
        let id = existing.push(Annotation {
            id: 0,
            range: 0..12,
            body: "q".into(),
            status: NoteStatus::Open,
            created_unix: 0,
            kind: NoteKind::Diagnosis,
            title: String::new(),
            level: "line".into(),
            orphaned: false,
            pass_id: 0,
            unverified: false,
        });
        existing.set_status(id, crate::document::NoteStatus::Dismissed);
        // Same span, same problem name -> suppressed only when title matches.
        let mk = |problem: &str| Diagnosis {
            quote: "слабое место".into(),
            problem: problem.into(),
            query: "?".into(),
            level: "line".into(),
        };
        // The dismissed diagnosis has empty title; a diagnosis with empty title
        // is suppressed, a differently-named one is not.
        let kept = to_annotations(text, vec![mk("hedging")], &existing, 1, 1);
        assert_eq!(kept.len(), 1);
        let suppressed =
            to_annotations(text, vec![Diagnosis { problem: "".into(), ..mk("") }], &existing, 1, 1);
        assert!(suppressed.is_empty());
    }

    #[test]
    fn open_duplicate_is_suppressed_on_rerun() {
        // A re-run that re-flags the same problem over an OPEN card must not
        // stack a duplicate (don't nag, don't bloat the lane).
        let text = "слабое место здесь";
        let mut existing = Annotations::default();
        let mk = |problem: &str| Diagnosis {
            quote: "слабое место".into(),
            problem: problem.into(),
            query: "?".into(),
            level: "line".into(),
        };
        let first = to_annotations(text, vec![mk("buried lede")], &existing, 1, 1);
        assert_eq!(first.len(), 1);
        for a in first {
            existing.push(a);
        }
        // Second pass re-flags the same problem at the same span -> suppressed.
        let again = to_annotations(text, vec![mk("buried lede")], &existing, 2, 2);
        assert!(again.is_empty(), "open duplicate should be suppressed");
        // A different problem at the same span still comes through.
        let other = to_annotations(text, vec![mk("passive voice")], &existing, 2, 2);
        assert_eq!(other.len(), 1);
    }
}
