# Design — Set aside (the margin-native scrapyard)

## 1 · The bet

A scrap is a note to self that happens to be written in manuscript, and
notes-to-self already have a home the writer's eye already visits: the
right margin. More decisive: the column/margin boundary is the one
boundary every subsystem respects *today*. Word count, export, the
caged reviewer, and cold read are all defined over the column; the
margin was never inside any of them. Every rival design must invent a
boundary and then teach four scopes to honour it; this design reuses
one that ships (the theory digest's Engelbart point: the baseline's
missing half is machine-known structure plus views, not a new
container). So the scrapyard is not a place but a **state**: parked
text steps sideways out of the column into a warm, receded card
anchored where it left — the same text relocated, origin remembered
(Nelson's transclusion; Final Draft's Alt Dialogue is the loved
precedent for parkings anchored at origin). The margin is also the
writer's oldest parallel surface — Berthoff's facing page — where
drawers and shelves are office furniture. And peripheral glanceability
is the win no other shape can offer: the scraps that matter during
revision are the ones cut from the region under the hand, and they are
already in view when the hand is there.

## 2 · The scene

At rest: the prose column; in the lane, a warm note beside one
paragraph, a cool machine card lower down, and between them a single
warm line set in *manuscript type*: "The dog had bitten the football
twice before anyone — · 214 words". A stranger concludes correctly:
this is a folded piece of the story, standing beside the story; it is
the writer's (warm, prose-set), not the machine's (cool, card-set);
there is more under the fold (the count says so, statically — P6, P9).

In use: the writer selects a paragraph; the familiar flank menu rises;
among the verbs sits **Set aside ⌃⇧A**. On press, the paragraph leaves
the column, the prose closes the gap, a warm one-liner settles in the
lane at that height, and the word count in the footer drops by 178.
Caret unmoved. Every frame reads as a still: a seam, a card, a smaller
number (P6). Click the line and it expands: full text, editable,
manuscript-formatted, a date stamp, and one action row — **Put back**.
Esc folds it and returns the caret home.

## 3 · Behavioural spec

**States.** A set-aside is *receded* (default: first words + word
count, one line), *expanded* (full text, scrolls internally if tall),
or *being edited* (caret inside; ordinary text — P3). One expanded at a
time; expansion is a reading state, never persisted layout. Set-asides
are born receded and pack with the lowest lane priority — active item,
then notes and machine cards, then set-asides — so they displace
down-lane first and never claim the lane's anchor object (P11). Colour:
warm ground (the writer's — P10), corroborated by form: set-asides wear
manuscript typography where notes wear note typography, so the two warm
kinds never rely on colour alone (P10, P8: different meanings,
different forms).

**Verbs & keys.**
- **Set aside** — flank menu + ⌃⇧A (the existing move-selection-out
  chord; A as in aside). Losslessly relocates the selection into a
  receded card anchored at the cut seam. No travel, no dialog, no
  category picker (Bernstein: schema-free capture or writers keep the
  divider).
- **Put back** — the expanded card's action row. Reinserts the full
  text at its anchor; the card vanishes; the seam flashes sage
  (returned — P10's sanctioned meaning; same flash the graveyard's Put
  back wears). Inverse in the same grammar, same door it left by (P13).
- **Annotate ⌃M** — unchanged; with a bare caret it anchors a note at
  the current sentence. *This is the jot.* One verb everywhere (P8).
- **Text mechanics** — click gives a caret in any card; select, copy,
  paste, format as anywhere the writer's words live (P3). Fragment
  retrieval to an arbitrary point is ordinary copy-paste, deliberately:
  evidence says retrieval is ~1–2% and phrase-grained.

**Focus & Esc.** Esc in a card or jot composer commits whatever was
typed (nothing destroys silently — P13; an empty composer just closes)
and goes home to the prose caret. Esc with nothing focused, after a
skim-scroll, returns the view to the caret — an extension of go-home
(it already restores your place leaving find), never a repurpose.

**Three tenses (F7).** One lane, two forms it mostly already has.
*Asides* are margin notes — already shipped, anchored at the point they
concern, which is where the practice digest says asides live (the TK
shape). *Parkings* are set-asides. *Seeds* are jots, captured at the
sentence where they struck — Tharp's box: seeds of *this* piece. The
global spark file is per-document v1's honestly unserved tense; the
practice digest itself recommends folding seeds into jots rather than
faking a global home.

**Empty state.** None exists. A margin without set-asides is the
margin. The scrapyard-as-place has an empty-state problem; the
scrapyard-as-state has nothing to be empty.

**First discovery / the F8 invitation.** The verb rests in the
selection menu — where the hand already is — at the exact moment the
practice is born: the hesitation before a deletion (practice digest:
F1 *is* the whole game; tools digest: discovery dies in menus, lives in
the selection path). First use teaches the entire system by watching:
where the text went (visible, beside the seam), that it is still hers
(warm, editable), how it returns (expand → Put back). No tour, no tip,
no copy (P2, P4).

**Assumptions (named).**
1. The lane exists at every supported width (≥800 pt), narrower but
   present.
2. Margin items live in the document file and materialize per
   checkpoint state; set-asides inherit this.
3. Bare-caret ⌃M is lawful (or is extended to be) — anchors to the
   current sentence.
4. The reviewer reads the column only; the writer's margin text is
   never sent to the machine.
5. The graveyard's Put back inserts at the nearest surviving origin
   when the origin is gone; set-asides mirror it.
6. A persistent word count is visible (titlebar or footer).
7. Find today scopes the column; extended below.

## 4 · Edge cases

**Find/replace.** Find covers the column plus the writer's margin text
(notes and set-asides), never machine cards. The count announces the
split as data: "12 in the piece · 3 set aside" (P4). A match inside a
receded card shows it expanded while current. Replace applies wherever
find matches: renaming a character renames her in parked scenes — the
baseline grants this too; external-file scrapyards silently break it.

**AI scope.** Column only (assumption 4). Parking a diagnosed passage
retires its diagnosis card — the prose it judged has left the piece;
put back makes it eligible for the next pass.

**Export & counts.** The column is the piece; the margin was never in
either scope. The announcement is the number moving: the count visibly
drops at the instant of parking. Export needs no new checkbox and no
trimming ritual — baseline failure 1 evaporates structurally.

**Time travel.** Set-asides are lane content stored per materialized
state (assumption 2); every checkpoint keeps its own geometry;
restoring never teleports text. A card in a parked past state sits
exactly where it sat.

**Cold read.** All apparatus hidden, set-asides with it; the read is
manuscript-pure by construction (baseline failure 2 gone). No parking
inside cold read; Esc leaves it first.

**Notes anchored in parked text.** Annotations ride with their text
into the card and back out — the moved text is the *same* text
(Nelson). While parked they render inside the expanded card. A note
*about* a set-aside is written into the card: writers already label
scraps in-text ("cut from ch. 2 — too much backstory").

**A 3,000-word pile.** Distribution is the defence: cards sit at their
origins, so no single screen hosts many; a lone 3,000-word set-aside is
one line at rest and scrolls internally expanded. A pathological
cluster (ten cuts from one page) stacks in cut order and displaces
down-lane; put back still targets each true origin. Conceded plainly:
there is no single gathered list (§6). If the origin prose is later
deleted entirely, the card migrates rather than vanishes — the lane's
existing rule.

**A jot mid-burst.** ⌃M is writer-initiated, so the door policy is
untouched. The composer opens beside the current line; caret, selection
and scroll stay put — the unchanged screen is the resumption cue
(Altmann & Trafton); Esc commits and returns. Cost: seconds, screen
never changes.

**Narrow and wide.** At 800 pt the lane narrows; one-liners truncate
harder but keep the word count (the static "more here" signal);
expanded cards scroll internally — reading a long scrap at 800 pt is
cramped, conceded. At 1600 pt the same forms, comfortable (one form
everywhere — P8).

## 5 · Scorecard vs the null baseline

- **F1 Park — beats.** One chord, zero travel, compliance lands beside
  the caret; the baseline is cut-scroll-paste-scroll-refind.
- **F2 Jot — beats.** ⌃M at the caret; the screen never changes; the
  baseline round-trips to the bottom mid-sentence.
- **F3 Skim & return — beats.** Scraps cut from the working region are
  already in peripheral view; distant ones skim as one-liners in situ,
  each beside the seam it left, Esc-home after; no residue. (The
  gathered-list loss is honestly priced in §6.)
- **F4 Retrieve — beats.** Put back restores origin structurally,
  which the baseline cannot remember; phrase-grained retrieval is the
  same copy-paste both ways.
- **F5 Never intrude — beats.** The margin was never in any scope;
  count, export, reviewer and cold read exclude it by construction, and
  compliance is visible as text leaving the column and the count
  dropping.
- **F6 One afterlife — beats.** One dialect (receded one-liners,
  expand, Put back, sage return) in two directions with meaning:
  sideways = warm, living, yours; down to the tail = drained, dead,
  recorded. The baseline's two afterlives are unrelated (its named
  failure 5).
- **F7 Three tenses — matches.** All three in one lane: asides
  genuinely better (at the point they concern), parkings better, seeds
  honestly worse — scattered at capture points, no single re-readable
  spark file. Net: a match, not a win.
- **F8 Invitation — beats.** The verb rests in the selection path at
  the exact moment the practice is born; the baseline must be invented
  by the writer before it can be used.

## 6 · Named losses & risks

**The hoard.** Pressfield's CULLS file is *longer than the book*. The
lane is a narrow column also needed by live notes and the machine's
cards; a heavy parker turns periphery into storage. Born-receded,
lowest packing priority and spatial distribution thin the load, but the
risk is structural: this design bets scrap volume distributes across
the document, and a writer who parks fifty darlings from one chapter
will feel the lane groan.

**No gathered pile.** The one reliable retrieval ritual — the
end-of-revision sweep — reads a contiguous pile in the baseline; here
it is a full-document scroll. I claim the in-context sweep is better
(each scrap beside the seam it left; "should this go back in?" is
answerable on sight), but the enemy says it plainly: a yard you must
walk the whole property to inventory is not a yard, it is litter.

**The wound stays visible.** Anchoring a parking at its origin keeps
the rejected version in peripheral view exactly where the writer is
rewriting. Some writers park in order to *forget*; the baseline's
bottom pile forgets better. The recede-to-one-line floor is the only
anaesthetic offered.

**Seeds scatter.** The spark file's documented power is one
chronological list re-read whole; seeds anchored where they struck
cannot be re-read as one document.

**The Spike risk.** Born-receded one-liners are quiet; too quiet and
set-asides go unseen and the practice dies of invisibility, like Word's
Spike. The invitation lives entirely in the selection menu — a narrower
door than a permanently visible edge (NN/g: hiding halves discovery),
mitigated only by the menu sitting in the path of every selection.

## 7 · Build sketch

One new lane-item kind in the existing packer (lowest priority, receded
default, expand state reusing the over-budget card mechanics); park and
put-back are rope splices between the column and the card's text,
recorded in history like any edit; margin content already materializes
per checkpoint state (assumption 2), so boundary versioning is
inherited, not built; find gains one scope extension over writer margin
text with a split count; the jot reuses the note composer and focus
path. No new surface, no new focus system, no new animation beyond what
the lane already performs.

## 8 · The name

The verb is **Set aside** (⌃⇧A); a card is **a set-aside**; the inverse
is **Put back**. There is no place-name because there is no place — the
feature is named by its verb, the way the writer already says it: "I
set that bit aside." ("The wings" — text waiting beside the stage — was
considered and rejected as decoration; the writer's own idiom needs no
metaphor, P4.)
