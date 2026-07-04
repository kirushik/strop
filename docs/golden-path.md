# The Golden Path — how a piece gets written with Strop

*(2026-07-03. The frame document for the writing–editing–checkpointing arc.
Evidence: six research dossiers in
`docs/research/writing-lifecycle-2026-07.md` [cited below as A–F], on top of
the 2026-06-14 core-loop dossiers. Method note at the end. Status legend used
throughout: ✅ shipped · 🔶 specced in docs · ⭕ missing → gap register §6.)*

## 0. What this document is

Strop's design so far models the **session** (the door, the reveal clock, the
intent ritual) and the **mechanism** (checkpoints, history, cards). What it
never modeled is the **piece**: the two-week arc from a contest announcement
to a submitted story. This document walks that arc twice — once for a SciFi
contest story, once for an opinion column — extracts the phase model, and
records the higher-order decisions so that downstream questions ("what is a
checkpoint?", "how many editorial modes?") become consequences instead of
debates.

Two rules govern everything below:

- **The golden path is a default, not a rail.** The craft literature shows
  multiple *viable* processes — multi-draft, cycling/one-pass, formula-outline
  [A]. The same surface behavior (re-editing page one) means opposite things
  in different schools, so the tool may never infer what phase the writer is
  in, and nothing on the path may be a gate. The path is what Strop makes
  *easy and legible*; every exit stays open.
- **Honest labels.** Claims below are marked research-backed only when they
  are; the rest is craft consensus, and Strop's docs and copy must never
  launder one into the other [C, D]. Where our earlier docs got a citation
  wrong, §8 corrects it.

## 1. The spine: five convergences

The full versions with citations are in the compendium header; the design
leans on these five, each found independently by ≥2 dossiers:

1. **Estrangement, not elapsed time, is the active ingredient of "cooling."**
   The author is the maximally-predicting reader of their own text; the one
   direct study found author-blindness *survived* a two-week delay. What
   writers actually do — print it, change the font, read aloud, send it to an
   e-reader — manufactures unfamiliarity. Time is one path to estrangement;
   re-presentation is another, and it fits deadlines [A, B, C].
2. **Phases are writer-performed, never tool-inferred.** Cognition: writing
   is recursive at the minutes scale; phases are defensible only as attractor
   states. Tools: imposed pipelines are the market's clearest failure story
   (Sudowrite Story Engine); scaffolds that rescue novices tax experts [C, E, A].
3. **No school lets evaluation touch an incomplete draft.** Even *human*
   feedback waits for a complete draft in every tradition. The lab-backed
   core: premature surface polish taxes generation (WM competition); total
   evaluation lockout is a stance, not science — so this shapes defaults, not
   locks [A, C, B].
4. **The deadline is the engine; displays bend, never shame.** Behavior
   anchors to deadlines (r=.82), not plans (r=.23). Broken streaks leave you
   below baseline; recalculating-forward targets are the only deadline design
   with warm user sentiment. The ADHD time-perception deficit (Tier A) makes
   *externalizing the deadline* the highest-value scaffold per pixel [D, E, A].
5. **Pre-prose and post-prose material already live inside the one document.**
   Composting, idea files, "notes at the bottom of the file," the
   graveyard/manuscraps pattern: folk practice keeps the not-manuscript in
   the manuscript's file. Dignify the pattern; don't build a binder [A, B, E].

## 2. Walkthrough I — the contest story (two weeks)

*4,000-word SciFi story, themed contest, hard cap, deadline in 14 days. "You"
throughout; feature annotations inline.*

**Days 0–2 · Sparks (P0 — ignition).** You read the announcement on the tram.
Nothing happens in Strop yet — and that is correct: ideas arrive off-keyboard,
and the tool's only jobs are *capture speed* and *zero judgment*. You open a
new document that evening ✅ (sub-second, no questions asked — the open-time
invariant) and type fragments: two premise candidates, an image, one line in a
voice you like. These are not prose; you mark them as **compost** ⭕(G4) — set
off from the manuscript, excluded from word count, passes, and export, but in
the same file where you will trip over them tomorrow. You also state the one
project-level fact worth stating: *deadline, July 17* ⭕(G3). The door is
closed ✅; no AI exists anywhere on this path yet, and won't until you ask
(pull-only ✅).

You discard your first premise on purpose — under a theme prompt everyone has
the first idea [A]. The craft's transition signal out of P0 is precise and
non-obvious: **you don't start drafting until the ending exists** — the
strongest documented predictor of a draft that finishes (Chiang, Kowal,
Moorcock) [A]. Strop doesn't test for this; it just gives the ritual a place:
when the ending arrives in the shower on day 2, you write it into the compost
and start drafting the next morning.

**Days 3–7 · The draft (P1 — generate).** Four sessions. Each opens on your
own last sentence, caret restored, with your own words in the banner: "Next:
write the confrontation at the airlock" ✅. Session goal `+800` if you want it
✅. The door stays closed: the margin holds only *your* notes (`ctrl-m` —
"check: does the shuttle have windows?") ✅; if you run a believing pass
mid-draft for morale, its results park through your typing bursts and land in
a lull ✅. The typing itself is hours, not days — Kowal drafted a Hugo
nominee in 90 minutes [A]; the calendar time is gestation between sessions.
When stuck, you take a walk (incubation is real but small, d≈0.29, and
minutes-scale — that's a walk, not a drawer [C]).

Each session ends with the ritual: "Next session I will ___" ✅ — the
best-anchored feature in the product (implementation intentions d=0.65; a
specific plan quiets unfinished-goal intrusions [C]). The end-session moment
also seals a checkpoint carrying the intent 🔶(R2a) — today the sentence
feeds the banner and evaporates, which the checkpoint brief already indicts.
The runway stays in done-so-far framing ("day 5 · draft growing") ⭕(G3) and a
missed day renders as *nothing* — not a deficit, not a broken chain [D].

*The variation points, because the path is not a rail:* the cycling writer
loops back over yesterday's pages in creative voice every session and never
has a revision phase — nothing in Strop resists this; the door simply stays
closed until they ship. The outliner did their structural work in compost
during P0. Both are golden paths [A].

**Day 8 · The seal (P1→P2 transition).** The draft is complete — every school
honors "finish what you start" [A]. You perform the transition: **seal the
draft** — a named checkpoint, "Draft complete," one act ✅ (mechanics) /
🔶(R2a: the record should know what it *is*, not just its name). This is the
project-level analogue of the door: a stance you take, not a state the tool
detects (D1). The story rests overnight — for short fiction the craft's rest
is days, not King's six weeks, which is book-scale [A; §8 errata].

**Day 9 · The cold read (P2 — the estranged reader).** The one phase no tool
on the market serves, and the one Strop should own ⭕(G1). You open the story
in a **reading presentation**: different type, different measure, page-shaped
— the manuscript deliberately made to look like someone else's ("an alien
relic"; the mechanism is prediction-breaking, and it's the best-attested
transition trick in the whole literature [A, C]). The caret is gone; reactions
land as margin notes, not edits — "drags here," "I don't believe her yet,"
"THIS is the story" — which are exactly the `ctrl-m` marginalia the revision
will be built from ✅. When you finish, you ask the editor to read it the same
way: the **believing pass** ✅ — center of gravity, what's alive, what the
piece is secretly about. (The believing pass turns out to be a P2 instrument
that was shipped without knowing it [F].) You now know what you wrote.

**Days 9–12 · The descent (P3 — evaluate, altitude by altitude).** Now the
door opens and the shipped machine does what it was built for: a
**developmental pass** ✅ against the sealed draft — structure, stakes, the
ending's landing — with the altitude order holding copy-level noise back ✅.
You cut the subplot: the two scenes go to the **graveyard** ⭕(G4) — same
primitive as compost, at the tail — because exile grants the permission to
cut that deletion doesn't [E]; the machine never re-suggests exiled text (red
line). For the contested middle scene you **try it both ways** 🔶(R3). Every
surgery is insured: a checkpoint seals before each pass 🔶(R2a), restore is
loud about being non-destructive 🔶(R2c), and history can answer "where was
this still good" by *meaning* — the cards open then, the intent then —
🔶(R2b), not by timestamp. When the structure stops moving, a **line pass** ✅.
Your own cold-read notes resolve alongside the editor's cards; done and
dismissed, they fade with the grace the margin already knows ✅.

**Day 13 · The fit and the finish (P4).** The cap is 4,000; you're at 4,430.
Cut-to-fit is a distinct late mode with its own craft (structural cuts before
line trims; King's −10%) [A, B] — the word count leans against the cap
⭕(G5-cut), the trims go to the graveyard, and a **copy pass** ✅ sweeps what
survived. You read it aloud — the estrangement engine's second use ⭕(G1) —
which has real evidence for surface errors and craft consensus for rhythm
[C]. The tool's over-polish guard is a mirror, never a verdict: "you've
re-edited this sentence four times; drift from the sealed draft is rising"
⭕(G6) — your own churn shown to you, because *doneness belongs to the writer
alone* (Saunders' needle is self-diagnosis; there is no scientific stopping
rule to borrow [A, D]).

**Day 14 · The ship (P5).** Export to the contest's format — standard
manuscript format, anonymized for blind judging — ⭕(G7; today only Markdown
✅). A last named checkpoint: "Submitted to XYZ" ✅ — the provenance record
that will answer, months from now, *which version did they read*. The runway
retires. The story is out; the file keeps the whole fortnight in its history,
including the graveyard, because rejected stories have long afterlives
("The Paper Menagerie" was a rejection first [A]) — the arc is not a one-way
pipeline; pieces park, revive, re-target.

## 3. Walkthrough II — the column (and where it diverges)

*900–1,200 words on the chilling effects of the Tornado Cash ruling, for a
niche outlet. Same writer, same tool, ~100× compressed.*

**The reservoir and the peg.** The piece began months ago — reading dockets,
arguing on group chats. When the ruling drops, the clock is the *peg's decay*:
24–48 hours, not two weeks [B]. P0 is an **angle test**: no drafting until a
one-sentence claim exists (the argument's "ending exists" signal). Evidence
arrives before prose — case names, holdings, links — and parks in compost
⭕(G4) or as footnotes ✅. Strop is not a reference manager and must not
become one (refusal §7); it holds *your words and your pointers*.

**The blurt and the same-day distance.** One sitting, loose-then-tight [B].
The cooling gap the story enjoyed does not exist here — and because the
estrangement engine manufactures distance instead of waiting for it (§1.1),
the same cold-read ritual works *the same evening*: changed presentation,
read-aloud, reactions as notes ⭕(G1). The believing pass still goes first —
even an argument needs its center of gravity named before it gets doubted.

**The descent, re-tuned.** The altitudes hold — developmental before line
before copy — but *developmental means something else for an argument*:
unity (one point?), thesis placement, evidence order, **steelman presence**
("who will disagree, and where do you answer them" — the doubting stance the
believing pass mirrors), kicker-echoes-lede [B]. Same engine, form-tuned
prompts ⭕(G5) — a fiction-tuned developmental pass on an op-ed reads as
noise, and trust dies fast. Two lenses exist here that fiction lacks: 30–50%
of the draft *will* die against the cap (cut-to-fit, sunk cost the named
enemy — "what can die without the argument collapsing"), and **claim-strength
calibration** — hedges cut where you're sure, qualifiers added where you
aren't, checkable assertions (names, dates, holdings) verified and protected
from semantic drift during cutting ⭕(G5) [B].

**The boundary Strop states plainly.** Submission, the editor's rewrites, the
headline fight — the endgame happens in email and track-changes, outside the
writer's tool, and pretending otherwise is how tools bloat [B]. What Strop
*does* own: the "Submitted" checkpoint means that when the edited version
comes back as pasted text, **history's vs-draft diff shows you exactly what
the desk changed** ✅ — a real, shipped answer to "diff foreign edits by eye
at 11pm" that nobody planned as such.

**What did NOT differ:** the stances and their sequence (generate → estrange →
believe → descend by altitude → fit → ship), the seal, the graveyard, the
history roles. **What differed:** durations (~100×), the developmental
vocabulary, two extra lenses, and the runway's shape (peg-decay, not
fortnight). One lifecycle, parameterized — not two products.

## 4. The phase model

Phases are *stances the writer takes*, marked by acts they already perform.
The tool holds no phase variable (D1); every row below describes defaults and
framing, never permissions.

| Stance | You're doing | Mind / fear | Transition OUT (writer's act) | Tool today | Tool must NEVER |
|---|---|---|---|---|---|
| **P0 Spark** | capturing fragments, discarding first ideas, finding the ending | diffuse, playful / *losing the spark* | the ending exists → first drafting session | fast open ✅, compost ⭕ | structure you, ask questions at open |
| **P1 Draft** | forward bursts, gestation between; own notes in margin | fragile momentum / *"this is trash"* (KFKD) | draft complete → **the seal** | door ✅, intents ✅, goals ✅, reveal clock ✅ | evaluate you, show yesterday's flaws, deficit-count a quiet day |
| **P2 Estranged read** | reading as a stranger; reacting, not editing; believing pass | detachment sought / *finding out it's bad* | notes exist; you know the center → first dev pass | believing pass ✅, cold read ⭕ | let the caret tempt you, verdict the draft |
| **P3 Descent** | dev → line passes; cuts to graveyard; forks; surgery on structure | analytical, cold-blooded / *breaking it, losing the good version* | structure stops moving → fit & polish | passes ✅, altitude order ✅, history ✅, cuttings 🔶, letter 🔶 | polish what structure will cut (✅ held), bury the safety |
| **P4 Fit & finish** | cut-to-cap, copy pass, read-aloud, title | convergent, tired / *"not good enough yet"* | fits the cap, needle stays up → export | copy pass ✅, count ✅, churn mirror ⭕ | equate more revision with better, verdict "done" |
| **P5 Ship** | format, anonymize, submit, let go | relief + reluctance / *letting go* | submitted checkpoint; the piece leaves | export (md only) ⭕, provenance ✅ | become a portal, keep the writer polishing |

Two structural notes. First, **the loop nests**: within any stance the
session-level GENERATE/EVALUATE oscillation continues (the door is the
session instrument; this table is the project weather). Second, **the arc
reverses legally**: a P4 discovery can reopen P3; a rejection reopens P1 —
recursion is the ground truth [C] and the table is a *tendency*, which is
exactly why none of it may be enforced.

## 5. The higher-order decisions

**D1 — Phases are rituals plus a record, never a state machine.** No phase
variable, no phase names in chrome, no inference from behavior, nothing
locked. A phase exists in three legible forms only: acts the writer performs
(seal, cold read, pass, export — each already a concrete mechanic), the
derived record of those acts (CheckpointMeta — the checkpoint-flow brief is
this decision's mechanism layer), and defaults that follow from *visible
recorded facts* ("no complete-draft seal exists" → the pass menu leads with
believing). *Counterargument honored:* a declared-phase system would be
simpler to build and explain; it dies on dossier A (same behavior, opposite
meanings across schools) and E (imposed pipelines are the market's failure
story). *Consequences:* checkpoints get kinds; the door stays session-scoped;
"which phase am I in" is never a question the tool can be wrong about.

**D2 — The deadline is a declared fact; the runway is arithmetic, not
motivation.** One optional per-document fact (a date, maybe a cap). The
display: quiet, recalculating-forward, framed by the smaller region ("day 4"
early, "3 days left" once the end is near — the framing itself must flip
[D]), lapse-neutral (a missed day renders as nothing), never red, never a
streak, never a quota. It structurally distrusts the writer's schedule:
the arithmetic protects the back half for revision, because the planning
fallacy is the best-replicated finding in the whole review (predicted 34
days, took 55; only the deadline predicted behavior) [D]. *Counterargument:*
"calm tool" and "deadline display" seem opposed — resolved by the Tier-A
ADHD time-horizon deficit: for our writer the deadline being *perceptually
absent* is what manufactures the day-13 panic; externalized gently, it's
calmer, not less. No deadline declared → the feature does not exist.

**D3 — Strop owns the estrangement ritual (the cold read).** The market gap
and the mechanism agree (§1.1). One act enters a reading presentation:
re-typeset (different face, measure, page shape — prediction-breaking),
caret-less; reactions become marginalia; Escape leaves, as always. Its AI
companion is the already-shipped believing pass; its natural sequel is the
editorial letter 🔶(R4), which is the whole-manuscript reading the card
system can't do. Second use: read-aloud posture in P4 (TTS is the evidence-
adjacent tool [C]). *Counterargument:* "a mode!" — it's a session-scale,
writer-invoked lock with Freewrite-grade precedent [E], the exact shape the
research permits; and the presentation machinery (read-only takeover,
re-typesetting) already exists in the history preview. *Consequence:* P2
stops being the empty phase; the writer's own reactions — not the AI's —
seed the revision.

**D4 — The project-level door: evaluation waits for the seal, as a default
that never argues.** Until a draft-complete seal exists, the pass menu leads
with believing and nothing else is *promoted* (everything stays available —
the frequency-ordered palette will follow a writer who diagnoses mid-draft
daily, and that's correct: menu order is the entire enforcement). After the
seal: developmental leads; while dev cards are open, line/copy stay held (the
shipped altitude order, now with a project-level first rung). *Grounds:* the
strongest craft convergence in dossier A — no school evaluates an incomplete
draft — braced by the narrower lab result (surface polish taxes generation
[C]). *Counterargument:* the cycling writer never seals — and never gets
nagged: no seal, no promotion change, no message. Defaults, not doctrine.

**D5 — One new document primitive: the aside (compost & graveyard).** A
region the writer marks as *not the manuscript* — excluded from word count,
passes, and export; visible, foldable, in the file. Head-asides are compost
(P0 fragments, the parked ending, evidence links); tail-asides are the
graveyard (cuts with a return address). One primitive, two mouths of the
lifecycle, and the Cuttings drawer 🔶(R3) becomes its curated tail rather
than a new surface. *Grounds:* triple-attested folk practice [A, B, E];
exile-grants-permission-to-cut [E]; the single-document bet needs an answer
for the not-manuscript or writers keep it in another app. *Counterarguments
taken seriously:* (a) scope creep toward a binder — held off by the rule
that an aside has no identity, no tree, no metadata, it is just *marked
text*; (b) engineering cost in packer/passes/export — real, which is why
it's sequenced after R2; (c) the red line: the machine never reads compost
for suggestions and never re-surfaces the graveyard — preservation only.

**D6 — Passes are altitude × stance × form; the menu shows one.** Altitudes:
developmental / line / copy (shipped). Stances: believing / doubting — the
believing pass exists; the doubting stance is the steelman/"who will
disagree" instrument, and it is the *argument form's* developmental core.
Form (story / essay-column) tunes prompt content only — the fiction
developmental vocabulary (stakes, POV, the ending's landing) and the
argument's (unity, thesis placement, evidence order, steelman, kicker) are
different words for the same altitude [B]. Form is declared once, optionally,
or never (default prompts stay form-neutral); it is a prompt parameter, not a
mode. Late lenses — cut-to-fit and claim-strength/fact-check — are P4
instruments keyed to a declared cap and to checkable-assertion spans [B].
*Consequence — Kirill's question dissolves as intended:* we don't ship N
modes; we ship 3 altitudes × 2 stances with small form-tunings, and the
palette leads with exactly one recommended pass derived from the record (D1),
everything else one fold away.

**D7 — Done-ness: mirrors and anchors, never verdicts.** The tool surfaces
the writer's own signals — churn on the same sentence, rising drift from the
sealed draft, the fact that the last two passes returned nothing new — and
the external anchor (the deadline, the cap). It never says "it's done," and
it never says "it isn't." *Grounds:* doneness has no external criterion and
the entire scientific canon on stopping rules is one Valéry quote [A, D];
Gaiman's asymmetry (readers right about *wrong*, wrong about *fix*) is
already Strop's core stance applied to time. *Consequence:* the anti-tinker
"nudge" from the old backlog is re-specified as the churn mirror — facts
about my own behavior, in my own margin, at my request.

**D8 — Re-entry is the framing surface, and framing is the cheapest
high-evidence intervention.** Eight minutes of "revision means global"
reframing measurably changed revision behavior [C]; the intent banner is
already the tool's re-entry voice. So the banner (and the pass menu under
it) carries the *task schema*, derived from the record: after the seal it
says the draft is resting and offers the cold read; after a developmental
pass it frames the session as structural, not sentence-level. Never advice,
never praise — orientation. *Consequence:* the "re-entry on-ramp" backlog
item merges into this; no new surface is built.

**D9 — Checkpoints are the lifecycle's ledger.** The checkpoint-flow brief
(`docs/writing-editing-checkpointing.md`) stands, reframed: its
CheckpointMeta is how D1's rituals become D1's record; its narrative history
list is how the arc becomes legible in retrospect; its loud-restore language
is the safety that makes P3 surgery fearless; the seal and the submitted
checkpoint are its two project-scale kinds. One addition from this round:
**labels as vocabulary** — a checkpoint may carry a writer-stamped tag
("draft 2", "as submitted") with zero behavioral consequence, the
screenwriting colored-page trick minus the crew [E].

**D10 — The refusals, project edition (§7)** — held as hard as the margin's
red lines, because every one is a documented failure mode somewhere else.

## 6. Gap register (ranked: leverage × evidence × fit)

| # | Gap | Serves | Grounds | Cost | Sequence |
|---|---|---|---|---|---|
| G1 | **The cold read** (estrangement presentation; reactions-as-notes; read-aloud posture) | P2, P4 | §1.1 triple convergence; market gap [E] | M (preview machinery exists) | with/after R2 |
| G2 | **R2 CheckpointMeta + narrative history + loud restore** (the brief, + end-session & pre-pass seals) | X, all transitions | brief + D1/D9 | M | **next** (already drafted) |
| G3 | **The runway** (deadline fact; bend-don't-shame arithmetic; phase-flipped framing) | P1–P5 | D2; Tier-A grounds [D] | S–M | after G2 (reads the record) |
| G4 | **Asides: compost & graveyard** | P0, P3, P4 | D5; folk triple [A,B,E] | L (model + packer + export) | after G2; absorbs R3's drawer |
| G5 | **Form-tuned prompts + late lenses** (doubting/steelman; cut-to-fit; claim-strength & checkable-assertions) | P3, P4 | D6 [B] | M (prompt work + spans) | independent |
| G6 | **The churn mirror** (writer-own signals; drift vs seal) | P4 | D7 | S–M | needs G2's seals |
| G7 | **Ship-shape export** (docx/standard MS format, anonymized; cap compliance) | P5 | last-48h logistics [A] | M | independent, unglamorous, high trust-value |

The editorial letter 🔶(R4) is not a gap — it's the specced whole-manuscript
instrument that slots between G1's cold read and the descent; its market
validation arrived with this round (Marlowe [E]).

## 7. Refusals (project edition)

No imposed pipeline or phase inference (E's clearest failure story). No
streaks, quotas, deficit displays, red days, or fresh-start prompts (D's
strongest triangulation). No completion verdicts in either direction (D7).
No reference manager, no submission tracker, no binder/corkboard — the
not-manuscript gets one humble primitive (D5) and the endgame beyond export
is honestly out of scope [B]. No AI first-move at any scale: session (shipped
law), pass (pull-only), project (D4 is a menu order, not a prompt). No
machine reuse of compost/graveyard — preservation only. And no laundering
craft consensus into "research shows" (§0) — including in our own marketing.

## 8. Errata for earlier docs

- **Woolf's "wet brush" quote** (2026-06-14 dossier, §4a): unattributable per
  the VW Society misquotations page — stop using it. The point it decorated
  (retype rather than copy-edit to defeat anchoring) stands on Darwin's
  authority-of-existing-text argument alone.
- **King's six-week drawer**: book-scale by his own text; short-fiction rest
  is overnight-to-days [A]. Don't port it.
- **Zeigarnik**: as a memory effect, refuted (2025 meta-analysis); keep only
  Ovsiankina's resumption tendency + plan-quiets-the-loop [C].
- **Amabile time-pressure diaries**: downgrade to "high pressure does not
  reliably help" — the working paper never passed peer review and a 2023
  meta-analysis codes its effects near zero [D]. attention-motion.md's uses
  survive (they lean on interruption studies, not the pressure claim).
- **Self-imposed interim deadlines improve performance** (Ariely &
  Wertenbroch): failed its preregistered replication [D]. The *demand* for
  deadlines is real; never cite the performance claim.

## 9. First review round (Kirill, 2026-07-03) — verdicts and amendments

**D3 (cold read): endorsed, extended.** Static *numbered* flippable pages,
different face and measure, possibly the subtlest paper texture. Two
unifications adopted: the same paged viewer serves as the **history
checkpoint preview** ("read this version" — in-window takeover, not a popup:
window management is clutter, and the read-only takeover pattern already
exists); and **entering a cold read quietly checkpoints first** (unnamed,
fingerprint-guarded — every cold read starts from a recorded state, which
also gives the churn mirror its reference point). The SEAL stays a separate,
explicit writer act — declaring a draft complete and wanting to read are
different things.

**D2 (runway): SHELVED.** Possibly useful, net-goodness unproven. G3 leaves
the build sequence; the deadline stays a fact the writer keeps outside the
tool until testing says otherwise.

**D5 (asides): direction accepted, UX genuinely open → mockup round.** The
open questions: in-flow marked region (above/below a marked line, foldable)
vs any side-by-side presentation (fights the no-panels instinct; might fight
scroll fatigue better); whether the natural CUT gesture should feed the
graveyard directly; and the trust framing — the graveyard is the *visible*
insurance for writers who don't trust invisible history (the commented-out-
code pattern among devs who don't trust git, dignified instead of shamed).
No existing UX preconditioning for either region (Word's page-area segments
are the anti-precedent).

**D4 (pass menu): the passes must explain themselves.** The names
(developmental/line/copy/believing) don't carry intent in menu space.
Direction: verb-question entries with one-line "when" explanations and a
why-line on the recommended one — the state machine lives in the
recommendation TEXT, never in a diagram or a new surface.

**D7 (churn mirror): accepted; the surface is the question.** Candidate
homes to mock: a margin heat line (ambient — risks being a permanent
judgment strip), the history rows (drift-vs-seal per checkpoint), the pass
menu's why-line ("copy ran twice today; nothing new"), and — most promising
— heat marks in the COLD READ margin only: the mirror shows where evaluation
already lives, never while writing. Mirrors are pull-surfaces; there is no
active intervention.

**D8 (re-entry): was unclear; the concrete form.** No new surface. The
intent banner (which already greets every open) gains record-driven text
variants, and the pass menu order follows: (1) mid-draft → today's behavior
("Next: write the airlock scene"); (2) after a seal → "The draft is resting
— sealed yesterday, 4,120 words. Read it cold?"; (3) mid-descent → "
Structural pass in progress — 3 developmental queries open." Three
sentences, driven by CheckpointMeta. That is the entire feature; the
evidence (8 minutes of reframing changes revision behavior) rides on what
those sentences SAY.

**D9 (checkpoints): two additions adopted.** (1) **The Ctrl+S reflex is
honored, never retrained**: Ctrl+S = fingerprint-guarded unnamed checkpoint
(no change → no checkpoint) + a subtle-but-certain acknowledgment. (2)
**Export auto-checkpoints** (kind: Exported), and the label derives from the
export filename's non-document-name part — "ferry-story-contest-final.md" →
label "contest-final": the writer names the version in the save dialog
without knowing they're naming a version. **Open sizing question** (flagged,
not resolved): reflex checkpoints × materialized states = real bytes; the
candidate policy is disclosed age-thinning of unnamed checkpoints (named/
seal/submitted never thin), but the arithmetic and the "nothing is lost"
guarantee need an engineering round together.

**New lifecycle fact honored:** the director's cut. Pieces get re-opened
months after submission and re-edited to different editorial requirements
(online → print). The record already supports it (the "as submitted" label
is the anchor; a re-target is just new work after it) — no new feature, but
walkthrough II's boundary section should not read as "the file goes dormant
forever."

**Feature audit (his question "what never appeared on the path?"):** the
outline rail never earned a scene in either walkthrough — short forms often
have no headings at all; its DESIGN §4.3 destiny (the beat list that ticks
off) folds naturally into COMPOST (a plotter's beats are compost items;
ticking one is resolving it), so the rail stays as cheap long-document
navigation and stops pretending to be the structure-holder. Voice-drift's
σ-glyph remains an instrument-orphan (correctly deferred); highlight,
underline, auto_copy_selection are harmless substrate. Footnotes appeared
only as the column's evidence parking — consistent with their import-
fidelity origin.

## 9.1 Second review round (Kirill on UX-lab v1, 2026-07-04) — verdicts

**Cold read.** Page numbers + flip-zone shading: endorsed. Corrections
adopted: (1) the typeface shift (Palatino-class vs the editor's Georgia-class)
is probably NOT a strong enough estrangement stimulus on its own — v2 offers
graded strengths (mild serif shift / slab / typewriter) to taste-test;
(2) the "justified + hyphenated" claim was a stale lab note — the build
deliberately went ragged-right after the Kindle-sin research; the real
editor owns its hyphenation, so justified+hyphenated stays open for the GPUI
implementation; (3) mock page breaks read as arbitrary because they were —
real pagination reflows; (4) texture concept liked, execution rejected: the
dot-grid's REGULARITY is the problem — v2 uses irregular noise
(feTurbulence-class); (5) typing-attempt feedback: no toast — PULSE THE
EXISTING BANNER phrase instead (no overlapping surfaces, ever);
(6) **churn heat is EVICTED from the cold read**: reading mode cultivates the
reader's eyes, and "you edited this ×14" yanks the writer back into writer
stance, contaminating the estrangement. It moves to writer-stance surfaces:
a summonable editor LENS (find-match-style tint, palette-invoked, P4) and/or
history rows; (7) history-preview banner: emphasis flipped — the checkpoint's
NAME (+ date) is the payload, "viewing a checkpoint" is chrome and goes muted.

**Asides.** (1) The side-panel compost variant as mocked was broken and fairly
lost — but the variant question stays open with a better candidate: the LEFT
rail (structure territory, where the outline rail lives); (2) checkboxes in
compost REJECTED — they break the equality of editing mechanics and drift
toward grocery-list tooling; struck-through text is the beat-done idiom (the
strikethrough format exists); prose must never grow native clickable
checkboxes; (3) the fold chip must BECOME the expanded block's header (the
mock read as "two composts"); (4) compost scroll behavior must be explicit:
it is in-flow and scrolls away with the document; (5) the blank-page moment
needs its own empathy round — where compost is explained the first time a
NEW document opens (without violating the zero-questions-at-open law);
(6) graveyard: natural, and READ-ONLY — put-back and delete are its only
verbs (compost is a workbench; the graveyard is an archive); (7) the
selection popover OVER text is rejected on principle — selection actions move
to a right-margin anchored menu at selection height (annotation territory),
which also naturally hosts "ask the editor about this selection" (a flow
Kirill already finds useful); (8) origin labels can't assume headings —
derive: nearest heading, else a quote of the preceding sentence, else a
paragraph ordinal; "show origin" = jump + flash the cut site (to demo);
(9) graveyard overflow: entries recede to one-line headers as they age (the
margin lane's recede idiom, reused); find works on it — it's document text.

**Passes.** "Ask the editor" endorsed strongly — and PROMOTED to the
subsystem's single home: it should carry the door state (toggle), the
open/resolved counts, the in-flight "editor is cooking" indicator, and the
pass menu. The popup anchors UNDER the button (never detached, never over
the column). Selection-scoped passes join the right-margin selection menu.
**Cut-to-fit demoted from a pass to a prompt inflection**: the need is niche,
trust in an LLM's cut-list is low, and the useful form ("the scene where she
enters can be simplified") is what a developmental/line pass already says
when a cap exists — the cap becomes context, not a feature. The cap itself
joins the existing goal surface (one "Set a goal…" home; no field zoo — the
deadline stayed shelved for the same reason). Margin cards: the
label/provenance anatomy gets a redesign round (no duplicated "developmental",
provenance always visible at low contrast, not hover-only).

**Ctrl+S / history.** The acknowledgment placement is settled
(titlebar + history-icon pulse >> bottom whisper). The ↺ icon FAILS — read
twice as refresh; needs a history-true glyph. And the big opening: history
navigation itself gets reimagined around "saves every keystroke" — a
pseudo-playback scrubber, inspiration from NLE/DAW timelines, bottom panel,
color-coded edit kinds against the plain black-on-white text ("the history
rewind is an explosion of color-coded technical abstraction, still
navigable"). A dedicated UX-lab round with several takes; research
commissioned (Etherpad time-slider, Docs' old revision slider, History Flow,
video/DAW scrubbing conventions).

**Re-entry: REJECTED as mocked; redo from fundamentals.** The named defects:
(A) the banner puts software voice INSIDE the writer's text territory — the
only surface where Strop fails to keep its opinions in the margins;
(B) "NEXT" and friends were tool-invented kickers — only the writer writes
in that space; (C) the state labels ("mid-descent", "in progress") task the
writer with our idiosyncratic bookkeeping, or worse, assert software
assumptions as states. Plus the grammar failure: one surface mixed a noun
phrase ("Believing read"), a suggestion ("Read it cold"), and a command
("Return to the open queries") — **"UI is grammar"** enters the fundamental
principles (research commissioned on menu-grammar guidance). The v2
direction: prose territory carries ONLY writer-authored words (the shipped
intent sentence, attributed as theirs); derived facts live in machine
territory (titlebar, the editor button, menu ordering) phrased as FACTS;
the confidence rule — the less certain the software's inference, the less
the UI may differ between states. Also "Copy — held while structural queries
are open" read as veiled critique of the writing; held-states must point at
the QUERIES, not the prose (e.g., "after the structural queries settle").

**Meta.** Mockups must simulate the living editor (caret, realistic text) on
every desk scene — a dead page reads wrong even as a sketch.

## 9.2 Third review round (Kirill on UX-lab v2, 2026-07-04) — verdicts

**Two new fundamental principles, named by Kirill, adopted:**
- **The screenshot test.** Every frame of an in-transition animation must
  make sense if captured as a still. (Extends attention-motion's "animate
  moves, not pops" — it constrains what may exist MID-move.)
- **Widget contracts: extend, never subvert.** A control that looks like a
  common widget must behave like one; enriching its content is fine, faking
  its nature is not. Logged as a SHIPPED bug under this principle: the
  titlebar search/palette control is a fake button that spawns a separate
  popup field instead of becoming the field with the dropdown glued to it.
  Same sin at small scale in lab v2: the editor-button menu rendered
  detached from its control. Both fix directions: the control IS the field/
  anchor; attached, flush, aligned.
- Corollary reaffirmed: **hover may only EXPAND visible data** (relative →
  absolute dates), never carry sole semantics — no orientation without a
  mouse, no comparing two hover-values.

**Cold read.** The BOOKISH face (Bookman-class) wins. And the fake dies:
if the page imitates a book, it hyphenates like one — the cold read goes
**justified + real hyphenation** (we own the layout engine; Knuth-Liang
pattern crates exist in Rust; ragged-right was the mock's browser-limitation
compromise, not a design). Texture, banner-pulse, checkpoint-name-led
history banner: accepted. Two openings noted: **footnote placement on
pages** is nontrivial (the classic pagination/float problem — its own
research item inside G1), and margin reactions need visible anchor links in
the real implementation.

**Asides.** The right-margin selection menu: accepted — next markup
iteration should RHYME it with the left-side formatting affordance (one
screen showing both flanks of a selection). Responsive behavior: textual
spec suffices for now. **Compost pivots from prose to CARDS** in a left
rail: "weird ideas, clippings, elements and todos" — the wall-of-text
metaphor was wrong for it, and narrowness stops hurting once items are
cards. The two-card-families cognitive-load worry gets this candidate
resolution: the split is ANCHORED vs UNANCHORED, not left vs right — a card
is one grammar; anchored cards live in the margin at their anchors (notes +
queries), unanchored material lives in the left rail (compost); the
graveyard entry is a card whose anchor is a PAST position. Text supremacy
stands: the prose column concedes nothing.

**The graveyard footer (Kirill's design, adopted whole).** A long document
can't rely on a tail section alone: a quiet sticky FOOTER chip carries the
graveyard's presence — click scrolls to the section; an exile blinks it,
ticks its counter, and offers an immediate undo; "show origin" RETAINS the
segment in the footer (small/truncated) while the main text scrolls to the
cut site; and when the actual graveyard scrolls into view the footer
UNSTICKS and becomes the section's own header — smoothly, passing the
screenshot test at every frame.

**The editor button.** v2's lane showed AI cards in drafting/cooking states
— a mockup error; the door's law stands (cards rest behind the rail).
Wording: "1 read waiting" is jargon — "**a read is ready**" / "2 reads
ready"; never "5 new" (no read-state exists, by design). Cards: "done"
gains the dotted-underline affordance of an in-text action; done (=
addressed) and × (= dismissed, teaches suppression) must stop looking like
accidental siblings — the card-anatomy round owns this.

**Re-entry, third pass — the intent may not deserve to be an ENTITY.**
Kirill's challenge: there is no polite UX moment to force/prompt a
note-to-future-self (popup-on-close = rage-uninstall), the entity's
lifecycle is murky (reappear after dismissal?), and a writer could just
TYPE the note and highlight it — caret+scroll restore (shipped) already
delivers the re-entry. Direction v3: **the intent becomes text** — the
End-Session verb (or plain habit) writes the writer's line INTO the
document (highlighted, at the caret) or as a compost card; no separate
entity, no banner, no lifecycle; the tutorial teaches the practice; the
evidence (implementation intentions d=0.65) backs the PRACTICE, not the
storage mechanism. Open: discoverability without any nudge (Kirill: "no
consistent answer" — candidate: the tutorial + the palette verb).

**The history strip: "the main not-there-yet." Full redesign round.**
v2 critiques, recorded as requirements: bar-height semantics were a mystery
(kill unexplained encodings); rollback-to-any-moment vs snapshots was
ambiguous (answer: ANY moment — the scrub is continuous; stop forcing the
near-continuous stream into discrete chunks); station labels must be
VISIBLE (with a collision policy), not hover-gated; sessions are COMPOSITE
(writing and cutting interleave in one sitting — a single-kind bar is a
lie); density was far too low (test at 3× events, longer text); the
Fingerprint dots were same-size unexplained marks, and localizing "an
editor's pass" to one y-position is semantically false (a pass reads the
whole document); Strata as drawn was nonsense (bands accumulated instead of
interleaving; the vertical axis wasn't position; a restore was
unrepresentable). Tufte's data-ink verdict: underused information density
throughout. KEPT: machine-room dark; live rewind in the main window.
**Method for v3 (Kirill's mandate): the Data Laboratory / infotanka
approach — pick the individual DATA ATOM and build the full picture from
it** (Misyutina's visualization course; 404fest-2013 lecture), plus a
design-icon persona panel (Tufte, Victor, Raskin, Engelbart, Birman) to
ideate and critique. The atom model to build from: text ops {t, pos, ±len}
in one linear stream, plus card events {raised/resolved/dismissed, anchor},
pass events {kind, span}, checkpoint events {kind, name}, restores
{from} — sessions and bursts are DERIVED (gaps in the stream), never
imposed. The strip could be the killer feature riding "we save every
keystroke"; it must be earned, not decorated.

## 10. Method note

This document was produced the way Strop says writing works: a believing
draft of the whole path was written first, blind, with falsifiable
predictions; six research dossiers were commissioned in parallel (A–F); the
draft was then doubted against them. Three of its claims died in the doubting
pass (the cooling *gap* as the active ingredient — it's estrangement; the
drafting phase measured in days — it's hours plus gestation; self-set interim
deadlines as pacing machinery — replication failure). The convergences in §1
are the claims that survived from at least two independent directions. The
believing draft and the diff are in the session record; the dossiers are in
the compendium.
