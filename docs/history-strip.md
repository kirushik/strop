# The History Strip, v2 — a seek bar that happens to be true

*(Supersedes v1 ("the Envelope", 2026-07-04 panel synthesis) after a
corridor test killed its pedagogy: a first-time viewer refused to learn
it. The fabric survives; the teaching inverted. Governed by
`design-principles.md` — P4 show-don't-explain, P5 corridor floor,
P8 grammar, P10 color-speaks-once. Panel texts:
`docs/research/history-strip-panel-2026-07.md`. Amended 2026-07-10
after the visits pass — §0.5, the wells, the rail-as-page-edge
composition, the two hit lanes, and the §4 Raskin reversal are that
round; the reassembly shipped on branch `strip-reassembly`.)*

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
- **The dwell** (minutes): *was it better before?* Compare pin, the
  Past book ("read this version"), the panel's named-version work.
  The strip hands off; it does not absorb these.

Jurisdiction: the strip owns everything *when*-shaped. The graveyard
owns deliberate cuts (first stop for "where is it" — the strip is the
fallback). The checkpoints panel owns *what/why* — named versions,
narrative rows, the foreign-edit diff, provenance. Undo owns the last
seconds. Resist bolting compare views or stats pages onto the strip:
the visits share one spine — a truthful material record, scrubbed by
hand — and differ only in tempo and heat.

**Red lines** (each one guards a visit):
1. *The strip describes the manuscript, never the writer.* No rates,
   streaks, comparisons, or verdicts; the fabric is material, not
   metrics. The one number is the word count — a property of the
   document.
2. *No reproach.* A gap is a well, not a debt; a return finds the room
   as it was left.
3. *Never fabricate a mark.* The fabric is testimony (provenance):
   legacy eras lay no flecks, and no expressiveness may add ink that
   isn't derived from the record.
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
(runs, veils, threads, envelope) sits low-mid. Nothing on the strip
explains anything: there is no legend, no axis label, no caption. A
surface that must be read before it can be touched is a failed surface
(P4) — the strip teaches by being scrubbed, not by being studied.

## 1. The fabric — one fixed quant, never zoomed

**The quant: one fleck = one word.** A ~2 px grain of amber. Words the
writer added are amber `#C8A951`; words the writer cut are the darker
burnt amber `#8A6D35` (value contrast — legible colorblind, P10; both
warm — the writer did both, and the fabric's warmth is the standing
proof the machine never writes). The quant rhymes with everything else
in Strop that counts: targets are words, session summaries are words,
the readout is words.

**x = working time, one fixed scale** (on the order of 1 px ≈ 30 s;
the constant is chosen once, product-wide, after measurement — never
per-document). Gaps over ~15 min fold into **wells** — recessed
full-height columns, the visible presence of time away (a re-entry
after days reads the well first). Two fixed tiers only (overnight;
days-away), never gap-proportional: the axis spends x on WORK, so
absence is punctuation, not a bar chart. Session starts carry real
dates ("Tue 1 Jul") in a quiet lane — dates are data, so they may be
words (P4). **There is no zoom.** One scale means the texture is
*learnable*: a glance's worth of amber always means the same amount of
work.

**The rail IS the page's top edge** (2026-07-10 recomposition, from
the lab mockup the first build mistranslated): the chips live in their
own row above; the envelope hangs from the rail itself, so the thumb
literally rides along the top of the manuscript, and the rail is
exactly as long as the history — at fitting scale it ends where the
page ends (a rail drawn past the page read as unreachable future; a
thumb at the rail's right end is always *now*).

**The rail and the fabric split their jobs at scale.** A seek bar's
contract is *the whole duration, always visible* — so when the history
outgrows the viewport the rail compresses the whole of it into full
width: thumb x = position in the whole history, at a fortnight or at a
novel. The fabric band keeps the fixed quant and follows under a
**view lock**: view = work − frac·travel, the one formula under which
the playhead line passes through the thumb AND the correct spot in the
cloth at every scale (at fitting scale it reduces to view = 0; at now
it lands the view on the tail — which is also where the strip OPENS:
the writer's own morning is the first thing in sight). Rail seeks,
stepping, open and Now lock; a fabric touch and the wheel do not — see
§3. This keeps the corridor contract honest at any length (P7) without
ever re-scaling the texture.

**y = position in the document, start at the top** — the text grows
downward, as text does. The cream **envelope** — document length over
time — hangs from the rail and steps downward as the story grows,
upward at cuts, visibly at restores. **Everything shares the
envelope's chars axis**: a fleck paints at its edit's position on that
same scale, so an append rides the growing edge and a mid-doc cut
lands inside the page (the first build normalized by the instantaneous
doc length over the full band — every append painted at the band
floor, a dirt band the page never touched); threads and veils are
bounded by the page too. The machine's marks stay inside the text it
actually read. The y-scale is set when the strip opens (current length
fills the band, with ~10% headroom) and does not change while it is
open: nothing re-scales under the viewer's eyes. The headroom exists
because a restore can make *now* longer than the open-time length; if
a restore exceeds even that, the re-scale happens at the restore — a
data change, the one lawful re-layout — never during viewing.

**One truth per x.** Materialized checkpoint states are the envelope's
ground anchors; run deltas accumulate *between* them and rebase at
each (a restore's wholesale swap is journal-suppressed, so run deltas
alone drift after one — two independent bookkeepings merge-sorted onto
one polyline drew sawtooth spikes wherever they disagreed). For the
same reason an import writes a materialized "Started" birth
checkpoint: without it the strip believes an imported novel began
empty, and a scrub past the first keystroke replays it away. (A
birth record alone is not "a past": with nothing journaled yet,
parking is refused until the axis outgrows the birth moment — but any
real edit in the journal is a past, however brief.)

**Density is emergent, not modal.** Real numbers, the contest story:
a 4,188-word final draft at typical drafting churn (~1.8× total
insertions) is roughly 7,500 words in and 3,300 out — call it eleven
thousand flecks over maybe twenty working hours, which at this scale is
~2,400 px of strip: two viewport-widths, a shallow scroll. Flow-state
drafting lays down ~6 words per pixel-column, so flecks alpha-fuse into
solid strokes; slow line-editing leaves distinct grains. The texture
becomes readable without any encoding switch: **stroke = flow, grain =
deliberation, a dark column = a big cut.** Nothing aggregates, nothing
re-encodes; fusing is what eleven thousand honest marks do at a fixed
scale.

**The rest of the fabric**, unchanged from v1 and subordinate in
contrast: AI passes are full-height translucent cool veils (the machine
read everything, so the mark spans everything); cards are 1-px cool
threads from raised to resolved (sage endcap) or dismissed (grey) —
their visible length is how long a question stayed open; checkpoints
are hairline ticks with their names in a label lane above the band;
restores are a sage tick, a visible envelope step, and a thin sage arc
riding the label lane back to the source station. All of it says what
it says in color and form only — no words repeat it (P10).

## 2. Words on the strip

Exactly three kinds of text exist:

1. **Station names** — the writer's own words ("Draft complete"), plus
   the honest automatics ("Started", "Restored", "Exported") — never
   "Saved": the product saves every keystroke, and a station named
   Saved would teach that unsaved states exist. Reflex checkpoints
   (Ctrl+S) are deliberately unnamed — bare ticks, lowest rank — and
   so are session starts (2026-07-10): the date lane already says when
   a sitting began, and a lane of "Session start" echoes was the
   doubled-print smear; only the document's very first station keeps a
   name ("Started" — its birth is data). Ranked omission on collision
   (writer-named > seal > before-restore > export > session-start >
   reflex; a "manual" tier isn't distinct — a manual checkpoint always
   carries the writer's own name, so it ranks writer-named); a label
   that doesn't fit is omitted whole, its tick stays, and a same-named
   twin at the same x is omitted rather than stacked. Hover expands,
   never reveals (P9).
2. **Dates** — real ones. "Today", "Tue 1 Jul". Never "day 12". The
   year appears whenever it isn't the current one — histories never
   expire.
3. **The readout** — one chip, fixed position at the left end, fixed
   width, tabular numerals: `Tue 12 Jul, 21:40 · 3,412 words`. The
   width is reserved *per locale* («Вт, 12 июл, 21:40 · 3 412 слов»
   has different metrics, space-thousands, 24h); the Compare notch's
   delta folds into the same single line. The readout never forms a
   sentence and **never embeds a station name** (P8's template ban:
   "after Before the line read" must be unconstructible). When the
   playhead is near a station, that station's tick and label
   *brighten* — association by light, not by grammar.

## 3. The controls

- **The thumb.** Two hit lanes, matching what each looks like (P7):
  the rail row and above is the *seek bar* — click = park at that
  fraction of the whole, drag = scrub, continuously, with the view
  locked so the playhead passes through the thumb; the fabric below is
  the *cloth* — a click lands on the moment UNDER the cursor at the
  current pan, and the view never yanks away from what was just
  touched (after a fabric touch the thumb alone shows the global
  position, until the next rail interaction re-binds them). The
  document above live-renders at frame rate either way (Victor's bar:
  scrub at frame rate or don't ship). Wheel/trackpad gestures pan the
  fabric only, never move the thumb. **While parked, the arrow keys
  step** to the previous/next station or big-cut shoulder — the rescue
  ratchet: "just before the damage" is one keypress. **Scrub stability
  law:** while the thumb moves, the only things that change are the
  thumb's x, the readout numerals, the document above, the dimmed
  not-yet region right of the playhead, and label brightening. No
  label re-ranks, re-flows, or changes length mid-drag — layout is
  computed when data changes, never while the viewer holds the thumb.
- **The past is quotable.** Selection works and RENDERS in a parked
  preview (a selection you can't see fails P6), and Copy lifts the
  words out — the surgical rescue, beside Restore's wholesale one.
  The live selection is saved at park and returns on Now/Esc/close
  (a preview round-trip must give back the identical frame).
- **Now** — the rightmost control, always. Click: back to the present.
  Esc does the same. At now the chip rests dim; **when parked it
  brightens in the same beat Restore appears** — the two exits from
  the past (keep this / leave) announce themselves as the pair they
  are (P8), from their fixed ends.
- **Restore** — appears beside the readout only when parked in the
  past. One word; the common word (P7: we honor the widget's face and
  *extend* its contract — our restore appends, destroys nothing; the
  envelope visibly steps and everything stays inked). And the restore
  is itself one Restore away from undone — the pre-restore now is just
  another moment on the strip, recovered by the same grammar (P13). No
  confirmation dialog exists anywhere in the strip — the safety is
  structural, so a warning would be a confession.

## 4. The notches (P5 — depth that never taxes the floor)

Unadvertised, resting where a curious hand falls, each one a *but of
course* when found:

- **Typing while parked refuses** — the banner's moment label pulses,
  one uniform refusal for every mutation. *(Amended 2026-07-10; v1/v2
  specced Raskin's law — typing in the past silently restores, then
  appends. The emotional lens killed it: the parked writer is either
  panicking, and wants the Restore verb the banner already offers, or
  deliberating, and would be horrified that a stray keystroke
  performed a compound verb on the whole document. Raskin's move
  served a demo, not a state — and a read-only face that secretly
  writes subverts P7. The litmus round's refusal ships; Restore stays
  one visible act away.)*
- **Hold the history key** — quasimode: strip rises, arrows scrub,
  Shift+arrows step station to station, release returns to now with
  nothing changed. Blind firing is always safe.
- **LEAP by phrase**: type a phrase while holding the history key —
  jump to the moment it was born; again, to its next change or death.
  Writers remember sentences, not timestamps.
- **Compare** (shift-click parks a second, faint playhead; the readout
  gains a delta line). No chrome advertises it; if no one ever finds
  it, it dies quietly in a later round.

## 5. What died in v2, and why

- **Zoom** (three altitudes, viewspec letters, session first-lines) —
  killed by the fixed quant. One scale, horizontal scroll. Stability
  beats altitude; Engelbart's per-session summary lines may return
  someday *outside* the strip (a sessions list is not a strip concern).
- **The legend and every caption** — P4. The v1 spec's
  one-sentence-per-mark discipline remains a *design gate* (a mark you
  can't caption in one sentence is a mark you can't ship) but the
  sentence lives in this document, never on the chrome.
- **The pin as visible chrome** — "why would I want to pin a second
  moment?" is a question a floor element must never raise. Demoted to
  a notch.
- **"Carry this forward"** → **Restore.** Concise, universal,
  contract-honoring-plus.
- **"Words arriving / words leaving"** — the strip does not
  editorialize; the words were cut by the writer, they didn't leave.
- **Composed readout sentences** ("after ⟨station⟩") — P8 template ban.
- **Any re-layout during scrub** — the jumpiness that made v1 feel
  unreliable; opposite of habit-forming.

## 6. Why this can still be the killer feature

Unchanged from v1, and stronger for the floor: it rides "we save every
keystroke" on machinery already shipped (materialized states →
microsecond any-state recompute → frame-rate scrubbing is ~free), it
repudiates the two fears writers actually have — silent loss and
destructive restore — *graphically*, and it now does so while looking,
to a stranger in a corridor, like the one history control every human
already knows how to hold.
