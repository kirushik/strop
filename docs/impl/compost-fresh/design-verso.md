# The back of the page — the verso

## 1 · The bet

Writers already own this container. Not the drawer or the shelf — office furniture — but the back of the sheet: where drafts have carried scraps for as long as manuscripts have been paper. Berthoff's double-entry notebook institutionalized the facing surface; Fountain's boneyard proved writers accept non-manuscript text inside the one document when output-scoping is automatic; the Reproof survey shows the folk practice is "same document, out of the way." The verso keeps the null baseline's entire virtue — one document, everything text, always saved — and fixes its one structural sin: the tool knows the boundary, because the boundary *is the face*. Nothing on the back can leak into export, counts, AI review, or a cold read — not by filter rules but by construction: those machines simply never look at the back of the sheet. And it is the only shape in the field that gives a 3,000-word scrap pile the manuscript's own generous measure: scraps are read and edited as prose, never as gutter crumbs.

## 2 · The scene

**At rest (the front).** The manuscript column as always — warm paper, right margin lane, history strip along the bottom. One new thing: at the paper's bottom-right corner, a small folded dog-ear, the fold revealing a sliver of a duller, unbleached paper tone carrying the first few words of whatever most recently arrived there — or blank, if nothing ever has. A stranger reads it at sight: this sheet has a back, and something is (or is not yet) written on it. Nothing else changed; the contrast budget stays on the prose (P11).

**In use (the back).** The whole viewport is the other face. Same column geometry, but the ground is the verso tone — still warm (writer-owned, living, P10), visibly duller: the unbleached side of the same stock. The annotation lane is mirrored to the **left** — flipping a sheet mirrors it — so no still frame can be mistaken for the front (P6). Scraps run down the column newest-first, each opened by a hairline rule and a quiet neutral caption — `cut · 30 Jun · from` *"and the dog took the football…"* — or `jotted · 5 Jul`, or nothing where the writer simply typed. Writer strings inside captions are set off by typography, never inlined into system prose (P8). Click anywhere: a caret; typing types; formatting works (P3). The same bottom-right dog-ear now peeks the *front's* brighter paper and the words around your parked caret — the way home, always visible: the one control that flips is the one thing that shows which face you are on (P12).

Stranger's conclusions: *I am on the back of my page. My draft is one click away, exactly where I left it. These fragments are mine to edit. The dated lines say where each came from.*

## 3 · Behavioural spec

**States.** One document, two text sequences: the front (manuscript, with its graveyard record at the tail) and the back (the scrapyard). Both fully editable, same text mechanics. The back is never a widget; it is a page (P3). Each face keeps its own caret, selection, scroll, and find state, preserved exactly across flips; flipping moves focus to the arriving face's caret.

**Verbs & keys.**

- **Park** — selection on the front, `ctrl-shift-a` ("To the back" on the selection menu's verb flank, wearing its chip). The selection lifts out, the paragraph closes beneath it, and the text slides to the dog-ear (~200 ms); the sliver then shows its opening words. Caret and scroll never move — compliance is visible at the destination corner (Spike's economy, Highland's legibility, neither's tax). The scrap lands atop the back under an origin-and-date caption. `ctrl-z` undoes in place now; "Put back" undoes it later (P13, twice).
- **Jot** — `ctrl-shift-a` with no selection: a one-line field in the verso's paper tone opens just under the caret line — a sliver of the back lifted to meet you. Type; `Enter` commits it to the top of the back (date caption) and the field vanishes; `Esc` cancels (standard capture-field contract, P7). The screen otherwise never changes: the untouched caret, selection and scroll are the resumption cue (Altmann & Trafton), and there is no destination, category, or title to decide — capture dies on the first filing decision (Bernstein).
- **Flip** — click the dog-ear, or `ctrl-shift-b`. Identical on either face; the verb is its own inverse (P13). ~180 ms horizontal turn with both paper tones legible in every frame (P6); reduced motion: crossfade.
- **Retrieve** — on the back, select and `ctrl-shift-a` ("To the front"): the text moves to the front's remembered caret and the sheet flips home with the arrival *selected* — the writer's own selection, not decoration (P1), ready for immediate undo or move. Every parked scrap's caption also ends in a quiet **Put back** — same word, same manners as the graveyard's (P8) — restoring an untouched scrap to its origin. Jots have no origin, so their captions carry no Put back: the grammar shows the difference.
- **Annotate** — `ctrl-m` works on both faces; back-face notes live in its left lane. One verb everywhere (P8).

**Esc grammar** (extends "go home"; never repurposed — habituation is sacred, Raskin). With a transient open (find, jot line, menu): Esc closes it and stays. Else, on the back: Esc flips home. Else, on the front: as today. Modes unwind last-in-first-out: parked in the past while on the back → Esc flips to that past state's front; Esc again returns to now.

**Never lost in the mode.** The state sits at the locus of attention: the ground under the caret is the other paper, the lane is on the wrong side, and the corner peeks the front's own words. Opening a document always lands on the front, whatever face was showing at close (F5). Cold read hides the dog-ear with the rest of the apparatus; the back is unreachable inside it — modes never stack.

**Empty state.** A blank back: verso tone, caret at top, dog-ear home. No copy of any kind (P2).

**First discovery / the F8 invitation.** Two structural paths, converging. (1) The dog-ear rests visibly from first launch — a folded corner is already-known (P5), and hiding halves discovery (NN/g), so it never hides even when empty; a curious click flips to a blank page that explains itself by being a page. (2) The first time a writer raises the selection menu — typically hesitating over a deletion, the exact moment the practice is born — "To the back" is sitting on the verb flank. The affordance rests at the moment of hesitation; nothing ever asks (P2).

**F7, resolved.** One pile, no taxonomy: seeds, parkings and unanchored asides land on one recency-ordered face, because "no longer dead-sure gone" is unclassifiable at capture (Marshall & Shipman; the collector's fallacy kills sorted systems). Captions distinguish the tenses automatically — origin-stamped (parked), date-stamped (jotted), unstamped (written in place) — so a months-later skim stays legible with zero filing. The aside tense's *anchored* half is already served elsewhere: a note about a passage is a margin note (`ctrl-m`); the back takes only what has no anchor. Split anchored-vs-unanchored, never three piles (Scrivener's four overlapping afterlives are the cautionary tale).

**F6, one spatial story.** Dead things fall; living things are turned to. What the writer deletes or exiles (`ctrl-shift-g`) falls to the document's tail — automatic, drained, read-only, a record you scroll onto. What she parks or jots goes to the back of the sheet — deliberate, warm, editable, a face she turns to. Both stay inside the one sheet; both return through the door they left by; both say **Put back** when they do. Deleting a substantial scrap *on* the back falls to the same one graveyard. The writer predicts the destination without thinking: *did I kill it, or keep it?*

**Assumptions (named).**

1. `ctrl-shift-b` and the no-selection arity of `ctrl-shift-a` are unclaimed.
2. A quiet word-count indicator exists at the window's lower edge; its face-scoping is specified below.
3. The graveyard's footer chip does not occupy the bottom-right paper corner; if it does, the dog-ear takes the top-right fold.
4. The margin apparatus can render on a left-hand lane.
5. Put back onto a deleted origin degrades to the nearest surviving neighbour, matching graveyard behaviour.
6. A reduced-motion preference exists.

## 4 · Edge cases

- **Find/replace**: face-local. Find on the front searches manuscript only (the search-sees-my-scraps failure, cured); flip and find to search scraps — which is exactly how memory-triggered retrieval works (Pressfield's Command-F). No cross-face query in v1; named loss (§6).
- **AI scope**: the reviewer never reads the back; cards only ever anchor front text; the back's lane holds writer notes only. On the back there is no machine presence at all.
- **Export & counts**: export renders the front, minus the graveyard record. The count at the window's edge counts the face you are looking at and wears its tone — on the front it is, at last, the count of the piece; scope is shown by the control that lives inside it (P12), never silently.
- **Time travel**: a checkpoint materializes the whole sheet — both faces and the boundary — so old states keep their own geometry and Restore appends both faces, itself reversible. Parked in the past, you may flip and read that state's back, read-only; edits pulse the banner; Esc unwinds back → front → now.
- **Cold read**: front only; the dog-ear hides with all apparatus. The read is manuscript-pure by construction, and visibly so.
- **Margin notes anchored in scraps**: parking a passage carries its margin notes to the back's lane, anchors intact; retrieval carries them home. Notes migrate rather than vanish, on either face.
- **A 3,000-word pile**: the back scrolls independently and remembers its position; full column width keeps long scraps readable prose. Nothing auto-collapses — the tool never abridges a face the writer edits; the hairline-and-caption rhythm *is* the riffle structure (Mander): skim by captions, read by choice.
- **A jot mid-burst**: the jot line is writer-initiated self-interruption, outside the machine's hold-back policy; it opens under the caret, the manuscript never reflows, Enter returns in under a second with the screen unchanged.
- **Narrow / wide**: the flip is viewport-total, so 800 pt and 1600 pt behave identically — no fight over horizontal budget at any width (the structural edge over any docked panel); the back mirrors the front's measure and margins at every size.

## 5 · Scorecard vs the null baseline

- **F1 Park — beats**: one chord, zero travel, compliance visible at the corner, origin remembered; the baseline's cut-scroll-paste-scroll is precisely the cost that turns cuts into deletions.
- **F2 Jot — beats**: the caret never leaves; the baseline buys the full round trip.
- **F3 Skim & return — beats**: flip, read at full measure, Esc restores caret, scroll and selection exactly; the baseline loses your place both ways.
- **F4 Retrieve — beats** (weighted low, as the evidence demands): lands at the insertion point, selected, or Put back to origin; the baseline is another double scroll plus hand-trimming.
- **F5 Never intrude — beats decisively**: export, counts, find, AI and cold read are manuscript-pure by construction and visibly scoped; baseline failures 1, 2, 6 all cured.
- **F6 One afterlife — beats**: killed-falls / kept-turns is a predictable two-preposition story with shared Put back grammar; the baseline has no story at all (failure 5).
- **F7 Three tenses — matches, honestly**: one undifferentiated editable pile is what the baseline offers too; automatic provenance and the anchored-aside home are garnish, not a new capability.
- **F8 Invitation — beats**: the divider convention must be invented; the dog-ear rests in sight from day one, and the park verb sits on the selection menu at the first hesitation over a delete.

## 6 · Named losses & risks

Stated as the enemy would. **It is a mode.** The back occupies the entire viewport; a writer who flips, gets pulled away, and returns can draft manuscript prose onto the wrong face — and every mitigation (tone, mirrored lane, dog-ear) is passive; none physically prevents the error, whose cost surfaces later, as a paragraph in the wrong world. **No juxtaposition, ever.** The one thing this shape can never do is show a scrap beside the manuscript gap it might fill; F3's "hunting for the piece that fits" happens through alternating flips, holding the gap in memory — an edge drawer does that one thing better, full stop. **The turn is one notch from theatre**: if the flip animation ever reads as delight rather than orientation, cut it to a crossfade without mourning (BumpTop). **The invitation rides on a corner fold**: if the dog-ear fails to read as "there's a back" to a stranger, F8 collapses toward the chord-only Spike failure. **Face-local find will one day eat a phrase** ("I know I wrote it — which side?"); a two-notch find widening is the known escape hatch, deliberately deferred.

## 7 · Build sketch

One retained-mode column renderer, reused whole: the back is the same block/line pipeline over a second text sequence with a face-scoped theme ground and a mirrored lane origin. The document model holds two sequences per state; checkpoints already materialize full states, so boundary versioning is free — a state is the sheet, both sides, and restoring one never teleports text. Scope machinery filters by sequence, not by rule. Per-face view state (caret, selection, scroll, find) is a small struct swapped at flip; the flip is a root-view transition between two prebuilt column views plus theme tokens. Genuinely new chrome is small: a corner overlay, a one-line capture field (shares the existing single-line input widget), and caption rows (the graveyard already renders stamped records). The costly honest bits: a P6-grade transition, and undo treating a cross-face move as one atom.

## 8 · The name

**The back of the page** — spoken, "the back." The verbs wear it plainly: *To the back*, *To the front*, *Put back*, *Flip*. It needs no explanation because the object explains it: every writer has turned a sheet over to scribble, and a folded corner has meant "there is more here" for centuries. (The Latin *verso* stays in this file; the writer never sees it.)
