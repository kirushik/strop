# Save stall + oplog bloat — 2026-06-19

Investigated after a tester reported Strop stalling for seconds on click and
occasionally hanging hard enough for Ubuntu's "terminate or wait?" dialog, on a
`.strop` working file that had ballooned to **2.86 MB** for a few pages + one
~4 KB JPEG. Two independent root causes, found by benchmarking the real file.

## Measurements (the 2.86 MB file)

| metric | value |
|---|---|
| text | 4,897 chars |
| current state (`ShallowSnapshot(now)`) | **67 KB** |
| image asset | 3,698 bytes |
| full snapshot (state + history) | **2.86 MB** |
| total oplog ops | 16,498 |
| commits | 32 |
| ops added by ONE `save_with_state` | **+60** |
| `Store::open` (cold load) | ~1.3 s |
| `save_with_state` BEFORE fix | **6.8–10 s (and rising each save)** |
| `save_with_state` AFTER fix | **24 ms** |

So ~2.8 MB is *history*, not content — and the current state is only 67 KB.

## Bug 1 — the hang (FIXED, commit on `better_card_placement`)

`save_with_state` → `collect_unreachable_assets` ran on **every idle-save** (1 s
after each edit, on the main thread). Because an image makes the assets map
non-empty, it did a Loro `state_at()` historical checkout **per checkpoint**
(~1 s each, ~6.8 s for 8) just to re-confirm the image was still referenced —
and delete nothing. As the oplog grew (Bug 2), each checkout got slower, so the
stall worsened over a session ("since a few runs ago").

**Fix:** if every stored asset is already referenced by the live doc or undo
history, nothing can be orphaned → skip the per-checkpoint checkout (the delete
loop would be a no-op anyway). Only an actual image *deletion* triggers the
scan. Provably equivalent; 6.8 s → 24 ms.

## Bug 2 — the bloat (NOT fixed; needs a persistence decision)

`save_with_state` → `rebuild_marks` **unmarks every style key and re-marks every
formatting span on every save** (+60 Loro ops here). Over a session of frequent
idle-saves that's tens of thousands of byte-heavy mark ops — the 2.8 MB. The
image only *catalysed* Bug 1; the bloat is `rebuild_marks`. With Bug 1 fixed the
oplog no longer makes saves slow (export is 24 ms regardless), so this no longer
hangs — but the file still grows ~60 ops/save and cold-open creeps up.

Why it's wholesale: the SpanSet (hot path) is the source of truth and Loro marks
are treated as a save-time mirror, rebuilt each save (`store.rs` comment). A
text edit shifts span ranges, so a content-hash gate wouldn't help (it changes
every edit). Skipping the rebuild loses formatting on reload (marks ARE the
persistence). So the fix is structural — options, for Kirill:

1. **Persist the SpanSet as JSON** (like blocks & annotations already are): one
   value-update op per save instead of 60 mark ops; drop `rebuild_marks`
   entirely. Simplest, matches existing patterns. **Cost:** formatting stops
   being a CRDT mark, so it won't merge across peers if/when collaboration
   lands. (Single-user today — likely fine; flag for the collab future.)
2. **Mirror marks incrementally** (issue `text.mark`/`unmark` only when the
   writer actually changes formatting, like text ops are mirrored in `apply`),
   and stop rebuilding at save. Keeps CRDT formatting; **cost:** must trust
   Loro's mark-anchoring to track text edits exactly (the wholesale rebuild
   exists because the author distrusted this — verify before relying on it).
3. **Periodic shallow-snapshot compaction** to reclaim the *existing* 2.8 MB
   (Loro `export({mode:"shallow-snapshot", frontiers: oplogFrontiers()})` →
   ~67 KB). **Cost:** discards all history before the snapshot point — breaks
   the 8 existing checkpoints / time-travel. Destructive; offer as an explicit
   "compact history" action, never automatic.

Recommendation: option 1 for new bloat (cheap, safe, matches blocks/annotations)
+ option 3 as an explicit user-invoked "compact" to shrink the existing file —
but both are persistence-format changes that want sign-off and careful
load-path testing, so they're deferred to a decision, not done unattended.

## Follow-ups
- Regression test for the GC gate (referenced asset survives a save; an orphaned
  one is still reclaimed via the slow path) — slot into Phase 6.
- Cold-open ~1.3 s is the Loro import of the full oplog; option 1/3 would shrink
  it too. Lower priority than the bloat itself.

## Resolution (2026-07-03) — all of it landed

The deferred decision resolved itself when the same disease resurfaced as a
NEW hang: toggling the history sidebar froze the app on the reporter's file
(now 4.77 MB / 5.7 KB of prose). `enter_history` ran `state_at` per
checkpoint on the main thread — 71 s measured for 13 checkpoints. Profiling
overturned one June assumption: the dominant cost was never the checkout
itself but **`to_delta()` reading the marks back** (4.68 s on the *current*
state) — months of unmark/remark cycles had left thousands of dead style
anchors in the text state, taxing open, every checkout, and checkpoint
sealing alike.

Shipped, in order (branch `better_card_placement`):

1. **`025a1e0` — checkpoints materialize their state at creation**
   (`Checkpoint::state`); rewind/preview/restore/asset-GC never time-travel.
   Legacy checkpoints backfill once, in the background, persisted (+14 KB).
   History toggle: 71 s → 190 µs.
2. **`d7609ee` — save guards + undo trim.** `SavedHashes` fingerprints per
   channel (annotations/blocks/history/spans): unchanged saves append ZERO
   bytes (test-asserted). Persisted undo tail 200 → 50 entries (the 1.58 MB
   JSON was ~all of the live bloat).
3. **`1cef5fd` — option 1 AND option 3, made safe.** Spans persist as JSON
   (marks = read-only legacy; `rebuild_marks` deleted — no more churn, no
   more `to_delta` after the first open). And `Store::open` compacts
   opportunistically — the June "DESTRUCTIVE" caveat died with materialized
   checkpoints, since nothing readable needs the oplog: shallow snapshot,
   round-trip-verified, original kept once as `*.pre-compact.bak`, atomic
   swap, every failure path non-destructive.

Measured on the real file: **history 71 s → 190 µs; file 4.77 MB → 82 KB;
cold open 5.8 s → 4.9 ms.** A legacy file heals in two launches: first open
reads marks once and backfills in the background (~9 s, release), first save
writes the spans JSON; the next open compacts.

Cost honestly stated: single-user-first. CRDT mark merging across peers is
given up (spans are latest-wins JSON, like blocks/annotations already were),
and oplog history older than the compaction horizon is no longer
time-travellable — checkpoints carry their own states instead. Both match
the product's local-first, single-writer reality; revisit at sync time.
