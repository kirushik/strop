# impl/14 — The door made visible (the chip)

*Status: ADJUDICATED 2026-07-16 — draft attacked by a four-lens
adversarial panel (Raskin / Norman / Birman / the working writer);
this is the synthesis. Panel verdicts live in the session record;
conflicts were resolved by argument, noted inline. Amends golden-path
door mechanics and the ux-glossary "door" row; margin-card-dynamics.md
governs all motion; attention-motion.md governs all timing.*

## 0. What was decided before the panel (unchanged)

Mode nouns abolished — no "Reading"/"Away" strings anywhere, in any
language. Glance without stance. No auto-flipping in either direction,
ever; running a pass still opens (asking is a stance). Auto-detecting
"the writer is writing" is a named non-feature. The state is carried
by the artifact.

## 1. The chip

One object, the door made visible. **Viewport-fixed at the margin
lane's top edge** (panel-unanimous: a control used blind must have one
position; riding the packer makes it a slot machine, and a
content-anchored chip turns ctrl-shift-r into a blind toggle over
off-screen state — the panel's first FATAL). It exists only while
diagnosis cards exist anywhere in the document; no referent → no
chrome; the empty slot is a dead zone (clicks there are no-ops, never
caret placement).

**Face: the squiggle mark + the count. No words, in any language.**
(Birman's resolution, writer-confirmed: the anchor's own mark is the
only honest icon — its referent is visibly in the prose, satisfying
the glossary's standing rule by construction; a comment-bubble glyph
would promise conversation; every count+word candidate dies in
Russian declension. The count answers "how many"; the mark answers
"of what"; nothing is left that isn't data — P4 satisfied by
emptiness.)

- Content-width pill, right-set in the lane (headers are full-width;
  markers are compact — the form, not the position, prevents the
  header misread). Box fixed to the widest label at the current count.
- ~46×18px for "⌇ 3": squiggle ~12×5px + numeral, PT Sans 10.5px,
  `MUTED_COLOR` at rest, on `DIAGNOSIS_CARD_BG` wash, hairline
  `RULE_COLOR`, full-radius. The chip is a card reduced to its count —
  wearing the cards' own wash is what files it with the machine
  family in the mixed margin (P10 first speaker; widget-vs-bare-text
  is the colorblind channel).
- **Contrast ceiling: never above a resting card** (P11; a chip
  "improved" for discovery becomes a badge, P2).
- **Toggle contract worn as form (P7/P8):** released face at rest;
  pressed-in face while cards are out (the lit-toggle grammar the
  titlebar already ships). Docs furniture teaches only the expand
  half; the pressed face is the signifier that un-pressing exists
  (Norman's FATAL). Same box, same spot, same count in both states.
- Hover: ink brightens to `TEXT_COLOR`, pointing hand; tooltip is the
  verb + shortcut — EN "Show the editor's notes" / "Put the editor's
  notes away" (ctrl-shift-r). RU strings go through the glossary gate
  with the §6 banned list; «пометки редактора» is the sanctioned
  noun («заметки» stays the writer's word). The internal word "door"
  never prints (a shipped tooltip currently leaks it — dies here).

**Count semantics (truth conditions chosen so the chip never lies):**
the count is the document-global number of **non-stale** diagnosis
cards. It is invariant across door state and glances (a glanced card
still exists). Stale cards — drained `unverified` ghosts of sentences
the writer already fixed — do not keep the badge warm (the writer's
P2 kill: "janitorial work on the tool's own stale opinions"); when
everything resting is stale, the chip's ink drains to the stale
grammar instead of counting. Count changes get the 120ms cross-fade +
one-frame luminance tick (attention-motion §2); never an odometer.

## 2. The chip's verbs

**Open (press at rest):** cards fan out in the shipped reveal
grammar. **If no anchor is in the viewport, the press must still
visibly act** (Norman's FATAL: an action with no visible consequence
teaches "broken"): borrow the shipped reveal_scroll — the nearest
anchor scrolls to the near edge. "The click that points gets taken
there." Opening flushes a parked pass (reveal-clock law).

**Close (press while out):** universal commit-on-blur runs first —
a machine-card composer with focus commits its text before its card
ghosts (P13 tolerates no data-loss TBD); then all cards recede (150ms
exit ghost). **Closing never flushes a parked pass** — the shipped
"every door touch flushes" law is amended to open-only; an invisible
flush riding a close is a hidden side effect (Raskin).

**Press feedback ≤100ms on every flip** (timing table); the chip's
face change is discrete, never gradual (change blindness).

**Esc rests the cards.** New rung in the escape ladder: a glanced
card recedes first; a second Esc (or Esc with no glance out) rests
the whole fan. Below the palette/strip/history rungs, above
collapse-selection. Esc never moves the caret. (The writer's
minute-one report: Esc is the first thing every hand tries, and
today it does nothing — while the ✕ every popup taught her
permanently poisons re-flagging. The safe gesture must be the
taught one.)

## 3. The glance

Squiggle-click selects that card and shows it **as transient overlay
chrome, not a lane citizen** (Raskin): it paints on the ghost layer
at its anchor's height, displaces nothing, re-packs nothing — caret
clicks keep exactly one persistent meaning, and fixing a comma inside
a flagged span never shakes the margin. It self-terminates: caret
leaving the span, click elsewhere, or Esc — but **typing inside the
span keeps it** (the glance's purpose is fixing the flagged sentence;
the card is the worksheet — writer's scenario 3 depends on this).
The chip does not react to glances (its count is glance-invariant by
construction). A glance may transiently occlude a writer-note card
packed at the same anchor — lawful: the glance is a self-terminating
ghost the writer summoned, and it displaces nothing (adjudicated at
the fix wave over a paint workaround; the overlap ends with the
glance).

## 4. The Away landing — the answer waits at the door

A pass completing while cards rest **parks behind the chip** — it
does not integrate. No new squiggles appear in the prose, no silent
count jump, no drawer-filing without a sound (the writer's scenario
4: ink appearing in the draft while "away" is exactly the false
presence this spec exists to kill). The chip takes one
announce-once beat (its own, after any in-progress motion settles)
and wears a **ready quality** — the parked-read face; the Ask
button's "a read is ready" face finally has a true referent. Opening
the chip lands everything in the reveal grammar. If no chip existed
(first pass ever), it is born with the ready face.

## 5. The Ask button

- In flight: **onset transient (150ms fade-in, decelerate) → static
  cool 6px dot → completion pip (150ms in / 400ms out, once).**
  The breathing pulse the field note wished for is **overruled by
  attention-motion.md:80** — loop/breathe/idle motion is banned
  outright (Bartram; WCAG 2.2.2); the shipped static dot was the
  only lawful design all along, and the code comments claiming a
  pulse are fixed by this spec, not by an animation. reduce_motion:
  pip degrades to a static tick per the doc.
- **No transient "Reading…" string** (3-of-4 panel, three independent
  arguments: no visible referent; the dot+color already speak — one
  speaker, P10; a vanishing label converts a glanceable control into
  a readable one forever). The presence pair is dead everywhere.
- The menu: **verbs only.** The read-request carrier sentences, a
  **Cancel row while a read cooks** (Norman: the in-flight ask
  currently has no inverse anywhere in the product — P13), the
  conditional provider-setup row. The statistics leave; the door row
  is gone. A menu with numbers in it is a dashboard.
- In the reading room the button stays present-but-dimmed (opacity
  ~0.55, inert — the treatment history already gets). The old O10
  hide-rule existed to prevent two "Reading"s co-occurring; the
  abolition kills its reason, and a doorknob that moves between
  entry and exit fails P6/P7 (Birman).

## 6. Naming law (banned strings — Sol does not improvise here)

"resting"/"open" as state words (presence register in participle
form; "open" additionally collides verb-vs-count-noun with the old
Ask face); bare "notes"/«заметки» (the writer's word); «отложены»
(Set aside), «ждут» (pressure), «убрать» (destructive smell),
«свернуты»/"collapsed" (collides with receded-in-place), «скрыты»/
"hidden" (system taxonomy that frightens), "3 reads" (glossary-
executed), "door"/"session" (internal register). The chip carries no
word; tooltips carry sanctioned verbs only.

## 7. Titlebar

**History moves left, to the omnibar's right flank** (2-of-3 panel;
Norman's argument decides: the misclick is a description-similarity
slip — "small muted glyph in the top-right cluster" — and only
changing the *neighborhood* changes the description; also
navigation-by-name beside navigation-by-time is a natural mapping).
Birman's counterarguments are answered in the layout: the palette
menu-glyph (≡) and the history clock are dissimilar forms (the
titlebar twins were two *pictorial* siblings), and every icon target
grows to **≥24×24px hit area (28 preferred), ≥8px between targets**
(WCAG 2.5.8; today's 25×17/2px fails it) — glyphs stay 13px; the
hitbox grows, not the ink. The book keeps the Ask button as its left
moat and the drag moat right (moat law upheld). Both view toggles
wear visible latch faces (P12), and the two view states are mutually
exclusive: entering one visibly unlatches the other (screenshot test
— any frame shows exactly which view-bit is on).

## 8. What dies with this spec

The menu-footer presence pair; the dropdown door row; every
"Reading"/"Away" string EN/RU; the anchor-click door flip
(editor.rs:11571 behavior — click now selects/glances only); the
"arrival teaches" rationale (the chip must pass the corridor cold,
in both states, no arrival witnessed — pedagogy is a bonus, never
the design); O10's hide-the-button rule; the door-word tooltip leak.

## 9. Named follow-ups (logged, not silently shipped)

1. **Dismissal permanence** — the card ✕ permanently suppresses
   re-flagging with no inverse and no visible warning of permanence;
   Esc-primacy and the chip blunt the minute-one sweep, but the verb
   itself still destroys silently (P13 debt, predates this spec).
2. **"done" vs ✕** — two adjacent exits with invisibly different
   meanings (the writer flips a coin every time). One round, both.
3. Where the evicted menu statistics live, if anywhere (palette?).
4. RU tooltip strings through the glossary gate against §6.

## 10. The spec's laws (panel sentences, adopted verbatim)

- *No gesture may change the door's state unless its indicator is
  visible at a fixed position in the viewport at the instant of
  actuation, and no gesture that reads a card may move, re-pack, or
  leave persistent state in the lane.* (Raskin)
- *Every press of the chip must change something the writer can see
  in the current viewport within 100ms — never a state she has to
  scroll, hover, or remember to confirm.* (Norman)
- *The chip is a card reduced to its count: it wears the cards' own
  wash and the anchor's own mark, and no word rides it in any
  language.* (Birman)
