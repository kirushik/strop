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
