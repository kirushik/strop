//! The edit-run journal: when, where, and how much — for every edit.
//!
//! The record layer under the history strip (docs/history-strip.md,
//! docs/impl/00-journal.md). Deliberately self-sufficient: the Loro oplog
//! carries no wall-clock timestamps and is destroyed by shallow compaction
//! (`compact_on_open`), so nothing here may lean on it. Reconstruction
//! anchors on materialized checkpoint states and replays forward.

use std::ops::Range;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::buffer::TextOp;
use crate::document::{count_line_breaks, BlockMap, NoteKind, NoteStatus, SpanSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardDisposition {
    Anchored,
    Orphaned,
    Migrated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardRebaseEntry {
    pub id: u64,
    pub range: Range<usize>,
    pub status: NoteStatus,
    pub title: String,
    pub level: String,
    pub pass_id: u64,
    pub orphaned: bool,
    pub unverified: bool,
    pub disposition: CardDisposition,
}

/// A pause longer than this starts a new run — the journal's time grain.
/// Word-level detail inside a run is jittered by the strip, never recorded.
pub const RUN_SPLIT_MS: i64 = 2_000;

/// A run never absorbs more than this much wall time, so one uninterrupted
/// flow-state burst doesn't smear minutes of typing into a single x-extent.
pub const RUN_MAX_MS: i64 = 15_000;

/// One coalesced stretch of editing. `pos` is a char offset in the document
/// AS IT WAS when the run began; replaying runs forward in order is exact by
/// construction. Semantics match `TextOp`: delete `del_chars` at `pos`, then
/// insert `ins` there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditRun {
    pub t0: i64,
    pub t1: i64,
    pub pos: usize,
    pub del_chars: usize,
    /// Exact count from deleted text. `None` means a legacy run whose count
    /// was never recorded; it must remain distinguishable from exact zero.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub del_words: Option<usize>,
    pub ins: String,
}

impl EditRun {
    /// Words inserted (counted from the kept text). Deleted words are
    /// estimated by the strip from `del_chars` — the deleted text itself is
    /// deliberately not stored (forward replay never needs it, and the
    /// graveyard captures cut prose as its own feature).
    pub fn ins_words(&self) -> usize {
        self.ins.split_whitespace().count()
    }

    /// Deleted words and whether the count is exact. Legacy runs retain the
    /// fixed character-ratio estimate without gaining false precision.
    pub fn deleted_words(&self) -> (usize, bool) {
        match self.del_words {
            Some(words) => (words, true),
            None => ((self.del_chars as f32 / 5.5).round() as usize, false),
        }
    }

    /// Net char growth (negative on cuts) — the envelope's derivative.
    pub fn delta_chars(&self) -> i64 {
        self.ins.chars().count() as i64 - self.del_chars as i64
    }
}

/// Non-edit history the strip draws: passes, card closures, restores,
/// exports — and every SEAM mutation (time-persistence 2: boundary changes
/// were invisible to the record; a scrub-restore across a park silently
/// re-scoped the pile as manuscript). Card lifecycle snapshots join that
/// stream in v3; checkpoints continue to carry their own creation time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JournalEvent {
    /// An AI read landed: `mode` is developmental|line|copy (or "believing").
    Pass { t: i64, mode: String, cards: u32 },
    CardRaised {
        t: i64,
        id: u64,
        card_kind: NoteKind,
        range: Range<usize>,
        body: String,
        title: String,
        level: String,
        pass_id: u64,
        status: NoteStatus,
        orphaned: bool,
        unverified: bool,
    },
    CardEdited {
        t: i64,
        id: u64,
        body: String,
        title: String,
        level: String,
        pass_id: u64,
        status: NoteStatus,
        orphaned: bool,
        unverified: bool,
    },
    /// A card left the margin by the writer's hand.
    CardClosed { t: i64, id: u64, resolved: bool },
    /// A restore appended: the document became `len_chars` long, copied
    /// from the state at `from_unix` (seconds, matching checkpoints).
    Restore { t: i64, from_unix: i64, len_chars: usize },
    CardsRebased { t: i64, entries: Vec<CardRebaseEntry> },
    Export { t: i64 },
    /// The scrap line moved: born, shifted, or evaporated (`at: None`).
    /// Recorded by every boundary mutation — park, chord-park, adoption,
    /// undo/redo, put-back re-birth, exile collapse, migration — and applied
    /// by `ReplayDoc::advance` interleaved with runs by timestamp, so a
    /// reconstruction (and the strip's Restore of it) carries the scrubbed
    /// moment's own seam. `at` is the tail-era boundary block index.
    Seam { t: i64, at: Option<usize> },
}

impl JournalEvent {
    pub fn t(&self) -> i64 {
        match self {
            Self::Pass { t, .. }
            | Self::CardRaised { t, .. }
            | Self::CardEdited { t, .. }
            | Self::CardClosed { t, .. }
            | Self::Restore { t, .. }
            | Self::CardsRebased { t, .. }
            | Self::Export { t }
            | Self::Seam { t, .. } => *t,
        }
    }
}

/// The append-only record. Recording is O(1) amortized; the open tail run
/// keeps absorbing ops until a pause, a size cap, or `settle` (every save
/// settles first, so persistence only ever sees finished runs).
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Journal {
    pub runs: Vec<EditRun>,
    pub events: Vec<JournalEvent>,
    /// Whether the last run may still coalesce. Transient: a loaded journal
    /// never re-opens its tail.
    #[serde(skip)]
    open: bool,
    /// Wholesale operations (restore, seed) suppress run recording — the
    /// caller records the honest event instead. Transient.
    #[serde(skip)]
    paused: bool,
    /// Deleted text for the open tail only. It is reduced to `del_words`
    /// eagerly and discarded at settle; deleted prose never reaches the wire.
    #[serde(skip)]
    deleted_text: String,
}

/// Wall clock in unix milliseconds. Callers clamp against the journal's own
/// tail (`Journal::clamp`) so a clock stepping backwards mid-session can
/// never produce a time-travelling record.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl Journal {
    /// Rebuild from persisted parts (the tail never re-opens on load).
    pub fn from_parts(runs: Vec<EditRun>, events: Vec<JournalEvent>) -> Self {
        Self {
            runs,
            events,
            open: false,
            paused: false,
            deleted_text: String::new(),
        }
    }

    /// Latest instant the journal knows about (runs and events both).
    fn last_ms(&self) -> i64 {
        let run = self.runs.last().map(|r| r.t1).unwrap_or(0);
        let ev = self.events.last().map(|e| e.t()).unwrap_or(0);
        run.max(ev)
    }

    fn clamp(&self, t: i64) -> i64 {
        t.max(self.last_ms())
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    /// Close the tail run; the next op starts a new one. Every save calls
    /// this first, so persisted runs are immutable once written.
    pub fn settle(&mut self) {
        self.open = false;
        self.deleted_text.clear();
    }

    /// Record one applied text op at wall time `t` (ms). Coalescing mirrors
    /// the feel of the undo contract without its word-boundary rule: typing
    /// extends forward, backspace eats backward, forward-delete stays put —
    /// split by pauses (RUN_SPLIT_MS) and the smear cap (RUN_MAX_MS).
    pub fn record(&mut self, op: &TextOp, t: i64) {
        self.record_inner(op, t, None);
    }

    /// Record an op with deleted text captured by the buffer before release.
    /// The text lives only in the open-run accumulator and is never serialized.
    pub fn record_deleted(&mut self, op: &TextOp, t: i64, deleted: &str) {
        self.record_inner(op, t, Some(deleted));
    }

    fn record_inner(&mut self, op: &TextOp, t: i64, deleted: Option<&str>) {
        if self.paused || (op.delete == 0 && op.insert.is_empty()) {
            return;
        }
        let t = self.clamp(t);
        if self.open
            && let Some(last) = self.runs.last_mut()
        {
            let fresh = t - last.t1 <= RUN_SPLIT_MS && t - last.t0 <= RUN_MAX_MS;
            let ins_end = last.pos + last.ins.chars().count();
            if fresh && op.delete == 0 && op.pos == ins_end {
                // Typing continues right after the run's inserted text.
                last.ins.push_str(&op.insert);
                last.t1 = t;
                return;
            }
            if fresh && op.insert.is_empty() && last.ins.is_empty() {
                if op.pos + op.delete == last.pos {
                    // Backspace eats backward.
                    last.pos = op.pos;
                    last.del_chars += op.delete;
                    if last.del_words.is_some() {
                        if let Some(deleted) = deleted {
                            self.deleted_text.push_str(deleted);
                            last.del_words = Some(self.deleted_text.split_whitespace().count());
                        } else {
                            last.del_words = None;
                            self.deleted_text.clear();
                        }
                    }
                    last.t1 = t;
                    return;
                }
                if op.pos == last.pos {
                    // Forward-delete stays in place.
                    last.del_chars += op.delete;
                    if last.del_words.is_some() {
                        if let Some(deleted) = deleted {
                            self.deleted_text.push_str(deleted);
                            last.del_words = Some(self.deleted_text.split_whitespace().count());
                        } else {
                            last.del_words = None;
                            self.deleted_text.clear();
                        }
                    }
                    last.t1 = t;
                    return;
                }
            }
        }
        self.deleted_text = deleted.unwrap_or_default().to_owned();
        self.runs.push(EditRun {
            t0: t,
            t1: t,
            pos: op.pos,
            del_chars: op.delete,
            del_words: deleted.map(|_| self.deleted_text.split_whitespace().count()),
            ins: op.insert.clone(),
        });
        self.open = true;
    }

    /// Append a non-edit event. An event is a boundary in the record: the
    /// open run seals (no run smears across it), and its moment sorts
    /// STRICTLY after every run already written — a card raised in the same
    /// millisecond as a keystroke was still raised after it, and replay must
    /// not re-apply that keystroke to a range that already contains it.
    /// Events may still share a millisecond with each other (Restore +
    /// CardsRebased are one deliberate pair).
    pub fn record_event(&mut self, ev: JournalEvent) {
        let run_floor = self.runs.last().map_or(i64::MIN, |r| r.t1 + 1);
        let clamped = self.clamp(ev.t()).max(run_floor);
        self.settle();
        let ev = match ev {
            JournalEvent::Pass { mode, cards, .. } => JournalEvent::Pass {
                t: clamped,
                mode,
                cards,
            },
            JournalEvent::CardRaised { id, card_kind, range, body, title, level, pass_id,
                status, orphaned, unverified, .. } => JournalEvent::CardRaised {
                t: clamped, id, card_kind, range, body, title, level, pass_id,
                status, orphaned, unverified,
            },
            JournalEvent::CardEdited { id, body, title, level, pass_id, status,
                orphaned, unverified, .. } => JournalEvent::CardEdited {
                t: clamped, id, body, title, level, pass_id, status, orphaned, unverified,
            },
            JournalEvent::CardClosed { id, resolved, .. } => JournalEvent::CardClosed {
                t: clamped,
                id,
                resolved,
            },
            JournalEvent::Restore {
                from_unix,
                len_chars,
                ..
            } => JournalEvent::Restore {
                t: clamped,
                from_unix,
                len_chars,
            },
            JournalEvent::CardsRebased { entries, .. } => JournalEvent::CardsRebased {
                t: clamped,
                entries,
            },
            JournalEvent::Export { .. } => JournalEvent::Export { t: clamped },
            JournalEvent::Seam { at, .. } => JournalEvent::Seam { t: clamped, at },
        };
        self.events.push(ev);
    }

    /// Index of the first run strictly after `t_ms` — `runs[..i]` is the
    /// prefix a reconstruction at `t_ms` replays (a run straddling t applies
    /// whole; runs are seconds long, below the scrub's perceptual grain).
    pub fn runs_until(&self, t_ms: i64) -> usize {
        self.runs.partition_point(|r| r.t0 <= t_ms)
    }

    /// The Seam events, in record order: `(t, at)` pairs. Events are
    /// appended time-clamped, so this sequence is monotonic in t.
    pub fn seams(&self) -> impl Iterator<Item = (i64, Option<usize>)> + '_ {
        self.events.iter().filter_map(|e| match e {
            JournalEvent::Seam { t, at } => Some((*t, *at)),
            _ => None,
        })
    }

    /// How many Seam events fall at or before `t_ms` — the replay cursor a
    /// reconstruction anchored AT `t_ms` starts from (the anchor state's own
    /// BlockMap already reflects them).
    pub fn seams_until(&self, t_ms: i64) -> usize {
        self.seams().take_while(|(t, _)| *t <= t_ms).count()
    }
}

/// A reconstructed past document: text + formatting evolved from an anchor
/// (a materialized checkpoint state, or the empty document) through the
/// journal's runs. Lean on purpose — no undo stacks, no notes — but built
/// from the SAME primitives live editing uses (`SpanSet::apply_op`,
/// `BlockMap::on_edit`), so the invariants ride along. Explicit formatting
/// toggles between anchors are not journaled and therefore not replayed:
/// mid-session states carry the anchor's formatting evolved through the
/// edits (docs/impl/01-history-strip.md §2, the accepted v1 grain).
#[derive(Debug, Clone)]
pub struct ReplayDoc {
    pub rope: ropey::Rope,
    pub spans: SpanSet,
    pub blocks: BlockMap,
    /// Runs consumed so far — the strip scrubs forward incrementally and
    /// re-anchors on backward jumps.
    pub applied: usize,
    /// Seam events consumed so far (see `Journal::seams_until`); the
    /// boundary itself lives in `blocks`, exactly as in the live doc.
    pub seams_applied: usize,
}

impl ReplayDoc {
    pub fn new(text: &str, spans: SpanSet, blocks: BlockMap, applied: usize) -> Self {
        let rope = ropey::Rope::from_str(text);
        let mut blocks = blocks;
        if blocks.len() != rope.len_lines() {
            // Rebuild the kinds but CARRY THE BOUNDARY through the clamp
            // (time-persistence 7): a foreign or off-by-one state must
            // degrade approximately, never into the silent scope trespass of
            // a dropped seam. Truly out-of-range indexes clamp to None.
            let mut fresh = BlockMap::new(rope.len_lines());
            fresh.set_aside_boundary(blocks.aside_boundary());
            fresh.set_scrap_line(blocks.scrap_line());
            blocks = fresh;
        }
        Self {
            rope,
            spans,
            blocks,
            applied,
            seams_applied: 0,
        }
    }

    /// Apply one run. Positions are clamped — a foreign or damaged journal
    /// renders approximately, it never panics.
    pub fn apply(&mut self, run: &EditRun) {
        let len = self.rope.len_chars();
        let pos = run.pos.min(len);
        let del = run.del_chars.min(len - pos);
        let block = self.rope.char_to_line(pos);
        let merged = self.rope.char_to_line(pos + del) - block;
        self.rope.remove(pos..pos + del);
        self.rope.insert(pos, &run.ins);
        self.blocks
            .on_edit(block, merged, count_line_breaks(&run.ins));
        self.spans.apply_op(&TextOp {
            pos,
            delete: del,
            insert: run.ins.clone(),
        });
        self.applied += 1;
    }

    /// Replay forward until `t_ms`, runs and Seam events interleaved by
    /// timestamp (time-persistence 2: the boundary evolves with the text it
    /// belongs to; on a tie the run applies first — a park's move ops precede
    /// its boundary rider). Returns whether anything changed (the strip
    /// repaints only then). Pre-Scraps journals carry no Seam events and
    /// reconstruct against their own era's anchors unchanged.
    pub fn advance(&mut self, journal: &Journal, t_ms: i64) -> bool {
        let runs_until = journal.runs_until(t_ms);
        let seams: Vec<(i64, Option<usize>)> = journal.seams().collect();
        let seams_until = seams.partition_point(|(t, _)| *t <= t_ms);
        let (from_runs, from_seams) = (self.applied, self.seams_applied);
        loop {
            let next_run = (self.applied < runs_until).then(|| journal.runs[self.applied].t0);
            let next_seam =
                (self.seams_applied < seams_until).then(|| seams[self.seams_applied].0);
            let take_run = match (next_run, next_seam) {
                (Some(rt), Some(st)) => rt <= st, // tie: the run first
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (None, None) => break,
            };
            if take_run {
                let run = journal.runs[self.applied].clone();
                self.apply(&run);
            } else {
                self.blocks.set_scrap_line(seams[self.seams_applied].1);
                self.seams_applied += 1;
            }
        }
        from_runs != self.applied || from_seams != self.seams_applied
    }

    pub fn text(&self) -> String {
        self.rope.to_string()
    }
}

/// Char-range helper for strip veils and threads (kept here so the app
/// never re-derives event geometry).
pub fn clamp_range(r: &Range<usize>, len: usize) -> Range<usize> {
    r.start.min(len)..r.end.min(len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;

    fn ins(pos: usize, text: &str) -> TextOp {
        TextOp {
            pos,
            delete: 0,
            insert: text.into(),
        }
    }

    fn del(pos: usize, n: usize) -> TextOp {
        TextOp {
            pos,
            delete: n,
            insert: String::new(),
        }
    }

    #[test]
    fn typing_coalesces_and_pauses_split() {
        let mut j = Journal::default();
        j.record(&ins(0, "h"), 1_000);
        j.record(&ins(1, "i"), 1_100);
        j.record(&ins(2, " "), 1_200);
        assert_eq!(j.runs.len(), 1);
        assert_eq!(j.runs[0].ins, "hi ");
        j.record(&ins(3, "x"), 1_200 + RUN_SPLIT_MS + 1);
        assert_eq!(j.runs.len(), 2, "a pause starts a new run");
    }

    #[test]
    fn backspace_eats_backward_and_forward_delete_stays() {
        let mut j = Journal::default();
        j.record(&del(5, 1), 1_000);
        j.record(&del(4, 1), 1_050);
        assert_eq!(j.runs.len(), 1);
        assert_eq!((j.runs[0].pos, j.runs[0].del_chars), (4, 2));
        let mut j = Journal::default();
        j.record(&del(3, 1), 1_000);
        j.record(&del(3, 1), 1_050);
        assert_eq!(j.runs.len(), 1);
        assert_eq!((j.runs[0].pos, j.runs[0].del_chars), (3, 2));
    }

    #[test]
    fn replace_is_one_run_and_typing_after_it_coalesces() {
        let mut j = Journal::default();
        j.record(
            &TextOp {
                pos: 2,
                delete: 3,
                insert: "ab".into(),
            },
            1_000,
        );
        j.record(&ins(4, "c"), 1_100);
        assert_eq!(j.runs.len(), 1);
        assert_eq!(j.runs[0].del_chars, 3);
        assert_eq!(j.runs[0].ins, "abc");
    }

    #[test]
    fn smear_cap_splits_a_long_flow_burst() {
        let mut j = Journal::default();
        let mut t = 1_000;
        let mut pos = 0;
        while t < 1_000 + RUN_MAX_MS + 4_000 {
            j.record(&ins(pos, "a"), t);
            pos += 1;
            t += 1_000;
        }
        assert!(j.runs.len() >= 2, "RUN_MAX_MS bounds a run's x-extent");
    }

    #[test]
    fn clock_stepping_backwards_stays_monotonic() {
        let mut j = Journal::default();
        j.record(&ins(0, "a"), 5_000);
        j.record(&ins(1, "b"), 4_000); // laptop clock jumped back
        assert!(j.runs.iter().all(|r| r.t0 <= r.t1));
        assert_eq!(j.runs.last().unwrap().t1, 5_000);
    }

    #[test]
    fn paused_journal_records_nothing() {
        let mut j = Journal::default();
        j.pause();
        j.record(&ins(0, "wholesale"), 1_000);
        j.resume();
        assert!(j.runs.is_empty());
    }

    #[test]
    fn settle_freezes_the_tail() {
        let mut j = Journal::default();
        j.record(&ins(0, "a"), 1_000);
        j.settle();
        j.record(&ins(1, "b"), 1_050);
        assert_eq!(j.runs.len(), 2, "settled tail never re-opens");
    }

    #[test]
    fn a_card_event_seals_the_burst_and_sorts_past_it() {
        let mut j = Journal::default();
        j.record(&ins(0, "hi"), 1_000);
        j.record_event(JournalEvent::CardRaised {
            t: 1_000, id: 7, card_kind: NoteKind::Note, range: 0..2,
            body: "note".into(), title: String::new(), level: String::new(),
            pass_id: 0, status: NoteStatus::Open, orphaned: false, unverified: false,
        });
        assert_eq!(j.events[0].t(), 1_001, "a same-ms raise still lands after the keystroke");
        // The burst sealed: typing 100ms later (well inside RUN_SPLIT_MS)
        // starts a new run on the event's far side.
        j.record(&ins(2, "!"), 1_100);
        assert_eq!(j.runs.len(), 2, "no run smears across an event");
        assert!(j.runs[1].t0 >= j.events[0].t());
    }

    #[test]
    fn restore_and_rebase_still_share_their_millisecond() {
        let mut j = Journal::default();
        j.record(&ins(0, "x"), 1_000);
        j.record_event(JournalEvent::Restore { t: 2_000, from_unix: 2, len_chars: 1 });
        j.record_event(JournalEvent::CardsRebased { t: 2_000, entries: Vec::new() });
        assert_eq!((j.events[0].t(), j.events[1].t()), (2_000, 2_000),
            "the restore pair is one deliberate moment");
    }

    #[test]
    fn deleted_words_are_exact_and_legacy_stays_distinct() {
        let mut j = Journal::default();
        j.record_deleted(&del(0, 11), 1_000, "hello world");
        assert_eq!(j.runs[0].deleted_words(), (2, true));
        let legacy: EditRun = serde_json::from_str(
            r#"{"t0":1,"t1":1,"pos":0,"del_chars":11,"ins":""}"#,
        ).unwrap();
        assert_eq!(legacy.del_words, None);
        assert_eq!(legacy.deleted_words(), (2, false));
        let exact_zero = EditRun { del_words: Some(0), ..legacy.clone() };
        assert_eq!(exact_zero.deleted_words(), (0, true));
        assert!(!serde_json::to_string(&legacy).unwrap().contains("del_words"));
        assert!(serde_json::to_string(&exact_zero).unwrap().contains("del_words"));
    }

    #[test]
    fn charwise_deletes_count_the_coalesced_deleted_text() {
        let text = "hello world";
        let mut backspace = Journal::default();
        for (i, ch) in text.chars().rev().enumerate() {
            backspace.record_deleted(&del(text.chars().count() - i - 1, 1),
                1_000 + i as i64, &ch.to_string());
        }
        assert_eq!(backspace.runs.len(), 1);
        assert_eq!(backspace.runs[0].del_words, Some(2));

        let mut forward = Journal::default();
        for (i, ch) in text.chars().enumerate() {
            forward.record_deleted(&del(0, 1), 1_000 + i as i64, &ch.to_string());
        }
        assert_eq!(forward.runs.len(), 1);
        assert_eq!(forward.runs[0].del_words, Some(2));
    }

    #[test]
    fn mixed_exact_and_legacy_deletion_degrades_and_text_never_serializes() {
        let mut mixed = Journal::default();
        mixed.record_deleted(&del(1, 1), 1_000, "o");
        mixed.record(&del(0, 1), 1_001);
        assert_eq!(mixed.runs[0].del_words, None);

        let mut exact = Journal::default();
        exact.record_deleted(&del(0, 11), 1_000, "hello world");
        exact.settle();
        let json = serde_json::to_string(&exact).unwrap();
        assert!(json.contains("\"del_words\":2"));
        assert!(!json.contains("hello"));
        assert!(!json.contains("world"));
        assert!(!json.contains("deleted_text"));
    }

    /// The property the whole strip stands on: replaying the journal onto
    /// the starting state reproduces the live document, edit for edit.
    #[test]
    fn replay_reproduces_the_document() {
        let mut doc = Document::new("The ferry crossed.\nTwice.", SpanSet::default(), {
            let rope = ropey::Rope::from_str("The ferry crossed.\nTwice.");
            BlockMap::new(rope.len_lines())
        });
        let start_text = doc.text();
        let (start_spans, start_blocks) = (doc.spans().clone(), doc.blocks().clone());

        let script: Vec<(usize, usize, &str)> = vec![
            (18, 0, " Slowly"),
            (0, 3, "A"),
            (10, 6, "sailed"),
            (23, 0, "\nFog came up the channel."),
            (5, 4, ""),
        ];
        for (pos, delete, insert) in script {
            let start = doc.char_to_byte(pos);
            let end = doc.char_to_byte(pos + delete);
            // Recording happens inside the document's own op drain — the
            // test exercises the real wiring, wall clock and all.
            doc.edit_bytes(start..end, insert);
        }
        doc.undo();
        doc.redo(); // undo/redo ops journal as ordinary inverse edits

        let journal = doc.journal().clone();
        let mut replay = ReplayDoc::new(&start_text, start_spans, start_blocks, 0);
        replay.advance(&journal, i64::MAX);
        assert_eq!(replay.text(), doc.text());
        assert_eq!(replay.blocks.len(), doc.blocks().len());
    }

    #[test]
    fn damaged_journal_replays_clamped_not_panicking() {
        let mut replay = ReplayDoc::new("short", SpanSet::default(), BlockMap::new(1), 0);
        replay.apply(&EditRun {
            t0: 0,
            t1: 0,
            pos: 999,
            del_chars: 999,
            del_words: None,
            ins: "!".into(),
        });
        assert_eq!(replay.text(), "short!");
    }
}
