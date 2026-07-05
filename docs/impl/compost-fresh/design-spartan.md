# The Spartan — the piece ends here

## 1 · The bet

The folk practice is not broken; the tool is blind to it. Writers already own the gesture (dump it at the bottom of this document), the container (the document itself), and the retrieval engine (find) — the incumbent wins on capture cost, editability, persistence, and searchability, and every research digest says so. What the practice lacks is a boundary the machine respects. So we ship the practice's missing half and nothing else: one line of writer-owned text — **the end-mark** — that the tool recognizes as where the piece ends. Everything below it is scraps: warm, editable, searchable prose that export, counts, cold read, and the AI reviewer no longer see. Because the boundary *is* text in the document, every checkpoint state carries its own geometry and time travel preserves it by construction — the requirement every containered rival must build per-state storage for, we get free (P3 taken to its logical end; Engelbart's viewspecs: not a new container, machine-known structure plus view control). No drawer, no panel, no flip surface. The strongest home for almost-manuscript text is the manuscript's own tail, one recognized sentence away.

## 2 · The scene

**At rest.** A document scrolled to its lower third. The warm paper column; the prose ends; a quiet line reads *· · · the piece ends here · · ·*. At exactly that line, the paper's warm coat ends — text continues below on the plain window ground: same typeface, same warm ink, blank-line-separated fragments, newest nearest the mark. Further down, the drained, read-only graveyard entries; then the document ends. The footer chip reads **1,842 · the piece**. A stranger concludes, correctly: the page ends at that line; the text below it is kept but is not the piece; the grey entries deeper down are dead. She clicks below the mark — a caret appears; typing types; formatting works (P3). Every frame is a legible still (P6): position on the scroll *is* the state — on the page, off the page's foot, in the ground.

**In use.** Mid-document, mid-revision, the writer selects a paragraph that has stopped belonging. The selection menu rises: formatting on one flank; on the other, **Annotate ⌃M · Set aside ⇧⌃A · Exile ⇧⌃G**. She strikes ⇧⌃A. The paragraph leaves; the prose closes over the gap; the footer ticks 1,842 → 1,798. Nothing else on screen moves; her caret sits where the paragraph was. The text now rests just below the mark, first in the pile, awaiting her end-of-revision sweep.

## 3 · Behavioural spec

**Triage of the six baseline failures** — what ships is decided by which failures are disease and which are tolerable:

1. *Export/counts/AI see scraps* — the disease. Fixed by the mark (scope law below).
2. *Cold read includes scraps* — same disease. Fixed by the mark.
3. *Park costs a round trip* — fixed with **zero new UI**: the already-shipped move-selection-out verb ⇧⌃A finally gets its destination: below the mark.
4. *Jot costs the same round trip* — **routed through shipped machinery**: anchored asides already have Annotate ⌃M (margin note at the caret, migrates if its anchor dies). Pile-bound thoughts are typed in place, selected, ⇧⌃A — the thought transits the prose for two seconds, then leaves no residue. Tolerable; no new capture UI earns rent here (Bernstein: capture wins by zero schema, not by a destination widget).
5. *No relation to the deletion record* — fixed by **geography alone**: the mark lives at the tail, so scraps sit directly above the graveyard. No mechanism.
6. *The tool doesn't know the boundary* — this **is** the mechanism.

**The mechanism — the end-mark.** The first line in the document whose letters, ignoring case and surrounding punctuation/whitespace, read **"the piece ends here"** is the boundary. Hand-typeable (`the piece ends here` alone qualifies); the verb stamps the canonical dressed form *· · · the piece ends here · · ·*. Lines matching it further down are ordinary text. The mark is writer-owned prose: caret, edit, cut, paste, delete all work. Move the line and the boundary moves. Delete it and the yard rejoins the piece — the ground re-warms and the count jumps at once, so the consequence is announced by the surfaces that changed, never by a dialog; undo restores it (P13: the mark's inverse is deleting a line of text). The mark is simultaneously the control and the indicator of the boundary (P12).

**The scope law — one sentence.** Surfaces that speak *for the audience* or *to the machine* — export, word counts, cold read, the AI reviewer — end at the mark (mark line excluded); surfaces that are *the writer's hands* — caret, typing, formatting, find, replace, history, margin notes — never do. Each scoped surface announces itself with data, not prose (P4): the count chip is labelled **N · the piece**; export names its content the same way; cold read simply ends where the piece ends; the AI's absence below the mark is corroborated by the ground (cards never appear beside scraps).

**Ground.** The warm paper is painted down to the mark; below it, text sits on the plain window ground. Colour speaks provenance once (P10) — the ink stays warm (writer's, alive) everywhere; the *ground* says on-the-page vs off-it, corroborated by position (below the mark) and form (the mark line, the ended paper edge). Nothing is drawn on the prose; the page the tool always painted just learns where to stop (P1).

**Verbs and keys.**
- **Set aside ⇧⌃A** (the shipped verb, destination now defined): relocates the selection, structure and margin-note anchors intact, to directly below the mark, newest-first. Caret does not travel. If no mark exists, the verb first writes one at the document's tail, above the graveyard — the writer's pen, moved by her explicit command, never uninvoked. Invoked on a selection already in the yard: moves it to the top of the yard (same meaning everywhere — "set this aside, fresh"; P8). Disabled while the selection contains the mark line, and inside the read-only graveyard.
- **Annotate ⌃M** — unchanged; the aside tense already has its home.
- **Exile ⇧⌃G** — unchanged; sits beside Set aside on the flank, so the selection menu itself teaches the two afterlives at the moment of cutting: *aside = kept warm above; exile = recorded dead below.*
- **Retrieve** — a scrap re-enters by the grammar it left in: relocation. Select it, cut, paste at the insertion point; or ⌃Z immediately after a park. No dedicated verb — retrieval is ~1–2%, phrase-grained, memory-triggered, and executed by find (Pressfield's ⌘F); building UI for it loses to the baseline's own move.

**The count chip.** The footer count reads **N · the piece**; clicking it scrolls to the mark; **Esc returns** to the prior caret and scroll exactly — the same restore grammar find already honours. This is the skim round trip (F3) and the park-compliance witness (F1: the tick you saw is inspectable one click away). It mirrors the graveyard's footer-chip contract (P7, P8) and hides while the mark is on screen.

**Focus and Esc.** No modes, no panels, no focus traps; nothing here can hold focus, because everything here is document text. Esc gains exactly one clause — return-from-chip-jump — an extension of its existing "leave find, restore position" habit, never a repurposing.

**Empty state.** No mark → no feature. No region, no chip suffix, no ghost divider, nothing to explain. The purest possible P2.

**First discovery / the F8 invitation.** The verb rests on the selection flank from day one, wearing its chord chip — the notch on the drill handle (P5). The moment a writer first hesitates over a large deletion is the moment her eyes are on that menu; *Set aside* sitting above *Exile* names a gentler option at the exact instant of the pang the practice exists to relieve (permission, not archive — the digests are unanimous). One strike and the convention assembles itself in front of her: a line saying the piece ends here, her paragraph resting under it. Nothing asked; the affordance was simply where her hand already was. Writers who already keep a divider discover it from the other end: type the sentence, watch the count and the page edge obey.

**Assumptions, named.**
1. A word-count display exists in the footer/status area; if it lives elsewhere, the chip behaviour moves with it.
2. ⇧⌃A ships today without a destination; defining it is this design's obligation, not new surface.
3. Relocation preserves margin-note anchors (a move is a move, not delete-plus-insert).
4. Opening a document restores last caret/scroll (or top) — either lands in the story, satisfying F5's landing clause.
5. The renderer can end the paper ground at a text offset without touching glyphs (the graveyard's distinct region is precedent).
6. Localized builds recognize all shipped translations of the phrase, and the verb stamps the locale's form.
7. Cold read already takes "the manuscript" as its input; re-scoping to the mark is a parameter.

## 4 · Edge cases

- **Find/replace** — whole document, unchanged. Deliberate: search is the retrieval engine, and a piece-wide rename *should* reach scraps so a later retrieval carries the new name. Nothing changed, so nothing to announce.
- **AI scope** — the reviewer's input ends at the mark, unconditionally; no card ever anchors below it. The caged machine never reads the yard, even on request, in v1.
- **Export & counts** — the piece only, mark excluded. Export-from-selection was considered for exporting scraps and **declined**: copy-paste covers the rare case; the affordance doesn't pay rent.
- **Time travel** — the mark is content, checkpoints materialize full states, so every past state carries its own boundary; parking in the past renders that state's geometry, counts and ground included; Restore appends and nothing teleports. Zero boundary-versioning machinery exists to break.
- **Cold read** — renders strictly above the mark; the fresh-eyes read ends where the piece ends. Reactions become margin notes after, as shipped.
- **Margin notes anchored in scraps** — lawful and warm; they ride along on park and on retrieval (assumption 3). AI cards never appear there.
- **A 3,000-word pile** — normal ("the file is always longer than the book" — Pressfield). Recency order keeps the live top skimmable; the old sinks toward the graveyard. Scraps are writer text, so the tool never folds or recedes them (that grammar is for machine-rendered records); the cost is scroll length, which the chip and ⌘F absorb.
- **Jot mid-drafting-burst** — type the thought where you are, select, ⇧⌃A. No stance change, no panel, no machine reaction (the lull policy holds); the caret ends where the thought interrupted, and the unchanged screen is the resumption cue (Altmann & Trafton).
- **800 pt / 1600 pt** — the yard is column text; it reflows with the column and costs zero horizontal space at any width.

## 5 · Scorecard vs the null baseline

- **F1 Park — beats.** One chord, no travel, lossless, anchors intact, count ticks; baseline pays cut–scroll–paste–scroll–refind.
- **F2 Jot — beats, narrowly.** Anchored asides tie (⌃M exists in the baseline too); pile-bound thoughts drop the round trip via type-select-⇧⌃A.
- **F3 Skim & return — beats, narrowly.** Chip-jump plus Esc-exact-return versus scroll-hunt-scroll-back-refind.
- **F4 Retrieve — matches.** The same manual move, honestly weighted for a 1–2% flow; whole-file find is the real engine either way.
- **F5 Never intrude — beats, decisively.** Export, counts, cold read, and the reviewer are manuscript-pure and visibly labelled; this erases baseline failures 1–2.
- **F6 One afterlife — beats.** Living scraps rest just past the page's foot, the dead record below them: the deeper, the deader; the flank menu pairs the two verbs. Baseline scraps float unrelated to the graveyard.
- **F7 Three tenses — matches.** Asides inline via notes, parkings and this-piece seeds in one recency pile, classification deferred (Malone; Marshall & Shipman); pre-piece global seeds are explicitly cut, as per-document scope demands.
- **F8 Invitation — beats.** The verb rests in the selection menu at the exact moment of hesitation; the baseline requires the writer to have already invented the trick.

## 6 · Named losses & risks

Stated as the enemy would: **this design's soul is a magic string, and its feedback is a tick.** (1) Structural meaning rides on one editable line of prose: a hand-typed near-miss ("the piece ends here." inside a paragraph? tolerated; "peice"? silently ordinary text) fails without an error, because there is nothing to error — mitigated only by the ground and count visibly *not* changing, which the writer must know to check. (2) Park compliance is a count decrement and a closed gap — quiet enough to flirt with the Spike's fate; a writer who misses the tick must scroll or click to trust the verb once. (3) There is no glance: skim means leaving the prose, and no side-by-side compare exists for retrieval-heavy revisers — a drawer rival genuinely wins that moment. (4) Seeds that precede any piece have no home; we cut that tense rather than pretend a per-document tail serves it. (5) A purist reading of P1 objects that the first ⇧⁠⌃A writes a sentence the writer didn't type; we answer that a verb's named, writer-invoked effect is the writer writing — but it is the design's one real amendment and it must be owned. (6) The pile is unstructured by design; a writer wanting curation tiers must type her own headings — which is the point, but it will be filed as a missing feature.

## 7 · Build sketch

Small by intent: one recognition pass (first matching line → boundary offset, recomputed on edit, cached per revision); one predicate `offset < mark` consumed by count, export, cold read, and the reviewer's input assembly; a ground-paint cutoff at the mark's layout position; ⇧⌃A becomes programmatic relocate-below-mark (plus stamp-if-absent) reusing the existing lossless-move plumbing and note-anchor migration; the chip gains a stored-return jump riding find's restore path. Boundary versioning costs nothing: the mark is document content and checkpoints already materialize full states. The two honest risks are auditing every scope consumer onto the single predicate, and recognition tolerance/i18n — both testable in isolation.

## 8 · The name

**The scraps** — the word writers already use, needing no glossary. The mark speaks the product's register in the document itself: *· · · the piece ends here · · ·*. The verb is **Set aside**; its neighbour Exile keeps the graveyard; and the pairing tells the whole spatial story in three words each: set aside above, exiled below, the piece before both.
