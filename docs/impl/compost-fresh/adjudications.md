# Scraps build — corner-case adjudications (2026-07-06)

Rulings over the five `corners-*.md` files in this directory (40 cases:
2 blockers, 15 decides, 23 notes). Each corner file holds the full
analysis and code anchors; this file states what is LAW for the build.
Where a ruling says ACCEPTED, the corner file's "resolution" text is
the spec verbatim. Deviations are spelled out. Gate 0: these rulings
follow recorded law + the writer's canon framing (Scraps deliberate/
counted/living; graveyard almost-by-accident "undo-plus-plus"); none
required re-opening a product decision.

## The foundation (both blockers, one ruling)

**Era-tagged boundary, persisted under its own name.** ACCEPTED
(seam-mechanics 1 + time-persistence 1, merged): a new blocks-map key
(`scrap_line`) + serde field beside the legacy `aside_boundary` (kept
as read-only top-era alias); era = which field is present; serde
default = Top so every existing file/checkpoint/history state decodes
as old-era; migrated saves write legacy `boundary: null` so top-era
builds degrade to no-boundary, never misread. NOT a new BlockKind
(older builds' serde would reset every block kind). All region math
stays centralized (manuscript_base_char / manuscript_char_range /
manuscript_slice / caret_region_bytes) and branches on era. Never
convert on restore — conversion happens only in the writer's one
recorded migration transaction.

**The seam is journaled.** ACCEPTED (time-persistence 2):
`JournalEvent::Seam { t, at }` recorded by every boundary mutation
(park, chord-park, adoption, undo, migration), applied by
ReplayDoc::advance interleaved by timestamp; strip_restore restores
the scrubbed moment's seam; pre-Scraps journals reconstruct against
their own era.

## Seam mechanics

1. Spanning selections: ACCEPTED — copy joins the two fragments with
   the seam's separator stripped; delete/type-over = one transaction,
   two edits, replacement lands manuscript-side; cut = copy+delete;
   region verbs never offered on spanning selections, formatting is.
2. Capture inside the pile: ACCEPTED — the graveyard's capture law
   applies below the seam too (one gradient; the writer's
   "undo-plus-plus" wants rescue everywhere). Seam-spanning deletions
   file up to two region-honest entries in the one undo atom,
   threshold evaluated per side (with graveyard-interplay 6).
3. Caret never rests on the seam: ACCEPTED (zero-width for caret
   motion; click maps by y; insertion impossible by construction).
4. Park is lossless in one atom: ACCEPTED — spans+kinds captured and
   re-stamped like cut_to_graveyard/put_back; margin notes re-anchor
   and travel INSIDE the atom (no second orphan-migration atom);
   Move to the manuscript carries them home the same way.
5. Atom contents: ACCEPTED — card retirement is a notes mutation
   inside the open park transaction; caret returns to `s` (flip
   corrected); no-selection jot takes one adjoining newline.
6. Seam evaporation: ACCEPTED with the RETYPE-RACE GUARD — textless
   = empty; blank leftovers + boundary removed in the same
   transaction; but while the caret is inside the region the seam
   stays (count reads 0) and evaporates when the caret leaves.
   Reason: immediate evaporation makes the writer's next keystroke
   silently land in the manuscript — a scope trespass by her own
   hands.
7. Provenance is a RANGE-ANCHORED SIDE RECORD (annotation anchoring
   reused), never item metadata: ACCEPTED — merge/split of scraps
   then needs no rule at all; each record follows its own text; the
   one-liner shown is the record containing the resting caret; a
   deleted fragment's record dies like a note anchor; jots create
   none.
8. Select-all: ACCEPTED — region-scoped by caret; a second ctrl-A
   widens to the whole document (P7-lawful). RECORDED as a named
   exception to the one-sentence scope law: ctrl-A+ctrl-C is a
   de-facto export path — an audience surface wearing a hand's glove.
9. put_back clamps into the region containing origin_pos; slop band
   retuned; ctrl-End lands at the last scrap's end: ACCEPTED.
10. Formatting spans the seam (one transaction, kinds never stamp the
    seam line); seam row keys the layout cache on the region count;
    flank below the seam swaps Set aside → Move to the manuscript;
    writer note cards below the seam are lawful: ACCEPTED.

## Time & persistence

1. Cross-era restore NORMALIZES (membership-preserving flip; the
   Restored checkpoint materializes tail-era): ACCEPTED. No permanent
   top-geometry editing mode. The past's own geometry is seen in
   preview/parked mode, which draws the state's own seam read-only
   (case 5 ACCEPTED; diff takes the newer side's seam).
2. Migration transaction: ACCEPTED all four — annotation ranges and
   graveyard origin_pos remapped arithmetically (never through
   apply_op's clamp); persisted undo/redo stacks dropped (precedented)
   so ctrl-Z cannot reinstate top geometry; journal paused around a
   Seam/Migration event; Before-migration / Migrated checkpoint pair;
   inverse = Restore of Before-migration (idempotent under
   normalization).
3. Seals/labels: ACCEPTED — nothing new to build. The spec's stranded
   "jot in transit can never land in a checkpoint" sentence is struck
   (capture-line era); the post-veto amendment governs.
4. Length-mismatch fallbacks re-apply the old boundary through the
   clamp instead of discarding it: ACCEPTED (degrade loudly, never
   into silent scope trespass).

## Scopes & search

1. Excursion latch state machine: ACCEPTED as written (Unlatched |
   Latched{home_caret, home_scroll} + session pile_end; set by chip /
   ctrl-shift-o / any omnibar-find-@ jump landing below the seam;
   cleared by Esc-home or the caret entering the manuscript by the
   writer's own act; clicks/edits INSIDE the latched tail never clear
   it — the find→cut→Esc→paste dance stays whole; home captured at
   travel time; first chip press of a session lands at the seam,
   later presses at pile_end).
2. Single Replace works everywhere (a watched, caret-precise edit is
   the writer's hands); Replace All sweeps manuscript-only: ACCEPTED,
   with the announced grammar ("replaced 7 in the piece · 2 in scraps
   untouched"), the All button wearing its scope, honest degradation
   at the caps.
3. Retirement is its own terminal, excluded from suppression;
   writer dismissals die with the park too (machine artifacts never
   travel with writer text — accept one possible re-nag); partial
   park shrinks the card to the manuscript remnant: ACCEPTED.
4. Geometry-flip checklist (count/export/AI rebase/select-all/Esc/
   caret gate/cold read all consume manuscript_slice; auto-cut gate
   re-reads as "not the graveyard slab"; export asset sidecar uses
   the slice — fixes a live leak): ACCEPTED.
5. Count chip: in scraps shows "scraps · N" alone (goal delta hidden,
   not recomputed); "piece ·" prefix exists only once a seam does;
   chip-travel places the caret at the seam: ACCEPTED.
6. Strip counts rebase against each state's OWN boundary; find inside
   a parked preview scopes to the past state's seam: ACCEPTED.
7. @-headings in scraps appear in document order with a muted
   "· scraps" suffix as data; an @-jump below the seam sets the
   latch; SaveCopyAs stays whole-document: ACCEPTED.

## Surfaces & attention

1. Scraps chip hides iff [seam_top, grave_header_top) intersects the
   viewport; graveyard chip keeps its shipped one-sided gate:
   ACCEPTED.
2. Graveyard chip keeps its FULL shipped contract as a pill —
   "Graveyard · N" + mark, 420ms exile blink, transient put-back
   quick-verb, same hide gates; chips ordered Scraps → Graveyard (the
   descent). ACCEPTED; the mockup's count-less chip is corrected (a
   named mockup divergence, now fixed in the mockup).
3. The receipt rides the writer-initiated exit-fade channel (never
   update_lane_motion's burst snap): ACCEPTED.
4. Two-station receipt: origin ghost fades ~150ms with a short
   downward drift (instant commit beneath); destination = chip pulse
   when the seam is off-screen XOR landed-block flash when on-screen;
   reduce_motion drops the drift and swaps pulse for the 420ms blink:
   ACCEPTED (true-slide rejected — two grammars for one verb).
5. Provenance one-liner is a packer citizen that only fades (never
   slides), displaced cards snap, one-caret-blink rest delay before
   showing, instant hide: ACCEPTED.
6. Narrow widths: the caret-block's provenance rides the narrow-notes
   drawer; Put back is additionally a palette verb: ACCEPTED.
7. SCRAP_WASH = 0xFAF7EE, painted over the measure + ~14px bleed,
   never viewport-wide; pile face = full ink at 0.8 size (today's
   muted=true dies); chroma-not-value distance from note cream
   flagged for the taste round: ACCEPTED.
8. P11: the seam is the tail's one anchor and wins the contrast
   budget; the graveyard header subordinates: ACCEPTED.
9. Seam and wash ride column_frame at every width: ACCEPTED.
10. Token audit A1–A9: ACCEPTED in full — COMPOST_TAIL deleted;
    ARRIVAL_FLASH unified (bar flash + region flash + chip pulse, one
    warm); destructive-hover off STALE_BG; **Put back wears one dress
    everywhere: muted ink, dotted underline (the whisper verb — the
    record's AI_ACCENT styling dies)**; strip.rs SAGE re-declaration
    removed; NOTE_CARD_BG-as-hover replaced by the documented hover
    overlay; the ordered-axis sentence (warm=living, cool=machine-
    live, drained=receding-from-life, red=error) written into
    color-language.md.

## Graveyard interplay

1. One seam-aware region function shared by capture / exile /
   put_back / show-origin; review #62's invariant re-pointed to
   "Put back never crosses the seam"; region unit tests: ACCEPTED.
2. GraveEntry gains a serde-defaulted region field; "from scraps ·
   date" whispers; origin quotes never cross the seam: ACCEPTED.
3. Put back of a scrap-origin entry after the seam evaporated
   RE-BIRTHS the seam and lands as the sole scrap: ACCEPTED (the
   manuscript-tail fallback silently converts dead scrap text into
   counted, exported prose — a worse lie).
4. Exiling/deleting the last scrap collapses the boundary inside the
   same transaction (undo restores text + seam together): ACCEPTED.
5. One capture law on both sides of the seam; the spec's gradient
   sentence is REWORDED (see 08 §2 edit): "substantial deletions file
   to the record under the manuscript's own capture law; Exile files
   any size deliberately; small deletions are history-only,
   everywhere." Pile-local zero threshold REJECTED — it floods the
   record during ordinary scrap rewriting; the graveyard is
   insurance, not a ledger.
6. Origin-parked-meanwhile: ACCEPTED accept-and-record — the whisper
   is a frozen past-tense fact; no content-following chain migration.
7. Chip choreography mechanics (bar → chip pair, seam_top recorded,
   per-section gates, destination-honest pulses, put-back quick-verb
   falls back to the section-header tint when hidden): ACCEPTED.
8. Asset GC: Graveyard::asset_refs() joins the reachable set;
   History::asset_refs extends to persisted undo/redo Graveyard
   elements: ACCEPTED (pre-existing hole, now load-bearing).

## Rulings beyond the corner files

- **The left rail dies** (07 echo, carried): render_rail, the
  titlebar toggle + presence dot, first-birth auto-open — all
  deleted. The "Compost Rail" command becomes **"Scraps"** — the
  travel verb (ctrl-shift-o), aliases keep compost/asides/компост
  for findability.
- **Naming sweep**: UI strings say Scraps / the scrap line / Set
  aside / Put back / Move to the manuscript. Code identifiers may
  keep `compost_`/`aside_` names where renaming is pure churn; new
  code says scraps. Docs keep "compost" as craft vocabulary only.
- **Build order**: Wave A = model/persistence/migration (this file's
  foundation + seam mechanics + time & persistence + graveyard
  entries/GC). Wave B = surfaces (seam row, wash, chips, receipts,
  latch, verbs, provenance UI, scopes announcements, token audit,
  rail deletion, naming, rig + mockup-fidelity evidence).
