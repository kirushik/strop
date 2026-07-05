//! Rich-text schema types and the span/anchor machinery.
//!
//! See docs/document-model.md. Spans are char-indexed ranges over the same
//! text stream the rope and Loro share; `SpanSet::apply_op` keeps them
//! consistent across every edit (including undo/redo, which arrive as
//! ordinary ops). The same adjustment math will anchor annotations.

use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::buffer::{Buffer, TextOp, Transaction};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Peritext expansion: typing at the right edge continues the style.
    /// Code and links must not grow under the caret.
    pub fn expands(&self) -> bool {
        !matches!(self, Self::Code | Self::Link(_) | Self::FootnoteRef(_))
    }
}

/// Per-block kind; lives beside the text, keyed by block index.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
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
        caption: String,
    },
    FootnoteDef {
        id: String,
    },
}

/// Block kinds aligned with the text's newline-separated blocks.
/// Invariant: `kinds.len() == rope.len_lines()`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockMap {
    kinds: Vec<BlockKind>,
    /// The out-of-band asides boundary (docs/impl/02-asides.md §1, review
    /// B13/H42). Blocks `0..b` are the compost rail, block `b` is the plain
    /// empty separator paragraph, blocks `b+1..` are the manuscript. `None`
    /// means no rail exists — an empty rail is simply absent (asides.md §1).
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
}

impl Default for BlockMap {
    fn default() -> Self {
        Self {
            kinds: vec![BlockKind::default()],
            aside_boundary: None,
        }
    }
}

impl BlockMap {
    pub fn new(blocks: usize) -> Self {
        Self {
            kinds: vec![BlockKind::default(); blocks.max(1)],
            aside_boundary: None,
        }
    }

    pub fn from_kinds(kinds: Vec<BlockKind>) -> Self {
        if kinds.is_empty() {
            Self::default()
        } else {
            Self {
                kinds,
                aside_boundary: None,
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

    /// The asides boundary index (see the field). Blocks `0..b` are compost,
    /// `b` the separator line, `b+1..` the manuscript.
    pub fn aside_boundary(&self) -> Option<usize> {
        self.aside_boundary
    }

    /// Install a boundary, clamped to a real interior line: a valid boundary
    /// needs at least one compost block before it (`b >= 1`) and must land
    /// strictly inside the block range. Anything else means "no rail" (`None`),
    /// so a corrupted or stale index degrades to the empty-rail state rather
    /// than panicking a slice.
    pub fn set_aside_boundary(&mut self, boundary: Option<usize>) {
        self.aside_boundary = match boundary {
            Some(b) if b >= 1 && b < self.kinds.len() => Some(b),
            _ => None,
        };
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
    /// `splits` how many it inserted. Merges keep the first block's kind;
    /// splits inherit it (Enter-at-heading-end is special-cased upstream).
    pub fn on_edit(&mut self, block: usize, merged: usize, splits: usize) {
        let block = block.min(self.kinds.len().saturating_sub(1));
        let drain_end = (block + 1 + merged).min(self.kinds.len());
        let removed = drain_end - (block + 1); // blocks actually spliced out
        self.kinds.drain(block + 1..drain_end);
        let kind = self.kinds[block].clone();
        for _ in 0..splits {
            self.kinds.insert(block + 1, kind.clone());
        }
        self.adjust_boundary(block, removed, splits);
    }

    /// Shift the out-of-band aside boundary across the block splice `on_edit`
    /// just performed, so it keeps pointing at the same separator line. This
    /// is the ONLY thing that keeps the index aligned (it is not a kind, so no
    /// splice path moves it "for free" — review B13/H42). `block` is the
    /// edit's first block, `removed`/`splits` the blocks spliced out and in
    /// after it. Never panics: the result is clamped into the post-splice
    /// range, and a boundary that collapses to 0 (no compost blocks left)
    /// becomes `None` — the empty rail the design says simply does not exist.
    fn adjust_boundary(&mut self, block: usize, removed: usize, splits: usize) {
        let Some(b) = self.aside_boundary else { return };
        let new_b = if b <= block {
            // At or before the edit's first block: an edit starting at `block`
            // only touches lines strictly after it, so an earlier boundary is
            // untouched.
            b
        } else if b < block + 1 + removed {
            // The boundary line itself sat inside the spliced-out span — it
            // merged into `block`. Clamp onto the merge point (the app guards
            // keep normal edits from ever reaching here; this is the
            // never-panic floor, not a routine path).
            block
        } else {
            // Strictly after the spliced-out span: shift by the net line delta.
            b - removed + splits
        };
        let last = self.kinds.len().saturating_sub(1);
        self.aside_boundary = match new_b.min(last) {
            0 => None,
            n => Some(n),
        };
    }
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

    /// Keep all spans consistent across a text edit: delete `op.delete`
    /// chars at `op.pos`, then insert `op.insert` there.
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
        for span in &mut self.spans {
            if op.delete > 0 {
                span.range.start = clamp(span.range.start);
                span.range.end = clamp(span.range.end);
            }
            if ins > 0 {
                // Typing at the left edge stays outside (start shifts);
                // strictly inside grows the span; at the right edge only
                // expanding styles absorb the insertion.
                if span.range.start >= op.pos {
                    span.range.start += ins;
                }
                if span.range.end > op.pos
                    || (span.range.end == op.pos && span.attr.expands())
                {
                    span.range.end += ins;
                }
            }
        }
        self.spans.retain(|s| s.range.start < s.range.end);
        self.spans.sort_by_key(|s| s.range.start);
    }
}

/// Margin annotation status; Done/Dismissed leave the margin but persist
/// (the engine must not re-raise a dismissed diagnosis on the same span).
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

impl Annotations {
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
        let clamp = |x: usize| {
            if x >= del_end {
                x - op.delete
            } else if x > op.pos {
                op.pos
            } else {
                x
            }
        };
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
            if op.delete > 0 {
                n.range.start = clamp(n.range.start);
                n.range.end = clamp(n.range.end);
            }
            if ins > 0 {
                // Non-expanding (ExpandType::None semantics).
                if n.range.start >= op.pos {
                    n.range.start += ins;
                }
                if n.range.end > op.pos {
                    n.range.end += ins;
                }
                // A zero-width anchor sitting exactly at the insertion point
                // would advance its start (>=) but not its end (>), inverting
                // the range. Keep the boundaries ordered. (Caught by the
                // notes property test.)
                if n.range.end < n.range.start {
                    n.range.end = n.range.start;
                }
            }
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
    pub fn file(
        &mut self,
        text: String,
        origin_quote: String,
        origin_pos: usize,
        cut_unix: i64,
        spans: SpanSet,
        kinds: Vec<BlockKind>,
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
        });
        id
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

/// Text + formatting + block structure with unified, transaction-aligned
/// undo. The buffer owns text history; span/block states are snapshotted
/// per transaction (they're small — snapshots beat op inversion).
#[derive(Debug, Default)]
pub struct Document {
    buffer: Buffer,
    spans: SpanSet,
    blocks: BlockMap,
    notes: Annotations,
    /// The graveyard record (docs/impl/02-asides.md §4). Lives here beside the
    /// notes so it rides the SAME undo snapshot (undo of a cut removes its
    /// entry) and the SAME op-absorption path (`origin_pos` shifts like a note
    /// anchor). See `GraveEntry`.
    graveyard: Graveyard,
    undo_states: Vec<(SpanSet, BlockMap, Annotations, Graveyard)>,
    redo_states: Vec<(SpanSet, BlockMap, Annotations, Graveyard)>,
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
    pub fn new(text: &str, spans: SpanSet, blocks: BlockMap) -> Self {
        let buffer = Buffer::new(text);
        let mut blocks = blocks;
        // Repair a stale/foreign block map against the actual text.
        let lines = buffer.rope().len_lines();
        if blocks.len() != lines {
            blocks = BlockMap::new(lines);
        }
        Self {
            buffer,
            spans,
            blocks,
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
        let ops = self.buffer.take_ops();
        let now = crate::journal::now_ms();
        for op in &ops {
            self.spans.apply_op(op);
            self.notes.apply_op(op);
            self.graveyard.apply_op(op);
            self.journal.record(op, now);
        }
        self.pending_ops.extend(ops);
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

    fn snapshot(&self) -> (SpanSet, BlockMap, Annotations, Graveyard) {
        (
            self.spans.clone(),
            self.blocks.clone(),
            self.notes.clone(),
            self.graveyard.clone(),
        )
    }

    pub fn notes(&self) -> &Annotations {
        &self.notes
    }

    pub fn set_notes(&mut self, notes: Annotations) {
        self.revision += 1;
        self.notes = notes;
    }

    pub fn graveyard(&self) -> &Graveyard {
        &self.graveyard
    }

    /// Install the persisted graveyard at load (like `set_notes`).
    pub fn set_graveyard(&mut self, graveyard: Graveyard) {
        self.revision += 1;
        self.graveyard = graveyard;
    }

    /// The out-of-band asides boundary (see `BlockMap::aside_boundary`).
    pub fn aside_boundary(&self) -> Option<usize> {
        self.blocks.aside_boundary()
    }

    /// Set the boundary as its own undoable transaction (the aside verb births
    /// or dissolves the rail; ctrl-z reverses it). The index is clamped by
    /// `BlockMap::set_aside_boundary`.
    pub fn set_aside_boundary(&mut self, boundary: Option<usize>) {
        self.revision += 1;
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        self.blocks.set_aside_boundary(boundary);
    }

    /// First manuscript char: the start of the line after the boundary, or 0
    /// when there is no rail. Everything manuscript-scoped (word counts, export,
    /// AI passes) rebases against this (recon TRAP 14; review H40).
    pub fn manuscript_base_char(&self) -> usize {
        match self.blocks.aside_boundary() {
            Some(b) => {
                let rope = self.buffer.rope();
                let line = (b + 1).min(rope.len_lines());
                rope.line_to_char(line)
            }
            None => 0,
        }
    }

    /// The manuscript as a char range (compost excluded). `None` boundary →
    /// the whole document. Used to word-count and slice without cloning.
    pub fn manuscript_char_range(&self) -> Range<usize> {
        self.manuscript_base_char()..self.buffer.rope().len_chars()
    }

    /// The manuscript region as a standalone `(text, spans, blocks)` triple with
    /// char offsets REBASED to 0 — directly usable by `to_markdown`, word
    /// counting, and the AI pass. Rebasing here (not leaving offsets relative to
    /// the full doc) is what keeps a card or an exported span from ever landing
    /// in the compost (review H40, TRAP 4). Add `manuscript_base_char()` back to
    /// any range that must return to full-document coordinates.
    pub fn manuscript_slice(&self) -> (String, SpanSet, BlockMap) {
        let base = self.manuscript_base_char();
        if base == 0 {
            return (self.text(), self.spans.clone(), self.blocks.clone());
        }
        let rope = self.buffer.rope();
        let text = rope.slice(base..).to_string();
        let mut spans = SpanSet::default();
        for s in self.spans.spans() {
            if s.range.end <= base {
                continue; // entirely in compost
            }
            let start = s.range.start.saturating_sub(base);
            let end = s.range.end - base;
            if end > start {
                spans.add(start..end, s.attr.clone());
            }
        }
        // The manuscript's own block kinds: everything after the separator line.
        let boundary = self.blocks.aside_boundary().unwrap_or(0);
        let first = (boundary + 1).min(self.blocks.len());
        let blocks = BlockMap::from_kinds(self.blocks.kinds()[first..].to_vec());
        (text, spans, blocks)
    }

    /// Delete `byte_range` and file the removed prose in the graveyard as ONE
    /// undoable transaction. Both the auto-cut trigger and the explicit "Send
    /// to the graveyard" verb route here. Because `edit_bytes` snapshots the
    /// PRE-cut side-state (graveyard included) before the filing, undoing the
    /// deletion restores a graveyard WITHOUT this entry — P13's inverse in the
    /// same grammar, no correlation table needed. `origin_pos` is the cut point
    /// (where Put back returns the text). Returns the new entry id.
    pub fn cut_to_graveyard(
        &mut self,
        byte_range: Range<usize>,
        origin_quote: String,
        cut_unix: i64,
    ) -> u64 {
        let rope = self.buffer.rope();
        let start_char = rope.byte_to_char(byte_range.start);
        let end_char = rope.byte_to_char(byte_range.end);
        let text: String = rope.slice(start_char..end_char).to_string();
        // Capture the cut's formatting and structure BEFORE the delete shifts
        // them away, so Put back is lossless (Bug D / P1). One block kind per
        // line the text will re-create (`count_line_breaks + 1`), so put_back
        // can re-stamp them onto exactly the re-inserted blocks.
        let spans = self.spans.slice(start_char..end_char);
        let first_line = rope.char_to_line(start_char);
        let n_blocks = count_line_breaks(&text) + 1;
        let kinds: Vec<BlockKind> = self
            .blocks
            .kinds()
            .iter()
            .skip(first_line)
            .take(n_blocks)
            .cloned()
            .collect();
        // The non-coalescing edit always opens a fresh transaction, so the
        // pre-cut snapshot is always taken and the filing below rides it.
        self.edit_bytes(byte_range, "");
        self.revision += 1;
        self.graveyard
            .file(text, origin_quote, start_char, cut_unix, spans, kinds)
    }

    /// Put an entry back into the manuscript at its re-anchored origin, as one
    /// undoable transaction: re-insert the text, then drop the entry (undo of
    /// the insertion restores the entry via the pre-insert snapshot). The target
    /// is CLAMPED into the manuscript region (review #62) so cut prose can never
    /// resurrect into the compost. Returns the caret char offset after the
    /// re-inserted text (for the paragraph flash), or `None` if the entry is gone.
    pub fn put_back(&mut self, id: u64) -> Option<usize> {
        let entry = self.graveyard.get(id)?.clone();
        let len = self.buffer.rope().len_chars();
        let base = self.manuscript_base_char();
        let at_char = entry.origin_pos.clamp(base, len);
        let at_byte = self.buffer.char_to_byte(at_char);
        self.edit_bytes(at_byte..at_byte, &entry.text);
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

    /// Insert `moved` as one or more compost items at the rail tail, within the
    /// CURRENTLY OPEN transaction (the caller has pushed the snapshot). Births
    /// the boundary when absent (at char 0 — the folk "notes at the top"). When
    /// the compost is non-empty a separator blank line precedes the new item
    /// (review H23) so parked thoughts never fuse into one. `quote_first_line`
    /// makes the first inserted block a `Blockquote` (the orphan-note anchor
    /// fragment carries the margin-note anchor typography, asides.md §2.3).
    /// Kinds of the whole inserted span are reset to Paragraph first, because
    /// `on_edit` clones the split block's kind and the payload must not inherit
    /// a neighbouring Heading/quote. Returns the payload's char length so a
    /// mover can compute the restored caret.
    fn insert_into_compost(&mut self, moved: &str, quote_first_line: bool) -> usize {
        let payload = format!("{moved}\n\n");
        let payload_len = payload.chars().count();
        let added = count_line_breaks(&payload); // breaks(moved) + 2
        let (at_char, new_boundary, first_block) = match self.blocks.aside_boundary() {
            None => (0usize, count_line_breaks(moved) + 1, 0usize),
            Some(b) => (self.manuscript_base_char(), b + added, b + 1),
        };
        let at_byte = self.buffer.char_to_byte(at_char);
        let (block, merged) = self.pre_edit_info(&(at_byte..at_byte));
        self.buffer.edit_bytes_grouped(at_byte..at_byte, &payload);
        self.blocks.on_edit(block, merged, count_line_breaks(&payload));
        // The whole inserted span (items + the new separator) is writer prose:
        // reset it to Paragraph, overriding on_edit's kind inheritance.
        for i in first_block..=new_boundary.min(self.blocks.len().saturating_sub(1)) {
            self.blocks.set_kind(i, BlockKind::Paragraph);
        }
        if quote_first_line {
            self.blocks.set_kind(first_block, BlockKind::Blockquote);
        }
        self.blocks.set_aside_boundary(Some(new_boundary));
        payload_len
    }

    /// Move `byte_range` (a manuscript selection or the caret's paragraph) into
    /// the compost rail — the aside verb (docs/impl/02-asides.md §2). A MOVE,
    /// never a cut: the graveyard is not touched (a writer-initiated move to a
    /// writer-owned region is exempt by construction — the suppression guard of
    /// review H41; nothing here files a corpse). One undoable transaction (the
    /// grouped delete + insert). Returns the caret char offset to restore at the
    /// collapse point (the writer parked a thought, she did not travel), or
    /// `None` if the range is empty or lands in the compost already.
    pub fn set_aside(&mut self, byte_range: Range<usize>) -> Option<usize> {
        let rope = self.buffer.rope();
        let s = rope.byte_to_char(byte_range.start);
        let e = rope.byte_to_char(byte_range.end);
        if s >= e || s < self.manuscript_base_char() {
            return None; // nothing to move, or it is compost already
        }
        let moved: String = rope.slice(s..e).to_string();
        self.revision += 1;
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        // Delete the prose (grouped so the following insert shares the step).
        let (block, merged) = self.pre_edit_info(&byte_range);
        self.buffer.edit_bytes_grouped(byte_range, "");
        self.blocks.on_edit(block, merged, 0);
        let payload_len = self.insert_into_compost(&moved, false);
        self.absorb_buffer_ops();
        Some(s + payload_len)
    }

    /// Land an orphaned WRITER note's text at the compost tail (asides.md §2.3;
    /// spec §3): a quoted anchor line (`Blockquote`) plus the body paragraph.
    /// The note is removed from the margin in the SAME undoable step. A move,
    /// not a cut (no graveyard). The caller resolves `CardFocus` first if the
    /// note is active (review B5). Diagnoses never migrate.
    pub fn migrate_note_to_compost(&mut self, note_id: u64, anchor_fragment: &str) -> bool {
        let Some(note) = self.notes.get(note_id) else {
            return false;
        };
        if note.kind != NoteKind::Note {
            return false; // machine cards are not writer material
        }
        let body = note.body.clone();
        // Anchor fragment on its own line, then the body — flattened so a
        // multi-line body stays one item (the rail's item grammar is blank-line
        // separated; internal newlines would split it, so collapse them).
        let anchor = anchor_fragment.replace('\n', " ");
        let body = body.replace('\n', " ");
        let moved = format!("{anchor}\n{body}");
        self.revision += 1;
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        self.insert_into_compost(&moved, true);
        self.absorb_buffer_ops();
        self.notes.remove(note_id);
        true
    }

    /// Add an author note as its own undoable transaction.
    pub fn add_note(&mut self, range: Range<usize>, body: String, created_unix: i64) -> u64 {
        self.revision += 1;
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        self.notes.add(range, body, created_unix)
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
            self.notes.push(d);
        }
    }

    pub fn edit_bytes(&mut self, byte_range: Range<usize>, text: &str) {
        self.revision += 1;
        let (block, merged) = self.pre_edit_info(&byte_range);
        if self.buffer.edit_bytes(byte_range, text) {
            // The buffer edit mutates only the rope + its own undo stack;
            // spans/blocks/notes stay pre-edit until on_edit/absorb_buffer_ops
            // run below, so snapshotting here captures the same pre-edit
            // side-state as before — but only when a transaction opens.
            self.undo_states.push(self.snapshot());
            self.redo_states.clear();
        }
        self.blocks
            .on_edit(block, merged, count_line_breaks(text));
        self.absorb_buffer_ops();
    }

    pub fn edit_bytes_coalescing(&mut self, byte_range: Range<usize>, text: &str) {
        self.revision += 1;
        let (block, merged) = self.pre_edit_info(&byte_range);
        if self.buffer.edit_bytes_coalescing(byte_range, text) {
            // Snapshot only when a new transaction actually opens. While
            // typing inside a word the buffer coalesces and returns false, so
            // the full SpanSet+BlockMap+Annotations clone is skipped on the
            // ~5-of-6 mid-word keystrokes it used to be allocated and dropped
            // on. Pre-edit side-state is intact here (see edit_bytes).
            self.undo_states.push(self.snapshot());
            self.redo_states.clear();
        }
        self.blocks
            .on_edit(block, merged, count_line_breaks(text));
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
        let cursor = self.buffer.undo()?;
        self.revision += 1;
        if let Some((spans, blocks, notes, graveyard)) = self.undo_states.pop() {
            self.redo_states.push((
                std::mem::replace(&mut self.spans, spans),
                std::mem::replace(&mut self.blocks, blocks),
                std::mem::replace(&mut self.notes, notes),
                std::mem::replace(&mut self.graveyard, graveyard),
            ));
        }
        // Buffer inverse ops still mirror to the store, but must NOT be
        // re-applied to spans/blocks (the snapshot is the correct state).
        // They DO journal — an undo is an honest edit and the envelope
        // visibly steps back.
        let ops = self.buffer.take_ops();
        let now = crate::journal::now_ms();
        for op in &ops {
            self.journal.record(op, now);
        }
        self.pending_ops.extend(ops);
        Some(cursor)
    }

    pub fn redo(&mut self) -> Option<Option<usize>> {
        let cursor = self.buffer.redo()?;
        self.revision += 1;
        if let Some((spans, blocks, notes, graveyard)) = self.redo_states.pop() {
            self.undo_states.push((
                std::mem::replace(&mut self.spans, spans),
                std::mem::replace(&mut self.blocks, blocks),
                std::mem::replace(&mut self.notes, notes),
                std::mem::replace(&mut self.graveyard, graveyard),
            ));
        }
        let ops = self.buffer.take_ops();
        let now = crate::journal::now_ms();
        for op in &ops {
            self.journal.record(op, now);
        }
        self.pending_ops.extend(ops);
        Some(cursor)
    }

    /// Replace the whole document state as ONE undoable transaction —
    /// checkpoint restore semantics: rewinding is a forward edit, history
    /// stays append-only, and ctrl-z takes you back to the present.
    pub fn restore_state(&mut self, text: &str, spans: SpanSet, blocks: BlockMap) {
        self.revision += 1;
        let snapshot = self.snapshot();
        // Re-anchor notes by content (the least-surprising restore semantics):
        // each live note follows the passage it covers into the restored text;
        // a note whose passage is gone detaches honestly instead of piling at
        // the document end. Computed against the OLD buffer text — captured
        // here, before the wholesale swap erases it — then installed after.
        let old_text = self.buffer.text();
        let mut reanchored = self.notes.clone();
        reanchored.reanchor(&old_text, text);
        // The graveyard is a record of cuts, not tied to any live passage, so
        // it simply survives the swap. Preserve the pre-swap entries here — the
        // wholesale op below would otherwise pin every origin_pos to the tail —
        // and re-clamp them to the new length after (Put back re-clamps into
        // the manuscript region anyway).
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
        self.spans = spans;
        let lines = self.buffer.rope().len_lines();
        self.blocks = if blocks.len() == lines {
            blocks
        } else {
            BlockMap::new(lines)
        };
        self.notes = reanchored;
        // Overwrite the origin_pos-mangled entries with the preserved ones,
        // clamped into the restored length.
        self.graveyard = saved_graveyard;
        self.graveyard.clamp(self.buffer.rope().len_chars());
    }

    /// Export undo/redo state for persistence (most-recent `cap` entries).
    /// Saved atomically with the text it refers to, so it restores exactly.
    pub fn export_history(&self, cap: usize) -> History {
        let (undo, redo) = self.buffer.export_history(cap);
        let tail = |v: &Vec<(SpanSet, BlockMap, Annotations, Graveyard)>| {
            v[v.len().saturating_sub(cap)..].to_vec()
        };
        History {
            undo,
            redo,
            undo_states: tail(&self.undo_states),
            redo_states: tail(&self.redo_states),
        }
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
}

/// Persisted cross-session undo/redo: the transaction stacks plus their
/// aligned span/block snapshots (one lifecycle for typing and formatting —
/// ctrl-z after reopen behaves exactly like before close).
///
/// The state tuple carries the `Graveyard` alongside spans/blocks/notes so a
/// cross-session undo of a cut also removes its entry. A pre-P2 file's history
/// is a 3-tuple: it fails this struct's serde arity, so `Store::open` drops it
/// (the length-mismatch guard in `import_history` never even runs) — the undo
/// STACK is lost for that one upgrade, while text/notes/graveyard all reload
/// via their own channels. Documented, one-time, non-destructive.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct History {
    undo: Vec<Transaction>,
    redo: Vec<Transaction>,
    undo_states: Vec<(SpanSet, BlockMap, Annotations, Graveyard)>,
    redo_states: Vec<(SpanSet, BlockMap, Annotations, Graveyard)>,
}

impl History {
    /// Asset ids any undo/redo state could resurrect (GC must keep them).
    pub fn asset_refs(&self) -> impl Iterator<Item = &str> {
        self.undo_states
            .iter()
            .chain(self.redo_states.iter())
            .flat_map(|(_, blocks, _, _)| blocks.asset_refs())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn set_aside_births_the_rail_keeps_the_caret_in_prose_and_undoes() {
        let mut doc = Document::new("alpha beta gamma", SpanSet::default(), BlockMap::default());
        assert_eq!(doc.aside_boundary(), None);
        // Move "beta " to the compost — a MOVE, so nothing is filed.
        let caret = doc.set_aside(6..11).unwrap();
        assert_eq!(doc.text(), "beta \n\nalpha gamma");
        assert_eq!(doc.aside_boundary(), Some(1));
        assert!(doc.graveyard().is_empty(), "a move never files a corpse");
        // Caret parked at the collapse point (after "alpha ").
        assert_eq!(&doc.text()[doc.char_to_byte(caret)..], "gamma");
        // The manuscript excludes the compost.
        assert_eq!(doc.manuscript_slice().0, "alpha gamma");
        // One undo reverses the whole move.
        doc.undo();
        assert_eq!(doc.text(), "alpha beta gamma");
        assert_eq!(doc.aside_boundary(), None);
    }

    #[test]
    fn set_aside_appends_a_second_item_with_a_separator() {
        let mut doc = Document::new("one two three four", SpanSet::default(), BlockMap::default());
        doc.set_aside(0..4).unwrap(); // "one " -> compost
        assert_eq!(doc.aside_boundary(), Some(1));
        // "two " is now at chars [0,4) of "two three four"; move it too.
        let two = doc.text().find("two ").unwrap();
        doc.set_aside(two..two + 4).unwrap();
        // Two compost items separated by a blank line, then the separator.
        assert_eq!(doc.aside_boundary(), Some(3));
        assert!(doc.text().starts_with("one \n\ntwo \n\n"), "{}", doc.text());
        assert_eq!(doc.manuscript_slice().0, "three four");
        // A restore to a boundary-less state clears the rail (reset).
        doc.restore_state("plain", SpanSet::default(), BlockMap::new(1));
        assert_eq!(doc.aside_boundary(), None);
        assert_eq!(doc.graveyard().len(), 0);
    }

    #[test]
    fn graveyard_apply_op_shifts_and_clamps_origin() {
        let mut g = Graveyard::default();
        let id = g.file("cut".into(), "before".into(), 10, 0, SpanSet::default(), Vec::new());
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
        let id = doc.cut_to_graveyard(4..len, "The ".into(), 42);
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
        let id = doc.cut_to_graveyard(3..9, "012".into(), 0);
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
        let id = g.file("XX".into(), String::new(), 0, 0, SpanSet::default(), Vec::new()); // origin in compost
        doc.set_graveyard(g);
        doc.put_back(id).unwrap();
        assert!(doc.text().starts_with("cc\n\n"), "compost untouched: {}", doc.text());
        assert!(doc.manuscript_slice().0.starts_with("XX"), "landed in the manuscript");
    }

    #[test]
    fn grave_delete_is_undoable() {
        let mut doc = Document::new("something reasonably long to hold", SpanSet::default(), BlockMap::default());
        let id = doc.cut_to_graveyard(0..9, String::new(), 0);
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
        let id = doc.cut_to_graveyard(0..cut_end, String::new(), 0);
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
        // The whole record round-trips too.
        let g: Graveyard = serde_json::from_str(
            r#"{"entries":[{"id":1,"text":"x","origin_quote":"","origin_pos":0,"cut_unix":0,"words":1}],"next_id":1}"#,
        )
        .expect("legacy graveyard loads");
        assert_eq!(g.len(), 1);
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

    #[test]
    fn migrate_writer_note_to_compost_but_never_a_diagnosis() {
        let mut doc = Document::new("body text here", SpanSet::default(), BlockMap::default());
        let note = doc.add_note(0..4, "my thought".into(), 0);
        assert!(doc.migrate_note_to_compost(note, "body"));
        assert!(doc.notes().get(note).is_none(), "note left the margin");
        assert_eq!(doc.aside_boundary(), Some(2));
        assert!(doc.text().starts_with("body\nmy thought\n\n"), "{}", doc.text());
        assert_eq!(doc.blocks().kind(0), &BlockKind::Blockquote, "anchor is a quote");
        assert!(doc.graveyard().is_empty(), "migration is a move, not a cut");
        // Undo restores the note and dissolves the rail.
        doc.undo();
        assert!(doc.notes().get(note).is_some());
        assert_eq!(doc.aside_boundary(), None);
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
}
