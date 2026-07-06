# Critique — design-tail ("Scraps"), interface panel

**Panel note first.** Blind, this designer re-derived Option B of
`docs/impl/07-compost.md` §3 — tail region, seam, no-travel jot, Esc
home, per-state boundary versioning, one downward gradient of
aliveness. Independent convergence is the strongest evidence yet that
the tail is the right place. What follows kills details, not the
direction.

## Ilya Birman — grammar

**One chip, two afterlives — the worst sentence in the design.** "One
tail, one chip" relabels the shipped graveyard chip to **Scraps ·
2,340** and retargets it at the seam. The prior round decided the
opposite — *two rhymed footer chips* (07-compost §3, B3/N5) — and the
reason was structural, not taste: the two piles differ by nature
(deliberate/living vs automatic/dead), and F6 demands a writer
*predict* where cut-vs-parked text went. Under one chip, an exile
(⌃⇧G) can only announce itself through a chip that names the living
pile — the feedback lies about the destination (P8), and the shipped
control's contract is subverted, not extended (P7). Testable: exile a
paragraph with a full pile; ask where the eye learns it went *dead*.

**The seam's law leaks per verb.** Counts, export, AI, cold read stop
at the seam; find crosses (correct — prior art agrees find navigates
everywhere); but replace-all crossing contradicts 07-compost R5, whose
reason was: *bulk mutation respects the boundary like every other bulk
operation*. "Announced" is not "consistent" — one boundary with
per-verb exceptions is grammar erosion. Likewise margin notes in
scraps contradict the gating law ("formatting works in compost; notes,
diagnoses, set-aside don't"), and the lane below the seam becomes
mixed-citizenship: the cool lane "simply ends," yet machine provenance
widgets live there.

**The provenance chip is a standing refusal, re-invented.**
`asides.md` §5 forbids item chrome at rest on compost items — badges
included — and the reason is P3's round-4 wound: "things on the left
are not the cards; probably just paragraphs." Parking the chip in the
margin is a lawyer's dodge; at rest, every parked block wears a
persistent widget. Worse, it's *drained* — the colour law's word for
stale/dead — pinned beside living, editable text (P10). And its "from
'…'" form invents a second anchor-quote typography where one grammar
already exists (asides.md §2.3, P8).

**Credit where due:** Set aside resting one row above Exile is
sentence-perfect; Put back kept origin-targeting, cleanly dodging the
one-word-two-destinations trap the prior round had to fix (R4); the
seam-count arithmetic (chrome count + seam count visibly sum) is real
grammar.

## Jef Raskin — modes

**Esc-in-tail is the R3 failure, re-shipped.** "Esc from anywhere in
the tail returns to the manuscript, exactly" — 07-compost R3 already
caught this and required an *excursion latch* (set by travel, cleared
by any click into the region), "never raw caret position." Without it:
a writer ten minutes deep in deliberately editing her pile taps Esc —
habituated from find — and is hurled to the manuscript. Double
habituation hazard: Esc closes find-in-scraps, the reflexive second
Esc ejects her. And the excursion has no inverse (P13): nothing
returns her to her tail position. Testable: scroll (don't chip) into
scraps, edit, press Esc.

**The region is an invisible mode.** Scraps render in full ink, same
type, same measure. Inside a 3,000-word pile the seam is off-screen,
so what typing *means* — manuscript or scrap — depends on invisible
history. "Position is the announcement" (the P12 claim) collapses the
moment the seam leaves the viewport; the chip's *absence* is the only
in-frame signal, and absence cannot be read. Prior art carried region
identity in the face — warm at a smaller size (07-compost §3), so
every line self-identifies. P6 fails flatly: a mid-pile screenshot is
unclassifiable. Make the state visible or accept the mode errors.

**⌃⇧A's overload is not the formatting precedent.** Bold-with-no-
selection arms a quasimode *at the caret*; this chord spawns a field
*elsewhere*. Two behaviours, one gesture, discriminated by selection
state — the design names the risk itself (§6) and defers it; capture
gestures must be monotonous *before* habituation forms, not after
telemetry.

## Don Norman — first contact

Minute 0: she opens the document — pixel-identical to nothing.
Correct (P2). But the jot capability now has **no resting place at
all**: the chip doesn't exist until the first park, and the chord is
advertised only on the selection menu, where it reads as *park*. A
writer who never parks can never discover jot — F2's entry path is
exactly the "F2 ✗ — there is no entry path… nothing teaches this" the
extraction audit recorded (07-compost §2), re-invented. P2 *demands*
a findable resting place; P5's floor is missing.

Minute 3: she selects a limp paragraph, meaning to delete it. The menu
offers Set aside above Exile — the gentle verb in the hesitation
moment. She presses it. The text vanishes. Her eye is on the vacated
line; the only feedback is a small chip appearing in the far footer
with a number. No pulse is specified (the design even brags the number
"updates silently"). She may conclude she deleted it. The graveyard's
blink-and-tick idiom (07-compost N4) exists precisely for this moment
and is not inherited.

Minute 10: she clicks the chip, meets her paragraph under a labelled
line — good — beside a *drained* chip. Drained, she has learned, means
the dead stuff further down. Is her paragraph dead? The colour
misteaches at first contact.

Minute 20: she scrolls down to reread her pile, edits a line, taps
Esc — teleported. "Did Esc undo my edit?" She scrolls back to check.
Trust spent.

## Panel verdict

Against the null baseline the design honestly **beats** it: the seam
kills scope-trespass (baseline failures 1, 2, 6), the verb kills the
round trip (3), the composer kills the jot trip (4), provenance links
the afterlives (5), and the empty state costs literally nothing. The
scorecard's self-assessment is broadly honest; F7 "matches" is
correctly conceded.

**Verdict: needs-surgery.** The place, the seam, the gradient, the
name are right — independently confirmed. The surgery list: restore
the two-chip presence; add the excursion latch; give jot a resting
place that exists at zero (palette row and/or an always-present quiet
chip); make the region self-identifying (the prior art's
warm-at-smaller-size face is the tested answer); drop or re-dress the
provenance chip per the asides.md §5 refusal; align replace-all and
margin-note gating with the standing boundary law or amend that law
explicitly.

**Grafts worth stealing even if this text dies:** (1) the adoption
gesture — select your existing under-the-divider pile, one ⌃⇧A, and
the tool has learned your scrap line, no detection, no prompt; (2)
provenance-with-Put-back-to-origin using margin-note anchor migration,
with jots bearing none — F7 solved without ever asking for a
classification; (3) the seam-count arithmetic and the announced find
scope ("7 in the piece · 2 in scraps"); (4) Set-aside-above-Exile as
the F8 invitation site; (5) the naming kit — Scraps, the scrap line,
"Scraps live; the graveyard remembers."
