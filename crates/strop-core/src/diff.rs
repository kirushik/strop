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
    if let Some(last) = out.last_mut()
        && last.op == op
    {
        last.text.push_str(text);
        return;
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

/// One block (paragraph) of the merged diff view, with provenance: which
/// paragraph of each source text it renders. Within a block, the Delete
/// segments concatenate to the old paragraph's tail and Same+Insert to the
/// new one's — byte-exact, so formatting can be projected through.
#[derive(Debug, PartialEq)]
pub struct BlockDiff {
    /// Index into `old.split('\n')`, if this block existed in the old text.
    pub old_par: Option<usize>,
    /// Index into `new.split('\n')`, if it exists in the new text.
    pub new_par: Option<usize>,
    pub segs: Vec<DiffSeg>,
}

fn block(old_par: Option<usize>, new_par: Option<usize>, segs: Vec<DiffSeg>) -> BlockDiff {
    BlockDiff {
        old_par,
        new_par,
        segs,
    }
}

/// Two-pass prose diff with block provenance. Input texts use '\n' as the
/// block separator (Strop's document model); one BlockDiff per line of the
/// merged reading view.
pub fn prose_diff_blocks(old: &str, new: &str) -> Vec<BlockDiff> {
    let old_pars: Vec<&str> = old.split('\n').collect();
    let new_pars: Vec<&str> = new.split('\n').collect();
    let diff = TextDiff::from_slices(&old_pars, &new_pars);
    let mut out = Vec::new();
    let seg = |op: DiffOp, text: &str| {
        let mut v = Vec::new();
        push(&mut v, op, text);
        v
    };
    for op in diff.ops() {
        match *op {
            similar::DiffOp::Equal {
                old_index,
                new_index,
                len,
            } => {
                for i in 0..len {
                    out.push(block(
                        Some(old_index + i),
                        Some(new_index + i),
                        seg(DiffOp::Same, old_pars[old_index + i]),
                    ));
                }
            }
            similar::DiffOp::Delete {
                old_index, old_len, ..
            } => {
                for i in 0..old_len {
                    out.push(block(
                        Some(old_index + i),
                        None,
                        seg(DiffOp::Delete, old_pars[old_index + i]),
                    ));
                }
            }
            similar::DiffOp::Insert {
                new_index, new_len, ..
            } => {
                for i in 0..new_len {
                    out.push(block(
                        None,
                        Some(new_index + i),
                        seg(DiffOp::Insert, new_pars[new_index + i]),
                    ));
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
                    let mut segs = Vec::new();
                    word_diff(&mut segs, old_pars[old_index + i], new_pars[new_index + i]);
                    out.push(block(Some(old_index + i), Some(new_index + i), segs));
                }
                for i in pairs..old_len {
                    out.push(block(
                        Some(old_index + i),
                        None,
                        seg(DiffOp::Delete, old_pars[old_index + i]),
                    ));
                }
                for i in pairs..new_len {
                    out.push(block(
                        None,
                        Some(new_index + i),
                        seg(DiffOp::Insert, new_pars[new_index + i]),
                    ));
                }
            }
        }
    }
    out
}

/// Flat segment view: blocks joined by Same '\n' separators (the merged
/// reading view's own newlines — NOT faithful to either source's bytes;
/// use `prose_diff_blocks` when projecting positions back to a source).
pub fn prose_diff(old: &str, new: &str) -> Vec<DiffSeg> {
    let mut out = Vec::new();
    for (i, b) in prose_diff_blocks(old, new).into_iter().enumerate() {
        if i > 0 {
            push(&mut out, DiffOp::Same, "\n");
        }
        for s in b.segs {
            push(&mut out, s.op, &s.text);
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

    #[test]
    fn blocks_carry_provenance() {
        let blocks = prose_diff_blocks("один\nтри", "один\nдва\nтри");
        let pars: Vec<(Option<usize>, Option<usize>)> =
            blocks.iter().map(|b| (b.old_par, b.new_par)).collect();
        assert_eq!(pars, vec![(Some(0), Some(0)), (None, Some(1)), (Some(1), Some(2))]);
    }

    #[test]
    fn block_segments_reconstruct_both_sources_byte_exact() {
        let old = "общий хвост остаётся\nудалённый абзац\nправка внутри строки тут";
        let new = "общий хвост остаётся\nправка прямо в строке тут\nвставленный абзац";
        let blocks = prose_diff_blocks(old, new);
        let old_pars: Vec<&str> = old.split('\n').collect();
        let new_pars: Vec<&str> = new.split('\n').collect();
        for b in &blocks {
            let from_old: String = b
                .segs
                .iter()
                .filter(|s| s.op != DiffOp::Insert)
                .map(|s| s.text.as_str())
                .collect();
            let from_new: String = b
                .segs
                .iter()
                .filter(|s| s.op != DiffOp::Delete)
                .map(|s| s.text.as_str())
                .collect();
            if let Some(p) = b.old_par {
                assert_eq!(from_old, old_pars[p]);
            }
            if let Some(p) = b.new_par {
                assert_eq!(from_new, new_pars[p]);
            }
        }
    }
}
