# impl/15 — The scrollbar (the space rail)

*Status: ADJUDICATED 2026-07-16 — draft attacked by a three-lens
panel (Birman / Norman / Tufte); this is the synthesis; conflicts
resolved by argument, dissents noted. The structural identity (Tufte):
this rail is the sibling of the history strip's RAIL, not its fabric —
it compresses the whole document always, so it may never grow fabric
(no density textures, no counts, nothing whose truth needs a fixed
quant). The strip answers "how far along"; this rail answers only
"where."*

## 0. The one-axis law (adopted verbatim)

*Every mark — heading tick, seam crossbar, caret fleck — plots in the
thumb's own axis: rendered scroll-space. No mark may be placed by any
other coordinate.* (Bytes/words would assert positions the thumb
cannot reach; the rail is a map of the scroll surface, not a meter of
the text.)

## 1. Geometry (exact)

- **The rail lives at the TRUE window edge** (product-owner
  adjudication 2026-07-17, superseding this spec's frame-edge
  draft): since the left drawer died, the whole window IS the
  writer's sheet — the column is a typewriter's line on it, the
  cards are notes stuck beside it — and a rail standing mid-desk
  would partition the sheet into page-and-not-page. Every scrollbar
  the corridor knows lives at the window edge (Word's bar is at the
  window, not the page), and maximized-Fitts comes free.
  **Nothing renders in the rail's 14px edge column but the rail** —
  no card, pill, or popover, ever (`NOTE_LANE_TOTAL` keeps its +8
  breathing so the lane never reaches it). The rail is topmost
  window furniture.
- **Thumb:** 6px wide, 3px radius, centered (4px off the content
  edge), **min-height 32px** (below the clamp the thumb is a handle,
  not a measure — a real, named distortion every ancestor ships;
  mapping stays linear in scroll fraction). Resting `MUTED_COLOR`
  α 0.45; hover-in-column and drag α 0.75. Never warm — the thumb is
  machinery.
- **Track: none.** The page is the track; a groove is ink narrating
  what the thumb already states. (This is also what lets the rail
  draw over the opaque footnote zone for free.)
- **Hit target: the full 14px column.**
- **Extent:** top below the titlebar band; bottom obeys the shared
  visible-bottom helper — the open history strip shortens the track
  like every companion; transient overlays that don't move the
  visible bottom (footnote zone, popovers) pass UNDER the rail and
  never remap it. The thumb is the one thing never covered.
- **Standing, one face** (unanimous): no auto-hide (the spec's birth
  wound is P5 — an evaporating bar re-fails the stranger between
  scrolls; appearing/vanishing is chrome performing, P2), no
  dim-after-idle second face.
- **Content shorter than the viewport: no rail at all** (2-of-3;
  the product's grammar is unanimous — the strip fabricates no rail,
  the empty compost rail is absent, the empty seam doesn't exist; a
  full-height thumb is a control wearing an enabled face that can do
  nothing, P7. Norman's dissent — threshold flicker, corridor
  discoverability on short docs — noted and overruled by house
  grammar; the rail's birth when a document first overflows is an
  honest datum.)

## 2. Marks

**Source: H1 + H2 + dividers ⁂; capped at H2** (H3+ is outline
territory). Dividers mark by right (Birman: the scene-break novelist
has no headings at all — refusing dividers starves exactly the books
that need the rail most; Tufte: ⁂ is already typography's unlabeled
tick, it moves to the rail with zero loss). Scraps-region headings
mark too — the rail is the writer's hands, and she navigates her own
pile (the compost-fresh scope law's own split).

**Forms — length encodes depth, ink does not** (at 1px, alpha steps
fall below the smallest effective difference): H1 = 10px hairline,
H2 = 6px hairline, right-anchored 2px in from the content edge;
divider = a **2×2px dot** on the thumb's centerline (the strip's
fleck quant — a nameless form for a nameless thing). All marks
`MUTED_COLOR` α 0.55 — above `RULE_COLOR`, below any card border.
The thumb passes over marks (within the rail the thumb is the
anchor; against the page the whole assembly stays sub-anchor, P11 —
the door's anchor is the prose). Squint test: a thumb plus faint
structure, never a barcode.

**No local culling, ever** (Tufte's law): min-spacing thinning
fabricates sparsity exactly where structure is densest, and the
surviving set churns with window height. Marks overprint into
texture — a saturated stretch honestly says "much structure here";
lengths keep containment legible. If extreme density genuinely
demands relief, drop a whole class globally (dividers first, then
H2) — one rule for the whole rail, so bare always means bare.

**The seam crossbar:** the scrap line marks **mandatorily** — a
full-column 14px × 1px crossbar, same ink, the ONLY mark that spans
the column (boundary ≠ station — the selvage/tick distinction
rotated). It is the truth condition of the whole-extent mapping
(Tufte's FATAL): the graveyard grows without bound, and on a
heavily-revised novel the thumb would otherwise read "the piece
continues" screens after the manuscript ended. No region wash below
it — the always-visible crossbar partitions the rail by position
alone; a wash would repeat it (P10).

## 3. The contract

- **Drag scrolls the document live, synchronously, every frame**
  (Norman's FATAL: the moving prose is the instrument; the label is
  commentary). The thumb never eases toward the pointer.
- **Track press: warp to that position and leave the thumb grabbed
  under the cursor** — the face we borrowed is a seek bar's (no
  arrows, no bevel, ticks, a readout), and seek bars jump on every
  platform, GTK's home contract included; the grab makes a misclick
  correctable by simply not letting go. Product-wide on all three
  OSes (a per-OS split would make one gesture mean two things, P8).
  Jump is instant — no smooth-scroll flight (animated travel is a
  time tax a map doesn't owe). PgUp/PgDn keep paging where it always
  lived. **Corollary: the rail keeps the seek-bar face forever — the
  day it grows arrows or a trough, the promise flips (P7).**
- **Marks act** (unanimous FATAL-class: the strip's law is not
  "ticks mean deliberate marks," it is "named objects act exactly" —
  an unclickable tick is the decorating mark the strip already died
  of once). A mark is the third hit class, resolving before the
  track lane: click parks the viewport with that heading at the top,
  pointer-exact, hit padding ±4px never overlapping a neighbor, ties
  to the closest painted mark. No magnetic capture of ordinary track
  clicks, no release detent (the strip's thumb parks on a moment; a
  settling viewport is the snapping scrubber P7 forbids). A mark
  that answers when interrogated is also the entire discoverability
  story — wrong first guesses are repaired in one gesture. Hover
  brightens a tick (expansion of the visible, P9-lawful); no
  tooltips.

## 4. The readout (not a follower)

The drag label is a **fixed slot, not a bubble riding the thumb**
(Birman: a traveling plate crosses writer-text cards — P1 — and text
in motion is unreadable during exactly the act it serves; the strip's
own precedent is a readout "stable while the hand works the fabric").

- Lives in the lane's top chrome band (the `margin_floor` row — 
  machine chrome, guaranteed card-free; the door chip yields while
  the readout is live). Right-aligned, right edge 8px off the rail
  column, max-width 248px, tail ellipsis.
- Content: **the innermost marked heading governing the viewport
  top** (exact at every frame — a state, not an event), verbatim,
  alone. The writer's own words displayed as data: prose family,
  13px, `TEXT_COLOR` α 0.75, no plate, no border, no background (a
  box around unclickable data falsely promises a button). Never
  composed into a sentence (template ban). Over a divider-governed
  region, the readout keeps naming the governing heading; the dot
  has nothing to say and honestly says nothing.
- **No percentage, page, or word hint** — the thumb is the position
  indicator (P12); a number is a dashboard about the chrome, and
  "percent of what" is unanswerable once the graveyard shares the
  extent.
- Exists from mousedown on the rail to mouseup; never on wheel
  scroll. **Markless documents: nothing** — a nearest-paragraph
  excerpt would be prose worn as chrome (P1's founding wound; a
  heading escapes only because the writer authored it AS a label).
  The live-scrolling page is the feedback.
- Entry instant (it appears under a deliberate press, P6-clean);
  exit fade ≤120ms; `reduce_motion` → instant both ways. This is the
  surface's only transition.

## 5. The caret fleck (adopted, fenced)

One ~2px **warm** fleck (`0xC8A951`) at the caret's scroll position
(2-of-3, Birman dissenting): the refusal list bans *machine* state,
and the caret is the writer's own standing-place — thumb = interval
(*where you are looking*), fleck = point (*where you are writing*),
different geometry classes carrying different data (lawful layering).
It is also the jump-click's missing inverse: click into chapter two
and the warm point is the visible way home — click it to return
(P13). Drawn on the thumb when coincident (screenshot-true: "you are
where you were"). **The fence, written as law: the point-series
count is one, forever** — no selection ranges, no find matches, no
edit-heat, no recency trail; the next point series proposed on this
rail argues against this sentence first.

## 6. Refusals (standing)

Machine state of any kind (find matches, diagnosis positions, card
locations); fabric (density textures, counts, envelopes); a trough;
arrows; auto-hide; magnetic snap; smooth-flight jumps; position
numbers; prose in the readout; local mark-culling; a second point
series; hover-only meaning (hover may only brighten what is visible).
Named gap, logged not designed here: mark names are mouse-gated
(keyboard-only writers get tick positions, never names) — the future
outline/LEAP surface owes them an answer.

## 7. The spec's laws (panel sentences, adopted)

- *Every mark on this rail plots in the thumb's own axis — rendered
  scroll-space — and the rail answers WHERE, never HOW FAR ALONG:
  counts, percents, and progress are the strip's jurisdiction.*
  (Tufte)
- *A press anywhere on the rail warps the view to that spot and
  leaves the thumb grabbed under the cursor — so every mark answers
  when touched, and a misclick is undone by simply not letting go.*
  (Norman)
- *The rail shows the writer's structure and the viewport's place in
  it — never anyone's position, match, count, or verdict — and
  anything that ever wants to ride this rail argues against this
  sentence first.* (Birman; amended by the fleck adoption: "anyone's"
  reads "the machine's," with the writer's caret as the one fenced
  exception.)
