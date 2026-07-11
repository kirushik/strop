# Design — Scraps: the drawer at the writer's left hand

## 1 · The bet

Every failure on the null baseline's list grows from one root: it stores not-manuscript *inside* the manuscript, then needs every scope — export, counts, AI, cold read — to pretend otherwise. Designs that keep scraps in the document's flow inherit that root and patch it with exception rules. The drawer removes it structurally: scraps live on a second sheet of the writer's own paper — same desk, same file, but never inside any manuscript scope, because they were never inside the manuscript's geometry. That is where practice evidence points too: writers keep the *living* pile beside the work (Lamott's pocket card, the writers'-room Candy Bag, Berthoff's facing page — a beside-the-page lineage that is writerly, not office furniture) and the *dead* record below it (boneyard, morgue). Strop has already built "below": the graveyard at the tail. The drawer builds "beside": deliberate, warm, editable, at the writer's left hand — present as a quiet edge, out of the text's way until summoned. Scraps are a different kind of thing; they get a different place, one gesture away.

## 2 · The scene

**At rest.** The manuscript column, centred on warm paper. At the extreme left edge, a slim vertical spine — a sliver of a second warm sheet showing its edge, like the page beneath the top one in a stack — carrying, set spine-wise like a shelved book, `Scraps · 14`. Nothing else. A stranger concludes: another page of mine rests beside this one, edge showing; it holds fourteen scraps; the edge will slide it out. Before the writer's first scrap ever, the spine does not exist and the surface is exactly today's.

**Open, wide window.** A second sheet, a shade deeper warm than the manuscript ground, sits in the left gutter beside the column — the column has not moved; its caret is hollow, waiting. On the sheet: the writer's scraps as plain text blocks, newest at top, each under a small drained provenance row (`30 Jun · set aside · Put back` on parked passages; `30 Jun` on typed thoughts). Older entries are one-line heads. The seam between the sheets is the spine, now at the drawer's right edge. The stranger concludes: this pile is mine (warm; clicking gives a caret), each entry remembers when it arrived and how, the pile ages downward, the draft waits untouched one glance to the right.

**Mid-park frame.** The selected passage ghosts from the column while the spine already reads `· 15`. Any still says: the text went behind that edge (P6).

## 3 · Behavioural spec

**The regions.** The document holds three sibling text regions in one file: manuscript, scrapyard, graveyard record. The scrapyard renders as the drawer; it is a full text surface (P3): caret, typing, selection, formatting, the two-flank menu — manuscript mechanics exactly. Entries are separated by provenance rows (system chrome, drained, small); within an entry, everything is the writer's text.

**One verb crosses the seam.** `Set aside` (ctrl-shift-a) — the brief's lossless move-selection-out verb, given its destination:

- *Park (F1).* Selection in the manuscript → the passage moves losslessly to the top of the drawer, structure and formatting intact, origin recorded. The caret collapses in place and never travels; the spine's count increments in peripheral vision — compliance visible from where the writer sits, the Spike's economy with the Spike's invisibility fixed. The verb rests on the selection menu's verb flank between Annotate (ctrl-m) and Exile (ctrl-shift-g): keep, set aside, kill — an honest severity gradient.
- *Jot (F2).* No selection → a small warm capture line slides out of the left edge at the caret's height: one text line, focus in it, the rest of the screen unchanged — the untouched caret, selection and scroll *are* the resumption cue (Altmann & Trafton via the theory digest). Enter, Esc, or clicking away commits it to the top of the drawer; the line slides back under the spine, teaching the destination by its own motion; focus returns to the manuscript caret. An empty line just closes. Typed text is never destroyed by any exit (P13); a wrong jot is deleted in the drawer, as text.
- *Take back (F4).* Selection in the drawer → the same chord moves it losslessly into the manuscript at the writer's insertion point (the hollow caret). Same door in and out, direction read from where you stand — the inverse is inferable, never taught (P13, P8). Ordinary cut and paste also works everywhere, because everything is text; the baseline's own retrieval survives intact.
- *Put back.* Parked entries carry a whisper-quiet `Put back` on their provenance row — the graveyard's verb, the graveyard's row anatomy (P8) — returning the passage to its origin. If edits have dissolved the origin, Put back re-anchors like a migrating margin note; if nothing survives, it falls back to the insertion point.

**Entry and exit.** The spine opens the drawer; the spine (now the seam) closes it — the control is the indicator of open/closed (P12). The omnibar command `scraps` opens it for keyboard hands. No dedicated open-chord in v1. Opening never moves or reflows the column: at wide widths the drawer occupies empty gutter; at narrow it overlays the column's left portion under a hairline seam and soft shadow — a sheet laid over the page, legible in any still (P6).

**Focus and Esc.** Opening moves focus into the drawer at its last caret position; the manuscript caret goes hollow but keeps its place. Esc walks home innermost-out, extending its habit, never repurposing it: capture line → committed and closed; drawer → closed, manuscript focus and scroll exactly restored (zero residue); then Esc's existing meanings (leave find, leave the parked past, leave cold read). One yielding law: the drawer never rests over prose that holds the live caret — clicking into overlaid manuscript closes it; a gutter-docked drawer, occluding nothing, stays open beside the work.

**States of an entry.** Fresh entries show whole; with age they recede to one-line heads that click open — the margin's and graveyard's recede grammar, spoken a third time (P8; Engelbart's truncation viewspecs). The drawer remembers its scroll position. Its anchor object (P11) is the newest entry, top of pile.

**Empty state.** An empty drawer (omnibar, or all scraps spent) is a blank warm sheet with a ready caret. No copy, no coaching (P2, P4). The spine appears with the first scrap ever and persists thereafter (label only, no count, at zero).

**First discovery and the invitation (F8).** No tour, no tip. A writer selects a paragraph she has stopped believing in; the menu she raises to delete it already shows `Set aside ⌃⇧A` beside the deletion she came for — the affordance resting exactly where the hand hesitates, which is the moment the practice evidence says scrap-keeping is born. One click parks it; a new edge appears at the left of her desk; the practice now has a place, invented — as far as she can feel — by her.

**Assumptions, named.**
1. ctrl-shift-a with no selection may lawfully mean "set aside what I'm about to type" — one verb, one seam, selection-or-typed.
2. The capture line is single-paragraph; longer thoughts continue in the drawer itself.
3. The graveyard records manuscript deletions only; drawer deletions are plain edits, recoverable through checkpoints — no afterlife of the afterlife.
4. A word-count control exists in chrome and can carry a scope label.
5. The graveyard's own export handling is out of scope here.
6. v1 has no in-drawer reorder beyond cut/paste, and no origin-preview; Put back acts, it does not rehearse.

## 4 · Edge cases

**Find/replace.** Find is focus-scoped: from the manuscript it searches the draft; from the drawer, the scraps. A tail row in the results reports the other surface as data-plus-action — `3 in scraps` / `2 in the draft` — click crosses over (opening the drawer if needed). Replace acts only within the focused surface, never silently across the seam. This beats the baseline's undifferentiated soup and serves Pressfield's memory-triggered Command-F retrieval.

**AI scope.** The reviewer's jurisdiction is the manuscript, structurally: diagnosis runs only over the manuscript region, cards anchor only there, and the drawer never carries one drop of cool ink — its unbroken warmth *is* the announcement, corroborated by place (P10: colour and form agree). Cards anchored in a passage being set aside expire with the departure; machine artifacts do not follow the writer's text out of the machine's cage.

**Export and counts.** Export emits the manuscript; scraps never leave home. The count counts the draft — and from the moment a second scope exists, its label says so (`draft · 1,842 words`); while the drawer holds focus, the same control reads `scraps · 312` (P12: the control indicating what is being counted, which is where you are). Scope announced, never silent.

**Time travel.** The scrapyard is a region of the document state, so every checkpoint materializes the drawer it had; the boundary versions per state by construction — nothing teleports. Parked in the past, the drawer opens read-only like everything else; attempted edits pulse the banner. Restore appends the whole past state, scraps included — a scrap since taken back returns to the drawer too: duplication is possible, loss is not, and the append is itself reversible.

**Cold read.** All apparatus hides; the spine hides with it. The piece, pure.

**Margin notes anchored in scraps.** Notes wholly inside a parked passage travel with it: the entry's provenance row gains a small warm note-mark; expanded, the notes render beneath the entry in their margin styling. Take back or Put back re-anchors them in the margin. Notes only partially covered stay behind and migrate, by the existing deletion rule.

**A 3,000-word pile.** Pressfield's file outgrew his book; the drawer is built for that: recede-with-age keeps the visible pile shallow, the scroll is independent of the manuscript's, provenance rows keep a months-old skim legible, and focus-scoped find handles the rest. No taxonomy is ever offered (Malone; Marshall & Shipman): one pile, recency-ordered, classification deferred forever.

**A jot mid-drafting-burst.** Writer-initiated, so the no-interruption policy is untouched; the capture line is the writer's own warm text and triggers no machine response. Screen unchanged but one small line; caret restored on commit; cost in seconds.

**Narrow and wide.** At 1600 pt the drawer docks in the gutter: draft and scraps genuinely side by side — a compare the baseline can never offer. At 800 pt it overlays the column's left, yields to the caret, and skims fine; only side-by-side compare is lost. At every width, the one law: the column never moves.

## 5 · Scorecard vs. the null baseline

- **F1 Park — beats.** One chord, zero travel, count ticks at the edge; baseline costs cut-scroll-paste-scroll-refind, which is why cuts die by deletion.
- **F2 Jot — beats.** Capture at caret height, screen otherwise frozen, caret restored; baseline costs the full round trip mid-sentence.
- **F3 Skim & return — beats.** Slide out, read, Esc; return is exact because the manuscript never scrolled; wide windows add side-by-side. Baseline loses your place both ways.
- **F4 Retrieve — beats, narrowly.** Baseline's cut-and-paste is preserved (text is text) plus a no-scroll take-back at the insertion point and origin-true Put back. Weighted low, as the evidence demands.
- **F5 Never intrude — beats, decisively.** Manuscript-pure export, counts, AI, cold read by construction, each scope visibly announced; this is baseline failures 1, 2 and 6 dissolved rather than patched.
- **F6 One afterlife — matches.** Two doors, two directions, one grammar: set aside = beside, warm, editable; deleted = below, drained, record; identical row anatomy and Put back on both. But the baseline's pile sits physically adjacent to the graveyard in one scroll — I trade seen adjacency for a told story, and call that a draw.
- **F7 Three tenses — matches.** One pile serves seeds, parkings and unanchored asides with automatic provenance and no capture-time filing; anchored asides keep their existing home in the margin (the inline-vs-pile fault line practice actually draws). The baseline's pile is equally free; neither serves global seeds.
- **F8 Invitation — beats.** The baseline must be invented by the writer; here the verb rests in the selection menu at the exact hesitating moment, and the first use births the place. Structural, never promotional.

## 6 · Named losses & risks

Stated as the enemy would. **First:** the drawer breaks Strop's deepest virtue — one continuous column where everything the writer owns is reachable by scrolling. Scraps become the only writer text a scroll cannot find; out of sight may read as out of file, and a writer who does not *trust* the drawer with her darlings keeps the divider habit, leaving the feature as chrome. The Engelbart digest says the baseline's missing half is views over one document, not a second container — I am spending the budget on the container. **Second:** the invitation is a single thread. A writer who deletes by backspace, never select-then-menu, may never meet `Set aside`; and the spine's absence before first use is exactly the hidden-navigation pattern NN/g measured halving discovery. **Third:** F6's spatial story is taught by consistency, not shown by adjacency; some writers will look for parked text at the bottom, where twenty years of habit put it. **Fourth:** at 800 pt the drawer covers the prose it should sit beside; the compare advantage is wide-window only. **Fifth:** focus-scoped find, layered Esc, and the yields-to-caret law are each one clean rule, but together they make the drawer the most modal object in the product — Raskin's mode-error warning applies to me before it applies to my rivals.

## 7 · Build sketch

The scrapyard is a third top-level text region in the document model, sibling to manuscript and graveyard, serialized into every checkpoint state — boundary versioning comes free because the boundary is structural, not an in-flow marker. Rendering is one edge-docked layer in the retained-mode tree: gutter-dock or overlay chosen by viewport width, never reflowing the column. Nearly everything is reuse: the text-surface widget and its selection menu, the graveyard's provenance-row / recede / Put back grammar, the margin's note migration, the omnibar command registry. New work: the spine, the slide layer, the capture line, and seam-crossing move bookkeeping (origin anchor per parked entry, re-anchoring on Put back). Scope enforcement is subtraction — export, counts, reviewer, and cold read simply iterate the manuscript region, and each surface's label says so.

## 8 · The name

**Scraps.** The folk word for the folk practice — writers already say "my scraps file" — plain and warm, and gentler than its drained twin the way the Candy Bag is gentler than the cutting-room floor; names change how cutting feels. The place is *Scraps* (so the spine reads `Scraps · 14`); the way in is *Set aside*; the ways out are *Take back* and *Put back*. In speech: "I set it aside — it's in my scraps."
