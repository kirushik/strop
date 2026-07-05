# Impl spec 00 — the edit-run journal

*(The record layer under the history strip and every future
process-literacy feature. Design doc: `docs/history-strip.md` v2.
Status: SPEC — pre-review draft.)*

## 0. Why it must exist (and why Loro can't be it)

The strip needs, for every edit: **when** (wall clock), **where**
(char span), **what kind** (insert/delete), **how much** (words). The
engine records none of this today: `buffer.rs` transactions coalesce
for undo but carry no timestamps, and the Loro oplog is not viable as
the record for two hard reasons:

1. **`compact_on_open` writes shallow snapshots** — the oplog is
   *deliberately destroyed* as the bloat fix. Any feature that reads
   deep history from Loro dies on its first compacted file.
2. Historical checkout (`state_at`) replays the whole oplog — the 5–7s
   per-checkpoint hang that materialized checkpoint states were built
   to kill. Frame-rate scrubbing cannot touch it.

So the journal is **self-sufficient**: its own append-only record,
persisted inside the `.strop` alongside checkpoints/history/
annotations, with reconstruction anchored on the *already-materialized*
checkpoint states.

## 1. Data model (`strop-core`, new module `journal.rs`)

```rust
/// One coalesced stretch of same-kind editing. `pos` is a char offset
/// in the document AS IT WAS when the run began — forward replay in
/// order is exact by construction.
pub struct EditRun {
    pub t0: i64,          // unix ms, first op of the run
    pub t1: i64,          // unix ms, last op absorbed
    pub pos: usize,       // char offset at run start
    pub del_chars: usize, // chars removed at pos (before ins)
    pub del_words: u32,   // counted at record time; del text NOT kept
    pub ins: String,      // text inserted at pos (in order)
}

pub enum JournalEvent {
    Pass    { t0: i64, t1: i64, kind: String, span: Option<Range<usize>> },
    CardRaised { t: i64, id: u64, anchor: usize, level: String },
    CardClosed { t: i64, id: u64, resolved: bool },
    Station { t: i64, checkpoint_ix: usize }, // ties a checkpoint to a journal position
    Restore { t: i64, from_unix: i64, new_len: usize },
    Export  { t: i64 },
}

pub struct Journal {
    pub runs: Vec<EditRun>,
    pub events: Vec<JournalEvent>,
}
```

Decisions baked in:

- **Forward-replay only.** Reconstruction always replays forward from
  an anchor ≤ t, so a run needs `del_chars` (a length), never the
  deleted text. This roughly halves storage; the graveyard captures cut
  *text* at cut time as its own feature, not from the journal.
- **Coalescing rule** mirrors the undo contract (`buffer.rs`):
  same-kind, contiguous, and a new run at a >2s pause or a whitespace
  boundary group. A replace-selection is one del run + one ins run.
  Target density: a fortnight story ≈ low thousands of runs.
- **Word counts at record time** (`del_words`; `ins` words derivable),
  so the strip's fabric never re-tokenizes history.
- **The clock** is `SystemTime` unix ms, clamped monotonic
  (`t = t.max(last_t1)`) — a laptop clock jumping backward must not
  produce a time-travelling record.

## 2. The recording choke point

`Document::take_ops` → `Store::apply(ops)` is where every buffer
mutation — typing, paste, formatting-driven edits, note promotion,
and undo/redo (which surface as ordinary inverse ops — honest, the
envelope visibly steps back) — already funnels through as
`TextOp{pos, delete, insert}` (buffer.rs:24-30, document.rs:642-644,
store.rs:766-777). The journal hook lives at that drain:
`journal.record(op, now_ms)` before the ops mirror into Loro.

**Wholesale ops are suppressed.** `restore_state` (and `seed`/import)
replace the entire text with one `edit_bytes(0..len, …)`; recording
that literally would journal the whole document as one giant run per
restore. A `journal.pause()` guard wraps wholesale operations, and the
caller records the honest event instead (`Restore{…}` with the new
length — the envelope steps without storing the text twice).

Event recording points: pass start/end where `diagnose`/`llm` runs
complete; card status changes in `Annotations::set_status` callers;
`Store::add_checkpoint` records `Station`; export path records
`Export`.

## 3. Persistence

The `Journal` serializes with serde exactly like `History` and
`Annotations`, saved in `save_with_state` and loaded into `Loaded`.
Absent on older files → `Journal::default()` (see §5 degradation).
The save fingerprint guard already means unchanged docs write zero
bytes; the journal only grows when edits happened, so the guard's
contract is preserved.

**Size budget** (the bloat saga must not restart): a fortnight story's
churn ≈ 45k chars of `ins` text + ~60 bytes/run overhead × ~4k runs ≈
**~300 KB uncompressed JSON worst case**. Acceptable for v1; the
measured number goes in the tests (§6). If real files exceed budget,
the future lever is age-tiered thinning (drop `ins` text on runs older
than N sessions, keep counts — the fabric only needs counts; the scrub
loses mid-session granularity older than N but keeps every checkpoint
anchor). The lever is DESIGNED but not built in v1.

## 4. Reconstruction (the scrub's engine)

```rust
/// Document text as it stood at time t.
pub fn text_at(&self, t: i64, checkpoints: &[Checkpoint]) -> String
```

1. Anchor: the latest checkpoint with `created_unix ≤ t` **and** a
   materialized `state` (they all have one after backfill); its
   `Station` event locates the journal index. No checkpoint ≤ t →
   anchor = empty doc at journal start.
2. Forward-replay runs with `anchor_t < t1 ≤ t` into a rope. A run
   straddling t (t0 ≤ t < t1) applies whole — runs are seconds long,
   below the scrub's perceptual grain.
3. **Scrub cache:** the strip holds the last `(t, rope)`; dragging
   right replays only the delta runs; dragging left re-anchors. Worst
   case = one session of runs (~hundreds of rope edits) per frame —
   well inside a 60fps budget, and the cache makes the common case
   incremental.

Fidelity: reconstruction is TEXT ONLY — the scrub preview renders
plain paragraphs. Full formatting fidelity exists exactly at stations
(checkpoint states carry spans+blocks); between stations the past is
plain. This is v1's honest cut, stated in the strip UI spec.

Envelope derivation: cumulative `ins.chars() − del_chars` per run,
seeded from each anchor's length — the strip never recomputes text to
draw the envelope, only to scrub.

## 5. Degradation & compatibility

- **Old files (no journal):** the strip still works at station
  granularity — envelope from checkpoint states' lengths, stations,
  restore events unavailable → not drawn. No error, no apology.
- **A file edited by an older build after the journal exists:** runs
  are missing for that era; reconstruction anchors still hold
  (checkpoints), the fabric shows a quiet gap. The journal is a
  best-effort record, never an integrity constraint.
- Journal misalignment (corrupt/foreign) → dropped like `History`
  (never trusted into a panic).

## 6. Tests

- Typing coalesces into runs with monotonic times; replace = del+ins
  pair; pause >2s splits.
- Restore/seed record events, not giant runs; `journal.pause()` guard
  is exception-safe (RAII).
- **Property test:** after arbitrary edit scripts,
  `text_at(now) == document.text()`, and `text_at(cp.t)` ==
  materialized state text for every checkpoint.
- Persistence roundtrip; legacy file loads with empty journal; corrupt
  journal dropped.
- Size: N words of scripted churn produce ≤ budgeted bytes.
- Clock-jump: a backwards SystemTime step still yields monotonic runs.
