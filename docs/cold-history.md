# Cold history: bounded RAM without a bounded past

Status: post-0.2 backlog design. The 0.2 boundary is deliberately smaller:
unlimited in-session undo remains in memory, while its side-state snapshots now
use structural sharing (`Arc` + copy-on-write). This document specifies the
next storage change; it is not permission to cap, coarsen, or discard history.

## Product invariant

Moving history between RAM and disk may change where it lives, never what the
writer can see or recover.

- The existing undo grain remains the truth: a coalesced typing transaction,
  formatting action, move, park, restore, etc. stays one step. “Stroke by
  stroke” means every one of those user-visible transactions, not raw keyboard
  events that the editor already deliberately groups into words.
- In-session undo and redo have no depth cap.
- Restart does not create a shallower past.
- The whole timeline remains visible without loading its exact contents.
- Restore remains a forward, undoable edit; no branch-DAG UI appears.
- A crash may lose neither the current document nor a history chunk already
  reported durable.

## What 0.2 establishes

`Buffer::Transaction` is already an exact reversible text delta: each primitive
edit stores its character position plus old and new text. `Document` aligns each
transaction with a five-part side state (spans, blocks, annotations, graveyard,
provenance). In 0.2 those five values are transparent `Arc`/COW wrappers, so a
plain edit does not clone an unchanged 5,000-block map merely to make an undo
frame. The JSON wire shape is unchanged.

The app currently persists the newest 50 undo/redo entries. That is a transport
limit, not the desired product contract. Raising it is the wrong next move:
Serde expands shared `Arc` values back into repeated full values, and the Loro
snapshot then carries those bytes through every save.

The journal cannot substitute for exact undo. It is intentionally a compact,
forward-only visual record: runs coalesce over 2-second gaps (with a 15-second
smear cap), deletions retain their length rather than deleted text, and most
side-state mutations are not reversible from it. It should remain the timeline
summary source until cold chunks can produce an equivalent summary.

## Reproducible measurements (2026-07-12)

Command:

```sh
cargo test -p strop-core --release cow_history_sharing_probe -- --ignored --nocapture
```

Fixture: 5,000 paragraph blocks, then 2,000 separate single-character
transactions that do not change the block map. Results on the release-test
machine:

| Measurement | Result |
|---|---:|
| Unchanged `BlockKind` clones avoided | 10,000,000 |
| Live block-map allocations across those frames | 1 |
| `SideState` handle, excluding Vec capacity and unique values | 40 bytes/frame |
| Serialize newest 50 frames as current JSON | 3,007,503 bytes / 4.56 ms |
| Serialize newest 200 frames as current JSON | 12,029,853 bytes / 22.43 ms |
| Apply 2,000 undos | 1.39 ms |

These are a structural stress fixture, not a prediction of an ordinary novel.
They establish two useful facts: live sharing solves the clone multiplier, and
the persisted snapshot representation destroys that sharing almost linearly.
Chunk work should be judged with additional prose-, formatting-, note-, image-,
and restore-heavy corpora before choosing final byte budgets.

## Target architecture

History has three representations, one logical sequence:

```text
whole-past summaries     prefetched exact chunks       hot exact transactions
      (small RAM)         (one behind / one ahead)          (RAM budget)
───────────────┬───────────────────────────────────┬──────────────────────────● now
               └──────── immutable durable storage ────────┘
```

### 1. One exact transaction record

Introduce a versioned `HistoryTxn` owned by a `HistoryStore` rather than by
parallel vectors in `Buffer` and `Document`:

- monotonic transaction id and parent id;
- timestamp and user-visible transaction kind;
- the existing reversible text edits;
- reversible typed side edits;
- optional cursor/selection landing metadata;
- enough summary data to feed the history strip without decoding the body.

Side edits should be an exhaustive Rust enum at mutation boundaries: span
changes, block splices/kinds/boundary, annotation changes, graveyard changes,
and provenance changes. Each variant must apply forward and inverse. Rare
wholesale operations may initially carry before/after side-channel values; the
common typing and formatting paths must not. This makes completeness a compiler
and test problem instead of a convention spread across serializers.

During transition, a chunk encoder may content-intern identical side values to
retain the Arc win on disk. That is a bridge, not the final answer for shifted
non-empty span sets; typed deltas are what remove their snapshot multiplier.

### 2. Hot RAM is a byte budget, not a history limit

Keep the newest exact records and their working values in memory. Seal from the
old end when measured resident bytes cross a configurable internal budget.
Depth is irrelevant: ten thousand tiny typing transactions may cost less than
one image/restore transaction. Sealing never deletes a record.

Do not choose the production budget from the synthetic result above. Benchmark
the corpus matrix, then choose a conservative default and retain telemetry-free
diagnostics (`STROP_PERF`) that report hot bytes, transaction count, chunk
count, page-ins, and worst page-in latency.

### 3. Immutable cold chunks

A sealed chunk contains:

- format version, document id, first/last transaction ids and parent ids;
- independently checksummed header, body, and timeline summary;
- a sparse transaction-offset index;
- compressed reversible records;
- a materialized anchor state at a measured interval, so arbitrary restore
  never replays from document birth;
- the strip summary for that interval.

Chunk boundaries should be driven primarily by uncompressed bytes, with a
transaction-count ceiling to bound index and replay work. Compression and
checksum construction run on the ordered persistence worker. The UI publishes
the chunk only after its commit record is durable.

Use checksummed framing rather than one large compressed stream: corruption
must sacrifice at most one independently identified frame/chunk, and a reader
must be able to report the exact damaged interval without trusting lengths from
an unverified body.

### 4. Prefetch makes the boundary disappear

The document keeps the current chunk plus its predecessor resident. When undo
enters the lower quarter of the hot window, prefetch the next cold chunk. Strip
hover/scrub also prefetches its interval. Redo mirrors the rule. A cache miss is
an asynchronous `HistoryStore::ensure(transaction_id)` operation; editing is
never blocked by compression, compaction, or a full-history scan.

Repeated Ctrl-Z must preserve ordering while a page-in is pending. The action
queue records intent, the UI gives restrained progress only beyond the normal
frame budget, and the queued operations execute once against the verified
chunk. Never silently stop at the RAM boundary.

### 5. One portable `.strop`, not a permanent sidecar

The final store should be a versioned outer envelope around the current Loro
snapshot, assets, immutable history chunks, and an index/commit footer. A
sidecar prototype is acceptable for tests, but not as the shipped authority:
copying one document must copy its whole recoverable past.

Normal save appends new immutable chunks, the newest document snapshot, and a
small commit record; it then syncs before exposing that generation. Old
generations are reclaimed by periodic atomic compaction, never on the UI
thread. Opening scans backward to the newest valid commit and ignores an
uncommitted tail. This is distinct from treating a `.tmp` file as a competing
recovery authority.

The current schema epoch lives inside the raw Loro snapshot. An outer envelope
therefore needs its own magic and container version. Migration must accept the
raw epoch-0/epoch-1 file as one document snapshot plus its current persisted
history tail, write the new envelope atomically, reopen/verify it, and only then
replace the old file. Older 0.2 binaries will not understand the envelope, so
this belongs in a deliberately versioned post-0.2 release.

## State and failure rules

- New edit after undo moves the logical head; it does not rewrite immutable
  chunks. Unreachable redo records become compaction garbage.
- Checkpoints and asset GC are roots. Cold records that can resurrect an asset
  keep it reachable until compaction proves the record unreachable.
- Chunk commit and document-snapshot commit share one generation. Neither may
  claim durability alone.
- A bad checksum fails closed for exact operations in that interval while the
  healthy current document still opens. Recovery UI names the interval and
  offers copying/revealing; it never invents a partial undo sequence.
- Unknown container, chunk, transaction, or side-edit versions are refused
  before mutation.
- Cancellation or process death during sealing leaves only an ignored tail;
  death during compaction leaves the prior file authoritative.

## Delivery sequence

1. **Corpus and accounting.** Add deterministic generators for prose-heavy,
   format-heavy, notes/graveyard, images, restores, and long redo branches.
   Measure live allocations, current JSON, reversible-delta bytes, compression,
   save preparation, and undo/page-in latency.
2. **Unify transaction ownership.** Replace the parallel buffer/side-state
   stacks with `HistoryTxn` behind an in-memory `HistoryStore`. Preserve every
   current semantic and wire fixture before adding disk behavior.
3. **Typed reversible side edits.** Instrument every mutation chokepoint; use
   fallback replacements only for rare wholesale operations. Property-test
   `apply(forward); apply(inverse)` and compare against current snapshot undo.
4. **Chunk codec and fault harness.** Deterministic encoding, sparse index,
   checksums, compression, truncation/bit-flip/unknown-version tests, and
   generation ordering tests.
5. **Cold backend and prefetch.** Introduce byte-budget sealing, async reads,
   intent queuing, and cache accounting. Stress repeated undo/redo across every
   boundary while foreground editing and saves continue.
6. **Outer envelope and migration.** Golden raw 0/1 fixtures, atomic migration,
   open/compaction fault injection, asset reachability, Save a Copy, and actual
   Windows/macOS/Linux filesystem tests.
7. **Remove the 50-entry transport tail.** Only after restart-spanning exact
   undo and arbitrary strip restore are equivalent to the unlimited in-memory
   model under the full state-machine suite.

## Acceptance gates

- A model test can generate more history than the RAM budget, restart at any
  transaction, and reach every prior state by undo/redo or Restore.
- Current-state text and all five side channels match the unlimited reference
  model after arbitrary edit/undo/redo/restore/save/reopen sequences.
- Resident exact-history memory plateaus within the chosen budget plus two
  documented chunks and transient codec buffers.
- Warm-boundary undo fits the frame budget on release hardware; cold misses are
  measured, asynchronous, prefetched in normal use, and never interpreted as
  “no more undo.”
- History-strip construction is bounded by summaries, not total exact-history
  bytes, while retaining the current visual/time semantics.
- Every committed generation survives truncation after each individual write
  and sync point; no test yields a newer document paired with older history or
  vice versa.
