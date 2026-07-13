# Inline images — build plan

*(2026-07-12. Spec: `docs/inline-images.md` — authoritative; this doc
is the architecture reconciliation and phase plan. Produced by a
four-subsystem code reconciliation + architect synthesis, with the
synthesis's rulings re-verified against the gpui fork checkout.)*

## Rulings (cross-subsystem seams, decided)

- **R1 — The clamp lives in `Document`, not `BlockMap`.**
  `BlockMap::on_edit` receives counts after the rope has spliced; it
  structurally cannot clamp. `BlockKind::is_furniture()` and the
  furniture split law live in `BlockMap`; the range decomposition
  lives in the `Document::edit_bytes` path, planned by a pure
  `clamp_plan(...)` so the geometry is headlessly testable. The
  spec's "model backstop" names Document's edit path as the
  backstop's home; `on_edit` stays a dumb splice plus the split law.
- **R2 — Gesture layer and backstop never both act.** Doors resolve
  to stage/refuse/exile before any edit is issued; the clamp is
  exercised only by edits that bypass gestures (spanning
  `replace_text_in_range`, paste-over-range, scripted edits).
  Gesture tests assert no model call; clamp tests drive `edit_bytes`
  directly.
- **R3 — Picture selection is its own state, not a `selected_range`
  costume.** One `Option<ImageSel>` field, `ImageSel { block,
  door_caret: Option<usize>, revision }`. Entering collapses
  `selected_range`; the flank/omnibar/seam surfaces never see it.
  `door_caret: Some` iff keyboard-born (§12 origin memory). Staged
  exile is `ImageSel` reached through a door — not a second state.
- **R4 — Transactions: reuse the four existing atoms.** Insert =
  `insert_image_block` (unchanged); exile = `exile_to_graveyard`
  whole-block (GC already keeps graveyard assets); replace-in-place
  = the `set_block_kind` snapshot path swapping `src` only;
  migration = load-time normalization like compaction, never
  journaled, never undoable.
- **R5 — Autorepeat freshness is editor state.** Capture-phase
  `on_key_down` (pattern at editor.rs:12998) records the key event
  before action dispatch (gpui actions don't carry it). Law
  implemented as key-up bookkeeping: completion refused until a
  key-up of the staging key was observed since the stage. Do NOT
  trust `is_held` — it lies on X11.
- **R6 — The split law is the model's; the editor's kind-stamping
  special case dies.** The editor keeps only Enter's direction
  rules (§6); it never stamps kinds around a split, or the layers
  fight over the tail block.

## Adjudicated pushback (spec already amended to match)

1. Linux clipboard is text-only cross-app (Wayland text MIMEs; X11
   `set_text`), and one-process-per-document makes cross-document
   paste external — pixels don't travel off-document on Linux. Ship
   the two-entry write anyway; the Markdown line is the spec's
   stated floor; fork patch = named follow-up. (Spec §9.)
2. Wire compat: writers keep emitting `"caption":""` — a missing
   field errors strict serde in released builds and falls back to
   the token parser, collapsing the whole BlockMap. Runtime enum
   still becomes `Image { src, alt }` behind a serde mirror.
   (Spec §10.)
3. Migration never touches history/checkpoint states; live doc
   only; restore re-runs migration. (Spec §10.)
4. Drop-gap indicator is a progressive layer; pointer-targeted
   landing is unconditional. (Spec §7.)
5. Clamp corner ruled: a range covering a furniture block whole
   PLUS partial flanks decomposes into left-partial + whole-cover +
   right-partial, one transaction. Property-test it.
6. Caption face reuses cold-read tokens (~0.8× muted italic,
   centered); centering costs ~15 contained lines in the two x
   translators (`x_for`/`index_at`) — verified worth it; no new
   font family this round.

## The refactor, shaped

**strop-core / document.rs**
- `BlockKind::Image { src, alt }` (caption field dies at runtime).
- `impl BlockKind { pub fn is_furniture(&self) -> bool }` —
  `matches!(Image{..} | Divider)`; mirrors the `expands()` class-
  method precedent. Every wall law reads this predicate, nowhere
  else.
- `BlockMap::on_edit`: when the split-source block is furniture,
  inserted fragments are born `Paragraph`. Merge arm untouched
  (prevented upstream per R1).
- `Document::edit_bytes` family: the clamp. Pure planner
  `clamp_plan(rope, blocks, range) -> ClampPlan` (sub-ranges +
  whole-cover flags); decomposition executes as grouped sub-edits
  in one transaction (`evaporate_scraps_in_tx` is the pattern).
- `Document::replace_image_src(block, src)` — snapshot path,
  preserves alt.
- Serde mirror (accepts + emits vestigial `caption`), and
  `migrate_image_captions()` at the store's open path.
- Watch: coalescing runs break at a wall crossing (correct — a wall
  IS a run boundary — but eyeball the settle/re-arm interplay);
  IME `replace_and_mark_text_in_range` spanning a wall gets a debug
  assertion (likely unreachable).

**strop-core / markdown.rs**
- Export `![alt](src "caption")`, caption = line text with spans
  flattened to inline syntax, soft breaks → spaces, empty caption →
  no title. Import re-parses the title. One shared parser
  `parse_image_line(&str) -> Option<ImageLine>` used by import AND
  paste precedence.

**strop-app / editor.rs**
- `ImageSel` as above, with a revision guard: the state decays on
  unseen document mutations; every legitimate in-state mutation
  (replace-in-place, alt commit) re-stamps revision through one
  funnel; `debug_assert!(kind is Image)` at every resolve is the
  tripwire.
- Doors slot into the existing backspace/delete guard blocks
  (editor.rs:6528/6556); word-delete variants share the guards.
- Typing while selected: click-born → caption end; keyboard-born →
  door caret. Enter: below on selection; above at non-empty caption
  start; below in empty caption. No caret while selected: suppress
  blink/IME/input-handler.
- Copy: `ClipboardItem { entries: [String(md_line), Image(bitmap)] }`
  (field is public). Paste: precedence via `parse_image_line`;
  existing `ClipboardEntry::Image` arm becomes foreign-clipboard
  fallback. Note X11 reads image atoms first — a foreign text+bitmap
  offer arrives image-only; self-copies are immune (cached item).
- Drop: `FileDropEvent::Pending` arrives as synthesized MouseMove
  with `cx.active_drag` carrying `ExternalPaths` (gpui
  window.rs:4602); paint the gap rule from it; `Submit` carries the
  drop position; caret untouched. Drop/paste onto selection →
  import then `replace_image_src`, re-checking `ImageSel{block,
  revision}` on the UI thread after the async import (the writer
  may have deselected).
- Paint: caption lays out BELOW the pixels (the typover dies at
  editor.rs:12310's overlay); empty-slot click band one caret-height
  under the pixels; 2/3-viewport proportional cap — the prepaint
  layout-reuse key is width-keyed today and must grow viewport
  height, or vertical resizes show stale image sizes
  (editor.rs:11530-11606, 11995-12007); wash covers the whole
  block; cursor-into-view accounts for picture height;
  per-visual-line center shift in `x_for`/`index_at` for wrapped
  centered captions.
- Alt strip: shown while selected, clickable = the same edit state.
- Cold read: every consumer of the caption FIELD switches to the
  caption LINE; audit `PageItem` anchor consumers — caption lines
  gaining real anchors changes what cold-read selection can target.

**strop-app / smoke.rs** — `seed:image`.

## Phases

W1 (parallel worktrees) → merge → W2 (sequential) → W3.

1. **THE WALL** (strop-core): `is_furniture`, split law, clamp
   planner + decomposition, corner rule. Gate: property tests —
   no edit sequence clones furniture, none fuses flowing↔furniture,
   partial ranges clamp (both blocks stand, separator survives),
   whole-cover takes the block, divider inherits everything;
   existing suite green. Adversarial table tests enumerate the §13
   panel cases as rows (the planner must not be its own oracle).
2. **THE CAPTION COMES HOME** (strop-core + mechanical editor
   fixes): two-field Image, serde mirror, open-time migration
   (indices resolved at apply time, not save time), markdown
   title round-trip incl. `]`/quote/newline torture (extend
   markdown.rs:651+ and store.rs:1744+ tests), `parse_image_line`,
   `replace_image_src`. Gate: legacy JSON with caption (incl. `]`
   and newline metadata) migrates once at open; Strop↔Strop
   markdown round-trips whole; empty caption emits no title.
3. **UNDER, NEVER ON** (editor paint): caption below, click band,
   margin-borrowing manifestation (bump the block's bottom margin
   once if it's under one caret-height), size cap + layout-key
   growth, into-view, cold-read parity. Gate: rig stills.
4. **STANDING AND LEAVING** (editor state): ImageSel + doors +
   freshness + decay + exile + Put back + alt strip. Gate: the
   field repro script; held-Backspace stops at the stage; Esc
   returns the door caret; refused stills identical.
5. **ARRIVALS AND DEPARTURES** (editor travel): drop + gap rule,
   paste precedence, copy/cut entries, replace-in-place,
   range-copy markdown form. Gate: precedence ladder (asset hash →
   sibling bitmap → literal visible line); foreign bitmap → §5b;
   replace-in-place one undo step.
6. **THE ACCEPTANCE SCRIPT**: seed:image, full §11 script under
   the rig, `document-model.md` §2/§6 amendments, named-cuts
   record. Gate: everything green end-to-end.

## Named follow-ups (not this round)

- gpui fork clipboard patch (image MIME cross-app on Linux); ride
  the pending fork-push ceremony with the macOS fixes.
- Store asset pixel dimensions at import to kill the 56px
  placeholder→decoded height wobble (rendering risk note).
- Divider gesture parity; drag-to-move; resize verb; private
  multi-image clipboard flavour (spec §12's named cuts).

House rules: hand-formatted tree — NEVER `cargo fmt`; commits
`--no-gpg-sign`; unit tests in-module under `#[cfg(test)]`; snap
cargo fails in sandbox — use the ~/.rustup 1.96.0 toolchain with
RUSTC set.
