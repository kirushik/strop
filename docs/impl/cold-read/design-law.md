# The cold read — design law (settled record, 2026-07-06)

*(Compiled after golden-path round 4 marked the cold read "settled"
(golden-path.md §9.3, line 705). This file is the binding-decision
register for the build spec: every settled decision with its citation,
every superseded statement with its winner, every question the rounds
left open, the P1–P13 constraints, and the interplay rules other specs
already fixed. Resolution rule used throughout: the latest review
round wins; the lab v4 mockup scene 1 is the approved design
(`docs/impl/acceptance.md` §2 — "a deliberate divergence is allowed; a
*silent* one is the failure mode"). Source docs: golden-path.md,
impl/05-cold-read.md, impl/review-ledger.md, ux-glossary.md,
mockups/ux-lab-2026-07.html, design-principles.md, attention-motion.md,
color-language.md, impl/08-compost-fresh.md +
impl/compost-fresh/adjudications.md, history-strip.md +
impl/01-history-strip.md, DESIGN.md, writing-editing-checkpointing.md,
asides.md.)*

---

## §1 · The settled law

### A. What it is, and how it is entered

**L1. Strop owns the estrangement ritual.** One writer-invoked act
enters a reading presentation: re-typeset (different face, measure,
page shape — prediction-breaking), caret-less; reactions become
marginalia; Escape leaves. It is a session-scale, writer-invoked lock,
not a phase the tool infers. *(golden-path.md D3, lines 268–280;
phase table P2 row, line 227: "let the caret tempt you" and "verdict
the draft" are the two NEVERs.)*

**L2. The form: static, numbered, flippable pages.** "Static
*numbered* flippable pages, different face and measure, possibly the
subtlest paper texture" — endorsed round 1 *(golden-path.md §9 D3,
lines 407–410)*; "page numbers + flip-zone shading: endorsed" round 2
*(§9.1, line 488)*; settled round 4 with "no further feedback; face,
justification, texture, banner as in v3" *(§9.3, lines 705–706)*.

**L3. Entry quietly checkpoints first.** "Entering a cold read quietly
checkpoints first (unnamed, fingerprint-guarded — every cold read
starts from a recorded state, which also gives the churn mirror its
reference point)" *(golden-path.md §9, lines 412–414)*. Impl form:
"Entry auto-creates the reflex checkpoint" *(05-cold-read.md §1)*.
Reflex checkpoints render on the strip as unnamed bare ticks, lowest
rank *(history-strip.md §2)*; fingerprint-guarded means no change →
no checkpoint *(golden-path.md §9 D9, lines 456–459)*.

**L4. The SEAL is a separate, explicit writer act.** "Declaring a
draft complete and wanting to read are different things"
*(golden-path.md §9, lines 414–416)*. No UI string ever says "seal"
*(ux-glossary.md, "seal" row: internal register)*.

**L5. Entry affordances: the palette verb + the history-preview
path.** Named as such in review *(review-ledger.md H17, line ~109)*.
There is **no invitation, banner offer, or prompt anywhere** — the
"Read it cold?" banner text died with re-entry (see §2-S6). Entry
rests where a hand falls (P2/P5); the exact palette string is open
(§3-O7).

**L6. The same paged viewer serves as the history checkpoint
preview.** "'Read this version' — in-window takeover, not a popup:
window management is clutter, and the read-only takeover pattern
already exists" *(golden-path.md §9, lines 410–412)*; "The
history-preview variant (paged view of a checkpoint state + Restore
chip) rides the same renderer" *(05-cold-read.md §1)*.

### B. The page (metrics are the approved design — lab v4 scene 1)

All values from `docs/mockups/ux-lab-2026-07.html` (scene 1 CSS,
lines 60–83; the mockup-fidelity gate makes these the reference:
acceptance.md §2 row "Cold read | 1 | page, banner strings, reaction
input, Esc returns").

**L7. Page geometry.** One page at a time, centered on a cool-grey
desk (`.cr-stage` background `#E4E6E9`). Page: 520px wide,
min-height 614px, paper `#FEFEFC` (distinct from the editor's
`#FBFAF8`), padding **52px top / 58px sides / 48px bottom**, soft
double shadow (`0 2px 4px` + `0 12px 30px` rgba(26,30,38,…)).

**L8. Type.** Body **16.5px / 1.66**. The face is **bookish**
(Bookman-class): font stack as fixed in the lab —
`'URW Bookman','Bookman Old Style','Bookman URW',Bookman,serif`.
("The BOOKISH face (Bookman-class) wins" — golden-path.md §9.2,
line 593.) The lab's other stacks (mild =
`'P052','Palatino Linotype',Palatino,serif` at 16.5px; typescript =
`'Courier Prime','Nimbus Mono PS','Courier New',monospace` at 15px)
are lab-ground taste-test variants, not product UI (§3-O3).

**L9. Justified + real hyphenation.** "If the page imitates a book,
it hyphenates like one — the cold read goes **justified + real
hyphenation** (we own the layout engine; Knuth-Liang pattern crates
exist in Rust; ragged-right was the mock's browser-limitation
compromise, not a design)" *(golden-path.md §9.2, lines 594–598)*.
The engine decision is settled in the glossary appendix: **the
`hyphenation` crate (80 language variants, Russian first-class via
Lebedev's ruhyphen patterns), with runtime dictionaries** for the
LPPL/GPL posture; CJK/Thai/Arabic are different line-breaking
problems, not gaps *(ux-glossary.md appendix, lines 51–81;
golden-path.md §9.3, lines 707–710)*. Build shape per the recon:
in-word break opportunities from `hyphenation` v0.8.4 (verify
`hyphenation_commons`' declared license before merge; load the ru
dictionary at runtime); per-paragraph greedy line-breaker over
`shape_line().width()` word measures, slack distributed across gaps,
each word painted as its own `ShapedLine`; Knuth–Plass later only "if
rivers offend"; a new layout cache layer *(05-cold-read.md §0)*.
The mock's CSS analogue records the hyphenation-limit intent:
`hyphenate-limit-chars: 6 3 3`.

**L10. Real pagination.** "Mock page breaks read as arbitrary because
they were — real pagination reflows" *(golden-path.md §9.1,
lines 494–495)*. Page breaks must be computed by the real layout;
the folio count must be true.

**L11. Paper texture: irregular noise.** "Texture concept liked,
execution rejected: the dot-grid's REGULARITY is the problem — v2
uses irregular noise (feTurbulence-class)" *(§9.1, lines 496–498)*;
accepted §9.2 line 598, settled §9.3. The approved reference:
an feTurbulence `fractalNoise` overlay, `baseFrequency 0.75`,
2 octaves, alpha ≈ 0.05, tiled 140px, non-interactive
(`pointer-events:none`) *(lab CSS line 68)*. GPUI execution is open
(§3-O4).

**L12. Running head + folio.** Top right, italic, small, muted:
`The Crossing — draft` (the document's name; see §3-O11 on the
"— draft" suffix). Bottom center folio: `— 2 of 9 —` (em-dash-wrapped
"N of M", muted, 12px serif) *(lab lines 70–71, 170–173, 384)*.

**L13. Flip zones.** Left/right **26%-width** full-height click
zones; hover shades with a subtle edge gradient
(`rgba(26,30,38,.035)` → transparent, inward); tooltips "previous
page" / "next page" *(lab lines 72–75, 174–175)*. Keyboard:
**ArrowRight / Space / PageDown** flip forward; **ArrowLeft / PageUp**
flip back *(lab JS lines 415–421)*.

### C. The banner (strings are LAW — purged to data in the settled round)

**L14. Reading-mode banner, exact grammar:**

> **Reading** · "Draft complete" · 4,120 words &nbsp;·&nbsp; Esc returns

— where **Reading** is the bold pulse target (`.pulseme`), the station
name is the writer's own words in typographic quotes, the count is the
manuscript word count, the separator before "Esc returns" is dimmed
(opacity .6), and nothing explains anything *(lab JS lines 362–364;
lab notes: "Strings purged (P4): the banner explains nothing now — it
states the mode, the draft's name, the count")*. The former
"sealed draft — day 8" became `"Draft complete" · Sun 6 Jul` — "the
writer's own station name + a real date (no 'seal', no numbered
days)" *(lab notes, scene 1)*.

**L15. History-preview banner, exact grammar:**

> **"Draft complete"** · Sun 6 Jul · 4,120 words &nbsp;[Restore]&nbsp; · Esc returns

— the checkpoint's NAME (+ real date) leads in bold; the Restore
control is a plain filled chip labeled **Restore** (one word — the
strip's verb, P8/P13); no safety lecture ("the strip is where safety
is *visible*" — lab notes). Emphasis flip decided in round 2: "the
checkpoint's NAME (+ date) is the payload, 'viewing a checkpoint' is
chrome and goes muted" *(golden-path.md §9.1, lines 503–505)*.

**L16. Titlebar mode label.** Document name, then muted:
`— reading` (reading mode) / `— viewing "Draft complete"` (history
variant) *(lab line 167, JS line 365)*. In the cold-read titlebar the
word count and the editor button are absent (the count lives in the
banner; the mock's tbar carries only ⌕, the history clock, ≡).

**L17. Banner colors.** The banner wears the drained wash
(`--stale` `#EFEEEA`) with muted ink; bold segments in full text ink;
below it a hairline rule *(lab CSS lines 61–65)*.

**L18. Typing pulses the banner — no toast, ever.** "Typing-attempt
feedback: no toast — PULSE THE EXISTING BANNER phrase instead (no
overlapping surfaces, ever)" *(golden-path.md §9.1, lines 498–499)*.
In v4 the pulse target is the word **Reading** itself *(lab notes;
JS `crPulse()`: warm selection-tint background `--seltint`
rgba(200,169,81,.33), .18s ramp, held ~900ms, once per attempt)*.
This is also the mode-matrix gate's "one pulse idiom — never a silent
swallow" *(acceptance.md §3)*.

### D. Reactions

**L19. Reactions are marginalia, never edits.** "The caret is gone;
reactions land as margin notes, not edits — … — which are exactly the
`ctrl-m` marginalia the revision will be built from"
*(golden-path.md Day 9, lines 121–133)*. They file as **margin notes
anchored by content** *(05-cold-read.md §1)*.

**L20. The reaction input.** Raised from a text selection on the
page. Contents, exactly: three quick-mark chips —
`? doubt` · `! alive` · `~ drags` — above a free-text line with
placeholder **"…or a few words"** ("the reaction input lost its
footnote; the placeholder carries it" — lab notes). Amber-bordered
(`--active`) floating card, ~250px, appearing under the selection;
Enter commits the typed text; Esc closes the input (not the mode —
two-level Esc, §5-I3) *(lab lines 79–83, 180, JS 391–414)*.

**L21. The reaction note in the lane.** Warm note card
(`--note` `#FAF4E2`, the writer's material — color-language.md), 13px
serif, led by the quoted anchor fragment as muted single-line data
(ellipsized, ~42-char capture), then glyph + text:
`"the ferry had made the crossing…" ~ drags — cut the schedule talk?`
*(lab lines 77–78, 178, JS crKeep)*. Lane: right of the page, 230px,
top-aligned with the page's text top *(lab line 76)*.

**L22. Reactions live the margin's full life afterward.** "Your own
cold-read notes resolve alongside the editor's cards; done and
dismissed, they fade with the grace the margin already knows"
*(golden-path.md, lines 146–148)*.

### E. What is banned inside the read

**L23. No churn heat in the cold read.** "Churn heat is EVICTED from
the cold read: reading mode cultivates the reader's eyes, and 'you
edited this ×14' yanks the writer back into writer stance,
contaminating the estrangement. It moves to writer-stance surfaces: a
summonable editor LENS (find-match-style tint, palette-invoked, P4)
and/or history rows" *(golden-path.md §9.1, lines 500–504)*. This
supersedes round 1's "heat marks in the COLD READ margin" candidate
(§2-S5).

**L24. No caret, no verdicts, no AI first-move.** The caret is gone
(L19); the tool never verdicts the draft (phase table, line 227); the
believing pass is the read's AI *companion*, pull-only, asked by the
writer after she finishes *(Day 9, lines 129–132; D3 line 273;
DESIGN.md §4.4: "The AI must never be the first to speak")*. The
door's law holds: no AI cards materialize in the reading lane.

**L25. No overlapping surfaces** (L18's rule, stated once, general).

### F. Scope, motion, color

**L26. The read ends at the scrap line.** "Export, counts, AI passes,
and cold read stop at the seam; caret, typing, formatting, find, and
history don't" *(08-compost-fresh.md §2, "Scopes — the one-sentence
law")*. Mechanically: **cold read consumes `manuscript_slice`**
*(compost-fresh/adjudications.md, Scopes & search 4)*. The read ends
at the piece's true last line; seam, Scraps, graveyard, chips are
simply absent *(design-tail.md, the winning design)*.

**L27. The history-preview variant shows the past state's own
geometry.** "Preview/parked mode … draws the state's own seam
read-only" *(adjudications.md, Time & persistence 1, case 5)*; "find
inside a parked preview scopes to the past state's seam"
*(adjudications.md, Scopes & search 6)*.

**L28. Motion tokens** (attention-motion.md §2): enter =
`cubic-bezier(0,0,0.2,1)` 250ms; in-place move =
`cubic-bezier(0.2,0,0,1)` 200ms; exit = `cubic-bezier(0.4,0,1,1)`
150ms; hover/press feedback first frame <100ms; **no
spring/overshoot/bounce, no looping motion, no scale/spin,
anywhere**; single-pulse transients only (WCAG 2.3.1).
`reduce_motion` law: every translate/scale degrades to an
**equal-duration opacity cross-fade, never a teleport** — read from
the OS setting + the shipped `reduce_motion` config, "a first-class
supported mode, not a degraded one" *(attention-motion.md §4)*.
Every transition frame must pass the screenshot test (P6).

**L29. Color.** Reactions are WRITER material = warm
(`NOTE_CARD_BG #FAF4E2`); the banner wash is drained
(`STALE_BG`-family `#EFEEEA`); the pulse is the warm selection tint
(the writer acting); the page paper `#FEFEFC` and desk `#E4E6E9` are
new surface values (mock-approved; token names to be minted at
build). No cool blue appears unless the machine does
*(color-language.md, hue→meaning table + ordered-axis paragraph)*.

### G. Acceptance (already fixed)

**L30.** Corridor floor: "it looks like a book; page-flip by
click/arrow keys; Esc returns to the desk." Rig: `coldread:open`,
page-count and reaction-note assertions; wshot golden of a rendered
page *(05-cold-read.md §2)*. Plus the three standing gates: the
legacy-document gate (`seed:legacy`), the mockup-fidelity gate
(scene 1: "page, banner strings, reaction input, Esc returns"), and
the mode-matrix gate — cold read is named there explicitly:
always-visible indicator in the writer's field of view, one pulse
idiom for every blocked verb, exits named on screen, rig asserts the
enter/leave dump bits *(acceptance.md §§1–3)*.

---

## §2 · Superseded / contradictory statements — and which wins

- **S1. Ragged-right vs justified.** §9.1 (golden-path.md lines
  491–495): "the build deliberately went ragged-right after the
  Kindle-sin research … justified+hyphenated stays open for the GPUI
  implementation." **Superseded by §9.2** (lines 593–598): "the fake
  dies … justified + real hyphenation; ragged-right was the mock's
  browser-limitation compromise, not a design." Settled §9.3 with the
  crate decision. *Winner: justified + real hyphenation.*

- **S2. Typeface: graded-strength taste test vs bookish.** §9.1
  (lines 489–491) ordered "graded strengths (mild serif shift / slab /
  typewriter) to taste-test." **§9.2 closed it: "The BOOKISH face
  (Bookman-class) wins"** (line 593). The lab v4 face cycler
  (bookish/mild/typescript) is a lab-ground control kept for
  reference, not a product surface (the lab's own convention:
  "everything inside the window is product; everything on this dark
  ground is lab"). *Winner: bookish, sole v1 face.* (Residual
  question in §3-O3.)

- **S3. Texture: dot-grid vs irregular noise.** §9 round 1 endorsed
  "possibly the subtlest paper texture" (line 409); §9.1 rejected the
  v1 execution ("the dot-grid's REGULARITY is the problem", lines
  496–498). *Winner: irregular feTurbulence-class noise (accepted
  §9.2, settled §9.3).*

- **S4. History-preview banner emphasis.** The v1 mock led with
  "viewing a checkpoint"; §9.1 (lines 503–505) flipped it: the
  checkpoint's NAME (+ date) is the payload; "viewing …" is muted
  chrome. *Winner: name-led (L15).*

- **S5. Churn heat in the cold-read margin.** Round 1 D7
  (lines 438–444) listed "heat marks in the COLD READ margin only" as
  the *most promising* churn-mirror home. **§9.1 evicted it**
  (lines 500–504). *Winner: eviction; the mirror lives in
  writer-stance surfaces (editor lens and/or history rows — that
  "and/or" is still open, but outside this build).*

- **S6. The banner offer "Read it cold?".** Round 1 D8 (lines
  446–454) specced a post-seal banner: "The draft is resting — sealed
  yesterday, 4,120 words. Read it cold?" §9.1 rejected re-entry as
  mocked; §9.2 challenged the entity; **§9.3 shelved re-entry
  entirely on P2** (lines 694–703): "no re-entry feature at all."
  *Winner: no software-initiated invitation to the cold read exists
  anywhere. Entry is the palette verb + the history-preview path
  (review-ledger H17).* Menu *ordering* by recorded facts (D4) is
  still lawful — order, never a prompt.

- **S7. 05-cold-read.md's status vs the rounds.** The impl seed
  (2026-07-05) is consistent with rounds 1–4 and cites them, but it
  **predates the compost-fresh round (2026-07-06)** — its scope
  paragraph doesn't mention the scrap line. The seam scope rules
  (L26–L27) come from 08-compost-fresh.md + adjudications.md and
  bind this build. Similarly, plan.md/review-ledger record the
  package as DEFERRED from wave 1 — "the one package needing new
  layout machinery — spec'd, reviewed, not rushed" *(plan.md §0)*.

- **S8. "Cold read and export end at the soft boundary"**
  *(07-compost.md line 67)* — superseded wholesale by
  08-compost-fresh.md (the tail/Scraps design and its structural
  seam). *Winner: 08 + adjudications.*

- **S9. Reaction-input placement vs the flank rule.** §9.1's asides
  verdict (lines 520–522) rejected "the selection popover OVER text …
  on principle — selection actions move to a right-margin anchored
  menu at selection height." The settled scene 1 nevertheless floats
  the reaction input directly under the page selection. The mockup is
  the approved design and round 4 raised no objection (lab convention
  + "no further feedback"), and asides.md §4 legitimizes at-hand
  menus ("a menu is not a region") — but the tension is real and any
  build-time move of the input to lane height is a *deliberate
  divergence to be named* (acceptance.md §2). Flagged again as
  §3-O8.

---

## §3 · Open questions the rounds explicitly left

- **O1. Footnote placement on pages.** "Nontrivial (the classic
  pagination/float problem — its own research item inside G1)"
  *(golden-path.md §9.2, lines 598–600)*. The v1 fallback IS settled:
  "v1 pages render footnote refs, definitions stay off-page"
  *(05-cold-read.md §1)*. Open: the eventual on-page placement
  algorithm.

- **O2. Visible anchor links for margin reactions.** "Margin
  reactions need visible anchor links in the real implementation"
  *(§9.2, lines 600–601)*. Round 4 added nothing; undesigned. Related
  must-solves folded into spec 05 by the review ledger: the **raise
  gesture**, **two-level Esc**, and the **paged margin model**
  (B0/B1); **pass-during-read**, **paged margin geometry**, and
  **visual→rope anchoring under hyphenation offsets** (H16/H18/H19)
  *(review-ledger.md, blocking + "Cold read (deferred package)")*.

- **O3. Does shipped v1 offer a face choice?** What was said,
  exactly: §9.1 — "v2 offers graded strengths (mild serif shift /
  slab / typewriter) to taste-test"; §9.2 — "The BOOKISH face
  (Bookman-class) wins"; 05 §1 — "bookish face" (no alternative
  named); the lab keeps the cycle `bookish/mild/typescript` **on the
  lab bar** (a taste-test control, outside the product window). No
  round ever said "ship a face setting." Default reading: **no
  writer-facing face choice in v1** (also per DESIGN.md principle 9,
  "settings are apologies"); a future choice is unaddressed, not
  rejected.

- **O4. Texture execution in GPUI.** The concept (irregular noise,
  feTurbulence-class, ~5% alpha) is endorsed; how to produce it in
  the engine (procedural noise, pre-baked tile, resolution/DPI
  behavior) is unspecced.

- **O5. Read-aloud posture (G1's second use).** D3 names it ("Second
  use: read-aloud posture in P4 — TTS is the evidence-adjacent tool
  [C]", lines 276–277) and G1's gap row includes it (line 363), but
  05's scope omits it. **Not in this package**; no round decided when
  or whether it ships.

- **O6. The believing-pass moment after finishing.** The walkthrough
  narrates it as the writer's own act ("When you finish, you ask the
  editor to read it the same way: the believing pass" — Day 9, lines
  129–131), and D4 already makes believing lead the pass menu before
  a seal exists. **No end-of-read affordance was ever specced**, and
  P2 forbids the tool offering one. Open only in the narrow sense:
  does the last page / exit moment carry anything at all? Default
  under P2: nothing.

- **O7. The entry verb's exact string.** H17 fixes the affordances
  (palette verb + history-preview path), the glossary fixes the
  register ("cold read" is a *carried* term — sentence forms only;
  Russian translates the function, «свежим взглядом», never the
  metaphor), but no doc fixes the English palette string.

- **O8. Reaction-input geometry vs the flank rule** — see §2-S9.

- **O9. Page-flip motion.** The pages are "static flippable"; the
  mock flips instantly. No round fixed a flip animation. Bounds if
  one is added: the timing tokens (L28), the screenshot test (P6),
  reduce_motion cross-fade; instant is lawful.

- **O10. "Reading" collides across surfaces (P8 audit item).** The
  door's presence grammar puts **"Reading" / "Away"** on the editor
  button (the *editor* reading — ux-glossary.md "door" row;
  golden-path §9.3 Birman audit), while the cold-read banner's mode
  word is **"Reading"** (the *writer* reading). Two actors, one word,
  both in chrome. No round noticed; the build spec should arbitrate
  or deliberately accept it.

- **O11. The running head's "— draft" suffix.** The mock's running
  head is `The Crossing — draft`; nothing in the rounds derives the
  suffix (station kind? fixed word?). Small, but strings are law —
  needs one decision.

- **O12. Find inside the reading mode.** The preview variant's find
  is specced (scopes to the past state's seam — L27); whether find is
  reachable inside the *reading* mode at all is unstated (the tbar
  keeps ⌕).

- **O13. License verification.** "Verify `hyphenation_commons`'
  declared license before merge" *(05-cold-read.md §0)* — an action
  item, not a design question.

---

## §4 · Principles checklist (design-principles.md P1–P13 × this build)

- **P1 Text sovereign** — nothing draws ON the page's prose; the
  selection tint and the anchor-fragment quote in a reaction note are
  lawful (record/relocate as data — the Birman-audit "durable
  provenance" typography), never decoration; the reaction input must
  not read as chrome worn by her words (watch §3-O8).
- **P2 The tool never wants anything** — no invitation to read, no
  end-of-read prompt, no "did you enjoy it": entry and exit are verbs
  resting in place (S6, O6).
- **P3 Everything the writer owns is text** — reactions are ordinary
  margin notes (editable text, warm) the moment they land; the
  qmark glyph is part of the note's text, not a widget state.
- **P4 Show, don't explain** — the banner states facts only (mode ·
  name · count · exit); the input's placeholder carries the free-text
  option; zero captions, zero legend; carried term "a read"/"cold
  read" only inside action sentences, one per sentence.
- **P5 Corridor floor** — it looks like a book; a stranger flips
  pages and leaves with Esc, reading nothing first (05 §2 acceptance
  is literally this test). Depth (reactions, history variant) is
  notch-tier.
- **P6 Screenshot test** — every frame of page flip, banner pulse,
  input appearance, and note arrival must read as a true still.
- **P7 Widget contracts** — pages behave as pages (flip zones,
  PageUp/Down/Space); Restore is a real button doing the strip's
  Restore (honored-plus: appends, destroys nothing); the pulse
  enriches existing chrome, never spawns a surface.
- **P8 UI is grammar** — banner segments are data separated by dots,
  never a sentence; the station name is quoted by typography, never
  inlined into system prose (the template ban: "viewing the version
  after X" is unconstructible); one verb, one word: Restore; the
  qmark grammar `? doubt / ! alive / ~ drags` is a parallel closed
  set. Audit O10.
- **P9 Hover only expands** — flip-zone shading and tooltips expand a
  zone that exists and works without the mouse (keyboard flips);
  nothing is hover-gated.
- **P10 Color speaks once** — warm note = writer's reaction (no
  "(yours)" label); drained banner = a resting/record surface; the
  cool family stays out because the machine is not speaking; every
  color meaning also carried by form/position.
- **P11 One anchor object** — the page. The contrast budget is spent
  on paper-and-ink; banner, lane, and desk subordinate (the lab draws
  exactly this hierarchy).
- **P12 The control is the indicator** — the mode lives on the
  surfaces that operate it: the banner names the state and its exit
  ("Esc returns"), the titlebar label changes with the mode, Restore
  appears only where restoring is possible; no separate status
  display anywhere.
- **P13 Every verb has an inverse in the same grammar** — Esc
  inverts entry; Restore is one Restore from undone (the pre-restore
  now is another moment on the strip); a reaction note dies by the
  margin's own dismiss/fade grammar; the entry reflex checkpoint
  makes even "I read it" a recorded, returnable moment.

---

## §5 · Interplay register — rules other specs already fixed

- **I1. Scraps / scrap line.** Cold read is an audience surface: it
  ends at the seam (08 §2 one-sentence law); implementation consumes
  `manuscript_slice` (adjudications, Scopes 4 — the geometry-flip
  checklist names cold read explicitly). Scraps, the seam row, both
  chips, and the graveyard are absent from the pages; the read ends
  at the piece's true last line.
- **I2. Graveyard.** Below the seam → never rendered in the read.
  Reactions never interact with it; a reaction whose anchor is later
  cut follows the *margin-note* orphan rule, not the graveyard.
- **I3. Esc / layer discipline.** DESIGN.md §0.6: Esc always
  dismisses exactly the topmost layer. Cold read joins Esc's shipped
  contract-set ("leave a transient context — find, the parked past,
  cold read"). Inside the read: Esc closes the reaction input first,
  then returns to the desk (two-level Esc, B0/B1 must-solve). The
  banner names the exit ("Esc returns").
- **I4. Margin / cards.** Reactions are `ctrl-m`-class writer notes
  anchored by content; they resolve alongside the editor's cards and
  fade with the margin's shipped grace (150ms exit ghost,
  reduce_motion cross-fade). A reaction whose anchor is cut later
  appends to Scraps with its anchor fragment in the margin-note
  anchor typography; the words "unanchored/orphaned" are forbidden
  (asides.md §2.3, ux-glossary.md).
- **I5. The door / editor button.** The door is a session
  instrument; the cold read is a writer stance — neither implies the
  other. The door's law stands inside the read: cards rest behind
  the rail; no AI card enters the reading lane; no read-state ("5
  new") exists by design (golden-path §9.2, editor button). The
  believing pass is the read's pull-only companion (D3);
  `PassKind::Believing` exists (review-ledger H27). Whether a pass
  may be *requested from inside* the read is must-solve H16; the
  precedent leans no: the editor button already disables its rows
  while a history preview is up, "the pass must not diagnose a
  document the screen isn't showing" (H33).
- **I6. History strip.** Entry's reflex checkpoint = bare tick,
  lowest rank, unnamed (history-strip.md §2). The paged viewer is
  the strip's "read this version" surface (L6); its Restore routes
  through the existing restore path — restore-as-forward, appends,
  materializes a "Restored" checkpoint, no confirmation dialog
  anywhere (history-strip.md §3; impl/01 §2). While previewing the
  past, the margin lane and rail hide (H36); strip counts and find
  rebase against each state's own boundary (adjudications, Scopes 6).
  Readout grammar (never a sentence, never a station name inside it)
  binds any readout-like string the preview shows.
- **I7. Checkpoints.** Kinds per writing-editing-checkpointing.md
  (SessionStart | SessionEnd | IdleGap | PrePass | BeforeRestore |
  Named, + the seal/submitted project kinds and Exported); the
  cold-read entry checkpoint is the reflex/unnamed kind,
  fingerprint-guarded; "Draft complete" in the banner is the
  writer's own station name — the record's word, displayed as data.
  "Seal", "checkpoint", "pass", "door" never appear in UI strings;
  where a category noun is unavoidable the word is "version"
  (ux-glossary.md).
- **I8. Counts.** The banner's word count is manuscript-only
  accounting (Birman audit; the seam scope law) — the same number
  the count chip calls "piece".
- **I9. Passes & altitude.** The cold read is P2's instrument; its
  notes seed the descent (Day 9 → Days 9–12); the altitude order and
  D4's menu-order defaults are untouched by this build.
- **I10. Footnotes.** v1: refs render on the page (the
  painted-superscript machinery the recon cites), definitions stay
  off-page (05 §1); the bottom-zone footnote surface stays an editor
  concern.
- **I11. Localization.** "Cold read" ships behind function-first
  localization («свежим взглядом»); every carried term in this
  surface needs its glossary row before a new UI string lands
  (ux-glossary.md standing rules).
- **I12. Rig / gates.** `coldread:open` + page-count +
  reaction-note assertions; wshot golden of a page; `seed:legacy`
  exercise; mode-matrix dump bits (`margin_hidden` etc. restored on
  exit); mockup-fidelity check against lab scene 1 with divergences
  named (acceptance.md; 05 §2).
