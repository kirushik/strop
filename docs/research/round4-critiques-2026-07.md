# Round-4 critiques — the Birman audit and the corridor test (2026-07-05)

*(Two commissioned reviews of the round-4 deliverables, preserved
verbatim. The Birman persona audited the four fresh specs
(design-principles.md, history-strip.md v2, asides.md, ux-glossary.md)
before the lab was built; every load-bearing finding was integrated —
see golden-path.md §9.3 for the ledger. The corridor test ran a
fresh-eyes persona (Vera, 63, retired literature teacher, no prior
exposure) against the built lab-v4 screenshots; it produced the
page-fill fix, the key-cap shortcut styling, and the legibility bumps,
and it is the round's evidence that the strip's media-player floor
holds.)*

---

# Part I — The Birman audit

Review complete. All four documents read, plus color-language.md. Findings below, per document, ranked by severity. Principles cited by number (P1–P10) or by name where it is my own doctrine the constitution does not yet contain.

---

# 1. asides.md — the compost rail (primary target)

**Verdict:** The direction is right — paragraphs, not cards; text mechanics, not item chrome. But the spec confuses *absence of chrome* with *invisibility of the interface*. Information itself is the interface; an empty text region contains no information and is therefore not an interface. And the "one continuous text" model is asserted, not finished: the separator semantics, the accounting, and the flank geometry all leak. Ten findings.

**A1. [critical] The empty rail is a surface that does not exist.** "An empty rail shows a bare caret slot, nothing else" — what is a bare caret slot? A resting caret is a lie (a caret means focus); an invisible typeable strip of margin is a false *non*-affordance — the one region of empty page that accepts typing, indistinguishable from every region that doesn't. This fails the corridor floor (P5) by the spec's own standard: the floor works when the surface *looks like something already known*; empty margin looks like nothing. Worse, your own glossary rule closes the escape hatch: "a metaphor may live in the UI only where its referent is visible" — an empty compost has no visible referent, so it cannot even be labeled. **Fix:** stop pretending "click below the last one and type" is the floor. The floor entry is the *aside verb* on a selection — discoverable, deliberate, and it *births* the rail visibly (the arrival blink lands on a rail that now exists). Direct typing into the rail is the drill notch, discovered once the rail has content and a visible tail. Rewrite §1's first bullet accordingly; the current answer presumes a "last one."

**A2. [critical] Separator semantics are unresolved, and the render as spec'd will lie.** Three collisions: (a) *blank line = boundary* plus *no auto-tidying* (§5) means consecutive blank lines are legal — do they render as stacked hairlines, or collapse? If collapsed, the render lies about the text: press down-arrow, the caret moves through a paragraph the eye cannot see. WYSIWYG betrayal. (b) Can the caret stand *in* the separator? If the hairline replaces the empty line's height, there is a model position with no visible line to sit on — text mechanics betrayed at the exact place the writer merges or splits items. (c) A prose selection containing a scene-break blank line, sent via the aside verb, silently arrives as *two* items — or the software eats the blank line, which is a mutation of the writer's text (P1). **Fix, all three at once:** the separator is exactly one empty paragraph, rendered at full line-height, hairline drawn at its midline, caret placeable on it, one hairline per empty paragraph, never collapsed, never synthesized. Then backspace-at-item-start visibly removes the hairline and merges — honest, operable, screenshot-true (P6). And a clipping containing a blank line honestly arrives as two items; the spec should say so out loud rather than hide it.

**A3. [critical] Data-model contradiction: the aside verb double-files.** §0: the graveyard receives prose "automatically, on every prose cut." §2: the aside verb "is a *move*: the prose loses the text." A move is a cut. As written, every aside lands in both piles — a compost item and a graveyard corpse of the same words. **Fix:** define the graveyard trigger as *deletion*, not *departure*: text that leaves prose for another writer-owned region is a move and files nowhere else. One sentence in §0; without it the two-pile table's clean boundary is fiction.

**A4. [high] The orphaned-note arrival is invisible where visibility is owed.** The only signal is "the same single blink" — transient. Fail the screenshot test on the *event* (P6): a still taken two seconds later shows a bare quoted line plus a note at the compost tail with zero provenance. "Text quoting text, the oldest convention" — no: the oldest convention carries attribution; a naked quoted fragment is just italic text. The writer returns tomorrow to a mystery. **Fix without words:** margin notes already display their anchor fragment in some typographic form; when the note moves to compost, the anchor line keeps *that exact form* — one grammar for anchor fragments everywhere (P8). A writer who has ever seen a margin note recognizes the pair instantly, and the form is durable, not a blink.

**A5. [high] The left flank lands on top of the rail — a literal P1 violation.** §4 puts the formatting flank "at the same offset from the text column" on the left; the compost rail also lives left of the text column. When a prose selection raises the flank, it floats over compost items — chrome ON writer text, which P1 forbids in its first paragraph. And when a selection *inside the rail* raises the flank, where does it go — further left, off-window? **Fix:** decide the gutter order now: flank rises *between* rail and prose (a reserved gutter), or the rail slides left to yield (a re-pack, you have the motion machinery). Draw it; this is a mockup-breaking unknown.

**A6. [high] Chirality contradicts itself on the aside verb's home.** §4's sentence: left = workshop (materials, tools), right = conversation. §2 puts the aside verb in the "palette / selection menu" — and the selection menu specified in §4 is the *right* flank of "Ask the editor…" sentences. Aside is a workshop act on the writer's material; by your own sentence it belongs left. But the left flank is defined as a *closed grid of eight toggles*, and aside is an action verb, not a toggle — adding it breaks the closed set that justified the grid. The two principles collide and the spec doesn't notice. **Fix:** state that chirality governs *resting regions* (rails, footers), while at-hand menus go where the hand is; or give the left flank a visually separated action row (see A10). Pick one; don't leave it to the implementer.

**A7. [mid] The accounting bleeds, and "zero new machinery" is false.** The compost is "a region of the document," its edits "warm flecks like any others." Then: does "3,412 words" in the strip readout count compost? Do session targets? Does the envelope step when a 400-word clipping arrives in the rail? Where on the strip's y-axis (position in document) do rail edits land? Almost certainly the manuscript alone must be counted — which means the strip, the readout, and the counters all need compost-awareness. That is new machinery; budget it. **Fix:** one rule in the spec: *counts and envelope are manuscript-only; rail edits are flecks rendered [in a thin sub-lane / excluded]* — decide which and draw it.

**A8. [mid] The parking "reflex" requires two precise mouse trips.** An idea mid-sentence → click the rail tail, type, click back into prose *at the exact word you left*. That is not a reflex, it is an excursion, and the spec doesn't even promise the prose caret survives. **Fix:** one key to the compost tail, Esc (or the same key) back, prose caret restored exactly. This is the canonical drill notch (P2's demand) and it costs one paragraph of spec.

**A9. [mid] The rail has no anchor object.** Uniform small type, hairlines, nothing dominant — a grey mush that gives the returning eye nowhere to land. Every surface needs one anchor everything else hangs off. **Fix:** the tail is the anchor — the live end, where arrivals blink and the caret slot sits; give it the one quiet distinction (a resting baseline mark after the last item — which simultaneously solves "where do I click to append" for the populated rail). Do *not* reach for freshness-desaturation of old items: "drained" already means stale/unverified in the color language and reusing it for mere age would corrupt the token (P10).

**A10. [low] Loose ends that will surface in the first build:** (a) cut-to-reorder boundary rules undefined — does cutting an item take its trailing empty paragraph? Specify: selection triple-click-style item selection includes the trailing separator, or you'll get orphan hairlines on every reorder. (b) "Drag the selection… the text widget's own native contract" — verify your TextField actually implements drag-of-selection before citing the contract; a documented affordance the widget doesn't honor is a lie in prose form (P7). (c) The 2×4 grid mixes six instant toggles with two argument-takers (link, footnote) — parallel form for non-parallel behavior (P8): six cells toggle, two open an input. Separate them visually (2×3 + a distinct pair). (d) Rail scroll model unspecified: independent of prose, opens at tail, position persisted — say so. (e) Minimum measure: a multi-paragraph clipping at reduced size in a narrow column is a wall of 25-character lines; set a floor (~35ch) and define what the rail does below it.

---

# 2. history-strip.md v2

**Verdict:** The inversion is correct and the floor is real — scrub-over-texture is a proven pattern (SoundCloud's waveform passes corridor tests daily), and "the safety is structural, so a warning would be a confession" is the right sentence. But the seek-bar claim is only true at blog-post scale, and two contract details are unfinished. Six findings.

**H1. [high] At novel scale the seek bar stops being a seek bar.** A seek bar's contract is *the whole duration, always visible*. Fixed quant with horizontal scroll means an 80k-word novel (~300 working hours) is ~36,000 px — fifteen viewport-widths, navigable only by scrolling blind. And "Now stays pinned at the right edge regardless of scroll" makes a still image lie: Now sitting adjacent to two-week-old fabric with no encoding of the fold (P6, P7 — the borrowed face subverts the borrowed contract exactly where the corridor tester would trust it). **Fix that keeps both truths:** split the two jobs. The *rail with the thumb* is always full-extent — a true seek bar, thumb x = position in the whole history. The *fabric band* below keeps the fixed quant and auto-scrolls to keep the thumb's neighborhood in view; the playhead line passes through both, binding them. The corridor floor stays intact at any length; the quant stays learnable; nothing re-scales under the eye.

**H2. [mid] Click-to-jump is missing from the contract.** Every media player jumps on rail click; the spec defines only drag. Omitting click subverts the very widget you borrowed (P7). Also undefined: wheel/trackpad horizontal scroll over the band versus thumb drag — two horizontal gestures, one surface. **Fix:** click on rail = park there (with the same live-render); scroll gestures pan the fabric only (moot for the rail under the H1 fix, since the rail no longer scrolls).

**H3. [mid] The "Saved" automatic station contradicts your best promise.** The product saves every keystroke; a station named "Saved" teaches that unsaved states exist. One word quietly re-imports the fear the whole strip exists to kill. **Fix:** delete "Saved" from the automatics. Started, Exported, writer-named, seal-derived — events that actually mean something happened.

**H4. [mid] The parked state's two exits sit at opposite ends of the screen.** Parked in the past, the writer faces a decision — keep this (Restore, far left by the readout) or leave (Now, far right). The choice never reads as a pair; parallel meanings, scattered forms (P8). The Restore-on-park appearance itself is *correct* contextual grammar — the readout says where you are, the verb acts on that where, and the fixed-width chip means no reflow — keep it. **Fix:** when parked, Now brightens in the same beat Restore appears, so the two exits announce themselves as the pair they are. And spec the accidental-keypress story explicitly: typing-restores is right (Raskin), but write down that the pre-restore now is itself one Restore away — the recovery must be discoverable in the same grammar.

**H5. [low] Readout format — right shape, four locale holes.** "Tue 12 Jul, 21:40 · 3,412 words" is good: data only, no sentence, tabular numerals. But (a) no year — histories never expire, so "12 Jul" is ambiguous across years; add the year when ≠ current. (b) Fixed width must be reserved per locale («Вт, 12 июл, 21:40 · 3 412 слов» — different metrics, and Russian uses space, not comma, as thousands separator). (c) 12/24h by locale. (d) The Compare notch's "delta line" contradicts the fixed single-line chip — reserve the second line's height always, or fold the delta into the chip's width.

**H6. [low] Envelope edge case:** y-scale fixed at open, current length fills the band — but Restore can produce a *now* longer than the open-time length (restoring a pre-cut state). The envelope then exceeds the band. Define it (headroom margin, or the one sanctioned re-scale on restore, honestly animated).

---

# 3. design-principles.md

**Verdict:** Genuinely good — principles with wounds attached, short on purpose, P4/P6/P7 are exactly right. But the constitution is missing the two doctrines both companion specs already obey by instinct, and P1 has a hinge that will be litigated every round. Five findings.

**C1. [high] No hierarchy principle.** The strip spec writes "the thumb and rail carry the highest contrast; the fabric sits low-mid" — correct, and citing nothing, because nothing exists to cite. The compost rail spec, lacking the principle, shipped a surface with no anchor at all (A9). **Fix — add P11:** *Every surface has one anchor object; the contrast budget is spent on it and everything else subordinates.* Born from: the rail's mush and the strip's instinct.

**C2. [high] No control-is-the-indicator principle.** The glossary's door pair implements it perfectly — the state lives as the label of the control that changes it — but ad hoc. Without the law you will eventually grow a status bar: state displayed in one place, changed in another, the classic split. **Fix — add P12:** *State is shown by the control that changes it; a status display separate from its control is forbidden where the control can carry the state.* (Now dim-at-now is another instance already in the specs.)

**C3. [mid] P1's "to make a point" is a hinge made of intent.** The graveyard excerpts prose; the compost anchor-quote excerpts prose; both are legal only because of the qualifier "to make a point" — an intent test that every future review will argue about. **Fix:** replace intent with an enumeration: the software may *record* the writer's text verbatim and may *relocate* it as still-editable text; it may never *decorate* it, *quote it rhetorically*, or *wear it as chrome*. Same law, mechanical test.

**C4. [mid] Reversibility is folklore, not law.** Put back, appending Restore, typing-restores — the product's strongest trust claim ("nothing here is destructive") is distributed across artifacts with no principle above them. The next feature designer isn't bound by it. **Fix — add:** *Every verb has an inverse in the same grammar; nothing destroys silently.* Born from: the graveyard and the strip, retroactively.

**C5. [low] The carrier sentence is a sanctioned chattiness channel with no budget.** P4 plus glossary register 2 together permit any explanation to smuggle itself in as a "carrier sentence." **Fix:** cap it in P4 — carried terms appear only in *action* rows (menu items, button phrases), never on passive surfaces, one carried term per sentence.

---

# 4. ux-glossary.md

**Verdict:** The three-register scheme and the function-over-metaphor localization rule are correct and rare. "The editor"/«редактор» and "version"/«версия» are the right calls. The door pair is the one serious miss. Six findings.

**G1. [high] "Let the editor read" / "Working alone" mixes grammatical kinds — the classic ambiguous toggle.** One face is an imperative action, the other a state description: the writer cannot tell whether the label shows what *is* or what *clicking does* (P8; and it violates C2 above). Worse, "Working alone" has the software narrating the *writer's* state — off-register; the control is about the machine's behavior, not a caption on the user. And in Russian it is a gender trap: «Работаю один/одна» forces the software to gender the writer. **Fix:** a parallel *state* pair whose subject is the editor, carried on the editor control itself: EN **"Reading" / "Away"** (presence grammar — borrowed from every messenger, corridor-free; and presence honestly implies the editor never initiates, which is P2's promise anyway), RU **«Читает» / «Не смотрит»** — parallel, gender-free, three syllables each.

**G2. [mid] "2 reads ready" should not pass your own bar.** A numeral plus a jargon noun is a badge wearing a fig leaf, and the Russian is stilted («2 прочтения готовы» — nobody says this). **Fix:** sentence forms only ("The editor has read it" / «Редактор прочитал») — or better, no count at all: the cards in the margin *are* the count. Information is the interface; don't caption it with arithmetic.

**G3. [mid] "aside → отложить translates cleanly" overclaims.** The verb, yes. The noun does not exist: Russian has no scrap-noun for "an aside" («реплика в сторону» is theater, nothing else fits). Safe only if the noun never reaches chrome. **Fix:** write the constraint into the row: *verb only in UI; the pile is named by "compost," the noun "aside" is internal.*

**G4. [low] Put back vs Restore — assign the Russian now, and record the why.** Two verbs for two operations a writer may conflate ("bring back what was"). They survive P8's one-verb law only because the objects differ: fragment-in-place vs whole-document. Say that in the table, or a future cleanup merges them. Russian assignment, fixed now before translators improvise: **Put back = «Вернуть»**, **Restore = «Восстановить»** — happily distinct, both single words.

**G5. [low] "компост / кладбище both carry" is asserted, not tested.** «Компост» in Russian is purely agricultural — no craft-literature resonance to lean on; risks reading as comedy rather than warmth. «Кладбище» lands *heavier* than "graveyard": English writing culture has "kill your darlings" softening it into wryness; Russian doesn't, and a 60+ tester may find a cemetery next to her text morbid, not self-deprecating. **Fix:** corridor-test both words in Russian before committing; hold functional fallbacks ready («вырезанное» for the graveyard — plain "what was cut"). Your own rule — function over metaphor at the language border — may apply to your two favorite metaphors.

**G6. [low] "The editor" has a homonym problem the table doesn't note.** Strop *is* an editor — текстовый редактор — in both languages. The word works only under a standing rule the glossary should state: *the product never calls itself an editor in any UI string; the word belongs to the person.* One row, cheap insurance.

---

**Summary for triage:** the compost rail needs a revision pass before mockups — A1 (empty-state floor), A2 (separator model), A3 (double-filing) are each mockup-blocking. The strip needs one structural change (H1: full-extent rail over fixed-quant fabric) and is otherwise shippable. The constitution needs two new principles (hierarchy, control-is-indicator) that both existing specs already obey unknowingly. The glossary needs one relabel (the door pair) and Russian corridor tests for the two pet metaphors.

---

# Part II — The corridor test

TRANSCRIPT — Corridor test, subject "Vera" (63, retired literature teacher; daily Word / WhatsApp / YouTube / e-reader user; no prior exposure to Strop)

---

**SCREENSHOT 1 — the writing view**

*What is this?*

"Well, it's a story. Someone's writing a story about a ferry — the text in the middle is the manuscript, that's obvious, and it's set in a nice serif, like a real book page. I like that immediately; it doesn't look like a spreadsheet pretending to be a page, which is what Word looks like these days.

On the left there's a narrow column that says... COMPOST. Hm. I had to squint — it's very small and very grey, that pale lettering is exactly the kind of thing my eyes skate over. Compost. As in the garden? Underneath it there are scraps — 'Premise B (dead)', an ending written out in bold, a lovely line about salt on the railing, and some items that are crossed out. Oh — I see. It's the writer's scrap heap. Ideas rotting down into soil. That's actually rather good, as a metaphor — I'd have my students keep a notebook like this. The crossed-out 'beats' are things they've decided against but didn't throw away. So this column is notes-to-self, not part of the story.

One paragraph in the middle is painted yellow-brown, and there are two little boxes floating next to it. The left one I recognise — B, I, U, that's bold, italic, underline, same as Word. The 'ab' in a little box, I don't know — and the one that looks like arrows and brackets, '</>' — no idea, that looks like something for programmers, I'd leave it alone. The right-hand box is a little menu: 'Add a note', 'Set aside, out of the story', 'Send to the graveyard', 'Ask the editor about this…'. So I've selected that paragraph — or the writer has — and these are things I can do to it. Full sentences, plain verbs. I'll say this: 'Send to the graveyard' made me laugh out loud. Morbid, but I understood it instantly, which is more than I can say for most icons.

And at the very bottom there's a thin grey strip: 'Graveyard · 1'. So there's one body in it already."

*What would I click first?*

"'Add a note.' That's the least frightening one and I know exactly what it will do. The menu items look clickable — they light up like buttons, they have little pictures. The Graveyard strip at the bottom looks clickable too, like a drawer that would slide open. The crossed-out things on the left — I'd want to click one to see if it uncrosses, but I wouldn't dare on someone else's manuscript."

*What confuses or worries me?*

"'Ask the editor about this…' — which editor? Is there a person? Is it the computer pretending to be an editor? That asterisk-flower symbol next to it tells me nothing. I would not click that one until someone told me who answers. Also those single letters off to the right of the menu — m, a, x, e — floating there like a crossword. I assume they're shortcuts but they look like debris. The '</>' button I would never touch, and honestly the whole COMPOST heading is too faint — grey-on-cream, six-point type. I'm sixty-three, not a hundred and three, but come on."

*Would I need to study anything first?*

"The left column and the menu, no — I understood those in a minute, and I'm pleased with myself, which is rare with new software. The little formatting box, half of it. 'Ask the editor' — that one needs a sentence of explanation somewhere before I'd trust it."

---

**T1 — "You deleted a paragraph yesterday you now regret. Can you tell from this screen whether it's gone forever?"**

"My eye goes straight to the bottom: 'Graveyard · 1'. There's a menu item that says 'Send to the graveyard', so deleted things must go there — and there's one thing in it. I'd click that strip and pray my paragraph is the one body in the plot. So no, I don't think it's gone forever — the program is clearly the sort that keeps corpses.

But — be honest with me — yesterday I would have deleted it the way I always do, select and press Delete. Did *that* go to the graveyard, or does only the ceremonial 'Send to the graveyard' go there? I can't tell from this screen. If I open the graveyard and it's not in there, I have nowhere else to look. There's a tiny clock up in the corner — I only notice it now that you ask — a clock might mean 'history', like the back-in-time thing, but it's a speck, I'd never have found it on my own. So: hopeful, not certain. Half marks to the program."

---

**SCREENSHOT 2 — the same program with a panel open at the bottom**

*What is this?*

"Same story, shorter now — hold on, no, the top says 4,188 words, it's *longer* than before, just less of it on screen. The little clock in the corner is pressed in now, it has a ring around it. So someone clicked the clock and this black panel rose up from the bottom.

The panel says 'History' — good, a word, thank you — and then a lozenge that says 'Today · 4,188 words'. And underneath... my first honest reaction? A hospital monitor. Or one of those charts on the news when the market falls. There's a pale line that starts high on the left and slides down, down, down across the whole width — and dates along the bottom, Mon 30 Jun, Thu 3 Jul, Sat 5 Jul, all the way to 'Yesterday'. My stomach actually tightened: a line going *down* over two weeks of dates reads as *losing something*. If that line is my book, my book is dying. I sat with it a moment and thought — no, wait, it says 'Started' at the top left and 'Draft complete' further along, and 'Submitted' at the far right with a little white dot, like the end of a film on YouTube. So it's a timeline of the writing, beginning to now. The dot at the right is *now*. Like the red dot on a YouTube bar. I got there. But I got there by reasoning, not by seeing — and I still don't know why the line droops. If it means the book got longer, it's drawn upside down, and I'd like a word with whoever did that.

There are also little amber specks scattered about — dust? fireflies? — and some blue vertical stripes, and a dashed line over one stretch. I haven't the faintest idea what any of those are. 'Restored' is written up there too, and 'Submitted' in bold. Submitted to whom? And a greyed-out button on the right that says 'Now'."

*What would I click first?*

"Somewhere in the middle of that timeline. On YouTube you click the bar and the film jumps there — the dates along the bottom are begging for it. That's the one thing on this panel that looks operable. The 'Now' button is grey, which I read as 'you're already at now', the way the volume is grey when it's at full."

*What worries me?*

"That I'll click into the past and break the present. On YouTube nothing breaks when you scrub — but this isn't a film, it's my manuscript, and manuscripts break. Also the date labels are small and dim grey on black; Sat versus Sun, I have to lean in. The specks and stripes I'd simply ignore, the way I ignore the dashboard lights I don't understand — which is not a compliment to the dashboard."

---

**T2 — "You want to see what your story looked like last Saturday. What do you do?"**

"That one I can actually do, and it's the first time today I'm sure of myself. The bottom of the panel says Sat 5 Jul, plain as a bus timetable. I click on the timeline directly above 'Sat 5 Jul'. I expect the page above to turn back into Saturday's version, the way clicking a YouTube bar jumps the film. If clicking doesn't work I'd try dragging that white dot leftwards to Saturday, the way you drag the playhead. Two tries, both borrowed straight from YouTube. If neither works, I give up and feel cheated, because everything about this panel *promised* me it works like that."

---

**SCREENSHOT 3 — after the researcher moved something**

*What is this?*

"Ah — see, it did exactly what I predicted, and I'm rather smug about it. The white dot has been dragged back to a line standing at 'Thu 10 Jul'. The lozenge now reads 'Thu 10 Jul · 4,094 words' instead of 'Today', and — look at the page! The page above has changed. The customs officer's charts and dates are gone from the text; the top corner says 4,094 words now. The whole document has travelled back to Thursday. That is genuinely impressive and slightly eerie, like the room rearranging itself behind you.

Two new things: a button that says 'Restore' has appeared next to the date lozenge, and the 'Now' button on the right has lit up bright. I understand both without being told, and I want that noted: *Restore* means 'keep this old version', and *Now* means 'take me back to today'. The pair of them appearing together is what makes it safe — a door back. If 'Now' hadn't lit up, I'd be panicking right now that I'd already destroyed today's text just by looking at Thursday."

*What would I never touch?*

"Still the specks and the blue stripes. And I notice 'Restored' was already written on the timeline before I did anything — someone restored once before, and the story evidently survived, because the line carries on past it. That little detail did more to reassure me than any button."

---

**T3 — "You looked at Thursday's version and want to KEEP it — make it today's text. What do you do? Are you afraid anything will be lost?"**

"I click 'Restore'. It's sitting right beside the date I'm looking at; there's nothing else it could mean.

Am I afraid? A little, and I'll tell you exactly of what: today's 4,188 words. Restore *from* Thursday — but restore *over* what? In my head there are two possibilities: the program lays Thursday's text on top of today like a fresh page, and today slides back into this history ribbon where I can always fetch it — or it *replaces* today, full stop, gone. The panel itself argues for the kind one: this strip claims to be the *whole* history, 'Started' to 'Submitted', every day of it, and there's already a 'Restored' marker from before with the line marching on afterwards — so restoring didn't end the world last time. That's real evidence and I did find it myself. But it's evidence a *literature teacher* pieces together from clues, like Miss Marple. The button itself promises nothing. I'd click it — but I'd click it the way you sign something a solicitor slides at you: fairly sure, with a held breath. One line of print — 'today stays in your history' — and I'd have clicked it whistling."

---

## RESEARCHER'S SUMMARY (out of character)

**Media-player floor: a pass, but a scraped one, and the pass is entirely borrowed capital.** The subject operated the strip without any instruction: she found the clock-press state, read the readout pill, located Saturday from the axis labels, predicted click-to-jump and drag-the-thumb before seeing screenshot 3, correctly decoded the Restore/Now pair on sight, and — critically — noticed the document itself had time-travelled (word count + missing paragraph). All three tasks completed. But every success routed through the YouTube seek-bar schema, and everything *outside* that schema failed: the y-flipped envelope was read as a two-week decline ("my book is dying") and produced a genuine startle at first contact; the amber word-quants and blue pass-bands were dismissed as dashboard noise ("dust, fireflies"); "Submitted" raised an unanswered "to whom?". On the editor screen, the clock icon failed as a history entry point (found only when prompted — T1 was answered via the Graveyard, and she correctly flagged that she cannot tell whether a plain Delete lands there). "Ask the editor" was refused outright pending an explanation of who answers, and the m/a/x/e shortcut hints read as "debris." Restore semantics were inferred, not perceived: she assembled non-destructiveness from circumstantial evidence (the prior "Restored" station with the line continuing past it — a design detail that earned its keep) but the button itself gave no guarantee, producing sign-with-held-breath anxiety.

**Single highest-leverage change: make the envelope read as a hanging page, not a falling line.** The intended metaphor (text hangs from the rail; envelope depth = length) never landed — a lone descending stroke over a dated axis is culturally hardwired as *loss*, and it poisoned her first three seconds with the panel, the exact window first-contact comprehension lives in. Fill the region between rail and envelope with a solid paper-cream sheet so it reads as a page growing downward rather than a vital sign crashing; that one rendering change converts the most alarming element into the most self-explanatory one. Runner-up, cheaper and nearly as valuable for this subject: one line of microcopy at park time ("Today stays on the strip") next to Restore — it would convert T3 from Miss-Marple inference to perception. Secondary notes for the backlog: low-contrast small type (COMPOST header, axis date labels) is at or below this demographic's legibility floor; the clock icon needs either a label or the Graveyard strip needs to cross-advertise history; and "Ask the editor" needs to disclose its answerer before trust-sensitive users will touch it.
