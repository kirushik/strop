# The History Strip, v3 — the seek bar meets its first users

*(Supersedes v2 ("a seek bar that happens to be true", 2026-07-04,
amended 2026-07-10) after the first real-user round on v0.2.0. What
users loved is untouched law: frame-rate scrubbing with the document
as its own preview, the timeline itself, flecks at document depth,
the readout, and Restore as an undoable forward edit. What they
asked — why is it so narrow, why can't I zoom, what are those
"session" marks, can I have a named version, how do I compare two
moments, where are my comments when I rewind — plus one confirmed
falsehood in the fabric (card threads drawn horizontal at today's
anchor) drive every change below. Adjudicated 2026-07-15 in a
two-round CTO RFC (Sol), against `design-principles.md` P1–P13.
Panel texts: `docs/research/history-strip-panel-2026-07.md`.)*

## 0.1 What the first users asked, and where this doc answers

| Question | Answer |
|---|---|
| "Why is it so narrow?" | §1a — the sheet and the desk |
| "Why can't I zoom?" | §1c — the adjudication, held |
| "What are those session marks?" | §1b, §2 — the marks die |
| "Can I have a named version?" | §3c — Name this version |
| "How do I compare two moments?" | §3d — Compare, promoted |
| "Where are my comments when I rewind?" | §3b — the past margin |
| *(bug)* threads drawn as horizontal lines | §1e, §6 — true paths |

Round two (the built v3 met its first fortnight of real use; a
three-voice design pass — CTO RFC, a Raskin lens on modes and exits,
a Birman lens on composition — re-solved the controls as one system):

| Round-two report | Answer |
|---|---|
| "I can't read Compare past the first page" | §3d — the room scrolls |
| "…at least show me what changed" | §3d — the change gutter |
| the strip hides the last paragraph | §1a — the floor law |
| "no way to inspect those cards in history" | §3b — placement + skeletons |
| controls "dissipated", Now in a corner | §2, §3e — geometry owns them |
| labels not clickable, "Started" mid-sheet | §2 — labels act; Started dies |
| "is my 6-week rest mostly emptiness?" | §1b — the well says its span |
| *(bug)* the past stayed on glass after close | §3e — fixed + rig-gated |

## 0.5 The visits — what the strip is FOR

The same writer arrives in opposite emotional states, sometimes in the
same hour, and the strip cannot ask why she came (P2). So one honest
surface must read correctly at three tempos of attention:

- **The glance** (2–5 s): *did it save? where was I? how is it going?*
  Served by the resting face alone: the strip opens on the TAIL (the
  writer's own morning already in the cloth — the trust visit is a
  rehearsal for the panic visit), wells show time away, the envelope
  arc shows the draft's shape, dates say when.
- **The hunt** (30–90 s): *get it back.* Rescue (panic — zero reading,
  one familiar control, big targets, reversible exits), passage
  retrieval (memory is of sentences, not dates), region forensics.
  Served by frame-rate scrubbing, arrow-stepping to stations and
  big-cut shoulders, visible selection + copy out of any past frame
  (the surgical rescue), and Restore for the wholesale one.
- **The dwell** (minutes): *was it better before?* Compare (§3d), the
  Past book ("read this version"), the panel's named-version work.
  v3 gives the dwell the chrome v2 promised it: the compare verb is
  findable now, and both moments carry their own margins.

Jurisdiction: the strip owns everything *when*-shaped. The graveyard
owns deliberate cuts (first stop for "where is it" — the strip is the
fallback). The checkpoints panel owns *what/why* — named versions'
management and narrative, the foreign-edit diff, provenance. Undo owns
the last seconds. v3 amends the boundary in exactly one place: the
strip carries the *creation* verb for a named version (§3c), because
the moment of naming is when-shaped — it happens at the scrub point,
at now or in the past. Management of what was named stays in the
panel. Resist bolting anything else on: the visits share one spine —
a truthful material record, scrubbed by hand — and differ only in
tempo and heat.

**Red lines** (each one guards a visit):
1. *The strip describes the manuscript, never the writer.* No rates,
   streaks, comparisons, or verdicts; the fabric is material, not
   metrics. The one number is the word count — a property of the
   document.
2. *No reproach.* A gap is a well, not a debt; a return finds the room
   as it was left.
3. *Never fabricate a mark.* The fabric is testimony (provenance):
   legacy eras lay no flecks, no expressiveness may add ink that isn't
   derived from the record — and (new, from the thread bug) **no
   geometry may assert a coordinate the record doesn't prove**. A
   dotted line still draws a y; where the record is silent, the strip
   draws nothing.
4. *The strip never initiates.* Opened by hand, closed cheap, no state
   worth managing.

## 0. The corridor floor

At first sight the strip is a **seek bar**. A thin rail across the top
of a dark band; a round thumb riding it; a time-and-words readout on
the left; a **Now** control at the far right. Drag the thumb and the
document above scrubs — like every video player the viewer has ever
touched. That is the entire floor, and it must survive a five-second
corridor test with a stranger who reads nothing.

Everything else in the band is *texture* until curiosity arrives. The
thumb and rail carry the highest contrast in the strip; the fabric
sits low-mid; the quiet action words (§2d) rest below both. Nothing on
the strip explains anything: no legend, no axis label, no caption. A
surface that must be read before it can be touched is a failed surface
(P4) — the strip teaches by being scrubbed, not by being studied.

## 1. The fabric

### 1a. The sheet and the desk — "why is it so narrow?"

Because the document is young — and v3 makes that reading available
instead of leaving a void that reads as broken. The history is a
**sheet**: it begins at the strip's left content edge and is exactly
as wide as the work, at the fixed quant, always. Where the sheet ends,
**now** is: a 1-px cream **selvage** descends from the rail's endpoint
through the full fabric height, and the thumb sits ON the selvage.
Beyond it lies the **desk** — ground one value darker (`#211F1A`
against the sheet's `#26251F`), carrying no fabric, no envelope fill,
no date treatment, nothing. One sentence per mark: *the cream edge is
the manuscript's now; beyond it is desk, not future.* (v2 already knew
a rail drawn past the page reads as unreachable future; v3 adds the
positive statement — a small, complete leaf on a larger desk, not a
failed progress bar at 10% of its track.)

The sheet is **left-anchored, permanently**. A centered sheet was
designed and killed (§5): it preserved the quant but not the
*position* — every old mark would slide between opens as history grew,
and the fixed scale's learnability argument applies to position too.
Left-anchored, yesterday's column is where it was yesterday; the strip
fills left to right, open after open, and that visible filling IS the
answer to "why so narrow": narrow because young; watch it grow.

**Minute one.** A fresh document with no journaled edit shows: the
readout (`Today, 21:40 · 0 words`), the quiet naming verb (§2d), a
dim Now, the thumb at the left content edge, the selvage under it —
a zero-length rail, because no rail is fabricated — and desk. Parking
is refused until there is a past to park in (the existing birth-record
rule). No text explains the emptiness; the selvage says "the page
begins here" without asserting elapsed work (P4, P6). The first run
lays the first flecks and the sheet grows rightward from there.

**The floor law** (round two; the strip hid the last paragraph). The
strip is a physical object on the floor of the window, not weather
over the prose: while it is open, the visible document viewport ends
at the strip's top border. Column width and wrapping never change —
opening history must not reflow the manuscript — but the scroll range
extends so the final baseline rests fully above the border with the
document's normal bottom breathing room, and text is clipped at the
border rather than sliding beneath the desk. One shared helper names
the visible bottom, and every companion obeys it: live margin cards,
the past margin, sidenotes, selection popovers, caret-reveal, and
page stepping. Cards anchored near the bottom pack upward inside the
visible lane; nothing ever descends behind the strip. Opening the
strip does not move the scroll (the no-jump invariant); it only
shortens what is visible until the writer scrolls.

### 1b. The quant, time, and the sittings

**The quant: one fleck = one word.** A ~2 px grain of amber. Words the
writer added are amber `#C8A951`; words the writer cut are the darker
burnt amber `#8A6D35` (value contrast — legible colorblind, P10; both
warm — the writer did both, and the fabric's warmth is the standing
proof the machine never writes). One honesty debt, named by the v3
panel (Tufte): v0.2 never counted cut words — the journal drops the
deleted text and the strip estimates its words from character count
at a fixed ratio, so half the quant was an estimate wearing a law.
v3 counts the words *before the text is let go* (`del_words` on the
run, §6); legacy runs keep the estimate, and an estimate may never
grow expressiveness of its own. And a boundary the same panel forced
into words: a run's *bounds and counts* are testimony; the placement
of grains within those bounds is typography — deterministic, seeded,
carrying no meaning — and no reading may ever depend on it.

**x = working time, one fixed scale** (1 px ≈ 30 s, product-wide,
never per-document). Gaps over ~15 min fold into **wells** — recessed
full-height columns, the visible presence of time away. Two fixed
tiers only (overnight; days-away), never gap-proportional: the axis
spends x on WORK, so absence is punctuation, not a bar chart.

**The wells and the dates carry the sittings — nothing else does.**
v2 left every session-seal checkpoint a bare full-height tick, and the
900-second idle seal leaked its internal name ("Session") into the
label lane — the marks users could not read, because they said
nothing. Both die in v3. A sitting's boundary is already expressed by
the well that caused it; a tick beside a well is duplicate grammar
(P8). What remains: the well, and one real date at the first activity
after it ("Today", "Tue 1 Jul") in the date lane. **The dates are
controls** (P12): clicking a date seeks to that sitting's first
recorded moment; hover may expand the visible date to its span
("Tue 1 Jul, 09:14–11:03" — expansion of the visible, P9). No string
containing "session" reaches the chrome, ever. Ticks in the label
lane now mean exactly one thing: a deliberate mark — a named version,
an export, a restore.

**A wide well says what it holds** (round two; "is my six-week rest
mostly emptiness?" — no: the fold already reduced it to one 20-px
well; the geometry was right and mute). The wide tier gains a
compact duration datum centered on the well in the date lane, 8–9 px
tabular: `6 days`, `6 wk`, `3 mo 2 wk` — whole days from 2 to 13,
weeks to 8, months plus weeks beyond. It reports the elapsed wall
time between the last recorded event before the fold and the first
after it — a fact about the record's interval, never a claim the
file stayed closed. Hover may expand it to the exact bounding
timestamps (the date-hover grammar, P9). It is data, not a control:
no underline, no pointer, no click. On collision it yields to
writer-named labels and the two bounding dates, and outranks reflex
labels; the well itself survives even where its datum cannot. The
overnight tier stays mute — the recess between two dates already
says everything a night says. Widths stay the two fixed tiers,
permanently: a six-week absence earns a different VALUE, not more x
— the axis spends x on work, and a proportional hole would make
absence the principal material of the sheet.

### 1c. No zoom — the adjudication, held against real users

Users asked for zoom the way music and video editors have it. The
answer stays no, and v3 records why against the live request rather
than in the abstract: one fixed scale is what makes the texture
*learnable* — a glance's worth of amber always means the same amount
of work, this week and in March. Continuous magnification turns
evidence into a camera view whose apparent density depends on an
invisible scale; it adds wheel ambiguity, label re-ranking, and a
corridor tax (P5, P6, P7, P12). What users actually need when they
reach for zoom is served without it:

- a young strip that looks intentional (§1a — the sheet);
- whole-history seeking at any length (the rail compresses, §1d);
- fine targeting (frame-rate scrub; arrow-stepping to stations and
  big-cut shoulders while parked);
- the wheel pans the cloth;
- comparison (§3d — the thing several "zoom" requests turn out to
  mean).

If a future round finds a visit these four cannot serve, it argues
here first. Density-adaptive rebinning, three-altitude modes, and
held "focus" emphasis states were all re-examined this round and
re-killed (§5). The strongest standing objection is recorded rather
than resolved: the v3 panel's three analytical lenses (Tufte's macro
reading, Victor's whole-trail-as-object, Engelbart's level-clipping)
independently converged on the same wound — at year scale the fixed
quant shows the whole story to no one, only a slit onto it. If that
visit (the mirror, the story of the draft — V9/V10) proves real, the
answer is a second *portrayal* of the same record living OUTSIDE the
strip — the sessions outline v2 §5 already reserved — never a
re-scaled fabric.

### 1d. The rail, the page, the envelope — unchanged law

**The rail IS the page's top edge**: the controls live in their own
row above; the envelope hangs from the rail; the thumb rides along the
top of the manuscript; the rail is exactly as long as the history.
At fitting scale it ends at the selvage (§1a). When the history
outgrows the viewport, the rail compresses the whole of it into full
width — a seek bar's contract is *the whole duration, always visible*
— while the fabric keeps the fixed quant under the **view lock**:
view = work − frac·travel, the one formula under which the playhead
passes through the thumb AND the correct spot in the cloth at every
scale. Rail seeks, stepping, open and Now lock; a fabric touch and
the wheel do not (§3a).

**y = position in the document, start at the top.** The cream
**envelope** — document length over time — hangs from the rail and
steps downward as the story grows, upward at cuts, visibly at
restores. **Everything shares the envelope's chars axis**: flecks,
veils, threads. The y-scale is set when the strip opens (current
length fills the band, ~10% headroom) and never changes while open.

**One truth per x.** Materialized checkpoint states are the
envelope's ground anchors; run deltas accumulate *between* them and
rebase at each. An import writes a materialized "Started" birth
checkpoint. (Unchanged from v2; the card record now follows the same
discipline — §6.)

**Density is emergent, not modal.** Flow-state drafting alpha-fuses
flecks into solid strokes; slow line-editing leaves distinct grains;
a dark column is a big cut. Nothing aggregates, nothing re-encodes.

**Veils** are unchanged: AI passes are full-height translucent cool
columns bounded by the page — the machine read everything, so the
mark spans everything, and never more than the text that existed.

### 1e. Threads — true paths, at last

A card's thread is a 1-px cool line from raised to resolved (sage
endcap) or dismissed (grey). v2 drew it **horizontal at the card's
position in today's text** — false twice: the anchor moves as text
is added and cut above it, and today's position is projected back to
moments where the card sat elsewhere (or nowhere). The panel's Victor
voice had already specced the truth: the thread *meanders as text
above it is added and cut* — the card riding its paragraph through
the growing draft, which is true and quietly beautiful.

v3 law: **a thread is a polyline of proven (x, y) points on the
envelope's chars axis.** For cards raised under the v3 record (§6),
the path is exact: seeded at the range-at-raise, advanced through
every run by the same range-transform the live margin uses, rebased
across restores by the explicit rebase record. The thread bends where
a paragraph above was cut; it collapses where its anchor was consumed.
Threads remain the machine's marks — cool, subordinate to the amber
(P11); writer notes appear in the past margin (§3b) but lay no thread.

**Where the record is silent, the strip draws nothing** (red line 3).
For legacy cards (recorded before v3), the path is recovered backward
from today's known anchor only as far as inversion is unambiguous —
the walk stops at the first edit that crosses the anchor, at any
restore, at any gap in the journal. The proven suffix draws solid; it
begins mid-air with a **hollow diamond** (5×5 px, 1-px stroke, no
fill, thread hue at higher alpha — form carries the meaning, not
color, P10): *a hollow diamond: the card's proven path begins here;
before it, the record doesn't say.* No dotted segment reaches back to
the raise — a dotted line still draws a y, which is the same lie
restyled. If nothing before today is provable, the card shows only
its terminal; if a closed card left no surviving annotation, it shows
nothing. (Rig gate, panel-demanded: the diamond must read as "the
record begins here", never as the card's raise — if corridor testing
shows it read as an event, it dies and plain absence ships.) A restore the record cannot carry a card across breaks the
thread: two solid segments separated by a 6-px blank with opposing
3-px cap ticks — *a break in a thread is a restore the record could
not carry the card across.*

**Threads answer the hand** (round two; the lines promised cards the
surface couldn't deliver). Every painted thread segment is a hit
target over its own geometry — the painted path never changes for
the sake of the hit. Clicking a thread parks at the exact moment
under the click, scrolls the parked preview so the card's proven
anchor is visible where it can be, and gives the matching past-margin
card (§3b) a brief focus outline. Hover strengthens the already
visible thread and its card together — association by light — and
never parks or opens anything (P9). No hit exists where no geometry
is drawn: an unprovable stretch of a card's past cannot be clicked
because it is not there.

## 2. Words on the strip

Exactly four kinds of text exist:

1. **Station names** — the writer's own words ("Draft complete"),
   plus the honest automatics ("Restored", "Exported") — never
   "Saved", never "Session", never "Checkpoint N", and since round
   two never an automatic "Started": the label bound itself to the
   earliest *surviving* version and appeared mid-sheet on any file
   whose journal predates its store — a birth the record didn't
   prove, red line 3 caught in the field. The sheet's own left edge
   is the beginning of the available record; it needs no caption. (A
   version the writer herself names "Started" is her data and shows
   normally.) Session starts lay no ticks and no labels at all
   (§1b). Ranked omission on collision (writer-named >
   before-restore > export > reflex); a label that doesn't fit is
   omitted whole, its tick stays; a same-named twin at the same x is
   omitted rather than stacked — which is also why duplicate names
   are *allowed*: names are writer-owned data (P3), never rewritten,
   never suffixed; time and position disambiguate. Hover expands,
   never reveals (P9). **Labels and ticks act** (round two): every
   painted station label and tick carries one shared exact-seek
   target — click parks at that station's own timestamp, precisely,
   no arithmetic asked of the hand. Labels wear the product's one
   clickable-text mark (below); hover brightens label and tick
   together. Hit padding is modest and never overlaps a neighbor;
   competing padded targets resolve to the closest painted tick,
   ties to the higher rank. A click outside any target keeps the
   two-lane law (§3a) — the cloth stays continuous; there is no
   magnetic snapping of ordinary fabric clicks. An omitted label's
   tick still carries the target; arrow stepping reaches every
   station regardless.
2. **Dates** — real ones. "Today", "Tue 1 Jul". Never "day 12". The
   year appears whenever it isn't the current one. New in v3: dates
   are seek targets (§1b). Wide wells carry their duration datum
   (§1b) — data, not a control.
3. **The readout** — recessed data at the sheet's origin, not a
   chip: fixed position aligned to the strip's left content edge,
   fixed width, tabular numerals: `Tue 12 Jul, 21:40 · 3,412 words`.
   Width reserved per locale. A low-contrast backing is allowed only
   where the fabric would fight legibility; it takes no border, no
   hover, no pointer — a box around unclickable data falsely
   promises a button (round two; the chip face dies). The readout
   never forms a sentence and never embeds a station name (P8's
   template ban). Near a station, that station's tick and label
   *brighten* — association by light, not by grammar. **While
   comparing (§3d), the readout becomes two parallel data blocks**:
   the pinned moment's dim, the active playhead's bright, the word
   delta on the bright one (`· +612 words` / `· no word-count
   change`) — the delta belongs to B because it is B − A, and
   "since" is banned: it composes a narrative relation (P8).
   Degradation is semantic, never ellipsis: same-date moments share
   one date token; then the locale's shortest unambiguous date+time
   form; at the narrowest, only the active block and the delta
   remain on the strip while the pinned moment's full readout stands
   in its own column header (§3d). Dates and counts are data; data
   is never clipped.
4. **The action words** — and the one grammar that rules them (round
   two; four idioms shared one bar and none could be learned). Form
   means exactly this, strip-wide and product-consistent:
   - **dashed underline** (the `inline_action` mark: muted ink and a
     1-px dashed rule at rest; ink brightens on hover, the dashes
     stay dashes) = a reversible text action. `Name this version`,
     `Compare`, `Done comparing`, clickable station labels,
     clickable dates, and `Now` while away from now all wear it.
     Dashed = actionable, never emphasis.
   - **dark fill** = commits a document-changing act. Exactly one
     control may wear it: `Restore`. Fill is not importance, not
     selection, not location.
   - **plain text** = data or inactive state: the readout, well
     durations, `Now` at now. No underline, no hover, no pointer.
   - **the drawn mark** = a surface operation: the dismiss saltire,
     top-right of the strip's frame, the only control that belongs
     to the container instead of the timeline.
   The cream fill the parked Now once wore dies: cream belongs to
   the selvage, and repeating it in a button invented a second
   "now". Where the verbs live is geometry's decision, not the
   screen's — §3e. These rest (P2) — nothing pulses, nothing appears
   on a timer, and the contrast order of the strip is untouched:
   thumb and rail first, Restore as the only filled verb, the quiet
   words last (P11). When width starves the moment dock, naming
   survives first (`Name version` is its only sanctioned compact
   form; Compare keeps its shift-click shortcut); below that the
   dashed verbs fold into an ellipsis control — itself dashed —
   while Restore, Now, and close never disappear.

The vocabulary is glossary law (`ux-glossary.md`): "checkpoint" and
"station" are internal register; the writer-facing category noun is
**version**. The palette command "Name a Checkpoint" — which never
prompted for a name and stamped "Checkpoint 7" — dies; the palette
verb is `Name this version` and routes into the same composer.

## 3. The controls

### 3a. The thumb — unchanged law

Two hit lanes, matching what each looks like (P7): the rail row and
above is the *seek bar* — click parks at that fraction of the whole,
drag scrubs continuously, view locked so the playhead passes through
the thumb; the fabric below is the *cloth* — a click lands on the
moment UNDER the cursor at the current pan, and the view never yanks.
The document live-renders at frame rate either way. Wheel/trackpad
pan the fabric only. While parked, arrow keys step to the previous/
next station or big-cut shoulder. Round two adds the third class of
hit: **named objects act exactly** — station labels and ticks (§2.1),
dates (§1b), thread segments (§1e), and Compare's gutter marks (§3d)
resolve before the lanes; everything unmarked keeps the lane grammar
untouched, and no target may steal a drag that began in a lane.
**Scrub stability law:** while the
thumb moves, the only things that change are the thumb's x, the
readout numerals, the document above, the past margin's projection
(§3b), the dimmed not-yet region, label brightening, and the moment
dock's recede/settle (§3e). No label re-ranks, re-flows, or changes
length mid-drag.

### 3b. The past margin — "where are my comments when I rewind?"

They come back. While parked, the live margin is replaced by the
**past margin**: the cards — writer notes and editor cards both — as
they stood at that moment. A card appears iff it was raised at or
before t and not yet closed; its body is the body **as of t** (the
committed-edit grain, §6), its status as of t, its anchor the
reconstructed historical range. Cards closed before t are gone; cards
raised after t do not yet exist. Every paused frame is a true page
from that day (P6). Scrubbing across a card's raise or close pops it
in or out with no animation — the past doesn't perform.

**Placement is the preview's own layout** (round two; v3.0 scattered
cards at anchor-fraction-of-document heights, beside nothing). A past
card stands beside its anchor's real on-screen paragraph: the
historical anchor maps through the same text layout that renders the
parked page, rides the preview's scroll, and feeds the same margin
packing the live lane uses — measured heights, culling, the
established off-screen treatment — in a read-only mode. No composer,
no resolution verbs; past cards are evidence. Clicking a past card
scrolls the preview to its anchor (navigation, never mutation).

Where a card's anchor at t cannot be proven (legacy record, an
unbridged restore), the card is not pinned to a guessed paragraph: it
sits in a small stack at the margin's foot, each card carrying a
broken-anchor mark — the same mark the margin already uses for
orphaned notes at now, and the stack has no heading: the cards are
the data, and a heading would be explanation (P4).

**The legacy skeleton** (round two — a recorded REVERSAL of v3.0's
"stays absent" ruling). v3.0 left the past margin empty for v0.2-era
records, and the field answered: the threads still draw there, so
the strip implies cards it refuses to show — a promise the surface
doesn't keep. The reversal keeps red line 3 by changing the claim,
not the honesty: where the parked t intersects a legacy card's
proven anchor suffix (§1e), the margin shows a **skeleton** — the
card's *current* body in its normal form, header stamped with the
plain datum `Now`, wearing the drained/unverified treatment and the
hollow-origin mark. The stamp is a fact about the body's source, not
an estimate of its historical content: nothing asserts this wording
existed at t, and no historical date ever touches the body. Where
the anchor at t is unproven, the skeleton joins the detached foot
stack; where even the card's relation to that time is unproven,
nothing shows — truthful absence still outranks invented history. A
card whose today-body wore yesterday's date would remain a quiet
forgery; a today-body wearing today's name is testimony.

The past is otherwise quotable, as before: selection works and
RENDERS in a parked preview, Copy lifts the words out, and the live
selection is saved at park and returns on Now/Esc/close.

### 3c. Name this version — "can I have a named version?"

Yes — at the scrub point, which is where the research said the commit
verb belongs (Etherpad's lesson: co-locate the naming act with the
timeline it marks; named checkpoints are what writers keep over
continuous history). The quiet verb (§2d) is present at now AND while
parked, living in the moment dock (§3e):

- **Activation** replaces the dock's verbs with one compact text
  field (placeholder `Version name` — field-purpose text, not
  solicitation). Focus enters it; the prose caret and selection are
  saved aside.
- **Enter** with content commits: the moment is materialized as a
  named version — at now, the live state at the commit timestamp;
  parked, the reconstructed parked state at the parked timestamp,
  **without restoring and without moving now** (naming a past moment
  is an act of record, not of surgery). The tick and the writer's
  label appear in the same frame the composer leaves (P6: no frame
  shows neither); the playhead, preview, and viewport do not move.
  Ranked label collision applies with writer-named at highest rank.
- **Esc**, clicking outside, or closing the strip cancels; empty
  Enter does nothing; no error text, no confirmation, no
  congratulation (P2). If persistence fails the composer stays —
  never a station that wasn't stored.

Names are the writer's words: never generated, never uniquified,
never truncated by the system (P3, P8). Renaming and everything else
about a version's life stays in the checkpoints panel — the strip
only lets her *plant the flag*, because planting happens on the
timeline.

### 3d. Compare — "how do I compare two moments?"

The v2 experiment — an unadvertised shift-click pin, "if no one ever
finds it, it dies quietly" — returned its result: users want the
capability and cannot find it. The resolution is contextual
promotion, not resting chrome: **Compare appears as a quiet verb only
once the writer is parked** — she has already entered the dwell; the
corridor floor never sees it (P5). Shift-click remains as the expert
shortcut it always was.

- **Compare** pins the parked moment as the *pinned moment* (A). Its
  playhead stays as a faint dashed line; the live playhead (B)
  continues to scrub normally. The readout becomes the two-block form
  (§2). The verb becomes `Done comparing`; Esc exits compare first,
  then the past — every step of the way out is the inverse of a step
  in (P13).
- **The document area becomes a reading room, not a diff.** Two
  read-only columns, A and B, each a continuous page headed by its
  moment's full readout string (`A · …` / `B · …`); in narrow
  windows, a segmented A/B switch over one column that preserves
  each side's reading position. Prose comparison answers "which
  reads better?" first — writers compare voice and flow, not
  opcodes; that priority is why the columns stay whole: no spacer
  holes punched through the prose to force alignment. **Each column
  scrolls** (round two — v3.0's room clipped at one page), and the
  columns scroll independently: any forced y-to-y coupling between
  two arbitrary moments asserts a correspondence the record doesn't
  prove. Wheel drives the column under the pointer; the narrow
  switch keeps each side's own offset. The active side is named by a
  stronger header rule, never a background wash.
- **The change gutter answers "where do I look"** (round two; the
  Changes toggle and its prefix/suffix single-region wash die). A
  paragraph-level alignment of the two moments — the established
  prose-diff grain: paragraph pass, never character confetti — lays
  permanent, quiet marks in each column's outer gutter: a warm bar
  beside changed paragraph runs on both sides, an arrival bar in B
  with a short departure notch at the corresponding boundary in A
  for prose only one side holds, the inverse for the other. The
  prose itself is never decorated: no inline strikeout, no insertion
  wash (P1 — held from v3.0). Unchanged stretches carry nothing.
  **The marks are ticks, and ticks act** (§2): clicking a gutter
  mark scrolls BOTH columns to stand that corresponding pair
  abreast — alignment on demand, at the writer's own click, instead
  of a standing claim; hover brightens the mark and its twin
  together and moves nothing (P9). Authorship is never inferred: the
  gutter asserts textual difference only.
- **Each column carries its own past margin** (§3b): the cards as of
  A beside A, as of B beside B — placed by each column's own layout
  and scroll, per §3b's placement law, never flattened into quotes
  under the prose. Uncertain cards obey §3b's rules per side;
  nothing cross-associates.
- Restore, while comparing, applies to the active moment; Now exits
  the whole past state, dropping the pin, as it always did.

### 3e. Where the controls live, and how the past is left

**Geometry owns the controls** (round two; v3.0 pinned three groups
to two screen edges and the desk's center, and the first user read
it as furniture pushed against walls). The strip's own coordinates
are the layout grid, three ownerships:

- **The sheet's origin owns the readout** (§2.3): recessed data at
  the left content edge, stable while the hand works the fabric.
- **The playhead owns the moment dock**: `Restore` (the one filled
  verb) with `Name this version` and `Compare` (dashed) beside it,
  the dock anchored to the parked playhead in the top control row,
  flipping to whichever side keeps it on the sheet/viewport and
  clamping at the edges. It does not chase the hand: while a drag
  scrubs, the dock recedes; it settles beside the playhead on
  release or keyboard step. Position is the documentation — these
  verbs concern the moment under this line.
- **The selvage owns Now**: the word stands at the selvage as the
  timeline's implicit terminal label — no tick, because the selvage
  IS the geometry; visually distinct from station names so nobody
  reads a freshly named version there. At now it is plain dim data;
  while parked or comparing it wears the dashed mark and full ink —
  the same word becomes the control, which is P12 exactly. When the
  selvage stands beyond the viewport, Now clamps to the near edge —
  still truthfully pointing the way home. Click: back to the
  present; Esc the same.
- **The frame owns close**: the dismiss saltire, top-right of the
  strip container — the only control that acts on the surface
  rather than on time.

The parked banner over the document keeps announcing the past — the
moment, the read-only condition, `Esc returns` — and keeps the
refusal pulse; since round two it carries **no verbs**. One
operation, one door: v3.0 showed two Restores at once (banner and
bar), and neither could be habituated. The dock owns the verb;
**Restore** appears there only when parked. One word; the common
word; our restore appends, destroys nothing, and is itself one
Restore away from undone (P13). No confirmation dialog exists
anywhere in the strip.

**The exit law** (round two; the scare that started this round). A
writer who left the past believed she hadn't: the exit was silent,
the scroll stayed where the past put it, and on an append-heavy
draft the present is pixel-identical to the past at that offset.
(The scare was also half real — a layout-cache bug kept the past
literally on glass after close; fixed, rig-gated on `frame_paras`,
and the law below is what makes the state legible even with the bug
dead.) Every departure from a parked or comparing state that is not
Restore now passes through one visible **return to now**:

1. any open composer resolves or cancels;
2. the pin drops, the live document returns to the glass;
3. the caret, selection, AND scroll captured at open are restored —
   the eye lands where the writer left, which no past frame can
   imitate;
4. the playhead travels to the selvage and the banner falls;
5. the frame holds one short beat (~180 ms) — a real present frame,
   not an animation for its own sake (under `reduce_motion`, a
   cross-fade with no positional flight);
6. then the initiating command completes: Esc-from-park and the Now
   label leave the strip open at now; the saltire, the history
   toggle, and a panel swap finish closing or swapping.

Esc keeps its ladder (comparing → parked → now → closed: every step
out inverts a step in, P13); the close controls stay single actions
that CLOSE — a close that refuses to close would trade one mode
error for another. Restore is the one exception to locus restore:
the document deliberately changed, so the eye stays where the
restored text is, and the strip re-bakes open at now.

## 4. The notches (P5 — depth that never taxes the floor)

- **Typing while parked refuses** — the banner's moment label pulses,
  one uniform refusal for every mutation. (The 2026-07-10 reversal of
  Raskin's law stands: the parked writer is panicking, and wants the
  Restore verb the banner offers, or deliberating, and would be
  horrified that a stray keystroke performed a compound verb.)
- **Hold the history key** — quasimode: strip rises, arrows scrub,
  Shift+arrows step station to station, release returns to now with
  nothing changed. Blind firing is always safe. *(Still unbuilt;
  still specced.)*
- **LEAP by phrase**: type a phrase while holding the history key —
  jump to the moment it was born; again, to its next change or death.
  Writers remember sentences, not timestamps. *(Still unbuilt; the
  highest-value notch on the surface.)*
- **Shift-click** pins the compare moment without touching the verb —
  the shortcut under the promoted surface (§3d).

## 5. What died, and why

**In v2** (kept for the record): zoom's three altitudes and viewspec
letters (fixed quant; stability beats altitude); the legend and every
caption (P4 — the one-sentence-per-mark discipline lives in this doc,
never on chrome); the pin as visible resting chrome; "Carry this
forward" → Restore; "words arriving/leaving"; composed readout
sentences; any re-layout during scrub.

**In v3:**

- **The centered young sheet** — preserved the quant, broke position:
  marks would slide between opens. Left-anchored won on the same
  learnability ground that fixed the scale.
- **Stretching young history to fill the width** — makes yesterday's
  work shrink as history arrives; a lie about the quant.
- **Zoom, re-adjudicated against a live user request** — held (§1c),
  with the redirections named. Also killed on the way: density-
  adaptive rebinning (mark identity changes across scale), a held
  Alt "focus isolation" mode (the strip editorializing attention; a
  new quasimode with no visit that needs it), and double-click-a-well
  centering (duplicate grammar for the clickable date).
- **Dotted "uncertain" thread segments** — a dotted line still draws
  a y; uncertainty is drawn as absence plus the diamond boundary,
  never as a softened claim (red line 3).
- **"+612 since Tue 8 Jul"** — "since" composes a narrative relation
  between two moments; the delta is data on the bright chip (P8).
- **Session ticks and every "session" string** — the marks users
  couldn't read; wells and clickable dates carry the sittings (§1b).
- **"Name a Checkpoint"** — a command that neither prompted for a
  name nor spoke the writer's register; replaced by the composer and
  the glossary's noun (§3c).
- **Annotation snapshots in every checkpoint** — would copy every
  card body into every checkpoint record (~10 KB per checkpoint on a
  20-card document; the wrong multiplication direction, the save-
  stall lesson). The card record is an event stream with a cursor
  (§6).
- **Per-keystroke card-body recording** — quadratic in body length;
  the committed-edit grain is the truthful unit (§6).

**In round two** (the controls re-solved as one system):

- **The three screen-edge control groups** — position said "the
  monitor ends here" about verbs that concern moments; geometry owns
  them now (§3e).
- **The banner's Restore** — two doors to one operation; the dock
  owns the verb, the banner indicates (§3e).
- **The cream parked-Now chip** — cream belongs to the selvage;
  repeating it in a button invented a second now (§2.4).
- **The strip's private hover-underline idiom** (`quiet_action`) —
  the product already had one clickable-text mark; a second idiom
  taxed every control with a second lesson (§2.4).
- **The automatic "Started" label** — bound to the earliest
  surviving version, it testified to births the record doesn't
  prove, mid-sheet on real files (red line 3, found in the field;
  §2.1).
- **The Changes toggle and its prefix/suffix wash** — one contiguous
  "changed region" balloons on any real session; the permanent
  paragraph change gutter replaces it (§3d).
- **The readout's and Now's chip faces** — boxes around data
  falsely promise buttons; data is plain, fill means mutation
  (§2.3–2.4).
- **Anchor-fraction card placement in the past margin** — a
  proportional guess at geometry; red line 3 extends to y. Cards
  stand beside their real paragraphs now (§3b).
- **Card quotes stacked under Compare columns** — cards are margin
  objects with anchors, not footnotes (§3d).
- **The empty legacy past margin** — REVERSED, the one v3.0 ruling
  round two overturned: threads promised what the margin refused.
  The skeleton stamped `Now` keeps the honesty and keeps the promise
  (§3b).

## 6. The card record — what v3 must write down

*(The recording laws; mechanics belong to the impl brief. The card
record follows the journal's own discipline: compact, append-only,
truthful about grain.)*

- **`CardRaised`** carries the full initial snapshot: id, kind,
  range-at-raise, body, flags, time. (v0.2 recorded no raise event at
  all — a card's raise time lived only on the live annotation, which
  is why closed-and-removed cards are unrecoverable there.)
- **`CardEdited`** carries the complete body at each *committed* edit
  — Enter, blur, card switch, close, and before history opens — never
  per keystroke, and never when the body is unchanged. Bodies are
  small; full copies beat deltas (O(1) replay, local damage).
- **`CardClosed`** stays the terminal event.
- **`CardsRebased`** — a restore is a wholesale swap, deliberately
  unjournaled as runs; card correspondence across it is therefore a
  wholesale fact too, recorded as its own event (post-restore ranges,
  statuses, dispositions — no bodies). Restore and its rebase are one
  logical act and commit as ONE durable generation — the panel
  (Engelbart) correctly refused a designed two-write crash window
  dressed as honesty. Broken threads (§1e) are for legacy records and
  true damage, never for our own writes.
- **`EditRun` gains `del_words`** — counted at record time, while the
  deleted text is still in hand (the text itself stays unstored; the
  count is cheap and makes the quant true — §1b).
- Checkpoints store a **cursor** into this stream, never copies of it.
- **The past margin is a pure projection**: (frozen card index, t) →
  cards. Built once at strip open; scrubbing binary-searches it and
  never rebakes the fabric (the stability law's `bakes` counter
  stands). Bodies live in the frozen index, not fetched live.
- **Writer notes and editor cards are both history.** Ruled this
  round: comments sit under the same "nothing is lost" law as prose —
  one law, not two retention policies. A note deleted at noon is
  still on the 11 a.m. page, exactly as cut prose is.
- **Legacy files degrade to proof**: reverse-walk what is provable
  (§1e), show no body at a time it can't be dated to, fabricate
  nothing.

## 7. Why this can still be the killer feature

Unchanged, and stronger with every round: it rides "we save every
keystroke" on machinery already shipped, it repudiates the two fears
writers actually have — silent loss and destructive restore —
*graphically*, and now it answers the first six questions real users
asked without adding a single surface that must be read before it can
be touched. The strip still looks, to a stranger in a corridor, like
the one history control every human already knows how to hold — and
to the writer of a year, like the fabric of her own book.
