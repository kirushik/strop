# Design brief — a home for out-of-manuscript text

Fresh slate, 2026-07-05. **For blind designers: this file and the three
research digests beside it are your ONLY sources.** Prior designs for
this feature exist in the product's repo; you must not see them. If a
product fact you need is missing here, make the smallest reasonable
assumption and NAME it in your spec.

## 1 · The product, in four sentences

Strop is a writing tool for one writer working on one prose document at
a time — essays, talks, chapters; pages, not books. The manuscript is a
single continuous column of text, always saved, with full local
history. An AI reviewer exists but is caged by design: it diagnoses
only (cards beside the text), never writes, never wants. The interface
creed: minimal, intuitive, empathetic, respectful — every surface
operable at first sight by a stranger, depth revealing itself in use,
never in dialogue.

## 2 · The question

Writers produce text that does not belong in the manuscript:

- **seeds** — ideas not ready *yet* (a premise, an ending, a beat, a
  phrase worth keeping);
- **parkings** — passages cut from the draft but *no longer* dead-sure
  gone;
- **asides** — notes to self that *never* were and never will be
  manuscript.

Design, from scratch, the home for that text. Nothing is decided: not
the place (an edge drawer? a panel? a parallel surface? a region of the
document? no place at all?), not the verbs, not the name (working name
in this brief: *the scrapyard*; propose your own).

The second half of the question is **the invitation**: most writers
never kept a scraps file; the design should make the practice feel
native enough that they start — but the tool never wants anything from
the writer (P2 below), so the invitation must be structural (the right
affordance resting in the right place), never promotional. No tips, no
tours, no empty-state marketing copy.

## 3 · The competitor you must beat

The null baseline is shipping NOTHING. A writer who practices this
method today just types a divider at the bottom of her document and
keeps scraps under it. Zero UI. Always saved, fully editable,
searchable, no new concepts, no discoverability problem (she invented
it, so she knows it). Very Engelbart: one continuous document,
convention over chrome.

Its known failures — i.e. the only places a design can win:

1. Export, word counts, and the AI reviewer all see the scraps as
   manuscript: the reviewer diagnoses her scrap pile, the count lies,
   the exported piece ends in her notes unless she remembers to trim.
2. A fresh-eyes read-through of "the piece" includes the scraps.
3. Parking a passage costs: cut, scroll to the bottom, paste, scroll
   back, find your place. Expensive enough mid-revision that some cuts
   just get deleted instead.
4. Jotting a stray thought mid-sentence costs the same round trip.
5. No relationship to the tool's automatic deletion record (§4): her
   deliberate scraps and the tool's rescued deletions live in unrelated
   places with unrelated habits.
6. The boundary is convention; the tool doesn't know it exists — so
   nothing can respect it, keep the AI out of it, scope a count by it,
   or preserve it through time travel.

Rules of engagement:

- Every UI element you add must beat this baseline somewhere specific;
  name where.
- A design that merely MATCHES the baseline with more chrome LOSES
  to it.
- "Shelve the feature" is a live, respectable outcome of this exercise.
  Do not strawman the baseline to make your direction look necessary.

## 4 · The editor as it stands — facts you build within

**The manuscript column.** One column, generous measure, warm paper
ground. Nothing is ever drawn on the prose; the machine never writes
into it (P1).

**The titlebar.** Window controls; a centred omnibar (find + command
palette — a real text field); an editor-state control that carries the
current working stance as its label.

**The door / working stances.** The writer explicitly switches between
drafting (generating) and reviewing (evaluating). While drafting,
machine results hold back and arrive at a natural lull; interrupting a
writing burst is forbidden by policy. There is also a **cold read**
stance: a distraction-free reading pass of the manuscript with all
apparatus hidden; reactions typed during it become margin notes after.

**The right margin.** The annotation lane: the writer's own notes
(warm, anchored to passages) and the AI's diagnosis cards (cool,
anchored to passages). Over budget, older cards recede to one-liners.
Notes whose anchor text is deleted migrate rather than vanish. The
margin is periphery; it never steals focus.

**The bottom edge.** The history strip: a seek-bar-like time control
over the document's checkpoints. Clicking parks you read-only in a past
state; a banner names where and when you are; attempted edits pulse the
banner; Esc returns to now; Restore appends the past state (itself
reversible).

**The graveyard** (shipped fact — your design must relate to it, F6).
The automatic safety record of deletions: substantial text the writer
deletes (and anything sent by the explicit exile verb, ctrl-shift-g) is
recorded losslessly and rendered as a READ-ONLY record section at the
document's very tail — drained neutral colour, full text, older entries
receded to expandable one-liners, whisper-quiet verbs — plus a footer
chip that navigates there and hides while the section is on screen.
Every entry offers "Put back": lossless, to its origin. The graveyard
is the *automatic, dead* afterlife: a record, not a place to write.

**Verbs & keys.** Selecting text raises a two-flank menu: formatting on
one flank, verbs on the other, each verb wearing its chord chip
(annotate ctrl-m; a lossless move-selection-out verb ctrl-shift-a
exists — its DESTINATION is yours to design; exile-to-graveyard
ctrl-shift-g). Esc is the universal "go home" key (it leaves find
restoring your selection, leaves the parked past, leaves cold read); it
is deeply habituated — extend it, never repurpose it.

**Text mechanics** (P3). Everything the writer owns is text: click
gives a caret, typing types, the same formatting works wherever the
writer's words live. The writer's things never become widgets.

**Colour law** (P10). Warm = the writer's; cool blue = the machine's;
drained neutral = stale/dead; sage = resolved/returned; red = errors
only. Each colour speaks once — and never alone (form and position
corroborate it for the colourblind).

**Persistence & time.** One local file holds everything; history is
checkpoints, each materializing a full document state; the strip
re-opens them. Consequence: any structural boundary ("this part is not
manuscript") must be stored PER STATE, so old checkpoints keep their
own geometry and restoring one never teleports text.

**Scope machinery.** Word counts, export, find/replace, and the AI
reviewer's scope are all defined over "the document" today. If your
design makes some text non-manuscript, specify every scope explicitly —
and how the writer SEES each scope (announced, never silent).

**Canvas.** Desktop, one window, viewport ~800–1600 pt wide. Overlays,
docked columns, edge drawers, extra in-document regions are all
buildable. A second OS window is out of scope. One document per window;
the scrapyard is per-document in v1 (no global pile).

## 5 · The flows — requirements, from real usage

- **F1 Park.** Mid-manuscript, a passage stops belonging. One gesture;
  the caret does not travel; compliance is visible from where the
  writer sits; lossless and reversible.
- **F2 Jot.** A thought arrives mid-sentence that was never manuscript.
  Capture must cost as near zero flow as possible — it happens while
  drafting; the caret comes right back, or never leaves.
- **F3 Skim & return.** While revising, the writer reads through her
  scraps hunting for the piece that fits, then returns exactly to her
  writing position. A cheap round trip, no residue.
- **F4 Retrieve.** A scrap re-enters the story at the writer's
  insertion point, structure intact. (Evidence says retrieval is much
  rarer than parking — weight accordingly — but the inverse must exist,
  P13.)
- **F5 Never intrude.** Opening the document lands in the story. Cold
  read, export, counts, AI scope: manuscript-pure, and visibly so.
- **F6 One afterlife.** Your scrapyard (deliberate, living, editable)
  and the graveyard (automatic, dead, a record) must read as one
  coherent spatial story — a writer should predict where cut-vs-parked
  text went without thinking.
- **F7 The three tenses.** Seeds / parkings / asides: serve all three
  in one home, or explicitly split them, or explicitly cut one — defend
  whichever you choose.
- **F8 The invitation.** A writer who never kept a scraps file starts
  keeping one, because the affordance rested in the right place at the
  right moment — never because anything asked her to.

## 6 · The constitution, condensed (P1–P13)

Cite these by number. Violating one without amending it kills a design.

- **P1 The text is sovereign.** Nothing is drawn ON the prose; the
  machine never writes into it. The software may *record* the writer's
  text verbatim and may *relocate* it as still-editable text; it may
  never decorate it, quote it rhetorically, or wear it as chrome.
- **P2 The tool never wants anything from you.** No software-initiated
  prompts, reminders, invitations, congratulations, or mentoring.
  Every capability has a calm, findable resting place — the notch on
  the drill handle — and discovery happens in use, not in dialogue.
- **P3 Everything the writer owns is text.** Click gives a caret,
  typing types, formatting works everywhere the writer's words live.
  Writer things never become widgets; widgets are the machine's shape.
- **P4 Show, don't explain.** Interface text is data or an actionable
  label — never a description of an affordance. If an element needs
  prose to be understood, redesign the element. One sanctioned channel
  for craft vocabulary: the carrier sentence on an *action* row.
- **P5 The corridor floor and the notch gradient.** Operable at first
  sight by a stranger because it looks like something already known;
  depth rests where a curious hand falls, revealed one notch at a time;
  nothing about the depth may tax the floor.
- **P6 The screenshot test.** Every frame of every transition makes
  sense as a still image; position and progress get static encodings.
- **P7 Widget contracts: extend, never subvert.** A control borrowing a
  common widget's face honours that widget's contract. Safer-than-
  ancestor is lawful; fake nature is not.
- **P8 UI is grammar.** Parallel meanings get parallel forms; one
  action keeps one verb everywhere; system templates never swallow
  writer text (writer strings are set off by typography, never inlined
  into system prose).
- **P9 Hover only expands.** Hover may enlarge what is visible; it may
  never carry sole meaning.
- **P10 Colour speaks once.** Colour carries provenance and state (§4);
  what colour says, words do not repeat; what colour says, form also
  says.
- **P11 One anchor object per surface.** Exactly one thing the
  returning eye lands on; the contrast budget is spent there.
- **P12 The control is the indicator.** State is shown by the control
  that changes it — never displayed in one place and changed in
  another.
- **P13 Every verb has an inverse in the same grammar.** Nothing
  destroys silently; nothing is rescued through a different door than
  it left by; the writer can infer the inverse without being told.

## 7 · Your deliverable

Write `design-<your-slug>.md` with exactly these sections:

1. **The bet** — one paragraph: why this place and shape are right for
   this text.
2. **The scene** — a screenshot-test description of the design at rest
   and in use: what a stranger sees, and what they correctly conclude.
3. **Behavioural spec** — states, verbs, keys, entry/exit, focus and
   Esc grammar, the empty state, the first-discovery moment, the F8
   invitation mechanism. Assumptions named in a list.
4. **Edge cases** — find/replace scope; AI scope; export and counts;
   checkpoint time travel (boundary versioning); cold read; margin
   notes anchored in scraps; a 3,000-word scrap pile; a jot arriving
   mid-drafting-burst; narrow (800 pt) and wide (1600 pt) windows.
5. **Scorecard** — F1–F8 versus the null baseline: beats / matches /
   loses, one line each. "Matches" is expected and honest.
6. **Named losses & risks** — the strongest argument against your own
   design, stated as well as its enemy would state it.
7. **Build sketch** — one honest paragraph (one-column retained-mode
   UI; structural boundaries must version per checkpoint state).
8. **The name** — what the feature is called, in the product's plain,
   warm register.

Max 2500 words. Every sentence load-bearing.
