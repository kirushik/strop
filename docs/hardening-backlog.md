# Hardening backlog

Filed 2026-07-17 from the five-lens review of the sharp-corners
branch (PR #28). These are prevention investments, not defects —
each names the regression class it closes. Work them as their
subsystems come under the knife; none blocks the merge.

## 1. Coordinate-domain newtypes

The rail-invisible-when-floating bug was a coordinate-frame
confusion (content-local vs window-absolute), and the class is
still open: `OffscreenRef.anchor_y`, `MarginCard.top/anchor_y` are
bare `f32` across three different domains, converted by hand at
each use (frame origin added in one place, scroll re-added in
another, CSD insets in a third). Introduce transparent newtypes —
`ContentY`, `ViewportY`, `WindowY`, `ContentWidth` — with explicit
conversions that *require* the frame origin/scroll/inset as
arguments. Apply first to rail marks, margin anchors, and reveal
scrolling; keep raw `f32` only at the GPUI paint boundary. The
compiler then refuses the whole bug class.

## 2. Keymap window lifecycle test matrix

`toggle_decision` has a three-row truth table; the shipped window
has a much larger state space. A GPUI controller matrix: open from
prose / text-field / flank / cold-read focus; assert single window,
raise-on-editor-chord, close-on-reference-chord/Esc/native-close,
and focus restoration to the *originating* handle; editor-close
veto leaves both windows until resolved; successful quit closes
both; reference close never saves. Plus rig scenarios capturing
BOTH surfaces (900×560 three-col no-scroll, 559px two-col, short
height) — needs two-surface capture support in the rig.

## 3. Footnote reserve boundary/property tests

The reserve algorithm converges iteratively (cap 32) but is tested
at one near-full-page example. Table/property tests around page
capacity: reserve exactly fits; exceeds by one line; several notes
share a page; repeated refs consume one note; unreferenced notes
finish last; a note taller than the body budget still makes
progress. Invariants: body bottom ≤ rule y; note block bottom =
page height; ref and note co-located; deterministic; converges
before the cap on adversarial ref distributions.

## 4. StripActionTier — the ladder as a type

The narrow-width degradation test copies the 104/230 thresholds
from production and samples incidental widths. Extract a
`StripActionTier` enum (More / Name / NameAndCompare) chosen by one
pure function; test the exact boundaries (ε below/at/above both
thresholds); render from the enum so impossible combinations cannot
be expressed. Keep slot disjointness as a property over a width
range.

## 5. Three missing rig stills

The newest surfaces have logic tests but no pixel pinning: (a) an
editor page with adjacent, wrapped, and blank-separated list runs
plus trailing prose; (b) a cold-read page showing a reference and
its bottom-set note together; (c) a styled cold-read page combining
highlight, underline, strike, and the diagnosis band. Smoke dumps
assert the geometry; the stills stay for eyes.
