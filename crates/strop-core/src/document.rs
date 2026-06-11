//! Rich-text schema types and the span/anchor machinery.
//!
//! See docs/document-model.md. Spans are char-indexed ranges over the same
//! text stream the rope and Loro share; `SpanSet::apply_op` keeps them
//! consistent across every edit (including undo/redo, which arrive as
//! ordinary ops). The same adjustment math will anchor annotations.

use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::buffer::{Buffer, TextOp, Transaction};

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
}

impl Default for BlockMap {
    fn default() -> Self {
        Self {
            kinds: vec![BlockKind::default()],
        }
    }
}

impl BlockMap {
    pub fn new(blocks: usize) -> Self {
        Self {
            kinds: vec![BlockKind::default(); blocks.max(1)],
        }
    }

    pub fn from_kinds(kinds: Vec<BlockKind>) -> Self {
        if kinds.is_empty() {
            Self::default()
        } else {
            Self { kinds }
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
        self.kinds.drain(block + 1..drain_end);
        let kind = self.kinds[block].clone();
        for _ in 0..splits {
            self.kinds.insert(block + 1, kind.clone());
        }
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
        self.spans = result;
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

    pub fn set_body(&mut self, id: u64, body: String) {
        if let Some(n) = self.notes.iter_mut().find(|n| n.id == id) {
            n.body = body;
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
            }
        }
        self.notes.sort_by_key(|n| n.range.start);
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
    undo_states: Vec<(SpanSet, BlockMap, Annotations)>,
    redo_states: Vec<(SpanSet, BlockMap, Annotations)>,
    pending_ops: Vec<TextOp>,
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
        for op in &ops {
            self.spans.apply_op(op);
            self.notes.apply_op(op);
        }
        self.pending_ops.extend(ops);
    }

    /// (block containing the edit start, newlines deleted) — computed
    /// against the pre-edit rope.
    fn pre_edit_info(&self, byte_range: &Range<usize>) -> (usize, usize) {
        let rope = self.buffer.rope();
        let start = rope.byte_to_char(byte_range.start);
        let end = rope.byte_to_char(byte_range.end);
        let block = rope.char_to_line(start);
        let merged = rope.slice(start..end).chars().filter(|c| *c == '\n').count();
        (block, merged)
    }

    fn snapshot(&self) -> (SpanSet, BlockMap, Annotations) {
        (self.spans.clone(), self.blocks.clone(), self.notes.clone())
    }

    pub fn notes(&self) -> &Annotations {
        &self.notes
    }

    pub fn set_notes(&mut self, notes: Annotations) {
        self.notes = notes;
    }

    /// Add an author note as its own undoable transaction.
    pub fn add_note(&mut self, range: Range<usize>, body: String, created_unix: i64) -> u64 {
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        self.notes.add(range, body, created_unix)
    }

    pub fn set_note_status(&mut self, id: u64, status: NoteStatus) {
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        self.notes.set_status(id, status);
    }

    pub fn set_note_body(&mut self, id: u64, body: String) {
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        self.notes.set_body(id, body);
    }

    /// Add a batch of diagnoses as ONE undoable transaction (one ctrl-z
    /// clears a whole pass).
    pub fn add_diagnoses(&mut self, diagnoses: Vec<Annotation>) {
        if diagnoses.is_empty() {
            return;
        }
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        for d in diagnoses {
            self.notes.push(d);
        }
    }

    pub fn edit_bytes(&mut self, byte_range: Range<usize>, text: &str) {
        let snapshot = self.snapshot();
        let (block, merged) = self.pre_edit_info(&byte_range);
        if self.buffer.edit_bytes(byte_range, text) {
            self.undo_states.push(snapshot);
            self.redo_states.clear();
        }
        self.blocks
            .on_edit(block, merged, text.matches('\n').count());
        self.absorb_buffer_ops();
    }

    pub fn edit_bytes_coalescing(&mut self, byte_range: Range<usize>, text: &str) {
        let snapshot = self.snapshot();
        let (block, merged) = self.pre_edit_info(&byte_range);
        if self.buffer.edit_bytes_coalescing(byte_range, text) {
            self.undo_states.push(snapshot);
            self.redo_states.clear();
        }
        self.blocks
            .on_edit(block, merged, text.matches('\n').count());
        self.absorb_buffer_ops();
    }

    /// Toggle `attr` over a char range as its own undoable transaction.
    pub fn toggle_format(&mut self, range: Range<usize>, attr: InlineAttr) {
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

    /// Set a block's kind as its own undoable transaction.
    pub fn set_block_kind(&mut self, block: usize, kind: BlockKind) {
        let snapshot = self.snapshot();
        self.buffer.push_empty_transaction();
        self.undo_states.push(snapshot);
        self.redo_states.clear();
        self.blocks.set_kind(block, kind);
    }

    /// Change a block's kind inside the current transaction (rides a text
    /// edit, e.g. the `# `-shortcut or Enter-at-heading-end).
    pub fn set_block_kind_in_current_tx(&mut self, block: usize, kind: BlockKind) {
        self.blocks.set_kind(block, kind);
    }

    /// Apply/clear an attribute inside the *current* transaction (sticky
    /// caret formatting riding a typing transaction) — undone together
    /// with the typed text.
    pub fn format_in_current_tx(&mut self, range: Range<usize>, attr: InlineAttr, on: bool) {
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
        if let Some((spans, blocks, notes)) = self.undo_states.pop() {
            self.redo_states.push((
                std::mem::replace(&mut self.spans, spans),
                std::mem::replace(&mut self.blocks, blocks),
                std::mem::replace(&mut self.notes, notes),
            ));
        }
        // Buffer inverse ops still mirror to the store, but must NOT be
        // re-applied to spans/blocks (the snapshot is the correct state).
        self.pending_ops.extend(self.buffer.take_ops());
        Some(cursor)
    }

    pub fn redo(&mut self) -> Option<Option<usize>> {
        let cursor = self.buffer.redo()?;
        if let Some((spans, blocks, notes)) = self.redo_states.pop() {
            self.undo_states.push((
                std::mem::replace(&mut self.spans, spans),
                std::mem::replace(&mut self.blocks, blocks),
                std::mem::replace(&mut self.notes, notes),
            ));
        }
        self.pending_ops.extend(self.buffer.take_ops());
        Some(cursor)
    }

    /// Replace the whole document state as ONE undoable transaction —
    /// checkpoint restore semantics: rewinding is a forward edit, history
    /// stays append-only, and ctrl-z takes you back to the present.
    pub fn restore_state(&mut self, text: &str, spans: SpanSet, blocks: BlockMap) {
        let snapshot = self.snapshot();
        let len = self.buffer.len_bytes();
        if self.buffer.edit_bytes(0..len, text) {
            self.undo_states.push(snapshot);
            self.redo_states.clear();
        }
        self.absorb_buffer_ops();
        // The wholesale text op mangled span/block adjustment; the restored
        // state is authoritative.
        self.spans = spans;
        let lines = self.buffer.rope().len_lines();
        self.blocks = if blocks.len() == lines {
            blocks
        } else {
            BlockMap::new(lines)
        };
    }

    /// Export undo/redo state for persistence (most-recent `cap` entries).
    /// Saved atomically with the text it refers to, so it restores exactly.
    pub fn export_history(&self, cap: usize) -> History {
        let (undo, redo) = self.buffer.export_history(cap);
        let tail = |v: &Vec<(SpanSet, BlockMap, Annotations)>| {
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
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct History {
    undo: Vec<Transaction>,
    redo: Vec<Transaction>,
    undo_states: Vec<(SpanSet, BlockMap, Annotations)>,
    redo_states: Vec<(SpanSet, BlockMap, Annotations)>,
}

impl History {
    /// Asset ids any undo/redo state could resurrect (GC must keep them).
    pub fn asset_refs(&self) -> impl Iterator<Item = &str> {
        self.undo_states
            .iter()
            .chain(self.redo_states.iter())
            .flat_map(|(_, blocks, _)| blocks.asset_refs())
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

    fn strong(range: Range<usize>) -> Span {
        Span {
            range,
            attr: InlineAttr::Strong,
        }
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
    fn attrs_at_reports_covering_spans() {
        let mut set = SpanSet::default();
        set.add(0..4, InlineAttr::Strong);
        set.add(2..6, InlineAttr::Emphasis);
        let at3: Vec<_> = set.attrs_at(3).collect();
        assert_eq!(at3.len(), 2);
        assert_eq!(set.attrs_at(5).count(), 1);
        assert_eq!(set.attrs_at(6).count(), 0); // end-exclusive
    }
}
