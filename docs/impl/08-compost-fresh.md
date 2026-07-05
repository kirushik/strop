# Scraps — the fresh-slate round (verdict + amended design)

2026-07-05. The writer's challenge: design the scrapyard from zero — no
left rail, no prior compost decisions — against an explicit competitor:
**shipping nothing** (a writer-kept section under her own divider; "very
Engelbart"). If nothing clearly beats that baseline, shelve the feature
like re-entry. This document is the synthesis; the full corpus (brief,
research, six designs, twelve critiques, three judgments) is in
`docs/impl/compost-fresh/`.

## 0 · Method

A blind-design tournament. Three research agents (web): tool precedents,
writers' actual practice, HCI theory. Six designers, each championing
one direction — **drawer** (edge panel), **verso** (flip the sheet
over), **tail** (in-flow region above the graveyard), **margin**
(scraps as lane citizens), **fold** (in-place collapse, the
boneyard school), **spartan** (the no-feature champion, win condition =
prove we should shelve). Designers read ONLY a fresh brief + the
research — no repo docs, no mockups, no code, and no `design-
principles.md` (its born-from stories leak the old rail); they got a
condensed P1–P13 instead. Then two adversarial panels per design
(Birman/Raskin/Norman with full prior-art access; a Novelist + a
Spartan-minimalist), then three judges (flow / rent / grammar) ranking
all six against shelving.

## 1 · The verdict

**All three judges, independently: winner `tail`, shelve case WEAK** —
including the rent judge, explicitly instructed to prefer shelving.
His words: "failures 1/2/6 are structural — convention cannot make the
tool know the boundary." And the decisive datum: **the blind tail
designer re-derived Option B of `07-compost.md` §3 from nothing but
the brief and the research** — tail region, seam, no-travel verbs, Esc
home, per-state boundary versioning, one downward gradient of
aliveness. The critique panel's own note: "independent convergence is
the strongest evidence yet that the tail is the right place." It also
matches the writer's original instinct ("shouldn't it better be just a
free-flow section?").

Why the others died (each verdict `needs-surgery` at best; full
critiques in the corpus):

- **verso** (fatal, interface panel): a whole-viewport residence mode —
  silent wrong-face drafting puts manuscript prose outside
  export/counts/AI with zero signal (worse than the baseline's visible
  surplus); the dog-ear wears the writer's words as chrome (P1's
  founding wound, verbatim).
- **drawer**: parked option C resurrected unanswered — the product's
  most modal object (second focus/find/Esc flow) for a rent of
  "side-by-side at 1600pt"; jot chord-only (the Word-Spike death);
  no re-read ritual → collector's-fallacy pile behind a closed edge.
- **margin**: inverts the researched emotional function — darlings
  stay in peripheral view at the rewrite site, so writers delete
  instead of park; heavy-parker geometry collapses the rationed lane;
  no gathered sweep.
- **fold**: gives every tense the aside's inline geometry against the
  research fault line (cuts go to piles; zero of 15+ inventoried
  writers keep cuts in place); Pressfield-ratio striation (~40 folds
  per 5,000 words) with no editable clean view, curable only by
  curation writers demonstrably don't perform.
- **spartan**: the geometry is right but the mechanism is a
  content-triggered invisible mode — a magic-string line whose typo,
  quotation, or deletion silently re-scopes count/AI/export with no
  error state possible.

The baseline was beaten — by the design that *is* the baseline,
dignified: the tool learns where her scrap line is and starts
respecting it.

## 2 · The amended design ("Scraps")

The blind tail spec + the surgery all its critiques demanded + the
grafts the judges voted across from the losers. Supersedes
`07-compost.md` §3.B where they differ; asides.md idioms cited where
they stand.

**The shape.** Manuscript → **the scrap line** (a hairline seam: quiet
label left, live word-count of the region right) → **Scraps** (the
writer's living pile: warm face at a *smaller size* — every line
self-identifies; full ink same-size was killed by the mid-pile
screenshot test) → the graveyard slab → the dead record. One downward
gradient of aliveness; byte 0 is the manuscript's first line; the
document opens on the story. Post-veto caution: the region wash is its
OWN token — visibly distant in value and form from the note-card cream
and the amber selection tint (region-wide wash vs bordered card vs
inline tint); the full warm-family revisit the writer floated is
parked to the taste round.

**The boundary.** One structural node in the document model — not a
character, not typed, never backspace-deletable; caret arrows across it
like a paragraph break; selections span it but never capture it.
Materialized **per checkpoint state**: past states keep their own
geometry, pre-seam states show no seam, Restore appends a state with
*its* seam. Text never teleports across the boundary by time travel.

**Empty state: nothing.** No seam, no chip, no region — a document with
no scraps is pixel-identical to today. The seam is born by the first
park/jot and **evaporates when the region empties** (the first park's
inverse, P13).

**Park — Set aside, ctrl-shift-a with a selection.** Rests **one row
above Exile** on the verb flank — the gentle verb at the hesitation
over a deletion; this adjacency is the entire F8 invitation (zero
promotional surface). The text departs losslessly to just under the
seam (newest nearest the story, ageing downward toward the record —
arrivals land at the station the chip opens on; append-at-end is the
named veto alternative). Caret, scroll, selection remnant untouched.
**The receipt is an arrival, not a decrement** (spartan graft, Norman's
minute-3 kill): the block visibly departs toward the foot in the
shipped re-pack slide grammar (cross-fade under reduce_motion) and the
Scraps chip pulses as an event — the graveyard's blink-and-tick idiom
inherited. Writer-initiated motion is lawful mid-burst (the reveal
clock governs machine deliveries only).

**Undo is the reflex inverse.** Park and jot are single history atoms:
immediate ctrl-Z restores the text verbatim and evaporates a just-born
seam; a jot in transit can never land in a checkpoint state as
manuscript. This is also the recovery for the stale-selection misfire
(intended jot, forgotten selection → a park you *watch happen* and undo
with one key).

**Jot — the capture line is SHELVED (post-veto, 2026-07-06).** The
writer's counter holds: *why not type it first and send it later?* —
and the mockup itself showed the indefensible frame (two blinking
carets; the caret is where typing lands, and multi-cursor stays in
programmers' IDEs). The steelman — no manuscript transit, no lull
checkpoint catching a stray thought mid-flight — wasn't worth a new
surface with weak discoverability. The flow that ships: Enter, type
the thought as its own paragraph, ctrl-shift-a — with no selection the
chord parks the caret's paragraph, which departs in the arrival
grammar while the caret lands back at the join. Zero new surface,
zero new verb, still one undo atom. History remembering that a
thought once occurred mid-manuscript is accepted as harmless.

**Typing and pasting in the pile (post-veto).** Entering the pile
directly is first-class — click in, type, paste; it is ordinary text.
Item identity keeps asides §1's blank-line model: a scrap is a
contiguous run of blocks; a blank line ends it; whatever is typed or
pasted below a blank line is a new scrap. Enter once breaks a line
inside a scrap; Enter twice starts a sibling. A small paste mid-scrap
extends that scrap; a paste containing blank lines lands as several.
No item-surgery UI — no split or merge verbs (the writer's 80-20
call); deleting the blank line between two scraps merges them as
plain editing (the two-provenances corner goes to the interaction
pass). Identity is geometry, never widgets (asides §5).

**Skim & return — the excursion latch** (07 R3, re-derived and
re-demanded). The Scraps chip (and ctrl-shift-o) travels to the seam;
**Esc returns home only when the tail was entered by chip or find this
excursion**; a caret placed by scroll-and-click makes the tail plain
text with Esc inert — Esc never moves a live caret out of text the
writer walked into herself. **Both ends remembered** (session-scoped):
after Esc-home, the next chip press resumes where the skim stopped, so
an iterated scrap-vs-passage hunt never restarts a 3,000-word pile.

**Retrieve — Move to the manuscript** (07's verb, now aimed at the
evidenced case: retrieval is ~1–2%, phrase-grained, memory-triggered).
A selection in Scraps offers it on the verb flank; the text lands at
the writing position the latch remembers, **arriving selected** (verso
graft — the selection is the feedback and the immediate undo/move
handle), one history atom. **Put back** (to origin) survives as the
parity verb inside the expanded provenance line — same grammar as the
graveyard's, reusing margin-note anchor migration; never the headline
mechanism.

**Provenance, made quiet** (the standing asides.md §5 refusal upheld —
no item chrome at rest). A parked block's origin one-liner (*from
"…first words" · 2 Jul*) renders in the margin **only while the caret
rests inside that block**; otherwise the pile is clean text. Jots bear
no provenance — parkings and asides self-distinguish and F7 is served
without ever asking for a classification.

**Scopes — the one-sentence law** (spartan's graft, adopted verbatim):
*surfaces that speak for the audience or to the machine end at the
scrap line; surfaces that are the writer's hands never do.* So export,
counts, AI passes, and cold read stop at the seam; caret, typing,
formatting, find, and history don't. Find announces its split — "7 in
the piece · 2 in scraps." **Replace All stays manuscript-only** (07 R5
stands: bulk mutation respects the boundary like every other bulk
operation; the blind design's seam-crossing replace is rejected), scope
announced, a live match in scraps told why. **The count control wears
its scope by caret region** — "piece · 1,842" / "scraps · 312" (drawer
graft, P12) — which also makes any mid-pile screenshot
self-identifying. Seam arithmetic stays legible: chrome count + seam
count visibly sum.

**The gating law, amended openly** (07's per-verb list superseded):
the writer's annotate (ctrl-m) now works in Scraps — P3's own demand
("wherever the writer's text is, the writer's tools are") outranks the
old blanket ban; notes park and travel with their block, and a
put-back carries its notes home. AI diagnosis never crosses the seam —
no cool card anchors below it, visible by the lane simply ending.
**Parking a diagnosed passage retires its card; Move to the manuscript
re-arms it for the next pass** (margin graft — machine artifacts never
follow writer text out of the machine's scope). Below the seam the
flank swaps Set aside for Move to the manuscript; Exile works (a
deleted scrap falls one level, into the record).

**Chips — two, rhymed** (07 B3/N5 stands; the blind design's one-chip
merge was its worst sentence — exile feedback must name the dead
destination, and the shipped graveyard chip's contract is not for
subverting, P7). Scraps chip warm with its count, pulsing on arrivals;
graveyard chip drained with its mark; each hides while its own section
is on screen; neither exists before its section does. The
"descending chip" (one chip, two stops) is named-rejected for the same
feedback-lies reason.

**Adoption & migration, two paths.** (1) A writer with her own divider
pile selects it and presses ctrl-shift-a once — the tool has learned
her scrap line; no detection, no prompt (the round's best P2 sentence).
(2) Documents from the shipped compost-at-top era migrate once,
live-doc only, one honestly-recorded transaction; historical
checkpoints keep their own era and restoring one never teleports text
(07 N3 stands).

**Naming — the round's recommendation.** Two blind designers
independently named it **Scraps**; the glossary already flags
«компост» as a translation comedy risk. The kit: **Scraps / the scrap
line / Set aside / Put back / Move to the manuscript**, and the line
that teaches the whole ontology: *"Scraps live; the graveyard
remembers."* "Compost" can survive as craft vocabulary in prose;
the writer arbitrates what the UI says.

## 3 · Delta vs 07-compost §3.B (what the veto is really about)

Kept from B: the place, the seam, opens-on-manuscript, per-state
versioning, two rhymed chips, tap-travel + excursion latch, Move to
the manuscript, warm-smaller face, Replace-All manuscript-only,
one-transaction migration. New or changed:

1. Jot: the at-caret capture line was proposed here, then SHELVED at
   veto — F2 ships as type-then-park (the chord with no selection
   parks the caret's paragraph; the caret stays at the join).
2. Park/jot are single undo atoms; ctrl-Z is the reflex inverse; a
   just-born seam evaporates on undo.
3. The receipt is an arrival (departure slide + chip pulse), never a
   silent count change.
4. The scope rule is one sentence (audience/machine vs writer's hands)
   instead of a per-surface list.
5. Count control is caret-scoped ("piece · N" / "scraps · N").
6. Provenance renders only under a resting caret; jots bear none.
7. Retrieval arrives selected; find scope announced as a split count.
8. AI cards retire on park, re-arm on return.
9. Annotate works in Scraps (gating law amended, P3-grounded).
10. Empty state is literally nothing (no standing chrome at zero).
11. The adoption gesture for existing divider piles.
12. Newest-under-the-seam ordering (veto alternative: append).
13. The name: Scraps (arbitration: vs Compost).
14. The 07 travel arbitration can close: across 24 agents, nobody
    championed asides.md §2.1's held-quasimode; the excursion latch was
    independently re-derived twice. Recommendation: tap-travel + latch.

## 4 · The challenge, answered

Against the six named baseline failures: scope trespass (1), cold-read
leakage (2), and the unknowable boundary (6) die with the seam; the
park round trip (3) dies with Set aside; the jot round trip (4) dies
with type-then-park (one chord, caret at the join) and with typing
straight into the pile; the unrelated afterlives (5) die with the gradient and
the rhymed chips. F7 honestly **matches** the baseline (one pile,
classification deferred — exactly her pile). Everything else beats it,
and the empty state ships zero standing chrome, so the cost of the win
to a writer who never uses it is nothing — pixel for pixel.

## 5 · Named losses

- **No adjacency, ever.** A scrap can't sit beside the passage being
  revised (Berthoff's facing page). The round trip at Esc-cost is the
  bet; if real usage shows sustained compare-while-writing, this
  direction has no answer short of a different one. Named, accepted.
- **The seam must be flawless text mechanics** — it is the only
  structural object living in the writer's text plane; one bad
  caret/paste/backspace edge at that line and the region reads as the
  tool disputing her page (the P3 trust the whole bet rests on).
- **Pile distance grows.** Intra-tail navigation is scrolling, as in
  the baseline; a huge pile pushes the record farther down. The floor
  is matches-the-baseline; folding is explicitly not v1 (receding
  writer-editable text would make her words behave as widgets).

## 6 · Intent echo (the veto point)

*Scraps are the editable text at your document's own tail, under a
scrap line the tool knows and respects: export, counts, the AI, and
cold read end at the line; your caret, typing, find, and history never
do. With no scraps, nothing exists — the first Set aside (one row
above Exile) creates the line; a stray thought is typed as its own
paragraph and parked by the same chord (no selection: the caret's
paragraph departs, the caret stays at the join); typing or pasting
straight into the pile just works (a blank line starts a new scrap);
emptying the pile dissolves it; ctrl-Z is always the instant inverse. Parked text visibly departs toward the
foot and the warm Scraps chip pulses; the drained graveyard chip stays
its own. The chip takes you down; Esc brings you exactly home (only
from excursions — a caret you walked in yourself is just text); the
next visit resumes where you stopped. A scrap moves back by "Move to
the manuscript," arriving selected at your writing spot; "Put back"
survives for origin returns. Your notes work in scraps; the machine's
cards never follow. Old checkpoints keep their own geometry. It is
your bottom-of-the-file pile, finally respected — and if you keep one
already, selecting it and pressing Set aside once teaches the tool
where your scrap line is.*

**Arbitrations — resolved at veto (2026-07-06):** (a) the name is
**Scraps** (adopted in the writer's review); (b) newest-under-the-seam
stands unvetoed; (c) the travel question is closed — tap-travel +
excursion latch; (d) the jot chord is moot, the capture line shelved.
The writer's own words fix the mental model for the record: Scraps =
deliberate, counted, visible, living; the graveyard = almost-by-
accident, "undo-plus-plus" that spares digging through history.
Post-veto mandates: the park receipt stays a careful attention-
directing animation; the living-vs-dead visual language is audited
app-wide for consistency; menu richness must survive the Rust build
(the editor-passes dropdown named the current worst offender); the
scrap wash gets its own token, distinct from note cards and selection.

## 7 · The record

Corpus: `docs/impl/compost-fresh/` — brief.md; research-{tools,
practice,theory}.md; design-{drawer,verso,tail,margin,fold,spartan}.md;
critique-<slug>-{interface,practice}.md ×12; judge-{flow,rent,
grammar}.md. Mockup scene: `docs/mockups/scraps-tail-2026-07.html`.
Judge rankings (flow / rent / grammar): tail first on all three;
spartan second on all three; verso last on all three; shelve rated
weak ×3. Grafts adopted: §2 above (each credited inline). Grafts
named-rejected: descending chip (feedback lies about the dead
destination); seam-crossing Replace-All (bulk-mutation law);
face-local find (memory doesn't index by face); standing provenance
chips (asides.md §5); fold-as-park, drawer, verso, margin as homes
(§1). No code until the veto.
