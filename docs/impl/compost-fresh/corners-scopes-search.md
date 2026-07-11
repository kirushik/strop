# Corner cases — scopes and search (Scraps build)

Domain: the one-sentence law (08 §2: audience/machine surfaces end at the
seam; the writer's hands never do) applied to counts, export, AI passes,
find/replace, the omnibar, and the Esc grammar. Code read at
`golden-path-impl` @ e535933 — all anchors reflect the OLD geometry
(compost at top; the build flips it to the tail).

## 1 · The excursion latch: what clears it (decide)

The two kill-shots pull in opposite directions. Raskin's: a writer who
*scrolled* into her pile and edits must not be teleported by Esc. The
Novelist's: the retrieval dance is find → cut → Esc → paste — Esc must
still work *after editing* inside a latched excursion. If "any click into
the region clears the latch" (07 R3's wording), the retrieval dance dies:
the writer must click the found scrap to cut it. If nothing clears it,
the scroll-entry writer is mauled. The amended spec (08 §2) keys the
latch to *how the tail was entered*, not to clicks after entry — that
reading must be made canon explicitly.

**Proposed state machine.**
States: `Unlatched` | `Latched{home_caret, home_scroll}` — plus a
session-scoped `pile_end: Option<pos>` that outlives the latch.

Set (arms the latch, capturing home at THAT moment): Scraps-chip click;
ctrl-shift-o (today the rail toggle, editor.rs:10820 — retargeted to
chip-travel); any omnibar jump landing below the seam — a find preview
step (`omni_apply_match`, editor.rs:4227–4239 fires on every keystroke,
arrow, Enter-cycle) and an `@`-heading jump. Home must be latched at
travel time, not read live from `last_manuscript_caret`
(editor.rs:5200–5204), which keeps updating and today inits to 0.

Clear: (a) Esc-home — consumed, and the caret/scroll at that instant is
written to `pile_end`; (b) the caret entering the manuscript by the
writer's own act (click, arrow across the seam) — excursion over,
`pile_end` remembered; (c) a caret placed in the tail while *unlatched*
(scroll-and-click) never arms it — Esc inert.

Explicitly NOT clearing: clicks, selection, cuts, and typing inside the
tail while latched — the excursion continues (this is the whole
retrieval flow, and the chip-entered skimmer who edits one line still
gets her Esc home; she entered by travel, so travel-back is the
contract).

**Find's two exits differ and both must reconcile.** Esc in the find
field (`TextFieldEvent::Cancel`, editor.rs:4168–4179) already restores
`omni_return` and walks the caret home — that path must also clear the
latch, or it stays armed and a later scroll-entered caret teleports.
The other exit — the writer clicks the found match in the pile to edit
it, light-dismissing the bar — keeps the latch (entered by find this
excursion). Esc-Esc habituation then works: Esc₁ closes find (palette
check is first in `escape_mode`, editor.rs:6095), Esc₂ travels home;
after home the latch is dead, Esc₃ falls through to strip/nothing and
never bounces back into the pile. The current home-jump fires on raw
region membership (editor.rs:6126–6133) — it must become latch-gated.

`pile_end` details: chip's first press of a session lands at the seam;
subsequent presses resume `pile_end`, clamped after restores/edits
shrink the doc. Failure if unhandled: either the "Esc mauls my pile
edit" trust wound or a retrieval flow that restarts a 3,000-word pile
every round trip. Principles: 08 §2 (excursion latch, both ends), P13.

## 2 · Single Replace vs the bulk law (decide)

`replace_all` today rewrites the scrap box silently (editor.rs:
4886–4912; extraction papercut 5). Spec: manuscript-only, announced,
live-match-in-scraps told why (07 R5). But `replace_current`
(editor.rs:4852–4879) — Enter in the replace field while the find
preview has selected a match *in scraps* — is unnamed. Two defensible
answers: (a) single replace works everywhere — it mutates one visible
selection under the writer's eyes, indistinguishable from typing over
it (writer's hands); (b) any mutation issued from the machine's field
respects the seam and the row refuses. I propose (a): the law's clause
is about surfaces that *sweep*; a caret-precise, watched edit is the
hand. Maintainer adjudicates — (b) is the stricter grammar.

Build-time consequences either way: the All button wears its scope
(P12) — "All · 7 in the piece"; the refusal is the announcement: after
the sweep the status line (replacing the eprintln at editor.rs:4909)
says "replaced 7 in the piece · 2 in scraps untouched", and the scrap
matches stay in the row list, each row carrying the region (match rows
today show only a line snippet, editor.rs:4408–4419). Split-count
honesty: `find_matches` caps at 500 (editor.rs:4943) and rows at 100
(4411) — an announced split must degrade to "500+ in the piece", never
state an exact lie. Classification is by match start; `find_matches`
runs over the full rope (4914–4951), so a query can straddle the seam —
a straddling match may be *selected* (selections span) but the seam
never enters the replacement edit or the clipboard, and `find`'s
selection seeding (editor.rs:4206–4210) must not capture the boundary
line into the query. Failure if unhandled: replace-all silently
rewrites the writer's pile — the one boundary everything else respects,
pierced by the bulk verb (07 R5's exact wound).

## 3 · Retire-on-park vs `is_suppressed`: re-arm is currently impossible (decide)

The retire half already exists: set-aside collapses the card's anchor
and `reconcile_dead_anchors` marks it `Dismissed` + journals
`CardClosed` (editor.rs:5880–5917). But `is_suppressed`
(document.rs:552–560) treats Dismissed records as suppressors, and a
range collapsed to point *k* still overlaps any span strictly
containing *k* (`n.start < end && start < n.end` holds for k inside).
So: park a diagnosed passage, Move it back, run a pass — a same-titled
diagnosis spanning the old cut point is silently suppressed. Spec item
8 ("re-arms it for the next pass") never happens. The fix has one
direction: park-retirement must be its own terminal, excluded from
suppression — either delete the annotation record after journaling, or
a `Retired` status `is_suppressed` ignores.

The genuine decide: does a *writer's* dismissal survive the park→return
round trip? It cannot survive by range (apply_op collapses it on
departure), and the margin graft says machine artifacts never follow
writer text out of the machine's scope — so I propose dismissals die
with the park and the writer may be re-nagged once for a problem she
dismissed before parking. Accepted as rare and honest; the alternative
(dismissal records travelling as hidden freight on the block) violates
the graft. Also to pin: partial park — a selection covering half a
diagnosed span shrinks the annotation to the manuscript remnant
(ordinary edit semantics); the card survives on the remnant, and only
fully-departed anchors retire (the doomed predicate,
editor.rs:5893–5897). A pass in flight during a park is safe by
construction: quotes anchor at reveal against the manuscript slice
(editor.rs:3474–3499) and departed text drops as "no quote matched".
Failure if unhandled: the editor permanently goes blind to a problem
class at a spot the writer never judged. Principles: 08 §2 item 8;
margin graft; P13.

## 4 · The flip inverts every region comparison (note)

Manuscript changes from `[base..len]` to `[0..boundary]`; every
`>= base` gate silently inverts. The checklist, so none is missed:
`manuscript_base_char` / `manuscript_char_range` / `manuscript_slice`
(document.rs:970–1016 — the one source; keep it the ONLY source) and
its consumers: word count (editor.rs:1622), export (2728), the AI
pass's rebase *including* the existing-notes filter `n.range.start >=
base` (3484–3493), select-all region bounds (5953–5963), the Esc region
check (6126), the caret-memory gate (5202). Cold read is deferred
(docs/impl/plan.md:13) and must be built on the same slice. Three
non-mechanical ones:

- `should_auto_cut` (editor.rs:1399–1407) excludes the compost today;
  post-flip scraps deletions must FILE to the graveyard (design-tail
  assumption 8; 08 §2 "a deleted scrap falls one level") — the gate's
  meaning changes, not just its sign.
- `put_back` clamps origin into the manuscript "so cut prose can never
  resurrect into the compost" (document.rs:1062–1068) — wrong for a
  corpse cut FROM scraps: Put back must return it to the pile (same
  door it left by, P13). Clamp into the region containing `origin_pos`
  under the current seam.
- The export asset sidecar collects `self.doc.blocks().asset_refs()` —
  the FULL document (editor.rs:2733–2738): an image living in scraps
  leaks its file into the exported `.assets/` dir while the markdown
  never references it. Use the slice's `mblocks`. (A live bug today
  too.)

Failure: any one missed comparison silently re-scopes count/AI/export —
the exact trespass class the seam exists to kill.

## 5 · The count control's double duty (decide)

The titlebar chip (editor.rs:10913–10959) is both the count readout and
the session-goal control (`delta = word_count − start`). Caret-scoped
per spec, in scraps it reads "scraps · 312" — but the goal is a piece
instrument: rendering "+240/500" beside a scraps label mixes scopes in
one chip, and clicking it sets a *piece* goal while wearing the scraps
face — P12 cut both ways. Propose: in scraps the chip shows
"scraps · N" alone (goal delta hidden, not recomputed); click still
opens the piece goal. Two sub-rules: the "piece ·" prefix exists only
once a seam does (a seamless doc says "1,842 words" as today — no
vocabulary before its referent, P4/chip law); and chip-travel must
place the caret (at the seam), or the label stays "piece" while the
viewport shows the pile and the spec's mid-pile-screenshot claim rests
on the wash alone. Parking mid-session honestly drops the goal delta by
the parked words — accepted; the arrival receipt is the counter-story.

## 6 · The strip counts the whole rope (note)

`strip_stations` (editor.rs:2344–2348), the preview readout `words_at`
(2552–2574), and the Compare pin (2651–2665) all `count_words` over the
full state text, while the live chip is manuscript-only (1622).
asides.md §1 already decides this: accounting is manuscript-only, and
checkpoint states materialize their own boundary (the `BlockMap` rides
the state), so each count must rebase against *that state's* seam.
Failure: park 2,000 words — the titlebar drops 2,000, the strip readout
for the same moment reports no change, and a Compare across the park
shows Δ0 for a visibly shorter piece. Same rule for any find announced
inside a parked preview (editor.rs:9366–9372): the past state's own
seam scopes its split.

## 7 · `@`-headings and the pile (note)

`outline_items` walks every line (editor.rs:11326–11343), so headings
typed in the region (heading chords are ungated there, extraction §6)
already appear, undistinguished. The law says they must appear — `@` is
navigation, the writer's hands — but a row that teleports into the pile
must self-identify. Propose: scrap headings stay in document order
(post-flip they trail naturally), each row wearing the scraps grammar
as data (a muted "· scraps" suffix — same register as the find split),
and an `@`-jump below the seam is a latch-setting travel (case 1).
Palette audit alongside: select-all is already region-scoped
(editor.rs:5950–5963) and ctrl-home/end cross freely (hands);
Export/Copy-as-Markdown ride the slice; `SaveCopyAs` copies the .strop
whole — correct, a file copy is the writer's document, not an audience
surface. Failure if unhandled: `@` becomes a hidden trapdoor into the
pile with no announcement and an inert Esc.
