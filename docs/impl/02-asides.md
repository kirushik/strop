# Impl spec 02 — asides: the compost rail and the graveyard

*(Design doc: `docs/asides.md`. Constitution: P1–P13. Status: SPEC —
pre-review draft; UI-layer hook points to be confirmed against the
app recon.)*

## 0. The two piles get two different representations

The design doc's table (§0) already implies the split:

- **Compost is editable writer text** → it must be REAL rope text:
  caret, undo, formatting, journal coverage all come free (P3), and
  the folk practice ("notes at the top of the file") is literally
  honored.
- **The graveyard is a read-only record** → it is NOT rope text. It is
  a side structure like `Annotations`, serialized in the `.strop`,
  rendered at the document's tail. This kills an entire class of
  region-editing edge cases (nothing protects read-only rope ranges,
  because there are none) and makes "the automatic pile" exactly what
  it claims: a record.

## 1. Compost: a head region behind a sentinel block

Per the original D5 shape (compost head / graveyard tail):

- New `BlockKind::AsideBoundary` — a sentinel block, one per document
  at most. Blocks BEFORE it are compost; everything after is
  manuscript. No parallel region vector, no new BlockMap invariant:
  the boundary is just a block, so every existing splice path keeps it
  aligned for free.
- **Empty compost = no sentinel.** The rail simply doesn't exist
  (asides.md §1: an empty rail is absent). The aside verb creates the
  sentinel on first use.
- The rail column renders blocks `0..boundary`; the main column
  renders `boundary+1..`. One document, one caret, one undo history,
  one journal. Clicking in the rail moves the caret there; the same
  formatting commands apply (P3).
- Separators: within compost, an empty paragraph renders at full line
  height with the hairline at its midline (asides.md §1) — a pure
  rendering rule on empty compost blocks; the text model is untouched.
- The tail anchor mark (P11) renders after the last compost block; a
  click there places the caret at the region's end.

### Guards (the corner cases that must be in tests)

- **Backspace at manuscript start** (caret just after the boundary)
  must not delete the sentinel and silently merge compost into prose.
  Rule: the boundary is deleted only when BOTH sides agree — i.e. only
  via the aside machinery (empty compost auto-removes its sentinel on
  save; explicit "dissolve" is not a v1 verb). The keystroke is a
  no-op there, like backspace at position 0.
- Select-all (Ctrl+A) scopes to the REGION the caret is in — the
  writer selecting-all in prose must not nuke the compost (and vice
  versa).
- Enter at compost end must not leak blocks into the manuscript
  (splits stay left of the boundary).
- Cursor motion across the boundary: Up/Down and Home/End treat the
  boundary as a hard edge (arrow keys don't wander from prose into
  the rail; the rail is entered by click or by the aside verb).
- Markdown export skips `0..=boundary`; word counts skip the same;
  AI passes are built from the manuscript slice only.
- The scrub preview (journal spec §4) is text-only and will show
  compost text at the top of past states — accepted v1 fidelity note.

## 2. The aside verb (floor entry)

`Set aside` on a selection (selection menu + palette):

1. Ensure the sentinel exists (create at 0 if absent).
2. Remove the selection from the prose (an ordinary journaled edit —
   NOT a graveyard cut: the trigger rule is deletion-not-departure,
   asides.md §0).
3. Append the text as new block(s) at the compost tail; blank lines
   inside the selection honestly become item boundaries.
4. The rail edge blinks once (same grammar as the graveyard bar);
   caret STAYS in the prose at the collapse point — the writer parked
   a thought, she didn't travel.

The parking notch (one held key to the tail and back, caret restored)
is spec'd in asides.md §2.1 and ships with the same command
infrastructure; Esc returns.

## 3. Orphaned margin notes → compost

`Annotations::reanchor` already computes `orphaned: true` on restore;
ordinary editing can also fully delete an anchor (range collapses).
New behavior: when a WRITER note (never a diagnosis) loses its anchor,
it leaves the margin and lands at the compost tail as text:

- One quoted line in the margin-note anchor typography (the block gets
  `BlockKind::Blockquote`; the note body follows as a plain paragraph).
- The words "unanchored/orphaned" appear nowhere; the rail blinks once.
- Diagnoses never move (machine cards are not writer material; a
  dismissed/danging diagnosis stays in the card lifecycle).

## 4. The graveyard record

```rust
pub struct GraveEntry {
    pub id: u64,
    pub text: String,        // the cut prose, verbatim
    pub origin_quote: String,// trailing fragment of the paragraph before the cut
    pub origin_pos: usize,   // best-effort char offset in today's doc (re-anchored like notes)
    pub cut_unix: i64,
    pub words: u32,
}
pub struct Graveyard { entries: Vec<GraveEntry>, next_id: u64 }
```

**What counts as a cut (the trigger):** a single deletion of ≥ 80
chars of prose (roughly a sentence) — one op or one coalesced run —
lands in the graveyard automatically. Smaller deletions are typing,
not cuts; the threshold is a named constant, reviewed in the corner-
case pass. `Send to the graveyard` (selection menu) files any size.
Deletions INSIDE compost or by undo/redo/restore never file (undo of
a cut also removes its entry — the inverse in the same grammar, P13).

- **Put back** inserts `text` at the re-anchored `origin_pos` (or at
  the nearest paragraph boundary), removes the entry, flashes the
  paragraph — one verb, both the entry button and the post-cut footer
  affordance (P8).
- **Delete** removes the entry (the journal still has the record).
- Entries render at the document tail (read-only, dimmed) under the
  sticky footer bar exactly per the lab: bar = "⚰ Graveyard · N",
  blinks + ticks on arrival, unsticks into the section header at
  scroll-end (screenshot-true).

## 5. Persistence

`Graveyard` serializes beside `Annotations` in `save_with_state`/
`Loaded`; absent on old files → default. Compost needs NO new
persistence (it is text + one block kind; the legacy token reader
gains an `aside` token).

## 6. Rig & tests

- smoke tokens: `seed:aside` (doc with compost+graveyard fixture),
  `aside:selection`, `exile:selection`, `putback:last`; dump gains
  `compost_blocks`, `grave_entries`, `counts:{manuscript_words,…}`.
- Tests: every guard in §1; the cut threshold both sides; put-back
  re-anchoring after surrounding edits; undo-of-cut removes entry;
  export/count/pass exclusion; orphan-note migration (writer note vs
  diagnosis); legacy-file load.
