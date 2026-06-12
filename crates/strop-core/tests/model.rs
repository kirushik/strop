//! Property suite for the document engine (PLAN.md Phase G).
//!
//! Three parts:
//! 1. A state-machine test driving `Document` (with a live `Store` mirror)
//!    against a deliberately dumb reference model — String + undo/redo
//!    stacks in CHAR coordinates. Invariants checked after every
//!    transition; undo/redo exactness checked against snapshots taken
//!    from the SUT itself.
//! 2. A markdown round-trip property over generated block/span models,
//!    restricted to the representable subset (markdown is deliberately
//!    lossy: empty blocks, list depth, formatting inside code spans and
//!    block-leading marker characters do not survive — those gaps are
//!    documented here, not silently shrunk away).
//! 3. Typograph properties: substitutions are byte-safe (span lands on a
//!    char boundary — Cyrillic/emoji input is the bug surface), change
//!    something, and re-running on the result quiesces.
//!
//! Case count: proptest's default 256, override with PROPTEST_CASES.

use std::ops::Range;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use proptest::prelude::*;
use proptest_state_machine::{ReferenceStateMachine, StateMachineTest, prop_state_machine};

use strop_core::Store;
use strop_core::document::{BlockKind, BlockMap, Document, InlineAttr, SpanSet};
use strop_core::markdown::{from_markdown, to_markdown};
use strop_core::typograph::{Lang, process};

// ---------------------------------------------------------------------------
// Part 1: Document + Store state machine
// ---------------------------------------------------------------------------

/// Small text fragments: Latin, Cyrillic, emoji, spaces, newlines — multi-
/// byte chars are exactly where char/byte confusion would bite. Block-
/// marker characters (#, -, digits at line start…) are deliberately absent
/// so the in-flight markdown check below stays within the representable
/// subset.
fn small_text() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            5 => proptest::char::range('a', 'z'),
            4 => proptest::char::range('а', 'я'),
            1 => Just('ё'),
            1 => prop_oneof![Just('🦀'), Just('💜'), Just('✍')],
            1 => Just(' '),
            1 => Just('\n'),
        ],
        1..6,
    )
    .prop_map(|v| v.into_iter().collect())
}

fn toggle_attr() -> impl Strategy<Value = InlineAttr> {
    // Code and Link are excluded HERE only because markdown cannot express
    // other formatting overlapping a code span / nested links; they get
    // full coverage in the round-trip property below (disjoint spans).
    prop_oneof![
        Just(InlineAttr::Strong),
        Just(InlineAttr::Emphasis),
        Just(InlineAttr::Strikethrough),
        Just(InlineAttr::Underline),
        Just(InlineAttr::Highlight),
    ]
}

fn block_kind() -> impl Strategy<Value = BlockKind> {
    // Depth is fixed at 0: to_markdown flattens list depth (known export
    // gap — nested lists are not yet emitted with indentation).
    prop_oneof![
        Just(BlockKind::Paragraph),
        (1u8..=3).prop_map(BlockKind::Heading),
        Just(BlockKind::Blockquote),
        any::<bool>().prop_map(|ordered| BlockKind::ListItem { ordered, depth: 0 }),
        Just(BlockKind::CodeBlock { info: String::new() }),
    ]
}

#[derive(Clone, Debug)]
enum Transition {
    InsertChars { pos: usize, s: String },
    DeleteRange { start: usize, end: usize },
    ToggleSpan { start: usize, end: usize, attr: InlineAttr },
    SetBlockKind { block: usize, kind: BlockKind },
    Undo,
    Redo,
    /// Check-only: serialize → parse → compare, when the current document
    /// is inside markdown's representable subset.
    MarkdownRoundtrip,
}

/// The reference: a String in char coordinates plus undo/redo stacks of
/// whole texts. Format-only transitions snapshot too — Document gives
/// every ToggleSpan/SetBlockKind its own transaction.
#[derive(Clone, Debug, Default)]
struct RefModel {
    text: String,
    undo: Vec<String>,
    redo: Vec<String>,
}

impl RefModel {
    fn len_chars(&self) -> usize {
        self.text.chars().count()
    }

    fn lines(&self) -> usize {
        self.text.split('\n').count()
    }

    fn char_to_byte(&self, ch: usize) -> usize {
        self.text
            .char_indices()
            .nth(ch)
            .map(|(b, _)| b)
            .unwrap_or(self.text.len())
    }
}

struct RefMachine;

impl ReferenceStateMachine for RefMachine {
    type State = RefModel;
    type Transition = Transition;

    fn init_state() -> BoxedStrategy<Self::State> {
        Just(RefModel::default()).boxed()
    }

    fn transitions(state: &Self::State) -> BoxedStrategy<Self::Transition> {
        let len = state.len_chars();
        let lines = state.lines();
        prop_oneof![
            3 => (0..=len, small_text()).prop_map(|(pos, s)| Transition::InsertChars { pos, s }),
            2 => (0..=len, 0..=len).prop_map(|(a, b)| Transition::DeleteRange {
                start: a.min(b),
                end: a.max(b),
            }),
            2 => (0..=len, 0..=len, toggle_attr()).prop_map(|(a, b, attr)| {
                Transition::ToggleSpan { start: a.min(b), end: a.max(b), attr }
            }),
            2 => (0..lines, block_kind())
                .prop_map(|(block, kind)| Transition::SetBlockKind { block, kind }),
            1 => Just(Transition::Undo),
            1 => Just(Transition::Redo),
            1 => Just(Transition::MarkdownRoundtrip),
        ]
        .boxed()
    }

    fn preconditions(state: &Self::State, transition: &Self::Transition) -> bool {
        // Re-checked after shrinking removes earlier transitions, so every
        // position must still be valid against the CURRENT state.
        let len = state.len_chars();
        match transition {
            Transition::InsertChars { pos, s } => *pos <= len && !s.is_empty(),
            Transition::DeleteRange { start, end } => start < end && *end <= len,
            Transition::ToggleSpan { start, end, .. } => start < end && *end <= len,
            Transition::SetBlockKind { block, .. } => *block < state.lines(),
            _ => true,
        }
    }

    fn apply(mut state: Self::State, transition: &Self::Transition) -> Self::State {
        match transition {
            Transition::InsertChars { pos, s } => {
                state.undo.push(state.text.clone());
                state.redo.clear();
                let b = state.char_to_byte(*pos);
                state.text.insert_str(b, s);
            }
            Transition::DeleteRange { start, end } => {
                state.undo.push(state.text.clone());
                state.redo.clear();
                let (bs, be) = (state.char_to_byte(*start), state.char_to_byte(*end));
                state.text.replace_range(bs..be, "");
            }
            Transition::ToggleSpan { .. } | Transition::SetBlockKind { .. } => {
                // Format-only transaction: text unchanged, undo point taken.
                state.undo.push(state.text.clone());
                state.redo.clear();
            }
            Transition::Undo => {
                if let Some(prev) = state.undo.pop() {
                    state.redo.push(std::mem::replace(&mut state.text, prev));
                }
            }
            Transition::Redo => {
                if let Some(next) = state.redo.pop() {
                    state.undo.push(std::mem::replace(&mut state.text, next));
                }
            }
            Transition::MarkdownRoundtrip => {}
        }
        state
    }
}

/// SUT: the real Document plus a live Store mirror (apply(take_ops) after
/// every transition, like the editor's sync_mutations), plus shadow
/// undo/redo stacks of SUT-observed (text, spans, blocks) triples — the
/// oracle for "undo restores exactly what was there".
struct DocSut {
    doc: Document,
    store: Store,
    store_path: PathBuf,
    shadow_undo: Vec<(String, SpanSet, BlockMap)>,
    shadow_redo: Vec<(String, SpanSet, BlockMap)>,
}

static STORE_SEQ: AtomicU64 = AtomicU64::new(0);

fn triple(doc: &Document) -> (String, SpanSet, BlockMap) {
    (doc.text(), doc.spans().clone(), doc.blocks().clone())
}

/// Star/tilde delimiters obey CommonMark flanking rules; `<u>`/`<mark>`
/// tags, backticks and brackets do not care.
fn is_delimiter_attr(attr: &InlineAttr) -> bool {
    matches!(
        attr,
        InlineAttr::Strong | InlineAttr::Emphasis | InlineAttr::Strikethrough
    )
}

/// In our generated alphabets the only non-word characters are spaces and
/// emoji; both can put a `**`/`~~` marker in a position CommonMark refuses
/// to parse (a closer between a symbol and a letter is not right-flanking;
/// a marker against whitespace does not flank at all). Sufficient check
/// for the generators' alphabets — not a general CommonMark model.
fn delimiter_flanking_ok(chars: &[char], range: &Range<usize>) -> bool {
    let symbolish = |c: char| !c.is_alphanumeric() && !c.is_whitespace();
    let first = chars[range.start];
    let last = chars[range.end - 1];
    if first.is_whitespace() || last.is_whitespace() {
        return false;
    }
    let before = range.start.checked_sub(1).map(|i| chars[i]);
    let after = chars.get(range.end);
    if symbolish(first) && before.is_some_and(|c| c.is_alphanumeric()) {
        return false; // opener not left-flanking
    }
    if symbolish(last) && after.is_some_and(|c| c.is_alphanumeric()) {
        return false; // closer not right-flanking
    }
    true
}

/// Is the document inside markdown's representable subset? Markdown is
/// deliberately lossy outside it: empty blocks vanish, paragraph edge
/// whitespace is trimmed, code-block lines are raw (formatting spans over
/// them do not survive), and star/tilde delimiters obey flanking rules.
/// (Block-marker chars never occur — see `small_text`.)
fn md_representable(doc: &Document) -> bool {
    let text = doc.text();
    let lines_ok = text.split('\n').all(|line| {
        !line.is_empty() && !line.starts_with(' ') && !line.ends_with(' ')
    });
    if !lines_ok {
        return false;
    }
    // Char ranges of code-block lines.
    let mut code_ranges: Vec<Range<usize>> = Vec::new();
    let mut base = 0usize;
    for (ix, line) in text.split('\n').enumerate() {
        let len = line.chars().count();
        if matches!(doc.blocks().kind(ix), BlockKind::CodeBlock { .. }) {
            code_ranges.push(base..base + len);
        }
        base += len + 1;
    }
    let chars: Vec<char> = text.chars().collect();
    let spans = doc.spans().spans();
    // Two star-class spans (Strong/Emphasis both mark with '*') whose
    // markers touch — coincident or adjacent boundaries, or a partial
    // overlap's close+reopen point — merge into one delimiter run and
    // parse under CommonMark's run-length rules ("***m*****💜**"), not
    // as the intended nesting. Representable: strictly nested or
    // separated by at least one character.
    let star = |a: &InlineAttr| matches!(a, InlineAttr::Strong | InlineAttr::Emphasis);
    let star_pairs_ok = spans.iter().enumerate().all(|(i, a)| {
        !star(&a.attr)
            || spans.iter().enumerate().all(|(j, b)| {
                i == j || !star(&b.attr) || {
                    let (x, y) = (&a.range, &b.range);
                    y.end < x.start
                        || y.start > x.end
                        || (y.start > x.start && y.end < x.end)
                        || (x.start > y.start && x.end < y.end)
                }
            })
    });
    if !star_pairs_ok {
        return false;
    }
    spans.iter().enumerate().all(|(ix, s)| {
        let off_code = code_ranges
            .iter()
            .all(|c| c.start >= s.range.end || s.range.start >= c.end);
        // Spans crossing a newline export per line; flanking applies to
        // each per-line piece. Checking the piece edges keeps this exact
        // enough for our alphabets.
        let flanking = !is_delimiter_attr(&s.attr) || {
            split_at_newlines(&chars, &s.range)
                .iter()
                .all(|piece| delimiter_flanking_ok(&chars, piece))
                && delimiter_stacking_ok(spans, ix, &chars)
        };
        off_code && flanking
    })
}

/// When several spans share a boundary (or one straddles another's end),
/// their markers stack ("==**", "~~*"); the OUTERMOST marker then borders
/// the surrounding text, and CommonMark refuses an opener/closer whose
/// outer neighbor is alphanumeric while its inner neighbor is punctuation
/// (the other markers). Intermediate close/reopen points inside a span
/// are always punctuation-adjacent on both sides and thus safe.
fn delimiter_stacking_ok(
    spans: &[strop_core::document::Span],
    me: usize,
    chars: &[char],
) -> bool {
    let s = &spans[me].range;
    let stack_at_start = spans
        .iter()
        .enumerate()
        .any(|(i, o)| i != me && o.range.start == s.start);
    let stack_at_end = spans.iter().enumerate().any(|(i, o)| {
        i != me
            && (o.range.end == s.end
                || (o.range.start > s.start && o.range.start < s.end && o.range.end > s.end))
    });
    let before = s.start.checked_sub(1).map(|i| chars[i]);
    let after = chars.get(s.end);
    if stack_at_start && before.is_some_and(|c| c.is_alphanumeric() && c != '\n') {
        return false;
    }
    if stack_at_end && after.is_some_and(|c| c.is_alphanumeric() && *c != '\n') {
        return false;
    }
    // Every other-span boundary strictly inside this span is a potential
    // close+reopen point for our markers; the closer there must not be
    // preceded by whitespace nor the reopener followed by it ("*uё*~~* ш*"
    // — the reopened '*' before a space is not left-flanking).
    let inner_points = spans.iter().enumerate().flat_map(|(i, o)| {
        let mut v = Vec::new();
        if i != me {
            v.push(o.range.start);
            v.push(o.range.end);
        }
        v
    });
    for p in inner_points {
        if p > s.start
            && p < s.end
            && (chars[p - 1].is_whitespace() || chars[p].is_whitespace())
        {
            return false;
        }
    }
    true
}

/// Split a char range at newline positions (markdown reopens spans per
/// block); pieces are the per-block marker extents.
fn split_at_newlines(chars: &[char], range: &Range<usize>) -> Vec<Range<usize>> {
    let mut pieces = Vec::new();
    let mut start = range.start;
    for i in range.clone() {
        if chars[i] == '\n' {
            if start < i {
                pieces.push(start..i);
            }
            start = i + 1;
        }
    }
    if start < range.end {
        pieces.push(start..range.end);
    }
    pieces
}

/// Per-position attribute coverage, the honest span equality: a round
/// trip may split/merge interval boundaries while covering the same
/// characters. Newline positions are excluded — markdown closes inline
/// spans at block ends and reopens them, so the separator itself is
/// legitimately uncovered after a round trip.
fn coverage(text: &str, spans: &SpanSet) -> Vec<Vec<InlineAttr>> {
    text.chars()
        .enumerate()
        .map(|(i, c)| {
            if c == '\n' {
                Vec::new()
            } else {
                let mut attrs: Vec<InlineAttr> = spans.attrs_at(i).cloned().collect();
                attrs.sort_by_key(|a| format!("{a:?}"));
                attrs
            }
        })
        .collect()
}

struct DocTest;

impl StateMachineTest for DocTest {
    type SystemUnderTest = DocSut;
    type Reference = RefMachine;

    fn init_test(
        _ref_state: &<Self::Reference as ReferenceStateMachine>::State,
    ) -> Self::SystemUnderTest {
        let path = std::env::temp_dir().join(format!(
            "strop-model-{}-{}.strop",
            std::process::id(),
            STORE_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_file(&path);
        let (store, _) = Store::open(&path).expect("temp store");
        store.seed("");
        DocSut {
            doc: Document::new("", SpanSet::default(), BlockMap::default()),
            store,
            store_path: path,
            shadow_undo: Vec::new(),
            shadow_redo: Vec::new(),
        }
    }

    fn apply(
        mut sut: Self::SystemUnderTest,
        _ref_state: &<Self::Reference as ReferenceStateMachine>::State,
        transition: Transition,
    ) -> Self::SystemUnderTest {
        match &transition {
            Transition::InsertChars { pos, s } => {
                sut.shadow_undo.push(triple(&sut.doc));
                sut.shadow_redo.clear();
                let b = sut.doc.char_to_byte(*pos);
                sut.doc.edit_bytes(b..b, s);
            }
            Transition::DeleteRange { start, end } => {
                sut.shadow_undo.push(triple(&sut.doc));
                sut.shadow_redo.clear();
                let (bs, be) = (sut.doc.char_to_byte(*start), sut.doc.char_to_byte(*end));
                sut.doc.edit_bytes(bs..be, "");
            }
            Transition::ToggleSpan { start, end, attr } => {
                sut.shadow_undo.push(triple(&sut.doc));
                sut.shadow_redo.clear();
                sut.doc.toggle_format(*start..*end, attr.clone());
            }
            Transition::SetBlockKind { block, kind } => {
                sut.shadow_undo.push(triple(&sut.doc));
                sut.shadow_redo.clear();
                sut.doc.set_block_kind(*block, kind.clone());
            }
            Transition::Undo => {
                let expected = sut.shadow_undo.pop();
                let before = triple(&sut.doc);
                let result = sut.doc.undo();
                match expected {
                    Some(exp) => {
                        assert!(result.is_some(), "undo refused with history present");
                        assert_eq!(triple(&sut.doc), exp, "undo restored a different state");
                        sut.shadow_redo.push(before);
                    }
                    None => assert!(
                        result.is_none(),
                        "undo succeeded with no history in the model"
                    ),
                }
            }
            Transition::Redo => {
                let expected = sut.shadow_redo.pop();
                let before = triple(&sut.doc);
                let result = sut.doc.redo();
                match expected {
                    Some(exp) => {
                        assert!(result.is_some(), "redo refused with redo history present");
                        assert_eq!(triple(&sut.doc), exp, "redo restored a different state");
                        sut.shadow_undo.push(before);
                    }
                    None => assert!(
                        result.is_none(),
                        "redo succeeded with no redo history in the model"
                    ),
                }
            }
            Transition::MarkdownRoundtrip => {
                if md_representable(&sut.doc) {
                    let text = sut.doc.text();
                    let md = to_markdown(&text, sut.doc.spans(), sut.doc.blocks());
                    let (text2, spans2, blocks2) = from_markdown(&md);
                    assert_eq!(text2, text, "markdown round-trip changed the text\nmd: {md:?}");
                    assert_eq!(
                        blocks2.kinds(),
                        sut.doc.blocks().kinds(),
                        "markdown round-trip changed block kinds\nmd: {md:?}"
                    );
                    assert_eq!(
                        coverage(&text2, &spans2),
                        coverage(&text, sut.doc.spans()),
                        "markdown round-trip changed span coverage\nmd: {md:?}"
                    );
                }
            }
        }

        // The store mirror, exactly as the editor wires it: drain the ops
        // this transition produced and apply them to Loro; the two text
        // streams must never diverge (undo/redo arrive as ordinary ops).
        sut.store.apply(&sut.doc.take_ops());
        assert_eq!(
            sut.store.text(),
            sut.doc.text(),
            "store mirror diverged from the buffer"
        );
        sut
    }

    fn check_invariants(
        sut: &Self::SystemUnderTest,
        ref_state: &<Self::Reference as ReferenceStateMachine>::State,
    ) {
        // 1. The rope IS the model string.
        assert_eq!(sut.doc.text(), ref_state.text, "rope diverged from the model");

        // 2. Spans sorted by start and inside the text (char coords).
        let len_chars = sut.doc.rope().len_chars();
        let spans = sut.doc.spans().spans();
        for pair in spans.windows(2) {
            assert!(
                pair[0].range.start <= pair[1].range.start,
                "spans out of order: {pair:?}"
            );
        }
        for s in spans {
            assert!(
                s.range.start < s.range.end && s.range.end <= len_chars,
                "span out of bounds: {:?} (len {len_chars})",
                s.range
            );
        }

        // 3. Block kinds aligned with the text's lines, always.
        assert_eq!(
            sut.doc.blocks().kinds().len(),
            sut.doc.rope().len_lines(),
            "block map misaligned with lines"
        );
    }

    fn teardown(
        sut: Self::SystemUnderTest,
        _ref_state: <Self::Reference as ReferenceStateMachine>::State,
    ) {
        let _ = std::fs::remove_file(&sut.store_path);
    }
}

prop_state_machine! {
    #[test]
    fn document_state_machine(sequential 1..24 => DocTest);
}

// ---------------------------------------------------------------------------
// Part 2: markdown round-trip over generated block/span models
// ---------------------------------------------------------------------------

/// A line of "prose" inside markdown's representable subset: starts and
/// ends with a word character, single spaces inside. Inline-escape
/// characters are exercised separately (escape_roundtrip) because
/// markdown cannot express them inside code spans.
fn md_line() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            6 => proptest::char::range('a', 'z'),
            4 => proptest::char::range('а', 'я'),
            1 => proptest::char::range('0', '9'),
            1 => Just('🦀'),
        ],
        1..8,
    )
    .prop_map(|v| v.into_iter().collect())
}

fn md_block() -> impl Strategy<Value = (BlockKind, String)> {
    prop_oneof![
        3 => md_line().prop_map(|l| (BlockKind::Paragraph, l)),
        1 => ((1u8..=3), md_line()).prop_map(|(n, l)| (BlockKind::Heading(n), l)),
        1 => md_line().prop_map(|l| (BlockKind::Blockquote, l)),
        1 => (any::<bool>(), md_line())
            .prop_map(|(o, l)| (BlockKind::ListItem { ordered: o, depth: 0 }, l)),
        1 => md_line().prop_map(|l| (BlockKind::CodeBlock { info: String::new() }, l)),
        1 => Just((BlockKind::Divider, String::new())),
    ]
}

fn span_attr() -> impl Strategy<Value = InlineAttr> {
    prop_oneof![
        Just(InlineAttr::Strong),
        Just(InlineAttr::Emphasis),
        Just(InlineAttr::Strikethrough),
        Just(InlineAttr::Underline),
        Just(InlineAttr::Highlight),
        Just(InlineAttr::Code),
        Just(InlineAttr::Link("https://e.example/x".into())),
    ]
}

proptest! {
    /// Generated block/span models survive to_markdown → from_markdown.
    /// Spans are disjoint and confined to a single non-code, non-divider
    /// line each (markdown cannot express overlap with code, spans over
    /// raw code lines, or marks on a thematic break).
    #[test]
    fn markdown_roundtrip(
        blocks_model in proptest::collection::vec(md_block(), 1..6),
        span_seeds in proptest::collection::vec((0usize..6, span_attr(), any::<u16>(), any::<u16>()), 0..3),
    ) {
        let text = blocks_model
            .iter()
            .map(|(_, l)| l.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let kinds: Vec<BlockKind> = blocks_model.iter().map(|(k, _)| k.clone()).collect();
        let blocks = BlockMap::from_kinds(kinds.clone());

        // Place each span inside one eligible line, in char coords.
        let mut spans = SpanSet::default();
        let mut used: Vec<Range<usize>> = Vec::new();
        for (block_pick, attr, a, b) in span_seeds {
            let ix = block_pick % blocks_model.len();
            if matches!(
                blocks_model[ix].0,
                BlockKind::CodeBlock { .. } | BlockKind::Divider
            ) {
                continue;
            }
            let base: usize = blocks_model[..ix]
                .iter()
                .map(|(_, l)| l.chars().count() + 1)
                .sum();
            let line_len = blocks_model[ix].1.chars().count();
            if line_len == 0 {
                continue;
            }
            let (mut s, mut e) = ((a as usize) % line_len, (b as usize) % line_len + 1);
            if s >= e {
                std::mem::swap(&mut s, &mut e);
                e += 1;
            }
            let range = base + s..base + e.min(line_len);
            // Disjoint with a one-char gap: touching star-class markers
            // would merge into one CommonMark delimiter run.
            if range.start >= range.end
                || used.iter().any(|u| u.start <= range.end && range.start <= u.end)
            {
                continue;
            }
            // Star/tilde markers must land in flanking-valid positions
            // (emoji edges break CommonMark emphasis parsing).
            let chars: Vec<char> = text.chars().collect();
            if is_delimiter_attr(&attr) && !delimiter_flanking_ok(&chars, &range) {
                continue;
            }
            used.push(range.clone());
            spans.add(range, attr);
        }

        let md = to_markdown(&text, &spans, &blocks);
        let (text2, spans2, blocks2) = from_markdown(&md);
        prop_assert_eq!(&text2, &text, "text changed; md: {:?}", md);
        prop_assert_eq!(blocks2.kinds(), blocks.kinds(), "kinds changed; md: {:?}", md);
        prop_assert_eq!(
            coverage(&text2, &spans2),
            coverage(&text, &spans),
            "span coverage changed; md: {:?}", md
        );
    }

    /// Inline escaping: paragraphs containing markdown-active characters
    /// come back verbatim (no spans — escapes inside code spans are a
    /// known, documented gap).
    #[test]
    fn escape_roundtrip(
        line in proptest::collection::vec(
            prop_oneof![
                4 => proptest::char::range('a', 'z'),
                2 => proptest::char::range('а', 'я'),
                2 => prop_oneof![
                    Just('*'), Just('_'), Just('`'), Just('['), Just(']'),
                    Just('~'), Just('\\')
                ],
            ],
            1..10,
        ).prop_map(|v| v.into_iter().collect::<String>())
         .prop_filter("representable", |l| !l.starts_with(' ') && !l.ends_with(' '))
    ) {
        let blocks = BlockMap::new(1);
        let md = to_markdown(&line, &SpanSet::default(), &blocks);
        let (text2, _, _) = from_markdown(&md);
        prop_assert_eq!(&text2, &line, "escape round-trip changed text; md: {:?}", md);
    }
}

// ---------------------------------------------------------------------------
// Part 3: typograph properties
// ---------------------------------------------------------------------------

fn typo_prefix() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            4 => proptest::char::range('a', 'z'),
            3 => proptest::char::range('а', 'я'),
            2 => Just(' '),
            2 => Just('.'),
            2 => Just('-'),
            2 => Just('"'),
            1 => Just('\''),
            1 => Just('«'),
            1 => Just('»'),
            1 => Just('…'),
            1 => Just('—'),
            1 => Just('🦀'),
            1 => Just('('),
        ],
        1..12,
    )
    .prop_map(|v| v.into_iter().collect())
}

fn apply_sub(prefix: &str, sub: &strop_core::typograph::Substitution) -> String {
    let cut = prefix.len() - sub.span;
    assert!(
        prefix.is_char_boundary(cut),
        "substitution span {} is not on a char boundary of {prefix:?}",
        sub.span
    );
    format!("{}{}", &prefix[..cut], sub.text)
}

proptest! {
    /// Substitutions are byte-safe and quiesce: the span never exceeds the
    /// prefix, always lands on a char boundary (Cyrillic/emoji!), actually
    /// changes the text, and repeated application reaches a fixed point
    /// within a few steps (no substitution loops).
    #[test]
    fn typograph_quiesces(prefix in typo_prefix(), ru in any::<bool>()) {
        let lang = if ru { Lang::Ru } else { Lang::En };
        let mut current = prefix;
        for _ in 0..4 {
            match process(&current, lang) {
                None => return Ok(()),
                Some(sub) => {
                    prop_assert!(
                        sub.span <= current.len(),
                        "span {} exceeds prefix {:?}", sub.span, current
                    );
                    let next = apply_sub(&current, &sub);
                    prop_assert_ne!(
                        &next, &current,
                        "no-op substitution {:?} on {:?}", sub, current
                    );
                    current = next;
                }
            }
        }
        prop_assert!(
            process(&current, lang).is_none(),
            "typograph did not quiesce after 4 substitutions: {:?}", current
        );
    }
}
