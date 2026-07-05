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

Per the original D5 shape (compost head / graveyard tail), REVISED by
the review (B13 + H42 — a new `BlockKind` variant would make older
builds' serde fall back to the token parser and silently reset EVERY
block kind in the file, and `on_edit`'s split-cloning would duplicate
a sentinel kind):

- **The boundary is an out-of-band block index**:
  `BlockMap.aside_boundary: Option<usize>`. Blocks `0..idx` are
  compost; the boundary line itself is a plain empty paragraph in the
  rope. The index is adjusted inside `BlockMap::on_edit` (which
  already sees every block splice) and persisted as its own small key
  beside `kinds` in the blocks map. An older build ignores the key:
  nothing resets, compost renders as leading paragraphs; a round-trip
  through an old build drops the boundary (text preserved, documented).
- **Empty compost = no boundary.** The rail simply doesn't exist
  (asides.md §1: an empty rail is absent). The aside verb creates the
  boundary on first use.
- **Selections never span the boundary** (review B4): drag and
  keyboard selection clamp at the region edge, so every verb's input
  is single-region by construction.
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
  must not delete the boundary line and silently merge compost into
  prose. Rule: the boundary is removed only via the aside machinery
  (empty compost auto-removes it on save; explicit "dissolve" is not a
  v1 verb). The keystroke is a no-op there, like backspace at
  position 0. Forward-delete at the last compost line end likewise.
- Select-all (Ctrl+A) scopes to the REGION the caret is in — the
  writer selecting-all in prose must not nuke the compost (and vice
  versa).
- Enter at compost end must not leak blocks into the manuscript
  (splits stay left of the boundary).
- Cursor motion: the edge is hard INTO the rail, soft OUT of it
  (review B3 — no keyboard traps): arrows never wander from prose into
  the rail, but Down at the last compost line crosses into the
  manuscript, and **Esc from any rail caret returns to the last
  manuscript caret position** (wired into escape_mode).
- Tail appends (aside verb, orphan migration) insert one separator
  blank line first when compost is non-empty, so items never fuse
  (review H23).
- Markdown export skips `0..=boundary`; word counts skip the same;
  AI passes are built from the manuscript slice only.
- The scrub preview (journal spec §4) is text-only and will show
  compost text at the top of past states — accepted v1 fidelity note.

## 2. The aside verb (floor entry)

`Set aside` on a selection — or, with an empty selection, on the
caret's paragraph (review H25) — via selection menu + palette:

1. Ensure the boundary exists (create at 0 if absent).
2. Remove the selection from the prose under an explicit
   **graveyard-suppression guard** (review H41 — a MOVE never files a
   corpse, whatever its size; mirror the journal pause pattern).
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
- If the note is the active card, the migration first resolves/clears
  `CardFocus` via the existing deselect path (review B5 — the FSM
  returns to Idle atomically with the card leaving the margin).
- The removal runs under the graveyard-suppression guard (a move,
  never a cut — review H41), and the append inserts the separator
  blank line first (review H23).
- The words "unanchored/orphaned" appear nowhere; the rail blinks once.
- Diagnoses never move (machine cards are not writer material; a
  dismissed/dangling diagnosis stays in the card lifecycle).

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

**What counts as a cut (the trigger):** a single SELECTION-deletion
op of ≥ 80 chars of prose lands in the graveyard automatically —
deterministic, and the editor still holds the deleted text at that
point (reviews H24 + H43: journal-run coalescing must not decide
this, and a backspace machine-gun never auto-files). `Send to the
graveyard` (selection menu) files any size. Deletions INSIDE compost,
moves (aside/orphan migration — suppression guard), and
undo/redo/restore never file; undo of a cut also removes its entry
(the inverse in the same grammar, P13).

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
`Loaded` **behind its own `SavedHashes` fingerprint channel, seeded at
open** (review B12 — an unguarded blob of verbatim cut text rewriting
per idle save is the 4.8 MB class). Compost needs only the small
boundary-index key beside `kinds`. Also: the compost rail and the
outline panel are mutually exclusive (opening one closes the other —
review H26), and AI passes slice the manuscript, anchor within the
slice, and re-offset ranges by the manuscript base (review H40).

## 6. Rig & tests

- smoke tokens: `seed:aside` (doc with compost+graveyard fixture),
  `aside:selection`, `exile:selection`, `putback:last`; dump gains
  `compost_blocks`, `grave_entries`, `counts:{manuscript_words,…}`.
- Tests: every guard in §1; the cut threshold both sides; put-back
  re-anchoring after surrounding edits; undo-of-cut removes entry;
  export/count/pass exclusion; orphan-note migration (writer note vs
  diagnosis); legacy-file load.
