# Critique — "Folds" (`fold`) · interface panel

*Birman / Raskin / Norman, 2026-07-05. Design was blind to prior art by
construction; we fault only re-invented failures, never the ignorance.*

## Ilya Birman — grammar

The design spends its one metaphor twice, and the seam shows. "Fold" is
the *boundary* verb (creates the region); "Unfold" *dissolves* it into
manuscript. But pleating open and shut — the *view* act — wears the same
picture and gets no verb at all. So the still image of an open fold
shows a thing lying visibly unfolded on the page, carrying a button that
says **Unfold** (P6 fails the screenshot test; P12's control-as-
indicator contradicts its own state). Two parallel meanings — *close
this* and *make this manuscript again* — do not get parallel forms
(P8); one is a click on a glyph, the other a labelled verb, and the
labelled verb reads as the click's synonym. Corridor test: show an open
fold, ask "what does Unfold do?" I predict a split between "closes it"
and "nothing — it already is." That confusion sits on a destructive-ish
boundary change.

Second: the crease row. Date, count, **Unfold**, **Exile** — persistent
per-item chrome resting on the writer's *living* text. asides.md §5
refuses exactly this ("item chrome at rest: buttons, handles, counts,
badges on compost items"), and the reason is P3, not taste: the
writer's things never become widgets. The graveyard's entry buttons are
lawful because the graveyard is the machine's read-only record; a fold
is compost-natured — deliberate, warm, editable — so the refusal
applies. Surgery: verbs move to the selection flank and palette; the
crease keeps only data (`folded 12 Jun · 214 words`).

Third, a standing-decision conflict: replace applies inside folds. The
prior round ruled Replace-All mutates manuscript only (07-compost §3),
and the *reason* was consistency — bulk mutation respects the boundary
like every other bulk operation (counts, export, AI). The fold's
counter-reason (folds are the writer's text) is arguable, but breaking
the one-law-for-all-bulk-ops rule needs an amendment, not a footnote.

Earned praise: the collapsed one-liner extending the graveyard's
receded-entry contract is real parallel-form-for-parallel-meaning;
warm-vs-drained carries living-vs-dead exactly per P10; graveyard
Put-back re-creating *the fold* at origin honors P13's same-door law.

## Jef Raskin — modes

The kill: **Esc from an open fold "restores the caret to its last
manuscript position."** That is a state whose meaning depends on
invisible history — the definition of a mode. Which "last"? Ten minutes
and three screens ago. Scroll (don't click) to a distant fold, open it,
edit, press the deeply habituated go-home key: the viewport leaps
somewhere the writer cannot predict. The prior art fought this exact
battle and built the cure — the excursion latch (07-compost §3, F3
bullet; critique record R3: "never raw caret position, so Esc inside a
deliberately-edited compost item doesn't teleport"). The blind design
has re-invented the pre-latch failure. The jot round-trip (F2) is
correct *as a quasimode* — chord in, Esc out, within one breath; it is
wrong as a universal rule for every open fold.

Smaller: `ctrl-shift-a` is two verbs on one chord (selection = park,
none = jot). Selection state is visible, so this is lawful gesture
polymorphism. The spring-loaded find fold is a genuine quasimode,
correctly shaped — but specify the tear: if the writer clicks into a
sprung-open fold, leaving find, does it stay open? Unspecified is how
modes are born. And fold-as-atom quietly rewrites the surface's oldest
habits at one line of the screen: a habituated deletion sweep whose
span includes a pleat relocates the fold's entire text to the tail
record. Lossless (P13) is not the same as intended; the design names
this risk itself, honestly, and it is real.

## Don Norman — first contact

Minute 0: no folds — the screen is identical to today. The empty state
is literally nothing; the strongest resting footprint in this exercise.
Minute 3: she selects a limp passage to delete it; the flank offers
**Fold** beside **Exile**. The invitation (F8) is the best-placed of
any direction — the gentler verb at the hand's exact position at the
exact moment the cut hurts. She presses it: the text pleats into one
warm line, her own first words, `· 214 words` — compliance visible at
the wound (F1, genuinely beats baseline).

The misreads: (a) the pleat glyph is invented; the corridor knows the
disclosure triangle. P5 says familiarity is *borrowed* — a stranger may
read a pleat line as an epigraph or subheading, and a short jot shown
whole ("check the ferry timetable…") as manuscript. That inversion is
dangerous: she believes a visible line exports. Scope is announced only
at moments (caret-count, export summary); between them it hangs on a
wash and a novel mark. (b) Mid-paragraph park — the commonest one: the
paragraph closes over the gap and the fold settles *below*. The bet's
headline ("provenance is structural; Unfold is F4 at origin") quietly
fails here: Unfold drops the sentence below the paragraph as a stray
block and she restitches by hand — *weaker* than graveyard Put-back,
which targets true origin. Scorecard's "F4 beats" is overclaimed. (c)
Month 3, Pressfield-grade: forty pleats striate the draft. Writers park
partly to get cuts *out of sight*; the fold's conviction keeps every
darling one line under her nose, and the only fold-free view is
read-only cold read. The design names this loss well — but it loses
precisely the heaviest practitioners of the practice it serves, and its
cure (curate folds to the graveyard) is the maintenance the practice
research says writers never perform.

## Panel findings

The gathered lens flirts with the two-homes failure (07-compost §2:
"two representations, one thing") but stays lawful: summoned, transient,
a viewspec not a panel. The tail-region decision (07-compost option B)
is *not* re-invented failure: its reason was F5's granite ("the
document opens on the manuscript") and folds meet F5's letter by scope
rather than geometry — a legitimate alternative answer. The fold's F6
story ("parked stays where you left it; dead lies at the tail") is
arguably *more* predictive than one-tail-zone. Chirality (asides.md §4)
is moot in-column.

**Verdict: needs-surgery; beats the null baseline.** It closes baseline
failures 1, 3, 4, 5, 6 at zero resting chrome — but the Esc mode, the
Unfold grammar, the crease chrome, and the mid-paragraph overclaim must
be cut out first, and the striation loss honestly caps who it serves.

**Kill-shots:** (1) Esc-from-fold teleports on invisible caret history
— re-invents the failure 07-compost's excursion latch fixed (F2/F3).
(2) "Unfold" labels a visibly open fold; boundary verb and view act
share one metaphor (P6/P8/P12). (3) Crease row is resting item chrome
on living writer text (P3; asides.md §5). (4) Mid-paragraph parks
cannot restitch on Unfold — "F4 at origin" overclaimed; inverse weaker
than Put-back (P13/F4). (5) No editable fold-free view; the heavy
composter loses to the baseline's confined pile (F5-spirit).

**Grafts:** the Fold-beside-Exile invitation; the zero-footprint empty
state; caret-scoped count readout (P12); the announced split-scope find
tally with spring-open-while-current; bless-the-existing-pile migration
(formalize the writer's bottom dump in place rather than teleporting
it).

*(~1,160 words)*
