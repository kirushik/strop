# Design — the tail section: **Scraps**

Slug: `tail`. The null baseline, dignified: the tool learns where her scrap line is and starts respecting it.

## 1 · The bet

The bottom-of-document dump is not a workaround to be replaced; it is the winning design, field-tested by most working writers who keep scraps at all (Reproof survey; Pressfield's CULLS; the "piggy bank"). Its only failures are that the tool doesn't *know* the boundary — so export, counts, the AI, and a cold read trespass — and that the round trip to the bottom costs travel. Both failures are fixable without moving the text anywhere else. Engelbart says the missing half of "one continuous document" is machine-known structure plus view control, not a second container; Fountain's boneyard proves writers accept non-manuscript text inside the one file when output-scoping is automatic; Ulysses proves "same place, different scope" is the cleanest shipped answer. So: scraps stay editable text at the document's own tail, below a seam the tool knows, above the graveyard record. Every added element must beat the divider convention somewhere nameable, and there are exactly four: the seam (beats baseline failures 1, 2, 6), the set-aside verb (failure 3), the footer composer (failure 4), and provenance-with-put-back (failure 5). Nothing else is added. The deepest advantage is F6: the tail becomes one continuous gradient of death — story, then living scraps, then the dead record — one direction, down, one spatial story a writer predicts without thinking.

## 2 · The scene

**At rest.** Scrolling past the manuscript's last paragraph, a stranger meets a hairline rule across the column with a small quiet label at its left — **Scraps** — and a count at its right — **2,340 words**. Below it: prose blocks in full ink, same measure, same type, separated by blank lines. Beside some blocks, in the margin, a small drained one-liner: *from "…the dog bites the football" · 2 Jul*. Further down, dimmer: the graveyard's drained, read-only record. In the footer, a quiet chip: **Scraps · 2,340**. The stranger concludes, correctly: the piece ends at the line; what's under the line is kept but not part of it; the dimmer stuff below that is deleted; *down* means *out of the piece*; the chip is the way down and the number is how much is kept. Full ink versus drained tells living from dead (P10 — corroborated by editability and position, not colour alone).

**In use.** Mid-manuscript, a selection raises the two-flank menu; on the verb flank, **Set aside ⌃⇧A** rests one row above **Exile ⌃⇧G** — the gentle verb above the kill. Press it: the selection leaves the prose with no residue, the caret and scroll do not move, and the footer chip's number grows by the words that left. Before-frame: selection. After-frame: text gone, bigger number. Both stills make sense alone (P6).

## 3 · Behavioural spec

**The seam.** A structural boundary block in the document — not a character, not typed, not deletable by backspace. Everything above is manuscript; everything between seam and graveyard is Scraps. The seam renders as the hairline + label + live word count of the region below it. It exists only while the region is non-empty: the first set-aside or jot creates it; emptying the region evaporates it (the inverse of the first park, P13). The caret arrows across it like a paragraph break; selections may span it (the seam itself never enters the clipboard, and deleting a spanning selection leaves the seam between the remnants). Typing above it is manuscript; typing below it is scraps — position is the announcement (P12: the seam is the control that *is* the scope indicator).

**Adopting an existing convention.** A writer who already keeps a pile under her own divider selects that pile and presses ⌃⇧A once. It moves below a new seam; her divider line she deletes or keeps as scrap text. One existing gesture; the tool has now "learned where her scrap line is." No detection magic, no prompt (P2).

**Set aside (park), ⌃⇧A with a selection.** The lossless move-out verb's destination is the top of Scraps, directly under the seam — newest nearest the story, the pile ageing downward toward the graveyard. The parked text arrives verbatim, editable, structure intact (P1: relocated, never decorated). No travel: caret, selection remnant, and scroll are untouched — the unchanged screen is the resumption cue (Altmann & Trafton). Compliance is the chip's count growing. Inverse: the provenance chip's **Put back** (below), or plain cut-and-paste — it's text.

**Jot, ⌃⇧A with no selection.** The same verb with a prospective object, like bold-with-no-selection (P7, P8): the footer chip opens into a one-line text field — a real field, warm caret, the omnibar's contract (Enter commits, Esc discards, Shift-Enter for a rare second line). Commit lands the line at the top of Scraps with a date; the field closes; the chip's count grows; the manuscript caret never moved and nothing scrolled. Zero filing decision, zero classification (Bernstein: capture dies when it demands a schema).

**Provenance.** Each parked block carries a drained one-liner chip in the margin: *from "…first words of origin passage" · date*. Machine bookkeeping wears machine shape (a widget) in the machine's district (the margin), leaving the scrap text pure (P1, P3). Click expands it: full provenance and a **Put back** action row (the sanctioned carrier-sentence channel, P4). Put back returns the block's *current* text to its origin anchor — same door it left by, same grammar as the graveyard's Put back (P13, F6, P8); the anchor migrates like a margin note if the origin passage was edited or deleted. Returned text speaks sage briefly in the margin, the product's existing "returned" word. Jots have no origin, so no chip — which quietly distinguishes parkings from asides without ever asking the writer to classify (F7).

**Navigation & the round trip (F3).** The footer chip — **Scraps · 2,340** — navigates to the seam and hides while the tail is on screen (the graveyard chip's shipped contract, extended: one tail, one chip; the graveyard sits below the pile). **Esc from anywhere in the tail returns to the last manuscript caret and scroll position, exactly** — the same "go home" Esc already performs for find and the parked past (extended, never repurposed). The excursion leaves no residue. Reaching the graveyard means passing your scraps — the end-of-revision re-read ritual, structural rather than prompted.

**Retrieval (F4).** Rare and phrase-grained (Pressfield: ~1–2%), so: find crosses the seam; skim is the chip-click away; and the cheap general inverse is select-in-scraps → cut → Esc (home, exactly) → paste at the insertion point. Put back covers the restore-to-origin case. No library, no browse UI.

**Focus & Esc grammar.** Esc in the jot field: discard and close (find's contract). Esc in the tail: home to the story. Esc elsewhere: unchanged. Opening the document always lands in the story (F5), whatever was focused at close.

**Empty state.** Nothing. No seam, no region, no chip, no menu noise beyond one verb row. A document with no scraps is pixel-identical to the null baseline — the design ships zero standing chrome until the practice begins.

**First discovery / the invitation (F8).** The moment is the hesitation over a deletion. The selection menu the writer already uses shows *Set aside* resting beside *Exile* — an affordance in the selection path, where discovery lives (Spike died in chord-space; Highland's Bin lived on the drag path). One press teaches everything: text leaves, a chip appears reading **Scraps · 41**, clicking it shows her words resting under a labelled line. The tool never asked; the notch was on the handle (P2, P5).

**Assumptions (named).**
1. A persistent word-count readout exists in chrome; it is scoped to the manuscript (above the seam).
2. An export surface exists and states what it exports and its count.
3. The margin-note anchor-migration machinery is reusable for provenance chips and put-back anchors.
4. The graveyard footer chip may be relabelled/extended by this design; it already hides when the tail is visible.
5. The selection menu's verb flank accepts one more row.
6. Global cross-document seeds are out of scope (per-document canvas rule); seeds here means seeds of *this* piece.
7. The document model can store one structural boundary node per checkpoint state.
8. The graveyard's substantial-deletion capture applies wherever the writer deletes — including inside Scraps.

## 4 · Edge cases

**Find/replace.** Find crosses the seam; the match count announces scope: *7 in the piece · 2 in scraps*. Replace-all acts on exactly what the count names — the announcement is the visibility. Memory-triggered search is the real retrieval engine; walling it off would break the practice.

**AI scope.** Diagnosis never crosses the seam: no cards anchor below it, ever; a review pass reads only the manuscript. Visible by absence — the cool lane simply ends at the seam. The writer's own annotate (⌃M) works in Scraps: her words, her tools (P3).

**Export & counts.** Both stop at the seam. The chrome count reads manuscript-only; the seam's own count carries the remainder, so the arithmetic is legible at a glance; the export surface states the manuscript count it will emit.

**Checkpoint time travel.** The seam is a document-structure node materialized per checkpoint state: every past state keeps its own geometry; pre-seam states show no seam; Restore appends the past state with *its* seam. Text never teleports across the boundary by time travel.

**Cold read.** Apparatus hidden, manuscript only: the read ends at the piece's true last line. The seam, Scraps, graveyard, and chip are simply absent — manuscript-pure, visibly so by the ending itself.

**Margin notes anchored in scraps.** Legal (writer's text is writer's text); they park and travel with the block; a put-back carries its notes home. AI cards cannot exist there, so no migration question arises for them.

**A 3,000-word pile.** Pressfield's file outgrows the book; expect it. The pile stays plain text: scroll and find, exactly as the baseline. Navigation *to* it stays one click and Esc home, so pile size never taxes the story side. Honest cost: the graveyard drifts farther below, reachable by scrolling past the pile (or the history of habit: Ctrl-End). No folding, no receding of scraps in v1 — receding writer-editable text would make her words behave as widgets (P3).

**Jot mid-drafting-burst.** Writer-initiated, so never policy-forbidden; the field opens and closes without scrolling or caret movement; the chip's number updates silently — no motion competes with the burst.

**Narrow/wide.** One column; the seam spans the measure; the tail reflows like any text. At 800 pt the provenance one-liners compress like receded margin cards; the footer chip and composer are footer-sized at any width.

## 5 · Scorecard vs the null baseline

- **F1 Park — beats.** One chord, zero travel, zero residue, compliance visible as the chip's count grows; baseline costs cut-scroll-paste-scroll-refind.
- **F2 Jot — beats.** Footer composer commits without the caret leaving; baseline costs the same round trip as a park.
- **F3 Skim & return — beats.** Chip down, Esc home to the exact position; baseline is two manual scrolls and find-your-place.
- **F4 Retrieve — beats.** Cut → Esc → paste plus put-back-to-origin; baseline has the same text but no cheap way home and no inverse machinery.
- **F5 Never intrude — beats.** Export, counts, AI, and cold read stop at a boundary the tool knows and shows; this is the baseline's defining failure.
- **F6 One afterlife — beats.** One gradient, downward: story → living scraps (ink, editable) → dead record (drained, read-only); deleting a scrap falls one level, into the record. Baseline has no relation to the graveyard at all.
- **F7 Three tenses — matches.** One pile, classification deferred (Malone; Marshall & Shipman), exactly as her own pile serves all three; parkings self-label via provenance, but seeds-of-other-pieces are served no better than the baseline serves them.
- **F8 Invitation — beats.** The baseline cannot invite — she must invent it. Here the verb rests in the selection path at the hesitation moment, and the first use builds the whole convention in front of her.

## 6 · Named losses & risks

**The enemy's best case.** "You shipped the baseline plus four gadgets. And the one thing a real second surface could give — seeing a scrap *beside* the passage you're revising — is structurally impossible in your direction: your skim is a teleport excursion, never adjacency. Berthoff's facing page, the writer's own container metaphor, is exactly what a tail cannot be." True, and accepted: v1 bets that the round trip at Esc-cost beats a second surface's standing complexity; if usage shows sustained compare-while-writing, this design has no answer short of a different direction.

**The seam must be flawless text mechanics.** It is the only structural object living inside the writer's text plane. Every caret, selection, paste, and backspace interaction at that line must feel like a paragraph break or the region reads as the tool disputing her page — one bad edge case and the P3 trust the whole bet rests on erodes.

**Distance grows with the pile.** Intra-tail navigation is scrolling, as in the baseline; a huge pile pushes the graveyard far away. Matches-the-baseline is the floor here, but a rival with folding or a drawer will call it a ceiling.

**One overloaded chord.** ⌃⇧A meaning park-or-jot by selection state is defended by the formatting precedent, but it is still two behaviours on one chord; if testing shows mis-fires, jot needs its own chord and the grammar takes the complexity instead.

## 7 · Build sketch

The seam is one new block-kind in the document model — a boundary node with no editable content — serialized in the file and materialized per checkpoint state like any other content, so history and Restore get geometry for free. Region membership is derived by position relative to the node; counts, export, AI-scope, and cold read filter on that derivation (viewspecs, not copies). Park and put-back reuse the existing lossless move machinery and the margin lane's anchor-migration; provenance chips are margin-lane citizens with the graveyard's receded-one-liner form. The composer is one transient text field docked at the footer chip, sharing the find field's key contract. Retained-mode one-column layout is untouched: the tail is ordinary blocks in the same scroll plane.

## 8 · The name

**Scraps.** The writers' own word — the surveys say "scrap file" unprompted — plain, warm, a little affectionate, honest about status without ceremony. The boundary is **the scrap line** ("everything under the scrap line stays out of the piece"). The verb is **Set aside**; its inverse is **Put back**. Scraps live; the graveyard remembers.
