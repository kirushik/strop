# Critique — design-spartan.md ("The Spartan"), interface panel

Panel: Birman / Raskin / Norman. Verdict up front: **needs-surgery, beats
the baseline.** The blind designer independently landed on the tail
placement that the prior round (docs/impl/07-compost.md §3B) chose after
an extraction audit — manuscript, then living scraps, then the dead
record, aliveness descending; cold read and export ending at the seam.
That convergence is the strongest evidence yet that the tail is right.
But the design re-invents the audit's two top-ranked failures and hangs
its soul on a mechanism the constitution's oldest wound forbids.

## Ilya Birman — grammar

**The stamped sentence is P1's founding wound re-opened.** P1 was born
from re-entry v3: machine-authored text appearing in the writer's space
("Someone changed my text"). ⇧⌃A writing *· · · the piece ends here · · ·*
into the document is the machine writing prose into the manuscript —
the design owns this as its "one real amendment" (§6.5), but it is not
an amendment, it is the wound. Worse, the mark is a **system template
living as writer text**: P8 forbids system prose swallowing writer
strings; here the inversion — writer-editable prose *is* the chrome.
An editable control whose identity is a magic string has no widget
contract at all (P7): edit one inner word and the boundary silently
dissolves, with "nothing to error" by the design's own admission.

**The count chip breaks its own borrowed contract.** "N · the piece"
as scope-announcement-as-data is excellent P4. But copying the
graveyard chip's hide-while-visible contract (§3, "mirrors the
footer-chip contract") onto a *data indicator* means the word count
vanishes whenever the mark is on screen — i.e., precisely where writers
finish drafts. Parallel form was given to non-parallel meanings: a
navigator may hide when its referent is visible; a count may not
(P8, P12). Testable: caret in the last paragraph → no word count
anywhere on screen.

**One verb, two actions.** "Set aside" means *remove from the piece*
in prose and *bump to fresh* inside the yard (§3). The design claims
"same meaning everywhere"; it isn't — one relocates across the
boundary, one reorders within it.

Praise where due: the one-sentence scope law ("for the audience / to
the machine ends at the mark; the writer's hands never do") is better
grammar than the per-surface lists in 07-compost §3B, and *Set aside*
matches the glossary's audited verb (ux-glossary, aside row).

## Jef Raskin — modes and monotony

**Typing is no longer monotonous.** The same gesture — typing a line of
prose — sometimes types and sometimes restructures the document. Type
"the piece ends here" as a line anywhere (quote it in an essay about
endings; paste a draft that contains it) and everything below silently
becomes scraps: count collapses, AI goes blind, export truncates. No
confirmation is possible because "there is nothing to error." A
content-triggered global mode-flip is the textbook invisible mode.
The first-match rule compounds it: whether *this* line is the boundary
depends on whether a matching line exists above it — meaning by
invisible global history.

**Esc's meaning depends on how you arrived.** Chip-jump arms a return;
scrolling there does not. The prior round hit exactly this and built
the excursion latch (07-compost §6, R3: set by travel, cleared by any
manuscript click) so Esc inside a deliberately-edited scrap doesn't
teleport. The Spartan specifies neither arming nor expiry — a
re-invented ambiguity, already solved.

**No keyboard travel verb at all.** Skim is mouse-only (the chip). The
standing designs offer ctrl-shift-o (07-compost) or a held quasimode
(asides.md §2.1); the Spartan resolves that open arbitration by
omission, which is not a resolution — F3 for a keyboard writer loses
to the baseline's ⌘F.

Genuine credit: at rest there are no modes, no panels, no focus traps —
everything is document text, and the ground-paint makes the
piece/scraps state visible at every scroll position. That part passes
the screenshot test cleanly (P6).

## Don Norman — first contact, minute by minute

Minute 0: she opens her essay. Nothing new anywhere — correct (P2).
Minute 3: she selects a flabby paragraph. The menu offers *Set aside*
above *Exile* — the pairing at the pang moment is the best invitation
mechanism on the table (F8). She strikes ⇧⌃A. **Her paragraph
vanishes.** The prose closes over; a footer number she wasn't watching
ticks down. To a stranger this is deletion with a euphemism. The design
concedes it: "a writer who misses the tick must scroll or click to
trust the verb once" (§6.2). F1 demands compliance *visible from where
the writer sits*; the shipped park flashes three surfaces
(compost-review extraction, flow 1). A first use that requires an act
of faith is how verbs die.
Minute 6: she finds the mark line. It reads as a typographic ornament.
Later, tidying, she may delete "that decoration" — the yard rejoins the
piece; if she's since scrolled away, the only witnesses are elsewhere.
And the writer who already keeps a `---` divider — the baseline
incumbent — gets nothing: her convention isn't recognized, and no path
short of the exact shibboleth phrase upgrades it. "Discover it from the
other end: type the sentence" is knowledge-in-the-head fantasy; nothing
in the world suggests that sentence is special.

## Panel synthesis — prior art, reasons, verdict

Re-invented failures, cited: **F2 jot** re-creates the extraction
audit's #1 papercut — the only jot path is transit-through-prose
(type mid-sentence, re-select precisely, chord), which 07-compost §2
marked F2 ✗ and §3B fixed with a no-travel **New scrap** verb precisely
because "the travel verb does NOT solve discovery" (critique N1/N2).
**F4 retrieve** re-creates papercut #2: the dead pile keeps lossless
Put back while the living pile's only return is cut/paste —
delete-plus-insert, which by the design's own assumption 3 kills
margin-note anchors and (per extraction, flow 4) loses spans. Set aside
ships with no same-grammar inverse: P13, and 07-compost §3B already
named the fix (*Move to the manuscript*).

Contradictions of standing decisions, with reasons: **Replace-all
pierces the boundary** — 07-compost §3B scoped Replace All to the
manuscript with an announced count because silent rewriting of the
scrap box was found in use (papercut #5) and bulk mutation must respect
the same boundary as every other bulk op. The Spartan's writer's-hands
law is a principled counter; but it must at least *announce* yard hits,
not claim "nothing to announce." **Notes anchored in scraps** — the
standing gating law (07-compost: "notes, diagnoses, set-aside don't"
work in compost) exists because the ungated ⌃M chord was a found bug
and the margin is the conversation about *the text* (asides.md §4).
The Spartan's counter-reason (anchors must ride along on park) is
good enough to reopen the question — as an argued amendment, not by
ignorance.

**Kill the string, keep the skeleton.** Recognition-by-content dies;
a structural seam stamped by the verb (per-state versioned, as
07-compost §3B already commits to) keeps every scope win. Baseline:
**beats** — F1/F5/F6/F8 wins are real and nearly chromeless; the null
baseline cannot scope export, counts, cold read, or the reviewer, and
those are the disease.
