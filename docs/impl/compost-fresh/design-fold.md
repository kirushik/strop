# Design — Folds

*Direction: in-situ scraps. Slug: `fold`.*

## 1 · The bet

Every rival answers "where do scraps live?" with a place. The fold answers: they already live somewhere — the exact spot where they stopped being manuscript — and *moving* them is the cost that kills the practice (baseline failures 3–4; research-practice: "the practice is born at the moment a cut hurts"). So the cut passage stays where it stands and pleats shut into one quiet line; only the boundary changes, never the location. Three things come free that no gathered place can buy: provenance is structural (a fold sitting in chapter 2 *was cut from* chapter 2 — the "cut from ch. 2" note writers add by hand, made geometry); park needs zero travel and its compliance is visible at the very point of the wound, not at some distant edge; and asides get what the TK and Fountain-`[[note]]` lineage proves they want — to stay at the text they concern. Fountain's boneyard already demonstrated that writers accept non-manuscript text inside the one document when output-scoping is automatic; its lone complaint — parked text "stays visually in your way" — is exactly what folding fixes. The manuscript becomes the map of its own compost.

## 2 · The scene

At rest, a draft mid-revision. Between two paragraphs sits a single shorter line: indented one em, the manuscript's own typeface a step smaller and lighter, warm ink, resting on a whisper of warm wash that stops short of the column edges. At its left, a small pleat mark — two short converging strokes, a tuck in paper. The line shows the writer's own first words, then an ellipsis and a quiet `· 214 words`. Three paragraphs later, another pleat line; this one is short enough that its whole text fits — `check the ferry timetable before this ships` — no ellipsis, no count. A stranger concludes, correctly: *strips of this page are folded shut; those are the writer's words on the crease; the essay flows past them; there is more inside the long one and that's all of the short one.* Nothing is drawn on the prose (P1) — folds are blocks *between* paragraphs, and the wash sits only under scrap text, never under manuscript. Nine folds in a long draft read as nine pencil-pleats scattered where they belong; they spend almost no contrast, so the column stays the anchor (P11).

Clicked open, the pleat mark rotates flat, the full scrap lies on the warm wash at full measure — ordinary editable prose, caret and formatting like anywhere else (P3) — and a thin crease row at its top carries `folded 12 Jun · 214 words` with two quiet verbs at rest: **Unfold** and **Exile**. Every frame is a legible still: pleated glyph = shut, open glyph = open (P6, P12 — the crease is both the control and the indicator).

## 3 · Behavioural spec

**The boundary.** A fold is a block-level region the document itself knows: excluded from export, counts, AI scope, and cold read; included in history. Fold content is the writer's living text — warm, rhyming with the margin's warm note tint: parallel meaning, parallel form (P8, P10). Folds never split a paragraph and never nest.

**Park (F1).** Select a passage → **Fold**, on the verb flank of the selection menu beside Exile, wearing its chord chip — this *is* the destination of the existing lossless move-out verb, `ctrl-shift-a`; the destination is "right here, folded." The selection pleats shut into one line in place; if the selection was mid-paragraph, the surrounding prose closes over the gap and the fold line settles directly below that paragraph. The caret does not travel; compliance is watched, not trusted. Folding a span that touches an existing fold coalesces it, order preserved.

**Jot (F2).** Same verb, no selection: `ctrl-shift-a` at the caret blooms an empty fold open on the block boundary below the current paragraph. Type the thought; **Esc pleats it shut and returns the caret to the exact mid-sentence position it left.** The paragraph under the hand never moves, no scroll occurs — the unchanged screen is the resumption cue (Altmann & Trafton, via research-theory). No title, no category, no destination decision (Bernstein's law: schema at capture kills capture). A seed is a jot planted wherever the writer stands — or wherever she carries it: folds travel as atoms under ordinary cut/paste.

**Open / close.** Click a fold line → it opens and the click gives a caret at that point (P3 honoured; the collapsed one-liner extends the graveyard's expandable-one-liner contract, P7). Click the crease row, or Esc from inside, → it closes. Open/close is view, not document: unlabeled, reversible, free.

**Unfold (inverse of Fold, P13).** On the crease row. The boundary dissolves; the text lies flat as manuscript exactly where it stands — same spot, same grammar, structure intact. This is F4 at origin, beating the baseline's "find where it came from, paste, restitch." Retrieval elsewhere is plain text-move: open, select, cut, paste at the insertion point — which the evidence (Pressfield's ~1–2%, phrase-grained) says is the right weight.

**Exile.** On the crease row, and `ctrl-shift-g` works on folds as everywhere: the fold's full text goes to the graveyard record; "Put back" there re-creates *the fold* at its origin — the same door it left by (P13). Selecting across a collapsed fold treats it as an atom; deleting such a span routes the fold's text losslessly to the graveyard like any substantial deletion. Arrow keys step over closed folds; the caret never wanders into one uninvited.

**Esc grammar (extended, never repurposed).** Inside an open fold, Esc closes it and restores the caret to its last manuscript position (the jot round-trip); in the fold lens, Esc restores position and selection exactly as find already does. Esc remains "go home."

**The gathered lens (F3) — a view, not a place.** Folds also need gathering, and the answer is a lens: type `folds` in the omnibar (or invoke it as a command) and the palette lists every fold as its one-liner, in document order — the map read top to bottom. Arrowing through the list brings each fold into view, opened; Enter stays, Esc returns home with selection restored. It exists only while summoned, costs zero resting chrome, and serves the one retrieval ritual the practice literature says keeps scrap files alive: the end-of-revision sweep. It is a viewspec over the one document (Engelbart), not a second home.

**Scopes, announced (F5).** The word-count readout counts manuscript only; with the caret inside a fold it reads that fold's own count — the control is the indicator (P12). The export summary states, as data, `6 folds stay behind`. Find searches manuscript *and* folds (Pressfield retrieves by ⌘F; search must reach scraps); the tally reads `14 matches · 3 in folds`, and a match inside a closed fold springs it open while current, re-pleating as you move past (a lawful spring-loaded quasimode). The AI reads exactly what cold read shows — cards simply never anchor in folds; the invariant is shown, never explained (P4).

**Empty state.** Nothing. No container, no seam, no chrome — a document without folds is indistinguishable from today. The feature's resting footprint is zero until first use, which the baseline's own virtue demands.

**First discovery / the invitation (F8).** The writer selects a passage she is about to delete. On the flank menu, beside **Exile**, sits **Fold** — a gentler verb adjacent to the kill verb, resting exactly where the hand already is at exactly the moment the cut hurts (research-tools lesson 6: discovery lives in the selection path; research-practice: permission is the product, consumed at cut-time). No tour, no tip, no empty-state copy (P2): the notch is the verb's position. One notch deeper (P5): the same chord with nothing selected jots; the omnibar lists **Fold** and **Folds** for the curious hand. And the writer who already dumps scraps under a divider at the bottom loses nothing: she can fold her existing pile where it lies — the fold doesn't compete with her convention, it blesses it, gaining her scope-truth without changing her habit.

**Assumptions, named.** (1) The selection flank menu accepts a new verb with chord chip. (2) A word-count readout exists in the chrome; only its scope behaviour is specified here. (3) The document model supports block-level region attributes versioned per checkpoint state (mandated by brief §4; folds sit between paragraphs, never inside one). (4) The omnibar can present navigable result rows, as find already does. (5) The graveyard's expandable one-liner is an established contract folds may extend. (6) Within-app clipboard can carry a fold as an atom; pasted outside, it flattens to plain text.

## 4 · Edge cases

**Find/replace.** In scope, tallied separately (`· 3 in folds`); replace applies wherever find matches — folds are the writer's text; the fold boundary survives replacement.

**AI scope.** Fold contents are invisible to the reviewer — not read even as context; no card ever anchors inside a fold. Margin notes the *writer* anchors in fold text (annotate works everywhere, P8) hide when the fold closes and return when it opens; on Unfold they persist as ordinary margin notes; on Exile they migrate as deleted-anchor notes already do.

**Export & counts.** Excluded, with the count shown at export as data. The titlebar count never lies again — baseline failure 1 closed.

**Time travel.** Fold boundaries are stored per checkpoint state, so every past state keeps its own geometry: a parked past shows the folds of that day, read-only like everything else; Restore appends that state, folds intact; nothing teleports.

**Cold read.** Folds vanish entirely — the prose closes over them with no seam. This is the honest answer to F5's "reader-facing surface strewn with private pockets": the *reader-facing* artifacts (cold read, export) are fold-free by construction; the pleats exist only in the working view, where the litter is the point. Leaving cold read, the pleats return with the rest of the apparatus.

**A 3,000-word scrap pile.** One fold, one line. A *hundred* scraps is the honest stress case: a heavily composted working view striates with pleat lines. Mitigations that exist without new chrome: folds are visually recessive; the lens sweeps them; Exile retires the dead ones to the graveyard. But see §6 — this is the design's real cost.

**A jot mid-drafting-burst.** Writer-initiated, so never held by the door policy; the fold opens instantly, no animation gate, no machine involvement; Esc returns the caret before the sentence cools.

**Narrow and wide.** Folds live inside the manuscript column and inherit its measure; at 800 pt nothing competes for width (no drawer to squeeze), at 1600 pt nothing floats away from the text it belongs to.

## 5 · Scorecard vs. the null baseline

- **F1 Park — beats.** One chord, zero travel, compliance watched at the wound itself; baseline costs cut–scroll–paste–scroll–refind.
- **F2 Jot — beats.** Chord, type, Esc; caret restored to the letter; screen never moves. Baseline costs the full round trip.
- **F3 Skim & return — matches.** The lens wins the return leg (Esc-home, position restored) but scattered scraps lose contiguous read-through against one bottom pile. Honest wash.
- **F4 Retrieve — beats.** Unfold restores at origin, structure and position intact, no memory required; elsewhere both designs are cut/paste.
- **F5 Never intrude — beats.** Export, counts, AI, cold read all scope out folds automatically and visibly; the baseline fails all four (its failures 1–2, 6).
- **F6 One afterlife — beats.** One spatial story: what you folded stays where you left it, living and warm; what you killed lies in the record at the tail, drained and read-only. Shared one-liner form, shared restore-at-origin grammar. Baseline has no story at all (failure 5).
- **F7 Three tenses — beats.** Asides at the text they concern, scope-true (baseline's inline TKs leak into export); parkings with structural provenance; seeds serviceably as planted jots — while global cross-piece seed-keeping is honestly out of scope (per-document v1), as it is for every direction.
- **F8 Invitation — beats.** The gentler verb rests beside the kill verb at the exact moment of hesitation, and the feature costs zero chrome until used; it can even formalize the writer's existing bottom-pile in place.

## 6 · Named losses & risks

The enemy's best line: **the fold keeps every dead darling permanently under your nose.** Obsidian users revolted against extraction that leaves a visible stump — and the fold *is* a stump, by conviction. Writers who park precisely to get cuts *out of sight* — to read their draft clean while revising — get no clean working view short of cold read, which is read-only. A Pressfield-grade composter (culls longer than the book) would striate his draft with dozens of pleats; the cure — curating dead folds into the graveyard — is exactly the maintenance the practice literature says writers won't perform (the collector's fallacy, the morgue dying with its librarians). Second loss: the reading rhythm of the working view is interrupted at every pleat, however quiet; twelve one-liners of not-manuscript are twelve small speed bumps in a skim. Third: fold-as-atom selection behaviour (delete = exile, arrow-over, clipboard flattening) is a genuinely new mechanic in a product whose creed is "typing types"; if it ever surprises, it violates the spirit of P3 at the exact centre of the writing surface. The counterarguments are real (the stump was a *link*, chrome pointing elsewhere — the fold is the writer's own words, one quiet line; the baseline's bottom pile is also always visible *and* leaks into export) — but a writer who wants her cuts gone-from-sight-yet-alive is served better by a gathered place, and this design should lose her honestly rather than grow a drawer.

## 7 · Build sketch

A fold is a block-level region attribute in the document model, versioned per materialized checkpoint state like any other text property, so past states keep their own geometry by construction. Rendering is two block variants in the existing one-column retained-mode UI — a one-line crease block and an open washed block — with open/closed held as session view-state, not document state. Export, count, AI-scope, cold-read, and find-tally all consume one shared region predicate; the lens reuses find's result-row navigation and Esc-restore machinery; no new surface, overlay, or column exists anywhere.

## 8 · The name

**Folds.** The verb is **Fold**; its inverse is **Unfold**; the omnibar lens is **Folds**. One plain, warm word that names the gesture, the thing, and the picture at once — paper folded over a passage you're not ready to lose. It sits naturally beside the product's existing register: you *fold* what might yet live; the *graveyard* keeps what died.

*(~2,150 words)*
