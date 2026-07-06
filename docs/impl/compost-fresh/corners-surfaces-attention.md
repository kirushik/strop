# Scraps corner cases — surfaces & attention

Domain: chips, receipts, motion, the wash, provenance-in-margin, P11, widths,
plus the ordered living-vs-dead token audit. Spec = `docs/impl/08-compost-fresh.md`
(amended). Code @ e535933 reflects the OLD compost-at-top geometry throughout.
No blockers found; the spec is buildable as written.

## 1. "Own section on screen" needs a two-sided test for Scraps — DECIDE

The shipped graveyard test is one-sided: `grave_tail_on_screen`
(editor.rs:5717-5722) is `section_top − scroll < viewport_height`, i.e.
"reached-or-passed". Correct for the LAST section (you can't scroll below it);
wrong for a middle one. Copied verbatim to the Scraps chip, a writer deep in a
long graveyard record (pile entirely ABOVE the viewport) still gets no Scraps
chip — no one-tap way back to the pile, only manual scrolling. Two defensible
definitions: (a) region-intersection — `[seam_top, grave_header_top)` overlaps
the viewport (matches mockup scene 3's "own section on screen → hidden" and
scene 1's "both on screen → both hidden"); (b) seam-row-visible — the chip
hides only while its destination station is in frame (P12 reading), which
would show the chip mid-pile. **Propose (a)** for Scraps; the graveyard keeps
its shipped one-sided formula unchanged (it stays the last section after the
flip). Both tests read `last_frame` (one-frame lag) — shipped idiom, keep.

## 2. The graveyard chip's contract under the reshape — DECIDE

Shipped contract (render_graveyard_bar, editor.rs:11469-11528): exists iff
`len > 0` and no history view; hides via grave_tail_on_screen; click scrolls
to the record; blinks 420 ms on exile (grave_flash, schedule_flash_clear
5931); carries the transient "put back" quick-verb during the flash window
(11509-11525); reads **"Graveyard · N"** + tombstone. The mockup
(scraps-tail-2026-07.html:161) reshapes the full-width band into two centered
pills and shows the graveyard chip **bare — no count**. Dropping N regresses
blink-AND-TICK: the exile receipt is precisely the count incrementing while
the bar blinks (07 N4); a bare chip names the destination but never confirms
the filing, and P12 loses the one datum the control carried. **Propose:** the
pill pair adopts the band's full contract — "Graveyard · N" + mark, blink,
transient put-back (the pill grows a segment during the flash window), same
z-slot and hide-gates (history_view; the strip overlay covers it). Scraps
chip rhymes: "Scraps · N", warm, same existence gating (region non-empty),
pulsing on arrivals. Chip order left→right = the gradient's descent: Scraps,
then Graveyard. Both fit at DOC_MIN_WIDTH 400.

## 3. Every park is "mid-burst" to the code — the receipt must not ride the lane's snap gate — NOTE

The one writer/machine discriminator lives at the *delivery* layer:
deliver_pass (editor.rs:3561-3573) parks machine results behind
`typing_burst_live()` (3454: any buffer edit < TYPING_LULL 1000 ms). The
*motion* layer is cause-agnostic: update_lane_motion snaps ALL motion when
`typing_burst_live()` (13416-13421). And `set_aside` → `sync_mutations`
stamps `last_text_edit` on every doc mutation (1626-1633) — so the park
itself opens a "burst". Consequence: if the departure receipt or the
card-retire animation reuses the lane's snap gate, **it never plays** — a
park is always ≤ 0 ms from a buffer edit. The spec already decides this
("writer-initiated motion is lawful mid-burst; the reveal clock governs
machine deliveries only") — the finding is that no code path expresses it.
Build: writer verbs get their own receipt channel, exempt from the burst
snap. Precedent exists: the exit-fade ghosts (`departing`,
editor.rs:979/3148-3170) are writer-initiated and already play regardless of
burst — model the receipt on them, and route "parking a diagnosed passage
retires its card" through the same `departing` fade (also burst-immune).
Failure if missed: text vanishes with zero motion right after typing —
Norman's minute-3 "did I delete it?", the exact kill the receipt exists for.

## 4. What "departs toward the foot" is when the foot is off-screen — DECIDE

The re-pack slide grammar (CARD_MOVE 200 ms, MOVE_STAGGER, editor.rs:114-119)
exists only in the margin lane; the prose plane has no slide machinery
(paragraphs teleport on every edit today). The park's destination (under the
seam, at the tail) is normally thousands of pixels off-screen — a literal
travel is impossible in the lane machinery and would be a viewport-crossing
attention spike anyway (attention-motion.md: animate moves, small fields).
**Propose the complementary two-station receipt, all shipped idioms:** at the
origin, a viewport-frozen ghost of the departed block fades ~150 ms with a
short downward drift (the `departing` snapshot idiom + a motion vector that
says *down*; commit is instant underneath — P6 holds, every frame shows
closed-up prose plus a fading echo); at the destination, exactly one of two
receipts, and the chip-hiding law makes them mutually exclusive by
construction: seam off-screen → the Scraps chip is visible and pulses
(mockup's chippulse ring); seam on-screen → the chip is hidden and the landed
block flashes instead (the shipped region-arrival flash, COMPOST_FLASH,
editor.rs:9862-9868). Under reduce_motion: drop the drift (pure cross-fade)
and replace the pulse ring (it is travel) with the shipped 420 ms background
blink — luminance, not motion. The alternative reading — a true slide when
both stations happen to be visible — buys object constancy for a rare case at
the cost of two grammars; recommend against.

## 5. The provenance one-liner vs the packer: caret-driven lane churn — DECIDE

Writer notes are legal in scraps, so the caret-gated provenance line will
share margin y-space with real cards; it must enter place_margin_cards or
overlap them. But it is *caret-driven*, and caret moves never stamp
`last_text_edit` (1631 comment: "caret moves must never touch it") — so
arrowing block-to-block through the pile re-packs the lane outside any burst
and update_lane_motion would fire 200 ms slides on every block crossing:
animation against the writer's own continuous movement, the exact thing the
discrete-vs-continuous rule forbids. **Propose:** (a) the one-liner joins the
packer as a card-shaped citizen (so no overlap) but its appearance/departure
uses only the appear fade (appear_fade, 12936, CARD_APPEAR 250 ms) /
departing fade — never slides — and any card it displaces snaps (extend the
snap set with "lane diff caused by caret travel"); (b) a short rest delay
(~one caret-blink) before it shows, hide-on-leave instant, so traversing a
60-scrap pile doesn't strobe the margin. It is machine bookkeeping: muted
ink, no warm/cool card fill (the mockup's `.prov` — muted, dotted-underline
Put back — is right). asides.md §5's no-standing-chrome refusal is satisfied
by the caret gate; P9 untouched (keyboard, not hover).

## 6. Narrow widths orphan provenance and Put back — DECIDE

Below ~956 px content width the lane does not exist (margin_fits,
editor.rs:13512-13518: COL_MAX 660 + NOTE_LANE_TOTAL 272 + COL_LEFT_MIN 24)
— at the spec's own 800 pt there is no margin, and the right verb flank
stands down too (flank_gate, 833-851: `right: lane_available && …`). The
amended spec houses Put back *inside the expanded provenance line* — at
narrow widths that verb is unreachable anywhere: half-screen-laptop writer
parks, later wants the origin return, and nothing offers it ("Move to the
manuscript" targets the latch position, not origin). **Propose:** the
caret-block's provenance row rides the narrow-notes drawer
(render_narrow_notes_panel, the cull=false path, 14545+) exactly as cards
do, and Put back is additionally a palette verb (the shipped narrow fallback
for flank verbs). Never draw provenance inside the pile itself — chrome on
writer text, P1.

## 7. The SCRAP_WASH token — proposal — NOTE

Mockup value: `rgba(250,244,226,.45)` = NOTE_CARD_BG at 45 % over the page.
**Propose baking it solid: `SCRAP_WASH: u32 = 0xFAF7EE`** (composite over
BG_COLOR), ~15.5:1 vs TEXT_COLOR (AAA holds). Distinctness audit against
theme.rs: NOTE_CARD_BG 0xFAF4E2 — same value, roughly half the chroma, and
*form* carries the rest (borderless region wash at full measure vs bordered
rounded card; wash text is 0.8-size); SELECTION composite ≈ 0xE6D9B5 — far
darker/saturated, no confusion; DIAGNOSIS_CARD_BG 0xEEF3FB — opposite hue;
STALE_BG 0xEFEEEA — grey vs warm, and the wash abutting the drained record
is exactly the living/dead boundary doing its job. Honest flag for the taste
round (where the writer parked the warm-family revisit): the mandate says
"visibly distant in **value**" from note cream, and no page-legible wash can
be — the distance here is chroma + form. Paint the wash over the column
measure plus the mockup's ~14 px bleed, **never viewport-wide** (at 1600 pt a
full-width band would run under the margin lane and change every card's
ground). The pile face itself: full TEXT_COLOR ink at 0.8 size — the current
`bstyle.muted = true` (editor.rs:9465) must not survive the flip (§10.A1).

## 8. P11: the tail's one anchor is the seam — NOTE

Post-flip the tail stacks four quiet things: seam row, wash, graveyard
header + slab, entries. P11 wants one landing point per surface: **the seam**
— it carries the only live datum (the region count) and is the object the
whole feature is about; the graveyard header subordinates. Check the
ordering, don't assume it: today's grave header is a 13 px MUTED label + a
drawn slab (editor.rs:8954, 9993-10008) while the mockup's seam is 11 px
muted — as drawn, the *dead* section head outweighs the living boundary and
the returning eye lands on the record. Give the seam ≥ the header's type
size/contrast (e.g., label+count at 12-13 px, TEXT_COLOR-leaning) and keep
the mockup's 3 px STALE_BG slab (lower-contrast than a 1 px RULE_COLOR
hairline against this page — verified by value, it reads quieter). Wash sits
lowest. Failure: two equal section heads = mush (P11's own word).

## 9. Seam geometry at 800 pt vs 1600 pt — NOTE

The seam (label · hairline · count) spans the **measure**, i.e. column_frame
(13582), not the viewport — at 1600 pt the centered 660 px column keeps seam
and wash off the empty flanks and the lane; at 800 pt the column has shifted
left (COL_LEFT_MIN) and the lane is gone, and the seam simply rides the
column. The count stays right-aligned to the measure so the arithmetic
(chrome count + seam count) reads on one axis at any width. Chips are footer
overlays, width-independent (case 2). The only 800 pt casualty is provenance
(case 6).

## 10. The ordered app-wide living-vs-dead token audit — NOTE (deliverable)

Law: warm = living/writer's; drained = dead/record (staleness = receding
toward death, lawful); structural labels/rules are off-axis. Grep-verified:

- **A1 (violation, fixed by the flip):** the living pile renders
  `muted = true` at 0.8 size (editor.rs:9462-9465) — the writer's living
  scraps wear the dead ink today. The flip must ship full ink + SCRAP_WASH.
- **A2:** `COMPOST_TAIL` (theme.rs:150-153), the drained anchor mark on the
  living pile — dies with the rail; delete the token, don't orphan it.
- **A3 (inconsistency):** one arrival event, two warms — the graveyard bar
  flash is inline `0xEFE6D0` (11492) while the rail edge and the section
  header flash `COMPOST_FLASH` gold (9868, 9990). Unify on one token and
  rename it `ARRIVAL_FLASH` (the "compost" name dies with the renaming kit
  anyway); the Scraps chip pulse joins the same grammar.
- **A4:** STALE_BG does triple duty — machine doubt (unverified cards,
  14061), record dressing (grave left-rule 10021; mockup entry fills), and
  *destructive-hover* (10680). The first two are lawful under the ordered
  axis; drained-as-danger-hover says "stale", not "kills" — move it to a
  neutral overlay.
- **A5 (P8, one verb two dresses):** record verbs (put back / show origin /
  delete) wear AI_ACCENT cool blue (8983, 11514) — defensible (P3: the
  record is machinery the writer operates) — but the provenance line's Put
  back is muted + dotted underline in the mockup. Pick one action dress for
  the same verb on both surfaces.
- **A6:** strip.rs:97 re-declares SAGE locally = theme::SAGE_COLOR; fold.
- **A7:** NOTE_CARD_BG (warm = *mine*) spent as a generic hover fill on menu
  rows (10680) against theme.rs's own chrome-stays-inline roster; use the
  documented `rgba(0x1A1A18,0A)` hover overlay.
- **A8 (consistent — adopt):** mockup chips teach the axis: Scraps chip
  NOTE_CARD_BG warm, graveyard chip STALE_BG drained (vs today's neutral
  0xF4F1EA band). Do it.
- **A9:** put-back's returned-paragraph flash is warm SELECTION_COLOR
  (9905-9911), not sage — consistent with "retrieval arrives selected"
  (amber = the writer acting); reserve sage for answered/returned notes and
  say so in color-language.md.
- **Rule to write down** (color-language.md, one sentence): the axis is
  ordered — warm = living, cool = machine-live, drained = receding from life
  (stale → dead), red = error; labels, rules, and chrome are off-axis.
