# Strop Performance Work (2026-06-16)

Started from a 14-agent audit (workflow `strop-perf-audit`), adversarially
verified against GPUI's actual caching behavior. This doc is the durable record:
the baseline, the load-bearing correction, the plan, and the progress log.

## Goal

User-perceived performance first — keystroke→paint latency, scroll/typing
stutter, idle jank, visible resource spikes. Lower steady-state RAM/CPU second.
Target confirmed by Kirill: must stay smooth on **novel-length single-file
manuscripts (50k–100k+ words)** AND stay zero-regression on short pieces.

## The load-bearing correction

The audit's first instinct — "prepaint re-shapes the whole document every
frame" — is only half true, and the false half matters. GPUI's `LineLayoutCache`
(`gpui/src/text_system/line_layout.rs:497`) is a two-frame cache. Because
`prepaint` touches every line every frame, lines stay warm, so the expensive
swash/cosmic-text **glyph shaping is a cache hit in steady state** — it is NOT
recomputed. What *is* redone every frame is the O(N) bookkeeping around the
cache: `doc.text()` whole-doc `String`, a fresh per-paragraph `String` + full
hash for the cache key, `kinds.to_vec()`, and the O(blocks × spans) nested
filter (`editor.rs:6271-6436`). The one time shaping really is O(N) is **frame 1
on cold open**, when the cache is empty.

## Baseline (measured)

Fixture: 52k words / ~918 blocks (`scripts/gen-large-fixture.py` → `/tmp/strop-large.md`).
Driven headless via `scripts/wrun.sh` with `STROP_PERF=1`.

| Scenario | prepaint time | notes |
|---|---|---|
| Cold-open first frame | **163 ms** | cache-cold; shaping genuinely O(N). Hits "<1s to document". |
| Warm steady-state (type/scroll/blink) | **3–5 ms / frame** | pure bookkeeping; ~25% of a 16 ms budget. ~2× on a 100k-word novel. |

This validates the plan: the warm-frame target is to cut bookkeeping toward
<1 ms (Waves 1–2 + culling); the cold-open target is viewport-only shaping
(Wave 3) so frame 1 shapes ~tens of blocks, not 918.

## Plan (waves)

- **Wave 0 — Measure.** Fixture + `STROP_PERF` baseline. ✅ done.
- **Wave 1 — Safe engine fruit** (strop-core, behavior-neutral, test-covered).
- **Wave 2 — Keystroke/scroll memoizations** (cut idle+scroll cost before the
  big change).
- **Wave 3 — The keystone:** split `prepaint` into a cheap O(N) geometry pass
  (per-block height cache) + an O(viewport+overscan+cursor) heavy pass. Collapses
  scroll jank, cold-open, resize reshape, blink and caret-move cost, and
  `last_frame` RAM at once.
- **Wave 4 — Blink/caret cleanup** (mostly self-resolves after Wave 3).
- **Wave 5 — Cold-open + save off-thread** (each justified by a measured number).

## Progress log

### Wave 1 — engine fruit (strop-core) — DONE
- **Snapshot reorder** (`document.rs` `edit_bytes`/`edit_bytes_coalescing`): take
  the full `SpanSet+BlockMap+Annotations` clone only when a transaction actually
  opens, instead of unconditionally per keystroke. The buffer edit doesn't touch
  spans/blocks/notes, so the post-edit-pre-absorb snapshot is the identical
  pre-edit side-state — behavior-neutral. Skips the clone on the ~5/6 mid-word
  keystrokes that used to allocate-and-drop it.
- **`count_line_breaks` fast-path** (`document.rs`): scan the (short) inserted
  text for a Unicode line-break char before building a throwaway `Rope`; the
  common keystroke inserts none and returns 0. Preserves ropey's exact
  CRLF-as-one / NEL / LS / PS counting via the fallback.
- **Deliberately skipped:** removing the redundant `sort_by_key` in
  `SpanSet/Annotations::apply_op`. Audit rated the payoff "near-none"; not worth
  touching order-sensitive CRDT code unsupervised.
- Verified: `cargo test -p strop-core` green (99 tests, incl. the proptest
  state machine).

### Wave 2 — view-tree / chrome dedup (strop-app) — DONE
- **`has_margin_cards()`**: cheap emptiness predicate behind `lane_has_content`.
  `column_frame` is called ~9× per render and each used to *build + position +
  height-estimate* every margin card just to test whether the lane is occupied;
  now it asks the filter-only predicate. The full `margin_cards()` build remains
  only where cards actually render (once).
- **`visible_footnotes()` early-out**: return immediately when the doc has no
  `FootnoteRef` spans, skipping the O(paragraphs) viewport scan + per-ref
  O(blocks) def search that ran every render (incl. idle blinks) even with zero
  footnotes. Behaviour-identical (the full path also returns `(vec![], 0)` then).
- Render-side, so `STROP_PERF` (prepaint timer) doesn't reflect them; they cut
  chrome cost on note/footnote-heavy docs. Verified: builds, no panic, prepaint
  unchanged via the headless rig (incl. `seed:diag`).

### Wave 3 — the keystone: layout reuse — DONE
`prepaint` is no longer O(document) per frame. Two tiers of reuse:

- **All-or-nothing frame reuse** (`LayoutKey` + `Document::revision`): when nothing
  layout-affecting changed since the last paint — scroll, cursor blink, or a
  collapsed-caret move — carry the previous frame's paragraphs forward verbatim
  and recompute only the scroll clamp and caret quad. The key captures revision
  (text/spans/blocks/notes), wrap width, font scale, *non-empty* selection (an
  empty one paints no highlight, so a caret move is a no-op), marked range, find
  query and active note; history preview and image blocks opt out (preview text
  and async image decode aren't captured by the revision). Measured on the 918-
  block fixture: **3–5 ms → 7–35 µs** (~500×).

- **Per-block shaped-line reuse** on full rebuilds (edits/format/selection): each
  block keeps its already-shaped `WrappedLine` from the previous frame when its
  `(text, runs, size, indent)` — exactly the shape key — is unchanged; only the
  blocks that actually changed are re-shaped. Matched by index (a split/merge
  shifts indices and re-shapes from the edit down).
  - *Why it's load-bearing:* the all-or-nothing reuse stops feeding GPUI's own
    two-frame `LineLayoutCache`, so it goes cold during a run of reuse frames.
    Without per-block reuse, the first edit after any scroll/pause/caret-move
    re-shaped the whole document — **measured 120 ms**, *worse* than the 3–5 ms
    baseline, a nasty resume-typing hitch. Per-block reuse makes rebuilds
    independent of GPUI's cache: edits are **2–4.5 ms** even with a cold cache.

**Net:** navigation/idle is microseconds, editing is low-single-digit ms; only
the cold-open first frame is still a full O(N) shape (**151 ms** — no previous
frame to reuse; a Wave-5 / async-first-paint target).

**Foundations & invariants:** `Document::revision` is a monotonic counter bumped
in all 16 `&mut` mutators, never serialized (over-bump = wasted rebuild; a missed
bump would risk a stale frame — test
`revision_bumps_on_every_layout_mutation_and_is_stable_otherwise`). The per-block
match key is the *complete* shape key, so a reused line is byte-identical to a
freshly shaped one.

**Verified:** full workspace suite green; headless pixel-diff vs `main`
(`scripts/wshot.sh`, `STROP_TEST_STILL=1`): **0 differing pixels** for pure
scroll, and **0 in the document body** for edits (the only delta was the titlebar
filename across the two capture files). Timings via `STROP_PERF` on the headless
rig.

### Wave 5 — cold-open + save — MEASURED, deferred to supervised work
Both remaining big items touch correctness-sensitive paths (persistence
atomicity; startup sequencing), so they were measured but not changed unsupervised.

- **Autosave stall (`save_now`, now `STROP_PERF`-timed):** measured **8 ms** on
  the 52k-word fixture with a shallow history — noticeable (½ a frame) but not a
  freeze. It grows with session length: `save_with_state` re-serializes
  annotations + block kinds + `export_history(200)` (**up to 400 full
  `SpanSet+BlockMap+Annotations` snapshots**) to JSON into Loro and re-runs
  `rebuild_marks` every save, then exports a full `ExportMode::Snapshot` and
  writes it — all on the UI thread (`store.rs:372-412`, `:501`). The safe wins
  (snapshot the export bytes on the UI thread then hand `fs::write` to the
  background executor; per-subsystem dirty flags so unchanged marks/history/
  blocks aren't re-serialized) need review because a wrong dirty flag silently
  fails to persist. Deferred.
- **Cold-open first frame (151 ms):** the one frame with no previous layout to
  reuse, so it does a full O(N) cold shape of every block. Cutting it needs
  viewport-only first-paint (defer off-screen shaping) — a bigger, higher-risk
  change than the reuse path. Deferred.

### Deferred (low value / need review)
- Drop the per-keystroke `store.apply` Loro `commit()` (Q): audit verdict
  "overstated / low value / needs-measurement"; touches CRDT frontier semantics.
- Span-bucketing sweep (O(blocks×spans) → O(blocks+spans) in the full-rebuild
  per-block filter): edits are already 2–4.5 ms post-Wave-3; marginal.
- `find_matches` memo by (query, revision): only active during find; transient.

## Tooling

- Zero-install / wired in repo: `STROP_PERF` (prepaint timer, already present),
  GPUI `FrameTiming` (`set_frame_trace_enabled`), `scripts/wrun.sh` headless rig.
- Installed: `samply` (+ `perf_event_paranoid=1`), `heaptrack` (apt),
  `cargo-flamegraph`. Skipped Tracy (no Ubuntu 26.04 package).

---

## Continuation notes (handoff, 2026-06-16)

Status: Waves 0–3 shipped and verified on branch `perf/audit-2026-06` (5 commits,
not yet merged to `main`). Manual testing felt "delightful" on a fast laptop; the
real signal will come from **corridor testing on less-performant machines**, where
the absolute numbers below scale up (a 3× slower CPU turns the 151 ms cold-open
into ~450 ms and the 8 ms save into ~24 ms — still under budget, but the hitches
become felt). That's the lens for prioritising what's left.

### Discoveries worth not re-learning
1. **GPUI already caches shaping** (two-frame `LineLayoutCache`). The per-frame
   cost was never re-shaping — it was O(N) bookkeeping. Any future "is prepaint
   slow?" question starts from this, not from "it re-shapes everything".
2. **The cold-cache eviction trap.** Any optimisation that *skips* `shape_text`
   on some frames lets GPUI's cache evict in 2 frames, so the next full rebuild is
   cold (measured 120 ms). The per-block shaped-line reuse exists precisely to
   make rebuilds independent of GPUI's cache. **Rule: never skip shaping frames
   without also owning the shaped lines yourself.**
3. **The complete staleness key is `(text, runs, size, indent)`** with width and
   font-scale gated globally. `runs` (from `runs_for_paragraph`) encode every
   styling input — selection, spans, notes, find tint — so comparing runs is a
   total check. This is why per-block reuse is provably safe.
4. `WrappedLine` is **not `Clone`** (it derefs to its inner `Arc`, so `.clone()`
   silently clones the Arc). Move it out of an owned `Option` slot instead.
5. **Autosave rewrites the opened file**, so headless pixel-diff captures must use
   fresh identical fixture copies per run, or the diff is just the accumulated
   text / titlebar filename.

### Opportunities not yet taken (ranked by corridor-test likelihood)
1. **Cold-open first frame (151 ms).** The one frame with no `prev` to reuse → a
   full cold shape of every block. Two ways to cut it: (a) shape only the viewport
   on first paint and shape the rest on a background/idle pass that fills the
   per-block cache (so scroll-down is warm) — cleanest; (b) shape viewport + cheap
   height *estimates* for off-screen blocks, corrected lazily. This is the single
   most likely corridor-test complaint on slow hardware.
2. **Autosave off-thread + dirty flags (`save_now`, 8 ms, grows).** Snapshot the
   `ExportMode::Snapshot` bytes on the UI thread, then hand `fs::write` to
   `background_executor`. Add per-subsystem dirty flags so `rebuild_marks` and the
   annotations/blocks/history JSON aren't re-serialised when unchanged. The
   history JSON is the bloat (`export_history(200)` = up to 400 full snapshots
   re-emitted every save). **Risk: persistence atomicity / data loss — review the
   ordering and the dirty-flag completeness before landing.**
3. **Unbounded undo RAM.** `Document::{undo,redo}_states` and `Buffer::undo_stack`
   are never trimmed in memory (only `export_history` caps the *persisted* tail).
   Over a long session this is O(transactions × doc-structure) of mostly-identical
   clones — matters on low-RAM laptops. Fix: a depth cap matching the persisted
   200, and/or `Arc`/COW snapshots (most transactions don't touch blocks/spans).
4. **Open-path cost before first paint** (mature docs): `read_state` rebuilds the
   `SpanSet` in O(runs²) (`SpanSet::add` retain-scans), and `add_checkpoint_if_changed`
   at startup does ~3 `read_state` + a Loro checkout. Single-pass span build +
   deferring the session-start checkpoint off the critical path.
5. **Marginal / transient:** span-bucketing sweep (O(blocks×spans)→O(blocks+spans)
   in the rebuild filter; edits already 2–4.5 ms); `find_matches` memo by
   `(query, revision)` (allocs ~16×doc bytes per find keystroke, find-only); the
   530 ms blink loop never quiesces (battery; re-arm-on-input is the fiddly part).

### Pending decisions (need Kirill)
- **Merge now or after corridor testing?** Recommendation: merge `perf/audit-2026-06`
  to `main` now — it's verified, pure win, and corridor testers should exercise the
  optimised build. Nothing in it is speculative.
- **Cold-open: acceptable at 151 ms, or invest in progressive first paint?** Decide
  once slow-laptop numbers land.
- **Undo history persistence: keep in Loro (file grows per save) or move to a
  sidecar overwrite?** Trade-off against the "single portable file" property.
- **Which deferred item first** (cold-open vs save off-thread vs RAM caps) — let
  corridor pain rank them.

### For corridor testing
- `STROP_PERF=1` already prints `prepaint …` and `save_now …` per occurrence to
  stderr — collect these from a slow machine.
- Better keystroke-latency metric for corridor runs: wire GPUI's built-in
  `FrameTiming` (`gpui::set_frame_trace_enabled(true)` +
  `FrameTimingCollector::collect_unseen()`); `dirty_to_draw_duration()` is the
  whole keystroke→draw time, not just prepaint. Zero new deps (it's in the pin).
- Repro a heavy doc: `python3 scripts/gen-large-fixture.py /tmp/x.md 100000`.
