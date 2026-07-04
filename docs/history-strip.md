# The History Strip — design spec (synthesis of the 2026-07-04 panel)

*(Companion to golden-path.md §9.2. Method: the Data Laboratory Δλ algorithm
applied to Strop's atoms, then a six-voice panel — Δλ/Misyutina, Tufte,
Victor, Raskin, Engelbart, Birman — each responding to the same brief and
Kirill's v2 critique. Full persona texts:
`docs/research/history-strip-panel-2026-07.md`. The panel converged on one
design; the decisions below mark where I chose among its variants.)*

## 0. The particle, first

Δλ discipline: no picture before the particle. Strop's history particle is
**the coalesced op-run** — one stretch of continuous writer activity of one
kind: `{time-interval, char-span, insert|delete}`. Everything else is event
particles over the same clock: passes `{interval, kind, span}`, cards
`{raised-t, anchor, altitude → resolved|dismissed-t}`, checkpoints
`{t, kind, name}`, restores `{t, from}`. Sessions and bursts are DERIVED
(gaps in the stream), never imposed — v2's central sin, named by all six.

## 1. The frame (one caption, printed on the strip)

> **Across — your working time, breaks folded. Up — the story's length;
> anything drawn at a height sits at that place in the text.**

- **x = working time.** Linear within sessions; gaps >15 min fold into thin
  labeled seams ("2 d") — compression declared, never smuggled (all six).
- **y = character position**, document start at the bottom, so the
  **envelope** — the derived length-over-time line — rises like a skyline.
- **The envelope is the strip's ONE bold object** (Birman's anchor law;
  Tufte's hero; Victor's coastline): cream `#FBFAF8` — literally the color
  of the page above — stepwise, never smoothed ("the steps are the truth").
  Collapsed to word-size it is the titlebar's sparkline (Tufte), inflating
  into the strip on open.

## 2. The marks (each with its printed sentence)

| Particle | Mark | Channels |
|---|---|---|
| op-run, insert | **solid warm-amber stroke** | x=when+duration · y=exact span touched · area=characters (Δλ additivity) · opacity composes → rework glows |
| op-run, delete | **hollow/hatched burnt-amber stroke** (`~#8A6D35`) | same channels; both kinds WARM — the writer did both, and the fabric's color proves the AI never writes (Δλ). Fill-state + value disambiguate without hue (colorblind-safe) |
| AI pass | **full-height translucent cool-blue veil** (selection-scoped: its span only), width = real duration, kind named in small type inside at rest | repairs v2's category error: the machine read everything, so the mark spans everything (unanimous) |
| card | **1-px cool-blue thread** from (raised-t, anchor-y) forward, meandering as the text around its anchor grows and cuts (Victor: "true and quietly beautiful"); lightness step = altitude; terminal: sage dot = resolved, hollow grey = dismissed; open threads run to now | open-duration becomes visible length — the writer's responsiveness, free of charge (Tufte) |
| checkpoint | **full-height hairline tick** + name in a dedicated **label lane above the field**, set at rest | see §3 wording & collision |
| restore | **sage tick; the envelope visibly steps; a thin sage arc rides the label lane back to the source station; everything left stays fully inked** | the anti-fear argument made without a single word (unanimous); caption: *"sage means something came back; nothing was destroyed"* |

## 3. Words (Birman's law)

Writer-named stations in the writer's own words ("Draft complete"), never
"checkpoint-7". Automatic ones speak human: **"Started", "Break", "Saved"**
— never "idle-gap", never "ctrl+s". Dates are real: **"Today",
"Yesterday", "Tue 12 Jul"** — never "day 12" (the writer does not number
her days). **Collision policy: ranked omission, never truncation** —
writer-named > seal > before-restore > export > manual > session-start >
save; a label that doesn't fit is omitted whole, its tick stays; zoom
readmits by rank, the way a city map reveals street names. Hover may only
EXPAND the visible ("Tue 12 Jul" → "Tue 12 Jul, 19:02–23:14").

## 4. The scrub (Victor's loop, Raskin's safety)

- The playhead is a cream hairline with a **permanent readout**: "Tue 12
  Jul, 21:40 · 8,214 words · after 'Draft complete'" — the control is the
  indicator (Birman). It stops ANYWHERE; the stream is continuous.
- Dragging live-renders the document above at frame rate, viewport locked
  to the edit locus. **"Scrub at frame rate or don't ship the strip"**
  (Victor). Right of the playhead the field dims one alpha step — a static
  encoding of position, so every frame is a legal still (screenshot test
  by construction).
- **Pin a second playhead** (`,`): the readout becomes a delta ledger
  ("+1,204 words · 2d 3h · 2 questions answered") and the document can
  show the interval diff. Two moments, comparable side by side
  (Victor/Engelbart/Δλ).
- **The history key is a quasimode** (Raskin): hold — the strip rises and
  arrows scrub (Shift = station to station); release — back to now,
  nothing changed; blind firing is always safe. Enter parks to read.
- **Typing means one thing everywhere** (Raskin's Law applied): typing
  while parked in the past first appends the restore (the sage arc draws
  itself), then the insertion. No confirmation dialog exists anywhere in
  the strip — a warning is a confession.
- **LEAP on the time axis** (Raskin): type a phrase while holding the
  history key → jump to the moment it was born; again → its next change or
  its death. Writers remember sentences, not timestamps.

## 5. Zoom (one geometry, three altitudes)

Continuous x-dilation anchored at the playhead with three landmark rungs —
**Project / Session / Burst**. The marks never re-encode; zoom RESOLVES
(Victor: "it never translates"). At Project scale strokes fuse into heat
(opacity ∝ ops per pixel-column — merge rule stated once: *marks merge,
counts survive*, Engelbart); at Burst scale a run's slope is typing speed —
Marey's trains, exactly (Tufte). Engelbart's addition, adopted: at Project
level **every session gets a derived first line** ("Tue 14 · 47 min ·
+1,204 −310 · 1 pass · 3 answered") — the campaign reads as an outline; and
kind-toggles/jumps as single letters (`t/p/c/k/r`; `j s` next station,
`j c` next unresolved question) with the active view echoed in the corner —
the strip is a pure function of (record, viewspec, playhead).

## 6. Refusals (the panel's unanimous floor)

Hover-only meaning. Mystery heights — any channel that can't earn one
printed sentence gets deleted, not documented. Single-kind session bars
(averaging is information destruction). Whole-document events drawn as
dots. Snap-to-chunk scrubbing. Confirmation dialogs. Smoothed envelopes.
Play-button-as-hero (Wave's grave). A "simple mode" that amputates the
grammar. Animation frames that depict states that never existed. And any
view that elides the deletions — a history that flatters is not a history.

## 7. Why this can be the killer feature

It rides the promise no competitor makes ("we save every keystroke") on
machinery we already shipped (materialized states → microsecond any-state
recompute → frame-rate scrubbing that collapsed Etherpad at 10k changesets
is ~free here), it repudiates the two fears writers actually have (silent
loss; destructive restore) *graphically* rather than verbally, and it gives
the writer what Engelbart called process literacy: the fabric of their own
fortnight, readable at a glance, explorable to the keystroke — the
golden path's whole arc (sessions, seal, cold read, passes, cuts, restore,
export) legible in one still image.
