//! Robustness properties — whole-bug-class guards complementing model.rs.
//!
//! 1. No untrusted input PANICS. The recurring bug class here is char/byte
//!    boundary confusion; these properties feed arbitrary (multibyte) input to
//!    every parsing/anchoring entry point and assert it returns rather than
//!    unwinds. (`Store::open` on arbitrary bytes, `from_markdown`,
//!    `import_image`, `diagnose::parse_for`, `diagnose::anchor`,
//!    `typograph::process`.)
//! 2. Full save -> reopen STATE round-trip — the biggest gap model.rs left
//!    open: it only mirrors the live `store.text()` and never closes the disk
//!    loop for spans, blocks, and notes.
//! 3. SpanSet algebraic invariants (sorted, non-empty, self-coverage).
//! 4. diff block reconstruction is byte-exact for arbitrary paragraph lists.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use proptest::prelude::*;

use strop_core::Store;
use strop_core::diagnose::{anchor, parse_for};
use strop_core::diff::{DiffOp, prose_diff_blocks};
use strop_core::document::{BlockKind, BlockMap, Document, InlineAttr, SpanSet};
use strop_core::images::import_image;
use strop_core::markdown::from_markdown;
use strop_core::typograph::{Lang, process};

static SEQ: AtomicU64 = AtomicU64::new(0);

fn unique_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "strop-robust-{}-{}-{tag}.strop",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ))
}

/// Per-position attribute coverage (newlines excluded — marks legitimately
/// don't cover the block separator). The honest span equality across a
/// persistence round trip, which may split/merge interval boundaries.
fn coverage(text: &str, spans: &SpanSet) -> Vec<Vec<InlineAttr>> {
    text.chars()
        .enumerate()
        .map(|(i, c)| {
            if c == '\n' {
                Vec::new()
            } else {
                let mut a: Vec<InlineAttr> = spans.attrs_at(i).cloned().collect();
                a.sort_by_key(|x| format!("{x:?}"));
                a
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 1. No untrusted input panics (pure functions — default case count).
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn from_markdown_never_panics(s in ".*") {
        let _ = from_markdown(&s);
    }

    #[test]
    fn typograph_process_never_panics(s in ".*", ru in any::<bool>()) {
        let lang = if ru { Lang::Ru } else { Lang::En };
        let _ = process(&s, None, lang);
    }

    #[test]
    fn diagnose_parse_never_panics(s in ".*") {
        let _ = parse_for(&s, "line");
    }

    #[test]
    fn anchor_never_panics(text in ".*", quote in ".*", after in 0usize..40) {
        let _ = anchor(&text, &quote, after);
    }

    #[test]
    fn import_image_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
        // Always Ok|Err, never a decoder unwind.
        let _ = import_image(bytes);
    }
}

// ---------------------------------------------------------------------------
// 2/Store. File-I/O properties — fewer cases to keep wall-clock small.
// ---------------------------------------------------------------------------

fn attr() -> impl Strategy<Value = InlineAttr> {
    prop_oneof![
        Just(InlineAttr::Strong),
        Just(InlineAttr::Emphasis),
        Just(InlineAttr::Underline),
        Just(InlineAttr::Highlight),
        Just(InlineAttr::Strikethrough),
        Just(InlineAttr::Code),
        Just(InlineAttr::Link("https://e.x".into())),
    ]
}

fn kind() -> impl Strategy<Value = BlockKind> {
    prop_oneof![
        Just(BlockKind::Paragraph),
        (1u8..=4).prop_map(BlockKind::Heading),
        Just(BlockKind::Blockquote),
        any::<bool>().prop_map(|o| BlockKind::ListItem { ordered: o, depth: 0 }),
        Just(BlockKind::CodeBlock { info: "rust".into() }),
        // Metadata with a ']' and an embedded newline — the exact shapes the
        // token format used to corrupt; JSON persistence must carry them.
        // (The caption is the block's own line now — inline-images §10 —
        // so alt carries the newline torture the field used to.)
        Just(BlockKind::Image {
            src: "asset:x.png".into(),
            alt: "a]b\nc".into(),
        }),
        Just(BlockKind::FootnoteDef { id: "1".into() }),
    ]
}

#[derive(Clone, Debug)]
enum Op {
    Insert(usize, String),
    Delete(usize, usize),
    Toggle(usize, usize, InlineAttr),
    SetKind(usize, BlockKind),
    Note(usize, usize, String),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0usize..16, "[a-zа-яё\n ]{0,4}").prop_map(|(p, s)| Op::Insert(p, s)),
        (0usize..16, 0usize..16).prop_map(|(a, b)| Op::Delete(a, b)),
        (0usize..16, 0usize..16, attr()).prop_map(|(a, b, t)| Op::Toggle(a, b, t)),
        (0usize..8, kind()).prop_map(|(b, k)| Op::SetKind(b, k)),
        (0usize..16, 0usize..16, "[a-я]{0,4}").prop_map(|(a, b, s)| Op::Note(a, b, s)),
    ]
}

fn clamp_range(a: usize, b: usize, len: usize) -> (usize, usize) {
    let (s, e) = (a.min(len), b.min(len));
    (s.min(e), s.max(e))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// Random arbitrary bytes written to a `.strop` must never crash `open()`.
    #[test]
    fn store_open_never_panics_on_arbitrary_bytes(
        bytes in proptest::collection::vec(any::<u8>(), 0..4096)
    ) {
        let path = unique_path("open-fuzz");
        std::fs::write(&path, &bytes).unwrap();
        let _ = Store::open(&path); // Ok|Err, never unwind
        let _ = std::fs::remove_file(&path);
    }

    /// THE persistence loop: build a document by a random op sequence, mirror
    /// it into the store exactly as the editor does, save full state, reopen,
    /// and assert text + span coverage + block kinds + notes all survive.
    #[test]
    fn full_state_survives_save_and_reopen(ops in proptest::collection::vec(op(), 0..24)) {
        let path = unique_path("state-roundtrip");
        let _ = std::fs::remove_file(&path);
        let (store, _) = Store::open(&path).unwrap();
        store.seed("");
        let mut doc = Document::new("", SpanSet::default(), BlockMap::default());

        for o in ops {
            let len = doc.rope().len_chars();
            match o {
                Op::Insert(p, s) => {
                    let p = p.min(len);
                    let b = doc.char_to_byte(p);
                    doc.edit_bytes(b..b, &s);
                }
                Op::Delete(a, b) => {
                    let (s, e) = clamp_range(a, b, len);
                    if s < e {
                        let (bs, be) = (doc.char_to_byte(s), doc.char_to_byte(e));
                        doc.edit_bytes(bs..be, "");
                    }
                }
                Op::Toggle(a, b, t) => {
                    let (s, e) = clamp_range(a, b, len);
                    if s < e {
                        doc.toggle_format(s..e, t);
                    }
                }
                Op::SetKind(blk, k) => {
                    let nb = doc.blocks().len();
                    if nb > 0 {
                        doc.set_block_kind(blk % nb, k);
                    }
                }
                Op::Note(a, b, body) => {
                    let (s, e) = clamp_range(a, b, len);
                    if s < e {
                        doc.add_note(s..e, body, 0);
                    }
                }
            }
            // Mirror to the store exactly as the editor's sync does.
            store.apply(&doc.take_ops());
            prop_assert_eq!(store.text(), doc.text(), "live mirror diverged");
        }

        store
            .save_with_state(doc.spans(), doc.blocks(), &doc.export_history(200), doc.notes(), &strop_core::journal::Journal::default(), doc.graveyard(), doc.provenance())
            .unwrap();

        let (_s2, loaded) = Store::open(&path).unwrap();
        let loaded = loaded.unwrap();
        prop_assert_eq!(&loaded.text, &doc.text(), "text changed across save/reopen");
        prop_assert_eq!(
            loaded.blocks.kinds(),
            doc.blocks().kinds(),
            "block kinds changed across save/reopen"
        );
        prop_assert_eq!(
            coverage(&loaded.text, &loaded.spans),
            coverage(&doc.text(), doc.spans()),
            "span coverage changed across save/reopen"
        );
        prop_assert_eq!(
            &loaded.annotations,
            doc.notes(),
            "notes changed across save/reopen"
        );
        let _ = std::fs::remove_file(&path);
    }
}

// ---------------------------------------------------------------------------
// 2b. The §2 wall law (docs/inline-images.md): furniture under arbitrary
//     edit sequences.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum WallOp {
    Insert(usize, String),
    Delete(usize, usize),
    Image(usize),
    Divider(usize),
    Undo,
}

fn wall_op() -> impl Strategy<Value = WallOp> {
    prop_oneof![
        3 => (0usize..32, "[ab\n ]{0,5}").prop_map(|(p, s)| WallOp::Insert(p, s)),
        3 => (0usize..32, 0usize..32).prop_map(|(a, b)| WallOp::Delete(a, b)),
        2 => (0usize..32).prop_map(WallOp::Image),
        1 => (0usize..32).prop_map(WallOp::Divider),
        1 => Just(WallOp::Undo),
    ]
}

fn is_break(c: char) -> bool {
    matches!(
        c,
        '\u{000A}' | '\u{000B}' | '\u{000C}' | '\u{000D}' | '\u{0085}' | '\u{2028}' | '\u{2029}'
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// The §2 wall law as invariants (spec §11): after ANY edit sequence —
    /// with the caption path never taken — (a) the block map stays
    /// line-aligned, (b) no picture is ever cloned (every src born unique
    /// stays unique), and (c) no edit leaves prose on a furniture block's
    /// line the writer didn't put in its caption: this sequence types no
    /// captions, so furniture lines must stay textless forever.
    #[test]
    fn furniture_survives_arbitrary_edit_sequences(
        ops in proptest::collection::vec(wall_op(), 0..32)
    ) {
        let mut doc = Document::new("alpha\nbeta", SpanSet::default(), BlockMap::default());
        let mut next_src = 0u32;
        for o in ops {
            let len = doc.rope().len_chars();
            match o {
                WallOp::Insert(p, s) => {
                    let p = p.min(len);
                    // The caption path is the ONLY legal writer of text on
                    // a furniture line; this sequence never takes it.
                    if !doc.blocks().kind(doc.rope().char_to_line(p)).is_furniture() {
                        let b = doc.char_to_byte(p);
                        doc.edit_bytes(b..b, &s);
                    }
                }
                WallOp::Delete(a, b) => {
                    let (s, e) = clamp_range(a, b, len);
                    if s < e {
                        let (bs, be) = (doc.char_to_byte(s), doc.char_to_byte(e));
                        doc.edit_bytes(bs..be, "");
                    }
                }
                WallOp::Image(p) => {
                    let b = doc.char_to_byte(p.min(len));
                    next_src += 1;
                    doc.insert_image_block(b, format!("asset:{next_src}"));
                }
                WallOp::Divider(p) => {
                    // Furniture is never minted under prose: stamp only a
                    // textless Paragraph, like the editor's divider verb.
                    let line = doc.rope().char_to_line(p.min(len));
                    if doc.blocks().kind(line) == &BlockKind::Paragraph
                        && doc.rope().line(line).chars().all(is_break)
                    {
                        doc.set_block_kind(line, BlockKind::Divider);
                    }
                }
                WallOp::Undo => {
                    doc.undo();
                }
            }
            // (a) kinds stay aligned with the text's lines across every
            // splice, clamped or not.
            prop_assert_eq!(
                doc.blocks().len(),
                doc.rope().len_lines(),
                "block map fell out of line alignment"
            );
            // (b) no clone: every Image src stays unique.
            let mut srcs: Vec<&str> = doc.blocks().asset_refs().collect();
            let born = srcs.len();
            srcs.sort();
            srcs.dedup();
            prop_assert_eq!(srcs.len(), born, "an edit cloned a picture");
            // (c) furniture lines stay textless: no merge ever moved prose
            // onto a picture's or divider's line.
            for (i, k) in doc.blocks().kinds().iter().enumerate() {
                if k.is_furniture() {
                    prop_assert!(
                        doc.rope().line(i).chars().all(is_break),
                        "prose flowed onto a furniture line: {:?}",
                        doc.text()
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 3. SpanSet algebraic invariants.
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn spanset_stays_sorted_nonempty_and_self_covering(
        ops in proptest::collection::vec((any::<bool>(), 0usize..20, 0usize..20, attr()), 0..40)
    ) {
        let mut set = SpanSet::default();
        for (add, a, b, at) in ops {
            let (s, e): (usize, usize) = (a.min(b), a.max(b));
            if add {
                set.add(s..e, at);
            } else {
                set.remove(s..e, &at);
            }
            // Sorted by start, and never an empty/inverted interval.
            for w in set.spans().windows(2) {
                prop_assert!(w[0].range.start <= w[1].range.start, "spans unsorted: {:?}", set.spans());
            }
            for sp in set.spans() {
                prop_assert!(sp.range.start < sp.range.end, "empty span: {:?}", sp);
            }
        }
        // Every span fully covers its own range under the same attr.
        for sp in set.spans() {
            prop_assert!(
                set.covers(sp.range.clone(), &sp.attr),
                "span does not cover itself: {:?}",
                sp
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 4. diff block reconstruction is byte-exact.
// ---------------------------------------------------------------------------

fn par_list() -> impl Strategy<Value = String> {
    proptest::collection::vec("[a-zа-я ]{0,6}", 1..5).prop_map(|v| v.join("\n"))
}

proptest! {
    #[test]
    fn diff_blocks_reconstruct_both_sources_byte_exact(old in par_list(), new in par_list()) {
        let blocks = prose_diff_blocks(&old, &new);
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
                prop_assert_eq!(&from_old, &old_pars[p].to_string());
            }
            if let Some(p) = b.new_par {
                prop_assert_eq!(&from_new, &new_pars[p].to_string());
            }
        }
    }
}
