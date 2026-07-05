# Impl spec 05 — the cold read (DEFERRED: stretch package)

*(Design docs: golden-path D3 + §9.2–9.3, lab scenes. Status: SPEC'd
for a later package — the recon verdict makes this the one feature
needing genuinely new layout machinery, so it ships after the
journal/strip/asides wave rather than half-baked inside it.)*

## 0. The recon verdict (2026-07-05)

gpui has **no justify** (`TextAlign` = Left/Center/Right only) and
**no hyphenation**; but `shape_line` + measured widths + paint-at-
arbitrary-origin are all available and already exercised by strop's
footnote-superscript code. So the book page is buildable **entirely
strop-side**:

1. `hyphenation` crate (v0.8.4, code MIT/Apache — passes deny as
   configured; verify `hyphenation_commons`' declared license before
   merge; load the ru dictionary at runtime for the LPPL/GPL posture,
   per ux-glossary appendix) → in-word break opportunities.
2. Per paragraph: measure words via `shape_line().width()`, greedy
   line-breaker (Knuth–Plass later if rivers offend), slack
   distributed across gaps, each word painted as its own `ShapedLine`.
3. New layout cache layer (the per-block `shape_text` cache doesn't
   apply); hit-testing is read-only-mode simple (no caret), reactions
   select by word-box.

## 1. Scope when picked up

- Read-only paged view (static flippable numbered pages, bookish
  face, texture), banner per lab v4 (data-only strings; typing pulses
  the Reading chip).
- Entry auto-creates the reflex checkpoint; reactions (? ! ~ + short
  note) file as margin notes anchored by content.
- The history-preview variant (paged view of a checkpoint state +
  Restore chip) rides the same renderer.
- Footnote placement on pages = its own research item (G1) — v1 pages
  render footnote refs, definitions stay off-page.

## 2. Acceptance

Corridor floor: it looks like a book; page-flip by click/arrow keys;
Esc returns to the desk. Rig: `coldread:open`, page-count and
reaction-note assertions; wshot golden of a rendered page.
