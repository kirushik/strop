//! The diagnosis run (C3): the handoff's editorial theory as a prompt and
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
\"query\": \"one question to the author, in the manuscript-query tradition — \
never replacement text\", \"level\": \"developmental|line|copy\"}}]\n\
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
) -> Vec<Annotation> {
    let mut out = Vec::new();
    let mut cursor = 0usize;
    for d in diagnoses {
        let Some(range) = anchor(text, &d.quote, cursor) else {
            continue; // model hallucinated the quote; drop silently
        };
        cursor = range.start;
        if existing.is_dismissed(&range, &d.problem) {
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
    fn dismissed_diagnoses_stay_dismissed() {
        let text = "слабое место здесь";
        let mut existing = Annotations::default();
        let id = existing.add(0..12, "q".into(), 0);
        existing.set_status(id, crate::document::NoteStatus::Dismissed);
        // Same span, same problem name -> suppressed only when title matches.
        let mk = |problem: &str| Diagnosis {
            quote: "слабое место".into(),
            problem: problem.into(),
            query: "?".into(),
            level: "line".into(),
        };
        // The dismissed note has empty title; a diagnosis with empty title
        // would be suppressed, a differently-named one would not.
        let kept = to_annotations(text, vec![mk("hedging")], &existing, 1);
        assert_eq!(kept.len(), 1);
        let suppressed = to_annotations(text, vec![Diagnosis { problem: "".into(), ..mk("") }], &existing, 1);
        assert!(suppressed.is_empty());
    }
}
