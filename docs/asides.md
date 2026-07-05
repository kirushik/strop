# Asides — the compost rail and the graveyard

*(The not-manuscript, specified as one package: what it is, how it
enters and leaves, what it looks like, what it never does. Supersedes
golden-path §9.2's "compost pivots to cards" — round 4 walked that
back, correctly: "things on the left are not the cards; probably just
paragraphs." Governed by `design-principles.md`, chiefly P3
(everything the writer owns is text) and P4 (show, don't explain).)*

## 0. Two piles, one nature

Both piles are the writer's text (P3) — never widgets. They differ by
how material *enters*:

| | **Compost** (left rail) | **Graveyard** (document footer) |
|---|---|---|
| enters by | the writer's own hand | automatically, on every prose cut |
| contains | ideas, clippings, elements, todos | prose that was cut |
| editable | fully — it's text | read-only (it is a record) |
| leaves by | promotion into prose, or deletion | **Put back**, or Delete |
| exists for | material that isn't manuscript *yet* | trust — visible insurance that cuts survive |

The graveyard is the *automatic* pile (the software's promise made
visible); the compost is the *deliberate* pile (the writer's scrap
box). Confusing them was the source of the "two card families"
cognitive-load worry; the table dissolves it.

**The trigger is deletion, not departure.** Text that leaves the prose
for another writer-owned region — an aside to the compost, a note
changing address — is a *move* and files nowhere else. Only text that
is deleted from the manuscript falls to the graveyard. (Without this
sentence every aside would double-file: a compost item plus a graveyard
corpse of the same words.)

## 1. Compost is a text region, not a card system

The rail holds **one continuous text**. A blank line separates items;
the rail renders each boundary as air plus a hairline. That is the
entire data model, and every question round 4 asked answers itself:

- **How do I add an item?** The floor entry (P5) is the **aside verb
  on a selection or the caret line** — deliberate, discoverable in the
  selection menu and the palette, and it *births* the rail visibly
  (the arrival lands on a rail that now exists). Typing directly into
  the rail — click the tail, type — is the drill notch, discovered
  once the rail has content. An empty rail is simply absent: no
  affordance pretends otherwise, because an invisible typeable strip
  of margin would be a false non-affordance and an empty region has no
  referent to label.
- **How do I edit one?** Click in it. It's text; the caret is the
  affordance.
- **The separator is exactly one empty paragraph**, rendered at full
  line height with the hairline at its midline. The caret can stand on
  it; two consecutive blank lines draw two hairlines; nothing
  collapses, nothing is synthesized — the render never lies about the
  text. Backspace at an item's start visibly removes the hairline and
  merges (screenshot-true at every step, P6). A clipping that contains
  a blank line honestly arrives as two items. Item-level selection
  (triple-click and drag-over) includes the trailing separator, so
  reorder never strands an orphan hairline.
- **The tail is the rail's anchor object (P11):** a quiet resting mark
  after the last item — the live end, where arrivals blink, where the
  click-to-append target sits. Not freshness-fading of old items:
  "drained" already means stale in the color language (P10).
- **How do I format it?** The same way as anywhere: *wherever the
  writer's text is, the writer's tools are* (P3). Selecting inside the
  rail raises the same formatting flank the prose gets. (This also
  names a shipped gap: today's note composer raises no formatting
  affordance at all — the flank must work in every warm-text surface.)
- **How do I reorder / group / promote?** Text mechanics: select, cut,
  paste — and drag-of-selection, which is the text widget's native
  contract (P7; our TextField must actually honor it before any UI
  copy cites it — a build item, not an assumption). No drag handles,
  no item chrome.
- **Todos?** Written as text, struck through when done — the folk
  practice. No checkboxes in any writer text, ever (standing refusal).
- **A multi-paragraph clipping** is one item so long as it contains no
  blank line.

Because the compost is a region of the document, checkpoints and the
every-keystroke promise cover it for free. **The accounting does not
bleed:** word counts, targets, the readout, and the strip's envelope
are manuscript-only; rail edits are recorded like everything else but
are not portrayed on the strip's fabric in v1 (its y-axis is
manuscript position — a thin rail sub-lane is the future option if it
earns itself).

Visually the rail reads as **a mini-column**: same type as the prose at
a smaller size, pronounced left margin, hairlines between items, no
per-item buttons, headers, counts, or borders at rest. Kirill's own
words are the brief: "an overgrown tasklist or a mini-column of
spreadsheet" — the *shape* of a list with the *mechanics* of text.
Its scroll is independent of the prose, opens at the tail, position
persisted; its measure never drops below ~35ch (below that the rail
collapses to its edge rather than rendering wall-of-syllable lines).

## 2. Inflows

1. **Typed** — the parking notch: an idea arrives mid-sentence. One
   held gesture ("the aside key") puts the caret at the compost tail;
   Esc (or the same key) returns to the prose with the caret restored
   *exactly* — a reflex must cost zero mouse trips, or it is an
   excursion, not a reflex.
2. **Sent from prose** — the aside verb (palette / selection menu)
   moves the selection to the compost tail. A move, not a cut: the
   prose loses the text as ordinary history, the rail gains it, the
   graveyard is not involved (§0).
3. **A margin note whose anchor was cut.** The note's text quietly
   appends to the compost tail, led by its anchor fragment **in the
   exact typographic form margin notes already use for anchor
   fragments** — one grammar for anchor quotes everywhere (P8), so a
   writer who has ever seen a margin note recognizes the pair at a
   glance, tomorrow, with no blink to have witnessed. The rail edge
   gives the same single blink the graveyard footer uses on arrival
   (one grammar for "something arrived over there").
   **The words "unanchored" and "orphaned" never appear anywhere** —
   they are our headspace, and they frighten (round 4: "50% of
   potential users would just freak out"). Nothing modal happens; the
   note simply changed address, and both its parts remain the writer's
   editable text.

## 3. The graveyard, de-chattified

The design stands as shipped in round 3 (sticky footer chip; click
scrolls; exile blinks the bar, ticks the counter; show-origin retains
the segment; the footer unsticks into the section header — screenshot
test at every frame). Round 4 removes the words:

- The caption **"read-only — put back or delete" dies** (P4): the
  entry is visibly read-only and already carries its two buttons. A
  string whose audience is "a user who hasn't understood yet" is a
  design failure wearing helpfulness.
- **One verb, one action (P8): "Put back."** On the entry, and on the
  post-exile affordance in the footer — which until now said "undo".
  Two labels for one action taught two actions that don't exist.
- An entry is: the cut text (dimmed, struck ancestry visible), its
  origin affordance, Put back, Delete. Nothing else.

## 4. Chirality — why compost lives on the left

**The left side is the writer's workshop: materials and tools. The
right side is the conversation about the text: notes, questions, the
editor.** The compost rail (material) and the formatting flank (tool)
share the left; margin notes, AI cards, and the ask menu share the
right. One sentence, and every future side-of-screen decision is a
consequence rather than a debate.

Two scope rules keep it honest. **Chirality governs resting regions**
(rails, footers, lanes); at-hand menus go where the hand is — the
selection menu may carry the aside verb even though asides land left,
because a menu is not a region. And **the flank rises in a reserved
gutter between rail and prose**, never over the rail (chrome on
writer text would break P1); a selection *inside* the rail raises the
same flank in the same gutter, at the rail's right edge.

### The flank geometry — a principled call, not taste

Round 4 asked whether the left formatting affordance should go
multi-column "to rhyme" with the wide, short menu on the right. The
principle that decides it:

> **Closed sets pack into grids; open sets stack into sentences.**

The formatting set is closed and small — the engine has exactly eight
inline attributes (emphasis, strong, strikethrough, underline,
highlight, code, link, footnote) — a recognition set, learned once,
operated by position and glyph. Closed toggle sets want a compact
glyph grid: **2 × 4**, which halves pointer travel versus a 1 × 8
stack (Fitts) and gives muscle memory a stable shape. One honesty
seam inside the grid (P8): six cells are instant toggles, two (link,
footnote) take an argument and open an input — the pair sits below a
hairline, parallel form reserved for parallel behavior. The right-side
menu is an *open* set of sentence verbs ("Ask the editor for…") — verbs
read, so they stack in one column, full phrases, top-down.

The **rhyme is in placement, not in shape**: both flanks rise with the
same motion, at the same vertical origin (the selection's block), the
same offset from the text column, the same material and elevation. They
counterweight the way a drop cap counterweights a marginal note —
balance, not mirror-symmetry. Forcing equal column counts would trade a
real principle for a visual pun.

## 5. Refusals

Checkboxes in writer text. Item chrome at rest (buttons, handles,
counts, badges on compost items). "Unanchored", "orphaned", or any
system taxonomy shown to the writer. Explanatory captions on either
pile (P4). A counter that gamifies compost size. Auto-tidying,
auto-sorting, or expiry of compost — it is the writer's scrap box, and
the tool never wants anything from her (P2).
