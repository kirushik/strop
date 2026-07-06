# Critique — design-margin.md ("Set aside", the margin-native scrapyard)

Panel: Birman / Raskin / Norman · 2026-07-05
Verdict: **needs-surgery** · vs null baseline: **beats** (where it works, it wins big; where it fails, it fails on the constitution, not on the baseline)

---

## Ilya Birman — the grammar

The strongest grammatical move here is Put back: return-a-fragment-to-its-origin, one verb, one meaning, same sage flash in both afterlives. That lands exactly on ux-glossary.md's Put-back row without having seen it. Graft it whatever happens.

Four faults.

**The noun re-opens a closed decision.** ux-glossary.md ("aside / set aside") ruled the *verb* is the only UI form; the noun stays internal, for a named reason: Russian has отложить but no scrap-noun. "A card is **a set-aside**" is the exact form the glossary forbade. Cheap fix, but it is a standing decision, not taste.

**Chirality.** asides.md §4 is one sentence of law: left = the writer's materials and tools; right = the conversation *about* the text. The reason behind it: every future side-of-screen call becomes a consequence instead of a debate, and it keeps the right lane semantically pure — *anchored means concerns this passage*. Set-asides are material wearing a conversation's address. The seed-jots corrupt the anchor grammar outright: a margin note means "about this sentence"; a seed anchored "where it struck" is about nothing there. Parallel form, two meanings — P8 broken at the lane's core semantic. And when the anchor prose is deleted the seed *migrates*, drifting somewhere it is even less about.

**Chrome on the living pile.** The rest state is a one-liner with a word-count badge; expanded adds a date stamp and an action row. asides.md §5 refuses precisely this list on the deliberate pile ("no per-item buttons… counts… badges"), and P3's born-from wound is this exact shape: "things on the left are not the cards; probably just paragraphs." The design borrows the graveyard's fold — but that fold is lawful on the *dead* pile because a record is the machine's shape (P3). The living pile is the writer's text; here it answers a click with an expand-toggle, not a caret.

**One form, two meanings inside the lane.** Receded-one-liner already means "older machine card over budget." Now it also means "writer's parked prose at rest." Typography-at-one-line is a thin corroborator.

## Jef Raskin — modes

**The unlatched Esc-return is a hidden mode.** "Esc with nothing focused, after a skim-scroll, returns the view to the caret" — whether Esc teleports the view depends on invisible scroll history. What clears the pending return — a click? typing? Unspecified. 07-compost.md §3 (R3) already learned this lesson: the return must be guarded by an explicit excursion latch, "never raw caret position." Re-invented, minus the fix.

**Esc-commits kills the abandon gesture.** Consistent with the product's commit-on-blur, yes — but it subverts the composer's ancestor contract (Esc = discard) everywhere at once (P7). There is now no way to *not* file what you typed; a habituated Esc turns keyboard noise into a note. Delete-it-afterwards is a different door, which is what P13 forbids.

**History-dependent display state.** "One expanded at a time" plus find auto-expanding the current match means a card's fold state depends on where find has been. Does it re-fold when find leaves? Every answer makes a still screenshot unexplainable without the session's history (P6).

**Position is a mode too.** The whole story is "beside the seam it left," but set-asides pack at *lowest* priority and displace down-lane. In a busy lane the compliance card lands somewhere else, silently. F1's "compliance visible from where the writer sits" is true only in an empty lane.

## Don Norman — first contact

Minute 1: she opens; lands in the story. Good — F5 is genuinely structural here.

Minute 3: she selects a doomed paragraph; the flank offers **Set aside** — a guessable verb resting exactly where the hand already is. This is the best F8 mechanism on the table.

Minute 4: she presses. The text vanishes; the gap closes. Now the misread: the product has taught her one reflex for vanished text — the footer chip, the graveyard. She checks it. Her paragraph is not there (moves never file to the graveyard — asides.md §0, "the trigger is deletion, not departure"). The one place she was taught to look is empty, and the warm one-liner "settled" peripherally among notes she wasn't watching, with no arrival blink — the product's established grammar for "something arrived over there" (asides.md §2, item 3) is simply omitted. F6 fails at the precise moment it was supposed to shine.

Minute 10: a stray thought. F2/F8 for seeds hangs entirely on bare-caret ⌃M — a chord with no visible resting place (the flank rises only on selection), which by the design's own assumption 3 isn't even lawful today. This is extraction-audit papercut #1 ("no from-scratch entry… UI teaches none of it," 07-compost.md §2, F2 ✗) re-invented intact. P5: the floor is missing; two of the three tenses are keyboard-secret.

Month 2: the sweep. No gathered pile, no presence chip, no count anywhere — she cannot even learn *how many* scraps she has. And the wound stays visible: the design's own §6 concedes the writer who parks to forget is better served by the baseline's bottom pile.

## Panel synthesis

**Re-invented failures (blindness was by design; these are the faults):** the card phase for writer scraps (P3 born-from; asides.md §1, §5); the missing from-scratch jot entry (07-compost.md §2 / extraction audit #1); the unlatched excursion return (07-compost.md §3 R3); the omitted arrival blink (asides.md §2.3).

**Contradicted standing decisions, with their reasons:** chirality (asides.md §4 — side-of-screen as law, right lane = about-the-text); the noun ban on "aside" (ux-glossary — untranslatable); item chrome on the living pile (asides.md §5 — the writer's box gets no badges).

**Kill-shots** (all testable, listed in the verdict JSON): jot has no floor (F2/F8/P5); parked text missing from the taught afterlife path (F6/P6); writer text as widgets at rest (P3); the sweep loses to the baseline (F3); seam-anchoring and Esc-return both silently degrade (F1, Raskin's latch).

**Why needs-surgery, not fatal:** F1 and F5 beat the baseline structurally, not cosmetically — one chord, zero travel, and scope purity *by construction* because the column/margin boundary is the one boundary all four scopes already respect. That insight survives even if the lane does not. Surgery required: a visible jot floor; de-widget the rest state; an arrival blink; a gathered inventory (or an honest F3 "loses"); latch the return; drop the noun. If the pile-lessness proves incurable against a tail-pile rival, the verdict hardens to fatal — the hoard risk (§6) is structural and the design says so itself, to its credit.

**Grafts:** in the JSON — scrapyard-as-state (reuse the already-respected boundary); the two-direction afterlife grammar (sideways = living, down = dead); count-drop as the park's receipt; find's split-count announcement; diagnosis-card retirement on park.
