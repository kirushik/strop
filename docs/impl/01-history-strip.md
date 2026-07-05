# Impl spec 01 — the history strip

*(Design doc: `docs/history-strip.md` v2. Depends on: spec 00
(journal). Status: SPEC — pre-review draft.)*

## 0. Placement & entry

A bottom strip overlay in the machine-room dark, following the
bottom-strip idiom (`render_composer_strip`/`render_goal_strip`:
`absolute().bottom_0().left_0().right_0().border_t_1()`), height ≈
196px (top row 30 + label lane 22 + fabric 130 + date lane 14).
Chrome colors inline per theme.rs convention (dark ground `0x26251F`,
readout chip `0x111009` — new inline values, machine-room family).

**Entry:** `ctrl-alt-h` (the existing history-toggle) opens the STRIP
(the new first history surface). The right-side history panel remains
reachable via the palette ("History panel") — replacing it outright is
Kirill's standing question, not v1's call. The titlebar clock control
toggles the strip. The strip and the panel never open together.

## 1. The view model (`StripView`)

Built from `(journal, checkpoints)` once per `(doc.revision,
journal.len)` — the bake — and cached; scrubbing NEVER rebuilds it
(the stability law). Contents:

- **Sessions/seams:** runs grouped by >15min gaps; x = working time at
  a fixed quant (`STRIP_PX_PER_SEC` ≈ 1px/30s; seams fold to 10px).
- **Fleck quads:** per run, `ins_words`/`del_words` 2×2 quads jittered
  (seeded hash of run index — deterministic) within the run's x-width
  and y-span; capped at 70/run. Amber `0xC8A951` at α≈0.5 / burnt
  `0x8A6D35` at α≈0.62.
- **The page + envelope:** cumulative length per run → stepwise
  polyline hanging from the rail (y = RAIL + len/maxLen·FABRIC_H, 10%
  headroom); the region rail→envelope fills faint cream (α≈0.13 — the
  corridor fix); the envelope strokes cream 1.3px.
- **Veils:** `Pass` events → full-height translucent cool columns
  (rail→envelope at t), kind label set inside at rest.
- **Threads:** CardRaised→CardClosed at anchor depth; sage/grey
  terminal dots; open threads run to now.
- **Stations:** checkpoints → hairline ticks + ranked-omission labels
  (two rows, computed at bake; near-right-edge labels set to the
  tick's left). Restore events → sage tick + dashed arc to source.
- **Dates lane:** session-start dates thinned once at bake ("Today",
  "Yesterday", "Tue 1 Jul" — real dates, never "day 12").

Rendering: one custom-painted element (paint quads/paths/text runs in
its paint pass, PT Sans for all strip type). No glyphs outside PT
fonts (icons drawn with divs/quads).

## 2. Scrub state & the frame loop

```rust
struct Strip { open: bool, pos_ms: i64, pin_ms: Option<i64>,
               parked: bool, scratch: Option<ScrubDoc> }
```

- Mousedown on the strip = park there; drag = continuous scrub (the
  drag pattern from the existing selection drag; strip stops wheel
  propagation like every panel).
- Per pointer-move: reconstruction via `ReplayDoc` (spec 00 §4 —
  CHAR-indexed replay, never `edit_bytes`; spans/blocks ride the same
  invariant machinery). Rightward drags replay only the delta runs;
  leftward re-anchor.
- The preview renders through the EXISTING `history_preview:
  PreviewDoc` path (checkpoint preview machinery) — the main column
  shows the past read-only; the strip's dim overlay covers x >
  playhead. **The margin lane and rail hide while previewing** (verify
  the checkpoint-preview path already does this; gate on
  `history_preview` if not) — cards anchored to the live document must
  not float over past text (review H36).
- **Stability law in code — bake vs view:** the BAKE (fleck quads,
  envelope, veils, threads, label layout, y-scale) is immutable while
  the strip is open; it does NOT re-bake on background pass arrivals
  or reflex checkpoints (review H35) — the one lawful in-session
  re-bake is the explicit Restore. The paint pass may additionally
  vary only: playhead/thumb x, the fabric VIEW OFFSET (auto-scroll
  keeping the playhead in view at novel scale, wheel pan — review B7),
  dim rect, readout string, station-label brightness, Restore/Now
  visibility. The rig asserts the `bakes` counter, not fleck geometry.
- The readout's word count tokenizes the reconstructed rope (cheap,
  exact); per-run word counts are fabric texture only (review H30).
- Readout: fixed-width chip, tabular numerals, `{date} · {n} words`;
  never a sentence, never a station name (P8 template ban).
- **Now** chip rightmost: dim at now, bright when parked; click or Esc
  returns to now (drops preview, keeps strip open).
- **Restore** appears beside the readout when parked: builds a
  `CheckpointState`-shaped value from the replay doc and routes
  through the EXISTING `restore_state` path (undoable forward edit,
  notes reanchor by content, orphaning rules apply). The restore path
  then materializes an automatic post-restore checkpoint ("Restored")
  — the reconstruction anchor (review B6) — and records
  `Restore{t, from_unix, len_chars}`; the bake refreshes (data
  changed — the one lawful re-layout).
- **Typing while parked** = restore-then-type: a text-insert keystroke
  while previewing performs the Restore first, then inserts. No
  confirmation dialog anywhere.
- **Pin** (shift-click): second faint playhead + delta folded into the
  readout line (`· +612 since Tue 8 Jul`). No advertising chrome.

## 3. Degradation

No journal (old file): fabric absent, envelope from checkpoint states
only, stations/dates still draw, scrub snaps between stations (the
strip is still a seek bar — just coarser). Journal without checkpoint
anchors ahead of it: replay from journal start on empty doc.

## 4. Corner cases (test matrix)

- Scrub during an open composer / mid-typing-burst: opening the strip
  is a discrete act — commit-on-blur fires first (universal rule).
- Restore while a pass is cooking: `ai_generation` staleness guard
  already drops the result; test it.
- Restore at t == now is a no-op (button hidden).
- Empty doc / single-session doc / doc with zero checkpoints.
- Clock skew across sessions (t1 < previous t1 clamped at record).
- A run straddling the playhead applies whole (documented grain).
- Reduce-motion: the strip has no travel animation to reduce; the
  preview swap is instant either way.
- Window narrower than the strip minimum: readout + Now survive,
  fabric clips left (rail stays full-extent per design §1).

## 5. Rig

Smoke tokens: `seed:journal` (deterministic synthetic fortnight),
`strip:open`, `strip:scrub:<0..1>`, `strip:pin:<0..1>`,
`strip:restore`, `strip:now`. Dump gains
`strip: {open, pos_ms, parked, runs, events, stations, words_at,
bakes}` — `bakes` is the session-monotonic bake counter (the
stability-law assertion: scrubbing must not bump it).
