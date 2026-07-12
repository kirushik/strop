# Images in the prose: the behavioural spec

*(Round of 2026-07-12, escalated from papercuts report 4 — that round
shipped the BASIC fix only (`Document::insert_image_block`, the caret
parked past the image) and named the rest as its own wound. This doc
is the full pass: what an image IS to the writer, and every law of
caret, deletion, splitting, selection, and travel around one. Cites
`design-principles.md` by number. `document-model.md` §5b stays
authoritative for the import pipeline; this doc amends its §2 Image
row and §6's image export. Revised once through a six-lens panel —
record in §14; build record in §13.)*

## 0. The wound

An image today is a `BlockKind` riding an empty text line, and every
text mechanic treats that line as an ordinary empty paragraph. So:
Delete from the paragraph above merges prose ONTO the picture (the
line text paints over the pixels — a typover); Enter inside that
block clones the picture onto both halves; a Backspace from the
right side can silently drain the picture out of existence; and
there is no way to select, copy, or deliberately delete the picture
at all. The only interaction is alt-text-on-double-click, which is
invisible until found. Every one of these is the same root failure:
**the picture has no standing in the text's grammar.**

## 1. Ontology: what an image is

An image block has three parts, each with a different owner:

- **The picture** — pixels the machine holds for the writer
  (content-addressed asset, import policy of `document-model.md`
  §5b). The writer placed it, but it is not text: it cannot take a
  caret, and no text mechanic may move, clone, or absorb it. It is
  **furniture** — like the scene-break divider, a thing that stands
  in the column but is not made of words.
- **The caption** — the writer's words under the picture. Caption is
  prose the writer owns, so by P3 it must BE text, with text
  mechanics: click gives a caret, typing types, selection selects,
  formatting works. **The caption is the image block's own text
  line.** There is no out-of-band caption field; the line the block
  kind rides, empty today by construction, is the caption. (This is
  the load-bearing decision of the round; §10 migrates the vestigial
  `caption` field into the line.)
- **The alt text** — the description that travels in the Markdown
  (`![alt](…)`) and speaks for the picture where pixels can't.
  Metadata, not page text: never painted on the page, and — by P9 —
  never delivered by hover alone. It has one calm home (§8).

One breath: *the picture is furniture, the caption is prose, the alt
is luggage.*

## 2. The wall law and the furniture class

**Text never flows across a picture, and a picture never rides a
text edit.** Concretely:

- No merge may move prose from a neighbouring block onto the
  picture's line, and no merge may drain the picture's kind while
  its neighbour survives (the today-bug, both polarities).
- No split may clone the picture. Enter inside a caption splits the
  *caption*; exactly one block keeps the picture (§6).
- The picture changes only by whole-block verbs: insert (§7),
  exile/delete (§5), put back, replace-in-place (§4), undo/redo.

The wall has two enforcement layers, and both are required:

- **Gesture layer** (editor): Delete/Backspace arriving at a picture
  boundary resolves to staging or refusal, never merging (§5). This
  is the writer-facing law — one keystroke never fuses prose and
  picture.
- **Model backstop** (`BlockMap`): block kinds split into two
  classes, **flowing** (paragraph, heading, quote, list, code,
  footnote def) and **furniture** (image, divider). `on_edit`'s
  merge-keeps-first / split-clones-both rule is the flowing rule
  only. Furniture laws:
  - **Split**: the first fragment keeps the furniture kind; every
    further fragment is born a Paragraph.
  - **Range clamp**: a deletion whose range crosses a furniture
    boundary without covering the furniture block whole is
    decomposed at the wall — the bytes on each side go, the
    separator at the furniture boundary survives, both blocks stand
    (possibly emptied). No edit may fuse a flowing block with a
    furniture block.
  - **Whole cover**: a deletion that covers a furniture block
    entirely takes it whole (§5's range door).

  The backstop exists because arbitrary edits (multi-line paste
  over a range, scripted edits, future tools) reach the model
  without passing the gesture layer; the model must be safe on its
  own, and property tests hold it there (§11).

The **divider** is furniture in the model backstop as of this round
(splits stopped cloning it, merges stopped fusing it — the same bug
class, fixed by the same law). Its *gesture* grammar — click-select,
staged exile at its boundaries — is deferred, named: today's
backspace-strips-the-kind behaviour stands until the divider gets
its own paragraph in a later round. The deferral is recorded so the
asymmetry is a decision, not a hole.

## 3. Caret grammar

The caption line is an ordinary line to the caret: up/down/left/
right enter and cross it exactly as they would any short line of
prose. Arrows never select the picture and never skip the caption —
vertical motion through an image block lands *in the caption*, under
the picture, where typing visibly types.

An empty caption paints no chrome and no placeholder, but it is
never unreachable:

- **The slot is always a click target.** The band one caret-height
  beneath the pixels hit-tests to the caption caret even when the
  caption is empty — a click door with zero painted chrome (P4:
  no "Add a caption" ghost text; P5: the click-under-the-picture
  gesture is the borrowed captioning move).
- **Manifestation costs no geometry.** The empty slot borrows the
  image block's existing bottom margin — the caret paints inside
  space already reserved, so caret travel through the document
  never reflows it. Only a caption with content adds height.
- When the caret arrives by arrow or click, it renders with full
  prominence, and cursor-into-view accounts for the picture's
  height so the caret is never revealed below the fold.

The picture itself is not a caret stop. There is no cursor state
"on" the pixels; the caret is either in the caption, in neighbouring
prose, or the picture is *selected* (§4) — three states, no fourth.
(A selection born at a keyboard door carries an origin memory — §5 —
but it is the same state with the same contract.)

## 4. Selecting the picture

Click on the pixels selects the picture whole. The selection wash
covers **the whole block — pixels and caption line together** — 
because the block is the unit every verb below acts on: what is
washed is what goes (P6; the still tells the truth about the blast
radius). The wash is warm — the writer owns it; same amber family as
prose selection (`color-language.md`). No caret anywhere while
selected. If any part of the block is out of view when selection
happens, it scrolls into view — a state whose still cannot be seen
does not count as shown.

While the picture is selected:

- **Delete / Backspace** (a fresh key-down) — the block leaves
  whole (§5).
- **Typing never destroys furniture.** A printable key deselects
  and types *as text*: a click-born selection appends at the
  caption's end (the captioning gesture — click the picture, type
  the caption); a keyboard-born selection restores its origin caret
  and types there (§5). The replace-on-type contract survives only
  for range selections, where it is the unambiguous ancestor
  contract (P7).
- **Copy / Cut** — §9. Cut is copy + leave-whole.
- **Enter** — opens a fresh paragraph *below* the image and puts
  the caret there; the picture stays (adjudicated §12, with the
  direction rule that pairs it with §6's Enter-at-caption-start).
- **Replace in place** — dropping an image file on the selected
  picture, or pasting a bitmap while selected, swaps the pixels
  through the §5b import pipeline: same block, caption and alt
  untouched, one undo step. (The "better export" verb; without it
  the only path destroyed the caption.)
- **Esc** — deselects and restores the caret to where it stood when
  the state was entered; when no prior caret exists (click-born,
  cold focus), it parks at caption start.
- **Arrows** — deselect toward their direction: left/up → end of
  the block before; right/down → start of the caption.
- **Click elsewhere / focus change** — deselects without harm.

Click on the caption is just a text click. Double-click on the
pixels opens the alt home (§8), unchanged from today.

## 5. Leaving: deletion is staged exile

Deleting a picture is never a character deletion; it is the
whole-block exile that already exists for prose blocks, so the verb
already has its inverse (P13): the block — picture *and* caption —
goes to the graveyard as one entry, and **Put back** restores both.
The asset GC already keeps graveyard-referenced bytes alive, so the
round trip is honest all the way down to pixels.

**Staged exile** is this round's new gesture — Strop's first
two-press destruction, owned as new (§12; LAW 2 and the refused-
Enter arm are spiritual kin, not precedent). Its full law:

- **Stage** (first press at a boundary door, below): the picture
  becomes selected — same state as §4, wash over the whole block,
  scrolled into view — and the selection remembers the caret
  position the press fired from (the *door caret*).
- **Complete**: a second press of Delete or Backspace exiles the
  block. **The completing press must be a fresh key-down: key
  autorepeat never crosses the stage.** The staging press swallows
  all repeats of its key until release — implemented literally:
  completion is refused until a key-up of the staging key has been
  observed since the stage (the platform's own repeat flag is not
  trusted; it lies on X11). (Without this the held Backspace erases
  the two-press safety at repeat rate — the panel was unanimous.
  §11 pins it with a property test.)
- **Decay**: Esc, arrows, click, scroll-jump, or focus change
  deselect exactly as §4 says; Esc and typing restore the door
  caret first, so a cancelled or abandoned stage returns the writer
  precisely whence she armed — cancellation relocating the locus is
  itself harm.
- **After exile**, the caret lands where the block's first
  character used to be: the start of the block that now follows,
  or the end of the previous block when the exiled block was last —
  the same landing prose whole-block exile uses.

The doors, each named:

- **Delete at the end of the block above** an image → stages.
  (The field repro's step 7: Delete from the empty paragraph above
  now stages instead of fusing.)
- **Backspace at the start of the block below** an image → stages.
  (Symmetric outside door, same grammar — P8.)
- **Backspace in an empty caption** → stages: block-removal is the
  only plausible intent in an empty room facing furniture.
- **Backspace at the start of a non-empty caption** → a wall:
  refused, no-op. The caret is *inside* text the writer is editing;
  offering whole-block destruction there is a scope escalation no
  ancestor performs (Notion's wall is the borrowed contract).
- **Delete at caption end** → a wall: refused, no-op, caret stays.
  Prose from below never climbs into a caption by keystroke, and a
  destructive key is never repurposed as navigation — the refused
  press whose still is identical before and after is the truth
  (P6). The panel rejected the earlier "steps to the next block"
  law unanimously.
- **A range selection spanning the image block whole**, deleted →
  the picture goes with it, unstaged. This door stands on P7 + P13
  alone: delete-selection is the borrowed contract, and its inverse
  (undo; graveyard for the furniture) is honest. The earlier claim
  that "the sweep showed the wash" does not hold for Ctrl+A or
  Shift+PageDown past the fold; the off-screen-sweep case is a
  recorded cost of the borrowed contract, not a justification.
- **A range crossing the boundary without covering the block** →
  clamps at the wall (§2): text goes, picture and separator stand.

## 6. Enter and the split laws

- **Enter mid-caption or at caption end** → the caption splits as
  text: the image block keeps the head, the tail becomes a new
  Paragraph below. The picture does not clone (§2 backstop: first
  fragment keeps the kind). Enter at the end of a non-empty caption
  reads as "step out below" — the tail is empty, a fresh paragraph
  opens under the image. (Repro step 8: no second picture, ever.)
- **Enter at the start of a non-empty caption** → room above: a
  fresh empty Paragraph opens *above* the image block; picture and
  caption stay together, caret stays at caption start. (Without
  this the split law would strand the caption below a captionless
  picture.)
- **Enter in an empty caption** → the end law wins: a fresh
  paragraph opens below and the caret moves into it. The two laws
  above collide only when the caption is empty — start and end are
  the same position — and "step out and keep writing" is the
  dominant intent under a picture (the common case: §7 births every
  image captionless). Adjudicated §12.
- **Shift+Enter in a caption** → soft break, as anywhere; captions
  may wrap and may break — they are prose.

There is no keystroke that moves prose INTO a caption or a caption
OUT to the body: that travel goes by cut/paste, deliberately.
Recorded with its reopening condition in §12.

## 7. Arriving: drop, paste, insertion shape

- The insertion shape is last round's, kept: an image always stands
  alone as its own block. At document end, a trailing empty
  paragraph opens so there is always an after. One transaction.
- **Drop targets the pointer and never moves the caret.** During
  drag-over, the gap between blocks nearest the pointer shows a
  quiet insertion rule (a still frame of it reads as "it will land
  here" — P6); on drop, the image stands there and the writer's
  caret stays exactly where it was — the hand aimed the drop, the
  caret wasn't involved. (The old caret-relative landing is a
  recorded Norman gulf-of-evaluation failure.) The pointer-targeted
  landing is unconditional law; the live insertion rule is a
  progressive layer — it paints wherever the platform delivers
  drag-over positions, and a platform that withholds them (Wayland
  is unverified) degrades to the landing law alone, recorded as a
  platform gap, never to caret-relative drop.
- **Paste** (Ctrl+V of a bitmap) targets the caret — the caret is
  where paste means — and parks it in the block after the new
  image: you paste a picture and keep writing prose, you don't fall
  into its caption.
- Dropping/pasting onto a *selected* picture replaces in place
  (§4).
- Import policy, dedupe, caps: `document-model.md` §5b, unchanged.

## 8. The alt home

Alt text is metadata with one calm resting place: the bottom strip
that already exists, opened by double-click on the pixels — and now
also present whenever the picture is *selected*, showing
`Alt text: <current>` as data. The strip is not a readout beside a
control; **the strip IS the control** (P12): clicking the alt line
gives it a caret and enters the same edit state double-click opens.
Alt is never painted in the column and never hover-revealed: by P9
hover may only expand what is visible, and alt is invisible by
design — a hover tooltip would be its sole channel, which P9
forbids. The double-take in the field report ("the sub-title isn't
shown on hover!") is answered the other way around: the visible
sub-title becomes the *caption* — real text, always visible, no
hover needed — and alt stays luggage.

## 9. Travel: copy, cut, paste

- **Copy with the picture selected** writes two clipboard entries:
  the Markdown form `![alt](asset:… "caption")` as text, and the
  pixels as a bitmap. Strop-to-elsewhere gets a real image or an
  honest Markdown line; elsewhere never gets a dead `asset:` link
  alone.
- **Paste inside Strop — precedence law**: when the clipboard's
  text entry parses as a Strop image line, the text form wins:
  caption and alt come from it always; pixels come from the asset
  store when the `asset:` hash resolves in this document, else from
  the sibling bitmap entry (blake3 dedupe keeps it byte-honest).
  Only a foreign clipboard — bitmap with no Strop image line —
  takes the bare §5b import path. This is what keeps caption and
  alt alive across documents; the earlier bitmap-first law silently
  dropped both.
- **A text-range copy that spans an image block** carries the block
  in its Markdown form; the same precedence law applies on paste.
  Cross-document, a range-paste whose `asset:` link resolves
  nowhere and has no bitmap sibling imports as the literal Markdown
  line — visible, not silent, per the document-model boundary rule.
  (A Strop-private clipboard flavour carrying asset bytes for
  multi-image ranges is the recorded reopening condition if field
  reports show cross-document moves are common.)
- **Platform cost, recorded**: the gpui fork's Linux clipboard
  writes text-only to other applications (Wayland offers text MIME
  types; X11 serves `item.text()`), and Strop runs one process per
  document — so on Linux today, cross-document and cross-app paste
  receives the honest Markdown line with caption and alt, and
  pixels travel only within a document. The two-entry write ships
  as specified (correct in-process, correct on platforms with
  multi-format pasteboards); the fork patch offering `image/png`
  beside text is a named follow-up, reopened by field reports of
  writers pasting pictures into other apps or across documents.
- **Cut** = copy + leave-whole, both halves of the verb in one
  gesture, inverse intact (P13: Put back, or paste it back — either
  door).

## 10. Storage, Markdown, migration

- `BlockKind::Image` loses its `caption` field: the block's line IS
  the caption. `src` and `alt` remain out-of-band block params.
- **Markdown**: exports as `![alt](src "caption")` — the standard
  title slot carries the caption. Empty caption emits no title.
  The title slot is a plain string, so caption *spans* flatten to
  CommonMark inline syntax inside it (`*…*`, `` ` ``…`` ` ``) and
  soft breaks become spaces; import re-parses inline syntax from
  the title, so Strop↔Strop round-trips whole. A foreign renderer
  shows the syntax literally in its tooltip — a recorded, visible
  cost at the boundary, same class as underline's `<u>`
  passthrough. (The alternative — exporting the caption as an
  italic paragraph and re-adopting it heuristically on import —
  was rejected: import-by-guess is what the autolink decision
  forswore.)
- **JSON block persistence**: readers keep accepting the old
  `caption` field; on load, a non-empty stored caption with an
  empty block line migrates INTO the line (one-way, at open, like
  the compaction pass). Writers KEEP emitting the field, always
  empty — dropping the key from the wire would make every released
  build's strict deserializer error and fall back to the legacy
  token parser, which collapses the whole BlockMap to paragraphs
  and persists the wreck on its next save; ~13 bytes per image
  block is the price of never misreading (same class as the
  boundary-key era rule). The wire key retires in a later era flip
  once pre-migration builds are extinct. The runtime enum still
  loses the field now: `Image { src, alt }`, with a serde mirror
  emitting and accepting the vestigial key. The legacy token format
  already drops image metadata and stays legacy.
- **Migration never touches history**: checkpoint and history
  states are read-only past; rewriting stored captions inside them
  would silently move historical text and misalign every recorded
  anchor. Only the live document migrates at open. A pre-migration
  checkpoint's preview shows the picture without its legacy
  caption — a recorded, visible cost — and restoring such a
  checkpoint re-runs the migration on the restored (now live)
  state, so nothing is ever lost; only old previews are quieter
  than they were.
- Asset store, GC reachability (live + graveyard + history +
  checkpoints), and the in-file-always decision: unchanged.

## 11. Rendering and the rig

- The picture paints first, the caption paints *below* it (today
  the line text paints at the picture's origin — over the pixels;
  that overlay is the typover and it dies this round).
- **Display size law**: the picture fits the column width, and its
  display height is capped at roughly two-thirds of the viewport
  with proportional fit — a tall portrait never swallows the page.
  (Natural size within those bounds; §5b already caps stored
  pixels.) There is **no resize verb in v1** — a named cut; it
  reopens if field reports show writers fighting the fit law.
- Caption face: quieter and smaller than body (the cold-read page
  already sets ~0.8× muted italic, centered). Live view matches the
  cold-read's caption optics so the two doors agree on what a
  caption looks like. A dedicated caption face (PT Caption was
  named) is typography polish and may ride a later round; the
  *position* law — under, never on — is this round's.
- The block-wide selection wash, the staged still, the drop-gap
  rule, and the refused-press stills must each read correctly as
  still frames (P6).
- **Rig**: a `seed:image` smoke seed (put_asset + insert_image_block
  + a caption) so `wshot`/`wrun` can finally drive pictures. The
  original field repro becomes the acceptance script: empty para →
  image → prose para; arrow up lands in caption; Delete at the
  empty block above stages, never fuses; Enter never duplicates;
  Backspace-Backspace (two fresh presses) exiles; a HELD Backspace
  run across a caption stops at the stage and the picture survives
  (the autorepeat property); Put back restores picture + caption.
  Model property tests pin the §2 backstop: splits never clone
  furniture, range deletes clamp at the wall, and no edit sequence
  ever leaves prose on a furniture block's line that the writer
  didn't put in its caption.

## 12. Adjudications

- **Caption = the block's line, not a field** — P3 decides it: the
  caption is the writer's words, so it must be text with text
  mechanics, not a widget-held string. The counterargument (a field
  is easier to keep "safe" from merges) is answered by the wall law
  living in the model backstop instead; safety by ontology, not by
  hiding the words in metadata.
- **Typing never destroys furniture** (panel round 1). The draft
  kept the strict selection-replace contract for typing on a
  selected picture; three lenses independently showed the naive
  captioning gesture — click the picture, type — would exile the
  picture on the first letter. The ancestor contract is contested
  (Docs replaces; PowerPoint/Keynote type into the object's text
  slot), so P7 does not force the destructive reading, and P3 does
  force words toward the block's own text line. Typing now enters
  the caption (click-born) or the origin caret (keyboard-born);
  replace-on-type survives only for range selections. Reopening
  condition: none — this direction only gets safer.
- **One selection state with an origin memory.** The panel split
  between collapsing armed/selected (one contract) and splitting
  them (quasimode). Collapsed won *because* typing became safe —
  the only destructive act left in the state is the deletion key
  itself, which is exactly what a staged exile means by its second
  press. The keyboard-born selection differs in one datum only —
  the remembered door caret that typing/Esc restore — which is
  origin, not mode: both origins obey "typing types text, deletion
  completes, everything else decays," and a wrong guess about
  where the caret returns costs one glance (Raskin's locus argument
  decided *what* gets restored, not a second contract).
- **Staged exile is new, and owned as new.** The draft cited LAW 2
  and the refused-Enter arm as precedent; the panel showed both
  claims false (LAW 2 resolves transient fields on gestures; the
  refused-Enter arm is surviving caret attrs, not arm-then-act).
  This section is the founding instance: the stage/complete/decay
  table in §5 is the definition, and the autorepeat-never-completes
  rule is load-bearing, not implementation detail.
- **Enter's direction rule.** Enter on a selected picture opens
  below; Enter at the start of a non-empty caption opens above.
  One law decides both: *Enter makes room on the side the insertion
  point faces* — a selected object faces past itself; a caret at
  text start faces before it. The empty-caption collision resolves
  to below (§6) because a captionless picture's dominant intent is
  "step out and keep writing." The strict replace-contract reading
  of Enter (vaporise the object, leave a blank line) stays rejected:
  everywhere else in Strop Enter makes room, and a key that makes
  room must not destroy furniture. Reopening condition: field
  reports of writers expecting replace.
- **Delete at caption end refuses; Backspace at non-empty caption
  start refuses.** Both walls, unanimously (a destructive key is
  never navigation; whole-block destruction is never offered from
  inside the text being edited). The split's gesture-inverse
  objection (Backspace at the start of the paragraph Enter just
  split off now refuses instead of rejoining) is answered: undo is
  the split's inverse; rejoin-by-Backspace would reintroduce
  one-keystroke prose-into-caption fusion, the exact wound of §0.
  Caption↔body travel goes by cut/paste; reopening condition:
  field reports of writers fumbling the four-step move earn a
  dedicated verb.
- **Alt has no hover channel** — P9 is categorical; the panel that
  killed hover-gated labels on the strip kills tooltips here.
- **Drop targets the pointer and leaves the caret alone** — the
  draft fixed the landing but still parked the caret at the drop;
  the panel caught the residue: a pointer drop that teleports the
  typing caret across the document is the same gulf in a new place.
- **Named cuts, each with its reopening condition:** no drag-to-
  move (cut/paste is the move verb; reopens on field evidence of
  reorder pain — the panel's three-photo walkthrough is noted); no
  resize verb (§11's fit law stands in; reopens on writers fighting
  the fit); divider gesture parity deferred (§2; model backstop
  covers it now); no Strop-private multi-image clipboard flavour
  (§9; reopens on cross-document move reports).

## 13. Build record (2026-07-12)

Built in six phases on branch `inline-images` (spec+plan 3b1c6fa):
two parallel worktree agents for the model (THE WALL, 3b49968; THE
CAPTION COMES HOME, b61774e; merged c457d38), then three sequential
editor phases (UNDER, NEVER ON 5c1b58e; STANDING AND LEAVING
f3f213c; ARRIVALS AND DEPARTURES 1217dba). The field repro that
opened the round is rig-verified dead: Delete above an image stages
instead of fusing, Enter never clones, held Backspace stops at the
stage, the refused-press stills are byte-identical, and Put back
restores picture + caption.

Discoveries the build recorded against the spec:

- **§6's soft-break sentence has no substrate**: Shift+Enter binds
  to the same Newline action, and U+2028 is a block *splitter* in
  the live model (ropey's line-break set), not an in-block soft
  break — the document-model §1 soft-break design is unbuilt. A
  caption Shift+Enter therefore splits lawfully (tail born
  Paragraph). The sentence stands as intent for whenever soft
  breaks arrive.
- **Whole-cover tightened for empty captions**: content + one
  bounding separator takes a block whole only when the caption is
  non-empty; an empty caption demands full enclosure (the
  bare-separator range is exactly the old Backspace-drain bug).
  The deliberate exile verb got its own model door
  (`delete_bytes_whole_block`) because the clamp — correctly —
  refuses to release an empty furniture line to arbitrary ranges.
- **One R6 exception**: room-above at a document-leading image
  stamps both fragments in one transaction (the furniture-keeps-
  first split law points the wrong way at offset 0 of block 0).
- **A fork bug found by §7's rig work**: gpui's drop dispatch gates
  on `is_hovered`, which stays false while the last input was
  keyboard — and file-drag events never flip the modality, so any
  drop right after typing was silently refused (pre-existing,
  affects real usage). A defer-dispatched synthetic MouseMove works
  around it; the one-arm fork fix rides the pending fork-push
  ceremony.
- **Pre-migration checkpoint previews**: no released build ever
  wrote a non-empty caption field (all constructors emitted empty),
  so §10's restore-re-runs-migration clause has, in practice,
  nothing to migrate; the wire mirror drops legacy values at parse
  and the raw-JSON re-read hook is recorded should that ever prove
  wrong.
- **Paste rulings** (§9, recorded in code): a replace aimed at a
  selection that decayed is dropped, never re-aimed; a
  picture-shaped paste with unresolvable pixels while selected is
  refused, never degraded into caption text; multi-line pastes
  rebuild only resolving image lines and degrade seam-spanning
  landings to literal text rather than guess at clamp geometry;
  middle-click PRIMARY paste shares the rebuild law.
- **The styled-block carve-out at the Backspace-below door**: when
  the block below an image is a Heading/quote/code/list, Backspace
  at its start strips the block's kind first (the house rule that
  predates this round) and only the SECOND press reaches the §5
  door and stages. Deliberate layering — the strip verb outranks
  the door because it acts on the block the caret is in, not on
  the neighbour — unit-tested as such.
- **Cold-read captions set as one line, clamped to the measure**
  with an ellipsis when long; the live editor wraps. N10
  (cold-read caption lines carry no anchors) stands. Reopening
  condition: a caption-heavy manuscript in the field — the fix is
  running captions through the breaker as wrapped set lines.

A same-day adversarial review round (three finder lenses over the
branch diff, every finding verified by an agent instructed to
refute it) yielded 14 candidates → 13 confirmed → all fixed. The
serious four were wall-law breaches the build's own tests had not
reached: the whole-cover rule silently voided between ADJACENT
furniture blocks (the drained-ghost state); the whole-block exile
restamp misfiring when the exiled block's neighbour is an
empty-caption image (geometric ambiguity — an empty line's start
IS its neighbour's separator); auto-cut filing graveyard entries
for bytes the clamp never deleted (Put back could clone a
picture); and the block-format verbs (heading/quote/code/list)
overwriting an Image kind outright. The rest: middle-click paste
skipping the §4 decay, word-delete reaching across the wall from
styled blocks, the scroll-jump decay entry unwired, pixel clicks
falling through to footnote jumps, the drop-gap rule painting
over a replace-in-place target, and the caption migration
counting only LF where the block metric counts all of ropey's
breaks.

## 14. Panel record (2026-07-12)

Six lenses (Raskin, Cooper, Norman, Tognazzini, corridor stranger +
borrowed conventions, internal consistency), 55 findings, all six
verdicts *unsound* against the draft. Every blocker traced to three
roots: the armed/selected contract collision, autorepeat crossing
the stage, and missing model law for partial ranges at the wall.
All three are resolved above (§4, §5, §2), the typing contract was
reversed outright (§12), Delete-at-caption-end lost its navigation
reading, the markdown title-slot fidelity limit is now stated
instead of implied (§10), and four scope holes became named cuts
with reopening conditions (§12). The false precedent citation was
withdrawn; staged exile is founded here.
