# Impl spec 03 — the selection flanks

*(Design docs: `docs/asides.md` §4, lab v4 scene 2. Status: SPEC —
pre-review draft.)*

## 0. What exists vs what changes

`render_selection_popover` already implements the coordinate math
(frame `cursor_position` + CSD inset correction) and a gutter-float
vertical toolbar (`format_tools`). The upgrade:

1. **Left flank** becomes the closed-set grid: inline attrs in 2
   columns — B I / U S̶ / highlight code — a hairline seam — link †
   (the two argument-takers) — a second seam — H1 H2 H3 (block row).
   Reuses `format_button`/`heading_button`/`footnote_button`; adds the
   missing **link button** (argument-taker: commits a URL via a small
   `TextField::single`, the rename-input idiom). Grid, not stack:
   halves travel (Fitts), and the seam separates toggle-grammar from
   input-grammar (P8).
2. **Right flank** is new: the selection MENU in the right margin at
   the selection's height (the lane side — where its results land),
   idle at α≈0.72, waking on hover; rows as carrier sentences,
   **mouse-only in v1** (reviews B2/B9: bare-letter hints would type
   over the live selection — no letter bindings, no key caps, matching
   the editor-button menu):
   - `✎ Add a note` → existing margin-note composer path
   - `☰ Set aside, out of the story` → spec 02 §2
   - `✂ Send to the graveyard` → spec 02 §4
   - `❋ Ask the editor about this…` → selection-scoped ask (the
     existing scoped-prompt machinery; v1 runs the default read over
     the selection span)
   Icons drawn as divs (no non-PT glyphs). Acting on ANY row first
   dismisses both flanks (review H20), so only one pinned object ever
   sits at that lane y.
3. Both flanks rise together on selection (mouse-up path already
   raises the popover) and share material, elevation, motion, and the
   same vertical origin — balance, not mirror-symmetry.

## 1. Geometry & collision rules

- Left flank in the reserved gutter between compost rail and prose
  (asides.md §4): when the rail is open, the gutter is the space
  `rail_right..col_left`; flank never overlays the rail (P1). When the
  window is too narrow for the gutter (existing `left_gutter >= 58`
  check), fall back to the existing horizontal popover with the
  FORMATTING SET ONLY — the verb rows stay reachable via the palette
  (review H21: four carrier sentences don't fit precisely where the
  window is too narrow).
- Right flank renders as an independent overlay that OCCLUDES cards at
  its y — it is transient, and the packer is untouched in wave 1
  (review B8: the pin-injection claim was unbacked by the code).
- Neither flank rises while any history surface (strip, panel,
  preview) is up (review H22).
- Selection inside the COMPOST rail raises the left flank at the
  rail's right edge (same gutter), and the right menu shows only
  `Add a note`-free rows that make sense there (no aside-from-aside;
  graveyard send allowed? NO — compost deletions don't file (spec 02
  §4); the menu shows formatting-only actions → v1: right flank
  simply doesn't rise for rail selections).

## 2. Dismissal & lifecycle

Existing rules carry: mousedown elsewhere, scroll, typing, Escape all
dismiss; palette/settings suppress. New: exiling the selection
dismisses both flanks (the selection is gone); aside likewise.

## 3. Tests & rig

- Popover coordinate tests already exist for CSD insets — extend for
  the grid variant and the right-menu lane pin.
- Packer test: menu-pin + active card + crowded lane → no overlap,
  menu at selection y.
- Narrow fallback: both flank contents reachable.
- Rig: `dump:ui` gains `flanks: {left, right, y}`; smoke `select:para`
  token to select the caret paragraph deterministically.
