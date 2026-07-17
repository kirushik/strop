# Backlog: the system voice (popups, setup, and the paid option)

*(Parked 2026-07-16 during the sharp-corners round, by Kirill's call:
"put it somewhere in backlog, let's keep it out of our current pass."
This file is the resurrection context — enough to restart the round
without re-deriving it.)*

## The wound

Field note, 2026-07-16: "Popups on the right still popupping. Probably
a big one, and requires an addition of a new fundamental UX primitive,
which doesn't interfere with neither text nor the sidebar comments."
Transient system messages currently surface as popups near the right
edge — colliding with the margin conversation (notes, cards) and
violating the calm the rest of the chrome earns.

## Why it's a round, not a fix

Strop has no designed channel for the software's *own* voice — the
things that are neither the writer's text nor the editor's reads:
setup guidance (API keys, model selection), failures that need a
decision, update notices, and — deliberately — the paid option. Each
of these currently improvises its surface. A primitive is needed, and
it must be designed against the constitution, because every easy
answer violates it:

- **P1**: never on the prose, never wearing the writer's words.
- **P2**: the tool never wants anything — no software-initiated
  interruptions, no nagging badges; the voice must have a calm,
  findable resting place (the drill notch), with discovery in use.
- **P4**: no explaining; strings are data or actionable labels.
- **P11**: it may not become a second anchor competing with the text.
- **Chirality (asides.md §4)**: left = writer's workshop, right = the
  conversation about the text. The system's voice is *neither* — it
  needs its own honest place, not a squat in the margin lane.
- **Color**: cool/machine family by law; red only for errors.

## The product intent (Kirill, verbatim in spirit)

The same primitive carries the commercial surface: "We can also push
the editor through our setup (and even upsell our own subscription
service down the road) using it — so it should be both *noticeable*
and *unintrusive*, respectful, not losing trust. Good product with a
paid option to save you effort of dealing with API keys and model
selection, not a salesman's trophy hunt. And definitely no forced
'click here, and now here, and now here' onboarding." (Forced tours
are already constitutionally dead under P2 — that part needs no
design, only a refusal.)

## Adjacent decisions already made (2026-07-16, do not relitigate)

- **No chat surface, ever** — arbitrary editor questions are
  caret/selection-anchored, answered as one margin card, no thread.
- The stale-read error card stops lying ("draft changed" verdict is
  being fixed to honest grounding language) — error-card tone is
  therefore already part of the voice's register.
- The quit-save veil dies except on genuine failure; the failure
  dialog (Try again / Save a copy / Quit anyway) is the current best
  specimen of the system speaking when it has earned the right.

## Open questions for the round

1. Inventory: which popups/toasts/dialogs exist today, each one's
   trigger, and which are even legitimate (some should simply die).
2. Where does the voice rest? (Footer? The omnibar's flank? A place
   that is neither warm-left nor conversation-right.)
3. The commerce notch: how does the paid option appear exactly once,
   at the moment of genuine relevance (e.g., the writer reaching for
   API-key setup), and never again unbidden?
4. Relationship between the voice's registers: failure that needs a
   decision (may interrupt) vs. information (must rest) vs. offer
   (must wait to be found).
5. Does the voice ever animate? (attention-motion.md governs; the
   reveal-clock law was written for machine deliveries.)

## Also parked here: the library and the christening (2026-07-16)

Deferred from the same round: a "promote into the library" verb for
documents born outside ~/Documents/Strop (imports now birth next to
their source .md, lazily — no .strop until first edit). The verb was
judged premature, but the intention is real (the library should
accumulate the writer's works) and the seam was identified: **the
christening** — the moment "Untitled" gets renamed into a real title
is the writer declaring "this is a work now," and a "…and keep it in
the library" choice belongs inside that act, not in a separate verb.

Kirill's addition at parking time: under a cloud subscription,
promotion carries real weight — the library is what gets seamlessly
synchronized / backed up / notarized. So the christening is a
first-class candidate for question 3 above (the commerce notch at a
moment of genuine relevance): the writer names a work; the tool's one
lawful sentence about protecting it lives there, once.

Prerequisites shipped in the sharp-corners pass (not deferred):
lazy birth on .md open; recents saying "moved or missing" instead of
silently hiding files.

## Method hint

The compost fresh-slate tournament (impl/08) worked: blind designers,
adversarial panels, judges, an explicit shipping-nothing champion.
This round has the same shape — and "shelve the upsell surface
entirely, keep only the failure register" deserves its own champion.

## Case file addition (2026-07-17): the refusal register

The clipboard round exposed the missing register concretely: pasting
a readable-but-not-an-image file now imports nothing and can only
say so on stderr — the strip pulse exists but its banner renders
only while PARKED, the cold-read pulse only in the reading room, the
chip pulse means compost arrival. Every refusal surface we have is
surface-LOCAL; the editing desk itself has no lawful way to decline.
Whatever grammar this round designs must cover the quiet "no" (a
paste, a drop, a command that cannot apply) before it covers any
upsell speech. Until then: log-only is the accepted, recorded state
of paste refusals — a deliberate gap, not an oversight.
