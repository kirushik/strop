# The keyboard map earns a room of its own

Adjudicated 2026-07-17 (four modeled users + Raskin/Tufte/Norman
panel, run sol-hotkeys; operator's separate-window instinct
CONFIRMED against the overlay-redesign and pinned-panel
alternatives). The overlay dies: it covers the sovereign prose (P1)
and vanishes on the next click — the novice's actual need is "a
card I keep beside my work."

## The window

- ONE modeless auxiliary OS window, title "Keyboard map", owned by
  the document process; contains NO Editor, Store, save state, or
  InstanceGuard (single-instance law is store ownership, not window
  count).
- ctrl-?, the Help row, and the palette action share one
  ensure/toggle path: absent → open; visible while editor-focused →
  bring/focus; reference-focused → close and restore the
  originating editor focus/caret. Esc in the reference = close +
  restore. Native close = the same. Never a duplicate.
- Closing the editor keeps the durable-save veto; a successful quit
  takes the reference with the process. The reference itself never
  triggers the quit preflight and never survives as an orphan.
- Default ~900×560 clamped to the work area, placed beside the
  editor when space permits. Bounds persist across sessions in
  their OWN record (never the editor's window.json), clamped fully
  on-screen after monitor changes. NOT always-on-top; ordinary
  stacking honors the desktop contract (an explicit pin control is
  a future, P12-stateful addition only if corridor evidence demands
  it).

## The sheet

- Content generated from commands::all() on every open — no copied
  list. The hardcoded "Text editing" baseline moves to one owned
  provider with tests (it may stay non-command data; it may not
  stay an inline literal).
- ≥780px: three equal columns of INTACT section blocks in registry
  order (deterministic shortest-column packing; a section never
  splits). 560–779: two columns. Below: one. Title/header band
  fixed; only the grid scrolls, and only when the measured content
  truly cannot fit — at the default size all rows fit with NO
  scrollbar, enforced by a test that FAILS when registry growth
  breaks the budget (then we revisit architecture, never shrink
  below 11px).
- Type: actions 11.5–12px one line, chords 11px right-aligned
  tabular, headings 10px uppercase, 15–16px row pitch, 20–24px
  gutters. Neutral paper/panel palette, near-black ink, taupe
  rules; NO warm/cool hues (the reference is neither writer nor
  machine voice); the title is the single high-contrast anchor
  (P11). No entrance animation, no pulse (attention-motion).
- Chord column labeled "Physical keys": Latin physical legends on
  every layout, never active-layout characters. Command labels stay
  English until app-wide RU strings; a future localization adapter
  maps display labels without touching registry identity, chords,
  or sections. The GPUI Windows/macOS non-Latin letter-chord gap
  stays an open platform risk the map exposes but cannot repair.

## Tests (from the RFC's plan)

Pure model: every registry command exactly once; counts and section
order deterministic; None-chord rows show the palette marker; no
silent truncation. Layout matrix: 900×560 three-col no-scroll,
two/one-col breakpoints, short-screen header-fixed scroll, intact
sections, min font, bounds clamping. Controller: single reuse,
toggle semantics, focus restoration, editor-close veto leaves both
windows until resolved, quit closes both, reference close never
saves/quits, independent bounds. Rig: stills at default/narrow/
short; two-surface capture support. Manual per-OS close/activation
checks remain listed risks.
