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

### Deferred (need measurement / review)
- Drop the per-keystroke `store.apply` Loro `commit()` (Q): audit verdict
  "overstated / low value / needs-measurement"; touches CRDT frontier semantics.
- Off-thread autosave snapshot export (Wave 5, `store.rs:501`): the "freezes
  when I pause" mechanism, but magnitude is doc-history-dependent — measure first.

## Tooling

- Zero-install / wired in repo: `STROP_PERF` (prepaint timer, already present),
  GPUI `FrameTiming` (`set_frame_trace_enabled`), `scripts/wrun.sh` headless rig.
- Installed: `samply` (+ `perf_event_paranoid=1`), `heaptrack` (apt),
  `cargo-flamegraph`. Skipped Tracy (no Ubuntu 26.04 package).
