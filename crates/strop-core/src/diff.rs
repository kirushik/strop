//! Prose diff for the rewind surface (research: Scrivener-validated
//! granularity): paragraph-level pass, word-level refinement inside
//! changed paragraphs (UAX-29 words), whole-paragraph fallback when a
//! pair changed beyond recognition. Never character confetti.

use similar::{ChangeTag, TextDiff};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffOp {
    Same,
    Insert,
    Delete,
}

#[derive(Debug, PartialEq)]
pub struct DiffSeg {
    pub op: DiffOp,
    pub text: String,
}

/// Inserted/deleted word counts — the writer's native delta unit.
pub fn word_delta(segs: &[DiffSeg]) -> (usize, usize) {
    let words = |s: &str| s.split_whitespace().count();
    segs.iter().fold((0, 0), |(ins, del), seg| match seg.op {
        DiffOp::Insert => (ins + words(&seg.text), del),
        DiffOp::Delete => (ins, del + words(&seg.text)),
        DiffOp::Same => (ins, del),
    })
}

fn push(out: &mut Vec<DiffSeg>, op: DiffOp, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = out.last_mut() {
        if last.op == op {
            last.text.push_str(text);
            return;
        }
    }
    out.push(DiffSeg {
        op,
        text: text.to_owned(),
    });
}

/// Word-level diff of one changed paragraph pair.
fn word_diff(out: &mut Vec<DiffSeg>, old: &str, new: &str) {
    // Ratio over words only — whitespace tokens would inflate similarity.
    let old_words: Vec<&str> = old.split_whitespace().collect();
    let new_words: Vec<&str> = new.split_whitespace().collect();
    let ratio = TextDiff::from_slices(&old_words, &new_words).ratio();
    let diff = TextDiff::from_unicode_words(old, new);
    if ratio < 0.4 {
        // Rewritten beyond recognition: whole-paragraph replace reads
        // better than interleaved confetti.
        push(out, DiffOp::Delete, old);
        push(out, DiffOp::Insert, new);
        return;
    }
    for change in diff.iter_all_changes() {
        let op = match change.tag() {
            ChangeTag::Equal => DiffOp::Same,
            ChangeTag::Insert => DiffOp::Insert,
            ChangeTag::Delete => DiffOp::Delete,
        };
        push(out, op, change.value());
    }
}

/// Two-pass prose diff. Input texts use '\n' as the block separator
/// (Strop's document model); output segments concatenate to the merged
/// old+new reading view.
pub fn prose_diff(old: &str, new: &str) -> Vec<DiffSeg> {
    let old_pars: Vec<&str> = old.split('\n').collect();
    let new_pars: Vec<&str> = new.split('\n').collect();
    let diff = TextDiff::from_slices(&old_pars, &new_pars);
    let mut out = Vec::new();
    let mut first = true;
    let mut sep = |out: &mut Vec<DiffSeg>| {
        if !first {
            push(out, DiffOp::Same, "\n");
        }
        first = false;
    };
    for op in diff.ops() {
        match *op {
            similar::DiffOp::Equal {
                old_index, len, ..
            } => {
                for par in &old_pars[old_index..old_index + len] {
                    sep(&mut out);
                    push(&mut out, DiffOp::Same, par);
                }
            }
            similar::DiffOp::Delete {
                old_index, old_len, ..
            } => {
                for par in &old_pars[old_index..old_index + old_len] {
                    sep(&mut out);
                    push(&mut out, DiffOp::Delete, par);
                }
            }
            similar::DiffOp::Insert {
                new_index, new_len, ..
            } => {
                for par in &new_pars[new_index..new_index + new_len] {
                    sep(&mut out);
                    push(&mut out, DiffOp::Insert, par);
                }
            }
            similar::DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                let pairs = old_len.min(new_len);
                for i in 0..pairs {
                    sep(&mut out);
                    word_diff(&mut out, old_pars[old_index + i], new_pars[new_index + i]);
                }
                for par in &old_pars[old_index + pairs..old_index + old_len] {
                    sep(&mut out);
                    push(&mut out, DiffOp::Delete, par);
                }
                for par in &new_pars[new_index + pairs..new_index + new_len] {
                    sep(&mut out);
                    push(&mut out, DiffOp::Insert, par);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn merged(segs: &[DiffSeg]) -> String {
        segs.iter().map(|s| s.text.as_str()).collect()
    }

    #[test]
    fn identical_is_one_same_run() {
        let segs = prose_diff("абзац один\nабзац два", "абзац один\nабзац два");
        assert!(segs.iter().all(|s| s.op == DiffOp::Same));
        assert_eq!(merged(&segs), "абзац один\nабзац два");
    }

    #[test]
    fn word_change_stays_word_sized() {
        let segs = prose_diff(
            "редактор называет проблему",
            "редактор называет вопрос",
        );
        let (ins, del) = word_delta(&segs);
        assert_eq!((ins, del), (1, 1));
        // The unchanged prefix survives as Same.
        assert!(
            segs.iter()
                .any(|s| s.op == DiffOp::Same && s.text.contains("называет"))
        );
    }

    #[test]
    fn rewritten_paragraph_falls_back_to_block_replace() {
        let segs = prose_diff(
            "старый текст совсем о другом предмете",
            "новая мысль, не имеющая ничего общего",
        );
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].op, DiffOp::Delete);
        assert_eq!(segs[1].op, DiffOp::Insert);
    }

    #[test]
    fn paragraph_insertion_between_unchanged() {
        let segs = prose_diff("один\nтри", "один\nдва\nтри");
        let (ins, del) = word_delta(&segs);
        assert_eq!((ins, del), (1, 0));
        assert!(merged(&segs).contains("два"));
    }
}
