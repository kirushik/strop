//! Rich-text schema types and the span/anchor machinery.
//!
//! See docs/document-model.md. Spans are char-indexed ranges over the same
//! text stream the rope and Loro share; `SpanSet::apply_op` keeps them
//! consistent across every edit (including undo/redo, which arrive as
//! ordinary ops). The same adjustment math will anchor annotations.

use std::ops::{Deref, DerefMut, Range};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::buffer::{Buffer, TextOp, Transaction};

/// An immutable side-state version shared by the live document and its undo
/// frames. Mutation is copy-on-write, while serde deliberately delegates to
/// `T` so persisted History keeps its existing tuple/object wire shape.
#[derive(Debug, Clone, Default)]
struct Shared<T>(Arc<T>);

impl<T> From<T> for Shared<T> {
    fn from(value: T) -> Self { Self(Arc::new(value)) }
}

impl<T> Deref for Shared<T> {
    type Target = T;
    fn deref(&self) -> &T { &self.0 }
}

impl<T: Clone> DerefMut for Shared<T> {
    fn deref_mut(&mut self) -> &mut T { Arc::make_mut(&mut self.0) }
}

impl<T: Serialize> Serialize for Shared<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (**self).serialize(serializer)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Shared<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        T::deserialize(deserializer).map(Self::from)
    }
}

/// Count line breaks in `text` using ropey's Unicode line-break set, so a
/// block-map split count always agrees with `Rope::len_lines()`: CRLF counts
/// as one break, and CR / VT / FF / NEL / U+2028 / U+2029 all count — unlike a
/// plain '\n' scan, which a paste of classic-Mac or PDF-copied text defeats.
pub(crate) fn count_line_breaks(text: &str) -> usize {
    // Fast path for the hot edit case (typing non-break chars): only build a
    // throwaway Rope — the price of ropey's exact CRLF-as-one / CR / VT / FF /
    // NEL / U+2028 / U+2029 counting — when a line-break char is actually
    // present. The common keystroke inserts none and returns 0 immediately.
    if !text.contains(|c: char| {
        matches!(
            c,
            '\u{000A}'
                | '\u{000B}'
                | '\u{000C}'
                | '\u{000D}'
                | '\u{0085}'
                | '\u{2028}'
                | '\u{2029}'
        )
    }) {
        return 0;
    }
    ropey::Rope::from_str(text).len_lines() - 1
}

/// The substring of `text` spanning char positions `start..end` (clamped by
/// the caller). Used to capture the passage a note covers before a wholesale
/// restore so it can be re-located by content.
fn char_slice(text: &str, start: usize, end: usize) -> String {
    text.chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
}

/// Length in chars of the line break ENDING exactly at `pos`: two for CRLF,
/// one for every other break ropey recognises, and zero when none ends here.
fn break_len_before(rope: &ropey::Rope, pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    match rope.char(pos - 1) {
        '\n' if pos >= 2 && rope.char(pos - 2) == '\r' => 2,
        '\n' | '\u{000B}' | '\u{000C}' | '\r' | '\u{0085}' | '\u{2028}'
        | '\u{2029}' => 1,
        _ => 0,
    }
}

/// Length in bytes of the line break ENDING exactly at `byte_pos`: two for
/// CRLF and NEL, three for U+2028 / U+2029, one for the ASCII single-char
/// breaks, and zero when none ends here. `byte_pos` is a char boundary.
pub fn break_len_before_bytes(rope: &ropey::Rope, byte_pos: usize) -> usize {
    let pos = rope.byte_to_char(byte_pos);
    let break_chars = break_len_before(rope, pos);
    byte_pos - rope.char_to_byte(pos - break_chars)
}

/// Byte position where `line`'s own text ends, excluding the line break
/// ropey assigns to it. The final line ends at the rope's byte length.
pub fn line_text_end_bytes(rope: &ropey::Rope, line: usize) -> usize {
    if line + 1 < rope.len_lines() {
        let next = rope.line_to_byte(line + 1);
        next - break_len_before_bytes(rope, next)
    } else {
        rope.len_bytes()
    }
}

/// Length in chars of the line break STARTING exactly at `pos`: two for CRLF,
/// one for every other break ropey recognises, and zero when none starts here.
fn break_len_at(rope: &ropey::Rope, pos: usize) -> usize {
    if pos >= rope.len_chars() {
        return 0;
    }
    match rope.char(pos) {
        '\r' if pos + 1 < rope.len_chars() && rope.char(pos + 1) == '\n' => 2,
        '\n' | '\u{000B}' | '\u{000C}' | '\r' | '\u{0085}' | '\u{2028}'
        | '\u{2029}' => 1,
        _ => 0,
    }
}

/// The char position where the line break PRECEDING `line_start` begins —
/// i.e. `line_start` minus its own leading break (one char, or two for CRLF,
/// which ropey counts as one break but two chars). A position without a
/// preceding break stays where it is.
fn line_break_before(rope: &ropey::Rope, line_start: usize) -> usize {
    line_start - break_len_before(rope, line_start)
}

/// Char range of the manuscript region of `rope` under `blocks`' boundary,
/// ERA-AWARE — the one source every scope consumer branches through
/// (adjudications, "the foundation"; scopes-search 4). Tail era: everything
/// BEFORE the scrap line, excluding the line break that joins the last
/// manuscript line to the seam (so the slice's block invariant holds and a
/// type-over of a manuscript select-all can never eat the seam). Top era:
/// everything after the separator line. No boundary: the whole text. Free of
/// `Document` so replay/strip states can rebase their own counts against
/// their own boundary (scopes-search 6).
pub fn manuscript_range_of(rope: &ropey::Rope, blocks: &BlockMap) -> Range<usize> {
    match blocks.boundary() {
        Some((BoundaryEra::Top, b)) => {
            let line = (b + 1).min(rope.len_lines());
            rope.line_to_char(line)..rope.len_chars()
        }
        Some((BoundaryEra::Tail, b)) => {
            let line = b.min(rope.len_lines().saturating_sub(1));
            0..line_break_before(rope, rope.line_to_char(line))
        }
        None => 0..rope.len_chars(),
    }
}

/// Char range of the Scraps (compost) region, era-aware; `None` when no
/// boundary exists. Symmetric to `manuscript_range_of`: the region never
/// includes the seam line or the break joining it.
pub fn scraps_range_of(rope: &ropey::Rope, blocks: &BlockMap) -> Option<Range<usize>> {
    match blocks.boundary() {
        Some((BoundaryEra::Top, b)) => {
            let line = b.min(rope.len_lines().saturating_sub(1));
            Some(0..line_break_before(rope, rope.line_to_char(line)))
        }
        Some((BoundaryEra::Tail, b)) => {
            let line = (b + 1).min(rope.len_lines());
            Some(rope.line_to_char(line)..rope.len_chars())
        }
        None => None,
    }
}

/// The manuscript region of a `(rope, spans, blocks)` state as a standalone
/// `(text, spans, blocks)` triple with char offsets REBASED to 0 — the ONE
/// slice function both books consume (cold-read adjudications F1):
/// `Document::manuscript_slice` delegates here for the live document, and the
/// Past book calls it directly over a checkpoint state's own triple, so the
/// pile can never enter either rendering. Era-aware like
/// `manuscript_range_of`; the seam line and the Scraps pile are excluded, and
/// every span is clipped to the slice (no span end past the slice length —
/// the regions-10 invariant).
pub fn manuscript_slice_of(
    rope: &ropey::Rope,
    spans: &SpanSet,
    blocks: &BlockMap,
) -> (String, SpanSet, BlockMap) {
    match blocks.boundary() {
        None => (rope.to_string(), spans.clone(), blocks.clone()),
        Some((BoundaryEra::Tail, b)) => {
            let range = manuscript_range_of(rope, blocks);
            let text = rope.slice(range.clone()).to_string();
            // `slice` clips and rebases; start is 0 so this is a clip.
            let spans = spans.slice(range);
            let first = b.min(blocks.len());
            let blocks = BlockMap::from_kinds(blocks.kinds()[..first].to_vec());
            (text, spans, blocks)
        }
        Some((BoundaryEra::Top, b)) => {
            let base = manuscript_range_of(rope, blocks).start;
            let text = rope.slice(base..).to_string();
            let mut sliced = SpanSet::default();
            for s in spans.spans() {
                if s.range.end <= base {
                    continue; // entirely in compost
                }
                let start = s.range.start.saturating_sub(base);
                let end = s.range.end - base;
                if end > start {
                    sliced.add(start..end, s.attr.clone());
                }
            }
            // The manuscript's block kinds: everything after the separator.
            let first = (b + 1).min(blocks.len());
            let blocks = BlockMap::from_kinds(blocks.kinds()[first..].to_vec());
            (text, sliced, blocks)
        }
    }
}

/// The one seam-aware region function (graveyard-interplay 1): which region
/// a char position falls in. Region ends are INCLUSIVE for position
/// classification — a caret at the very end of the last manuscript line is
/// manuscript-side; the seam owns only the boundary line and its joining
/// breaks. With no boundary everything is manuscript.
pub fn region_of_char(rope: &ropey::Rope, blocks: &BlockMap, ch: usize) -> Region {
    if blocks.boundary().is_none() {
        return Region::Manuscript;
    }
    let m = manuscript_range_of(rope, blocks);
    if ch >= m.start && ch <= m.end {
        return Region::Manuscript;
    }
    match scraps_range_of(rope, blocks) {
        Some(s) if ch >= s.start && ch <= s.end => Region::Scraps,
        _ => Region::Seam,
    }
}

/// The MEMBERSHIP-PRESERVING geometry flip (time-persistence 3/4): a
/// Top-era `(text, spans, blocks)` state re-expressed in the Tail era — the
/// manuscript first, then the scrap line, then the pile in its old order.
/// No block changes sides (07 N3's "text never teleports"); only positions
/// move, so span ranges are remapped ARITHMETICALLY. The caller (restore
/// normalization, the migration) remaps its own side records the same way
/// via `flip_char_map`. Precondition: `blocks.len()` matches the text's
/// line count (the caller checks; a mismatched state doesn't flip).
pub fn flip_state(text: &str, spans: &SpanSet, blocks: &BlockMap) -> (String, SpanSet, BlockMap) {
    let Some((BoundaryEra::Top, b)) = blocks.boundary() else {
        return (text.to_owned(), spans.clone(), blocks.clone());
    };
    let rope = ropey::Rope::from_str(text);
    let b = b.min(rope.len_lines().saturating_sub(1));
    let map = flip_char_map(&rope, b);
    let pile: String = rope.slice(0..map.pile_end).to_string();
    let manu: String = rope.slice(map.old_manu_start..).to_string();
    let new_text = format!("{manu}\n\n{pile}");
    let mut new_spans = SpanSet::default();
    for s in spans.spans() {
        // Clip each span into the region(s) it inhabits and remap; the
        // separator line's own chars (if any span covered them) vanish with
        // it — the seam never carries formatting.
        for (lo, hi) in [(0, map.pile_end), (map.old_manu_start, rope.len_chars())] {
            let cs = s.range.start.max(lo);
            let ce = s.range.end.min(hi);
            if cs < ce {
                new_spans.add(map.pos(cs)..map.pos(ce), s.attr.clone());
            }
        }
    }
    // Kinds: manuscript first, the seam line (never kind-stamped —
    // Paragraph), then the pile's kinds in order.
    let mut kinds: Vec<BlockKind> = blocks.kinds()[(b + 1).min(blocks.len())..].to_vec();
    if kinds.is_empty() {
        // A top-era "everything is compost" state: the flipped text still
        // opens with one empty manuscript line.
        kinds.push(BlockKind::Paragraph);
    }
    let seam = kinds.len();
    kinds.push(BlockKind::Paragraph);
    kinds.extend_from_slice(&blocks.kinds()[..b.min(blocks.len())]);
    let mut new_blocks = BlockMap::from_kinds(kinds);
    new_blocks.set_scrap_line(Some(seam));
    (new_text, new_spans, new_blocks)
}

/// The char-position remap the flip induces, exposed so side records
/// (annotation ranges, graveyard `origin_pos`) are moved arithmetically —
/// NEVER through `apply_op`'s clamp, which would strand every in-pile
/// anchor at char 0 (time-persistence 4).
pub struct FlipCharMap {
    /// One past the last pile char in the OLD text (the pile without the
    /// break joining it to the separator line).
    pub pile_end: usize,
    /// First manuscript char in the OLD text.
    pub old_manu_start: usize,
    /// Chars in the old manuscript (its new home is `0..manu_len`).
    pub manu_len: usize,
}

impl FlipCharMap {
    /// Where an old-text char position lands in the flipped text. Pile
    /// positions land after the manuscript + the two seam breaks; separator
    /// positions clamp onto the seam's blank line; `pile_end` itself (the
    /// pile's inclusive tail position) maps to the new document end.
    pub fn pos(&self, old: usize) -> usize {
        if old >= self.old_manu_start {
            old - self.old_manu_start
        } else if old <= self.pile_end {
            self.manu_len + 2 + old
        } else {
            // On the separator machinery between pile and manuscript.
            self.manu_len + 1
        }
    }
}

/// Build the flip's char map for a top-era rope with separator line `b`.
pub fn flip_char_map(rope: &ropey::Rope, b: usize) -> FlipCharMap {
    let sep_start = rope.line_to_char(b.min(rope.len_lines().saturating_sub(1)));
    let pile_end = line_break_before(rope, sep_start);
    let old_manu_start = rope.line_to_char((b + 1).min(rope.len_lines()));
    FlipCharMap {
        pile_end,
        old_manu_start,
        manu_len: rope.len_chars() - old_manu_start,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum InlineAttr {
    Emphasis,
    Strong,
    Strikethrough,
    Underline,
    Highlight,
    Code,
    Link(String),
    FootnoteRef(String),
}

impl InlineAttr {
    /// Peritext expansion: typing at the right edge continues the style —
    /// split by what the mark *means* (papercuts-2026-07 §1 A3, the
    /// Peritext/ProseMirror `inclusive` distinction):
    ///
    /// - **Emphasis-class** (Strong, Emphasis, Underline): character styles a
    ///   writer extends by typing — appending to a bold word keeps it bold.
    /// - **Extent-class** (Highlight, Strikethrough, joining Code, Link,
    ///   FootnoteRef): statements about a *fixed extent* of existing text —
    ///   marked once, they do not grow by appending.
    pub fn expands(&self) -> bool {
        matches!(self, Self::Strong | Self::Emphasis | Self::Underline)
    }
}

/// Per-block kind; lives beside the text, keyed by block index.
///
/// Serialization goes through `WireBlockKind` (docs/inline-images.md §10):
/// the runtime `Image` variant lost its vestigial `caption` field — the
/// block's own line IS the caption now — but the wire keeps the key in
/// both directions, because a released build's strict serde errors on a
/// missing field and falls back to the legacy token parser, which
/// collapses every kind in the file and persists the wreck on its next
/// save (same class as the boundary-key era rule).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(from = "WireBlockKind", into = "WireBlockKind")]
pub enum BlockKind {
    #[default]
    Paragraph,
    Heading(u8),
    Blockquote,
    ListItem {
        ordered: bool,
        depth: u8,
    },
    Divider,
    CodeBlock {
        /// Markdown fence info string; stored for round-trip, never acted on.
        info: String,
    },
    Image {
        src: String,
        alt: String,
    },
    FootnoteDef {
        id: String,
    },
}

/// The persistence shape of `BlockKind` — identical to the runtime enum
/// except that `Image` still carries the retired `caption` key. Writers
/// emit it empty forever (until an era flip once pre-migration builds are
/// extinct); readers accept it present or absent, and the open-time
/// migration (`store`'s `migrate_image_captions`) is what moves a
/// surviving non-empty value into the block's line. ~13 bytes per image
/// block is the price of a released build never misreading our output
/// (docs/inline-images.md §10; build plan, adjudicated pushback 2).
#[derive(Serialize, Deserialize)]
enum WireBlockKind {
    Paragraph,
    Heading(u8),
    Blockquote,
    ListItem {
        ordered: bool,
        depth: u8,
    },
    Divider,
    CodeBlock {
        info: String,
    },
    Image {
        src: String,
        alt: String,
        #[serde(default)]
        caption: String,
    },
    FootnoteDef {
        id: String,
    },
}

impl From<BlockKind> for WireBlockKind {
    fn from(kind: BlockKind) -> Self {
        match kind {
            BlockKind::Paragraph => Self::Paragraph,
            BlockKind::Heading(n) => Self::Heading(n),
            BlockKind::Blockquote => Self::Blockquote,
            BlockKind::ListItem { ordered, depth } => Self::ListItem { ordered, depth },
            BlockKind::Divider => Self::Divider,
            BlockKind::CodeBlock { info } => Self::CodeBlock { info },
            BlockKind::Image { src, alt } => Self::Image {
                src,
                alt,
                caption: String::new(),
            },
            BlockKind::FootnoteDef { id } => Self::FootnoteDef { id },
        }
    }
}

impl From<WireBlockKind> for BlockKind {
    fn from(kind: WireBlockKind) -> Self {
        match kind {
            WireBlockKind::Paragraph => Self::Paragraph,
            WireBlockKind::Heading(n) => Self::Heading(n),
            WireBlockKind::Blockquote => Self::Blockquote,
            WireBlockKind::ListItem { ordered, depth } => Self::ListItem { ordered, depth },
            WireBlockKind::Divider => Self::Divider,
            WireBlockKind::CodeBlock { info } => Self::CodeBlock { info },
            // The caption is dropped HERE, not migrated: this conversion
            // also decodes checkpoint/history states, which are read-only
            // past (build plan, pushback 3). Only the live document's
            // open path moves a legacy caption into the line.
            WireBlockKind::Image { src, alt, caption: _ } => Self::Image { src, alt },
            WireBlockKind::FootnoteDef { id } => Self::FootnoteDef { id },
        }
    }
}

impl BlockKind {
    /// The §2 class split (docs/inline-images.md, "the wall law"): block
    /// kinds divide into **flowing** — made of words, following the
    /// merge-keeps-first / split-clones-both rule — and **furniture**
    /// (Image, Divider): things that stand in the column but are not made
    /// of words, which no text mechanic may move, clone, or absorb. Every
    /// wall law reads this predicate, nowhere else (build plan, "the
    /// refactor, shaped").
    pub fn is_furniture(&self) -> bool {
        matches!(self, Self::Image { .. } | Self::Divider)
    }
}

/// Which geometry a boundary index describes (the Scraps build,
/// docs/impl/compost-fresh/adjudications.md, "the foundation"). The era is
/// NOT stored as its own field: it is derived from WHICH boundary field a
/// `BlockMap` carries — `aside_boundary` (legacy, serde default) means Top,
/// `scrap_line` means Tail. Every pre-Scraps file, checkpoint state, and
/// history snapshot therefore decodes as Top-era without rewriting a byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryEra {
    /// The shipped compost-at-top geometry: blocks `0..b` are the pile,
    /// block `b` the separator, blocks `b+1..` the manuscript.
    Top,
    /// The Scraps geometry: blocks `0..b` are the manuscript, block `b` the
    /// scrap line (seam), blocks `b+1..` the pile.
    Tail,
}

/// Which region of the document a position falls in — the ONE seam-aware
/// region function shared by capture, exile, put back and show-origin
/// (adjudications, graveyard-interplay 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    Manuscript,
    /// The boundary's own line (plus the line break joining it to the
    /// manuscript) — zero-width for the caret, owned by neither region.
    Seam,
    Scraps,
}

/// Block kinds aligned with the text's newline-separated blocks.
/// Invariant: `kinds.len() == rope.len_lines()`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockMap {
    kinds: Vec<BlockKind>,
    /// The out-of-band asides boundary (docs/impl/02-asides.md §1, review
    /// B13/H42) — the LEGACY, Top-era field, kept as a read-only alias so
    /// every pre-Scraps file/checkpoint/history state decodes as old-era
    /// (adjudications, "the foundation"). Blocks `0..b` are the compost
    /// pile, block `b` is the plain empty separator paragraph, blocks
    /// `b+1..` are the manuscript. `None` means no pile exists.
    ///
    /// It is an INDEX, never a `BlockKind` variant: a new kind would make an
    /// older build's serde fall back to the token parser and silently reset
    /// EVERY block kind in the file, and `on_edit`'s split-cloning would
    /// duplicate a sentinel. An older build ignores this field (it persists
    /// as its own key beside `kinds`), so compost folds into the manuscript —
    /// text preserved, boundary dropped, documented. `on_edit` keeps it
    /// aligned across every splice; never trusted unclamped (`adjust_boundary`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    aside_boundary: Option<usize>,
    /// The Tail-era boundary — the scrap line. Blocks `0..b` are the
    /// manuscript, block `b` the seam's own blank line, blocks `b+1..` the
    /// Scraps pile. Persisted as its own serde field AND its own key in the
    /// Loro blocks map (`scrap_line`), beside the legacy `boundary` key which
    /// migrated saves write as `null` — so a top-era build reading a tail-era
    /// file degrades to no-boundary (the documented safe path), never to a
    /// misread. Era = which field is present; when both somehow are (a
    /// damaged file), the tail field wins — new builds only ever write it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scrap_line: Option<usize>,
}

impl Default for BlockMap {
    fn default() -> Self {
        Self {
            kinds: vec![BlockKind::default()],
            aside_boundary: None,
            scrap_line: None,
        }
    }
}

impl BlockMap {
    pub fn new(blocks: usize) -> Self {
        Self {
            kinds: vec![BlockKind::default(); blocks.max(1)],
            aside_boundary: None,
            scrap_line: None,
        }
    }

    pub fn from_kinds(kinds: Vec<BlockKind>) -> Self {
        if kinds.is_empty() {
            Self::default()
        } else {
            Self {
                kinds,
                aside_boundary: None,
                scrap_line: None,
            }
        }
    }

    pub fn len(&self) -> usize {
        self.kinds.len()
    }

    pub fn is_empty(&self) -> bool {
        false // invariant: at least one block
    }

    pub fn kinds(&self) -> &[BlockKind] {
        &self.kinds
    }

    pub fn kind(&self, block: usize) -> &BlockKind {
        self.kinds
            .get(block)
            .unwrap_or(&BlockKind::Paragraph)
    }

    pub fn set_kind(&mut self, block: usize, kind: BlockKind) {
        if let Some(slot) = self.kinds.get_mut(block) {
            *slot = kind;
        }
    }

    /// The LEGACY Top-era boundary index (see the field). Blocks `0..b` are
    /// compost, `b` the separator line, `b+1..` the manuscript. Read-only in
    /// spirit: only migration and legacy tests still install one.
    pub fn aside_boundary(&self) -> Option<usize> {
        self.aside_boundary
    }

    /// The Tail-era boundary index (see the field). Blocks `0..b` are the
    /// manuscript, `b` the scrap line, `b+1..` the Scraps pile.
    pub fn scrap_line(&self) -> Option<usize> {
        self.scrap_line
    }

    /// The boundary with its era, whichever field carries it. The tail field
    /// wins if both are somehow present (new builds only write `scrap_line`;
    /// migrated saves null the legacy key).
    pub fn boundary(&self) -> Option<(BoundaryEra, usize)> {
        match (self.scrap_line, self.aside_boundary) {
            (Some(b), _) => Some((BoundaryEra::Tail, b)),
            (None, Some(b)) => Some((BoundaryEra::Top, b)),
            (None, None) => None,
        }
    }

    /// Install a Top-era boundary, clamped to a real interior line: a valid
    /// boundary needs at least one compost block before it (`b >= 1`) and must
    /// land strictly inside the block range. Anything else means "no rail"
    /// (`None`), so a corrupted or stale index degrades to the empty-rail
    /// state rather than panicking a slice. Installing one clears any tail
    /// boundary — the two eras never coexist.
    pub fn set_aside_boundary(&mut self, boundary: Option<usize>) {
        self.aside_boundary = match boundary {
            Some(b) if b >= 1 && b < self.kinds.len() => Some(b),
            _ => None,
        };
        if self.aside_boundary.is_some() {
            self.scrap_line = None;
        }
    }

    /// Install a Tail-era boundary, clamped like `set_aside_boundary`: a
    /// valid scrap line needs at least one manuscript block before it
    /// (`b >= 1`) and at least one scrap block after it (`b + 1 < len`).
    /// Anything else means "no Scraps" — a corrupted or stale index degrades
    /// to the empty state (the seam simply doesn't exist), never a panic.
    /// Installing one clears any legacy top boundary.
    pub fn set_scrap_line(&mut self, boundary: Option<usize>) {
        self.scrap_line = match boundary {
            Some(b) if b >= 1 && b + 1 < self.kinds.len() => Some(b),
            _ => None,
        };
        if self.scrap_line.is_some() {
            self.aside_boundary = None;
        }
    }

    /// Asset ids referenced by Image blocks (for the save-time GC sweep).
    pub fn asset_refs<'a>(&'a self) -> impl Iterator<Item = &'a str> + 'a {
        self.kinds.iter().filter_map(|k| match k {
            BlockKind::Image { src, .. } => Some(src.as_str()),
            _ => None,
        })
    }

    /// Repair after a text edit. `block` is the block containing the edit
    /// start (pre-edit), `merged` how many newlines the edit deleted,
    /// `splits` how many it inserted. Merges keep the first block's kind,
    /// and FLOWING splits inherit it (Enter-at-heading-end is special-cased
    /// upstream). Furniture never clones (§2 wall law, docs/inline-images.md):
    /// when the split-source block is furniture, the first fragment keeps
    /// the kind and every inserted fragment is born a Paragraph. Merges
    /// across a furniture wall never reach here unclamped — the Document
    /// edit path decomposes them at the wall first (build plan R1).
    pub fn on_edit(&mut self, block: usize, merged: usize, splits: usize) {
        let block = block.min(self.kinds.len().saturating_sub(1));
        let drain_end = (block + 1 + merged).min(self.kinds.len());
        let removed = drain_end - (block + 1); // blocks actually spliced out
        self.kinds.drain(block + 1..drain_end);
        let kind = self.kinds[block].clone();
        let born = if kind.is_furniture() {
            BlockKind::Paragraph
        } else {
            kind
        };
        for _ in 0..splits {
            self.kinds.insert(block + 1, born.clone());
        }
        self.adjust_boundary(block, removed, splits);
    }

    /// Shift the out-of-band boundary (either era's) across the block splice
    /// `on_edit` just performed, so it keeps pointing at the same separator
    /// line. This is the ONLY thing that keeps the index aligned (it is not a
    /// kind, so no splice path moves it "for free" — review B13/H42). `block`
    /// is the edit's first block, `removed`/`splits` the blocks spliced out
    /// and in after it. Never panics: the result is clamped into the
    /// post-splice range, and a boundary whose region structurally vanished
    /// dissolves to `None` — an empty pile simply does not exist. (Textless-
    /// but-structurally-present emptiness is the editor-level evaporation
    /// rule, seam-mechanics 6; THIS is only the never-panic floor.)
    fn adjust_boundary(&mut self, block: usize, removed: usize, splits: usize) {
        let shifted = |b: usize| {
            if b <= block {
                // At or before the edit's first block: an edit starting at
                // `block` only touches lines strictly after it, so an earlier
                // boundary is untouched.
                Some(b)
            } else if b < block + 1 + removed {
                // The boundary line itself sat inside the spliced-out span —
                // it merged into `block`. Clamp onto the merge point (the app
                // guards keep normal edits from ever reaching here; this is
                // the never-panic floor, not a routine path).
                Some(block)
            } else {
                // Strictly after the spliced-out span: shift by the net delta.
                Some(b - removed + splits)
            }
        };
        let last = self.kinds.len().saturating_sub(1);
        if let Some(new_b) = self.aside_boundary.and_then(shifted) {
            // Top era: no compost blocks left (b = 0) dissolves the rail.
            self.aside_boundary = match new_b.min(last) {
                0 => None,
                n => Some(n),
            };
        }
        if let Some(new_b) = self.scrap_line.and_then(shifted) {
            // Tail era: the seam needs a manuscript block before it AND a
            // scrap block after it, or the region is structurally gone.
            self.scrap_line = match new_b.min(last) {
                n if n >= 1 && n + 1 < self.kinds.len() => Some(n),
                _ => None,
            };
        }
    }
}

/// A wall-clamped deletion, planned (§2 wall law, docs/inline-images.md;
/// build plan R1). `cuts` are the surviving sub-deletions in ascending byte
/// order over the PRE-EDIT rope — the executor runs them right-to-left so
/// the earlier ranges stay valid — and `insert_at` is where a replacement's
/// inserted text lands: the range start's side, stepped out of a surviving
/// separator.
#[derive(Debug, PartialEq, Eq)]
struct ClampPlan {
    cuts: Vec<Range<usize>>,
    insert_at: usize,
}

/// Plan the §2 range clamp: decompose a deletion whose byte range crosses a
/// furniture wall. The caller has already established the gate (the range
/// deletes at least one line break and some block in its line span is
/// furniture); this is pure geometry so the law is headlessly testable.
///
/// - A separator bounding a furniture block survives unless the deletion
///   takes that block WHOLE: the bytes on each side go, the separator at
///   the wall stands, both blocks stand (possibly emptied).
/// - Whole cover (§5's range door): the range encloses the block's full
///   line span — content plus every bounding separator it has — or covers
///   a NON-EMPTY caption plus at least one bounding separator. An empty
///   caption demands full enclosure, so a lone Backspace/Delete range at
///   its wall can never silently drain the picture (§0, the today-bug).
/// - Whole cover plus partial flanks decomposes into left-partial +
///   whole-cover + right-partial in ONE transaction (adjudicated pushback
///   5): the flanks do NOT fuse — the surviving separator stands between
///   them.
///
/// Taking a block whole consumes exactly one bounding separator: the
/// preceding one when it is coverable and unclaimed (`on_edit`'s
/// merge-keeps-first then drains the furniture kind for free), else the
/// following one — that form keeps the furniture kind on the merged line,
/// so the executor restamps it to the first surviving block's kind
/// (`edit_bytes_clamped`). Adjacent furniture chains resolve left to
/// right: a following-separator cover claims the wall it shares with the
/// next block, whose own cover then falls through to ITS following
/// separator. A wall the previous block left merely PROTECTED (standing
/// partial, or spared by its own prec-side cover) is still consumable —
/// the cover reclaims it from the protected set.
fn clamp_plan(rope: &ropey::Rope, blocks: &BlockMap, byte_range: &Range<usize>) -> ClampPlan {
    let (s, e) = (byte_range.start, byte_range.end);
    let last_line = rope.len_lines() - 1;
    let start_line = rope.byte_to_line(s);
    let end_line = rope.byte_to_line(e).min(last_line);
    // Byte geometry of line `i`: [line_start, text_end) is the content,
    // [text_end, next line_start) its trailing separator (empty on the
    // last line).
    let line_start = |i: usize| rope.line_to_byte(i);
    let text_end = |i: usize| {
        if i < last_line {
            rope.char_to_byte(line_break_before(rope, rope.line_to_char(i + 1)))
        } else {
            rope.len_bytes()
        }
    };
    // Separators the wall keeps standing, ascending, UNCLIPPED — a range
    // starting mid-separator (a char-aligned start between CR and LF) must
    // still land its replacement on the wall's left face, never inside the
    // break. A bounding separator of every furniture block in the span
    // lands here unless a whole-cover consumed it; the cut subtraction
    // below clips to the range.
    let mut protected: Vec<Range<usize>> = Vec::new();
    let push_protected = |protected: &mut Vec<Range<usize>>, sep: &Range<usize>| {
        if sep.start < e && sep.end > s && protected.last() != Some(sep) {
            protected.push(sep.clone());
        }
    };
    // The line whose FOLLOWING separator a whole-cover consumed — the wall
    // it shares with the next block is spoken for.
    let mut foll_claimed_by: Option<usize> = None;
    for f in start_line..=end_line {
        if !blocks.kind(f).is_furniture() {
            continue;
        }
        let (cs, ce) = (line_start(f), text_end(f));
        let prec = (f > 0).then(|| text_end(f - 1)..cs);
        let foll = (f < last_line).then(|| ce..line_start(f + 1));
        let covers = |sep: &Range<usize>| s <= sep.start && e >= sep.end;
        let prec_covered = prec.as_ref().is_some_and(&covers);
        let foll_covered = foll.as_ref().is_some_and(&covers);
        let prec_claimed = f > 0 && foll_claimed_by == Some(f - 1);
        let content_covered = s <= cs && e >= ce;
        let enclosed = content_covered
            && (prec.is_some() || foll.is_some())
            && prec.as_ref().is_none_or(&covers)
            && foll.as_ref().is_none_or(&covers);
        let door = content_covered && ce > cs && ((prec_covered && !prec_claimed) || foll_covered);
        let mut consume_prec = false;
        let mut consume_foll = false;
        if enclosed || door {
            if prec_covered && !prec_claimed {
                consume_prec = true;
                // The consumed wall may already stand in `protected`: it is
                // the previous furniture block's FOLLOWING separator, pushed
                // while that block stood partial — or kept standing by its
                // own prec-side cover. The whole-cover law (§2, §5's range
                // door) outranks that push: reclaim the wall, or the cut
                // subtraction below spares it and the enclosed picture
                // drains to §0's ghost. Nothing of the neighbour is harmed —
                // `on_edit`'s merge-keeps-first hands the merged line to the
                // STANDING block's kind.
                if protected.last() == prec.as_ref() {
                    protected.pop();
                }
            } else if foll_covered {
                consume_foll = true;
                foll_claimed_by = Some(f);
            }
            // Neither consumable (its one coverable wall was claimed by the
            // previous block's cover): demoted to a partial — both walls
            // stand, only the caption bytes in range go.
        }
        if !consume_prec
            && !prec_claimed
            && let Some(sep) = &prec
        {
            push_protected(&mut protected, sep);
        }
        if !consume_foll
            && let Some(sep) = &foll
        {
            push_protected(&mut protected, sep);
        }
    }
    // The cuts: the range minus the standing walls.
    let mut cuts: Vec<Range<usize>> = Vec::new();
    let mut at = s;
    for sep in &protected {
        if sep.start > at {
            cuts.push(at..sep.start);
        }
        at = at.max(sep.end);
    }
    if at < e {
        cuts.push(at..e);
    }
    // Replacement text lands at the range start's side; a start inside a
    // surviving separator steps back onto the wall's left face.
    let insert_at = protected
        .iter()
        .find(|sep| sep.start <= s && s < sep.end)
        .map_or(s, |sep| sep.start);
    ClampPlan { cuts, insert_at }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub range: Range<usize>,
    pub attr: InlineAttr,
}

/// Inline formatting as an interval set, kept sorted by start.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpanSet {
    spans: Vec<Span>,
}

impl SpanSet {
    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    fn needs_normalization(&self, len_chars: usize) -> bool {
        !self.spans.iter().all(|s| {
            s.range.start < s.range.end && s.range.end <= len_chars
        }) || !self.spans.windows(2).all(|p| p[0].range.start <= p[1].range.start)
    }

    fn affected_by(&self, op: &TextOp) -> bool {
        let ins = !op.insert.is_empty();
        self.spans.iter().any(|s| {
            (op.delete > 0 && s.range.end > op.pos)
                || (ins && (s.range.start >= op.pos || s.range.end > op.pos
                    || (s.range.end == op.pos && s.attr.expands())))
        })
    }

    /// Repair spans loaded from an untrusted/older file against the text
    /// they describe. Serde itself accepts inverted, unsorted, overlapping,
    /// and out-of-range intervals; the editor's char→byte conversion does
    /// not. The repair sorts once and merges same-attribute intervals in a
    /// linear sweep after grouping, avoiding quadratic open time.
    pub fn normalize(&mut self, len_chars: usize) {
        if !self.needs_normalization(len_chars) {
            // Preserve equal-start attribute ordering: it is observable in
            // undo snapshots and controls deterministic Markdown nesting.
            return;
        }
        for span in &mut self.spans {
            span.range.start = span.range.start.min(len_chars);
            span.range.end = span.range.end.min(len_chars);
        }
        self.spans.retain(|s| s.range.start < s.range.end);
        // Group equal attributes, merge their intervals in one sweep, then
        // restore the public start-sorted invariant. This avoids feeding a
        // loaded set through `add` one span at a time (quadratic on a richly
        // formatted long document).
        self.spans
            .sort_by(|a, b| a.attr.cmp(&b.attr).then(a.range.start.cmp(&b.range.start)));
        let mut merged: Vec<Span> = Vec::with_capacity(self.spans.len());
        for span in self.spans.drain(..) {
            if let Some(last) = merged.last_mut()
                && last.attr == span.attr
                && span.range.start <= last.range.end
            {
                last.range.end = last.range.end.max(span.range.end);
            } else {
                merged.push(span);
            }
        }
        // `sort_by_key` is stable: equal-start spans retain the deterministic
        // attribute order established by the grouping sort above. Do not use
        // `sort_unstable_by_key` here; equal-start order controls Markdown
        // marker nesting and is observable in undo snapshots.
        merged.sort_by_key(|s| s.range.start);
        self.spans = merged;
    }

    pub fn attrs_at(&self, pos: usize) -> impl Iterator<Item = &InlineAttr> {
        self.spans
            .iter()
            .filter(move |s| s.range.start <= pos && pos < s.range.end)
            .map(|s| &s.attr)
    }

    /// Is every position in `range` covered by spans with this attribute?
    /// (Same-attr spans may be adjacent after edits; chains count.)
    pub fn covers(&self, range: Range<usize>, attr: &InlineAttr) -> bool {
        if range.start >= range.end {
            return false;
        }
        let mut covered_to = range.start;
        for s in &self.spans {
            if s.attr != *attr || s.range.end <= covered_to {
                continue;
            }
            if s.range.start > covered_to {
                return false; // spans are sorted: this is a gap
            }
            covered_to = s.range.end;
            if covered_to >= range.end {
                return true;
            }
        }
        false
    }

    /// Apply an attribute over a range, merging with touching/overlapping
    /// spans of the same attribute.
    pub fn add(&mut self, range: Range<usize>, attr: InlineAttr) {
        if range.start >= range.end {
            return;
        }
        let mut merged = range;
        self.spans.retain(|s| {
            if s.attr == attr && s.range.start <= merged.end && merged.start <= s.range.end {
                merged.start = merged.start.min(s.range.start);
                merged.end = merged.end.max(s.range.end);
                false
            } else {
                true
            }
        });
        let at = self
            .spans
            .partition_point(|s| s.range.start < merged.start);
        self.spans.insert(
            at,
            Span {
                range: merged,
                attr,
            },
        );
    }

    /// Clear an attribute from a range, splitting spans that straddle it.
    pub fn remove(&mut self, range: Range<usize>, attr: &InlineAttr) {
        let mut result = Vec::with_capacity(self.spans.len());
        for span in self.spans.drain(..) {
            if span.attr != *attr || span.range.end <= range.start || range.end <= span.range.start
            {
                result.push(span);
                continue;
            }
            if span.range.start < range.start {
                result.push(Span {
                    range: span.range.start..range.start,
                    attr: span.attr.clone(),
                });
            }
            if range.end < span.range.end {
                result.push(Span {
                    range: range.end..span.range.end,
                    attr: span.attr.clone(),
                });
            }
        }
        // A split tail can land after a later-starting span of another
        // attr; the set must stay sorted — covers() walks it assuming
        // order and would otherwise see phantom gaps (toggle would then
        // re-add instead of clearing). Found by the model.rs state
        // machine, 2026-06-12.
        result.sort_by_key(|s| s.range.start);
        self.spans = result;
    }

    /// The formatting over `range`, clipped to it and rebased so `range.start`
    /// becomes 0 — the spans of a slice, captured so a later re-insertion can
    /// restore them (the graveyard's Put back reapplies what was cut, marks and
    /// all — P1: the tool records the writer's text verbatim, and formatting is
    /// part of the text). A pure read; the set itself is untouched.
    pub fn slice(&self, range: Range<usize>) -> SpanSet {
        let mut out = SpanSet::default();
        for s in &self.spans {
            let start = s.range.start.max(range.start);
            let end = s.range.end.min(range.end);
            if start < end {
                out.add(start - range.start..end - range.start, s.attr.clone());
            }
        }
        out
    }

    /// Keep all spans consistent across a text edit performed by the TYPING
    /// hand: delete `op.delete` chars at `op.pos`, then insert `op.insert`
    /// there, letting expanding styles absorb an appended insertion at their
    /// right edge (the Peritext continuation).
    pub fn apply_op(&mut self, op: &TextOp) {
        self.apply_op_inner(op, true);
    }

    /// Like `apply_op`, but for a MACHINE-performed insertion (put back,
    /// paste): expansion never fires at a span's right edge (papercuts-2026-07
    /// §1 A3 — "expansion is a typing affordance"). This is the belt that keeps
    /// resurrected/pasted text from being dressed in a neighbour's bold before
    /// its own spans re-add.
    pub fn apply_op_verbatim(&mut self, op: &TextOp) {
        self.apply_op_inner(op, false);
    }

    fn apply_op_inner(&mut self, op: &TextOp, by_typing: bool) {
        let del_end = op.pos + op.delete;
        let ins = op.insert.chars().count();
        // The paragraph seam kills momentum (papercuts-2026-07 §1 A2): an
        // expanding span at its right edge absorbs a typed insertion only up to
        // the FIRST newline in it — a bold run grows by the pre-seam text and
        // stops at the seam, never streaming onto the next paragraph. An
        // insertion that OPENS with a newline expands nothing (pre-seam is
        // empty); one with no newline expands fully. A newline typed strictly
        // INSIDE a run still grows it (the split keeps both halves marked) —
        // only the right-edge append is clamped. Checking `starts_with('\n')`
        // alone missed an embedded seam ("text\nmore" typed at an edge swallowed
        // the whole insertion); the pre-seam char count is the belt.
        let edge_grow = match op.insert.find('\n') {
            Some(nl) => op.insert[..nl].chars().count(),
            None => ins,
        };
        let clamp = |x: usize| {
            if x >= del_end {
                x - op.delete
            } else if x > op.pos {
                op.pos
            } else {
                x
            }
        };
        for span in &mut self.spans {
            if op.delete > 0 {
                span.range.start = clamp(span.range.start);
                span.range.end = clamp(span.range.end);
            }
            if ins > 0 {
                // Typing at the left edge stays outside (start shifts);
                // strictly inside grows the span; at the right edge only
                // expanding styles absorb the insertion, and only when the
                // typing hand appends non-seam text.
                if span.range.start >= op.pos {
                    span.range.start += ins;
                }
                if span.range.end > op.pos {
                    span.range.end += ins;
                } else if span.range.end == op.pos && by_typing && span.attr.expands() {
                    // Right-edge append: absorb only the pre-seam segment.
                    span.range.end += edge_grow;
                }
            }
        }
        self.spans.retain(|s| s.range.start < s.range.end);
        self.spans.sort_by_key(|s| s.range.start);
    }
}

/// Margin annotation status; Done/Dismissed leave the margin but persist
/// (the engine must not re-raise a dismissed diagnosis on the same span).
///
/// There is deliberately NO park terminal here (scopes-search 3, adjudicated
/// alternative): retire-on-park DELETES the diagnosis record after journaling
/// its `CardClosed` — a new enum variant would make every OLD build fail the
/// whole annotations parse. Semantics are identical: a deleted record can't
/// suppress, so returned text is re-flaggable (Move to the manuscript re-arms
/// by construction), and ctrl-Z still resurrects the card via the park atom's
/// notes snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoteStatus {
    Open,
    Done,
    Dismissed,
}

/// Annotation species: human ink vs machine query — visually and
/// behaviorally distinct in the margin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum NoteKind {
    #[default]
    Note,
    Diagnosis,
}

/// An overlay annotation anchored to a char range — never part of the text
/// stream. `title`/`level` are diagnosis fields (named problem;
/// developmental|line|copy); serde defaults keep older files loading.
///
/// `orphaned` records that a checkpoint restore could not find the passage
/// this note covered in the restored text: the note detached and was parked
/// at its best-effort former offset rather than following live content. It
/// rides through persistence so the rail can flag a lost anchor; ordinary
/// editing never sets it. (Set only by `Annotations::reanchor`.)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    pub id: u64,
    pub range: Range<usize>,
    pub body: String,
    pub status: NoteStatus,
    pub created_unix: i64,
    #[serde(default)]
    pub kind: NoteKind,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub level: String,
    #[serde(default)]
    pub orphaned: bool,
    /// The review pass that raised this diagnosis (0 = legacy, or a writer
    /// note). Lets a newer pass rest older-pass cards behind the rail.
    #[serde(default)]
    pub pass_id: u64,
    /// A diagnosis whose flagged text was edited since it was raised: the claim
    /// may no longer hold, so the card greys — NEVER auto-dismissed, only the
    /// writer dismisses. Set in `apply_op`; writer notes never go unverified.
    #[serde(default)]
    pub unverified: bool,
}

/// The annotation overlay. Anchors adjust like non-expanding spans
/// (insertions at the edges stay outside; a fully deleted anchor collapses
/// to a point and survives as an orphan, Hypothesis-style).
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Annotations {
    notes: Vec<Annotation>,
    next_id: u64,
}

/// Apply the annotation anchor law to one range. This is the shared pure
/// primitive for live adjustment and historical replay: non-expanding at
/// insertion boundaries, collapsing a fully consumed range to a point.
pub fn transform_annotation_range(range: &Range<usize>, op: &TextOp) -> Range<usize> {
    let del_end = op.pos + op.delete;
    let ins = op.insert.chars().count();
    let clamp = |x: usize| {
        if x >= del_end {
            x - op.delete
        } else if x > op.pos {
            op.pos
        } else {
            x
        }
    };
    let mut transformed = range.clone();
    if op.delete > 0 {
        transformed.start = clamp(transformed.start);
        transformed.end = clamp(transformed.end);
    }
    if ins > 0 {
        if transformed.start >= op.pos {
            transformed.start += ins;
        }
        if transformed.end > op.pos {
            transformed.end += ins;
        }
        if transformed.end < transformed.start {
            transformed.end = transformed.start;
        }
    }
    transformed
}

impl Annotations {
    fn needs_normalization(&self, len_chars: usize) -> bool {
        self.notes.iter().any(|n| n.range.start > n.range.end || n.range.end > len_chars)
            || self.notes.windows(2).any(|p| p[0].range.start > p[1].range.start)
            || self.next_id < self.notes.iter().map(|n| n.id).max().unwrap_or(0)
    }

    fn affected_by(&self, op: &TextOp) -> bool {
        let ins = !op.insert.is_empty();
        self.notes.iter().any(|n| {
            (op.delete > 0 && n.range.end > op.pos)
                || (ins && (n.range.start >= op.pos || n.range.end > op.pos))
        })
    }

    pub fn normalize(&mut self, len_chars: usize) {
        for note in &mut self.notes {
            note.range.start = note.range.start.min(len_chars);
            note.range.end = note.range.end.min(len_chars).max(note.range.start);
        }
        self.notes.sort_by_key(|n| n.range.start);
        self.next_id = self
            .next_id
            .max(self.notes.iter().map(|n| n.id).max().unwrap_or(0));
    }

    pub fn add(&mut self, range: Range<usize>, body: String, created_unix: i64) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.notes.push(Annotation {
            id,
            range,
            body,
            status: NoteStatus::Open,
            created_unix,
            kind: NoteKind::Note,
            title: String::new(),
            level: String::new(),
            orphaned: false,
            pass_id: 0,
            unverified: false,
        });
        self.notes.sort_by_key(|n| n.range.start);
        id
    }

    pub fn notes(&self) -> &[Annotation] {
        &self.notes
    }

    pub fn get(&self, id: u64) -> Option<&Annotation> {
        self.notes.iter().find(|n| n.id == id)
    }

    pub fn set_status(&mut self, id: u64, status: NoteStatus) {
        if let Some(n) = self.notes.iter_mut().find(|n| n.id == id) {
            n.status = status;
        }
    }

    /// Drop a note entirely (its text left the margin — orphan migration to
    /// compost, spec §3). Returns it so a caller could re-file it.
    pub fn remove(&mut self, id: u64) -> Option<Annotation> {
        let ix = self.notes.iter().position(|n| n.id == id)?;
        Some(self.notes.remove(ix))
    }

    pub fn set_body(&mut self, id: u64, body: String) {
        if let Some(n) = self.notes.iter_mut().find(|n| n.id == id) {
            // Only the writer's own notes are editable. A diagnosis body is
            // fixed (AI cards are read-only review queries), so the composer /
            // draft path can NEVER overwrite one — the corruption where a
            // note's live draft leaked onto every clicked AI card and persisted.
            if n.kind == NoteKind::Note {
                n.body = body;
            }
        }
    }

    /// Open notes in document order.
    pub fn open(&self) -> impl Iterator<Item = &Annotation> {
        self.notes
            .iter()
            .filter(|n| n.status == NoteStatus::Open)
    }

    pub fn push(&mut self, mut annotation: Annotation) -> u64 {
        self.next_id += 1;
        annotation.id = self.next_id;
        let id = annotation.id;
        self.notes.push(annotation);
        self.notes.sort_by_key(|n| n.range.start);
        id
    }

    /// Has a *dismissed* diagnosis with this title already covered this
    /// range? The engine must not re-raise what the author waved off.
    pub fn is_dismissed(&self, range: &Range<usize>, title: &str) -> bool {
        self.notes.iter().any(|n| {
            n.status == NoteStatus::Dismissed
                && n.title == title
                && n.range.start < range.end
                && range.start < n.range.end
        })
    }

    /// Should a freshly-anchored diagnosis at `range`/`title` be suppressed on a
    /// new pass? Yes if the writer already DISMISSED that problem there (don't
    /// re-nag) OR an OPEN one already covers it (don't stack a duplicate on a
    /// re-run). Matched by title + span overlap — the "same issue" proxy. (Done
    /// is excluded: a resolved issue that genuinely recurs may surface again.)
    pub fn is_suppressed(&self, range: &Range<usize>, title: &str) -> bool {
        self.notes.iter().any(|n| {
            n.kind == NoteKind::Diagnosis
                && matches!(n.status, NoteStatus::Dismissed | NoteStatus::Open)
                && n.title == title
                && n.range.start < range.end
                && range.start < n.range.end
        })
    }

    pub fn apply_op(&mut self, op: &TextOp) {
        let del_end = op.pos + op.delete;
        let ins = op.insert.chars().count();
        for n in &mut self.notes {
            // Staleness: a diagnosis whose flagged text is edited can no longer
            // be vouched for → mark it unverified (it greys; only the writer
            // dismisses). Tested on the ORIGINAL range, before adjustment: a
            // deletion overlapping the span, or an insertion strictly inside it.
            // Typing OUTSIDE the span never greys it (so writing near a card
            // doesn't grey the world); writer notes never decay.
            if n.kind == NoteKind::Diagnosis && !n.unverified {
                let hit = (op.delete > 0 && op.pos < n.range.end && del_end > n.range.start)
                    || (ins > 0 && op.pos > n.range.start && op.pos < n.range.end);
                if hit {
                    n.unverified = true;
                }
            }
            n.range = transform_annotation_range(&n.range, op);
        }
        self.notes.sort_by_key(|n| n.range.start);
    }

    /// Re-anchor every note by the passage it covered, when the whole text is
    /// replaced wholesale (checkpoint restore). Each note follows its covered
    /// substring to wherever that text now lives in `new_text`; the search
    /// starts from the note's own former offset, so repeated passages resolve
    /// in document order exactly like `diagnose::anchor` does for quotes.
    ///
    /// A note whose covered passage is gone (or that was a zero-width point
    /// with nothing to match) DETACHES: it is flagged `orphaned` and parked at
    /// its clamped former offset — never collapsed onto the document end,
    /// which is what a naive wholesale-delete adjustment would do. Status,
    /// body, kind and identity are preserved; only `range`/`orphaned` change.
    pub fn reanchor(&mut self, old_text: &str, new_text: &str) {
        let old_len = old_text.chars().count();
        let new_len = new_text.chars().count();
        for n in &mut self.notes {
            let start = n.range.start.min(old_len);
            let end = n.range.end.min(old_len).max(start);
            let covered = char_slice(old_text, start, end);
            match crate::diagnose::anchor(new_text, &covered, start.min(new_len)) {
                Some(found) => {
                    n.range = found;
                    n.orphaned = false;
                }
                None => {
                    let p = start.min(new_len);
                    n.range = p..p;
                    n.orphaned = true;
                }
            }
        }
        self.notes.sort_by_key(|n| n.range.start);
    }
}

/// A parked block's origin, as a RANGE-ANCHORED SIDE RECORD (adjudications,
/// seam-mechanics 7) — never item metadata. Reuses the annotation anchoring
/// grammar: the range shifts under every edit like a non-expanding note
/// anchor. Unlike a note, a record whose covered text is fully deleted DIES
/// with it (provenance describes text; no text, no record), so merge/split
/// of scraps needs no rule at all — each record follows its own text. Jots
/// create none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceRecord {
    pub id: u64,
    /// The parked text's current char range (follows edits like an anchor).
    pub range: Range<usize>,
    /// Trailing fragment of the paragraph the text was parked from — the
    /// margin one-liner's "from …" quote. A frozen past-tense fact.
    pub origin_quote: String,
    /// When the park happened (unix seconds, like `GraveEntry::cut_unix`).
    pub parked_unix: i64,
}

/// The provenance overlay: one record per selection-park, riding the same
/// undo snapshot and op-absorption path as notes and the graveyard. Persists
/// behind its own store channel; absent in older files (serde default).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    records: Vec<ProvenanceRecord>,
    next_id: u64,
}

impl Provenance {
    fn needs_normalization(&self, len_chars: usize) -> bool {
        self.records.iter().any(|r| r.range.start >= r.range.end || r.range.end > len_chars)
            || self.records.windows(2).any(|p| p[0].range.start > p[1].range.start)
            || self.next_id < self.records.iter().map(|r| r.id).max().unwrap_or(0)
    }

    fn affected_by(&self, op: &TextOp) -> bool {
        let ins = !op.insert.is_empty();
        self.records.iter().any(|r| {
            (op.delete > 0 && r.range.end > op.pos)
                || (ins && (r.range.start >= op.pos || r.range.end > op.pos))
        })
    }

    pub fn normalize(&mut self, len_chars: usize) {
        for record in &mut self.records {
            record.range.start = record.range.start.min(len_chars);
            record.range.end = record.range.end.min(len_chars).max(record.range.start);
        }
        self.records.retain(|r| r.range.start < r.range.end);
        self.records.sort_by_key(|r| r.range.start);
        self.next_id = self
            .next_id
            .max(self.records.iter().map(|r| r.id).max().unwrap_or(0));
    }

    pub fn records(&self) -> &[ProvenanceRecord] {
        &self.records
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn add(&mut self, range: Range<usize>, origin_quote: String, parked_unix: i64) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.records.push(ProvenanceRecord {
            id,
            range,
            origin_quote,
            parked_unix,
        });
        self.records.sort_by_key(|r| r.range.start);
        id
    }

    /// The record containing a resting position — the one whose one-liner
    /// the margin shows (two merged fragments show two honest origins, by
    /// caret position; seam-mechanics 7).
    pub fn at(&self, pos: usize) -> Option<&ProvenanceRecord> {
        self.records
            .iter()
            .find(|r| r.range.start <= pos && pos < r.range.end)
    }

    /// Anchor adjustment, in the notes' non-expanding grammar — except a
    /// record whose range collapses to a point is DROPPED: a deleted
    /// fragment's provenance dies with its text (never orphans).
    pub fn apply_op(&mut self, op: &TextOp) {
        let del_end = op.pos + op.delete;
        let ins = op.insert.chars().count();
        let clamp = |x: usize| {
            if x >= del_end {
                x - op.delete
            } else if x > op.pos {
                op.pos
            } else {
                x
            }
        };
        for r in &mut self.records {
            if op.delete > 0 {
                r.range.start = clamp(r.range.start);
                r.range.end = clamp(r.range.end);
            }
            if ins > 0 {
                if r.range.start >= op.pos {
                    r.range.start += ins;
                }
                if r.range.end > op.pos {
                    r.range.end += ins;
                }
                if r.range.end < r.range.start {
                    r.range.end = r.range.start;
                }
            }
        }
        self.records.retain(|r| r.range.start < r.range.end);
        self.records.sort_by_key(|r| r.range.start);
    }

    /// Wholesale-swap re-anchoring (checkpoint restore): each record follows
    /// its covered passage by content, like `Annotations::reanchor` — but a
    /// record whose passage is gone dies instead of detaching.
    pub fn reanchor(&mut self, old_text: &str, new_text: &str) {
        let old_len = old_text.chars().count();
        let new_len = new_text.chars().count();
        self.records.retain_mut(|r| {
            let start = r.range.start.min(old_len);
            let end = r.range.end.min(old_len).max(start);
            let covered = char_slice(old_text, start, end);
            match crate::diagnose::anchor(new_text, &covered, start.min(new_len)) {
                Some(found) if !covered.is_empty() => {
                    r.range = found;
                    true
                }
                _ => false,
            }
        });
        self.records.sort_by_key(|r| r.range.start);
    }
}

/// Which region a graveyard entry was cut FROM (graveyard-interplay 2):
/// recorded at filing time because it cannot be derived from `origin_pos`
/// later (an evaporated seam pins in-pile positions indistinguishably from
/// manuscript-end ones). Serde default = `Manuscript`, the exact
/// backward-compat pattern `spans`/`kinds` use — every pre-Scraps entry was
/// a manuscript cut by construction (in-pile cuts were refused).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GraveRegion {
    #[default]
    Manuscript,
    Scraps,
}

/// One deletion the graveyard is holding (docs/impl/02-asides.md §4). The
/// automatic pile — asides.md §0's "visible insurance that cuts survive." It
/// is a record, NOT rope text, so no region-editing edge case ever touches it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraveEntry {
    pub id: u64,
    /// The cut prose, verbatim. Captured at cut time by the editor (the doomed
    /// range still exists then); the `TextOp` drain no longer holds it, so the
    /// journal cannot supply this — the graveyard captures it itself (H43).
    pub text: String,
    /// A trailing fragment of the paragraph that PRECEDED the cut, so the entry
    /// can name its origin without storing the whole surrounding context.
    pub origin_quote: String,
    /// Best-effort char offset where the cut sat — shifted on every edit like a
    /// note anchor (`apply_op`), so Put back lands where the passage belongs.
    /// Put back re-clamps this into the manuscript region (review #62) so cut
    /// prose can never resurrect INTO the compost.
    pub origin_pos: usize,
    pub cut_unix: i64,
    pub words: u32,
    /// The cut prose's inline formatting, rebased so the entry text starts at 0
    /// (Bug D / P1: Put back restores what was cut, marks and all). `serde`
    /// default keeps entries persisted before this field loading (they simply
    /// put back as plain text, exactly the old lossy behaviour).
    #[serde(default)]
    pub spans: SpanSet,
    /// The cut span's per-block kinds, one per line of `text` (so a cut heading
    /// or list item comes back styled, not as body paragraphs). `serde` default
    /// for the same backward-compatibility reason as `spans`.
    #[serde(default)]
    pub kinds: Vec<BlockKind>,
    /// The region the text was cut FROM (see `GraveRegion`): Put back and
    /// show-origin clamp into THIS region, so text never crosses the seam on
    /// a round trip; the whisper can honestly say "from scraps".
    #[serde(default)]
    pub region: GraveRegion,
    /// Whether the exile verb read this cut as WHOLE paragraph block(s): the
    /// cut took its bounding separator along (no empty grave in the prose,
    /// papercuts-2026-07 §B1) and Put back rebuilds it as its own standing
    /// block(s) rather than splicing it mid-paragraph (§B2). `serde` default
    /// false keeps every pre-papercuts entry loading as a plain fragment.
    #[serde(default)]
    pub whole_blocks: bool,
    /// Whether the exile consumed a BLANK-LINE separator (the widening it
    /// shares with Set aside: the prose closes up beneath the departing
    /// block). Put back synthesizes the same blank line, so the round trip
    /// is byte-identical. Only meaningful with `whole_blocks`; `serde`
    /// default false keeps older entries returning with a plain join.
    #[serde(default)]
    pub blank_sep: bool,
}

/// The graveyard record: a side structure mirroring `Annotations` in every way
/// that matters — it shifts with the text (`apply_op`), rides the undo snapshot
/// (so undo of a cut removes its entry — P13's inverse-in-the-same-grammar),
/// and persists behind its own fingerprint channel (review B12). Rendered
/// read-only at the document tail; leaving is Put back or Delete only.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Graveyard {
    entries: Vec<GraveEntry>,
    next_id: u64,
}

impl Graveyard {
    fn needs_normalization(&self, document_len: usize) -> bool {
        self.entries.iter().any(|e| {
            e.origin_pos > document_len
                || e.spans.needs_normalization(e.text.chars().count())
        }) || self.next_id < self.entries.iter().map(|e| e.id).max().unwrap_or(0)
    }

    fn affected_by(&self, op: &TextOp) -> bool {
        let ins = !op.insert.is_empty();
        self.entries.iter().any(|e| {
            (op.delete > 0 && e.origin_pos > op.pos)
                || (ins && e.origin_pos >= op.pos)
        })
    }

    pub fn normalize(&mut self, document_len: usize) {
        for entry in &mut self.entries {
            entry.origin_pos = entry.origin_pos.min(document_len);
            entry.spans.normalize(entry.text.chars().count());
        }
        self.next_id = self
            .next_id
            .max(self.entries.iter().map(|e| e.id).max().unwrap_or(0));
    }

    /// Entries in filing order (oldest first). The footer renders newest-first;
    /// that is a view concern, so storage keeps the honest insertion order.
    pub fn entries(&self) -> &[GraveEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, id: u64) -> Option<&GraveEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// File a cut. `words` is counted here, once, from the verbatim text.
    /// `spans`/`kinds` carry the cut's formatting and block structure so Put
    /// back is lossless (Bug D); a plain-text caller passes the defaults.
    /// `region` records which side of the seam the text left (Put back
    /// returns through the same door — graveyard-interplay 1/2).
    /// `whole_blocks` records whether the exile verb read the cut as complete
    /// paragraph block(s), so Put back rebuilds a paragraph, not a splice;
    /// `blank_sep` whether the consumed separator was a blank line, so Put
    /// back rebuilds that too.
    #[allow(clippy::too_many_arguments)]
    pub fn file(
        &mut self,
        text: String,
        origin_quote: String,
        origin_pos: usize,
        cut_unix: i64,
        spans: SpanSet,
        kinds: Vec<BlockKind>,
        region: GraveRegion,
        whole_blocks: bool,
        blank_sep: bool,
    ) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        let words = text.split_whitespace().count() as u32;
        self.entries.push(GraveEntry {
            id,
            text,
            origin_quote,
            origin_pos,
            cut_unix,
            words,
            spans,
            kinds,
            region,
            whole_blocks,
            blank_sep,
        });
        id
    }

    /// Asset ids referenced by entries' Image block kinds — the graveyard's
    /// contribution to the GC reachable set (graveyard-interplay 9): Put back
    /// can re-stamp an Image block, so its bytes must survive while the entry
    /// does.
    pub fn asset_refs<'a>(&'a self) -> impl Iterator<Item = &'a str> + 'a {
        self.entries.iter().flat_map(|e| {
            e.kinds.iter().filter_map(|k| match k {
                BlockKind::Image { src, .. } => Some(src.as_str()),
                _ => None,
            })
        })
    }

    /// Remove an entry (Put back's second half, or Delete). Returns it so the
    /// caller can re-insert its text.
    pub fn remove(&mut self, id: u64) -> Option<GraveEntry> {
        let ix = self.entries.iter().position(|e| e.id == id)?;
        Some(self.entries.remove(ix))
    }

    /// Shift every `origin_pos` across a text edit exactly as a non-expanding
    /// note anchor point moves (mirrors `Annotations::apply_op`), so Put back
    /// stays honest as the manuscript changes under it.
    pub fn apply_op(&mut self, op: &TextOp) {
        let del_end = op.pos + op.delete;
        let ins = op.insert.chars().count();
        for e in &mut self.entries {
            if op.delete > 0 {
                e.origin_pos = if e.origin_pos >= del_end {
                    e.origin_pos - op.delete
                } else if e.origin_pos > op.pos {
                    op.pos
                } else {
                    e.origin_pos
                };
            }
            if ins > 0 && e.origin_pos >= op.pos {
                e.origin_pos += ins;
            }
        }
    }

    /// Clamp every `origin_pos` into `len` (after a wholesale text swap, where
    /// `apply_op` of the giant op would otherwise pin them all to the tail).
    pub fn clamp(&mut self, len: usize) {
        for e in &mut self.entries {
            e.origin_pos = e.origin_pos.min(len);
        }
    }
}

/// One transaction's side-state snapshot: everything beside the text that
/// undo/redo must restore in lockstep. A TUPLE (not a struct) on purpose —
/// it is the persisted `History` element, and its serde shape is the
/// documented compatibility contract (a pre-Scraps 4-tuple history fails the
/// arity check and is dropped once, non-destructively; see `History`).
type SideState = (
    Shared<SpanSet>, Shared<BlockMap>, Shared<Annotations>,
    Shared<Graveyard>, Shared<Provenance>,
);

/// What a park did — enough for the editor to journal card closures and
/// aim the receipt, without re-deriving any of it.
#[derive(Debug)]
pub struct ParkOutcome {
    /// Where the caret returns (char offset): the collapse point `s` —
    /// the writer parked a thought, she did not travel.
    pub caret: usize,
    /// Diagnosis cards retired inside the atom (for `CardClosed` events).
    pub retired: Vec<u64>,
    /// True when the gesture was the adoption (nothing moved; the seam was
    /// born above the writer's own trailing pile).
    pub adopted: bool,
}

/// Text + formatting + block structure with unified, transaction-aligned
/// undo. The buffer owns text history; span/block states are snapshotted
/// per transaction (they're small — snapshots beat op inversion).
#[derive(Debug, Default)]
pub struct Document {
    buffer: Buffer,
    spans: Shared<SpanSet>,
    blocks: Shared<BlockMap>,
    notes: Shared<Annotations>,
    /// The graveyard record (docs/impl/02-asides.md §4). Lives here beside the
    /// notes so it rides the SAME undo snapshot (undo of a cut removes its
    /// entry) and the SAME op-absorption path (`origin_pos` shifts like a note
    /// anchor). See `GraveEntry`.
    graveyard: Shared<Graveyard>,
    /// Parked blocks' origin records (see `Provenance`): same lifecycle as
    /// the graveyard — snapshot-riding, op-absorbed, own store channel.
    provenance: Shared<Provenance>,
    undo_states: Vec<SideState>,
    redo_states: Vec<SideState>,
    pending_ops: Vec<TextOp>,
    /// The edit-run record (docs/impl/00-journal.md). Fed at the op drains —
    /// absorb, undo, redo — so every text mutation is journaled with a wall
    /// clock, including inverse edits. Wholesale swaps (restore) pause it
    /// and record their honest event instead.
    journal: crate::journal::Journal,
    /// Monotonic counter bumped by every layout-affecting mutation (text,
    /// spans, blocks, note ranges). The view caches its laid-out frame keyed
    /// on this, so a scroll/blink/caret-move with an unchanged document reuses
    /// the previous layout instead of rebuilding it. Transient (never
    /// serialized); over-bumping only costs a wasted rebuild, missing a bump
    /// would risk a stale frame — so every `&mut self` mutator bumps it.
    revision: u64,
}

impl Document {
    pub fn new(text: &str, mut spans: SpanSet, blocks: BlockMap) -> Self {
        let buffer = Buffer::new(text);
        let mut blocks = blocks;
        spans.normalize(buffer.rope().len_chars());
        // Repair a stale/foreign block map against the actual text.
        let lines = buffer.rope().len_lines();
        if blocks.len() != lines {
            let mut repaired = BlockMap::new(lines);
            // Keep a still-valid serialized region boundary. Dropping it
            // silently reclassifies private Scraps as manuscript; the
            // restore/replay repair paths already preserve it the same way.
            repaired.set_aside_boundary(blocks.aside_boundary());
            repaired.set_scrap_line(blocks.scrap_line());
            blocks = repaired;
        }
        Self {
            buffer,
            spans: spans.into(),
            blocks: blocks.into(),
            ..Default::default()
        }
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn spans(&self) -> &SpanSet {
        &self.spans
    }

    pub fn blocks(&self) -> &BlockMap {
        &self.blocks
    }

    /// Monotonic layout revision (see the `revision` field): equal across two
    /// reads ⟺ no layout-affecting mutation happened between them.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Block index containing a byte offset.
    pub fn block_of_byte(&self, byte: usize) -> usize {
        self.buffer.rope().byte_to_line(byte)
    }

    // Hot-path delegates, so the editor reads as `doc.rope()` etc.
    pub fn rope(&self) -> &ropey::Rope {
        self.buffer.rope()
    }

    pub fn text(&self) -> String {
        self.buffer.text()
    }

    pub fn len_bytes(&self) -> usize {
        self.buffer.len_bytes()
    }

    pub fn slice_bytes(&self, range: Range<usize>) -> String {
        self.buffer.slice_bytes(range)
    }

    pub fn byte_to_utf16(&self, byte: usize) -> usize {
        self.buffer.byte_to_utf16(byte)
    }

    pub fn utf16_to_byte(&self, utf16: usize) -> usize {
        self.buffer.utf16_to_byte(utf16)
    }

    pub fn char_to_byte(&self, ch: usize) -> usize {
        self.buffer.char_to_byte(ch)
    }

    /// Drain text ops for the durable-store mirror.
    pub fn take_ops(&mut self) -> Vec<TextOp> {
        std::mem::take(&mut self.pending_ops)
    }

    fn absorb_buffer_ops(&mut self) {
        self.absorb_buffer_ops_inner(true);
    }

    /// Absorb a MACHINE insertion (put back, paste): spans re-anchor without
    /// right-edge expansion (the A3 machine-insertion law), so the inserted
    /// text is never dressed in a neighbour's mark. Every other side structure
    /// shifts identically to the typing path.
    fn absorb_buffer_ops_verbatim(&mut self) {
        self.absorb_buffer_ops_inner(false);
    }

    fn absorb_buffer_ops_inner(&mut self, by_typing: bool) {
        let ops = self.buffer.take_ops();
        let now = crate::journal::now_ms();
        for (op, deleted) in &ops {
            // The `affected_by` gate rides OUTSIDE the typing/verbatim split:
            // it is what keeps an untouched structure structurally shared, and
            // the predicate carries the typing law (right-edge expansion), so
            // it is a superset of what the verbatim path can move.
            if self.spans.affected_by(op) {
                if by_typing {
                    self.spans.apply_op(op);
                } else {
                    self.spans.apply_op_verbatim(op);
                }
            }
            if self.notes.affected_by(op) { self.notes.apply_op(op); }
            if self.graveyard.affected_by(op) { self.graveyard.apply_op(op); }
            if self.provenance.affected_by(op) { self.provenance.apply_op(op); }
            self.journal.record_deleted(op, now, deleted);
        }
        self.pending_ops.extend(ops.into_iter().map(|(op, _)| op));
    }

    /// Journal a boundary mutation (time-persistence 2): every seam
    /// birth/move/evaporation records a `Seam` event so `ReplayDoc` can
    /// evolve the boundary by timestamp and strip-restore reproduces the
    /// scrubbed moment's seam. `before` is the tail boundary as it stood
    /// when the mutating call began.
    fn journal_seam(&mut self, before: Option<usize>) {
        let after = self.blocks.scrap_line();
        if before != after {
            self.journal
                .record_event(crate::journal::JournalEvent::Seam {
                    t: crate::journal::now_ms(),
                    at: after,
                });
        }
    }

    pub fn journal(&self) -> &crate::journal::Journal {
        &self.journal
    }

    /// Mutable for event recording (passes, card closures, restores) and
    /// pre-save settling. Journal changes never affect layout — no
    /// `revision` bump.
    pub fn journal_mut(&mut self) -> &mut crate::journal::Journal {
        &mut self.journal
    }

    /// Install the persisted journal at load (like `set_notes`).
    pub fn set_journal(&mut self, journal: crate::journal::Journal) {
        self.journal = journal;
    }

    /// (block containing the edit start, line breaks deleted) — computed
    /// against the pre-edit rope. The break count uses ropey's own
    /// line metric so it agrees with `len_lines()` for *every* separator
    /// (LF, CR, CRLF-as-one, VT, FF, NEL, U+2028, U+2029) — not just '\n',
    /// which a paste of classic-Mac / PDF-copied text can smuggle in.
    fn pre_edit_info(&self, byte_range: &Range<usize>) -> (usize, usize) {
        let rope = self.buffer.rope();
        let start = rope.byte_to_char(byte_range.start);
        let end = rope.byte_to_char(byte_range.end);
        let block = rope.char_to_line(start);
        let merged = rope.char_to_line(end) - block;
        (block, merged)
    }

    fn snapshot(&self) -> SideState {
        (
            self.spans.clone(),
            self.blocks.clone(),
            self.notes.clone(),
            self.graveyard.clone(),
            self.provenance.clone(),
        )
    }

    fn normalize_side_structures(&mut self) {
        let len = self.buffer.rope().len_chars();
        if self.spans.needs_normalization(len) { self.spans.normalize(len); }
        if self.notes.needs_normalization(len) { self.notes.normalize(len); }
        if self.graveyard.needs_normalization(len) { self.graveyard.normalize(len); }
        if self.provenance.needs_normalization(len) { self.provenance.normalize(len); }
        let lines = self.buffer.rope().len_lines();
        if self.blocks.len() != lines {
            let mut repaired = BlockMap::new(lines);
            repaired.set_aside_boundary(self.blocks.aside_boundary());
            repaired.set_scrap_line(self.blocks.scrap_line());
            self.blocks = repaired.into();
        }
    }

    pub fn notes(&self) -> &Annotations {
        &self.notes
    }

    pub fn set_notes(&mut self, mut notes: Annotations) {
        self.revision += 1;
        notes.normalize(self.buffer.rope().len_chars());
        self.notes = notes.into();
    }

    pub fn graveyard(&self) -> &Graveyard {
        &self.graveyard
    }

    /// Install the persisted graveyard at load (like `set_notes`).
    pub fn set_graveyard(&mut self, mut graveyard: Graveyard) {
        self.revision += 1;
        graveyard.normalize(self.buffer.rope().len_chars());
        self.graveyard = graveyard.into();
    }

    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    /// Install the persisted provenance at load (like `set_notes`).
    pub fn set_provenance(&mut self, mut provenance: Provenance) {
        self.revision += 1;
        provenance.normalize(self.buffer.rope().len_chars());
        self.provenance = provenance.into();
    }

    /// The out-of-band asides boundary — LEGACY Top-era reader (see
    /// `BlockMap::aside_boundary`).
    pub fn aside_boundary(&self) -> Option<usize> {
        self.blocks.aside_boundary()
    }

    /// The Tail-era boundary (see `BlockMap::scrap_line`).
    pub fn scrap_line(&self) -> Option<usize> {
        self.blocks.scrap_line()
    }

    /// The boundary with its era, whichever field carries it.
    pub fn boundary(&self) -> Option<(BoundaryEra, usize)> {
        self.blocks.boundary()
    }

    /// Set the LEGACY top boundary as its own undoable transaction. Kept for
    /// fixtures and migration tests; live parks go through `set_aside`.
    pub fn set_aside_boundary(&mut self, boundary: Option<usize>) {
        self.revision += 1;
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        self.blocks.set_aside_boundary(boundary);
    }

    /// Set the tail boundary as its own undoable transaction (the adoption
    /// gesture's mechanism; the seam birth/evaporation otherwise ride the
    /// park/edit transactions). Journals the seam.
    pub fn set_scrap_line(&mut self, boundary: Option<usize>) {
        self.revision += 1;
        let before = self.blocks.scrap_line();
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        self.blocks.set_scrap_line(boundary);
        self.journal_seam(before);
    }

    /// First manuscript char. Top era: the start of the line after the
    /// boundary; tail era and boundary-less: 0 (the document opens on the
    /// story). Everything manuscript-scoped rebases against the range this
    /// starts (recon TRAP 14; review H40; scopes-search 4).
    pub fn manuscript_base_char(&self) -> usize {
        self.manuscript_char_range().start
    }

    /// One past the last manuscript char (see `manuscript_range_of`).
    pub fn manuscript_end_char(&self) -> usize {
        self.manuscript_char_range().end
    }

    /// The manuscript as a char range (the pile excluded, era-aware). `None`
    /// boundary → the whole document. Used to word-count and slice without
    /// cloning.
    pub fn manuscript_char_range(&self) -> Range<usize> {
        manuscript_range_of(self.buffer.rope(), &self.blocks)
    }

    /// The Scraps pile as a char range, era-aware; `None` when no boundary.
    pub fn scraps_char_range(&self) -> Option<Range<usize>> {
        scraps_range_of(self.buffer.rope(), &self.blocks)
    }

    /// The one seam-aware region function (graveyard-interplay 1).
    pub fn region_of_char(&self, ch: usize) -> Region {
        region_of_char(self.buffer.rope(), &self.blocks, ch)
    }

    /// Does the Scraps region hold no text at all? (Blank blocks may remain —
    /// "textless = empty", seam-mechanics 6. The editor evaporates the seam on
    /// this, guarded by the caret's retype race.)
    pub fn scraps_textless(&self) -> bool {
        match self.scraps_char_range() {
            Some(r) if self.blocks.scrap_line().is_some() => self
                .buffer
                .rope()
                .slice(r)
                .chars()
                .all(char::is_whitespace),
            _ => false,
        }
    }

    /// The manuscript region as a standalone `(text, spans, blocks)` triple with
    /// char offsets REBASED to 0 — directly usable by `to_markdown`, word
    /// counting, and the AI pass. Rebasing here (not leaving offsets relative to
    /// the full doc) is what keeps a card or an exported span from ever landing
    /// in the pile (review H40, TRAP 4). Add `manuscript_base_char()` back to
    /// any range that must return to full-document coordinates (0 in tail era —
    /// the manuscript is the document's head).
    pub fn manuscript_slice(&self) -> (String, SpanSet, BlockMap) {
        manuscript_slice_of(self.buffer.rope(), &self.spans, &self.blocks)
    }

    /// Delete the seam line and its blank leftovers within the CURRENTLY
    /// OPEN transaction (the caller has snapshotted): the evaporation half
    /// of "textless = empty" (seam-mechanics 6) and of "exiling the last
    /// scrap collapses the boundary in the same atom" (graveyard-interplay
    /// 4). Undo restores text + seam together for free, via the pre-edit
    /// snapshot's BlockMap.
    fn evaporate_scraps_in_tx(&mut self) {
        let before = self.blocks.scrap_line();
        if before.is_none() {
            return;
        }
        self.revision += 1;
        let start = self.manuscript_end_char();
        let end = self.buffer.rope().len_chars();
        if start < end {
            let sb = self.buffer.char_to_byte(start);
            let eb = self.buffer.char_to_byte(end);
            let (block, merged) = self.pre_edit_info(&(sb..eb));
            self.buffer.edit_bytes_grouped(sb..eb, "");
            self.blocks.on_edit(block, merged, 0);
            self.absorb_buffer_ops();
        }
        self.blocks.set_scrap_line(None);
        self.journal_seam(before);
    }

    /// The standalone evaporation: the retype-race guard's release
    /// (seam-mechanics 6). While the caret sat inside a textless pile the
    /// seam held (count honestly reading 0); when the caret leaves, the
    /// editor calls this — its own undoable transaction. Returns whether the
    /// seam evaporated.
    pub fn evaporate_scraps(&mut self) -> bool {
        if !self.scraps_textless() {
            return false;
        }
        self.revision += 1;
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        self.evaporate_scraps_in_tx();
        true
    }

    /// Evaporate within the transaction the caller's edit just opened, IF the
    /// pile is textless. The editor calls this right after a deletion whose
    /// caret ends manuscript-side ("blank leftovers + boundary removed in the
    /// same transaction"). Returns whether the seam evaporated.
    pub fn evaporate_scraps_if_textless_in_tx(&mut self) -> bool {
        if !self.scraps_textless() {
            return false;
        }
        self.evaporate_scraps_in_tx();
        true
    }

    /// Delete `byte_range` and file the removed prose in the graveyard as ONE
    /// undoable transaction. Both the auto-cut trigger and the explicit "Send
    /// to the graveyard" verb route here — on EITHER side of the seam (one
    /// capture law, graveyard-interplay 5); a qualifying TYPE-OVER routes
    /// through the sibling `replace_to_graveyard` (§4's amendment: with or
    /// without replacement text). Because `edit_bytes` snapshots
    /// the PRE-cut side-state (graveyard included) before the filing, undoing
    /// the deletion restores a graveyard WITHOUT this entry — P13's inverse
    /// in the same grammar, no correlation table needed. `origin_pos` is the
    /// cut point (where Put back returns the text) and the entry records its
    /// REGION so the return can never cross the seam. `collapse_emptied`
    /// (the explicit Exile verb) folds the seam's evaporation into the same
    /// atom when the cut empties the pile; the auto-capture path passes
    /// false and leaves that to the editor's retype-race guard. The DELETION
    /// is always exact-bytes (two verbs, two contracts — only exile widens),
    /// but the ENTRY records `whole_blocks: true` when the normalized range
    /// covered complete blocks, so a backspaced paragraph still returns as a
    /// standing paragraph, never a splice glued to its neighbour (papercuts
    /// follow-up, report 3). Returns the new entry id.
    pub fn cut_to_graveyard(
        &mut self,
        byte_range: Range<usize>,
        origin_quote: String,
        cut_unix: i64,
        collapse_emptied: bool,
    ) -> u64 {
        self.file_removal(byte_range, "", false, origin_quote, cut_unix, collapse_emptied)
    }

    /// The type-over half of §4's capture law ("anything big that leaves in
    /// one stroke, survives"): delete `byte_range`, file the REMOVED prose,
    /// and land `replacement` where it stood — ONE undoable transaction, so
    /// undo restores the old text, peels the replacement, and drops the
    /// entry together (the same parity `cut_to_graveyard` documents).
    /// `machine` picks the replacement's span anchoring: a paste re-anchors
    /// verbatim (the A3 machine-insertion law), the typing hand extends.
    /// The editor's auto-cut trigger is the only caller; the explicit verbs
    /// never carry a replacement.
    pub fn replace_to_graveyard(
        &mut self,
        byte_range: Range<usize>,
        replacement: &str,
        machine: bool,
        origin_quote: String,
        cut_unix: i64,
    ) -> u64 {
        self.file_removal(byte_range, replacement, machine, origin_quote, cut_unix, false)
    }

    /// Shared body of `cut_to_graveyard` / `replace_to_graveyard`: the one
    /// removal-capture engine, `replacement` empty for the plain cut.
    fn file_removal(
        &mut self,
        byte_range: Range<usize>,
        replacement: &str,
        machine: bool,
        origin_quote: String,
        cut_unix: i64,
        collapse_emptied: bool,
    ) -> u64 {
        let repl_chars = replacement.chars().count();
        let (block, merged) = self.pre_edit_info(&byte_range);
        if self.crosses_furniture_wall(block, merged) {
            // §2 ("no text mechanic may move, clone, or absorb" furniture):
            // the edit below CLAMPS, so walls and spared blocks inside
            // `byte_range` survive the delete. Filing the raw slice would
            // grave bytes still standing in the document — Put back then
            // re-inserts the surviving separator and stamps the picture's
            // kind onto a fresh line: a plain range delete minting a second
            // picture. File the clamp's ACTUAL cuts instead, one
            // region-honest entry per surviving sub-deletion; they all ride
            // `edit_bytes`' one pre-cut snapshot, so a single undo still
            // removes every entry with the text's return.
            let cuts = clamp_plan(self.buffer.rope(), &self.blocks, &byte_range).cuts;
            let mut pending = Vec::with_capacity(cuts.len());
            let mut removed = 0usize;
            for cut in &cuts {
                let rope = self.buffer.rope();
                let (s, e) = (rope.byte_to_char(cut.start), rope.byte_to_char(cut.end));
                // Origins are POST-edit positions: each cut's start, less
                // the chars the earlier cuts take (the walls between them
                // stand, so nothing else shifts), plus the replacement —
                // it lands at the range start's side, left of every cut.
                pending.push((self.grave_capture(s, e), s - removed + repl_chars));
                removed += e - s;
            }
            // Re-plans identically; the clamped executor runs the cuts and
            // lands the replacement in one transaction.
            if machine {
                self.edit_bytes_verbatim(byte_range, replacement);
            } else {
                self.edit_bytes(byte_range, replacement);
            }
            self.revision += 1;
            // A separator-only range plans no cuts and files nothing; the
            // returned 0 is never a live entry id (`Graveyard::file` starts
            // at 1) and every caller ignores it.
            let mut id = 0;
            for ((text, spans, kinds, region, whole), origin) in pending {
                id = self.graveyard.file(
                    text,
                    origin_quote.clone(),
                    origin,
                    cut_unix,
                    spans,
                    kinds,
                    region,
                    whole,
                    false,
                );
            }
            if collapse_emptied && self.scraps_textless() {
                self.evaporate_scraps_in_tx();
            }
            return id;
        }
        let rope = self.buffer.rope();
        let start_char = rope.byte_to_char(byte_range.start);
        let end_char = rope.byte_to_char(byte_range.end);
        let (text, spans, kinds, region, whole) = self.grave_capture(start_char, end_char);
        // The non-coalescing edit always opens a fresh transaction, so the
        // pre-cut snapshot is always taken and the filing below rides it.
        if machine {
            self.edit_bytes_verbatim(byte_range, replacement);
        } else {
            self.edit_bytes(byte_range, replacement);
        }
        self.revision += 1;
        let id = self.graveyard.file(
            text,
            origin_quote,
            start_char,
            cut_unix,
            spans,
            kinds,
            region,
            whole,
            false,
        );
        if collapse_emptied && self.scraps_textless() {
            self.evaporate_scraps_in_tx();
        }
        id
    }

    /// The capture half of a graveyard filing, read BEFORE the delete
    /// shifts anything: entry text, its formatting and per-line block
    /// kinds (Bug D / P1 — Put back is lossless), the region it leaves,
    /// and §B1's whole-block detection. Plain delete KEEPS exact-byte
    /// deletion — that ruling stands — but the ENTRY records block-ness
    /// when the selection was in fact one-or-more complete blocks
    /// (papercuts follow-up, report 3): otherwise a backspaced paragraph
    /// returns as a splice and glues to its neighbour. Reuses §B1's
    /// normalization (a range ending at a block's char 0 reclassifies as
    /// ending at the previous block's text end) for the DETECTION only —
    /// a whole-block entry stores the NORMALIZED text (bounding separator
    /// excluded, put_back rebuilds exactly one) so both doors feed §B2
    /// the same shape; a fragment stores the exact deleted chars as ever.
    fn grave_capture(
        &self,
        start_char: usize,
        end_char: usize,
    ) -> (String, SpanSet, Vec<BlockKind>, GraveRegion, bool) {
        let rope = self.buffer.rope();
        let mut norm_end = end_char;
        let trailing_break = break_len_before(rope, norm_end);
        if trailing_break > 0 && norm_end - start_char >= trailing_break {
            norm_end -= trailing_break;
        }
        let first_line = rope.char_to_line(start_char);
        let last_line = rope.char_to_line(norm_end);
        let text_end_of_last = if last_line + 1 < rope.len_lines() {
            line_break_before(rope, rope.line_to_char(last_line + 1))
        } else {
            rope.len_chars()
        };
        let whole = start_char < norm_end
            && start_char == rope.line_to_char(first_line)
            && norm_end == text_end_of_last
            && region_of_char(rope, &self.blocks, start_char)
                == region_of_char(rope, &self.blocks, norm_end);
        let entry_end = if whole { norm_end } else { end_char };
        let text: String = rope.slice(start_char..entry_end).to_string();
        let region = match region_of_char(rope, &self.blocks, start_char) {
            Region::Manuscript => GraveRegion::Manuscript,
            // A cut can't start ON the seam via any verb; the never-panic
            // mapping says the payload lies below it.
            Region::Seam | Region::Scraps => GraveRegion::Scraps,
        };
        // One block kind per line the text will re-create
        // (`count_line_breaks + 1`), so put_back can re-stamp them onto
        // exactly the re-inserted blocks.
        let spans = self.spans.slice(start_char..entry_end);
        let n_blocks = count_line_breaks(&text) + 1;
        let kinds: Vec<BlockKind> = self
            .blocks
            .kinds()
            .iter()
            .skip(first_line)
            .take(n_blocks)
            .cloned()
            .collect();
        (text, spans, kinds, region, whole)
    }

    /// The exile verb's capture (papercuts-2026-07 §B1): interpret the
    /// selection before cutting. A WHOLE-block selection — normalized, running
    /// from a block's text start to a block's text end over one or more
    /// complete blocks — takes its bounding separator along, so the prose is
    /// left with exactly the separator that joined the neighbours and NO empty
    /// grave; a BLANK-LINE separator is consumed whole (the widening Set
    /// aside performs, adjudicated shared — the two verbs still differ by
    /// destination and record), and the entry records it (`blank_sep`) so
    /// Put back restores the separator situation byte-identically. The entry
    /// records `whole_blocks: true` so Put back rebuilds a paragraph (§B2). Anything else (a partial or mixed selection) falls
    /// through to `cut_to_graveyard`'s exact-byte semantics — two verbs, two
    /// contracts (plain delete stays exact-bytes; only exile interprets
    /// intent). Cut and its consumed separator are ONE transaction, so plain
    /// Ctrl+Z restores text + separator and removes the entry in one step.
    /// Returns the new entry id.
    pub fn exile_to_graveyard(
        &mut self,
        byte_range: Range<usize>,
        origin_quote: String,
        cut_unix: i64,
        collapse_emptied: bool,
    ) -> u64 {
        let rope = self.buffer.rope();
        let s = rope.byte_to_char(byte_range.start);
        let mut e = rope.byte_to_char(byte_range.end);
        // Normalize: a selection ending at a block's char 0 (triple-click and
        // shift+down already include the trailing newline) is reclassified as
        // ending at the previous block's text end — otherwise "consume one
        // more" would eat two separators and fuse the neighbours.
        let trailing_break = break_len_before(rope, e);
        if trailing_break > 0 && e - s >= trailing_break {
            e -= trailing_break;
        }
        // Whole-block detection on the normalized range: both ends sit on a
        // block edge, and the two ends live in the same region (the exile verb
        // never spans the seam — its separator math would otherwise chew the
        // seam's own blank line).
        let first_line = rope.char_to_line(s);
        let last_line = rope.char_to_line(e);
        let len_chars = rope.len_chars();
        let text_start_of = |line: usize| rope.line_to_char(line);
        let text_end_of = |line: usize| {
            if line + 1 < rope.len_lines() {
                line_break_before(rope, rope.line_to_char(line + 1))
            } else {
                len_chars
            }
        };
        // `s <= e`, not `s < e`: an EMPTY line that IS a whole block — an
        // image with an empty caption (inline-images §5: the block's text
        // is the caption, and every picture is born captionless) — must go
        // through this door too, or its exile would fall to the byte-range
        // cut below, no-op, and strand the picture. The kind rides the
        // entry, so Put back rebuilds the picture from an empty-text grave.
        let whole = s <= e
            && s == text_start_of(first_line)
            && e == text_end_of(last_line)
            && region_of_char(rope, &self.blocks, s) == region_of_char(rope, &self.blocks, e);
        if !whole {
            // Restore any normalization: the exact-byte path owns the raw
            // selection so a partial cut behaves exactly as today.
            return self.cut_to_graveyard(byte_range, origin_quote, cut_unix, collapse_emptied);
        }

        // Whole-block: the entry holds the block texts (internal separators
        // kept) MINUS the bounding one; the delete takes that bounding
        // separator too. Trailing when a same-region block follows; leading
        // when this is the last block(s) of the region/document.
        let region = match region_of_char(rope, &self.blocks, s) {
            Region::Manuscript => GraveRegion::Manuscript,
            Region::Seam | Region::Scraps => GraveRegion::Scraps,
        };
        let entry_text: String = rope.slice(s..e).to_string();
        let n_blocks = last_line - first_line + 1;
        let kinds: Vec<BlockKind> = self
            .blocks
            .kinds()
            .iter()
            .skip(first_line)
            .take(n_blocks)
            .cloned()
            .collect();
        let spans = self.spans.slice(s..e);
        // A same-region block follows iff the char at `e` is a separator whose
        // next line stays in this region. The separator may be a BLANK LINE
        // (Set aside's widening, adjudicated shared: exiling BBB from
        // "AAA\n\nBBB\n\nCCC" must leave "AAA\n\nCCC" — the prose closes up
        // beneath the departing block, never stacked blanks with the caret
        // stranded on one). Every widened position must stay in this region:
        // the seam's own blank line is a boundary, not a separator, and the
        // widening never chews it. Nor is a TEXTLESS FURNITURE line one —
        // an empty-caption image is a standing block (inline-images §5),
        // and a neighbour's exile must not destroy the picture.
        let my_region = region_of_char(rope, &self.blocks, s);
        let region_of = |ch: usize| region_of_char(rope, &self.blocks, ch);
        let tb1 = break_len_at(rope, e);
        let tb2 = if tb1 > 0 { break_len_at(rope, e + tb1) } else { 0 };
        let trailing_blank = tb2 > 0
            && !self.blocks.kind(last_line + 1).is_furniture()
            && region_of(e + tb1) == my_region
            && region_of(e + tb1 + tb2) == my_region;
        let trailing_sep = tb1 > 0 && region_of(e + tb1) == my_region;
        let lb1 = break_len_before(rope, s);
        let lb2 = if lb1 > 0 { break_len_before(rope, s - lb1) } else { 0 };
        let leading_blank = lb2 > 0
            && !self.blocks.kind(first_line - 1).is_furniture()
            && region_of(s - lb1) == my_region
            && region_of(s - lb1 - lb2) == my_region;
        let (del_start, del_end, origin) = if trailing_blank {
            (s, e + tb1 + tb2, s) // block + its trailing blank-line separator
        } else if trailing_sep {
            (s, e + tb1, s)
        } else if leading_blank {
            // The last block(s) of blank-separated prose: eat the blank line
            // joining us to the block above.
            (s - lb1 - lb2, e, s - lb1 - lb2)
        } else if lb1 > 0 {
            // Leading separator: eat the newline joining us to the block above.
            (s - lb1, e, s - lb1)
        } else {
            // A lone block that is the entire region: no separator to take.
            (s, e, s)
        };
        let blank_sep = trailing_blank || leading_blank;
        let del_byte = self.char_to_byte(del_start)..self.char_to_byte(del_end);
        self.delete_bytes_whole_block(del_byte, first_line);
        // A lone block that is the entire document leaves its LINE standing
        // (the rope's one-line floor): furniture must not survive its own
        // exile as a kind on the empty remnant — the picture would stand
        // twice, live and in the grave (inline-images §5). The remnant is
        // born-again prose; the pre-edit snapshot already covers the kind,
        // so one undo restores text and furniture together.
        if del_start == s && del_end == e && self.blocks.kind(first_line).is_furniture() {
            self.blocks.set_kind(first_line, BlockKind::Paragraph);
        }
        self.revision += 1;
        let id = self.graveyard.file(
            entry_text,
            origin_quote,
            origin,
            cut_unix,
            spans,
            kinds,
            region,
            true,
            blank_sep,
        );
        if collapse_emptied && self.scraps_textless() {
            self.evaporate_scraps_in_tx();
        }
        id
    }

    /// Delete (or type over) a seam-spanning selection: ONE transaction, TWO
    /// edits — above and below the separator, the seam line untouched, "the
    /// seam between the remnants" (seam-mechanics 2). `above`/`below` are
    /// the byte sub-ranges the editor split at the region edges; the
    /// replacement lands MANUSCRIPT-side and the caret returns to the
    /// selection start. Each side that independently clears
    /// `capture_threshold` (chars) files its own region-honest graveyard
    /// entry inside the same atom (graveyard-interplay 6) — a type-over
    /// files like a deletion (§4's amendment: anything big that leaves in
    /// one stroke, survives); `capture: false` (the IME commit, which only
    /// replaces its own preedit) files nothing. If the
    /// below edit empties the pile, evaporation rides the atom too (the
    /// caret ends manuscript-side, so the retype guard does not apply).
    /// Returns the caret char offset (the selection start).
    #[allow(clippy::too_many_arguments)]
    pub fn delete_spanning_seam(
        &mut self,
        above: Range<usize>,
        below: Range<usize>,
        replacement: &str,
        capture: bool,
        capture_threshold: usize,
        origin_quote: String,
        cut_unix: i64,
    ) -> usize {
        let rope = self.buffer.rope();
        let (a_s, a_e) = (rope.byte_to_char(above.start), rope.byte_to_char(above.end));
        let (b_s, b_e) = (rope.byte_to_char(below.start), rope.byte_to_char(below.end));
        let above_text: String = rope.slice(a_s..a_e).to_string();
        let below_text: String = rope.slice(b_s..b_e).to_string();
        // Per-side capture data, taken before anything shifts.
        let side = |s: usize, text: &str| {
            let spans = self.spans.slice(s..s + text.chars().count());
            let first_line = self.buffer.rope().char_to_line(s);
            let n = count_line_breaks(text) + 1;
            let kinds: Vec<BlockKind> = self
                .blocks
                .kinds()
                .iter()
                .skip(first_line)
                .take(n)
                .cloned()
                .collect();
            (spans, kinds)
        };
        let (above_spans, above_kinds) = side(a_s, &above_text);
        let (below_spans, below_kinds) = side(b_s, &below_text);

        self.revision += 1;
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        // Below first, so the above offsets stay valid; then above, carrying
        // the replacement (manuscript-side by law).
        let (block, merged) = self.pre_edit_info(&(below.start..below.end));
        self.buffer.edit_bytes_grouped(below.start..below.end, "");
        self.blocks.on_edit(block, merged, 0);
        let (block, merged) = self.pre_edit_info(&(above.start..above.end));
        self.buffer.edit_bytes_grouped(above.start..above.end, replacement);
        self.blocks.on_edit(block, merged, count_line_breaks(replacement));
        self.absorb_buffer_ops();

        // Region-honest filings, threshold per side (graveyard-interplay 6).
        let repl_chars = replacement.chars().count();
        if capture && a_e - a_s >= capture_threshold && !above_text.is_empty() {
            self.graveyard.file(
                above_text,
                origin_quote.clone(),
                a_s,
                cut_unix,
                above_spans,
                above_kinds,
                GraveRegion::Manuscript,
                false,
                false,
            );
        }
        if capture && b_e - b_s >= capture_threshold && !below_text.is_empty() {
            // The below cut point, in post-edit coordinates (shifted by the
            // above replacement).
            let pos = b_s - (a_e - a_s) + repl_chars;
            self.graveyard.file(
                below_text,
                String::new(),
                pos,
                cut_unix,
                below_spans,
                below_kinds,
                GraveRegion::Scraps,
                false,
                false,
            );
        }
        if self.scraps_textless() {
            self.evaporate_scraps_in_tx();
        }
        a_s
    }

    /// Put an entry back at its re-anchored origin, as one undoable
    /// transaction: re-insert the text, then drop the entry (undo of the
    /// insertion restores the entry via the pre-insert snapshot). The target
    /// is CLAMPED into the region the text was cut FROM (review #62,
    /// re-pointed by graveyard-interplay 1: Put back never crosses the seam
    /// — it returns to the region the text left). A scrap-origin entry whose
    /// pile has evaporated RE-BIRTHS the seam and lands as the sole scrap
    /// (graveyard-interplay 3). Returns the caret char offset after the
    /// re-inserted text (for the paragraph flash), or `None` if the entry is
    /// gone.
    pub fn put_back(&mut self, id: u64) -> Option<usize> {
        let entry = self.graveyard.get(id)?.clone();
        let rope = self.buffer.rope();
        let (at_char, rebirth) = match entry.region {
            GraveRegion::Manuscript => {
                let m = manuscript_range_of(rope, &self.blocks);
                (entry.origin_pos.clamp(m.start, m.end), false)
            }
            GraveRegion::Scraps => match scraps_range_of(rope, &self.blocks) {
                // Either era's pile: return into it.
                Some(s) => (entry.origin_pos.clamp(s.start, s.end), false),
                None => (rope.len_chars(), true),
            },
        };
        let before_seam = self.blocks.scrap_line();
        if !rebirth && entry.whole_blocks {
            // Rebuild the exile as its OWN standing block(s) (papercuts §B2):
            // snap the drifted origin to the nearest block boundary (never
            // mid-paragraph) and reconstruct the one bounding separator, so the
            // text stands alone wearing its own kinds and spans, nothing of the
            // neighbours'. The insertion is verbatim (no neighbour mark dresses
            // it — the machine-insertion belt) and separators + text + kinds +
            // spans ride ONE transaction (the atomic suspenders).
            let region = match entry.region {
                GraveRegion::Manuscript => manuscript_range_of(rope, &self.blocks),
                GraveRegion::Scraps => scraps_range_of(rope, &self.blocks)
                    .unwrap_or_else(|| manuscript_range_of(rope, &self.blocks)),
            };
            let clamped = entry.origin_pos.clamp(region.start, region.end);
            let line = rope.char_to_line(clamped);
            let line_start = rope.line_to_char(line);
            let next_start = if line + 1 < rope.len_lines() {
                rope.line_to_char(line + 1)
            } else {
                rope.len_chars()
            };
            // Nearest of the two block edges bracketing the origin.
            let boundary = if clamped - line_start <= next_start.saturating_sub(clamped) {
                line_start
            } else {
                next_start
            }
            .clamp(region.start, region.end);
            let at_end = boundary >= rope.len_chars();
            let boundary_line = rope.char_to_line(boundary);
            let boundary_start = rope.line_to_char(boundary_line);
            let boundary_end = if boundary_line + 1 < rope.len_lines() {
                line_break_before(rope, rope.line_to_char(boundary_line + 1))
            } else {
                rope.len_chars()
            };
            let empty_line = boundary == boundary_start
                && boundary_start == boundary_end;
            // A trailing separator when a block follows the landing; a leading
            // one when the returned block is the last of the document; none at
            // all into an empty document (no phantom blank block) — or into a
            // trailing EMPTY block (the grave a plain delete leaves standing):
            // the return fills that block rather than opening a second blank.
            // An entry whose exile widened over a BLANK-LINE separator
            // (`blank_sep`) synthesizes the blank line back, so the round
            // trip is byte-identical — at the tail, only the breaks the
            // document is missing are prepended. Synthesized LF can mix
            // with CRLF; Ropey and BlockMap stay aligned.
            let sep = if entry.blank_sep { "\n\n" } else { "\n" };
            let (payload, text_offset) = if rope.len_chars() == 0 || empty_line {
                (entry.text.clone(), 0)
            } else if at_end {
                let b1 = break_len_before(rope, rope.len_chars());
                let b2 = if b1 > 0 {
                    break_len_before(rope, rope.len_chars() - b1)
                } else {
                    0
                };
                let standing = (b1 > 0) as usize + (b2 > 0) as usize;
                let missing = sep.len().saturating_sub(standing);
                (format!("{}{}", &sep[..missing], entry.text), missing)
            } else {
                (format!("{}{}", entry.text, sep), 0)
            };
            let at_byte = rope.char_to_byte(boundary);
            self.edit_bytes_verbatim(at_byte..at_byte, &payload);
            let text_start = boundary + text_offset;
            let insert_block = self.buffer.rope().char_to_line(text_start);
            for (i, kind) in entry.kinds.iter().enumerate() {
                self.blocks.set_kind(insert_block + i, kind.clone());
            }
            for s in entry.spans.spans() {
                self.spans.add(
                    text_start + s.range.start..text_start + s.range.end,
                    s.attr.clone(),
                );
            }
            self.revision += 1;
            self.graveyard.remove(id);
            // Caret at the returned block's START (§B2: reveal the landing).
            return Some(text_start);
        }
        if rebirth {
            // "Emptying dissolves it" gets its inverse: returning re-creates
            // it. The entry lands as the sole scrap under a re-born seam, all
            // one atom (the boundary rides the pre-insert snapshot).
            let lines = self.buffer.rope().len_lines();
            let at_byte = self.buffer.len_bytes();
            let payload = format!("\n\n{}", entry.text);
            self.edit_bytes_verbatim(at_byte..at_byte, &payload);
            let seam = lines;
            self.blocks.set_scrap_line(Some(seam));
            // The seam line is never kind-stamped; the payload re-stamps its
            // own kinds below (first payload line = seam + 1).
            self.blocks.set_kind(seam, BlockKind::Paragraph);
            for (i, kind) in entry.kinds.iter().enumerate() {
                self.blocks.set_kind(seam + 1 + i, kind.clone());
            }
            let text_start = at_char + 2; // past the "\n\n"
            for s in entry.spans.spans() {
                self.spans.add(
                    text_start + s.range.start..text_start + s.range.end,
                    s.attr.clone(),
                );
            }
            self.revision += 1;
            self.graveyard.remove(id);
            self.journal_seam(before_seam);
            return Some(text_start + entry.text.chars().count());
        }
        let at_byte = self.buffer.char_to_byte(at_char);
        // Verbatim insert: the machine-insertion law protects partial entries
        // too — the resurrected splice never absorbs into a neighbour's
        // expanding span before its own spans re-add (papercuts §A3/§B2).
        self.edit_bytes_verbatim(at_byte..at_byte, &entry.text);
        // Re-stamp the cut's block kinds and re-add its spans (Bug D / P1): a
        // heading comes back a heading, bold comes back bold. These ride the
        // transaction `edit_bytes` opened (no new snapshot), so one undo peels
        // the text AND its restored formatting back off together. Kinds land on
        // exactly the re-inserted blocks; spans shift by the insertion offset.
        let insert_block = self.buffer.rope().char_to_line(at_char);
        for (i, kind) in entry.kinds.iter().enumerate() {
            self.blocks.set_kind(insert_block + i, kind.clone());
        }
        for s in entry.spans.spans() {
            self.spans
                .add(at_char + s.range.start..at_char + s.range.end, s.attr.clone());
        }
        self.revision += 1;
        self.graveyard.remove(id);
        Some(at_char + entry.text.chars().count())
    }

    /// Delete an entry outright (the journal still holds the record), as its own
    /// undoable side-state step.
    pub fn grave_delete(&mut self, id: u64) {
        if self.graveyard.get(id).is_none() {
            return;
        }
        self.revision += 1;
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        self.graveyard.remove(id);
    }

    /// Insert `moved` as the NEWEST scrap — just under the seam, newest
    /// nearest the story (08 §2) — within the CURRENTLY OPEN transaction
    /// (the caller has pushed the snapshot). Births the tail boundary when
    /// absent. A blank separator line keeps items from fusing (asides §1's
    /// blank-line item model). `quote_first_line` makes the first inserted
    /// block a `Blockquote` (the orphan-note anchor fragment carries the
    /// margin-note anchor typography). Kinds of the inserted span are reset
    /// to Paragraph (overriding `on_edit`'s inheritance) — the caller
    /// re-stamps captured kinds after; the seam line is NEVER kind-stamped.
    /// Returns the char offset where `moved`'s first char landed.
    fn insert_into_scraps(&mut self, moved: &str, quote_first_line: bool) -> usize {
        let moved_breaks = count_line_breaks(moved);
        let (at_char, payload, text_at, seam, first_block, last_reset) =
            match self.blocks.scrap_line() {
                None => {
                    // Birth: append seam + item at the rope tail. The seam
                    // line lands at the pre-insert line count (the payload's
                    // first break closes the current last line; the second
                    // opens the pile). Reset the seam + the item's lines.
                    let len = self.buffer.rope().len_chars();
                    let seam = self.buffer.rope().len_lines();
                    (
                        len,
                        format!("\n\n{moved}"),
                        len + 2,
                        seam,
                        seam + 1,
                        seam + 1 + moved_breaks,
                    )
                }
                Some(b) => {
                    // Existing pile: the new item lands at the pile's head,
                    // its separator blank line after it. Reset the item's
                    // lines + the new separator (the old first scrap, one
                    // line further, keeps its own kinds).
                    let line = (b + 1).min(self.buffer.rope().len_lines());
                    let at = self.buffer.rope().line_to_char(line);
                    (
                        at,
                        format!("{moved}\n\n"),
                        at,
                        b,
                        b + 1,
                        b + 2 + moved_breaks,
                    )
                }
            };
        let at_byte = self.buffer.char_to_byte(at_char);
        let (block, merged) = self.pre_edit_info(&(at_byte..at_byte));
        self.buffer.edit_bytes_grouped(at_byte..at_byte, &payload);
        self.blocks.on_edit(block, merged, count_line_breaks(&payload));
        // The inserted span (item + separator) is writer prose: reset it to
        // Paragraph, overriding on_edit's kind inheritance. The seam line
        // itself stays Paragraph too — no kind may ever stamp it.
        for i in seam..=last_reset.min(self.blocks.len().saturating_sub(1)) {
            self.blocks.set_kind(i, BlockKind::Paragraph);
        }
        if quote_first_line {
            self.blocks.set_kind(first_block, BlockKind::Blockquote);
        }
        self.blocks.set_scrap_line(Some(seam));
        text_at
    }

    /// `Set aside` — the park/jot verb (08 §2), tail era. Moves a manuscript
    /// selection (or, with `jot`, the caret's paragraph) to just under the
    /// scrap line as ONE undo atom holding everything the adjudications name
    /// (seam-mechanics 4–6): a MOVE, never a cut (the graveyard is exempt by
    /// construction — review H41); spans and block kinds captured and
    /// re-stamped so the text departs losslessly; margin notes whose anchors
    /// live inside the range re-anchor and travel WITH their block (no
    /// second orphan-migration atom); open diagnosis cards on the range
    /// retire (record DELETED after the caller journals CardClosed — deleted
    /// records can never suppress, so a returned passage is re-flaggable) and
    /// writer dismissal records inside it die (machine artifacts never travel
    /// with writer text); a provenance record is filed for selection parks (jots
    /// bear none); the seam's birth is journaled. The caret returns to `s` —
    /// the writer parked a thought, she did not travel.
    ///
    /// ADOPTION (08 §2, its own gesture through the same chord): with no
    /// boundary and a selection that already trails the document, nothing
    /// moves — the seam is born ABOVE the selection (reusing the writer's
    /// own blank divider line when there is one). The tool has learned her
    /// scrap line.
    ///
    /// Returns the park outcome, or `None` when the range is empty, not
    /// entirely manuscript-side (region verbs are single-region;
    /// seam-spanning selections are refused), or the doc is top-era
    /// (conversion is exclusively the migration's).
    pub fn set_aside(
        &mut self,
        byte_range: Range<usize>,
        origin_quote: String,
        parked_unix: i64,
        jot: bool,
    ) -> Option<ParkOutcome> {
        if matches!(self.blocks.boundary(), Some((BoundaryEra::Top, _))) {
            return None;
        }
        let rope = self.buffer.rope();
        let s = rope.byte_to_char(byte_range.start);
        let e = rope.byte_to_char(byte_range.end);
        if s >= e {
            return None;
        }
        let m = self.manuscript_char_range();
        if s < m.start || s > m.end || e > m.end {
            return None; // scraps-side or seam-spanning: not this verb's input
        }
        // Adoption: no boundary + the selection already trails the document.
        let adoptable = self.blocks.boundary().is_none()
            && rope.slice(e..).chars().all(char::is_whitespace)
            && !jot;
        let raw: String = rope.slice(s..e).to_string();
        if adoptable && let Some(outcome) = self.adopt_scraps(s) {
            return Some(outcome);
        }
        let rope = self.buffer.rope(); // re-borrow past the adoption branch
        // Trim edge line breaks off the moved payload: the jot's adjoining
        // newline (and a paragraph selection's trailing one) belongs to the
        // JOIN being deleted, not to the pile item.
        let is_break = |c: char| {
            matches!(
                c,
                '\u{000A}' | '\u{000B}' | '\u{000C}' | '\u{000D}' | '\u{0085}' | '\u{2028}'
                    | '\u{2029}'
            )
        };
        let lead = raw.chars().take_while(|c| is_break(*c)).count();
        let trail = raw.chars().rev().take_while(|c| is_break(*c)).count();
        let (s2, e2) = (s + lead, e - trail);
        if s2 >= e2 {
            return None; // nothing but line breaks
        }
        let moved: String = rope.slice(s2..e2).to_string();
        let spans = self.spans.slice(s2..e2);
        let first_line = rope.char_to_line(s2);
        let n_blocks = count_line_breaks(&moved) + 1;
        let kinds: Vec<BlockKind> = self
            .blocks
            .kinds()
            .iter()
            .skip(first_line)
            .take(n_blocks)
            .cloned()
            .collect();
        // Anchors living entirely inside the moved text, captured relative to
        // s2 so they can be re-anchored inside the pile after the move.
        let travelling: Vec<(u64, usize, usize)> = self
            .notes
            .notes
            .iter()
            .filter(|n| {
                n.kind == NoteKind::Note
                    && n.status == NoteStatus::Open
                    && n.range.start >= s2
                    && n.range.end <= e2
                    && n.range.start < n.range.end
            })
            .map(|n| (n.id, n.range.start - s2, n.range.end - s2))
            .collect();
        let retired: Vec<u64> = self
            .notes
            .notes
            .iter()
            .filter(|n| {
                n.kind == NoteKind::Diagnosis
                    && n.status == NoteStatus::Open
                    && n.range.start >= s2
                    && n.range.end <= e2
            })
            .map(|n| n.id)
            .collect();
        let dead_dismissals: Vec<u64> = self
            .notes
            .notes
            .iter()
            .filter(|n| {
                n.kind == NoteKind::Diagnosis
                    && n.status == NoteStatus::Dismissed
                    && n.range.start >= s2
                    && n.range.end <= e2
            })
            .map(|n| n.id)
            .collect();

        let before_seam = self.blocks.scrap_line();
        self.revision += 1;
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        // Delete the prose (grouped so the following insert shares the step).
        let (block, merged) = self.pre_edit_info(&byte_range);
        self.buffer.edit_bytes_grouped(byte_range, "");
        self.blocks.on_edit(block, merged, 0);
        let text_at = self.insert_into_scraps(&moved, false);
        self.absorb_buffer_ops();
        // Re-stamp the captured formatting and structure (lossless departure,
        // seam-mechanics 5): kinds onto exactly the re-inserted blocks —
        // never the seam line, which insert_into_scraps keeps Paragraph —
        // and spans at the landing offset.
        let insert_block = self.buffer.rope().char_to_line(text_at);
        for (i, kind) in kinds.iter().enumerate() {
            self.blocks.set_kind(insert_block + i, kind.clone());
        }
        for sp in spans.spans() {
            self.spans
                .add(text_at + sp.range.start..text_at + sp.range.end, sp.attr.clone());
        }
        // Notes travel with their block, INSIDE the atom (the apply_op pass
        // collapsed them onto the cut point; re-point them at the moved text).
        for (id, rs, re) in &travelling {
            if let Some(n) = self.notes.notes.iter_mut().find(|n| n.id == *id) {
                n.range = text_at + rs..text_at + re;
                n.orphaned = false;
            }
        }
        self.notes.notes.sort_by_key(|n| n.range.start);
        // Card retirement rides the same atom (a notes mutation inside the
        // open transaction — ctrl-Z resurrects the card for free). The record
        // is DELETED, not re-statused: deletion can never suppress a future
        // pass, and old builds keep parsing the annotations whole.
        for id in &retired {
            self.notes.remove(*id);
        }
        for id in &dead_dismissals {
            self.notes.remove(*id);
        }
        // Provenance: a range-anchored side record for selection parks; jots
        // create none (seam-mechanics 7).
        if !jot {
            self.provenance
                .add(text_at..text_at + moved.chars().count(), origin_quote, parked_unix);
        }
        self.journal_seam(before_seam);
        Some(ParkOutcome {
            caret: s.min(self.buffer.rope().len_chars()),
            retired,
            adopted: false,
        })
    }

    /// `Move to the manuscript` — the retrieval verb's MODEL half (08 §2;
    /// seam-mechanics 4: "Move to the manuscript carries them home the same
    /// way"). Moves a Scraps selection to `dest_char` in the manuscript as
    /// ONE atom: spans and kinds captured and re-stamped, contained writer
    /// notes re-anchored to travel home, the provenance record covering the
    /// moved text dying with it (its text left the pile — `apply_op`'s
    /// collapse-drop runs inside the same transaction), one adjoining blank
    /// separator absorbed so the pile strands no empty item slot, and an
    /// emptied pile evaporating in the same transaction. Wave B wires the
    /// verb surface, the latch destination, and arriving-selected. Returns
    /// the landed char range (for the selection), or `None` when the range
    /// is empty or not entirely pile-side.
    pub fn move_to_manuscript(
        &mut self,
        byte_range: Range<usize>,
        dest_char: usize,
    ) -> Option<Range<usize>> {
        let Some((BoundaryEra::Tail, _)) = self.blocks.boundary() else {
            return None;
        };
        let rope = self.buffer.rope();
        let s = rope.byte_to_char(byte_range.start);
        let e = rope.byte_to_char(byte_range.end);
        let pile = self.scraps_char_range()?;
        if s >= e || s < pile.start || e > pile.end {
            return None;
        }
        let is_break = |c: char| {
            matches!(
                c,
                '\u{000A}' | '\u{000B}' | '\u{000C}' | '\u{000D}' | '\u{0085}' | '\u{2028}'
                    | '\u{2029}'
            )
        };
        let raw: String = rope.slice(s..e).to_string();
        let lead = raw.chars().take_while(|c| is_break(*c)).count();
        let trail = raw.chars().rev().take_while(|c| is_break(*c)).count();
        let (s2, e2) = (s + lead, e - trail);
        if s2 >= e2 {
            return None;
        }
        let moved: String = rope.slice(s2..e2).to_string();
        let spans = self.spans.slice(s2..e2);
        let first_line = rope.char_to_line(s2);
        let n_blocks = count_line_breaks(&moved) + 1;
        let kinds: Vec<BlockKind> = self
            .blocks
            .kinds()
            .iter()
            .skip(first_line)
            .take(n_blocks)
            .cloned()
            .collect();
        // Widen the delete over the item's blank separator — the trailing
        // one when a successor exists, else the leading one — so no empty
        // slot strands between neighbours (asides §1's item grammar).
        let (mut del_s, mut del_e) = (s.min(s2), e.max(e2));
        if del_e + 2 <= pile.end && rope.char(del_e) == '\n' && rope.char(del_e + 1) == '\n' {
            del_e += 2;
        } else if del_s >= pile.start + 2
            && rope.char(del_s - 1) == '\n'
            && rope.char(del_s - 2) == '\n'
        {
            del_s -= 2;
        }
        let travelling: Vec<(u64, usize, usize)> = self
            .notes
            .notes
            .iter()
            .filter(|n| {
                n.kind == NoteKind::Note
                    && n.status == NoteStatus::Open
                    && n.range.start >= s2
                    && n.range.end <= e2
                    && n.range.start < n.range.end
            })
            .map(|n| (n.id, n.range.start - s2, n.range.end - s2))
            .collect();
        let dest = dest_char.min(self.manuscript_char_range().end);

        let before_seam = self.blocks.scrap_line();
        self.revision += 1;
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        // Delete from the pile first (it sits AFTER the manuscript, so the
        // destination's position is untouched), then insert at the home spot.
        let del_sb = self.buffer.char_to_byte(del_s);
        let del_eb = self.buffer.char_to_byte(del_e);
        let (block, merged) = self.pre_edit_info(&(del_sb..del_eb));
        self.buffer.edit_bytes_grouped(del_sb..del_eb, "");
        self.blocks.on_edit(block, merged, 0);
        let dest_b = self.buffer.char_to_byte(dest);
        let (block, merged) = self.pre_edit_info(&(dest_b..dest_b));
        self.buffer.edit_bytes_grouped(dest_b..dest_b, &moved);
        self.blocks
            .on_edit(block, merged, count_line_breaks(&moved));
        self.absorb_buffer_ops();
        // Re-stamp the captured structure at the landing.
        let insert_block = self.buffer.rope().char_to_line(dest);
        for (i, kind) in kinds.iter().enumerate() {
            self.blocks.set_kind(insert_block + i, kind.clone());
        }
        for sp in spans.spans() {
            self.spans
                .add(dest + sp.range.start..dest + sp.range.end, sp.attr.clone());
        }
        // The notes travel home inside the atom.
        for (id, rs, re) in &travelling {
            if let Some(n) = self.notes.notes.iter_mut().find(|n| n.id == *id) {
                n.range = dest + rs..dest + re;
                n.orphaned = false;
            }
        }
        self.notes.notes.sort_by_key(|n| n.range.start);
        // A pile this move emptied dissolves in the same transaction.
        if self.scraps_textless() {
            self.evaporate_scraps_in_tx();
        }
        self.journal_seam(before_seam);
        Some(dest..dest + moved.chars().count())
    }

    /// The adoption gesture's mechanism (08 §2 "Adoption & migration" path
    /// 1): the writer's own trailing divider pile becomes Scraps in place —
    /// nothing moves; the seam is her own blank divider line above the
    /// selection. Returns `None` (fall through to an ordinary park) when
    /// there is no blank divider to adopt: without one this is just a park
    /// of trailing text, with a park's full semantics. Open diagnoses whose
    /// anchors land below the new seam retire (AI cards never anchor below
    /// it). One transaction, journaled. `s` is the selection start (chars).
    fn adopt_scraps(&mut self, s: usize) -> Option<ParkOutcome> {
        let rope = self.buffer.rope();
        let first_block = rope.char_to_line(s);
        if first_block < 2 {
            return None; // need a manuscript block AND a divider above
        }
        let above = first_block - 1;
        if !rope.line(above).chars().all(char::is_whitespace) {
            return None; // no divider: an ordinary park
        }
        let before_seam = self.blocks.scrap_line();
        self.revision += 1;
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        self.blocks.set_scrap_line(Some(above));
        // The clamp can refuse a degenerate shape: the empty transaction
        // then stands as a no-op step, and no adoption is reported.
        self.blocks.scrap_line()?;
        // The new pile may carry open machine cards: retire them (the
        // gating law — no cool card anchors below the seam).
        let pile_start = self
            .scraps_char_range()
            .map(|r| r.start)
            .unwrap_or(usize::MAX);
        let retired: Vec<u64> = self
            .notes
            .notes
            .iter()
            .filter(|n| {
                n.kind == NoteKind::Diagnosis
                    && n.status == NoteStatus::Open
                    && n.range.start >= pile_start
            })
            .map(|n| n.id)
            .collect();
        for id in &retired {
            self.notes.remove(*id);
        }
        self.journal_seam(before_seam);
        Some(ParkOutcome {
            caret: s.min(self.buffer.rope().len_chars()),
            retired,
            adopted: true,
        })
    }

    /// Land an orphaned WRITER note's text in the Scraps pile (asides.md
    /// §2.3; spec §3): a quoted anchor line (`Blockquote`) plus the body
    /// paragraph. The note is removed from the margin in the SAME undoable
    /// step. A move, not a cut (no graveyard). The caller resolves
    /// `CardFocus` first if the note is active (review B5). Diagnoses never
    /// migrate. (The identifier keeps the compost_ name — pure-churn rename.)
    pub fn migrate_note_to_compost(&mut self, note_id: u64, anchor_fragment: &str) -> bool {
        let Some(note) = self.notes.get(note_id) else {
            return false;
        };
        if note.kind != NoteKind::Note {
            return false; // machine cards are not writer material
        }
        if matches!(self.blocks.boundary(), Some((BoundaryEra::Top, _))) {
            return false; // top-era docs migrate wholesale, never piecemeal
        }
        let body = note.body.clone();
        // Anchor fragment on its own line, then the body — flattened so a
        // multi-line body stays one item (the pile's item grammar is
        // blank-line separated; internal newlines would split it).
        let anchor = anchor_fragment.replace('\n', " ");
        let body = body.replace('\n', " ");
        let moved = format!("{anchor}\n{body}");
        let before_seam = self.blocks.scrap_line();
        self.revision += 1;
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        self.insert_into_scraps(&moved, true);
        self.absorb_buffer_ops();
        self.notes.remove(note_id);
        self.journal_seam(before_seam);
        true
    }

    /// Add an author note as its own undoable transaction.
    pub fn add_note(&mut self, range: Range<usize>, body: String, created_unix: i64) -> u64 {
        self.revision += 1;
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        let id = self.notes.add(range, body, created_unix);
        self.record_card_raised(id);
        id
    }

    pub fn set_note_status(&mut self, id: u64, status: NoteStatus) {
        self.revision += 1;
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        self.notes.set_status(id, status);
    }

    pub fn set_note_body(&mut self, id: u64, body: String) {
        self.revision += 1;
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        self.notes.set_body(id, body);
        self.record_card_edited(id);
    }

    /// Mirror an in-progress composer draft onto the note without disturbing
    /// the undo stack: the keystroke autosave path (Editor heartbeat) writes
    /// here every tick so a crash mid-compose never loses the draft, while
    /// undo boundaries stay tied to the Enter-commit in `set_note_body`.
    pub fn set_note_body_draft(&mut self, id: u64, body: String) {
        self.revision += 1;
        self.notes.set_body(id, body);
    }

    /// Current persisted body of a note, for change-detection on the draft
    /// autosave path (skip the write — and the dirty flag — when unchanged).
    pub fn note_body(&self, id: u64) -> Option<&str> {
        self.notes.get(id).map(|n| n.body.as_str())
    }

    /// Remove a note outright (LAW 2 / C4 empty-discard: a never-written note the
    /// writer clicked away from must not persist as a blank card). No undo
    /// snapshot of its own — the `add_note` that created it already pushed one,
    /// so the pair leaves the stack where it found it.
    pub fn remove_note(&mut self, id: u64) -> Option<Annotation> {
        self.revision += 1;
        self.notes.remove(id)
    }

    /// Cancel a never-written note (LAW 2 / C4 empty-discard), removing its
    /// paired empty undo step when still topmost and otherwise scrubbing every
    /// history snapshot so no later undo or redo can resurrect the blank card.
    pub fn cancel_provisional_note(&mut self, id: u64) -> Option<Annotation> {
        self.revision += 1;
        let removed = self.notes.remove(id)?;
        let paired_add_is_top = self
            .undo_states
            .last()
            .is_some_and(|(_, _, notes, _, _)| notes.get(id).is_none());
        if paired_add_is_top && self.buffer.pop_empty_transaction() {
            self.undo_states.pop();
        } else {
            for (_, _, notes, _, _) in &mut self.undo_states {
                notes.remove(id);
            }
            for (_, _, notes, _, _) in &mut self.redo_states {
                notes.remove(id);
            }
        }
        Some(removed)
    }

    /// Add a batch of diagnoses as ONE undoable transaction (one ctrl-z
    /// clears a whole pass).
    pub fn add_diagnoses(&mut self, diagnoses: Vec<Annotation>) {
        if diagnoses.is_empty() {
            return;
        }
        self.revision += 1;
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        for d in diagnoses {
            let id = self.notes.push(d);
            self.record_card_raised(id);
        }
    }

    fn record_card_raised(&mut self, id: u64) {
        let Some(n) = self.notes.get(id).cloned() else { return };
        self.journal.record_event(crate::journal::JournalEvent::CardRaised {
            t: crate::journal::now_ms(),
            id: n.id,
            card_kind: n.kind,
            range: n.range,
            body: n.body,
            title: n.title,
            level: n.level,
            pass_id: n.pass_id,
            status: n.status,
            orphaned: n.orphaned,
            unverified: n.unverified,
        });
    }

    fn record_card_edited(&mut self, id: u64) {
        let Some(n) = self.notes.get(id).cloned() else { return };
        let latest_body = self.journal.events.iter().rev().find_map(|event| match event {
            crate::journal::JournalEvent::CardRaised { id: event_id, body, .. }
            | crate::journal::JournalEvent::CardEdited { id: event_id, body, .. }
                if *event_id == id => Some(body.as_str()),
            _ => None,
        });
        if latest_body == Some(n.body.as_str()) {
            return;
        }
        self.journal.record_event(crate::journal::JournalEvent::CardEdited {
            t: crate::journal::now_ms(),
            id: n.id,
            body: n.body,
            title: n.title,
            level: n.level,
            pass_id: n.pass_id,
            status: n.status,
            orphaned: n.orphaned,
            unverified: n.unverified,
        });
    }

    /// The whole-block doors' own splice (the exile verb's delete): the §2
    /// clamp gates ARBITRARY ranges — and deliberately demands full
    /// enclosure before it lets an EMPTY furniture line go, so a stray
    /// range can never silently drain a captionless picture. A door that
    /// has already resolved its range to a whole block plus exactly ONE
    /// bounding separator (inline-images §5) is the deliberate verb that
    /// law protects, so it splices past the gate — keeping the clamped
    /// executor's restamp discipline: a cut that takes a furniture line by
    /// its following separator leaves the furniture kind on the merged
    /// line under `on_edit`'s merge-keeps-first, and the first surviving
    /// block's kind is restamped over it, captured before the drain.
    ///
    /// `taken` is the exiled block's own line, known to the door — the
    /// restamp fires only when the cut STARTS on that block (the
    /// trailing-separator form). Geometry alone cannot decide: a
    /// leading-separator cut starts on the separator byte, and when the
    /// block above is furniture with an EMPTY caption that byte IS its
    /// line's start too — the geometric guard misread the neighbour as
    /// the taken block and restamped the surviving picture away (§5).
    fn delete_bytes_whole_block(&mut self, byte_range: Range<usize>, taken: usize) {
        let (block, merged) = self.pre_edit_info(&byte_range);
        let restamp = (merged > 0
            && block == taken
            && self.blocks.kind(block).is_furniture()
            && byte_range.start == self.buffer.rope().line_to_byte(block))
            .then(|| self.blocks.kind(block + merged).clone());
        self.revision += 1;
        if self.buffer.edit_bytes(byte_range, "") {
            // Snapshot AFTER the buffer edit, BEFORE on_edit: spans/blocks/
            // notes are still pre-edit here (edit_bytes' own pattern).
            self.undo_states.push(self.snapshot());
            self.redo_states.clear();
        }
        self.blocks.on_edit(block, merged, 0);
        if let Some(kind) = restamp {
            self.blocks.set_kind(block, kind);
        }
        self.absorb_buffer_ops();
    }

    /// The §2 wall gate, priced for the hot path: an edit clamps only when
    /// it deletes at least one line break AND some block in its pre-edit
    /// line span is furniture. The common keystroke pays one `merged == 0`
    /// branch; only multi-line deletions pay the kind scan.
    fn crosses_furniture_wall(&self, block: usize, merged: usize) -> bool {
        if merged == 0 {
            return false;
        }
        let kinds = self.blocks.kinds();
        let lo = block.min(kinds.len());
        let hi = (block + merged + 1).min(kinds.len());
        kinds[lo..hi].iter().any(BlockKind::is_furniture)
    }

    /// Execute a wall-clamped edit (§2 wall law; build plan R1): the
    /// deletion decomposes per `clamp_plan` and runs as grouped sub-edits
    /// in ONE transaction — undo restores the pre-edit state in a single
    /// step, exactly like the aside move — with the replacement text (if
    /// any) landing at the range start's side. A cut that takes a
    /// furniture line by its FOLLOWING separator keeps that line's kind
    /// under `on_edit`'s merge-keeps-first, so the merged line is
    /// restamped to the first surviving block's kind, captured before the
    /// drain. A fully refused range (separators only) mutates nothing and
    /// leaves no undo step behind.
    fn edit_bytes_clamped(&mut self, byte_range: &Range<usize>, text: &str, by_typing: bool) {
        let plan = clamp_plan(self.buffer.rope(), &self.blocks, byte_range);
        if plan.cuts.is_empty() && text.is_empty() {
            return;
        }
        self.revision += 1;
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        for cut in plan.cuts.iter().rev() {
            let (block, merged) = self.pre_edit_info(cut);
            let restamp = (merged > 0
                && self.blocks.kind(block).is_furniture()
                && cut.start == self.buffer.rope().line_to_byte(block))
                .then(|| self.blocks.kind(block + merged).clone());
            self.buffer.edit_bytes_grouped(cut.clone(), "");
            self.blocks.on_edit(block, merged, 0);
            if let Some(kind) = restamp {
                self.blocks.set_kind(block, kind);
            }
        }
        if !text.is_empty() {
            let at = plan.insert_at..plan.insert_at;
            let (block, _) = self.pre_edit_info(&at);
            self.buffer.edit_bytes_grouped(at, text);
            self.blocks.on_edit(block, 0, count_line_breaks(text));
        }
        if by_typing {
            self.absorb_buffer_ops();
        } else {
            self.absorb_buffer_ops_verbatim();
        }
    }

    pub fn edit_bytes(&mut self, byte_range: Range<usize>, text: &str) {
        if byte_range.is_empty() && text.is_empty() {
            return;
        }
        let (block, merged) = self.pre_edit_info(&byte_range);
        if self.crosses_furniture_wall(block, merged) {
            self.edit_bytes_clamped(&byte_range, text, true);
            return;
        }
        self.revision += 1;
        if self.buffer.edit_bytes(byte_range, text) {
            // The buffer edit mutates only the rope + its own undo stack;
            // spans/blocks/notes stay pre-edit until on_edit/absorb_buffer_ops
            // run below, so snapshotting here captures the same pre-edit
            // side-state as before — but only when a transaction opens.
            self.undo_states.push(self.snapshot());
            self.redo_states.clear();
        }
        let splits = count_line_breaks(text);
        if merged != 0 || splits != 0 { self.blocks.on_edit(block, merged, splits); }
        self.absorb_buffer_ops();
    }

    /// Like `edit_bytes`, but the insertion is MACHINE-performed (put back,
    /// paste): spans re-anchor verbatim, without right-edge expansion (the A3
    /// machine-insertion law). Used so a resurrected passage lands wearing its
    /// OWN marks, never the neighbour's.
    pub fn edit_bytes_verbatim(&mut self, byte_range: Range<usize>, text: &str) {
        if byte_range.is_empty() && text.is_empty() {
            return;
        }
        let (block, merged) = self.pre_edit_info(&byte_range);
        if self.crosses_furniture_wall(block, merged) {
            self.edit_bytes_clamped(&byte_range, text, false);
            return;
        }
        self.revision += 1;
        if self.buffer.edit_bytes(byte_range, text) {
            self.undo_states.push(self.snapshot());
            self.redo_states.clear();
        }
        let splits = count_line_breaks(text);
        if merged != 0 || splits != 0 { self.blocks.on_edit(block, merged, splits); }
        self.absorb_buffer_ops_verbatim();
    }

    pub fn edit_bytes_coalescing(&mut self, byte_range: Range<usize>, text: &str) {
        if byte_range.is_empty() && text.is_empty() {
            return;
        }
        let (block, merged) = self.pre_edit_info(&byte_range);
        if self.crosses_furniture_wall(block, merged) {
            // A wall IS a run boundary: the clamped edit never coalesces.
            self.edit_bytes_clamped(&byte_range, text, true);
            return;
        }
        self.revision += 1;
        if self.buffer.edit_bytes_coalescing(byte_range, text) {
            // Snapshot only when a new transaction actually opens. While
            // typing inside a word the buffer coalesces and returns false, so
            // the full SpanSet+BlockMap+Annotations clone is skipped on the
            // ~5-of-6 mid-word keystrokes it used to be allocated and dropped
            // on. Pre-edit side-state is intact here (see edit_bytes).
            self.undo_states.push(self.snapshot());
            self.redo_states.clear();
        }
        let splits = count_line_breaks(text);
        if merged != 0 || splits != 0 { self.blocks.on_edit(block, merged, splits); }
        self.absorb_buffer_ops();
    }

    /// Toggle `attr` over a char range as its own undoable transaction.
    pub fn toggle_format(&mut self, range: Range<usize>, attr: InlineAttr) {
        self.revision += 1;
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        if self.spans.covers(range.clone(), &attr) {
            self.spans.remove(range, &attr);
        } else {
            self.spans.add(range, attr);
        }
    }

    /// Set (or, with `url` empty/`None`, clear) a hyperlink over `range`, as one
    /// undoable transaction. Unlike `toggle_format`, this REPLACES: any existing
    /// link over the range is dropped first regardless of its target, so editing
    /// a link never strands a stale overlapping span (`SpanSet::remove` is
    /// URL-exact, so the old targets are gathered first). The selection flank's
    /// link cell is the only caller (docs/impl/03-flanks.md §0.1, review 88).
    pub fn set_link(&mut self, range: Range<usize>, url: Option<String>) {
        if range.start >= range.end {
            return;
        }
        self.revision += 1;
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        let mut olds: Vec<String> = self
            .spans
            .spans()
            .iter()
            .filter(|s| s.range.start < range.end && range.start < s.range.end)
            .filter_map(|s| match &s.attr {
                InlineAttr::Link(u) => Some(u.clone()),
                _ => None,
            })
            .collect();
        olds.sort();
        olds.dedup();
        for u in olds {
            self.spans.remove(range.clone(), &InlineAttr::Link(u));
        }
        if let Some(url) = url.filter(|u| !u.is_empty()) {
            self.spans.add(range, InlineAttr::Link(url));
        }
    }

    /// The hyperlink target covering `range`, if any — the first overlapping
    /// `Link` span. The flank pre-fills its URL field from this so editing an
    /// existing link shows its current target (docs/impl/03-flanks.md §0.1).
    pub fn link_over(&self, range: Range<usize>) -> Option<String> {
        self.spans.spans().iter().find_map(|s| match &s.attr {
            InlineAttr::Link(u) if s.range.start < range.end && range.start < s.range.end => {
                Some(u.clone())
            }
            _ => None,
        })
    }

    /// Set a block's kind as its own undoable transaction.
    pub fn set_block_kind(&mut self, block: usize, kind: BlockKind) {
        self.revision += 1;
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        self.blocks.set_kind(block, kind);
    }

    /// Change a block's kind inside the current transaction (rides a text
    /// edit, e.g. the `# `-shortcut or Enter-at-heading-end).
    pub fn set_block_kind_in_current_tx(&mut self, block: usize, kind: BlockKind) {
        self.revision += 1;
        self.blocks.set_kind(block, kind);
    }

    /// Insert a dropped/pasted image as its OWN standing block after the
    /// block holding `cursor_byte` (papercuts follow-up, report 4): an image
    /// is never spliced mid-paragraph, and the caret must never land ON it —
    /// typing at the image's line would write invisible text into the image
    /// block. Separators are ensured (a trailing empty paragraph is opened
    /// when the image lands last) and the returned caret BYTE is the start
    /// of the block FOLLOWING the image. One transaction: the separator
    /// insert and the kind stamp undo together.
    pub fn insert_image_block(&mut self, cursor_byte: usize, src: String) -> usize {
        self.insert_image_block_full(cursor_byte, src, String::new(), "", &SpanSet::default())
    }

    /// `insert_image_block` carrying the §9 travelling form's luggage: the
    /// rebuilt picture lands with its alt AND its caption — text and spans
    /// both — in the same one transaction, so a cross-clipboard rebuild is
    /// one undo step exactly like a bare insert. `caption` must be one
    /// line (the paste grammar guarantees it); `caption_spans` are
    /// char-based, rebased to the caption's own start.
    pub fn insert_image_block_full(
        &mut self,
        cursor_byte: usize,
        src: String,
        alt: String,
        caption: &str,
        caption_spans: &SpanSet,
    ) -> usize {
        debug_assert!(!caption.contains('\n'), "a caption is one line by grammar");
        let rope = self.buffer.rope();
        let cursor_char = rope.byte_to_char(cursor_byte.min(self.len_bytes()));
        let line = rope.char_to_line(cursor_char);
        let par_end_char = if line + 1 < rope.len_lines() {
            rope.line_to_char(line + 1) - 1
        } else {
            rope.len_chars()
        };
        let has_following = par_end_char < rope.len_chars();
        let par_end_byte = rope.char_to_byte(par_end_char);
        // "\n" opens the image's own block (its line text = the caption)
        // between this paragraph and the next; when nothing follows, the
        // trailing "\n" ALSO opens the empty paragraph the caret needs to
        // stand in (§7: there is always an after).
        let payload = if has_following {
            format!("\n{caption}")
        } else {
            format!("\n{caption}\n")
        };
        self.edit_bytes(par_end_byte..par_end_byte, &payload);
        let image_block = line + 1;
        self.set_block_kind_in_current_tx(image_block, BlockKind::Image { src, alt });
        let cap_start = self.buffer.rope().line_to_char(image_block);
        for s in caption_spans.spans() {
            self.format_in_current_tx(
                cap_start + s.range.start..cap_start + s.range.end,
                s.attr.clone(),
                true,
            );
        }
        // Start of the block after the image: past the paragraph's end, the
        // separator that opened the image block, and the caption line.
        self.buffer.rope().line_to_byte(image_block + 1)
    }

    /// The §7 drop law's topmost gap: an image standing BEFORE the first
    /// block. `insert_image_block` can only stand a block after the one
    /// holding a byte, so the top gap gets its own splice: one "\n" opens
    /// the new first line, then BOTH kinds are stamped deterministically —
    /// the new line the picture, the pushed-down line its old kind — so
    /// neither split law (flowing clones; furniture births Paragraph) can
    /// misdress either side. One transaction; returns the byte after the
    /// image block (the old first block's new start).
    pub fn insert_image_block_before_first(&mut self, src: String) -> usize {
        let old_kind = self.blocks.kind(0).clone();
        self.edit_bytes(0..0, "\n");
        self.set_block_kind_in_current_tx(
            0,
            BlockKind::Image {
                src,
                alt: String::new(),
            },
        );
        self.set_block_kind_in_current_tx(1, old_kind);
        1
    }

    /// Swap a picture's pixels in place (docs/inline-images.md §4's
    /// replace-in-place, plan R4) — the "better export" verb. Same block,
    /// alt untouched; the caption is the block's own line, so it is
    /// untouched by construction. One undo step, via the `set_block_kind`
    /// snapshot path. A non-image block is a no-op: the caller's picture
    /// selection may have decayed under an async import.
    pub fn replace_image_src(&mut self, block: usize, src: String) {
        let BlockKind::Image { alt, .. } = self.blocks.kind(block).clone() else {
            return;
        };
        self.set_block_kind(block, BlockKind::Image { src, alt });
    }

    /// Apply/clear an attribute inside the *current* transaction (sticky
    /// caret formatting riding a typing transaction) — undone together
    /// with the typed text.
    pub fn format_in_current_tx(&mut self, range: Range<usize>, attr: InlineAttr, on: bool) {
        self.revision += 1;
        if on {
            self.spans.add(range, attr);
        } else {
            self.spans.remove(range, &attr);
        }
    }

    /// Undo one transaction (text, formatting, and block kinds together).
    /// Outer None = nothing to undo; inner None = format-only (keep cursor).
    pub fn undo(&mut self) -> Option<Option<usize>> {
        let had_transaction = self.buffer.undo_len() != 0;
        let Some(cursor) = self.buffer.undo() else {
            if had_transaction { self.undo_states.clear(); }
            return None;
        };
        self.revision += 1;
        let before_seam = self.blocks.scrap_line();
        if let Some((spans, blocks, notes, graveyard, provenance)) = self.undo_states.pop() {
            self.redo_states.push((
                std::mem::replace(&mut self.spans, spans),
                std::mem::replace(&mut self.blocks, blocks),
                std::mem::replace(&mut self.notes, notes),
                std::mem::replace(&mut self.graveyard, graveyard),
                std::mem::replace(&mut self.provenance, provenance),
            ));
            self.normalize_side_structures();
        }
        // Buffer inverse ops still mirror to the store, but must NOT be
        // re-applied to spans/blocks (the snapshot is the correct state).
        // They DO journal — an undo is an honest edit and the envelope
        // visibly steps back. A boundary change (undo of a park evaporating
        // a just-born seam) records its Seam event like any other mutation.
        let ops = self.buffer.take_ops();
        let now = crate::journal::now_ms();
        for (op, deleted) in &ops {
            self.journal.record_deleted(op, now, deleted);
        }
        self.pending_ops.extend(ops.into_iter().map(|(op, _)| op));
        self.journal_seam(before_seam);
        Some(cursor)
    }

    pub fn redo(&mut self) -> Option<Option<usize>> {
        let had_transaction = self.buffer.redo_len() != 0;
        let Some(cursor) = self.buffer.redo() else {
            if had_transaction { self.redo_states.clear(); }
            return None;
        };
        self.revision += 1;
        let before_seam = self.blocks.scrap_line();
        if let Some((spans, blocks, notes, graveyard, provenance)) = self.redo_states.pop() {
            self.undo_states.push((
                std::mem::replace(&mut self.spans, spans),
                std::mem::replace(&mut self.blocks, blocks),
                std::mem::replace(&mut self.notes, notes),
                std::mem::replace(&mut self.graveyard, graveyard),
                std::mem::replace(&mut self.provenance, provenance),
            ));
            self.normalize_side_structures();
        }
        let ops = self.buffer.take_ops();
        let now = crate::journal::now_ms();
        for (op, deleted) in &ops {
            self.journal.record_deleted(op, now, deleted);
        }
        self.pending_ops.extend(ops.into_iter().map(|(op, _)| op));
        self.journal_seam(before_seam);
        Some(cursor)
    }

    /// Replace the whole document state as ONE undoable transaction —
    /// checkpoint restore semantics: rewinding is a forward edit, history
    /// stays append-only, and ctrl-z takes you back to the present.
    ///
    /// CROSS-ERA RESTORE NORMALIZES (time-persistence 3): a Top-era incoming
    /// state is flipped through `flip_state` first — membership preserved
    /// (07 N3's "never teleports" = no block changes sides), position
    /// normalized — so live top geometry never re-enters an editor whose
    /// guards and verbs are tail-era. The past's own geometry is what the
    /// PREVIEW shows; restore materializes tail-era.
    pub fn restore_state(&mut self, text: &str, spans: SpanSet, blocks: BlockMap) {
        let (text, spans, blocks) = match blocks.boundary() {
            Some((BoundaryEra::Top, _)) if blocks.len() == count_line_breaks(text) + 1 => {
                flip_state(text, &spans, &blocks)
            }
            _ => (text.to_owned(), spans, blocks),
        };
        let text = text.as_str();
        self.revision += 1;
        let before_seam = self.blocks.scrap_line();
        let snapshot = self.snapshot();
        // Re-anchor notes by content (the least-surprising restore semantics):
        // each live note follows the passage it covers into the restored text;
        // a note whose passage is gone detaches honestly instead of piling at
        // the document end. Computed against the OLD buffer text — captured
        // here, before the wholesale swap erases it — then installed after.
        // Provenance re-anchors the same way, except a record whose passage
        // vanished dies (records describe text, never orphan).
        let old_text = self.buffer.text();
        let mut reanchored = self.notes.clone();
        reanchored.reanchor(&old_text, text);
        let mut prov = self.provenance.clone();
        prov.reanchor(&old_text, text);
        // The graveyard is a record of cuts, not tied to any live passage, so
        // it simply survives the swap. Preserve the pre-swap entries here — the
        // wholesale op below would otherwise pin every origin_pos to the tail —
        // and re-clamp them to the new length after (Put back re-clamps into
        // the entry's own region anyway).
        let saved_graveyard = self.graveyard.clone();

        let len = self.buffer.len_bytes();
        // The wholesale swap must not journal as one document-sized run;
        // the caller records the honest Restore event instead.
        self.journal.pause();
        if self.buffer.edit_bytes(0..len, text) {
            self.undo_states.push(snapshot);
            self.redo_states.clear();
        }
        self.absorb_buffer_ops();
        self.journal.resume();
        // The wholesale text op mangled span/block/note adjustment; the
        // restored state and the content-based re-anchoring are authoritative.
        self.spans = spans.into();
        let lines = self.buffer.rope().len_lines();
        self.blocks = if blocks.len() == lines {
            blocks
        } else {
            // Length mismatch (trailing-newline disagreement, foreign state):
            // rebuild the kinds but CARRY THE BOUNDARY through the clamp
            // instead of discarding it (time-persistence 7) — a dropped seam
            // is a silent scope trespass; a truly out-of-range index degrades
            // to None inside the setters.
            let mut fresh = BlockMap::new(lines);
            fresh.set_aside_boundary(blocks.aside_boundary());
            fresh.set_scrap_line(blocks.scrap_line());
            fresh
        }.into();
        self.notes = reanchored;
        self.provenance = prov;
        // Overwrite the origin_pos-mangled entries with the preserved ones,
        // clamped into the restored length.
        self.graveyard = saved_graveyard;
        self.graveyard.clamp(self.buffer.rope().len_chars());
        self.journal_seam(before_seam);
    }

    /// Export undo/redo state for persistence (most-recent `cap` entries).
    /// Saved atomically with the text it refers to, so it restores exactly.
    pub fn export_history(&self, cap: usize) -> History {
        let (undo, redo) = self.buffer.export_history(cap);
        let tail = |v: &Vec<SideState>| v[v.len().saturating_sub(cap)..].to_vec();
        History {
            undo,
            redo,
            undo_states: tail(&self.undo_states),
            redo_states: tail(&self.redo_states),
        }
    }

    /// Drop the undo/redo stacks entirely — the migration's one deliberate
    /// use (time-persistence 4): after the flip, a cross-session ctrl-Z must
    /// not be able to reinstate a top-era boundary into the live doc. The
    /// documented pre-P2 precedent: the STACK is lost, nothing else is.
    fn clear_history(&mut self) {
        self.buffer.import_history(Vec::new(), Vec::new());
        self.undo_states.clear();
        self.redo_states.clear();
    }

    /// Restore persisted undo/redo state. Misaligned data (foreign or
    /// corrupted file) is dropped — never trusted into a panic.
    pub fn import_history(&mut self, history: History) {
        if history.undo.len() != history.undo_states.len()
            || history.redo.len() != history.redo_states.len()
        {
            return;
        }
        self.buffer.import_history(history.undo, history.redo);
        self.undo_states = history.undo_states;
        self.redo_states = history.redo_states;
    }

    /// The MIGRATION transaction (time-persistence 4; 08 §2 "Adoption &
    /// migration" path 2): flip a live top-era document to the tail era,
    /// once, at open, before the first edit. Deliberately NOT a ctrl-Z atom
    /// — the undo/redo stacks are DROPPED (the documented pre-P2 precedent)
    /// so no cross-session undo can reinstate top geometry; the inverse is
    /// Restore of the caller's "Before migration" checkpoint, which restore
    /// normalization makes idempotent. Side records — annotation ranges,
    /// graveyard `origin_pos`, provenance — are remapped ARITHMETICALLY via
    /// the flip's char map, never through `apply_op`'s clamp (which would
    /// strand every in-pile anchor at char 0). The journal is paused around
    /// the wholesale move (two edits: delete the pile prefix, append it at
    /// the tail — one-time oplog growth ≈ 2× pile size) and records one
    /// honest `Seam` event. The caller records the "Before migration" /
    /// "Migrated" checkpoint pair. Returns whether a migration ran.
    pub fn migrate_top_to_tail(&mut self) -> bool {
        let Some((BoundaryEra::Top, b)) = self.blocks.boundary() else {
            return false;
        };
        if self.blocks.len() != self.buffer.rope().len_lines() {
            return false; // a mismatched map never flips (degrade, don't guess)
        }
        let rope = self.buffer.rope();
        let b = b.min(rope.len_lines().saturating_sub(1));
        let map = flip_char_map(rope, b);
        let pile: String = rope.slice(0..map.pile_end).to_string();
        let (_, new_spans, new_blocks) =
            flip_state(&self.buffer.text(), &self.spans, &self.blocks);
        let saved_notes = self.notes.clone();
        let saved_grave = self.graveyard.clone();
        let saved_prov = self.provenance.clone();

        self.revision += 1;
        self.journal.pause();
        if map.old_manu_start > 0 {
            let prefix_bytes = self.buffer.char_to_byte(map.old_manu_start);
            self.buffer.edit_bytes(0..prefix_bytes, "");
        }
        let tail_byte = self.buffer.len_bytes();
        self.buffer.edit_bytes(tail_byte..tail_byte, &format!("\n\n{pile}"));
        // Ops still mirror to the durable store (pending_ops); the journal is
        // paused, and the side-state mangling below is overwritten wholesale.
        self.absorb_buffer_ops();
        self.journal.resume();

        self.spans = new_spans.into();
        self.blocks = new_blocks.into();
        self.notes = saved_notes;
        for n in &mut self.notes.notes {
            n.range = map.pos(n.range.start)..map.pos(n.range.end);
            if n.range.end < n.range.start {
                n.range.end = n.range.start;
            }
        }
        self.notes.notes.sort_by_key(|n| n.range.start);
        self.graveyard = saved_grave;
        for e in &mut self.graveyard.entries {
            // Region stays Manuscript: top-era cuts were manuscript cuts by
            // construction; only the position moves with the flip.
            e.origin_pos = map.pos(e.origin_pos);
        }
        self.provenance = saved_prov;
        for r in &mut self.provenance.records {
            r.range = map.pos(r.range.start)..map.pos(r.range.end);
            if r.range.end < r.range.start {
                r.range.end = r.range.start;
            }
        }
        self.provenance.records.retain(|r| r.range.start < r.range.end);
        // ctrl-Z must not reach back across the flip (precedented drop).
        self.clear_history();
        self.journal_seam(None);
        true
    }
}

/// Persisted cross-session undo/redo: the transaction stacks plus their
/// aligned span/block snapshots (one lifecycle for typing and formatting —
/// ctrl-z after reopen behaves exactly like before close).
///
/// The state tuple carries the `Graveyard` and `Provenance` alongside
/// spans/blocks/notes so a cross-session undo of a cut also removes its
/// entry (and of a park, its record). A pre-P2 file's history is a 3-tuple
/// and a pre-Scraps file's a 4-tuple: either fails this struct's serde
/// arity, so `Store::open` drops it (the length-mismatch guard in
/// `import_history` never even runs) — the undo STACK is lost for that one
/// upgrade, while text/notes/graveyard all reload via their own channels.
/// Documented, one-time, non-destructive (and for top-era files the
/// migration drops the stacks regardless — time-persistence 4).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct History {
    undo: Vec<Transaction>,
    redo: Vec<Transaction>,
    undo_states: Vec<SideState>,
    redo_states: Vec<SideState>,
}

impl History {
    /// Asset ids any undo/redo state could resurrect (GC must keep them) —
    /// including entries a persisted Graveyard element could Put back
    /// (graveyard-interplay 9: undo of a grave_delete resurrects an entry
    /// whose kinds may hold an Image).
    pub fn asset_refs(&self) -> impl Iterator<Item = &str> {
        self.undo_states
            .iter()
            .chain(self.redo_states.iter())
            .flat_map(|(_, blocks, _, graveyard, _)| {
                blocks.asset_refs().chain(graveyard.asset_refs())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_annotation_transform_matches_the_original_interval_law() {
        let old = |range: &Range<usize>, op: &TextOp| {
            let del_end = op.pos + op.delete;
            let ins = op.insert.chars().count();
            let clamp = |x: usize| if x >= del_end { x - op.delete }
                else if x > op.pos { op.pos } else { x };
            let mut r = range.clone();
            if op.delete > 0 { r.start = clamp(r.start); r.end = clamp(r.end); }
            if ins > 0 {
                if r.start >= op.pos { r.start += ins; }
                if r.end > op.pos { r.end += ins; }
                if r.end < r.start { r.end = r.start; }
            }
            r
        };
        for start in 0..8 {
            for end in start..8 {
                for pos in 0..8 {
                    for delete in 0..=8 - pos {
                        for insert in ["", "x", "xy"] {
                            let op = TextOp { pos, delete, insert: insert.into() };
                            let range = start..end;
                            assert_eq!(transform_annotation_range(&range, &op), old(&range, &op));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn card_record_uses_creation_and_committed_body_grain() {
        let mut doc = Document::new("alpha", SpanSet::default(), BlockMap::new(1));
        let id = doc.add_note(0..5, String::new(), 10);
        assert!(matches!(doc.journal().events.last(),
            Some(crate::journal::JournalEvent::CardRaised { id: raised, body, .. })
                if *raised == id && body.is_empty()));
        doc.set_note_body_draft(id, "draft".into());
        assert_eq!(doc.journal().events.len(), 1, "draft keystrokes are not events");
        doc.set_note_body(id, "draft".into());
        assert!(matches!(doc.journal().events.last(),
            Some(crate::journal::JournalEvent::CardEdited { id: edited, body, .. })
                if *edited == id && body == "draft"));
        doc.set_note_body(id, "draft".into());
        assert_eq!(doc.journal().events.len(), 2, "unchanged commits are suppressed");
    }

    #[test]
    fn document_cut_records_words_from_the_actual_deleted_text() {
        let mut doc = Document::new("one two-three", SpanSet::default(), BlockMap::new(1));
        doc.edit_bytes(0..doc.len_bytes(), "");
        assert_eq!(doc.journal().runs[0].del_words, Some(2));
    }

    #[test]
    fn document_charwise_deletes_count_whole_runs() {
        let mut backspace = Document::new("hello world", SpanSet::default(), BlockMap::new(1));
        while backspace.len_bytes() > 0 {
            let end = backspace.len_bytes();
            backspace.edit_bytes(end - 1..end, "");
        }
        assert_eq!(backspace.journal().runs.len(), 1);
        assert_eq!(backspace.journal().runs[0].del_words, Some(2));

        let mut forward = Document::new("hello world", SpanSet::default(), BlockMap::new(1));
        while forward.len_bytes() > 0 {
            forward.edit_bytes(0..1, "");
        }
        assert_eq!(forward.journal().runs.len(), 1);
        assert_eq!(forward.journal().runs[0].del_words, Some(2));
    }

    #[test]
    fn byte_break_lengths_and_line_text_ends_cover_ropeys_full_set() {
        for (separator, bytes) in [
            ("\n", 1),
            ("\u{000B}", 1),
            ("\u{000C}", 1),
            ("\r", 1),
            ("\r\n", 2),
            ("\u{0085}", 2),
            ("\u{2028}", 3),
            ("\u{2029}", 3),
        ] {
            let rope = ropey::Rope::from_str(&format!("alpha{separator}beta"));
            let next = rope.line_to_byte(1);
            assert_eq!(break_len_before_bytes(&rope, next), bytes, "{separator:?}");
            assert_eq!(line_text_end_bytes(&rope, 0), 5, "{separator:?}");
            assert_eq!(line_text_end_bytes(&rope, 1), rope.len_bytes(), "{separator:?}");
        }

        let rope = ropey::Rope::from_str("alpha beta");
        assert_eq!(break_len_before_bytes(&rope, 5), 0);
    }

    #[test]
    fn side_snapshots_share_unchanged_components_and_cow_only_the_mutated_one() {
        let mut doc = Document::new("abc", SpanSet::default(), BlockMap::new(1));
        doc.edit_bytes_coalescing(3..3, "x");
        let first = doc.undo_states.last().unwrap();
        assert!(Arc::ptr_eq(&doc.spans.0, &first.0.0));
        assert!(Arc::ptr_eq(&doc.blocks.0, &first.1.0));
        assert!(Arc::ptr_eq(&doc.notes.0, &first.2.0));
        assert!(Arc::ptr_eq(&doc.graveyard.0, &first.3.0));
        assert!(Arc::ptr_eq(&doc.provenance.0, &first.4.0));

        doc.toggle_format(0..1, InlineAttr::Strong);
        let before = doc.undo_states.last().unwrap();
        assert!(!Arc::ptr_eq(&doc.spans.0, &before.0.0));
        assert!(Arc::ptr_eq(&doc.blocks.0, &before.1.0));
        assert!(Arc::ptr_eq(&doc.notes.0, &before.2.0));
        assert!(Arc::ptr_eq(&doc.graveyard.0, &before.3.0));
        assert!(Arc::ptr_eq(&doc.provenance.0, &before.4.0));

        doc.undo(); doc.undo();
        assert_eq!(doc.text(), "abc");
        doc.redo(); doc.redo();
        assert_eq!(doc.text(), "abcx");
        assert!(doc.spans().covers(0..1, &InlineAttr::Strong));
    }

    #[test]
    fn edits_after_nonempty_overlays_keep_unaffected_versions_shared() {
        let mut doc = Document::new("abc", SpanSet::default(), BlockMap::new(1));
        doc.add_note(0..1, "note".into(), 1);
        doc.edit_bytes(3..3, "x");
        let before_edit = doc.undo_states.last().unwrap();
        assert!(!doc.notes().notes().is_empty());
        assert!(Arc::ptr_eq(&doc.notes.0, &before_edit.2.0));
    }

    /// Deterministic allocation proxy plus an undo timing probe. Run with
    /// `cargo test -p strop-core --release cow_history_sharing_probe -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn cow_history_sharing_probe() {
        let lines = 5_000;
        let steps = 2_000;
        let text = "\n".repeat(lines - 1);
        let mut doc = Document::new(&text, SpanSet::default(), BlockMap::new(lines));
        for _ in 0..steps { doc.edit_bytes(0..0, "x"); }
        let block_ptr = Arc::as_ptr(&doc.blocks.0);
        assert!(doc.undo_states.iter().all(|s| Arc::as_ptr(&s.1.0) == block_ptr));
        eprintln!(
            "cow history: {steps} frames × {lines} blocks; 1 shared BlockMap allocation ({} BlockKind clones avoided)",
            steps * lines
        );
        eprintln!(
            "cow history: SideState handle = {} bytes/frame before Vec capacity and uniquely changed values",
            std::mem::size_of::<SideState>()
        );
        for cap in [50, 200] {
            let started = std::time::Instant::now();
            let bytes = serde_json::to_vec(&doc.export_history(cap)).unwrap();
            eprintln!(
                "cow history: serialized cap {cap} = {} bytes in {:?}",
                bytes.len(),
                started.elapsed()
            );
        }
        let started = std::time::Instant::now();
        for _ in 0..steps { assert!(doc.undo().is_some()); }
        eprintln!("cow history: {steps} undos in {:?}", started.elapsed());
    }

    #[test]
    fn shared_history_keeps_the_legacy_json_wire_shape() {
        let mut doc = Document::new("abc", SpanSet::default(), BlockMap::new(1));
        doc.edit_bytes(3..3, "x");
        let value = serde_json::to_value(doc.export_history(10)).unwrap();
        assert_eq!(value, serde_json::json!({
            "undo": [{"edits": [{"start": 3, "old": "", "new": "x"}]}],
            "redo": [],
            "undo_states": [[
                {"spans": []}, {"kinds": ["Paragraph"]},
                {"notes": [], "next_id": 0}, {"entries": [], "next_id": 0},
                {"records": [], "next_id": 0}
            ]],
            "redo_states": []
        }));
        let history: History = serde_json::from_value(value).unwrap();
        let mut reopened = Document::new("abcx", SpanSet::default(), BlockMap::new(1));
        reopened.import_history(history);
        assert_eq!(reopened.undo(), Some(Some(3)));
        assert_eq!(reopened.text(), "abc");
    }

    #[test]
    fn rejected_persisted_transaction_clears_its_aligned_side_stack() {
        let value = serde_json::json!({
            "undo": [{"edits": [{"start": 0, "old": "", "new": "foreign"}]}],
            "redo": [],
            "undo_states": [[
                {"spans": []}, {"kinds": ["Paragraph"]},
                {"notes": [], "next_id": 0}, {"entries": [], "next_id": 0},
                {"records": [], "next_id": 0}
            ]],
            "redo_states": []
        });
        let mut doc = Document::new("local", SpanSet::default(), BlockMap::new(1));
        doc.import_history(serde_json::from_value(value).unwrap());
        assert_eq!(doc.undo(), None);
        let history = doc.export_history(10);
        assert!(history.undo.is_empty() && history.undo_states.is_empty());
        assert_eq!(doc.text(), "local");
    }

    fn op(pos: usize, delete: usize, insert: &str) -> TextOp {
        TextOp {
            pos,
            delete,
            insert: insert.into(),
        }
    }

    #[test]
    fn diagnosis_greys_only_when_its_flagged_text_is_edited() {
        let mk = |kind: NoteKind, range: Range<usize>| Annotation {
            id: 0,
            range,
            body: "x".into(),
            status: NoteStatus::Open,
            created_unix: 0,
            kind,
            title: "t".into(),
            level: "line".into(),
            orphaned: false,
            pass_id: 1,
            unverified: false,
        };
        let mut anns = Annotations::default();
        let diag = anns.push(mk(NoteKind::Diagnosis, 10..20));
        let note = anns.push(mk(NoteKind::Note, 10..20));

        // An edit OUTSIDE the span (insert at 0) greys nothing; it just shifts
        // both anchors to 11..21.
        anns.apply_op(&op(0, 0, "x"));
        assert!(!anns.get(diag).unwrap().unverified, "outside edit must not grey");

        // An edit INSIDE the (shifted) span greys the diagnosis — and only it;
        // the writer's own note never decays.
        anns.apply_op(&op(15, 0, "y"));
        assert!(anns.get(diag).unwrap().unverified, "in-span edit greys the diagnosis");
        assert!(!anns.get(note).unwrap().unverified, "writer notes never grey");
    }

    #[test]
    fn composer_body_path_never_mutates_a_diagnosis() {
        let mk = |kind: NoteKind, body: &str| Annotation {
            id: 0,
            range: 0..1,
            body: body.into(),
            status: NoteStatus::Open,
            created_unix: 0,
            kind,
            title: "t".into(),
            level: "line".into(),
            orphaned: false,
            pass_id: 0,
            unverified: false,
        };
        let mut anns = Annotations::default();
        let note = anns.push(mk(NoteKind::Note, "mine"));
        let diag = anns.push(mk(NoteKind::Diagnosis, "the AI's query"));

        // The composer/draft path edits a writer note...
        anns.set_body(note, "edited".into());
        assert_eq!(anns.get(note).unwrap().body, "edited");
        // ...but can never overwrite a diagnosis body (the leak class).
        anns.set_body(diag, "leaked draft".into());
        assert_eq!(
            anns.get(diag).unwrap().body,
            "the AI's query",
            "a diagnosis body must be immutable to the composer path"
        );
    }

    fn strong(range: Range<usize>) -> Span {
        Span {
            range,
            attr: InlineAttr::Strong,
        }
    }

    #[test]
    fn revision_bumps_on_every_layout_mutation_and_is_stable_otherwise() {
        let mut doc = Document::new("hello world", SpanSet::default(), BlockMap::default());
        let r0 = doc.revision();
        // Read-only access never bumps (the view's reuse fast-path depends on
        // this: equal revisions across two reads ⟺ no layout change between).
        let _ = (doc.text(), doc.spans().spans().len(), doc.blocks().len());
        assert_eq!(doc.revision(), r0, "read-only access must not bump revision");

        doc.edit_bytes_coalescing(5..5, "X");
        let r1 = doc.revision();
        assert!(r1 > r0, "text edit must bump revision");

        // Format toggle changes no text (buffer.version may not move) but must
        // still bump — this is the case a text-only signal would miss.
        doc.toggle_format(0..5, InlineAttr::Strong);
        let r2 = doc.revision();
        assert!(r2 > r1, "format toggle must bump revision");

        let id = doc.add_note(0..5, "n".into(), 0);
        let r3 = doc.revision();
        assert!(r3 > r2, "adding a note must bump revision");

        doc.set_note_status(id, NoteStatus::Done);
        let r4 = doc.revision();
        assert!(r4 > r3, "note status change must bump revision");

        doc.undo();
        assert!(doc.revision() > r4, "undo must bump revision");
    }

    #[test]
    fn set_link_replaces_the_target_and_empty_clears_it() {
        let mut doc = Document::new("hello world", SpanSet::default(), BlockMap::default());
        // Set a link over "hello".
        doc.set_link(0..5, Some("https://a.example".into()));
        assert_eq!(doc.link_over(0..5).as_deref(), Some("https://a.example"));
        // Re-setting with a NEW url replaces (never leaves the old one alongside):
        // exactly one Link span survives over the range, and it carries the new
        // target — the editing-a-link case remove()'s URL-exactness would miss.
        doc.set_link(0..5, Some("https://b.example".into()));
        assert_eq!(doc.link_over(0..5).as_deref(), Some("https://b.example"));
        let links = doc
            .spans()
            .spans()
            .iter()
            .filter(|s| matches!(&s.attr, InlineAttr::Link(_)))
            .count();
        assert_eq!(links, 1, "editing a link must not strand the old target");
        // An empty commit removes the link.
        doc.set_link(0..5, None);
        assert_eq!(doc.link_over(0..5), None);
        assert!(
            !doc.spans().spans().iter().any(|s| matches!(&s.attr, InlineAttr::Link(_))),
            "empty commit clears the link"
        );
    }

    #[test]
    fn typing_at_right_edge_expands_styles_not_code() {
        let mut set = SpanSet::default();
        set.add(0..4, InlineAttr::Strong);
        set.apply_op(&op(4, 0, "x"));
        assert_eq!(set.spans(), &[strong(0..5)]);

        let mut set = SpanSet::default();
        set.add(0..4, InlineAttr::Code);
        set.apply_op(&op(4, 0, "x"));
        assert_eq!(
            set.spans(),
            &[Span {
                range: 0..4,
                attr: InlineAttr::Code
            }]
        );
    }

    #[test]
    fn typing_at_left_edge_stays_outside() {
        let mut set = SpanSet::default();
        set.add(2..6, InlineAttr::Strong);
        set.apply_op(&op(2, 0, "ab"));
        assert_eq!(set.spans(), &[strong(4..8)]);
    }

    #[test]
    fn typing_inside_grows() {
        let mut set = SpanSet::default();
        set.add(2..6, InlineAttr::Strong);
        set.apply_op(&op(4, 0, "xy"));
        assert_eq!(set.spans(), &[strong(2..8)]);
    }

    #[test]
    fn deletion_clamps_and_drops() {
        // Delete across the left boundary.
        let mut set = SpanSet::default();
        set.add(4..8, InlineAttr::Strong);
        set.apply_op(&op(2, 4, "")); // delete [2,6)
        assert_eq!(set.spans(), &[strong(2..4)]);

        // Delete the entire span: it disappears.
        let mut set = SpanSet::default();
        set.add(4..8, InlineAttr::Strong);
        set.apply_op(&op(3, 6, ""));
        assert!(set.spans().is_empty());
    }

    #[test]
    fn replace_inside_span() {
        // Replacing text inside a span keeps the span around the result.
        let mut set = SpanSet::default();
        set.add(0..10, InlineAttr::Emphasis);
        set.apply_op(&op(3, 4, "xy")); // 10 chars -> 8 chars
        assert_eq!(
            set.spans(),
            &[Span {
                range: 0..8,
                attr: InlineAttr::Emphasis
            }]
        );
    }

    #[test]
    fn add_merges_same_attr() {
        let mut set = SpanSet::default();
        set.add(0..4, InlineAttr::Strong);
        set.add(4..8, InlineAttr::Strong);
        assert_eq!(set.spans(), &[strong(0..8)]);
        // Different attrs never merge.
        set.add(2..6, InlineAttr::Emphasis);
        assert_eq!(set.spans().len(), 2);
    }

    #[test]
    fn remove_splits_straddling_span() {
        let mut set = SpanSet::default();
        set.add(0..10, InlineAttr::Strong);
        set.remove(3..6, &InlineAttr::Strong);
        assert_eq!(set.spans(), &[strong(0..3), strong(6..10)]);
        // Other attrs untouched.
        let mut set = SpanSet::default();
        set.add(0..10, InlineAttr::Strong);
        set.remove(3..6, &InlineAttr::Emphasis);
        assert_eq!(set.spans(), &[strong(0..10)]);
    }

    #[test]
    fn covers_handles_chains_and_gaps() {
        let mut set = SpanSet::default();
        set.add(0..4, InlineAttr::Strong);
        set.add(6..9, InlineAttr::Strong);
        assert!(set.covers(1..3, &InlineAttr::Strong));
        assert!(!set.covers(1..7, &InlineAttr::Strong)); // gap at 4..6
        assert!(!set.covers(1..3, &InlineAttr::Emphasis));
        // Adjacent spans (possible after edits) chain.
        let mut set = SpanSet::default();
        set.add(0..4, InlineAttr::Code);
        set.add(4..8, InlineAttr::Code); // Code merges too via add
        assert!(set.covers(2..6, &InlineAttr::Code));
    }

    #[test]
    fn format_toggle_is_undoable() {
        let mut doc = Document::new("hello world", SpanSet::default(), BlockMap::default());
        doc.toggle_format(0..5, InlineAttr::Strong);
        assert!(doc.spans().covers(0..5, &InlineAttr::Strong));
        // Format-only transaction: undo keeps the cursor (inner None).
        assert_eq!(doc.undo(), Some(None));
        assert!(doc.spans().spans().is_empty());
        doc.redo();
        assert!(doc.spans().covers(0..5, &InlineAttr::Strong));
    }

    #[test]
    fn typing_with_sticky_attr_undoes_together() {
        let mut doc = Document::new("", SpanSet::default(), BlockMap::default());
        doc.edit_bytes_coalescing(0..0, "w");
        doc.format_in_current_tx(0..1, InlineAttr::Strong, true);
        doc.edit_bytes_coalescing(1..1, "o"); // same tx; expansion grows span
        assert!(doc.spans().covers(0..2, &InlineAttr::Strong));
        assert_eq!(doc.undo(), Some(Some(0)));
        assert_eq!(doc.text(), "");
        assert!(doc.spans().spans().is_empty());
    }

    #[test]
    fn undo_restores_formatting_deleted_with_text() {
        // The smoke-run bug: delete styled text, undo, the style returns
        // (and the neighbor no longer swallows the restored range).
        let mut doc = Document::new("bold plain", SpanSet::default(), BlockMap::default());
        doc.toggle_format(0..4, InlineAttr::Strong);
        doc.edit_bytes(2..8, "");
        assert_eq!(doc.text(), "boin");
        doc.undo();
        assert_eq!(doc.text(), "bold plain");
        assert!(doc.spans().covers(0..4, &InlineAttr::Strong));
        assert!(!doc.spans().covers(0..5, &InlineAttr::Strong));
    }

    #[test]
    fn restore_state_is_one_undoable_transaction() {
        let mut doc = Document::new("новый текст", SpanSet::default(), BlockMap::default());
        doc.toggle_format(0..5, InlineAttr::Strong);
        doc.edit_bytes(0..0, "ещё ");
        doc.restore_state("старый", SpanSet::default(), BlockMap::new(1));
        assert_eq!(doc.text(), "старый");
        assert!(doc.spans().spans().is_empty());
        // One undo returns to the pre-restore present, formatting included.
        doc.undo();
        assert_eq!(doc.text(), "ещё новый текст");
        assert!(doc.spans().covers(4..9, &InlineAttr::Strong));
    }

    #[test]
    fn notes_anchor_adjust_and_undo() {
        let mut doc = Document::new("первый абзац", SpanSet::default(), BlockMap::default());
        let id = doc.add_note(0..6, "лид?".into(), 0);
        assert_eq!(doc.notes().open().count(), 1);
        // Typing before the anchor shifts it; the note follows its text.
        doc.edit_bytes(0..0, "ещё ");
        let n = doc.notes().get(id).unwrap();
        assert_eq!(n.range, 4..10);
        // Status change is its own undoable step.
        doc.set_note_status(id, NoteStatus::Done);
        assert_eq!(doc.notes().open().count(), 0);
        doc.undo();
        assert_eq!(doc.notes().open().count(), 1);
        // Undo the typing, then the note creation: overlay restores fully.
        doc.undo();
        assert_eq!(doc.notes().get(id).unwrap().range, 0..6);
        doc.undo();
        assert!(doc.notes().notes().is_empty());
    }

    #[test]
    fn cancelling_topmost_provisional_note_removes_its_empty_undo_step() {
        let mut doc = Document::new("base", SpanSet::default(), BlockMap::default());
        doc.edit_bytes(4..4, " edit");
        let depth = doc.undo_states.len();
        let id = doc.add_note(0..4, String::new(), 0);
        assert_eq!(doc.undo_states.len(), depth + 1);
        assert!(doc.cancel_provisional_note(id).is_some());
        assert_eq!(doc.undo_states.len(), depth);
        doc.undo();
        assert_eq!(doc.text(), "base", "next undo reaches the real edit");
    }

    #[test]
    fn cancelling_intervened_provisional_note_scrubs_undo_and_redo() {
        let mut doc = Document::new("base", SpanSet::default(), BlockMap::default());
        let id = doc.add_note(0..4, String::new(), 0);
        doc.add_diagnoses(vec![Annotation {
            id: 0,
            range: 0..4,
            body: "query".into(),
            status: NoteStatus::Open,
            created_unix: 0,
            kind: NoteKind::Diagnosis,
            title: "diagnosis".into(),
            level: "line".into(),
            orphaned: false,
            pass_id: 1,
            unverified: false,
        }]);
        assert!(doc.cancel_provisional_note(id).is_some());
        doc.undo();
        assert!(doc.notes().get(id).is_none(), "undo must not resurrect it");
        doc.redo();
        assert!(doc.notes().get(id).is_none(), "redo must not resurrect it");
    }

    #[test]
    fn committed_note_keeps_its_existing_undo_and_redo_history() {
        let mut doc = Document::new("base", SpanSet::default(), BlockMap::default());
        let id = doc.add_note(0..4, String::new(), 0);
        doc.set_note_body(id, "kept".into());
        doc.undo();
        assert_eq!(doc.notes().get(id).unwrap().body, "");
        doc.undo();
        assert!(doc.notes().get(id).is_none());
        doc.redo();
        assert!(doc.notes().get(id).is_some());
    }

    #[test]
    fn restore_reanchors_notes_to_their_content() {
        // A note follows the passage it covers to wherever that text lives in
        // the restored version — not collapsed to the document end.
        let mut doc = Document::new("alpha beta gamma", SpanSet::default(), BlockMap::default());
        let id = doc.add_note(6..10, "on beta".into(), 0); // "beta"
        doc.restore_state("xx beta yy", SpanSet::default(), BlockMap::new(1));
        let n = doc.notes().get(id).unwrap();
        assert_eq!(n.range, 3..7, "note should track its passage to its new offset");
        assert!(!n.orphaned, "a found passage is not orphaned");
        assert_eq!(doc.text(), "xx beta yy");
    }

    #[test]
    fn restore_reanchors_repeated_passages_in_document_order() {
        // Two notes on different occurrences of the same word must keep their
        // own occurrence (positional hint), not both snap to the first.
        let mut doc = Document::new("foo foo foo", SpanSet::default(), BlockMap::default());
        let a = doc.add_note(0..3, "first".into(), 0);
        let b = doc.add_note(8..11, "third".into(), 0);
        doc.restore_state("foo foo foo", SpanSet::default(), BlockMap::new(1));
        assert_eq!(doc.notes().get(a).unwrap().range, 0..3);
        assert_eq!(doc.notes().get(b).unwrap().range, 8..11);
    }

    #[test]
    fn restore_detaches_note_whose_passage_is_gone() {
        // The passage vanished in the restored version: the note DETACHES —
        // flagged orphaned and parked at its clamped former offset, never
        // piled at the document end.
        let mut doc = Document::new("keep DELETED keep", SpanSet::default(), BlockMap::default());
        let id = doc.add_note(5..12, "on deleted".into(), 0); // "DELETED"
        doc.restore_state("keep keep", SpanSet::default(), BlockMap::new(1));
        let n = doc.notes().get(id).unwrap();
        assert!(n.orphaned, "a vanished passage detaches");
        assert_eq!(n.range.start, n.range.end, "a detached note is a point");
        let end = doc.rope().len_chars();
        assert!(n.range.start < end, "detached note must not pile at the document end");
    }

    #[test]
    fn restore_reanchor_is_one_undoable_step() {
        // Undo of a restore brings every note back to its exact pre-restore
        // anchor and clears the orphaned flag.
        let mut doc = Document::new("keep WORD keep", SpanSet::default(), BlockMap::default());
        let id = doc.add_note(5..9, "on word".into(), 0); // "WORD"
        doc.restore_state("nothing here", SpanSet::default(), BlockMap::new(1));
        assert!(doc.notes().get(id).unwrap().orphaned);
        doc.undo();
        let n = doc.notes().get(id).unwrap();
        assert_eq!(n.range, 5..9, "undo restores the pre-restore anchor");
        assert!(!n.orphaned, "undo clears the orphaned flag");
    }

    #[test]
    fn attrs_at_reports_covering_spans() {
        let mut set = SpanSet::default();
        set.add(0..4, InlineAttr::Strong);
        set.add(2..6, InlineAttr::Emphasis);
        let at3: Vec<_> = set.attrs_at(3).collect();
        assert_eq!(at3.len(), 2);
        assert_eq!(set.attrs_at(5).count(), 1);
        assert_eq!(set.attrs_at(6).count(), 0); // end-exclusive
    }

    #[test]
    fn span_normalization_has_deterministic_ties() {
        // Repair first groups by attribute so same-attribute overlaps can be
        // merged in one sweep. Its final stable start-sort must retain that
        // canonical attribute order for equal starts; Markdown nesting and
        // persisted undo snapshots both observe it.
        let mut set = SpanSet {
            spans: vec![
                Span { range: 2..5, attr: InlineAttr::Strong },
                Span { range: 0..3, attr: InlineAttr::Underline },
                Span { range: 0..2, attr: InlineAttr::Emphasis },
                Span { range: 0..2, attr: InlineAttr::Strong },
            ],
        };

        set.normalize(5);

        assert_eq!(
            set.spans(),
            &[
                Span { range: 0..2, attr: InlineAttr::Emphasis },
                Span { range: 0..5, attr: InlineAttr::Strong },
                Span { range: 0..3, attr: InlineAttr::Underline },
            ]
        );
    }

    #[test]
    fn span_normalization_does_not_reorder_an_already_valid_set() {
        // Equal-start ordering can encode the user's marker nesting. A load
        // that needs no repair must not rewrite that otherwise-valid state.
        let original = vec![
            Span { range: 0..3, attr: InlineAttr::Underline },
            Span { range: 0..3, attr: InlineAttr::Emphasis },
        ];
        let mut set = SpanSet { spans: original.clone() };

        set.normalize(3);

        assert_eq!(set.spans(), original);
    }

    #[test]
    fn remove_keeps_spans_sorted() {
        // A partial remove used to push the split tail in place, leaving
        // it after later-starting spans of other attrs; covers() assumes
        // sorted order and saw phantom gaps. Found by the tests/model.rs
        // state machine (2026-06-12).
        let mut set = SpanSet::default();
        set.add(0..10, InlineAttr::Emphasis);
        set.add(2..3, InlineAttr::Underline);
        set.remove(0..5, &InlineAttr::Emphasis);
        let starts: Vec<usize> = set.spans().iter().map(|s| s.range.start).collect();
        let mut sorted = starts.clone();
        sorted.sort_unstable();
        assert_eq!(starts, sorted, "spans must stay sorted after remove");
        assert!(set.covers(5..10, &InlineAttr::Emphasis));
        assert!(set.covers(2..3, &InlineAttr::Underline));
    }

    #[test]
    fn block_map_invariant_survives_non_lf_separators() {
        // ropey (unicode_lines) counts CR/VT/FF/NEL/LS/PS as line breaks, but
        // the split count used to scan only '\n' — a paste of classic-Mac or
        // PDF-copied text broke kinds.len() == rope.len_lines().
        for sep in ["\r", "\u{000b}", "\u{000c}", "\u{2028}", "\u{2029}", "\u{0085}"] {
            let mut doc = Document::new("ab", SpanSet::default(), BlockMap::default());
            doc.edit_bytes(1..1, sep);
            assert_eq!(
                doc.blocks().len(),
                doc.rope().len_lines(),
                "invariant broken inserting {sep:?}"
            );
            // Deleting it must rejoin the blocks.
            doc.edit_bytes(1..1 + sep.len(), "");
            assert_eq!(
                doc.blocks().len(),
                doc.rope().len_lines(),
                "invariant broken deleting {sep:?}"
            );
        }
        // CRLF must count as ONE break (the trap a naive Unicode scan falls
        // into) — and the coalescing paste path must hold too.
        let mut doc = Document::new("ab", SpanSet::default(), BlockMap::default());
        doc.edit_bytes_coalescing(1..1, "\r\n");
        assert_eq!(doc.blocks().len(), doc.rope().len_lines());
    }

    // ---- asides: the boundary index, the graveyard, the manuscript slice ----

    #[test]
    fn aside_boundary_shifts_across_block_splices() {
        // 6 blocks; boundary at 2: compost [0,1], separator [2], manuscript
        // [3,4,5]. `on_edit(block, merged, splits)` is the only thing that keeps
        // the out-of-band index aligned, so it is exercised at every relation.
        let mk = || {
            let mut b = BlockMap::new(6);
            b.set_aside_boundary(Some(2));
            b
        };
        // Split ABOVE the boundary (new line in compost, block 0) → +1.
        let mut b = mk();
        b.on_edit(0, 0, 1);
        assert_eq!(b.aside_boundary(), Some(3));
        // Split BELOW (new manuscript line, block 4) → unchanged.
        let mut b = mk();
        b.on_edit(4, 0, 1);
        assert_eq!(b.aside_boundary(), Some(2));
        // Split AT the boundary line itself → the new line lands after it.
        let mut b = mk();
        b.on_edit(2, 0, 1);
        assert_eq!(b.aside_boundary(), Some(2));
        // Merge ABOVE (block 0 absorbs block 1) → -1.
        let mut b = mk();
        b.on_edit(0, 1, 0);
        assert_eq!(b.aside_boundary(), Some(1));
        // Merge BELOW (manuscript blocks 3+4) → unchanged.
        let mut b = mk();
        b.on_edit(3, 1, 0);
        assert_eq!(b.aside_boundary(), Some(2));
        // Edit entirely inside compost with no line change → unchanged.
        let mut b = mk();
        b.on_edit(1, 0, 0);
        assert_eq!(b.aside_boundary(), Some(2));
    }

    #[test]
    fn aside_boundary_merged_away_dissolves_never_panics() {
        // A merge that engulfs the boundary line (backspace at manuscript
        // start): the boundary clamps onto the merge point, and an emptied
        // compost dissolves the rail rather than pointing past a deleted line.
        let mut b = BlockMap::new(4);
        b.set_aside_boundary(Some(1)); // one compost block, separator at 1
        b.on_edit(0, 1, 0); // merge the compost block into the separator
        assert_eq!(b.aside_boundary(), None, "emptied compost dissolves the rail");
    }

    /// The park verb, tail era: the seam is born at the tail, the text
    /// departs BELOW the manuscript, the caret stays at the collapse point,
    /// and one undo evaporates a just-born seam (08 §2 "Undo").
    #[test]
    fn set_aside_births_the_seam_at_the_tail_and_undoes() {
        let mut doc = Document::new("alpha beta gamma", SpanSet::default(), BlockMap::default());
        assert_eq!(doc.boundary(), None);
        // Park "beta " — a MOVE, so nothing is filed.
        let out = doc.set_aside(6..11, "alpha".into(), 7, false).unwrap();
        assert_eq!(doc.text(), "alpha gamma\n\nbeta ");
        assert_eq!(doc.scrap_line(), Some(1));
        assert_eq!(doc.aside_boundary(), None, "the legacy field stays empty");
        assert!(doc.graveyard().is_empty(), "a move never files a corpse");
        // Caret at the collapse point `s` (before "beta"'s old spot).
        assert_eq!(&doc.text()[doc.char_to_byte(out.caret)..doc.char_to_byte(out.caret) + 5], "gamma");
        // The manuscript excludes the pile.
        assert_eq!(doc.manuscript_slice().0, "alpha gamma");
        assert_eq!(doc.scraps_char_range(), Some(13..18));
        // The park journals its Seam event.
        assert!(doc.journal().seams().count() == 1);
        // A selection park files a provenance record over the parked text.
        assert_eq!(doc.provenance().records().len(), 1);
        let r = &doc.provenance().records()[0];
        assert_eq!(&doc.text()[doc.char_to_byte(r.range.start)..doc.char_to_byte(r.range.end)], "beta ");
        // One undo reverses the whole move, seam and record included.
        doc.undo();
        assert_eq!(doc.text(), "alpha beta gamma");
        assert_eq!(doc.boundary(), None);
        assert!(doc.provenance().is_empty());
    }

    #[test]
    fn set_aside_lands_newest_under_the_seam() {
        let mut doc = Document::new("one two three four", SpanSet::default(), BlockMap::default());
        doc.set_aside(0..4, String::new(), 0, false).unwrap(); // "one "
        assert_eq!(doc.text(), "two three four\n\none ");
        assert_eq!(doc.scrap_line(), Some(1));
        // Park "two " as well: newest lands nearest the story (08 §2).
        doc.set_aside(0..4, String::new(), 0, false).unwrap();
        assert_eq!(doc.text(), "three four\n\ntwo \n\none ", "newest under the seam");
        assert_eq!(doc.scrap_line(), Some(1));
        assert_eq!(doc.manuscript_slice().0, "three four");
        // A restore to a boundary-less state clears the pile (reset).
        doc.restore_state("plain", SpanSet::default(), BlockMap::new(1));
        assert_eq!(doc.boundary(), None);
        assert_eq!(doc.graveyard().len(), 0);
    }

    /// The jot form: the chord with no selection parks the caret's paragraph
    /// plus one adjoining newline (the editor passes the widened range), so
    /// no empty block strands at the join; the jot bears NO provenance.
    #[test]
    fn jot_parks_the_paragraph_and_bears_no_provenance() {
        let mut doc = Document::new("story line\na thought", SpanSet::default(), BlockMap::default());
        // The editor widens the paragraph [11,20) by its leading newline.
        let out = doc.set_aside(10..20, String::new(), 0, true).unwrap();
        assert_eq!(doc.text(), "story line\n\na thought");
        assert_eq!(doc.scrap_line(), Some(1));
        assert!(doc.provenance().is_empty(), "jots create no record");
        assert!(out.retired.is_empty());
        // Undo restores the Enter-typed shape verbatim.
        doc.undo();
        assert_eq!(doc.text(), "story line\na thought");
        assert_eq!(doc.boundary(), None);
    }

    /// Park is lossless (seam-mechanics 5): spans and block kinds captured
    /// and re-stamped; the seam line itself is never kind-stamped.
    #[test]
    fn park_carries_spans_and_kinds_and_never_stamps_the_seam() {
        let blocks = BlockMap::from_kinds(vec![
            BlockKind::Heading(2),
            BlockKind::Paragraph,
        ]);
        let mut spans = SpanSet::default();
        spans.add(0..4, InlineAttr::Strong); // "Head"
        let mut doc = Document::new("Head\nbody stays", spans, blocks);
        doc.set_aside(0..5, String::new(), 0, false).unwrap();
        assert_eq!(doc.text(), "body stays\n\nHead");
        let seam = doc.scrap_line().unwrap();
        assert_eq!(doc.blocks().kind(seam), &BlockKind::Paragraph, "seam is never stamped");
        assert_eq!(doc.blocks().kind(seam + 1), &BlockKind::Heading(2), "kind re-stamped");
        let start = doc.scraps_char_range().unwrap().start;
        assert!(doc.spans().covers(start..start + 4, &InlineAttr::Strong), "span re-added");
        // And the round home: undo restores the original shape.
        doc.undo();
        assert_eq!(doc.text(), "Head\nbody stays");
        assert_eq!(doc.blocks().kind(0), &BlockKind::Heading(2));
    }

    /// Notes travel with their block INSIDE the park atom; open diagnoses
    /// retire (a terminal excluded from suppression); dismissed diagnosis
    /// records on the range die with the park (scopes-search 3).
    #[test]
    fn park_moves_notes_retires_diagnoses_and_drops_dead_dismissals() {
        let mut doc = Document::new("keep this\npark me now", SpanSet::default(), BlockMap::default());
        let note = doc.add_note(10..14, "mine".into(), 0); // "park"
        let mk_diag = |range: Range<usize>, status: NoteStatus| Annotation {
            id: 0,
            range,
            body: "q".into(),
            status,
            created_unix: 0,
            kind: NoteKind::Diagnosis,
            title: "t".into(),
            level: "line".into(),
            orphaned: false,
            pass_id: 1,
            unverified: false,
        };
        doc.add_diagnoses(vec![mk_diag(15..18, NoteStatus::Open)]); // "me n"
        let open_diag = doc.notes().notes().iter().find(|n| n.kind == NoteKind::Diagnosis).unwrap().id;
        doc.add_diagnoses(vec![mk_diag(18..21, NoteStatus::Dismissed)]);
        let dismissed = doc
            .notes()
            .notes()
            .iter()
            .find(|n| n.status == NoteStatus::Dismissed)
            .unwrap()
            .id;

        let out = doc.set_aside(10..21, String::new(), 0, false).unwrap();
        assert_eq!(doc.text(), "keep this\n\n\npark me now");
        // The writer note travelled with its text (still covers "park").
        let n = doc.notes().get(note).unwrap();
        assert_eq!(
            &doc.text()[doc.char_to_byte(n.range.start)..doc.char_to_byte(n.range.end)],
            "park",
            "the note re-anchored inside the pile"
        );
        // The open diagnosis retired — the record is DELETED (retire-on-park;
        // a deleted record can never suppress, and old builds keep parsing).
        assert!(doc.notes().get(open_diag).is_none(), "retirement deletes the record");
        assert_eq!(out.retired, vec![open_diag]);
        assert!(!doc.notes().is_suppressed(&(15..18), "t"), "retirement must not suppress");
        // The dismissal record died with the park.
        assert!(doc.notes().get(dismissed).is_none(), "dismissals die with the park");
        // ctrl-Z resurrects everything (the atom's notes snapshot).
        doc.undo();
        assert_eq!(doc.text(), "keep this\npark me now");
        assert_eq!(doc.notes().get(note).unwrap().range, 10..14);
        assert_eq!(doc.notes().get(open_diag).unwrap().status, NoteStatus::Open);
        assert!(doc.notes().get(dismissed).is_some());
    }

    /// The adoption gesture (08 §2): a trailing selection with no boundary
    /// births the seam ABOVE the selection, in place — the tool has learned
    /// where her scrap line is. Nothing moves; the blank divider is reused.
    #[test]
    fn adoption_births_the_seam_above_a_trailing_pile() {
        let mut doc = Document::new(
            "the piece itself\n\nold cut one\n\nold cut two",
            SpanSet::default(),
            BlockMap::default(),
        );
        let start = doc.text().find("old cut one").unwrap();
        let end = doc.len_bytes();
        let out = doc.set_aside(start..end, String::new(), 0, false).unwrap();
        assert!(out.adopted);
        assert_eq!(doc.text(), "the piece itself\n\nold cut one\n\nold cut two", "nothing moved");
        assert_eq!(doc.scrap_line(), Some(1), "her own divider line became the seam");
        assert_eq!(doc.manuscript_slice().0, "the piece itself");
        assert!(doc.provenance().is_empty(), "adoption parks nothing");
        // Undo un-teaches it.
        doc.undo();
        assert_eq!(doc.boundary(), None);
    }

    #[test]
    fn graveyard_apply_op_shifts_and_clamps_origin() {
        let mut g = Graveyard::default();
        let id = g.file("cut".into(), "before".into(), 10, 0, SpanSet::default(), Vec::new(), GraveRegion::Manuscript, false, false);
        assert_eq!(g.get(id).unwrap().words, 1);
        // Insert before the origin → shifts right (10 → 12).
        g.apply_op(&op(0, 0, "xx"));
        assert_eq!(g.get(id).unwrap().origin_pos, 12);
        // Delete spanning the origin → clamps to the deletion point (→ 10).
        g.apply_op(&op(10, 5, ""));
        assert_eq!(g.get(id).unwrap().origin_pos, 10);
        // Wholesale clamp caps it at the new length.
        g.clamp(3);
        assert_eq!(g.get(id).unwrap().origin_pos, 3);
    }

    #[test]
    fn cut_to_graveyard_files_and_undo_of_the_cut_removes_the_entry() {
        let text = "The quick brown fox jumps over the lazy dog, over and over.";
        let mut doc = Document::new(text, SpanSet::default(), BlockMap::default());
        let len = doc.len_bytes();
        let id = doc.cut_to_graveyard(4..len, "The ".into(), 42, false);
        assert_eq!(doc.text(), "The ");
        assert_eq!(doc.graveyard().len(), 1);
        let e = doc.graveyard().get(id).unwrap();
        assert!(e.text.starts_with("quick brown"));
        assert_eq!(e.origin_pos, 4);
        assert_eq!(e.cut_unix, 42);
        // Undo of the cut restores the prose AND removes the entry — the
        // inverse in the same grammar (P13), one step.
        doc.undo();
        assert_eq!(doc.text(), text);
        assert_eq!(doc.graveyard().len(), 0);
        // Redo re-files it.
        doc.redo();
        assert_eq!(doc.graveyard().len(), 1);
        assert_eq!(doc.text(), "The ");
    }

    #[test]
    fn put_back_follows_edits_and_removes_the_entry() {
        let mut doc = Document::new("012345678", SpanSet::default(), BlockMap::default());
        let id = doc.cut_to_graveyard(3..9, "012".into(), 0, false);
        assert_eq!(doc.text(), "012");
        // Type before the origin: the origin_pos rides along.
        doc.edit_bytes(0..0, "XY");
        assert_eq!(doc.text(), "XY012");
        let caret = doc.put_back(id).unwrap();
        assert_eq!(doc.text(), "XY012345678");
        assert_eq!(caret, 11);
        assert!(doc.graveyard().is_empty());
    }

    #[test]
    fn put_back_clamps_into_the_manuscript_never_the_compost() {
        // Rail present; an entry whose origin drifted into the compost must
        // still return to the manuscript (review #62).
        let mut b = BlockMap::new(3);
        b.set_aside_boundary(Some(1));
        let mut doc = Document::new("cc\n\nmanuscript body", SpanSet::default(), b);
        assert_eq!(doc.aside_boundary(), Some(1));
        let base = doc.manuscript_base_char();
        assert!(base > 0);
        let mut g = Graveyard::default();
        let id = g.file("XX".into(), String::new(), 0, 0, SpanSet::default(), Vec::new(), GraveRegion::Manuscript, false, false); // origin drifted into the pile
        doc.set_graveyard(g);
        doc.put_back(id).unwrap();
        assert!(doc.text().starts_with("cc\n\n"), "compost untouched: {}", doc.text());
        assert!(doc.manuscript_slice().0.starts_with("XX"), "landed in the manuscript");
    }

    #[test]
    fn grave_delete_is_undoable() {
        let mut doc = Document::new("something reasonably long to hold", SpanSet::default(), BlockMap::default());
        let id = doc.cut_to_graveyard(0..9, String::new(), 0, false);
        assert_eq!(doc.graveyard().len(), 1);
        doc.grave_delete(id);
        assert_eq!(doc.graveyard().len(), 0);
        doc.undo();
        assert_eq!(doc.graveyard().len(), 1, "delete of an entry is undoable");
    }

    #[test]
    fn put_back_restores_spans_and_block_kinds() {
        // A section: a heading, a paragraph carrying a bold span, a list item —
        // then a manuscript tail to put back before. Cut it all, put it back,
        // and the formatting AND structure must be byte-for-byte what they were
        // (Bug D / P1 — put_back was silently flattening headings to body and
        // dropping inline marks).
        let text = "Heading line\nsome bold here\nlist thing\nkeep this tail";
        let blocks = BlockMap::from_kinds(vec![
            BlockKind::Heading(1),
            BlockKind::Paragraph,
            BlockKind::ListItem { ordered: false, depth: 0 },
            BlockKind::Paragraph,
        ]);
        let mut spans = SpanSet::default();
        spans.add(18..22, InlineAttr::Strong); // "bold" in block 1
        let mut doc = Document::new(text, spans, blocks);
        // Cut the three leading blocks (through their trailing newline), so the
        // origin is the block-aligned start the graveyard's auto-cut produces.
        let cut_end = doc.char_to_byte(39); // start of "keep this tail"
        let id = doc.cut_to_graveyard(0..cut_end, String::new(), 0, false);
        assert_eq!(doc.text(), "keep this tail");
        // The entry carries the structure + formatting.
        let e = doc.graveyard().get(id).unwrap();
        assert_eq!(e.kinds[0], BlockKind::Heading(1));
        assert_eq!(e.kinds[2], BlockKind::ListItem { ordered: false, depth: 0 });
        assert!(e.spans.covers(18..22, &InlineAttr::Strong), "bold captured (cut started at 0)");

        doc.put_back(id);
        assert_eq!(doc.text(), text, "text restored verbatim");
        assert_eq!(doc.blocks().kind(0), &BlockKind::Heading(1), "heading came back a heading");
        assert_eq!(doc.blocks().kind(1), &BlockKind::Paragraph);
        assert_eq!(
            doc.blocks().kind(2),
            &BlockKind::ListItem { ordered: false, depth: 0 },
            "list item came back a list item"
        );
        assert!(doc.spans().covers(18..22, &InlineAttr::Strong), "bold restored in place");
        // Undo peels the whole put-back (text + restored marks/kinds) back off.
        doc.undo();
        assert_eq!(doc.text(), "keep this tail");
        assert_eq!(doc.graveyard().len(), 1, "entry restored by the undo");
    }

    #[test]
    fn graveyard_entry_without_span_kind_fields_still_loads() {
        // A `.strop` written before Bug D has no `spans`/`kinds` keys; serde
        // defaults must fill them in (empty), so the file loads and its entries
        // simply put back as plain text — the old behaviour, never a parse error.
        let json = r#"{"id":7,"text":"a buried line","origin_quote":"before","origin_pos":4,"cut_unix":99,"words":3}"#;
        let e: GraveEntry = serde_json::from_str(json).expect("legacy entry loads");
        assert_eq!(e.text, "a buried line");
        assert_eq!(e.origin_pos, 4);
        assert!(e.spans.spans().is_empty());
        assert!(e.kinds.is_empty());
        // A pre-papercuts entry has no `whole_blocks` key: serde-default false,
        // so it loads and puts back as a plain fragment (§B1). Same for the
        // widening's `blank_sep` — an older entry returns with a plain join.
        assert!(!e.whole_blocks);
        assert!(!e.blank_sep);
        // The whole record round-trips too.
        let g: Graveyard = serde_json::from_str(
            r#"{"entries":[{"id":1,"text":"x","origin_quote":"","origin_pos":0,"cut_unix":0,"words":1}],"next_id":1}"#,
        )
        .expect("legacy graveyard loads");
        assert_eq!(g.len(), 1);
    }

    // ---- papercuts-2026-07: A2 seam / A3 class split + machine law / B1 / B2 ----

    #[test]
    fn enter_ends_the_run_next_char_is_plain() {
        // A2: an expanding span never absorbs an insertion across a newline —
        // Enter at the end of a bold run opens a plain paragraph.
        let mut doc = Document::new("", SpanSet::default(), BlockMap::default());
        doc.edit_bytes_coalescing(0..0, "bold");
        doc.toggle_format(0..4, InlineAttr::Strong);
        // Caret at the right edge of the bold run; press Enter (typed).
        doc.edit_bytes_coalescing(4..4, "\n");
        // The seam did NOT swallow the newline: the run is still 0..4.
        assert!(doc.spans().covers(0..4, &InlineAttr::Strong));
        assert!(!doc.spans().covers(0..5, &InlineAttr::Strong));
        // The next paragraph opens plain.
        doc.edit_bytes_coalescing(5..5, "x");
        assert_eq!(doc.text(), "bold\nx");
        assert!(!doc.spans().covers(5..6, &InlineAttr::Strong));
    }

    #[test]
    fn right_edge_append_with_an_embedded_seam_stops_at_the_newline() {
        // A2 belt (papercuts finding 10): a typed insertion that CONTAINS a
        // newline part-way through — "text\nmore" — is absorbed by an expanding
        // right-edge span only up to the seam. `starts_with('\n')` alone would
        // have let the whole thing stream onto the next paragraph.
        let mut set = SpanSet::default();
        set.add(0..4, InlineAttr::Strong);
        set.apply_op(&op(4, 0, "text\nmore")); // typed at the right edge
        // The span grew by "text" (4 chars) only, ending right at the seam.
        assert_eq!(set.spans(), &[strong(0..8)]);
        // An insertion that OPENS with the seam expands nothing.
        let mut set = SpanSet::default();
        set.add(0..4, InlineAttr::Strong);
        set.apply_op(&op(4, 0, "\nmore"));
        assert_eq!(set.spans(), &[strong(0..4)]);
        // A machine insertion never expands at all, seam or no seam.
        let mut set = SpanSet::default();
        set.add(0..4, InlineAttr::Strong);
        set.apply_op_verbatim(&op(4, 0, "text\nmore"));
        assert_eq!(set.spans(), &[strong(0..4)]);
    }

    #[test]
    fn enter_inside_a_run_splits_it_intact_and_undoes() {
        // A2 (the seam kills momentum, never marks): Enter INSIDE a bold run
        // splits it into two intact spans — both halves stay bold — and one
        // undo restores the single pre-split span.
        let mut doc = Document::new("bold", SpanSet::default(), BlockMap::default());
        doc.toggle_format(0..4, InlineAttr::Strong);
        doc.edit_bytes_coalescing(2..2, "\n"); // "bo\nld"
        assert_eq!(doc.text(), "bo\nld");
        // Both halves are bold (the newline grew the run to keep them marked).
        assert!(doc.spans().covers(0..2, &InlineAttr::Strong)); // "bo"
        assert!(doc.spans().covers(3..5, &InlineAttr::Strong)); // "ld"
        // Undo the Enter: one span restored, caret state returned.
        assert!(matches!(doc.undo(), Some(Some(_))));
        assert_eq!(doc.text(), "bold");
        assert_eq!(doc.spans().spans().len(), 1);
        assert!(doc.spans().covers(0..4, &InlineAttr::Strong));
    }

    #[test]
    fn extent_marks_do_not_grow_by_appending() {
        // A3: Highlight and Strikethrough are extent-class — marked once, they
        // do not grow when the writer types at their right edge. Strong stays
        // emphasis-class (the typing hand extends it).
        for extent in [InlineAttr::Highlight, InlineAttr::Strikethrough] {
            let mut doc = Document::new("word", SpanSet::default(), BlockMap::default());
            doc.toggle_format(0..4, extent.clone());
            doc.edit_bytes_coalescing(4..4, "s"); // type at the right edge
            assert_eq!(doc.text(), "words");
            assert!(!doc.spans().covers(4..5, &extent), "{extent:?} must not grow");
            assert!(doc.spans().covers(0..4, &extent), "{extent:?} keeps its extent");
        }
        // Bold at the right edge still grows (the convention).
        let mut doc = Document::new("word", SpanSet::default(), BlockMap::default());
        doc.toggle_format(0..4, InlineAttr::Strong);
        doc.edit_bytes_coalescing(4..4, "s");
        assert!(doc.spans().covers(0..5, &InlineAttr::Strong), "bold extends by typing");
    }

    #[test]
    fn machine_insertion_never_absorbs_into_a_neighbour_span() {
        // A3 machine-insertion law: put back of a partial entry at the right
        // edge of a neighbour's Strong span is NOT dressed in that bold.
        let mut spans = SpanSet::default();
        spans.add(0..4, InlineAttr::Strong); // "bold" bold; "X" plain
        let mut doc = Document::new("boldX", spans, BlockMap::default());
        // Cut the trailing plain "X" (a partial, non-block cut).
        let id = doc.cut_to_graveyard(4..5, String::new(), 0, false);
        assert_eq!(doc.text(), "bold");
        assert!(!doc.graveyard().get(id).unwrap().whole_blocks);
        // Put it back at the bold run's right edge: verbatim, so no expansion.
        doc.put_back(id);
        assert_eq!(doc.text(), "boldX");
        assert!(doc.spans().covers(0..4, &InlineAttr::Strong));
        assert!(!doc.spans().covers(4..5, &InlineAttr::Strong), "X not absorbed into bold");
    }

    #[test]
    fn whole_paragraph_exile_leaves_no_grave_and_undoes_in_one_step() {
        // B1: exiling a whole middle paragraph takes its bounding separator —
        // no empty block — and Ctrl+Z restores text + separator and drops the
        // entry in one step.
        let mut doc = Document::new("AAA\nBBB\nCCC", SpanSet::default(), BlockMap::default());
        let id = doc.exile_to_graveyard(4..7, String::new(), 0, false); // "BBB"
        assert_eq!(doc.text(), "AAA\nCCC");
        assert_eq!(doc.blocks().len(), 2, "no empty block strands");
        let e = doc.graveyard().get(id).unwrap();
        assert!(e.whole_blocks);
        assert_eq!(e.text, "BBB");
        // One undo restores the paragraph AND its separator, entry gone.
        doc.undo();
        assert_eq!(doc.text(), "AAA\nBBB\nCCC");
        assert_eq!(doc.blocks().len(), 3);
        assert!(doc.graveyard().is_empty());
    }

    #[test]
    fn whole_block_exile_normalizes_trailing_newline_selection() {
        // B1 normalization: a triple-click-shaped selection ending at the next
        // block's char 0 (includes the trailing \n) yields the identical
        // outcome — one separator consumed, never two.
        let mut doc = Document::new("AAA\nBBB\nCCC", SpanSet::default(), BlockMap::default());
        let id = doc.exile_to_graveyard(4..8, String::new(), 0, false); // "BBB\n"
        assert_eq!(doc.text(), "AAA\nCCC");
        assert_eq!(doc.blocks().len(), 2);
        assert_eq!(doc.graveyard().get(id).unwrap().text, "BBB");
    }

    #[test]
    fn whole_block_put_back_rebuilds_its_own_paragraph() {
        // B2: a whole-block entry returns as its own standing paragraph wearing
        // its own kind + spans, neighbours untouched, even after edits shift
        // the origin.
        let mut spans = SpanSet::default();
        spans.add(4..7, InlineAttr::Strong); // "BBB" bold
        let mut doc = Document::new("AAA\nBBB\nCCC", spans, BlockMap::default());
        let id = doc.exile_to_graveyard(4..7, String::new(), 0, false);
        assert_eq!(doc.text(), "AAA\nCCC");
        // An edit before the origin drifts it; put back must still land as a
        // block, not a splice.
        doc.edit_bytes(0..0, "Z"); // "ZAAA\nCCC"
        let caret = doc.put_back(id).unwrap();
        assert_eq!(doc.text(), "ZAAA\nBBB\nCCC");
        assert_eq!(doc.blocks().len(), 3, "BBB is its own block");
        // BBB wears its own bold; the neighbours do not.
        let bbb_start = doc.rope().char_to_line(caret); // caret is at BBB's start
        assert_eq!(bbb_start, 1);
        assert!(doc.spans().covers(5..8, &InlineAttr::Strong), "BBB bold restored");
        assert!(!doc.spans().covers(0..4, &InlineAttr::Strong), "ZAAA not bold");
        assert!(!doc.spans().covers(9..12, &InlineAttr::Strong), "CCC not bold");
        assert_eq!(caret, 5, "caret at the returned block's start");
    }

    #[test]
    fn whole_document_exile_and_put_back_has_no_phantom_block() {
        // The lone-block case: exiling the whole document takes no separator
        // (there is none), and put back into the emptied doc lands bare — no
        // phantom leading blank block.
        let mut doc = Document::new("only", SpanSet::default(), BlockMap::default());
        let id = doc.exile_to_graveyard(0..4, String::new(), 0, false);
        assert_eq!(doc.text(), "");
        assert_eq!(doc.blocks().len(), 1);
        doc.put_back(id);
        assert_eq!(doc.text(), "only");
        assert_eq!(doc.blocks().len(), 1);
    }

    #[test]
    fn last_block_exile_and_put_back_round_trip() {
        // B1/B2: the last block of the document consumes its LEADING separator
        // on exile and rebuilds as a trailing block on put back.
        let mut doc = Document::new("AAA\nBBB\nCCC", SpanSet::default(), BlockMap::default());
        let id = doc.exile_to_graveyard(8..11, String::new(), 0, false); // "CCC"
        assert_eq!(doc.text(), "AAA\nBBB");
        assert_eq!(doc.blocks().len(), 2, "no trailing empty block");
        let caret = doc.put_back(id).unwrap();
        assert_eq!(doc.text(), "AAA\nBBB\nCCC");
        assert_eq!(doc.blocks().len(), 3);
        assert_eq!(caret, 8, "caret at CCC's start");
    }

    // ---- exile widens over blank separators (adjudicated: shared with
    // ---- Set aside — the verbs still differ by destination and record) ----

    #[test]
    fn exile_widens_over_the_blank_separator_and_round_trips_byte_identical() {
        // Exiling BBB from blank-separated prose leaves "AAA\n\nCCC" — never
        // "AAA\n\n\nCCC" with the caret stranded on a stacked blank — and
        // Put back restores the paragraph AND its blank separator.
        let mut doc =
            Document::new("AAA\n\nBBB\n\nCCC", SpanSet::default(), BlockMap::default());
        let id = doc.exile_to_graveyard(5..8, String::new(), 0, false); // "BBB"
        assert_eq!(doc.text(), "AAA\n\nCCC", "the prose closes up");
        let e = doc.graveyard().get(id).unwrap();
        assert!(e.whole_blocks);
        assert!(e.blank_sep, "the entry records the blank separator");
        assert_eq!(e.text, "BBB");
        let caret = doc.put_back(id).unwrap();
        assert_eq!(doc.text(), "AAA\n\nBBB\n\nCCC", "byte-identical round trip");
        assert_eq!(caret, 5, "caret at the returned block's start");
        assert!(doc.graveyard().is_empty());
    }

    #[test]
    fn blank_separator_exile_stays_a_single_history_atom() {
        let mut doc =
            Document::new("AAA\n\nBBB\n\nCCC", SpanSet::default(), BlockMap::default());
        doc.exile_to_graveyard(5..8, String::new(), 0, false);
        assert_eq!(doc.text(), "AAA\n\nCCC");
        // ONE undo restores the paragraph AND its blank separator, entry gone.
        doc.undo();
        assert_eq!(doc.text(), "AAA\n\nBBB\n\nCCC");
        assert_eq!(doc.blocks().len(), 5);
        assert!(doc.graveyard().is_empty());
    }

    #[test]
    fn blank_separator_exile_covers_the_document_edges() {
        // First block: the trailing blank goes with it.
        let mut doc = Document::new("BBB\n\nCCC", SpanSet::default(), BlockMap::default());
        let id = doc.exile_to_graveyard(0..3, String::new(), 0, false);
        assert_eq!(doc.text(), "CCC");
        assert!(doc.graveyard().get(id).unwrap().blank_sep);
        doc.put_back(id).unwrap();
        assert_eq!(doc.text(), "BBB\n\nCCC", "first block round trip");

        // Last block: the LEADING blank goes — no stacked blanks strand at
        // the tail — and put back synthesizes the missing breaks back.
        let mut doc = Document::new("AAA\n\nCCC", SpanSet::default(), BlockMap::default());
        let id = doc.exile_to_graveyard(5..8, String::new(), 0, false);
        assert_eq!(doc.text(), "AAA");
        assert!(doc.graveyard().get(id).unwrap().blank_sep);
        let caret = doc.put_back(id).unwrap();
        assert_eq!(doc.text(), "AAA\n\nCCC", "last block round trip");
        assert_eq!(caret, 5, "caret at CCC's start");

        // A lone block has no separator to widen over.
        let mut doc = Document::new("only", SpanSet::default(), BlockMap::default());
        let id = doc.exile_to_graveyard(0..4, String::new(), 0, false);
        assert_eq!(doc.text(), "");
        assert!(!doc.graveyard().get(id).unwrap().blank_sep);
        doc.put_back(id).unwrap();
        assert_eq!(doc.text(), "only");
    }

    #[test]
    fn exile_widening_never_reads_a_textless_furniture_line_as_a_blank() {
        // [P, Image{empty caption}, P]: the image's empty line is a STANDING
        // block (inline-images §5), not a blank separator — exiling AAA takes
        // one join as before, never the picture's line with it.
        let mut doc = Document::new(
            "AAA\n\nBBB",
            SpanSet::default(),
            BlockMap::from_kinds(vec![
                BlockKind::Paragraph,
                BlockKind::Image { src: "asset:pic".into(), alt: String::new() },
                BlockKind::Paragraph,
            ]),
        );
        let id = doc.exile_to_graveyard(0..3, String::new(), 0, false);
        assert_eq!(doc.text(), "\nBBB", "the image's line still stands");
        assert!(!doc.graveyard().get(id).unwrap().blank_sep);
    }

    #[test]
    fn exile_widening_never_chews_the_seam_blank_line() {
        // The manuscript's last block with a pile below: the blank after BBB
        // is the seam's join, not a separator — the widening stops at the
        // region edge and takes the LEADING blank instead, leaving the seam
        // and the pile exactly as they stood.
        let mut doc = Document::new("x", SpanSet::default(), BlockMap::default());
        let mut b = BlockMap::new(5);
        b.set_scrap_line(Some(3));
        doc.restore_state("AAA\n\nBBB\n\npile", SpanSet::default(), b);
        let id = doc.exile_to_graveyard(5..8, String::new(), 0, false); // "BBB"
        assert_eq!(doc.text(), "AAA\n\npile");
        assert_eq!(doc.manuscript_slice().0, "AAA", "manuscript closed up");
        assert!(doc.text().ends_with("pile"), "the pile untouched");
        let e = doc.graveyard().get(id).unwrap();
        assert!(e.blank_sep);
        assert_eq!(e.region, GraveRegion::Manuscript);
        doc.undo();
        assert_eq!(doc.text(), "AAA\n\nBBB\n\npile", "one undo restores it all");
        assert!(doc.graveyard().is_empty());
    }

    // ---- papercuts follow-up (2026-07-11 owner round): reports 3 & 4 ----

    #[test]
    fn plain_deleted_whole_paragraph_returns_standing_not_spliced() {
        // Report 3: select a full middle paragraph with the mouse (text only,
        // no bounding \n), plain-delete it (auto-files, exact bytes), sweep
        // the leftover empty line with a second Backspace, put back. The
        // text must stand as its own paragraph wearing its own kind and
        // spans — glued to nobody.
        let blocks = BlockMap::from_kinds(vec![
            BlockKind::Paragraph,
            BlockKind::Heading(2),
            BlockKind::Paragraph,
        ]);
        let mut spans = SpanSet::default();
        spans.add(4..7, InlineAttr::Strong); // "BBB" bold
        let mut doc = Document::new("AAA\nBBB\nCCC", spans, blocks);
        let id = doc.cut_to_graveyard(4..7, String::new(), 0, false); // plain delete
        // The exact-byte ruling stands: the empty grave line remains.
        assert_eq!(doc.text(), "AAA\n\nCCC");
        let e = doc.graveyard().get(id).unwrap();
        assert!(e.whole_blocks, "a complete-block plain delete records block-ness");
        assert_eq!(e.text, "BBB");
        // The second Backspace sweeps the leftover empty line.
        doc.edit_bytes(3..4, "");
        assert_eq!(doc.text(), "AAA\nCCC");
        let caret = doc.put_back(id).unwrap();
        assert_eq!(doc.text(), "AAA\nBBB\nCCC", "stands as its own paragraph");
        assert_eq!(caret, 4, "caret at the returned block's start");
        assert_eq!(doc.blocks().kind(1), &BlockKind::Heading(2), "its own kind restored");
        assert!(doc.spans().covers(4..7, &InlineAttr::Strong), "its own bold, nobody else's");
        assert!(!doc.spans().covers(0..3, &InlineAttr::Strong));
        assert!(!doc.spans().covers(8..11, &InlineAttr::Strong));
    }

    #[test]
    fn plain_delete_with_trailing_newline_normalizes_and_returns_standing() {
        // The reversed order: the selection grabbed the trailing separator
        // too (shift+down / triple-click shape), so ONE Backspace leaves no
        // empty line. B1's normalization classifies it; the entry sheds the
        // separator so put back rebuilds exactly one, never two.
        let mut doc = Document::new("AAA\nBBB\nCCC", SpanSet::default(), BlockMap::default());
        let id = doc.cut_to_graveyard(4..8, String::new(), 0, false); // "BBB\n"
        assert_eq!(doc.text(), "AAA\nCCC", "exact bytes: no leftover line this way");
        let e = doc.graveyard().get(id).unwrap();
        assert!(e.whole_blocks);
        assert_eq!(e.text, "BBB", "the entry sheds the bounding separator");
        let caret = doc.put_back(id).unwrap();
        assert_eq!(doc.text(), "AAA\nBBB\nCCC");
        assert_eq!(caret, 4);
        assert_eq!(doc.blocks().len(), 3);
    }

    #[test]
    fn plain_deleted_last_paragraph_put_back_fills_the_leftover_grave() {
        // Put back BEFORE the writer sweeps the empty line: the return fills
        // the standing grave block rather than opening a second blank.
        let mut doc = Document::new("AAA\nBBB", SpanSet::default(), BlockMap::default());
        let id = doc.cut_to_graveyard(4..7, String::new(), 0, false); // "BBB"
        assert_eq!(doc.text(), "AAA\n");
        assert!(doc.graveyard().get(id).unwrap().whole_blocks);
        let caret = doc.put_back(id).unwrap();
        assert_eq!(doc.text(), "AAA\nBBB", "fills the grave, no doubled blank");
        assert_eq!(caret, 4);
        assert_eq!(doc.blocks().len(), 2);
    }

    #[test]
    fn immediate_middle_paragraph_put_back_fills_the_leftover_grave() {
        let mut doc = Document::new("AAA\nBBB\nCCC", SpanSet::default(), BlockMap::default());
        let id = doc.cut_to_graveyard(4..7, String::new(), 0, false);
        assert_eq!(doc.text(), "AAA\n\nCCC");
        let caret = doc.put_back(id).unwrap();
        assert_eq!(doc.text(), "AAA\nBBB\nCCC");
        assert_eq!(caret, 4);
        assert_eq!(doc.blocks().len(), 3);
    }

    #[test]
    fn unicode_break_whole_block_doors_put_back_without_stray_break_chars() {
        for sep in ["\n", "\r\n", "\r", "\u{0085}", "\u{2028}"] {
            let original = format!("AAA{sep}BBB{sep}CCC");
            let start = 3 + sep.len();
            let end = start + 3;

            let mut exiled =
                Document::new(&original, SpanSet::default(), BlockMap::default());
            let id = exiled.exile_to_graveyard(
                start..end,
                String::new(),
                0,
                false,
            );
            assert!(exiled.graveyard().get(id).unwrap().whole_blocks, "{sep:?}");
            exiled.put_back(id).unwrap();
            let authored = format!("AAA{sep}BBB\nCCC");
            assert_eq!(exiled.text(), authored, "{sep:?}");
            assert_eq!(exiled.blocks().len(), 3, "{sep:?}");

            let mut deleted =
                Document::new(&original, SpanSet::default(), BlockMap::default());
            let id = deleted.cut_to_graveyard(start..end, String::new(), 0, false);
            assert!(deleted.graveyard().get(id).unwrap().whole_blocks, "{sep:?}");
            deleted.put_back(id).unwrap();
            assert_eq!(deleted.text(), original, "{sep:?}");
            assert_eq!(deleted.blocks().len(), 3, "{sep:?}");
        }
    }

    #[test]
    fn partial_selection_plain_delete_still_splices() {
        // Anything short of complete blocks keeps today's fragment contract.
        let mut doc = Document::new("AAA\nBBB\nCCC", SpanSet::default(), BlockMap::default());
        let id = doc.cut_to_graveyard(5..7, String::new(), 0, false); // "BB" mid-block
        assert!(!doc.graveyard().get(id).unwrap().whole_blocks);
        assert_eq!(doc.text(), "AAA\nB\nCCC");
        doc.put_back(id).unwrap();
        assert_eq!(doc.text(), "AAA\nBBB\nCCC", "a fragment splices back in place");
        assert_eq!(doc.blocks().len(), 3);
    }

    // ---- §4's type-over amendment: anything big that leaves in one
    // ---- stroke, survives — with or without replacement text ----

    #[test]
    fn type_over_files_the_removed_text_and_undoes_in_one_step() {
        let long = "x".repeat(100);
        let original = format!("AAA\n{long}\nCCC");
        let mut doc = Document::new(&original, SpanSet::default(), BlockMap::default());
        let id = doc.replace_to_graveyard(4..104, "NEW", false, "AAA".into(), 7);
        assert_eq!(doc.text(), "AAA\nNEW\nCCC");
        let e = doc.graveyard().get(id).unwrap();
        assert_eq!(e.text, long, "the entry holds the REMOVED text, not the replacement");
        assert!(e.whole_blocks, "a complete-block type-over records block-ness");
        // ONE undo restores the old text, peels the replacement, and drops
        // the entry together (the cut law's parity).
        doc.undo();
        assert_eq!(doc.text(), original);
        assert!(doc.graveyard().is_empty());
    }

    #[test]
    fn paste_over_everything_files_exactly_one_entry_with_the_old_text() {
        // Select-all + paste: the machine-performed replacement (verbatim
        // span anchoring) still files ONE entry carrying the whole old text.
        let mut doc = Document::new("AAA\nBBB\nCCC", SpanSet::default(), BlockMap::default());
        let end = doc.len_bytes();
        doc.replace_to_graveyard(0..end, "fresh start", true, String::new(), 0);
        assert_eq!(doc.text(), "fresh start");
        assert_eq!(doc.graveyard().len(), 1);
        assert_eq!(doc.graveyard().entries()[0].text, "AAA\nBBB\nCCC");
        doc.undo();
        assert_eq!(doc.text(), "AAA\nBBB\nCCC");
        assert!(doc.graveyard().is_empty());
    }

    #[test]
    fn type_over_across_a_furniture_wall_clamps_and_files_the_actual_cuts() {
        // §2 wall law under the amendment: the walls and the picture stand,
        // the replacement lands at the range start's side, and each ACTUAL
        // sub-deletion files its own entry at post-edit origins — never the
        // raw slice, which would grave bytes still standing.
        let mut doc = Document::new("0123456789\ncap\nabcdefghij", SpanSet::default(), {
            let mut b = BlockMap::new(3);
            b.set_kind(
                1,
                BlockKind::Image { src: "asset:img".into(), alt: String::new() },
            );
            b
        });
        doc.replace_to_graveyard(2..13, "XX", false, String::new(), 0);
        assert_eq!(doc.text(), "01XX\np\nabcdefghij");
        assert!(doc.blocks().kind(1).is_furniture(), "the picture survives the type-over");
        assert_eq!(doc.graveyard().len(), 2, "one entry per surviving sub-deletion");
        let entries = doc.graveyard().entries();
        assert_eq!(entries[0].text, "23456789");
        assert_eq!(entries[0].origin_pos, 4, "post-edit: after the landed replacement");
        assert_eq!(entries[1].text, "ca");
        assert_eq!(entries[1].origin_pos, 5);
        // One undo restores text, walls, and empties the graveyard.
        doc.undo();
        assert_eq!(doc.text(), "0123456789\ncap\nabcdefghij");
        assert!(doc.graveyard().is_empty());
    }

    #[test]
    fn image_insert_stands_alone_and_carets_the_following_block() {
        // Report 4: an image is its own block; the caret lands at the START
        // of the following block, never on the image.
        let mut doc = Document::new("AAA\nBBB", SpanSet::default(), BlockMap::default());
        let caret = doc.insert_image_block(1, "asset:img1".into()); // cursor inside AAA
        assert_eq!(doc.text(), "AAA\n\nBBB");
        assert_eq!(doc.blocks().len(), 3);
        assert!(
            matches!(doc.blocks().kind(1), BlockKind::Image { src, .. } if src == "asset:img1")
        );
        assert_eq!(doc.blocks().kind(2), &BlockKind::Paragraph);
        assert_eq!(caret, 5, "caret at the start of the FOLLOWING block");
        assert_eq!(doc.block_of_byte(caret), 2, "typing lands after the image, never in it");
        // Undo removes the block and its kind stamp in one step.
        doc.undo();
        assert_eq!(doc.text(), "AAA\nBBB");
        assert_eq!(doc.blocks().len(), 2);
        assert_eq!(doc.blocks().kind(1), &BlockKind::Paragraph);
    }

    #[test]
    fn image_insert_at_document_end_opens_a_paragraph_after() {
        // The image landing last opens an empty paragraph for the caret.
        let mut doc = Document::new("AAA", SpanSet::default(), BlockMap::default());
        let caret = doc.insert_image_block(2, "asset:img2".into());
        assert_eq!(doc.text(), "AAA\n\n");
        assert_eq!(doc.blocks().len(), 3);
        assert!(matches!(doc.blocks().kind(1), BlockKind::Image { .. }));
        assert_eq!(doc.blocks().kind(2), &BlockKind::Paragraph);
        assert_eq!(caret, 5);
        assert_eq!(doc.block_of_byte(caret), 2);
    }

    #[test]
    fn image_insert_full_lands_caption_alt_and_spans_in_one_step() {
        // §9's rebuild: the travelling form's caption (text + spans) and
        // alt land WITH the block, one transaction, one undo step.
        let mut doc = Document::new("AAA\nBBB", SpanSet::default(), BlockMap::default());
        let mut spans = SpanSet::default();
        spans.add(0..4, InlineAttr::Emphasis);
        let caret =
            doc.insert_image_block_full(1, "asset:img1".into(), "a cat".into(), "look here", &spans);
        assert_eq!(doc.text(), "AAA\nlook here\nBBB");
        assert!(matches!(
            doc.blocks().kind(1),
            BlockKind::Image { src, alt } if src == "asset:img1" && alt == "a cat"
        ));
        assert_eq!(caret, 14, "caret at the start of the FOLLOWING block");
        assert!(
            doc.spans().attrs_at(4).any(|a| *a == InlineAttr::Emphasis),
            "caption spans re-anchor onto the caption line"
        );
        doc.undo();
        assert_eq!(doc.text(), "AAA\nBBB", "one undo takes the whole rebuild back");
        assert_eq!(doc.blocks().kind(1), &BlockKind::Paragraph);
        assert_eq!(doc.spans().spans().len(), 0);
    }

    #[test]
    fn image_insert_before_first_stamps_both_sides() {
        // §7's topmost gap: the new picture stands BEFORE block 0, and the
        // pushed-down line keeps its old kind — for a flowing first block
        // and for a furniture one alike (neither split law may misdress).
        let mut doc = Document::new(
            "Head\nBBB",
            SpanSet::default(),
            BlockMap::from_kinds(vec![BlockKind::Heading(1), BlockKind::Paragraph]),
        );
        let after = doc.insert_image_block_before_first("asset:top".into());
        assert_eq!(doc.text(), "\nHead\nBBB");
        assert!(matches!(doc.blocks().kind(0), BlockKind::Image { src, .. } if src == "asset:top"));
        assert_eq!(doc.blocks().kind(1), &BlockKind::Heading(1));
        assert_eq!(after, 1, "byte after the image block = the old first block");
        doc.undo();
        assert_eq!(doc.text(), "Head\nBBB");
        assert_eq!(doc.blocks().kind(0), &BlockKind::Heading(1));

        // Furniture first block: the old picture keeps its pixels on the
        // pushed-down line; the new one stands above it.
        let mut doc = Document::new(
            "cap\nBBB",
            SpanSet::default(),
            BlockMap::from_kinds(vec![img("asset:old"), BlockKind::Paragraph]),
        );
        doc.insert_image_block_before_first("asset:new".into());
        assert_eq!(doc.text(), "\ncap\nBBB");
        assert!(matches!(doc.blocks().kind(0), BlockKind::Image { src, .. } if src == "asset:new"));
        assert!(matches!(doc.blocks().kind(1), BlockKind::Image { src, .. } if src == "asset:old"));
    }

    // ---- the §2 wall law: furniture vs the edit path ------------------

    fn img(src: &str) -> BlockKind {
        BlockKind::Image {
            src: src.into(),
            alt: String::new(),
        }
    }

    /// "AAA\ncap\nBBB" with the middle line an image whose caption is its
    /// own text (the §1 ontology): AAA 0..3, sep 3, cap 4..7, sep 7,
    /// BBB 8..11.
    fn cap_doc() -> Document {
        Document::new(
            "AAA\ncap\nBBB",
            SpanSet::default(),
            BlockMap::from_kinds(vec![BlockKind::Paragraph, img("asset:a"), BlockKind::Paragraph]),
        )
    }

    fn plan(doc: &Document, range: Range<usize>) -> ClampPlan {
        clamp_plan(doc.rope(), doc.blocks(), &range)
    }

    #[test]
    fn clamp_plan_stops_partial_ranges_at_the_wall() {
        let doc = cap_doc();
        // Mid-paragraph into mid-caption: bytes on each side go, the
        // separator at the wall survives.
        assert_eq!(
            plan(&doc, 1..5),
            ClampPlan { cuts: vec![1..3, 4..5], insert_at: 1 }
        );
        // From inside the caption out below: same wall, other polarity.
        assert_eq!(
            plan(&doc, 5..9),
            ClampPlan { cuts: vec![5..7, 8..9], insert_at: 5 }
        );
        // A separator-only range is refused whole — no cuts at all. Both
        // walls, both polarities (the today-bug's Backspace drain, §0).
        assert_eq!(plan(&doc, 3..4).cuts, Vec::<Range<usize>>::new());
        assert_eq!(plan(&doc, 7..8).cuts, Vec::<Range<usize>>::new());
    }

    #[test]
    #[allow(clippy::single_range_in_vec_init)] // one-cut plans are table rows, not typos
    fn clamp_plan_takes_a_covered_block_whole() {
        let doc = cap_doc();
        // Separator + caption + separator: enclosed, taken by the PRECEDING
        // separator (merge-keeps-first then drains the image kind); the
        // following separator stands.
        assert_eq!(plan(&doc, 3..8), ClampPlan { cuts: vec![3..7], insert_at: 3 });
        // Whole-cover plus both partial flanks: left-partial + whole-cover
        // + right-partial (adjudicated pushback 5). The flanks do NOT fuse —
        // the surviving separator stands between them.
        assert_eq!(
            plan(&doc, 1..10),
            ClampPlan { cuts: vec![1..7, 8..10], insert_at: 1 }
        );
        // A non-empty caption plus ONE separator is the §5 range door too.
        assert_eq!(plan(&doc, 4..8), ClampPlan { cuts: vec![4..8], insert_at: 4 });
    }

    #[test]
    #[allow(clippy::single_range_in_vec_init)] // one-cut plans are table rows, not typos
    fn clamp_plan_empty_caption_demands_full_enclosure() {
        // "AAA\n\nBBB": empty caption at 4..4, seps 3..4 and 4..5. A lone
        // separator range must never drain the picture; both separators
        // covered takes it whole (consuming the preceding one).
        let doc = Document::new(
            "AAA\n\nBBB",
            SpanSet::default(),
            BlockMap::from_kinds(vec![BlockKind::Paragraph, img("asset:a"), BlockKind::Paragraph]),
        );
        assert_eq!(plan(&doc, 3..4).cuts, Vec::<Range<usize>>::new());
        assert_eq!(plan(&doc, 4..5).cuts, Vec::<Range<usize>>::new());
        assert_eq!(plan(&doc, 3..5), ClampPlan { cuts: vec![3..4], insert_at: 3 });
    }

    #[test]
    #[allow(clippy::single_range_in_vec_init)] // one-cut plans are table rows, not typos
    fn clamp_plan_walls_between_adjacent_furniture() {
        // "AAA\nc1\nc2\nBBB", two images back to back: the shared separator
        // is a wall for both; a range across it clamps on both sides.
        let doc = Document::new(
            "AAA\nc1\nc2\nBBB",
            SpanSet::default(),
            BlockMap::from_kinds(vec![
                BlockKind::Paragraph,
                img("asset:a"),
                img("asset:b"),
                BlockKind::Paragraph,
            ]),
        );
        assert_eq!(
            plan(&doc, 5..8),
            ClampPlan { cuts: vec![5..6, 7..8], insert_at: 5 }
        );
    }

    #[test]
    #[allow(clippy::single_range_in_vec_init)] // one-cut plans are table rows, not typos
    fn clamp_plan_whole_cover_reclaims_a_wall_the_neighbour_left_standing() {
        // Two images back to back, the SECOND fully enclosed (content plus
        // both bounding separators): the shared wall was already pushed as
        // img1's protected FOLLOWING separator one iteration earlier, so
        // consume_prec must reclaim it — or the cover is silently void and
        // the enclosed picture drains to the §0 ghost (§2 whole cover,
        // §5's range door).
        let doc = Document::new(
            "c1\nc2\nBBB",
            SpanSet::default(),
            BlockMap::from_kinds(vec![img("asset:a"), img("asset:b"), BlockKind::Paragraph]),
        );
        assert_eq!(plan(&doc, 2..6), ClampPlan { cuts: vec![2..5], insert_at: 2 });
        // A chain of enclosed EMPTY captions resolves left to right: each
        // cover consumes its own preceding separator — including the wall
        // the previous cover just left protected.
        let chain = Document::new(
            "AAA\n\n\nBBB",
            SpanSet::default(),
            BlockMap::from_kinds(vec![
                BlockKind::Paragraph,
                img("asset:a"),
                img("asset:b"),
                BlockKind::Paragraph,
            ]),
        );
        assert_eq!(plan(&chain, 3..6), ClampPlan { cuts: vec![3..5], insert_at: 3 });
    }

    #[test]
    #[allow(clippy::single_range_in_vec_init)] // one-cut plans are table rows, not typos
    fn clamp_plan_furniture_at_document_edges() {
        // Image first: no preceding separator exists, so enclosure covers
        // content + the following one; the cover runs on into the flank.
        let start = Document::new(
            "cap\nBBB",
            SpanSet::default(),
            BlockMap::from_kinds(vec![img("asset:a"), BlockKind::Paragraph]),
        );
        assert_eq!(plan(&start, 0..6), ClampPlan { cuts: vec![0..6], insert_at: 0 });
        // Image last: no following separator; the preceding one is the take.
        let end = Document::new(
            "AAA\ncap",
            SpanSet::default(),
            BlockMap::from_kinds(vec![BlockKind::Paragraph, img("asset:a")]),
        );
        assert_eq!(plan(&end, 1..7), ClampPlan { cuts: vec![1..7], insert_at: 1 });
    }

    #[test]
    #[allow(clippy::single_range_in_vec_init)] // one-cut plans are table rows, not typos
    fn clamp_plan_replacement_starting_in_a_wall_lands_left() {
        // A replacement whose range STARTS inside a surviving separator
        // steps back onto the wall's left face (the range start's side).
        let doc = cap_doc();
        assert_eq!(plan(&doc, 3..6), ClampPlan { cuts: vec![4..6], insert_at: 3 });
        // A CRLF wall: '\r' and '\n' are separate chars, so a char-aligned
        // range CAN start between them — the landing steps back onto the
        // break's first byte, and the cut spares the break whole.
        let crlf = Document::new(
            "AAA\r\ncap\r\nBBB",
            SpanSet::default(),
            BlockMap::from_kinds(vec![BlockKind::Paragraph, img("asset:a"), BlockKind::Paragraph]),
        );
        assert_eq!(plan(&crlf, 4..7), ClampPlan { cuts: vec![5..7], insert_at: 3 });
    }

    #[test]
    fn wall_clamped_delete_spares_both_blocks_and_undoes_in_one_step() {
        let mut doc = cap_doc();
        doc.take_ops();
        doc.edit_bytes(1..5, "");
        assert_eq!(doc.text(), "A\nap\nBBB", "bytes on each side go, the wall stands");
        assert!(
            matches!(doc.blocks().kind(1), BlockKind::Image { src, .. } if src == "asset:a"),
            "the picture never rides a text edit"
        );
        // The grouped cuts mirror to the store as ordinary ops.
        let mut mirror: Vec<char> = "AAA\ncap\nBBB".chars().collect();
        for op in doc.take_ops() {
            mirror.splice(op.pos..op.pos + op.delete, op.insert.chars());
        }
        assert_eq!(mirror.iter().collect::<String>(), doc.text(), "Loro mirror diverged");
        // One ctrl-z restores text AND kinds together.
        doc.undo();
        assert_eq!(doc.text(), "AAA\ncap\nBBB");
        assert!(matches!(doc.blocks().kind(1), BlockKind::Image { .. }));
    }

    #[test]
    fn wall_crossing_cut_files_only_the_deleted_bytes_and_put_back_never_clones() {
        // A range cut from mid-paragraph into mid-caption: the clamp spares
        // the wall and the picture, so the graveyard must record only what
        // actually left — filing the raw slice graved a separator that
        // still stands plus the Image kind, and Put back then re-inserted
        // the separator and stamped a SECOND picture from a plain text
        // gesture (§2: no text mechanic may move, clone, or absorb).
        let mut doc = cap_doc();
        doc.cut_to_graveyard(1..6, String::new(), 0, false);
        assert_eq!(doc.text(), "A\np\nBBB", "the clamp spared the wall");
        let images =
            |doc: &Document| doc.blocks().kinds().iter().filter(|k| k.is_furniture()).count();
        assert_eq!(images(&doc), 1);
        let ids: Vec<u64> = doc.graveyard().entries().iter().map(|e| e.id).collect();
        for id in ids {
            doc.put_back(id).expect("every filed entry returns");
        }
        assert_eq!(doc.text(), "AAA\ncap\nBBB", "put back restores exactly the cut");
        assert_eq!(images(&doc), 1, "a text mechanic must never clone the picture");
        assert!(
            matches!(doc.blocks().kind(1), BlockKind::Image { src, .. } if src == "asset:a")
        );
    }

    #[test]
    fn wall_refused_range_leaves_no_undo_step() {
        // A separator-only deletion at the wall is refused whole: nothing
        // changes and no empty transaction pollutes the undo stack.
        let mut doc = cap_doc();
        doc.edit_bytes(7..8, "");
        assert_eq!(doc.text(), "AAA\ncap\nBBB");
        assert_eq!(doc.undo(), None, "a refusal must not be undoable");
    }

    #[test]
    fn wall_whole_cover_takes_the_picture_and_flanks_stand_apart() {
        let mut doc = cap_doc();
        doc.edit_bytes(1..10, "");
        assert_eq!(doc.text(), "A\nB", "flanks do NOT fuse across the taken block");
        assert_eq!(doc.blocks().kinds(), &[BlockKind::Paragraph, BlockKind::Paragraph]);
        doc.undo();
        assert_eq!(doc.text(), "AAA\ncap\nBBB");
        assert!(matches!(doc.blocks().kind(1), BlockKind::Image { .. }));
    }

    #[test]
    fn wall_whole_cover_at_document_start_restamps_the_merged_line() {
        // The document-start image has no preceding separator, so the take
        // consumes the FOLLOWING one — on_edit's merge-keeps-first would
        // strand the image kind on the survivor's text; the restamp hands
        // the line to the first surviving block's kind.
        let mut doc = Document::new(
            "cap\nBBB",
            SpanSet::default(),
            BlockMap::from_kinds(vec![img("asset:a"), BlockKind::Paragraph]),
        );
        doc.edit_bytes(0..6, "");
        assert_eq!(doc.text(), "B");
        assert_eq!(doc.blocks().kinds(), &[BlockKind::Paragraph]);
        doc.undo();
        assert_eq!(doc.text(), "cap\nBBB");
        assert!(matches!(doc.blocks().kind(0), BlockKind::Image { .. }));
    }

    #[test]
    fn wall_whole_cover_beside_a_standing_twin_takes_the_enclosed_picture() {
        // The executor run of the reclaim rule above: deleting the enclosed
        // second image of an adjacent pair must take it WHOLE — never leave
        // it standing with a drained caption (§2's forbidden ghost).
        let mut doc = Document::new(
            "c1\nc2\nBBB",
            SpanSet::default(),
            BlockMap::from_kinds(vec![img("asset:a"), img("asset:b"), BlockKind::Paragraph]),
        );
        doc.edit_bytes(2..6, "");
        assert_eq!(doc.text(), "c1\nBBB");
        assert!(
            matches!(doc.blocks().kind(0), BlockKind::Image { src, .. } if src == "asset:a"),
            "the standing twin keeps its caption and kind"
        );
        assert_eq!(doc.blocks().kind(1), &BlockKind::Paragraph);
        // The empty-caption chain: both enclosed pictures go, the flanks
        // stand apart on the surviving wall.
        let mut chain = Document::new(
            "AAA\n\n\nBBB",
            SpanSet::default(),
            BlockMap::from_kinds(vec![
                BlockKind::Paragraph,
                img("asset:a"),
                img("asset:b"),
                BlockKind::Paragraph,
            ]),
        );
        chain.edit_bytes(3..6, "");
        assert_eq!(chain.text(), "AAA\nBBB");
        assert_eq!(
            chain.blocks().kinds(),
            &[BlockKind::Paragraph, BlockKind::Paragraph],
            "no drained ghost survives the enclosure"
        );
    }

    #[test]
    fn wall_replacement_clamps_the_delete_and_inserts_at_the_start_side() {
        let mut doc = cap_doc();
        doc.edit_bytes(1..5, "XY");
        assert_eq!(doc.text(), "AXY\nap\nBBB", "text lands on the range start's side");
        assert!(matches!(doc.blocks().kind(1), BlockKind::Image { .. }));
        doc.undo();
        assert_eq!(doc.text(), "AAA\ncap\nBBB");
    }

    #[test]
    fn caption_edits_and_wall_insertions_never_clamp() {
        // Entirely inside the caption: ordinary text mechanics, the image
        // kind stays put (P3 — the caption IS text).
        let mut doc = cap_doc();
        doc.edit_bytes(4..6, "");
        assert_eq!(doc.text(), "AAA\np\nBBB");
        assert!(matches!(doc.blocks().kind(1), BlockKind::Image { .. }));
        // A pure insertion at the wall is not a crossing: no clamp, and the
        // separator count grows as typed.
        let mut doc = cap_doc();
        doc.edit_bytes(3..3, "x");
        assert_eq!(doc.text(), "AAAx\ncap\nBBB");
        assert!(matches!(doc.blocks().kind(1), BlockKind::Image { .. }));
    }

    #[test]
    fn split_never_clones_furniture() {
        // §2 split law: the first fragment keeps the furniture kind; every
        // further fragment is born a Paragraph — an Enter inside a caption
        // can never mint a second picture (repro step 8).
        let mut b = BlockMap::from_kinds(vec![BlockKind::Paragraph, img("asset:a"), BlockKind::Paragraph]);
        b.on_edit(1, 0, 1);
        assert!(matches!(b.kind(1), BlockKind::Image { .. }));
        assert_eq!(b.kind(2), &BlockKind::Paragraph);
        assert_eq!(b.kinds().iter().filter(|k| k.is_furniture()).count(), 1);
        // The divider is the same furniture class, same law.
        let mut b = BlockMap::from_kinds(vec![BlockKind::Divider]);
        b.on_edit(0, 0, 2);
        assert_eq!(
            b.kinds(),
            &[BlockKind::Divider, BlockKind::Paragraph, BlockKind::Paragraph]
        );
    }

    #[test]
    fn splitting_a_caption_yields_image_plus_paragraph() {
        // Enter at caption end: the image keeps the head, the tail is a new
        // Paragraph below (§6).
        let mut doc = cap_doc();
        doc.edit_bytes(7..7, "\n");
        assert_eq!(doc.text(), "AAA\ncap\n\nBBB");
        assert!(matches!(doc.blocks().kind(1), BlockKind::Image { .. }));
        assert_eq!(doc.blocks().kind(2), &BlockKind::Paragraph);
        // Enter at caption START: the first fragment (now the empty line)
        // keeps the picture; the caption text rides the new Paragraph. The
        // editor's direction rule (§6, a later phase) will route around
        // this, but the model law must hold on its own.
        let mut doc = cap_doc();
        doc.edit_bytes(4..4, "\n");
        assert_eq!(doc.text(), "AAA\n\ncap\nBBB");
        assert!(matches!(doc.blocks().kind(1), BlockKind::Image { .. }));
        assert_eq!(doc.blocks().kind(2), &BlockKind::Paragraph);
        assert_eq!(doc.blocks().kinds().iter().filter(|k| k.is_furniture()).count(), 1);
        // A multi-line paste into the caption: every inserted fragment is
        // born a Paragraph — one picture, however many lines arrive.
        let mut doc = cap_doc();
        doc.edit_bytes(5..5, "x\ny\nz");
        assert_eq!(doc.text(), "AAA\ncx\ny\nzap\nBBB");
        assert_eq!(doc.blocks().kinds().iter().filter(|k| k.is_furniture()).count(), 1);
        assert!(matches!(doc.blocks().kind(1), BlockKind::Image { .. }));
    }

    #[test]
    fn replace_image_src_swaps_pixels_keeps_alt_and_caption_one_undo() {
        // Spec inline-images §4: replace-in-place is a whole-block verb —
        // new pixels, same block, alt untouched, the caption line never
        // even enters the transaction. One undo step returns the old src.
        let mut doc = Document::new("AAA\na caption\nBBB", SpanSet::default(), {
            let mut b = BlockMap::new(3);
            b.set_kind(1, BlockKind::Image { src: "asset:old.png".into(), alt: "cat".into() });
            b
        });
        doc.replace_image_src(1, "asset:new.png".into());
        assert_eq!(
            doc.blocks().kind(1),
            &BlockKind::Image { src: "asset:new.png".into(), alt: "cat".into() }
        );
        assert_eq!(doc.text(), "AAA\na caption\nBBB", "the caption line is untouched");
        doc.undo();
        assert_eq!(
            doc.blocks().kind(1),
            &BlockKind::Image { src: "asset:old.png".into(), alt: "cat".into() },
            "one undo step restores the old pixels"
        );
        // A decayed selection (the block is no longer an image) is a no-op.
        doc.replace_image_src(0, "asset:other.png".into());
        assert_eq!(doc.blocks().kind(0), &BlockKind::Paragraph);
    }

    #[test]
    fn image_kind_wire_keeps_the_caption_key_for_released_builds() {
        // Build plan, adjudicated pushback 2: a released build's enum
        // REQUIRES `caption`, and its serde error path falls back to the
        // legacy token parser — collapsing the whole BlockMap. So the wire
        // must keep emitting the key. This replica IS the released shape.
        #[derive(Debug, PartialEq, Deserialize)]
        enum OldBlockKind {
            #[allow(dead_code)]
            Paragraph,
            #[allow(dead_code)]
            Heading(u8),
            #[allow(dead_code)]
            Blockquote,
            #[allow(dead_code)]
            ListItem { ordered: bool, depth: u8 },
            #[allow(dead_code)]
            Divider,
            #[allow(dead_code)]
            CodeBlock { info: String },
            Image { src: String, alt: String, caption: String },
            #[allow(dead_code)]
            FootnoteDef { id: String },
        }
        let kinds = vec![
            BlockKind::Paragraph,
            BlockKind::Image { src: "asset:abc.png".into(), alt: "a]b".into() },
        ];
        let json = serde_json::to_string(&kinds).unwrap();
        assert!(json.contains(r#""caption":"""#), "the wire keeps the key: {json}");
        let old: Vec<OldBlockKind> = serde_json::from_str(&json)
            .expect("a released build's REQUIRED-caption serde must parse our output");
        assert_eq!(
            old[1],
            OldBlockKind::Image {
                src: "asset:abc.png".into(),
                alt: "a]b".into(),
                caption: String::new(),
            }
        );
        // The reverse door: an old build's output (caption present, maybe
        // non-empty) parses into the two-field runtime enum, value dropped
        // here (the open-time migration owns the value, not serde).
        let legacy = r#"[{"Image":{"src":"asset:x.png","alt":"","caption":"fig 1"}}]"#;
        let new: Vec<BlockKind> = serde_json::from_str(legacy).unwrap();
        assert_eq!(new[0], BlockKind::Image { src: "asset:x.png".into(), alt: String::new() });
        // And a future captionless wire parses too (the era flip's far side).
        let future = r#"[{"Image":{"src":"asset:y.png","alt":"a"}}]"#;
        let new: Vec<BlockKind> = serde_json::from_str(future).unwrap();
        assert_eq!(new[0], BlockKind::Image { src: "asset:y.png".into(), alt: "a".into() });
    }

    #[test]
    fn manuscript_slice_excludes_compost_and_rebases_span_offsets() {
        let mut b = BlockMap::new(3);
        b.set_aside_boundary(Some(1)); // "COMP","", "MANU tail"
        let mut spans = SpanSet::default();
        spans.add(0..4, InlineAttr::Strong); // in compost "COMP"
        spans.add(6..10, InlineAttr::Emphasis); // "MANU" in the manuscript
        let doc = Document::new("COMP\n\nMANU tail", spans, b);
        assert_eq!(doc.manuscript_char_range(), 6..15);
        let (text, mspans, mblocks) = doc.manuscript_slice();
        assert_eq!(text, "MANU tail");
        // The compost span is excluded; the manuscript span is rebased to 0..4.
        assert!(mspans.covers(0..4, &InlineAttr::Emphasis));
        assert!(!mspans.covers(0..4, &InlineAttr::Strong));
        assert_eq!(mblocks.len(), 1);
        // Export runs on the slice, so the compost never reaches Markdown.
        let md = crate::markdown::to_markdown(&text, &mspans, &mblocks);
        assert!(md.contains("MANU"));
        assert!(!md.contains("COMP"));
    }

    // ---- manuscript_slice_of: the ONE slice both books consume (F1) ----

    /// Top-era state: the pile never enters, and the slice's first words are
    /// the manuscript's true first words (the Past book renders pre-Scraps
    /// checkpoint states through this exact function).
    #[test]
    fn manuscript_slice_of_top_era_pile_never_enters() {
        let rope = ropey::Rope::from_str("pile alpha\n\nManuscript opens here\nsecond line");
        let mut blocks = BlockMap::new(4);
        blocks.set_aside_boundary(Some(1));
        let (text, _, mblocks) = manuscript_slice_of(&rope, &SpanSet::default(), &blocks);
        assert!(text.starts_with("Manuscript opens"), "true first words");
        assert!(!text.contains("pile"), "the pile never enters");
        assert_eq!(mblocks.len(), 2);
        assert_eq!(mblocks.boundary(), None, "the slice carries no seam");
    }

    /// Tail era: the manuscript is the head, the seam and pile are clipped
    /// off, and the joining line break never rides along.
    #[test]
    fn manuscript_slice_of_tail_era_clips_seam_and_pile() {
        let rope = ropey::Rope::from_str("One two three\nmore prose\n\nscrap alpha\nscrap beta");
        let mut blocks = BlockMap::new(5);
        blocks.set_scrap_line(Some(2));
        let (text, _, mblocks) = manuscript_slice_of(&rope, &SpanSet::default(), &blocks);
        assert_eq!(text, "One two three\nmore prose");
        assert_eq!(mblocks.len(), 2);
        assert_eq!(mblocks.boundary(), None);
    }

    /// No boundary: the whole state passes through untouched.
    #[test]
    fn manuscript_slice_of_no_boundary_is_the_whole_doc() {
        let rope = ropey::Rope::from_str("just prose\nand more");
        let mut spans = SpanSet::default();
        spans.add(0..4, InlineAttr::Strong);
        let blocks = BlockMap::new(2);
        let (text, mspans, mblocks) = manuscript_slice_of(&rope, &spans, &blocks);
        assert_eq!(text, "just prose\nand more");
        assert!(mspans.covers(0..4, &InlineAttr::Strong));
        assert_eq!(mblocks.len(), 2);
    }

    /// Everything-compost Top state: the manuscript is empty, and the slice
    /// says so honestly (the empty book renders one blank page — regions 4).
    #[test]
    fn manuscript_slice_of_everything_compost_is_empty() {
        let rope = ropey::Rope::from_str("pile\n");
        let mut blocks = BlockMap::new(2);
        blocks.set_aside_boundary(Some(1));
        let (text, mspans, mblocks) = manuscript_slice_of(&rope, &SpanSet::default(), &blocks);
        assert_eq!(text, "");
        assert!(mspans.spans().is_empty());
        // from_kinds([]) falls back to the one-paragraph invariant, which is
        // exactly what an empty text's rope reports (len_lines == 1).
        assert_eq!(mblocks.len(), 1);
    }

    /// Blank-manuscript Tail doc (regions 15): a blank region slices to the
    /// empty string, never to the seam or the pile.
    #[test]
    fn manuscript_slice_of_blank_tail_manuscript_is_empty() {
        let rope = ropey::Rope::from_str("\n\nscrap text");
        let mut blocks = BlockMap::new(3);
        blocks.set_scrap_line(Some(1));
        let (text, _, mblocks) = manuscript_slice_of(&rope, &SpanSet::default(), &blocks);
        assert_eq!(text, "");
        assert_eq!(mblocks.len(), 1, "one blank manuscript block");
        assert!(!format!("{mblocks:?}").contains("scrap_line: Some"));
    }

    /// Spans straddling the boundary are clipped — the invariant the book's
    /// styling relies on: no span end may exceed the slice length
    /// (regions 10), in either era.
    #[test]
    fn manuscript_slice_of_clips_boundary_straddling_spans() {
        // Tail: manuscript "abcde" (0..5), span 3..9 straddles into the pile.
        let rope = ropey::Rope::from_str("abcde\n\nxyz");
        let mut blocks = BlockMap::new(3);
        blocks.set_scrap_line(Some(1));
        let mut spans = SpanSet::default();
        spans.add(3..9, InlineAttr::Emphasis);
        let (text, mspans, _) = manuscript_slice_of(&rope, &spans, &blocks);
        let len = text.chars().count();
        assert!(mspans.spans().iter().all(|s| s.range.end <= len), "no span past the slice");
        assert!(mspans.covers(3..5, &InlineAttr::Emphasis));
        // Top: manuscript "story" at base 6, span 2..8 straddles in from the
        // pile and clamps to 0..2; a pile-only span vanishes.
        let rope = ropey::Rope::from_str("pile\n\nstory");
        let mut blocks = BlockMap::new(3);
        blocks.set_aside_boundary(Some(1));
        let mut spans = SpanSet::default();
        spans.add(2..8, InlineAttr::Emphasis);
        spans.add(0..2, InlineAttr::Strong);
        let (text, mspans, _) = manuscript_slice_of(&rope, &spans, &blocks);
        let len = text.chars().count();
        assert!(mspans.spans().iter().all(|s| s.range.end <= len));
        assert!(mspans.covers(0..2, &InlineAttr::Emphasis));
        assert!(!mspans.covers(0..2, &InlineAttr::Strong), "pile-only span dropped");
        // And the Document method is the same function (F1: one slice, both
        // books) — the triple matches the free function's exactly.
        let mut b = BlockMap::new(3);
        b.set_scrap_line(Some(1));
        let doc = Document::new("abcde\n\nxyz", SpanSet::default(), b);
        let via_doc = doc.manuscript_slice();
        let rope = ropey::Rope::from_str("abcde\n\nxyz");
        let mut b = BlockMap::new(3);
        b.set_scrap_line(Some(1));
        assert_eq!(via_doc, manuscript_slice_of(&rope, &SpanSet::default(), &b));
    }

    #[test]
    fn new_repairs_untrusted_spans_before_char_to_byte_use() {
        let spans: SpanSet = serde_json::from_str(
            r#"{"spans":[
                {"range":{"start":9,"end":2},"attr":"Strong"},
                {"range":{"start":0,"end":999},"attr":"Emphasis"},
                {"range":{"start":1,"end":3},"attr":"Emphasis"}
            ]}"#,
        )
        .unwrap();
        let doc = Document::new("x", spans, BlockMap::default());
        assert_eq!(doc.spans().spans().len(), 1);
        assert_eq!(doc.spans().spans()[0].range, 0..1);
        // This is the renderer's formerly-panicking conversion.
        assert_eq!(doc.rope().char_to_byte(doc.spans().spans()[0].range.end), 1);
    }

    #[test]
    fn block_count_repair_preserves_a_still_valid_scrap_line() {
        let mut stale = BlockMap::new(5);
        stale.set_scrap_line(Some(2));
        let doc = Document::new("one\ntwo\n\nscrap", SpanSet::default(), stale);
        assert_eq!(doc.blocks().len(), 4);
        assert_eq!(doc.scrap_line(), Some(2));
    }

    #[test]
    fn empty_document_edit_changes_neither_revision_nor_undo() {
        let mut doc = Document::new("abc", SpanSet::default(), BlockMap::default());
        let revision = doc.revision();
        doc.edit_bytes(1..1, "");
        assert_eq!(doc.revision(), revision);
        assert!(doc.take_ops().is_empty());
        assert!(doc.undo().is_none());
    }

    #[test]
    fn migrate_writer_note_to_compost_but_never_a_diagnosis() {
        let mut doc = Document::new("body text here", SpanSet::default(), BlockMap::default());
        let note = doc.add_note(0..4, "my thought".into(), 0);
        assert!(doc.migrate_note_to_compost(note, "body"));
        assert!(doc.notes().get(note).is_none(), "note left the margin");
        assert_eq!(doc.scrap_line(), Some(1), "the note lands in a tail pile");
        assert_eq!(doc.text(), "body text here\n\nbody\nmy thought");
        assert_eq!(doc.blocks().kind(2), &BlockKind::Blockquote, "anchor is a quote");
        assert!(doc.graveyard().is_empty(), "migration is a move, not a cut");
        // Undo restores the note and dissolves the pile.
        doc.undo();
        assert!(doc.notes().get(note).is_some());
        assert_eq!(doc.boundary(), None);
        // A diagnosis is never writer material — it does not migrate.
        doc.add_diagnoses(vec![Annotation {
            id: 0,
            range: 0..4,
            body: "q".into(),
            status: NoteStatus::Open,
            created_unix: 0,
            kind: NoteKind::Diagnosis,
            title: "t".into(),
            level: "line".into(),
            orphaned: true,
            pass_id: 1,
            unverified: false,
        }]);
        let did = doc
            .notes()
            .notes()
            .iter()
            .find(|n| n.kind == NoteKind::Diagnosis)
            .unwrap()
            .id;
        assert!(!doc.migrate_note_to_compost(did, "body"));
    }

    // ---- Scraps (Wave A): era, flip, seam mechanics, migration ----

    /// Era decode (adjudications, "the foundation"): the legacy field means
    /// Top, the new field Tail, serde default Top-era-compatible — every
    /// pre-Scraps JSON decodes exactly as before; when both are present the
    /// tail field wins.
    #[test]
    fn boundary_era_decodes_by_field_and_defaults_top() {
        let legacy: BlockMap =
            serde_json::from_str(r#"{"kinds":["Paragraph","Paragraph","Paragraph"],"aside_boundary":1}"#)
                .unwrap();
        assert_eq!(legacy.boundary(), Some((BoundaryEra::Top, 1)));
        let tail: BlockMap =
            serde_json::from_str(r#"{"kinds":["Paragraph","Paragraph","Paragraph"],"scrap_line":1}"#)
                .unwrap();
        assert_eq!(tail.boundary(), Some((BoundaryEra::Tail, 1)));
        let none: BlockMap = serde_json::from_str(r#"{"kinds":["Paragraph"]}"#).unwrap();
        assert_eq!(none.boundary(), None);
        // Damaged file carrying both: the tail field wins (new builds only
        // ever write scrap_line; migrated saves null the legacy key).
        let both: BlockMap = serde_json::from_str(
            r#"{"kinds":["Paragraph","Paragraph","Paragraph"],"aside_boundary":1,"scrap_line":1}"#,
        )
        .unwrap();
        assert_eq!(both.boundary(), Some((BoundaryEra::Tail, 1)));
        // And a tail-era map round-trips through serde.
        let mut b = BlockMap::new(3);
        b.set_scrap_line(Some(1));
        let json = serde_json::to_string(&b).unwrap();
        let back: BlockMap = serde_json::from_str(&json).unwrap();
        assert_eq!(back.boundary(), Some((BoundaryEra::Tail, 1)));
    }

    #[test]
    fn scrap_line_shifts_across_block_splices_and_dissolves() {
        // 6 blocks; seam at 3: manuscript [0..3], seam [3], scraps [4,5].
        let mk = || {
            let mut b = BlockMap::new(6);
            b.set_scrap_line(Some(3));
            b
        };
        // Split in the manuscript → seam shifts down.
        let mut b = mk();
        b.on_edit(0, 0, 1);
        assert_eq!(b.scrap_line(), Some(4));
        // Split in the pile → unchanged.
        let mut b = mk();
        b.on_edit(4, 0, 1);
        assert_eq!(b.scrap_line(), Some(3));
        // Merge that engulfs the seam line → clamps onto the merge point
        // (the never-panic floor; the app guards keep normal edits away).
        let mut b = mk();
        b.on_edit(2, 2, 0);
        assert_eq!(b.scrap_line(), Some(2));
        // A merge that leaves no scrap block after the seam → dissolves.
        let mut b = mk();
        b.on_edit(4, 1, 0); // 5 blocks left, seam at 3, one scrap at 4
        assert_eq!(b.scrap_line(), Some(3));
        b.on_edit(3, 1, 0); // seam swallows the last scrap
        assert_eq!(b.scrap_line(), None);
    }

    /// The flip is membership-preserving (time-persistence 3/4): every block
    /// keeps its side, spans follow their text, the seam is never stamped.
    #[test]
    fn flip_state_moves_the_pile_below_and_remaps_spans() {
        // Top era: pile ["one ", "", "two "], separator, manuscript.
        let text = "one \n\ntwo \n\nthree four";
        let mut blocks = BlockMap::from_kinds(vec![
            BlockKind::Blockquote,
            BlockKind::Paragraph,
            BlockKind::Paragraph,
            BlockKind::Paragraph,
            BlockKind::Heading(1),
        ]);
        blocks.set_aside_boundary(Some(3));
        let mut spans = SpanSet::default();
        spans.add(0..3, InlineAttr::Strong); // "one" in the pile
        spans.add(12..17, InlineAttr::Emphasis); // "three" in the manuscript
        let (ntext, nspans, nblocks) = flip_state(text, &spans, &blocks);
        assert_eq!(ntext, "three four\n\none \n\ntwo ");
        assert_eq!(nblocks.boundary(), Some((BoundaryEra::Tail, 1)));
        assert_eq!(nblocks.kind(0), &BlockKind::Heading(1), "manuscript kind kept");
        assert_eq!(nblocks.kind(1), &BlockKind::Paragraph, "seam never stamped");
        assert_eq!(nblocks.kind(2), &BlockKind::Blockquote, "pile kind kept");
        assert!(nspans.covers(0..5, &InlineAttr::Emphasis), "manuscript span rebased");
        assert!(nspans.covers(12..15, &InlineAttr::Strong), "pile span rebased");
        // The regions partition the same text both ways.
        let rope = ropey::Rope::from_str(&ntext);
        assert_eq!(manuscript_range_of(&rope, &nblocks), 0..10);
        assert_eq!(scraps_range_of(&rope, &nblocks), Some(12..22));
    }

    /// Cross-era restore NORMALIZES (time-persistence 3): restoring a
    /// top-era state materializes tail-era, membership preserved.
    #[test]
    fn restore_of_a_top_era_state_normalizes_to_tail() {
        let mut doc = Document::new("live text", SpanSet::default(), BlockMap::default());
        let mut old = BlockMap::from_kinds(vec![BlockKind::Paragraph; 3]);
        old.set_aside_boundary(Some(1));
        doc.restore_state("pile line\n\nmanuscript body", SpanSet::default(), old);
        assert_eq!(doc.text(), "manuscript body\n\npile line");
        assert_eq!(doc.boundary().map(|(e, _)| e), Some(BoundaryEra::Tail));
        assert_eq!(doc.manuscript_slice().0, "manuscript body");
        // Undo returns to the pre-restore present.
        doc.undo();
        assert_eq!(doc.text(), "live text");
    }

    /// The length-mismatch fallback carries the boundary through the clamp
    /// (time-persistence 7) instead of silently dropping the seam.
    #[test]
    fn restore_length_mismatch_keeps_the_boundary_through_the_clamp() {
        let mut doc = Document::new("x", SpanSet::default(), BlockMap::default());
        // A blocks map one line short of its text (trailing-newline drift).
        let mut blocks = BlockMap::new(3);
        blocks.set_scrap_line(Some(1));
        doc.restore_state("manu\n\nscrap\n", SpanSet::default(), blocks);
        assert_eq!(
            doc.scrap_line(),
            Some(1),
            "a fixable boundary survives the fallback"
        );
    }

    /// Spanning deletion (seam-mechanics 2 + graveyard-interplay 6): one
    /// transaction, two edits, the seam between the remnants; each side
    /// files region-honest when it clears the threshold; one undo atom.
    #[test]
    fn spanning_delete_keeps_the_seam_and_files_per_side() {
        let mut doc = Document::new(
            "manuscript body here\n\nscrap one text\n\nscrap two",
            SpanSet::default(),
            BlockMap::default(),
        );
        let mut b = BlockMap::new(5);
        b.set_scrap_line(Some(1));
        doc.restore_state("manuscript body here\n\nscrap one text\n\nscrap two", SpanSet::default(), b);
        let text = doc.text();
        // Select from mid-manuscript through mid-pile: above = "body here",
        // below = "scrap one text\n\nscrap " (both sides over threshold 5).
        let a_start = text.find("body").unwrap();
        let a_end = 20; // manuscript end
        let b_start = 22; // pile start
        let b_end = text.find("two").unwrap();
        let caret = doc.delete_spanning_seam(
            a_start..a_end,
            b_start..b_end,
            "",
            true,
            5,
            "manuscript".into(),
            9,
        );
        assert_eq!(doc.text(), "manuscript \n\ntwo");
        assert_eq!(doc.scrap_line(), Some(1), "the seam stands between the remnants");
        assert_eq!(caret, a_start);
        // Two region-honest entries in the one atom.
        assert_eq!(doc.graveyard().len(), 2);
        let regions: Vec<GraveRegion> =
            doc.graveyard().entries().iter().map(|e| e.region).collect();
        assert_eq!(regions, vec![GraveRegion::Manuscript, GraveRegion::Scraps]);
        assert_eq!(doc.graveyard().entries()[0].text, "body here");
        assert_eq!(doc.graveyard().entries()[1].text, "scrap one text\n\nscrap ");
        // ONE undo reverses the whole thing — text, seam, and both filings.
        doc.undo();
        assert_eq!(doc.text(), "manuscript body here\n\nscrap one text\n\nscrap two");
        assert_eq!(doc.graveyard().len(), 0);
        assert_eq!(doc.scrap_line(), Some(1));
    }

    /// Type-over of a spanning selection: replacement lands manuscript-side,
    /// nothing files (the text was replaced, not destroyed).
    #[test]
    fn spanning_type_over_lands_the_replacement_manuscript_side() {
        let mut doc = Document::new("x", SpanSet::default(), BlockMap::default());
        let mut b = BlockMap::new(3);
        b.set_scrap_line(Some(1));
        doc.restore_state("manu tail\n\nscrap", SpanSet::default(), b);
        // Select "tail" through "scr" and type "X" over it.
        doc.delete_spanning_seam(5..9, 11..14, "X", false, 0, String::new(), 0);
        assert_eq!(doc.text(), "manu X\n\nap");
        assert_eq!(doc.scrap_line(), Some(1));
        assert!(doc.graveyard().is_empty(), "a type-over never files");
    }

    /// Evaporation (seam-mechanics 6): textless = empty. The standalone
    /// path (the retype-guard release) removes blank leftovers + boundary
    /// as ONE undoable step; a spanning delete that empties the pile
    /// evaporates inside its own atom.
    #[test]
    fn textless_pile_evaporates_and_undo_restores_it() {
        let mut doc = Document::new("x", SpanSet::default(), BlockMap::default());
        let mut b = BlockMap::new(3);
        b.set_scrap_line(Some(1));
        doc.restore_state("manu\n\nscrap", SpanSet::default(), b);
        // Delete the scrap's text: blank blocks remain, the seam stands.
        let s = doc.text().find("scrap").unwrap();
        doc.edit_bytes(s..s + 5, "");
        assert_eq!(doc.scrap_line(), Some(1), "structurally intact");
        assert!(doc.scraps_textless());
        // The guard's release (the editor calls this when the caret leaves).
        assert!(doc.evaporate_scraps());
        assert_eq!(doc.text(), "manu");
        assert_eq!(doc.boundary(), None);
        // Undo restores the blank leftovers + the seam together.
        doc.undo();
        assert_eq!(doc.scrap_line(), Some(1));
        // And the earlier text deletion is still its own step.
        doc.undo();
        assert_eq!(doc.text(), "manu\n\nscrap");
    }

    /// Exile of the last scrap collapses the boundary INSIDE the same
    /// transaction (graveyard-interplay 4): undo restores text + seam
    /// together; the entry is region-honest.
    #[test]
    fn exiling_the_last_scrap_collapses_the_seam_in_the_same_atom() {
        let mut doc = Document::new("x", SpanSet::default(), BlockMap::default());
        let mut b = BlockMap::new(3);
        b.set_scrap_line(Some(1));
        doc.restore_state("manu\n\nscrap", SpanSet::default(), b);
        let s = doc.text().find("scrap").unwrap();
        let id = doc.cut_to_graveyard(s..s + 5, String::new(), 3, true);
        assert_eq!(doc.text(), "manu");
        assert_eq!(doc.boundary(), None, "the emptied seam collapsed in the atom");
        assert_eq!(doc.graveyard().get(id).unwrap().region, GraveRegion::Scraps);
        doc.undo();
        assert_eq!(doc.text(), "manu\n\nscrap");
        assert_eq!(doc.scrap_line(), Some(1), "undo restores text + seam together");
        assert!(doc.graveyard().is_empty());
    }

    /// Put back is region-preserving (graveyard-interplay 1/3): a scrap-
    /// origin entry returns to the pile; after the pile evaporated it
    /// RE-BIRTHS the seam and lands as the sole scrap.
    #[test]
    fn put_back_returns_to_the_pile_and_rebirths_an_evaporated_seam() {
        let mut doc = Document::new("x", SpanSet::default(), BlockMap::default());
        let mut b = BlockMap::new(5);
        b.set_scrap_line(Some(1));
        doc.restore_state("manu\n\nkeep\n\nscrap", SpanSet::default(), b);
        let s = doc.text().find("scrap").unwrap();
        let id = doc.cut_to_graveyard(s..s + 5, String::new(), 3, true);
        assert_eq!(doc.scrap_line(), Some(1), "\"keep\" still holds the pile open");
        // Put back lands in the PILE, never the manuscript.
        doc.put_back(id).unwrap();
        assert!(doc.manuscript_slice().0.trim_end().ends_with("manu"), "{}", doc.text());
        assert!(doc.text().contains("scrap"));
        let pile = doc.scraps_char_range().unwrap();
        let pos = doc.text().find("scrap").unwrap();
        assert!(doc.rope().byte_to_char(pos) >= pile.start, "returned below the seam");

        // Now the re-birth: exile everything, seam evaporates, put back.
        let mut doc = Document::new("x", SpanSet::default(), BlockMap::default());
        let mut b = BlockMap::new(3);
        b.set_scrap_line(Some(1));
        doc.restore_state("manu\n\nsole scrap", SpanSet::default(), b);
        let s = doc.text().find("sole").unwrap();
        let id = doc.cut_to_graveyard(s..doc.len_bytes(), String::new(), 3, true);
        assert_eq!(doc.boundary(), None);
        let caret = doc.put_back(id).unwrap();
        assert_eq!(doc.text(), "manu\n\nsole scrap");
        assert_eq!(doc.scrap_line(), Some(1), "the seam re-birthed");
        assert_eq!(caret, doc.rope().len_chars());
        // Undo removes text + seam and restores the entry — one atom.
        doc.undo();
        assert_eq!(doc.text(), "manu");
        assert_eq!(doc.boundary(), None);
        assert_eq!(doc.graveyard().len(), 1);
    }

    /// A manuscript-origin corpse whose position drifted below the seam
    /// still returns to the MANUSCRIPT (the region-preserving clamp, both
    /// directions).
    #[test]
    fn put_back_clamps_a_manuscript_entry_out_of_the_pile() {
        let mut doc = Document::new("x", SpanSet::default(), BlockMap::default());
        let mut b = BlockMap::new(3);
        b.set_scrap_line(Some(1));
        doc.restore_state("manu\n\nscrap", SpanSet::default(), b);
        let mut g = Graveyard::default();
        let id = g.file(
            "XX".into(),
            String::new(),
            doc.rope().len_chars(), // drifted to the doc tail (inside the pile)
            0,
            SpanSet::default(),
            Vec::new(),
            GraveRegion::Manuscript,
            false,
            false,
        );
        doc.set_graveyard(g);
        doc.put_back(id).unwrap();
        assert_eq!(doc.manuscript_slice().0, "manuXX", "landed at the manuscript end");
        assert!(doc.text().ends_with("scrap"), "the pile untouched");
    }

    /// Provenance records die with their text and follow it while it lives
    /// (seam-mechanics 7): no merge/split rules needed.
    #[test]
    fn provenance_records_follow_their_text_and_die_with_it() {
        let mut p = Provenance::default();
        p.add(10..20, "from here".into(), 0);
        p.add(25..30, "elsewhere".into(), 0);
        // An insertion before both shifts both.
        p.apply_op(&op(0, 0, "abc"));
        assert_eq!(p.records()[0].range, 13..23);
        assert_eq!(p.records()[1].range, 28..33);
        // The record containing a caret position is the one-liner shown.
        assert_eq!(p.at(15).unwrap().origin_quote, "from here");
        assert!(p.at(24).is_none());
        // Deleting a fragment's whole text kills ITS record only.
        p.apply_op(&op(13, 10, ""));
        assert_eq!(p.records().len(), 1);
        assert_eq!(p.records()[0].origin_quote, "elsewhere");
    }

    /// The migration transaction (time-persistence 4): text flips, side
    /// records remap arithmetically, the undo stacks drop, the journal
    /// carries one Seam event and no wholesale run.
    #[test]
    fn migration_flips_a_top_era_doc_and_drops_the_stacks() {
        let text = "old one\n\nold two\n\nthe piece begins\nand continues";
        let mut blocks = BlockMap::from_kinds(vec![BlockKind::Paragraph; 6]);
        blocks.set_aside_boundary(Some(3));
        let mut spans = SpanSet::default();
        spans.add(0..3, InlineAttr::Strong); // "old" in the pile
        let mut doc = Document::new(text, spans, blocks);
        // A note in the pile and one in the manuscript; a graveyard entry
        // whose origin sits in the pile (drifted there via apply_op).
        let pile_note = doc.add_note(4..7, "pile note".into(), 0); // "one"
        let manu_note = doc.add_note(22..27, "manu note".into(), 0); // "piece"
        let mut g = Graveyard::default();
        let gid = g.file("corpse".into(), String::new(), 22, 0, SpanSet::default(), Vec::new(), GraveRegion::Manuscript, false, false);
        doc.set_graveyard(g);
        // Give it an undo stack that could reach back across the flip.
        doc.edit_bytes(0..0, "Z");
        doc.undo();

        assert!(doc.migrate_top_to_tail());
        assert_eq!(doc.text(), "the piece begins\nand continues\n\nold one\n\nold two");
        assert_eq!(doc.boundary().map(|(e, _)| e), Some(BoundaryEra::Tail));
        assert_eq!(doc.manuscript_slice().0, "the piece begins\nand continues");
        // Side records were remapped arithmetically — never clamped to 0.
        let n = doc.notes().get(pile_note).unwrap();
        assert_eq!(
            &doc.text()[doc.char_to_byte(n.range.start)..doc.char_to_byte(n.range.end)],
            "one",
            "the in-pile note followed its text"
        );
        let n = doc.notes().get(manu_note).unwrap();
        assert_eq!(
            &doc.text()[doc.char_to_byte(n.range.start)..doc.char_to_byte(n.range.end)],
            "piece"
        );
        let e = doc.graveyard().get(gid).unwrap();
        assert_eq!(e.origin_pos, 4, "manuscript origin rebased by the flip");
        // The pile span followed its text.
        let pile_start = doc.scraps_char_range().unwrap().start;
        assert!(doc.spans().covers(pile_start..pile_start + 3, &InlineAttr::Strong));
        // The stacks are gone: ctrl-Z cannot reinstate top geometry.
        assert!(doc.undo().is_none(), "migration drops the undo stack");
        // The journal recorded the seam, and no wholesale run.
        assert_eq!(doc.journal().seams().count(), 1);
        assert!(
            doc.journal().runs.iter().all(|r| r.ins.len() < 20),
            "the move was journal-paused, not a document-sized run"
        );
        // Idempotence: a second call is a no-op.
        assert!(!doc.migrate_top_to_tail());
    }

    /// Graveyard asset refs join the GC reachable set (graveyard-interplay
    /// 9), both live and through persisted history states.
    #[test]
    fn graveyard_assets_are_gc_reachable_live_and_through_history() {
        let mut doc = Document::new("before\nimage-here\nafter, with enough text", SpanSet::default(), {
            let mut b = BlockMap::new(3);
            b.set_kind(
                1,
                BlockKind::Image { src: "asset:img1".into(), alt: String::new() },
            );
            b
        });
        // Cut the image block: its only reference is now the grave entry.
        let start = doc.text().find("image-here").unwrap();
        doc.cut_to_graveyard(start..start + 11, String::new(), 0, false);
        assert_eq!(doc.graveyard().asset_refs().collect::<Vec<_>>(), vec!["asset:img1"]);
        // And the persisted history's graveyard elements carry it too.
        let hist = doc.export_history(10);
        assert!(
            hist.asset_refs().any(|a| a == "asset:img1"),
            "history reaches assets via its Graveyard elements"
        );
    }

    /// The retrieval verb's model half: a scrap moves home carrying its
    /// notes and formatting, its provenance record dies with its departure,
    /// the separator slot closes, and emptying the pile dissolves the seam —
    /// all one atom (seam-mechanics 4; 08 §2 "Retrieve").
    #[test]
    fn move_to_manuscript_carries_notes_home_and_evaporates_an_emptied_pile() {
        let mut doc = Document::new("the story so far", SpanSet::default(), BlockMap::default());
        // Park "story " with a note on it.
        let note = doc.add_note(4..9, "keep?".into(), 0); // "story"
        doc.set_aside(4..10, "the".into(), 7, false).unwrap();
        assert_eq!(doc.text(), "the so far\n\nstory ");
        assert_eq!(doc.provenance().records().len(), 1);
        let n = doc.notes().get(note).unwrap();
        let pile = doc.scraps_char_range().unwrap();
        assert!(n.range.start >= pile.start, "the note parked with its text");
        // Move it home, to char 4 (after "the ").
        let scrap_start = doc.char_to_byte(pile.start);
        let scrap_end = doc.len_bytes();
        let landed = doc.move_to_manuscript(scrap_start..scrap_end, 4).unwrap();
        assert_eq!(doc.text(), "the story so far");
        assert_eq!(landed, 4..10, "arrives at the writing position, range for the selection");
        assert_eq!(doc.boundary(), None, "the emptied pile dissolved in the atom");
        let n = doc.notes().get(note).unwrap();
        assert_eq!(
            &doc.text()[doc.char_to_byte(n.range.start)..doc.char_to_byte(n.range.end)],
            "story",
            "the note travelled home"
        );
        assert!(doc.provenance().is_empty(), "the record died with the departure");
        // One undo returns the scrap to the pile, seam and record included.
        doc.undo();
        assert_eq!(doc.text(), "the so far\n\nstory ");
        assert_eq!(doc.scrap_line(), Some(1));
        assert_eq!(doc.provenance().records().len(), 1);
    }

    /// ReplayDoc applies Seam events interleaved by timestamp
    /// (time-persistence 2): a reconstruction carries the scrubbed moment's
    /// own seam, and the fallback BlockMap keeps the boundary.
    #[test]
    fn replay_applies_seam_events_by_timestamp() {
        use crate::journal::{EditRun, Journal, JournalEvent, ReplayDoc};
        let mut j = Journal::default();
        j.runs.push(EditRun { t0: 1_000, t1: 1_000, pos: 4, del_chars: 0,
            del_words: None, ins: "\n\nscrap".into() });
        j.events.push(JournalEvent::Seam { t: 1_000, at: Some(1) });
        j.events.push(JournalEvent::Seam { t: 5_000, at: None });
        let mut r = ReplayDoc::new("manu", SpanSet::default(), BlockMap::new(1), 0);
        // Advance to t=2s: the park's run applied, then its seam event.
        assert!(r.advance(&j, 2_000));
        assert_eq!(r.text(), "manu\n\nscrap");
        assert_eq!(r.blocks.scrap_line(), Some(1), "the scrubbed moment has its seam");
        // Advance past the evaporation.
        assert!(r.advance(&j, 6_000));
        assert_eq!(r.blocks.scrap_line(), None);
        // The length-mismatch fallback carries the boundary through a clamp.
        let mut b = BlockMap::new(9);
        b.set_scrap_line(Some(3));
        let r = ReplayDoc::new("a\n\nb\n\nc", SpanSet::default(), b, 0);
        assert_eq!(r.blocks.scrap_line(), Some(3), "fallback keeps a fixable seam");
    }
}
