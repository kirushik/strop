# The compost, re-grounded — design review (DRAFT, awaiting the writer's verdict)

2026-07-05. Trigger: the writer's review — "it's not clear how I can
type an item into the compost; it's sorta both in the left rail and
on top of everything simultaneously? Shouldn't it better be just a
free-flow section?" This is a design correction (Gate 0): analysis →
critique → intent echo → only then code. Nothing below is built yet.

## 1 · The flows the compost must serve

- **F1 Park.** Mid-draft, a passage stops belonging. One gesture, no
  travel, visible compliance, reversible. (Works today.)
- **F2 Jot.** A thought that was never manuscript — premises, endings,
  beats, a phrase to keep. Typed *into the compost directly*, as
  cheaply as parking. The lab called this "typing straight into the
  rail is the notch".
- **F3 Skim & return.** While revising: read the scraps, then return
  to the writing position instantly. Round trip, not a one-way door.
- **F4 Retrieve.** Move an item back into the story.
- **F5 Never intrude.** Opening the document, reading, cold read,
  export, counts, AI scope — the compost stays out of all of them.
- **F6 One afterlife system.** Compost (deliberate, living, editable)
  and graveyard (automatic, dead, a record) must read as one spatial
  story, not two unrelated inventions.

## 2 · What we have, held against those flows

Compost text lives at the TOP of the rope (blocks before the
boundary), styled in-flow, with a left-rail *navigator* panel, a
titlebar toggle + presence dot, and arrival blinks. Judged honestly:

- F1 ✓ (after this round's fixes).
- F2 ✗ — there is no entry path. A writer must already know the
  region exists, travel to the top of the document, click into it,
  and manage the blank-line structure by hand. Nothing teaches this.
- F3 ~ — the panel jumps you in; nothing brings you back.
- F4 ~ — manual select/cut/paste only. (Acceptable, but unnamed.)
- F5 ✗ — the document *opens on the compost*. Ctrl-Home lands in it.
  The story's first screen is its scraps. This is the "on top of
  everything" complaint, and it is structural, not cosmetic.
- F6 ✗ — compost at the top pole, graveyard at the bottom pole, with
  different navigation idioms (panel vs footer bar).
- And the two-homes problem: the same content manifests as the
  in-flow region AND the panel's row list — two representations, one
  thing (the "both in the left rail and on top" complaint).

## 3 · Options

**A. Patch in place** (keep top region, kill the panel, add an entry
verb). Fixes the two-homes count; F5 still fails — the document still
opens on scraps. Rejected.

**B. The free-flow tail section** (the writer's instinct; recommended
— and the critique narrowed WHY: B wins on F5 alone, and F5 is
granite. "The document opens on the manuscript, not the scraps"
requires no discovery, no teaching, no new verb — it just stops
being broken. Everything else below is how B serves the other flows
without new sins). Compost becomes an editable text region at the
document TAIL: the manuscript, then the soft seam (blank line +
hairline + tail mark + "Compost" whisper), then the compost text,
then the HARD seam (the Graveyard header slab — "your text stops
being editable here" must survive the screenshot test), then the
graveyard record. The boundary's meaning flips: manuscript = blocks
*before* it.

- F5 (the headline): the manuscript leads; byte 0 is its first line;
  ctrl-home lands on the title; cold read and export end at the soft
  seam naturally. The caret-trap on open dissolves structurally.
- F2 (amended per critique N2 — the travel verb does NOT solve
  discovery, and must not carry this flow): the primary from-scratch
  jot is a no-travel **New scrap** capture — files at the compost
  tail, the caret never leaves the manuscript, the footer chip
  pulses (the graveyard's own file-without-travel idiom, mirrored).
  Set-aside remains the move-existing-text entry.
- F3: `ctrl-shift-o` (and the titlebar control) is the TRAVEL verb —
  go to the compost's end; Esc returns to the writing position you
  left. TAP-TRAVEL + ESC-RETURN, never tap-to-toggle (a toggle's
  meaning would depend on hidden caret state — a mode). OPEN
  ARBITRATION for the writer: asides.md §2.1 specced this as a HELD
  quasimode (hold to visit, release to return); the two docs now
  disagree and the writer picks. The return is guarded by an
  excursion latch (set by travel, cleared by any manuscript click) —
  never raw caret position, so Esc inside a deliberately-edited
  compost item doesn't teleport.
- Navigation/presence: TWO rhymed footer chips (critique B3/N5) —
  compost warm with its count, graveyard drained with its coffin —
  same family, distinct natures, each hiding when its own section is
  on screen, the compost chip pulsing on arrivals. Not one merged
  bar (two anchor objects in one surface violates P11 from both
  sides). The left-rail panel is DELETED: one thing, one home; the
  titlebar control keeps no separate dot — the control itself lights
  while the caret is in the tail (P12).
- F6: one tail zone, decreasing aliveness as you descend — story →
  scraps (editable, dimmed face) → record (read-only). One spatial
  grammar with the graveyard, shared navigation.
- Arrivals (set-aside, orphan migrations, new scraps) APPEND at the
  compost's end — chronological is the arrival default only, never a
  maintained sort (the plotter curates the order; asides.md §5
  forbids auto-tidy). The compost face stays WARM at a smaller size —
  the aliveness gradient is carried by editability, position, size
  and caret-presence; drained colour remains the graveyard's alone
  (P10).
- **F4 gets its verb (from the extraction audit):** a compost
  selection's right menu offers **Move to the manuscript** — the
  selection travels to the writing position you left, spans and
  kinds preserved (the graveyard's lossless machinery, reused).
  Set-aside finally has its inverse (P13). NOT named "Put back":
  the graveyard's put-back targets the text's origin; this targets
  the caret — one word for two destinations would break P8.
- **Gating becomes one law:** what the menu refuses, the chord
  refuses (today ctrl-m anchors margin cards to compost while the
  menu hides the verb). Formatting works in compost; notes,
  diagnoses, set-aside, auto-cut don't.
- **Find/Replace, named:** Find *navigates* everywhere — you search
  for your own scraps. Replace All *mutates* the manuscript only —
  bulk mutation respects the boundary like every other bulk
  operation (counts, export, AI scope). Today replace-all silently
  rewrites the scrap box. The scope is ANNOUNCED (the count message
  says "in the manuscript"), and a live find-match sitting in
  compost is told why it didn't change rather than silently ignored.
- The caret-trap on open (today: byte 0 = inside the scrap pile,
  ctrl-home lands there, early Esc bounces back into it) dissolves
  structurally: byte 0 becomes the manuscript's first line.
- Mechanically: NO caret/layout surgery — one column, one flow. The
  cost is the boundary flip, and its sharpest edge is the MIGRATION
  (critique N3): the boundary is persisted per materialized
  checkpoint state, so semantics must be VERSIONED PER STATE — the
  live document migrates once (one transaction, honestly recorded);
  historical checkpoints read their own era and restoring one never
  teleports text (P1). A legacy-top-boundary fixture with a sealed
  old checkpoint is the regression test. Also: the tail gives
  compost the manuscript's full measure for free — the ~35ch floor
  asides.md fought for, which both the rail and option C could never
  afford (the unnamed gain beside the named chirality loss).

**C. The editable left column** (the lab mockup's original). Honesty
note from the critique: C also fixes F5 (the manuscript owns the main
column), so B is not the only cure — the dividing line is "not-top",
not "must-be-tail". B beats C on build cost (no two-flow caret
surgery), on the writer's own instinct, and on typography (C's narrow
column fights the measure floor that B gets free). Parked unless the
writer wants the periphery argument back.

**A-as-fallback:** if the migration hazard proves too dangerous, A +
"open scrolled to the manuscript / compost folded" is the cheaper,
non-migrating partial F5 fix. Named so the fallback is a decision,
not a scramble.

## 4 · The named loss in B

asides.md §4 argued compost-on-the-left for *chirality* — scraps in
peripheral vision while drafting. B abandons that: tail-compost is
not glanceable mid-document. In its defence: the built rail was never
glanceable either (a toggled overlay), F2/F3 are travel-shaped flows
anyway, and the periphery is already owned by the margin and the
strip. The loss is real and named; the writer arbitrates.

## 5 · Intent echo (the veto point)

One paragraph, per Gate 0: *The compost becomes the editable text
region at the document's tail, above the graveyard — one home, no
panel, warm face at a smaller size. The document opens on the
manuscript (that is the point). A no-travel **New scrap** verb files
a fresh thought at the tail without moving your caret; set-aside
keeps moving existing text there; a compost selection gets **Move to
the manuscript** (lossless, to your writing spot). `ctrl-shift-o`
travels to the tail; Esc brings you home (tap-travel — unless you
prefer asides.md's original held-quasimode, which is the one open
arbitration). Two rhymed footer chips (compost/graveyard) navigate
and pulse. Find navigates everywhere; Replace All announces its
manuscript-only scope. Existing files migrate once, live-doc only —
old checkpoints keep their era and never teleport text. The left
rail dies.*

## 6 · Critique record (Birman/Raskin/Norman, 2026-07-05)

Accepted: B1 warm-not-dimmed (P10), B2 two named seams, B3+N5 two
rhymed chips not one bar, B4 append-not-sort, B5 control-carries-
state, B6 the measure gain named, R1 tap-travel-not-toggle, R3
excursion latch, R4 verb renamed (one word two destinations breaks
P8), R5 announced Replace-All scope, R6 travel-vs-field-focus
commit-not-abandon, N1/N2 F2 split from travel (no-travel New scrap
primary), N3 versioned-per-state migration, N4 chip pulses on
arrival, N6 F5 is the headline. Open for the writer: tap-travel vs
held-quasimode (asides.md §2.1 conflict). Honesty notes: C also
fixes F5; A is the named fallback if migration proves too hot.*
