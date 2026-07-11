# Critique — design-verso, "the back of the page" · interface panel

*Reviewed against the brief (F1–F8, null baseline), design-principles.md
(P1–P13), and the prior art the designer was blind to: asides.md,
impl/07-compost.md (the earlier round for this same feature, with its
extraction audit), ux-glossary.md. Blindness is never the fault; only
re-invented failures are.*

## Ilya Birman — interface as grammar

**The living pile wears the dead pile's uniform.** Each scrap opens
with a stamped caption (`cut · 30 Jun · from "…"`) and a resting **Put
back** button. In this product, a stamped entry carrying quiet verbs is
the *graveyard's* grammar — the read-only record. Parallel forms must
mean parallel meanings (P8); here the record's form dresses fully
editable text, so the form lies about the nature. And what *is* a
caption in the text model? If it is text, the writer can corrupt system
provenance with a keystroke; if it is not, the back is editable prose
interleaved with read-only widget rows — P3 broken either way ("writer
things never become widgets"). The verso even names the tell: "the
graveyard already renders stamped records" (§7). Prior art got here the
hard way: asides.md §1 fixed the scrap pile's data model as **one
continuous text**, blank line = separator, "no per-item buttons,
headers, counts, or borders at rest"; §5 explicitly refuses
"explanatory captions on either pile." Round 4's wound: "things on the
left are not the cards; probably just paragraphs."

**Newest-first is a maintained sort.** Arrivals inserting at the top
perpetually disturb the writer's own arrangement at its most visible
point. asides.md §5 forbids auto-sorting the scrap box (it is hers, P2);
07-compost B4: "chronological is the arrival *default* only, never a
maintained sort — the plotter curates the order."

**The mirrored lane re-litigates a settled sentence.** asides.md §4
fixed chirality — left = the writer's workshop, right = the
conversation about the text — precisely *so that* side-of-screen never
becomes a debate again. Verso moves notes to the left for P6's sake,
and now `ctrl-m` yields mirror-image geometry depending on face: one
verb, two spatial outcomes.

**Credit:** the two-verb retrieval (Put back → origin; To the front →
caret) independently reproduces 07-compost R4's resolution — one word
per destination, P8 honoured blind.

## Jef Raskin — modes and habituation

**The bet is a residence mode, and §6 concedes it.** Tone, mirrored
lane, dog-ear are *indicators*, and indicators do not stop habituated
hands — the locus of attention is the sentence, never the ground. The
design's own enemy statement admits "none physically prevents the
error." The prior art already walked this spectrum *away* from danger:
asides.md §2 specced visiting the pile as a **held quasimode** ("a
reflex must cost zero mouse trips"), and 07-compost R1 rejected even a
toggle travel-verb because "a toggle's meaning would depend on hidden
caret state — a mode," leaving tap-travel vs quasimode as the one open
arbitration. Verso leaps past both to full residency — the most
mode-dangerous point on a map the product has already drawn. And the
error inverts F5's cost profile: the baseline fails by *visible
surplus* (scraps leak into export — noticeable); verso fails by
*invisible absence* — manuscript prose drafted on the wrong face is
silently unread by the AI, uncounted, unexported. Testable: interrupt a
flipped writer for ten minutes; count wrong-face paragraphs.

**The spec contradicts itself on stacking:** "modes never stack" (§3)
versus "modes unwind last-in-first-out" for parked-past + back (§4) — a
three-deep Esc chain.

**Esc-to-flip lacks 07-compost R3's excursion latch** (cleared by
deliberate engagement): a writer who has settled in to *write* on the
back gets teleported by a reflex Esc. Return is cheap, but a habituated
key must never surprise.

**The jot field is clean quasimodal grammar** (Enter/Esc, P7) — but one
line: a two-line thought hits Enter and files half of itself.

## Don Norman — first contact, minute by minute

Minute one: manuscript, a small folded corner, blank sliver. A
dog-ear's borrowed meaning is *bookmark* ("I marked this page") or
*page-curl* ("next page") — not "this sheet has a back." She clicks
expecting continuation; the entire viewport swaps to an empty, duller
page. **Her draft is gone.** The only reassurance is a corner sliver
she must notice — and that sliver quotes "the words around your parked
caret": the writer's own prose rendered as the face of a control. That
is P1's founding wound verbatim — re-entry v3 died for dressing the
writer's line as a UX element ("may never quote it rhetorically, or
wear it as chrome"). The empty back is the highest-anxiety frame in the
product, and its cure is forbidden by P2/P4, so it stays frightening.

**F6, walked honestly:** cut text is reached by *scrolling down*;
parked text by *flipping through a corner*. Two afterlives, two
navigation physics. 07-compost §2 scored exactly this shape a failure
("F6 ✗ — different navigation idioms, panel vs footer") and §3B's cure
was ONE spatial grammar: story → scraps → record, one descent,
decreasing aliveness. Verso rebuilds the adjudicated failure with
better prose around it. Face-local find likewise: 07-compost resolved
"Find *navigates* everywhere — you search for your own scraps" (memory
does not index by face; scope the mutation, never the navigation);
verso re-invents the scoped-find failure and even predicts it will "eat
a phrase" (§6). Also: the bottom-right corner is contested real estate
(history strip, graveyard chip); assumption 3 hand-waves the collision.

## Panel verdict

Real wins over the null baseline: F1/F2 mechanics, F5 purity by
construction, full-measure scraps, per-face state restoration. But F3's
"beats" overclaims — the baseline can show scraps and manuscript
*simultaneously* around the divider; verso can never juxtapose (§6
concedes an edge drawer "does that one thing better"). The mode is not
a removable organ: excise the flip and what remains is the tail-region
design that already exists. **Fatal — with organs worth harvesting.**

**Kill-shots:** (1) residence mode / wrong-face drafting, F1·F2·F5;
(2) dog-ear quoting the writer's prose as chrome, P1; (3) captions +
resting buttons + maintained newest-first = record grammar on living
text, P3/P8, asides.md §1/§5; (4) two navigation idioms for the two
afterlives, F6, re-invents 07-compost §2's scored failure; (5)
face-local find, F3, re-invents what 07-compost §3B already resolved.

**Grafts:** the killed-falls / kept-turns two-preposition test for F6;
retrieve arrives *selected* at the insertion point; "capture dies on
the first filing decision" as an F2 acceptance rule, plus the
destination-tone sliver opening under the caret; per-region
caret/selection/scroll/find restoration as the F3 bar; cross-face move
as one undo atom.
