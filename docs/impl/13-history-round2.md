# Impl contract 13 — history strip round two: exits, verbs, reading

Binding spec: `docs/history-strip.md` (round-two amendments: §0.1
round-two table, §1a floor law, §1b well durations, §1e thread hits,
§2 grammar law, §3a hit arbitration, §3b placement + skeletons, §3d
reading room, §3e geometry-owned controls + exit law, §5 round-two
deaths). Design provenance: CTO RFC + Raskin/Birman lenses,
2026-07-16; CEO adjudications recorded in the spec text.

Precondition already on the branch: the stale-preview LayoutKey fix
and the `frame_paras` rig round-trip (7b188c4). Nothing in these
waves may weaken that gate.

## Binding laws (all waves)

- The representation is untouchable: fabric, envelope, flecks,
  thread geometry, quant, sheet/desk/selvage, well widths.
- Scrub stability law: the bake is immutable while the strip is up;
  only open and Restore bump `bakes`. New hitboxes are prepaint
  artifacts, never bake mutations.
- One grammar (§2.4): dashed `inline_action` mark for text actions;
  dark fill for Restore alone; plain for data; the saltire for the
  frame. No new idioms, no cream fills.
- Red line 3 everywhere: no label, coordinate, duration, or mark the
  record doesn't prove. Estimates stay distinguishable.
- Vocabulary law: "version"; no user-visible "checkpoint"/"session"/
  "seal". New user-visible strings in this round: `Now` (skeleton
  stamp; same word as the selvage label), well durations
  (`6 days`/`6 wk`/`3 mo 2 wk` forms).
- P9 hover reveals/brightens, never acts. P2 the tool never wants.
- No inline diff decoration of prose, ever (P1).
- No new persistence formats; no journal/checkpoint schema changes.
- Hand-formatted tree: match local style; NEVER cargo fmt.

## Wave A — exits, the floor, and the geometry-owned bar

Surface: `crates/strop-app/src/editor.rs` (open/close/return ~3200–
3700, Esc ladder ~8975, strip render ~24300–24600, StripElement
prepaint below it), banner render.

1. **Locus capture/restore.** `open_strip` captures scroll_top
   beside the existing `saved_sel`. Every non-Restore departure
   (return-to-now, saltire, ctrl-alt-h toggle, Esc ladder steps,
   panel swap via `enter_history`) restores caret, selection, AND
   captured scroll. Restore does not (§3e: the document changed on
   purpose). Account for window resize while open (clamp).
2. **Return-through-now transition** (§3e law, steps 1–6): one
   shared exit routine with a continuation (stay-open / close /
   panel-swap); ~180 ms present-state beat; cross-fade only under
   `reduce_motion`; must be idempotent and safe under rapid repeated
   commands (a second toggle during the beat completes, never
   re-enters); never calls `strip_bake`.
3. **Floor law** (§1a): one helper names the visible document bottom
   (window height − strip height when open); max_scroll gains the
   clearance so the final baseline sits ≥24 px above the strip's
   border; text clips at the border (no painting under the desk);
   consumers unified: live margin lane, past margin, sidenote
   reveal, selection popovers, caret reveal, page stepping. No-jump:
   opening the strip never moves scroll_top.
4. **The bar** (§3e/§2): readout de-chipped (recessed data, no
   border/hover) at the sheet origin; moment dock (Restore filled +
   Name this version + Compare, dashed) anchored to the parked
   playhead, receding during scrub, settling on release, flip/clamp
   deterministic; Now at the selvage (plain at now, dashed+ink when
   away, clamps to the near viewport edge when the selvage is
   off-view); saltire stays at the frame's top-right; the parked
   banner keeps moment + `Esc returns` + pulse and loses its Restore
   button; `quiet_action` dies in favor of the `inline_action` mark
   (a chrome-sized variant is fine — same dashes, same hover
   semantics); the narrow-width ellipsis fold wears the dashed mark.
   The naming composer (§3c) opens inside the dock.

## Wave B — Compare becomes a reading instrument

Surface: `crates/strop-app/src/editor.rs` compare room (~22230–
22350), `crates/strop-core/src/diff.rs` (read-only reuse; extend
only with pure helpers + tests).

1. **Scroll.** Each column is its own scroll container with its own
   handle (fix the flex-stretch clip); wheel drives the column under
   the pointer; the narrow A/B switch preserves each side's offset;
   active side named by a stronger header rule (the amber selection
   wash on press dies; quotable-press selection behavior stays).
2. **Change gutter.** Paragraph-level alignment of A/B texts —
   prefer `prose_diff_blocks` provenance (old_par/new_par); if
   performance demands, hash-based LCS with a bounded fallback;
   cached per (pin_ms, pos_ms) pair while the pin stands. Permanent
   quiet marks in each column's OUTER gutter: warm bar on changed
   runs (both sides); arrival bar in B + departure notch in A for
   B-only prose; inverse for A-only. No prose decoration. Click a
   mark → both columns scroll to stand the pair abreast; hover
   brightens the mark and its twin, moves nothing. Kill the Changes
   toggle and `structural_diff`'s compare usage (leave the function
   if other callers exist; delete if orphaned).
3. Card quotes under the columns die here (margins arrive in Wave C
   — one wave without them beats shipping the flattened quotes).

## Wave C — the threads' promise: cards in the parked past

Surface: `crates/strop-app/src/editor.rs` (`render_past_margin`
~19930, margin packing seams, thread paint/prepaint), strip.rs
Thread (carry card identity if missing).

1. **Placement law** (§3b): historical anchor → y through the parked
   preview's real layout; rides preview scroll; reuses the existing
   margin packing (measured heights, culling, off-screen treatment)
   read-only; no composer/verbs; click a past card → scroll preview
   to its anchor. The doc-fraction lane dies.
2. **Legacy skeletons** (§3b): where parked t intersects a legacy
   card's proven suffix — current body, normal card form, header
   stamped `Now`, drained/unverified treatment, hollow-origin mark;
   unproven anchor → detached foot stack; unproven relation →
   nothing. No historical date near the body.
3. **Thread hits** (§1e): segments become hit targets over painted
   geometry only; click parks at the exact x, reveals the anchor,
   focus-outlines the card briefly; hover brightens thread + card;
   no drag steal from the lanes.
4. **Compare margins** (§3d): per-side past margins placed by each
   column's own layout and scroll.

## Wave D — the bake's words: labels, Started, wells

Surface: `crates/strop-app/src/strip.rs` (stations ~845–930, wells,
label pass), editor.rs StripElement prepaint (hitboxes beside
`strip_date_hits`).

1. **Exact label/tick targets** (§2.1): shared hit region per
   painted station (label + tick), exact `at_ms` park; modest
   padding, closest-tick arbitration, rank tiebreak; dashed mark on
   labels, hover brightens label+tick; omitted-label ticks keep a
   ≥12×24 target; no fabric snapping; no drag steal.
2. **Automatic Started dies** (§2.1): remove the birth relabel
   branch. ALSO: system-written station names must not leak as
   labels — the import birth checkpoint is stored literally as
   "Started" and tutorial files as "Fresh tutorial"; these are not
   writer names. Filter by the `manual` flag (a writer-typed
   "Started" with manual=true still shows). Verify against the
   import path before trusting the flag.
3. **Well durations** (§1b): wide tier only; `17 h` never (overnight
   mute); `6 days` (2–13 d), `6 wk` (2–8 w), `3 mo 2 wk` beyond;
   baked text items shaped in the label pass; collision priority:
   writer names > bounding dates > duration > reflex labels; hover
   expands to exact bounding timestamps via the date-hover
   machinery; never a control.

## Wave E — the fixture law and the rig

Surface: `crates/strop-app/src/editor.rs` debug seeds, smoke.rs,
`scripts/rig-check.sh`.

1. **`seed:novel`**: one canonical long fixture composing the
   existing seams — 8–12 viewport heights of varied paragraphs with
   a distinctive final paragraph; ≥6 named versions incl. two
   crowded in one label width; one Exported, one Restored; overnight
   + six-week wells; recorded card history (raise/edit/move/resolve/
   detach) + one writer note + one legacy card with proven suffix;
   A/B moments whose diff has: early replacement, middle insertion,
   distant deletion, long unchanged run, final append. Deterministic
   timestamps from the existing now/day arithmetic.
2. **Dump fields**: compare per-side scroll offset + max, gutter
   region count, saved-vs-live scroll after exit, focused past-card
   id, station hit-target count, well duration strings.
3. **Rig assertions** (each on `seed:novel` unless noted):
   - floor: parked + live, last paragraph readable above the strip;
   - exit: close/Now restore pre-open scroll (and `frame_paras`
     round-trip stays green);
   - compare: both extents nonzero, independent; narrow switch keeps
     offsets; gutter regions ≥4 with an unmarked middle;
   - labels: click parks at the exact station timestamp; no
     automatic Started anywhere; fabric click stays continuous;
   - wells: the six-week well is 20 px and carries its datum;
   - cards: past cards track anchor y across two scroll positions;
     thread click parks + focuses; the legacy skeleton is stamped
     `Now`;
   - stability: `bakes == 1` across every interaction above.
4. **Stills**: parked at document end; wide Compare at unequal
   offsets; crowded label row; six-week well; legacy skeleton.

## Acceptance (every wave)

```
TC=$HOME/.rustup/toolchains/1.96.0-x86_64-unknown-linux-gnu/bin
XDG_RUNTIME_DIR=/tmp/strop-runtime RUSTC=$TC/rustc PATH=$TC:$PATH $TC/cargo test --workspace
RUSTC=$TC/rustc PATH=$TC:$PATH $TC/cargo build -p strop-app && XDG_RUNTIME_DIR=/tmp/strop-runtime bash scripts/rig-check.sh
```

Necessary, not sufficient: the CEO takes stills of every changed
surface before merge. Unit tests accompany pure logic (duration
formatting, alignment, inset math, hit arbitration).

Wave order: A ∥ B, merge; then C ∥ D, merge; then E. Workers never
commit; never run cargo fmt; target dirs on the real disk only
(NEVER /tmp); XDG_RUNTIME_DIR=/tmp/strop-runtime for socket tests
and the rig.
