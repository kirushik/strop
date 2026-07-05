# The omnibar and the compost rail — behavioural spec

Born 2026-07-05 from the litmus round's process failure: "Search
pretends to be a field" was a request to make it a *real* field, and
it got hot-patched into a button instead. This spec is the corrected
intent, run through the design flow (spec → persona critique → build).
The critique's verdicts are folded in and marked (S-numbers); its
record sits at the bottom.

## 1 · The omnibar (the top-centre search)

**What it is.** A real single-line text field in the titlebar centre —
not a button wearing field clothes, not a field wearing button
clothes. The palette is its dropdown, never a second *query* input
(the ctrl-h replace row is a sanctioned distinct function, S6).

**At rest** — field-dressed with a stated runway (S4): fixed width
(320px), hairline border, cream fill, muted left-aligned placeholder
"Search", chord in the tooltip. Hover shows an I-beam. The empty
runway IS the type-here affordance. The first interaction swaps the
live field in at identical geometry — the box types the moment it is
touched (H1's spirit: no visual lie, no jump).

**Open** (click, or `ctrl-f` seeded with the selection, or
`ctrl-shift-p` seeding `>`): the same slot holds the live field,
focused, amber-bordered. The results card hangs from the field's own
left edge (S5 — a menu attached to its control), flush under the bar.
Typing filters live; the prefix grammar is unchanged (plain = find,
`>` = command, `@` = heading; the empty state names them, and that
line may wrap — Russian runs ~40% longer, S10). Find previews live —
the current match scrolls into view behind the card.

**The match counter** ("3/17") rides the field row, right of the
query — where the eye reads it (S7; the ranges are already computed
for the live preview, so the old perf excuse was false).

**Keys.** Enter in find = next match (bar stays; clicking a match row
likewise jumps and stays — one behaviour, H3). Enter on a
command/heading = execute and close. Up/Down move the row selection.
**Esc closes, returns focus to the prose, and walks the selection
home** — the find preview moves `selected_range` across matches, so
cancel restores what the omnibar opened on (S3, P13). An
Enter-executed jump does NOT restore (travel was the point), and
click-away doesn't either (the click placed a new caret on purpose).
`ctrl-h` adds the replace row at the card's top. A mousedown on card
chrome that isn't a row refocuses the query field (H2) — the card
never looks active while keystrokes route to the prose.

**Never.** No second query input anywhere; no state where the centre
control is a dead label; no lecture beyond the one empty-state line.

## 2 · The left rail = the compost

The outline dies. Third-time product decision, now recorded: nobody
cares about a multilevel header structure in a three-page blogpost.
The palette's `@` mode keeps the outline's **jump** function; its
you-are-here **map** function dies with it, accepted and named (S9).
Heading formatting stays `ctrl-1..3`.

- The left panel (`ctrl-shift-o`, the titlebar toggle, tooltip
  "Compost") lists the compost: header + one row per item, click
  scrolls to it and flashes it. Empty compost opens to the header and
  air — no hint, no lecture.
- **Panel identity (S1, arbitrated):** the compost's *text* lives in
  the document (the styled region past the boundary), editable in
  place; the panel is its **navigator** — an interim, and a named
  one. The end state per asides.md is the editable left *column*;
  that is the two-flow caret decision reserved for the joint session,
  not a thing to gamble on solo. Until then the panel row list is
  honest chrome over text that remains text (P3 holds because the
  region itself stays the writer's editable prose).
- **Set aside never closes the panel** (the old mutual-exclusivity
  relic did). Compliance is viewport-visible regardless of scroll and
  panel state (S2, N1): the compost's FIRST birth opens the rail
  once; later arrivals blink the newest row when the rail is open and
  blink the **titlebar toggle** when it's closed (P12 — the control
  is the indicator). A non-empty compost leaves a lasting presence
  dot on the toggle (H5) — presence, never a count (asides.md §5's
  anti-gamification line holds).
- The palette command reads "Compost Rail" (alias "outline" for
  muscle memory).

## 3 · Feedback triage (the process rule this spec exists to encode)

UX feedback splits into **bugs** (behaviour diverges from an agreed
design — fix directly, gates apply) and **design corrections** (the
agreed design itself is wrong). A design correction NEVER goes
straight to code: it gets an intent echo (one-paragraph behavioural
spec, cheap to veto), a critique pass (the personas), and only then
implementation against the fidelity gates. When the writer's words
support two readings, the tool takes the reading that makes the
product more capable, not the one that is cheaper to build — and says
which reading it took. A correction made three times is a standing
product decision: record it where it can't be relitigated by default.

## 4 · Critique record (2026-07-05, Birman/Raskin/Norman personas)

Accepted: S2 (viewport-visible compliance), S3 (Esc restores
selection), S4 (runway), S5 (dropdown anchored to field box), S6
(replace-row exception named), S7 (counter back in the field row —
the perf rationale was false), S8 (labels renamed), S9 (@ replaces
jump only, map loss named), S10 (empty state may wrap), H2 (chrome
click refocuses), H5 (presence dot). Arbitrated: S1 — panel stays a
navigator *for now*, end state is the editable left column, decided
with the writer (the deviation is named here, which is the rule).
Noted: H3 was already converged in code (find rows jump-and-stay);
H4 (drag-overshoot) is covered by the field's occlusion — drags
starting in the field never reach the window-drag handle; H1's letter
(one persistent entity) traded for identical-geometry swap, its
spirit (no lie, no jump) kept.
