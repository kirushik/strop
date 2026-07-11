# Papercuts, round 2026-07 — eleven cuts, four wounds

*(Dogfooding feedback from the owner, 2026-07-11, given the full UX
treatment rather than eleven hotfixes: the cuts cluster into four
underlying wounds, each gets a writer scenario, a rule, and a build
note. Governed by `design-principles.md` (cited by number throughout),
`ux-glossary.md`, `asides.md`, `docs/impl/03-flanks.md`,
`docs/impl/05-cold-read.md`. Status: ADJUDICATED — a five-lens critic
panel (Raskin, Norman, Cooper, Tognazzini, the corridor stranger)
reviewed the draft 2026-07-11; §6 records the verdicts and every
amendment folded back into the rules below.)*

## 0. The shape of the round

The eleven reported cuts are not eleven bugs. Traced to root causes
they collapse into four wounds:

| wound | cuts it explains |
|---|---|
| **A. The caret's momentum** — inline styles ride the caret further than the writer's intent | keyboard selection raises no flanks; bold survives Enter; style-after-selection continues onto new typing |
| **B. The exile round-trip** — a paragraph is not yet a first-class citizen of the graveyard | empty paragraph left behind; put back glues to the previous paragraph and wears its formatting |
| **C. The margin's manners** — the conversation lane doesn't follow the house grammar for clickability, focus, and yielding | done/× undressed; Ctrl+M composer occluded; "Note" label; margin click-out doesn't resolve; rename field lingers |
| **D. The cold read's front door and back room** — the newest room has no doorknob and broken furniture | space files the note; layout switch closes it; no visible entry to the cold read |

One mechanical fact does the most damage: `InlineAttr::expands()`
lets a span swallow any text inserted at its right edge, with no
paragraph seam, no distinction between the typing hand and the
machine's own insertions, and no distinction between "typing
momentum" and "an extent the writer marked." That single rule
underlies three of the four A-cuts and the formatting half of B.

### Two laws the panel forged across clusters

The critics found the same disease in three places (C2's dismissal
duty, C4's field list, D2's blur heuristic): rules stated as
*per-command duties* rot when the next command ships. Both are
restated here once, as derived state, and the clusters below are
corollaries:

- **LAW 1 — flank visibility is a derived predicate.** Each frame:
  flanks are visible iff *(settled non-empty selection) ∧ (the prose
  owns keyboard focus) ∧ (no transient field is open) ∧ (not
  latched)*. The latch is set by focus-moving or text-moving verbs
  (annotate, aside, exile) and by collapsing the selection; it clears
  only when the selection *changes*. No per-command `dismiss_flanks`
  duty anywhere; the occlusion bug, the re-raise bug, and the
  after-commit re-offer all fall out of the predicate.
- **LAW 2 — transient fields resolve on writer gestures, never on
  bare focus events.** A click, a chord, an invoked command resolves
  an open field; a focus-out with no gesture behind it (keyboard
  layout switch, app deactivation, window churn) never does. One law
  for the margin composer, the rename field, the session goal, the
  checkpoint name, and the cold-read note — attached to the shared
  TextField machinery, not to call sites.

## 1. Wound A — the caret's momentum

### The writer's scenario

She is drafting, hands on the keyboard. She marks a passage — with
the mouse when wandering, with shift+arrows when in flow. Marking is
sometimes a prelude to formatting, sometimes to deleting, sometimes
to nothing (reading with the hands). When she applies a style, the
style is a *statement about that extent* — "this phrase is stressed,"
"this passage is highlighted for later." It is almost never a mode
she wishes to enter. The tool must distinguish the two intents
without asking.

### A1. The flanks answer to the selection, not to the mouse

**Today:** `selection_popover` is set only in `on_mouse_up`
(editor.rs:8761). A keyboard selection never raises the flanks. The
June research that birthed the flanks prescribed mouse-up-only *plus*
a keyboard summon; the summon shipped (`ctrl-.`, commands.rs:149)
but is undiscoverable, and the symmetric raise never shipped.

**Rule:** *the flanks belong to the selection, whatever hand made
it* (LAW 1). A settled non-empty selection raises them; a collapsed
selection lowers them. Precisions, all panel-forged:

- **Settle:** one constant, **250 ms** since the last extension (and
  never mid-drag). One value, not a range — a range forks into
  dialects the hand can't learn (Raskin). The beat gates only the
  first rise; once risen, the flanks track further growth and never
  lower mid-extension (no strobing).
- **Pure display on passive raise:** flanks raised by selection
  claim no keyboard focus and reroute no keystroke. Escape's meaning
  never depends on whether they happen to be up: Escape cancels an
  open field if any, else collapses the selection (which lowers the
  flanks as a consequence). The flanks are not an Escape layer —
  a timing-gated key meaning would be a mode error by construction.
- **`ctrl-.` becomes raise-and-enter:** the explicit summon moves
  focus *into* the flank — arrows navigate cells, Enter fires,
  Escape returns to the prose at the saved caret. A surface that
  appears for the keyboard must be operable by it (WAI-ARIA toolbar
  contract; this cut was literally "my keyboard habits block the
  UX element"). Passive raise stays display-only; the deliberate
  chord is the deliberate entry. The palette row gains formatting
  aliases so "strikethrough"-shaped searches find it.
- **Readout:** flank format cells reflect the selection's current
  attrs (`attr_active`) — the control is the indicator (P12), and
  it confirms selection-apply the moment it happens.
- **Geometry:** flanks rise beside the visible end of the
  selection; if no part of the selection is on screen, they don't
  rise.

Click-on-selection-to-reveal (the owner's suggested escape hatch) is
**rejected** on P7: everywhere in computing, a click inside a
selection collapses it to a caret; a click that instead opens chrome
would subvert the text widget's contract to patch an asymmetry we
can remove at the source.

### A2. The paragraph seam — Enter ends the run

**Today:** `newline` demotes heading/divider blocks to Paragraph
(editor.rs:8013-8023) but inserts the `\n` as ordinary text, so an
expanding span swallows the newline and the next paragraph opens
still-bold. Heading resets; bold doesn't; the writer reads this as
caprice.

**Rule:** *inline momentum dies at the paragraph seam.* A new
paragraph starts with clean inline state, exactly as it starts with
a clean block kind. Mechanically: an expanding span never absorbs an
insertion across a newline — the seam terminates expansion — and
`newline` clears sticky `caret_attrs`.

**The seam kills momentum, never marks** (Raskin, Norman — the
data-loss misreading this sentence exists to prevent): Enter
*inside* an existing span splits it into two spans, both intact; a
bold sentence broken into two paragraphs is bold on both sides.
Only the caret's forward-looking state dies at the seam. If a soft
break exists or ever ships, it is a seam too — monotony. And undo
of the Enter restores the pre-split state exactly, sticky attrs
included (P13).

(Word and Docs carry bold across Enter; we deliberately don't. A
bolded run that should span paragraphs is two statements, restated
with two keystrokes; a bold paragraph that should have been plain
is a wound requiring select + unbold + retype of intent. The
asymmetry of cost decides it, and the complaint corpus agrees —
"every new paragraph is bold" threads against Docs, Word's
"low-grade harassment" thread — while the 2026-07 convention
research found no writer asking for the carry.)

### A3. Marks of extent do not grow by appending

**Today:** every attr except Code/Link/FootnoteRef `expands()`
(document.rs:275-281): text typed at a span's right edge is
absorbed. So select-a-passage → highlight → move caret to the end →
type, and the highlight streams onto the new prose (the BMW
turn-indicator that never cancels).

**Rule:** split the inline set by what the mark *means* — the split
Peritext codifies (emphasis marks end-inclusive; link/comment marks
end-exclusive) and ProseMirror ships as `MarkSpec.inclusive`:

- **Emphasis-class** (Strong, Emphasis, Underline): character styles
  a writer extends by typing — appending to a bold word keeps it
  bold (the near-universal convention; every editor that broke it
  drew bug reports, not gratitude). These keep `expands() == true`.
- **Extent-class** (Highlight, Strikethrough — joining Code, Link,
  FootnoteRef): statements about a *fixed extent* of existing text.
  Marked once, they do not grow by appending. `expands() == false`.
  Highlight has no industry convention to borrow (Word's highlighter
  is a selection-painter, so the question never arises there);
  classifying it as annotation-like is our call, and the owner's
  motivating case ("highlight a passage for later, move on") is the
  argument. Strikethrough-as-extent is likewise a house call —
  struck text means "this extent is dead," and the dead don't grow.

The classification tracks the intent that created the mark — bold
is something you type *in*; a highlight is a statement *about* text
that already exists — so it dissolves into expectation rather than
becoming a taxonomy to learn (Raskin's verdict). Applying any style
to a selection keeps the selection (styles stack); it arms nothing;
the persistent selection is the visible state indicator.

**Expansion is a typing affordance** (LAW 2's sibling, the panel's
deepest catch): `expands()` fires today on *any* insertion
(document.rs:693-705), including the machine's own — which is
exactly how put-back gets dressed in the neighbor's bold.
Machine-performed insertions (put back, paste) never trigger edge
expansion; only the typing hand extends a span. This law is a
prerequisite for B2 and closes the same hole for paste.

**Disarm-on-apply is refused, permanently** (see §5): per-span end
polarity would make two pixel-identical bold spans behave
differently by invisible history — hidden state defeating habit
formation absolutely. Emphasis-class continuation after
selection-apply is visible as it happens, at the locus of
attention, one reflex-chord from off; that is not a wound.
Reopenable only on Strop-native dogfooding evidence post-split.

## 2. Wound B — the exile round-trip

### The writer's scenario

She kills a paragraph — deliberately, whole. The verb she performed
is "this paragraph leaves the manuscript." What she must not be left
holding: its empty grave in the prose (a blank line she now deletes
by hand, resenting the tool that made her do it), and on regret, a
resurrection that returns as a limb grafted onto the neighboring
paragraph wearing the neighbor's bold.

### B1. Exiling whole paragraphs takes the breaks along

**Today:** a full-paragraph selection covers the text but never the
bounding `\n`; `cut_to_graveyard` deletes exactly the selected bytes
(document.rs:1671), leaving an empty block. Nothing collapses it.

**Rule:** *a whole-block exile leaves no grave in the prose.* Stated
as an outcome, not a byte recipe (the panel's normalization catch —
triple-click and shift+down selections *already include* the
trailing `\n`, ending at the next block's char 0; "consume one more"
would eat two and fuse the neighbors):

- **Normalize first:** a selection ending at a block's char 0 is
  reclassified as ending at the previous block's text end.
- **Whole-block detection** on the normalized selection: start at a
  block's text start, end at a block's text end, covering one or
  more complete blocks. Anything else — including mixed selections
  that start or end mid-block — keeps today's exact-byte semantics
  (which produce no empty-block artifact for partial cuts). Two
  verbs, two contracts: plain delete stays exact-bytes; only the
  exile verb interprets intent, and only in the whole-block case.
- **The outcome:** after a whole-block exile, exactly the
  separators that joined the neighbors remain — one where the
  block(s) stood, none dangling. Trailing separator consumed;
  leading one when the last block of the document is cut.
- The entry records `whole_blocks: true` (serde-default false —
  legacy entries unaffected).
- **Undo is the same atom** (P13, four critics independently): the
  cut and its separator are one transaction; Ctrl+Z restores text
  plus separator in one step and removes the graveyard entry, as
  today's test already demands for the text alone.

### B2. Put back rebuilds a paragraph, not a splice

**Today:** `put_back` inserts `entry.text` bare at a drifted char
offset (document.rs:1830); with no `\n` it merges into whatever
paragraph the offset landed in, span expansion dresses it in the
neighbor's inline marks before the entry's own spans are re-added,
and the block re-stamp then mislabels the fused line.

**Rule:** *what left as a paragraph returns as a paragraph* (P13 —
the inverse in the same grammar). For a `whole_blocks` entry,
put-back inserts at the nearest block boundary to the drifted
origin (never mid-paragraph), with separators reconstructed so the
returned text stands as its own block(s), wearing its own recorded
kinds and spans and nothing of its neighbors'. Panel additions:

- **Atomic dressing:** separators, text, kinds, and the entry's own
  spans land as one structured insertion — today's bug is an
  ordering bug (expansion dresses the text before the entry's spans
  re-add), and the fix may not leave the order to chance twice. The
  A3 machine-insertion law (expansion never fires on programmatic
  inserts) is the belt; the atomic insertion is the suspenders —
  and it protects *partial* entries too, which the seam rule alone
  never covered.
- **Reveal the landing** (Norman's action loop, Tognazzini's
  invisible resurrection): put back scrolls the returned block into
  view if needed and places the caret at its start. An invisible
  resurrection reads as a failed one.
- **Form shows the contract** (Raskin's hidden bit): `whole_blocks`
  must not be invisible state. Whole-block entries stand as
  paragraph blocks in the graveyard (as they already do); fragment
  entries wear the anchor-fragment typographic form margin notes
  already use for quoted fragments — one grammar for "a piece of
  your text, quoted" (P8), and the entry's shape now predicts its
  return behavior.

**Non-goal this round:** the drifting `origin_pos` ("location
indication is off") stays best-effort. The owner can live with it;
a content-anchored origin (the checkpoint-notes Option-A machinery)
is the known future fix and is out of scope here.

## 3. Wound C — the margin's manners

### The writer's scenario

The margin is a conversation held while writing. Its cost model is
strict: glances, not studies; exits that land back in the prose
exactly where thought left off. Every extra click, every control
that doesn't look like one, every field that won't close is a toll
on the way back to the sentence.

### C1. Clickable words wear the house dress; glyph actions are icons

**Today:** "done" and "×" on cards signal clickability only by
cursor and hover recolor (editor.rs:16264-16310) while their peers
("Put back", the door's inactive pole) wear the dashed underline
(DESIGN.md: dashed = "this text is clickable").

**Rule:** two forms, one law (P8): *a clickable word is dashed; a
clickable glyph is an icon button with the standard hover plate.*
"done" gets the dashed underline. The typographic "×" is replaced by
the plate's `dismiss` icon with the same hover-plate treatment the
titlebar icons use — form says "control," not "letter," and the
plate is the hit region, sized like its titlebar kin (≥ the icon
buttons' existing target; no sub-Fitts targets on cards). Status
words ("Diagnosis", level, "· detached") remain undressed ink,
which is precisely how they now read as different from actions.

### C2. Taking the offered verb retracts the offering hand

**Today:** Ctrl+M (`add_note`) opens the composer but leaves
`selection_popover == true`; the right verb flank paints after the
margin in the element tree (editor.rs:17364 vs :17288) and occludes
the fresh composer at its own y. The writer types blind.

**Rule:** a corollary of LAW 1, stated positively (the corridor
stranger's cut): *style toggles keep the flanks raised* — the
selection persists and the offer stands, now with the cell readout
confirming the act (A1) — while *focus-moving or text-moving verbs*
(annotate, aside, exile) set the latch. The latch, plus LAW 1's
no-transient-field-open conjunct, means the composer can never be
overpainted and the flanks cannot re-rise over it a settle-beat
later; when the composer commits and the saved selection returns,
the unchanged selection keeps the latch — no nagging re-offer
(P2). No per-command dismissal duty; one predicate, evaluated each
frame.

### C3. The card's label is data, not taxonomy

**Today:** writer-note cards are captioned "Note"
(editor.rs:14689). The word tells the writer nothing she doesn't
see — it is a category noun on a surface that already carries
warm color and her own words.

**Rule:** P4 — words on chrome are data or verbs. The caption
becomes the note's **creation moment**, in messenger grammar (the
panel's same-day catch — during a normal session every card is from
today, and a lane of identical "11 Jul" captions costs the same
attention as a lane of "Note"): time-of-day for today's notes, the
strip's quiet date grammar for older ones; hover may expand to the
full timestamp (P9). `created_unix` is already stored
(document.rs:753). Diagnosis cards keep their level word (that *is*
data); "· detached" stays. Warm color continues to say "yours"
(P10), and the moment-vs-level-word split keeps the two card
families separable without color.

### C4. A click that lands nowhere still lands

**Today:** a mouse-down in the margin lane's empty space early-
returns (editor.rs:8617-8620) without moving keyboard focus, so an
open composer's commit-on-blur never fires; the writer clicks
beside the card — the natural "I'm done" gesture — and nothing
happens. The document-name rename field has the same disease: its
close waits on a blur that dead-zone clicks never deliver (the
perceived "delayed write" is mostly an *undelivered blur*; the
rename itself is a cheap atomic `fs::rename`, store.rs:388).

**Rule:** *there are no dead zones: any click outside an open field
resolves it* (LAW 2). Panel-forged clauses:

- **Commit AND act, one gesture:** the resolving mouse-down commits
  the field *and* delivers its ordinary meaning at the target — a
  swallowed first click would trade the field-that-won't-close
  excise for a two-click excise. Exits split by landing point
  (Norman): a click *into the prose* commits and places the caret
  at the click point — overriding a deliberate click target with a
  saved position would be a mapping violation; a click into a true
  dead zone (margin blank, chrome) commits and restores the
  caret/selection saved at field-open, so the exit lands where
  thought left off. Keyboard commits restore the saved position
  likewise.
- **Stationary targets:** the commit may re-pack the margin lane;
  resolving clicks hit-test against pre-commit geometry (or lane
  re-layout defers to mouse-up), so the control the writer aimed at
  is still under her mouse-up.
- **Empty fields discard:** click-away on an empty composer or
  rename discards rather than commits — the tool must not
  manufacture a blank card from a stray click. One law across the
  field family. Escape-cancel is preserved on every field.
- **Optimistic commits:** the field closes the instant the gesture
  lands; durable writes (rename, save) happen after. Any remaining
  measured latency is a build bug against this rule, not a
  follow-up investigation (instrument with STROP_PERF if the rig
  shows one).
- The law binds to the shared TextField/composer machinery, not to
  call sites — the enumeration (composer, rename, goal, checkpoint
  name, cold-read note) is the *test list*, not the implementation.

## 4. Wound D — the cold read's front door and back room

### The writer's scenario

Reading cold, she is a *reader* holding a book; the margin note is
a pencil in her other hand. A reader's pencil must never fight the
page: typing a note is typing, whatever the keyboard layout, however
many words. And the room itself: the cold read is the product's
strongest ritual, currently entered only through the palette or a
chord nobody meets — a locked front door on the best room.

### D1. The open note owns its keys

**Today:** `space`/`shift-space` are bound to page-flips in the
ambient `ColdRead` context (editor.rs:668-669); the reaction input's
`NoteInput` context doesn't bind them, so a space mid-note flips the
page — which by design commits the note first. One word per note.

**Rule:** *while a note is open, the pencil owns the hand* — stated
structurally, not as a two-key patch (Raskin): an open note input
owns **all** printable and text-editing keys (space, arrows, the
lot); ambient cold-read bindings act only when no field is focused;
the only keys that reach past the field are its explicit
commit/cancel chords. The comment at editor.rs:19016 ("the open
reaction input owns its keystrokes") becomes true, including for
whatever binding ships next. The mouse obeys the same law: a click
on the page while a note is open resolves the note and is consumed
— it must not also flip the page (the one place where C4's
commit-AND-act becomes commit-only, because the "act" would be a
page turn the reader didn't mean; the panel's unanimous carve-out).

### D2. A layout switch is not a departure

**Today:** the input's `on_focus_out` (editor.rs:18709) treats the
transient blur fired by an input-source/layout switch as a real
departure and files/closes the note mid-word.

**Rule:** a corollary of LAW 2: *only a writer's gesture resolves a
field; a focus event with no gesture behind it is never a
departure.* The mechanism (builders own it): a short time-based
grace (~100 ms) on blur, cancelled if focus returns to the same
field; app deactivation (alt-tab mid-note) never commits. The law
covers the whole field family — the margin composer, rename, goal,
and checkpoint fields fire the same transient blur on a layout
switch, and for a bilingual writer layout switches happen in
*every* field, in every session. Verified against a real
input-source switch, not just a synthetic blur.

### D3. One pencil, everywhere

Cold-read reactions are ordinary margin notes in the same store
(editor.rs:18807 → document.rs:2323). The input follows the margin
composer's conventions — multiline, same commit/cancel keys — one
grammar for "writing a note to the text" (P8). (The formatting
flank inside composers — the P3 gap `asides.md` already names —
remains deferred; this round only aligns the input surfaces.)

### D4. The reading room gets a doorknob

**Today:** entry is the palette row + `ctrl-shift-l`. Nothing on
the chrome says the room exists.

**Rule:** a resting place, not an advertisement (P2's drill notch,
P5's floor). A pictorial-family icon button joins the titlebar's
right cluster between the history clock and the editor button —
the reader's mark (an open book). Its tip is the action phrase
("Read it cold", with the chord), a carrier sentence in an action
surface, lawful under P4/the glossary. Because `ReadItCold`
already toggles, the control is the indicator (P12): inside the
cold read the same control shows the active state and clicking it
leaves — the door pair's law, applied to the reading room. The
other titlebar controls keep their current in-mode fates (word
pill and editor button hidden, "— reading" beside the name); the
book control is the one that stays live, so the mode always
carries its own exit (P6: any still of the cold read shows the
lit control that leaves it). Titlebar mechanics: `flex_shrink_0`,
only the editor button truncates; a new bespoke SVG joins the icon
plate. Build order: D1/D2 land before the doorknob — the door
opens onto a room with working furniture.

## 5. What this round refuses

- No formatting flank inside composers yet (named gap, own round).
- No content-anchored graveyard origins (B non-goal).
- No zoom/animation/motion work; every change here is either input
  grammar, structure, or dress.
- No new settings. Every rule above is one behavior, not a toggle
  (P2 — a preference is the tool asking the writer to configure
  its manners).
- **No history-dependent span behavior** (panel-added, closing
  A3's question): a mark's edge semantics derive from its *type*,
  never from how it was applied. Two visually identical spans may
  not differ in behavior by invisible history — per-span end
  polarity / disarm-on-apply is refused, reopenable only on
  Strop-native dogfooding evidence after the class split ships.
- Named, deferred, not forgotten (Norman): nothing on screen
  signifies the caret's armed inline state ("will my next character
  be bold?") — the flank readout (A1) answers it only while a
  selection lives. A caret-state indicator is a future P12
  question, not this round's.

## 6. The critic panel — verdicts and adjudication

*(Five independent lenses, 2026-07-11, each reading the draft cold
against the constitution: Raskin (modes, habits), Norman
(signifiers, action loops), Cooper (posture, excise), Tognazzini
(Fitts, latency, accessibility), and the corridor stranger (P5's
Russian-blogging persona). No section was rejected; every section
but A2/D3/D4 drew amendments; all amendments above are theirs
unless noted. The record:)*

- **Unanimous catch — A1×C2 oscillation:** the draft's A1 was a
  level-triggered invariant and C2 an event; with the selection
  alive under a fresh composer, A1 re-raised what C2 dismissed.
  Resolved as LAW 1 (derived predicate + latch). This was the
  draft's biggest internal defect.
- **Raskin's deep catch — expansion on machine insertions:**
  `expands()` fires on any insert, so B2's wound survived the
  draft's own fix for partial entries (and paste). Resolved as the
  A3 machine-insertion law.
- **Data-loss guard (Raskin, Norman):** the seam rule as drafted
  permitted an implementation that amputates marks when Enter
  splits a styled run. Resolved: the seam kills momentum, never
  marks.
- **B1 normalization (Cooper, Tognazzini, corridor):** triple-click
  selections already include the trailing newline; "consume one
  more" would fuse neighbors. Resolved: normalize, then ensure the
  outcome.
- **Undo atomicity (four lenses independently):** whole-block exile
  and its separator must round-trip through plain Ctrl+Z as one
  step (P13).
- **C4 split exits (Norman) + commit-AND-act (Cooper, Raskin) +
  pre-commit hit-testing (Tognazzini) + empty-discard (Cooper):**
  all folded into C4.
- **One true critic conflict, adjudicated:** Raskin demanded the
  flanks never touch keyboard routing; Tognazzini demanded
  keyboard operability (WAI-ARIA). Resolution: passive raise is
  pure display (Raskin wins the default); the explicit `ctrl-.`
  summon focuses into the flank (Tognazzini wins the deliberate
  path). Escape is never claimed by the flanks.
- **A3 question closed against disarm-on-apply** (Raskin, Cooper,
  corridor, Tognazzini concurring; the convention research had
  recommended disarming): the panel judged invisible per-span
  history a worse wound than visible, reflex-cancellable
  continuation. Recorded in §5 with its reopening condition.
- **Adopted from single lenses:** settle constant fixed at 250 ms
  (Raskin/Tognazzini); flank readout of applied state, P12
  (Tognazzini); put-back reveals its landing (Norman, Tognazzini);
  graveyard form shows block-ness via the anchor-fragment grammar
  (Raskin); C3 messenger time grammar (corridor, Cooper); cold-read
  click-while-note-open is commit-only (Cooper, Norman, Tognazzini);
  D1 before D4 in build order (Raskin); off-screen selection raises
  no flanks (Cooper).
