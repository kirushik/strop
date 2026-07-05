# Corner cases — graveyard interplay (Scraps build)

Domain: how the living pile and the dead record touch. Canon framing
(08 §6): Scraps = deliberate, counted, visible, living; graveyard =
almost-by-accident, "undo-plus-plus." Code read at branch
`golden-path-impl`; all anchors reflect the OLD geometry (compost at
top) per extraction-summary.md.

## 1. Every graveyard region test encodes "manuscript = after the boundary"; the flip inverts them all silently — including Put back's clamp

**note** · P13; 08 §2 "The boundary"; review #62's intent

The capture predicate `auto_cut_qualifies` (editor.rs:1401–1408)
gates on `start_char >= manuscript_base`; `send_to_graveyard`
refuses `start_char < manuscript_base_char()` (editor.rs:5627);
`Document::put_back` clamps the landing to
`origin_pos.clamp(base, len)` where `base = manuscript_base_char()`
(document.rs:1064–1068, review #62: "cut prose can never resurrect
INTO the compost"); `show_grave_origin` clamps the same way
(editor.rs:5765–5766). After the flip, `manuscript_base_char()`
semantics invert (manuscript starts at 0; the pile is after the seam).
If any site survives untranslated: auto-capture fires inside the pile
"as manuscript," and — the user-visible failure — **a scrap exiled
from the pile Puts back into the manuscript**: text teleports across
the seam on a round trip, the exact promise the seam exists to keep.

**Resolution:** one region function (`region_of(char) -> Manuscript |
Scraps`, seam-aware), used by capture, the exile verb, Put back, and
show-origin. Review #62's invariant is re-pointed, not deleted: *Put
back never crosses the seam — it returns to the region the text left*
(region-preserving clamp, both directions). The unit test at
editor.rs:16010 flips meaning with the geometry and must be rewritten
against regions, not offsets.

## 2. `GraveEntry` must record its origin region, or "from scraps" honesty is impossible

**note** · 08 §6 (the framing is canon); P8 (feedback never lies); asides.md §0

Capture inside the pile is standing law (design-tail §3 assumption 8,
unamended by 08; Exile below the seam is explicit in 08 §2). But the
entry records only `origin_quote` + `origin_pos`
(document.rs:650–677). Two dishonesty holes: (a)
`origin_quote_before` (editor.rs:5657–5680) takes the trailing
fragment of whatever paragraph precedes — for a cut at the pile top it
steps back over the newline into the seam's own line (empty quote) or
the manuscript's last paragraph; the whisper ("cut from after …",
editor.rs:8998, 9064) then names the wrong region. (b) Region cannot
be derived from `origin_pos` later: when the pile empties and the seam
evaporates, `Graveyard::apply_op` (document.rs:747–764) pins the
position to the collapse point — indistinguishable from a
manuscript-end origin.

**Resolution:** add `region` to `GraveEntry` with a serde default of
`Manuscript` (the exact backward-compat pattern `spans`/`kinds`
already use, document.rs:670–676). The whisper for a scrap-origin
entry reads "from scraps · ⟨date⟩" (or quotes the preceding *scrap*;
the quote never crosses the seam — one law with counts/export). Put
back and show-origin consult the recorded region (case 1's clamp).

## 3. Put back of a scrap-origin entry after the pile has emptied: re-birth the seam

**decide** · P13 (same door back); 08 §2 "evaporates when the region empties … the first park's inverse"

The seam is gone; the entry's recorded region is Scraps; where does
Put back land? Two defensible answers. (a) **Re-birth the seam** and
land as the sole scrap: it left through the pile's door, it returns
through it; the machinery exists — `insert_into_compost` births the
boundary when absent (document.rs:1114–1136), and seam birth/death
already rides the undo snapshot because `BlockMap` owns
`aside_boundary`. (b) Fall back to the manuscript tail, arriving
selected: the writer deliberately dissolved the pile; resurrecting a
region she emptied is the tool re-imposing structure (P2-flavored).

**Proposed:** (a). "Emptying dissolves it" gets its inverse
"returning re-creates it" — the exact mirror of undo-of-first-park
evaporating a just-born seam (08 §2 Undo). (b) silently converts dead
scrap text into manuscript prose — it changes what the word counts
and the export contain, which is a worse lie than an unasked-for
seam. Unhandled failure: Put back either no-ops or dumps a scrap into
the piece with no announcement.

## 4. Exiling the last scrap must collapse the seam in the same undo atom

**note** · 08 §2 "Empty state: nothing"; P13; P6

`cut_to_graveyard` (document.rs:1025–1056) deletes text and files the
entry but never touches `aside_boundary`. Exile of the whole pile (or
of the only scrap) leaves an empty region under a standing seam — a
labelled hairline counting nothing, which 08 forbids ("a document
with no scraps is pixel-identical to today"). And if the build clears
the boundary as a *separate* step, ctrl-Z restores the seam without
the text (or vice versa) — a P6-failing intermediate state that never
logically existed.

**Resolution:** the region-emptying cut clears the boundary inside
the transaction `edit_bytes` opened (the same ride-the-open-
transaction pattern `put_back` uses for kind re-stamping,
document.rs:1071–1079). Undo then restores text + seam together for
free, because the pre-cut snapshot's `BlockMap` carries the boundary.
Same law for ordinary deletions that empty the region (backspacing
the last scrap away).

## 5. "Deleting a scrap falls one level into the record" is only true above 80 chars — honest wording, or a pile-local mechanism?

**decide** · P4 (no folklore claims); 08 §6 framing; H24

`AUTO_CUT_MIN_CHARS = 80` (editor.rs:240); only a single
selection-deletion qualifies — backspace runs never file
(editor.rs:8306–8312). The modal scrap is a short jot: a 60-char
scrap deleted by selection falls **two** levels (history only), and
any scrap backspaced away files nothing. The F6 scorecard claim is
false as commonly read. 08 §2 itself scopes the sentence to Exile
("Exile works (a deleted scrap falls one level, into the record)") —
Exile files any size (editor.rs:5618–5632).

Options: (a) **honest wording** — the gradient claim is documented
as: *substantial deletions file to the record under the manuscript's
own capture law, one level down; Exile files any size deliberately;
small deletions are history-only, everywhere.* (b) **mechanism** —
zero/lower the threshold inside the pile so deleting a whole scrap
always files. (b) floods the record: scraps are living text under
active rewriting; every sentence-sized selection-delete during scrap
revision would file a corpse, and the graveyard's canon register
("almost-by-accident insurance") tolerates a missed small cut far
better than record noise. **Proposed:** (a) — one capture law both
sides of the seam; fix the words, not the threshold. The maintainer
should adjudicate because (b) has a real constituency (a scrap is a
*counted* object; its silent death is more surprising than a
sentence's).

## 6. A deletion spanning the seam: what does capture file?

**decide** · 08 §2 (selections span the seam, never capture it); P8

New spec: selections may span the seam; deleting a spanning selection
leaves the seam between the remnants (design-tail §3) — mechanically
two deletions. Today this is unreachable (`clamp_to_region`,
editor.rs:5971–5984, review B4); the flip removes that clamp, so the
capture path meets a case it has never seen. One entry stitching text
from both regions is dishonest: the text was never contiguous (the
seam stood between), and its Put back must land in *some* region —
either answer moves words across the seam.

Options: (a) **per-side evaluation** — split at the seam; each side
that clears the threshold files its own region-honest entry; all of
it one undo atom (both filings ride the one transaction, exactly how
`edit_bytes` + `file` already compose in `cut_to_graveyard`). (b)
**manuscript-side only** — insurance exists for the piece; the scrap
side falls under case 5's law. **Proposed:** (a); it is the only
answer where every filed entry survives cases 1–2's honesty rules
without a special case. Unhandled failure: a 500-char cross-seam
delete either files nothing (manuscript prose lost uninsured) or
files a chimera entry whose Put back invents a region.

## 7. Put back when the origin paragraph was itself parked meanwhile: the pin stands

**note** · Graveyard anchors are positions, not content; P13

A park is delete-at-origin + insert-under-the-seam.
`Graveyard::apply_op` (document.rs:747–764) pins `origin_pos` of any
entry inside the deleted range to the deletion start; the pile-side
insert (at/after the seam, beyond the pin) never re-shifts it. So the
dead text Puts back at the manuscript spot its context vacated, and
the whisper still quotes the departed paragraph.

**Resolution:** accept and *record* this — the whisper is a frozen
historical fact ("cut from after …" is past tense; honest), and the
entry's contract is "return to the piece at the place of the cut,"
not "follow my old neighbours." Chaining grave anchors through park
moves (content-following migration) buys real machinery for a rare
double-afterlife chain the writer repairs with one move. Name it in
the build notes so nobody "fixes" it into teleporting dead text into
the pile — which would violate case 1's region-preserving clamp.

## 8. Chip choreography: the graveyard "chip" is a full-width bar; two rhymed chips need per-section gates and destination-honest pulses

**note** · 08 §2 "Chips — two, rhymed"; P7; P12; named-rejected descending chip

`render_graveyard_bar` (editor.rs:11469–11527) is an absolute
`left_0().right_0()` footer **bar**, hidden by
`grave_tail_on_screen` (editor.rs:5717–5722) reading
`frame.grave_section_top` (recorded editor.rs:9721–9731). What moves:
(a) the bar becomes one of two chips sharing a footer row, gradient
order (Scraps, then graveyard); (b) the frame records a symmetric
`seam_top` — and the Scraps chip's hide gate must be
**region-body-based**, not header-based: a mid-pile viewport (seam
scrolled off above) is *inside* the section; showing a "Scraps · N"
travel chip there invites travel to where you already are. The
graveyard chip keeps its header-based gate. (c) Pulses stay
destination-honest: park pulses only the Scraps chip (`rail_flash`
successor, editor.rs:5607), exile — from either region — blinks only
the graveyard chip (`grave_flash`, editor.rs:5645); this is the
recorded reason the descending chip died. (d) The transient "put
back" quick-verb (editor.rs:11509–11525) stays on the graveyard chip;
when the record's header is on-screen and the chip hidden, the exile
blink lands on the section (the existing `grave_flashing` tint,
editor.rs:9984) — never on the Scraps chip. The grave section's
*position* needs no work: it is shaped after the last rope block
(editor.rs:9725–9732), so it lands below the pile automatically once
the pile is rope tail; only the `manuscript_bottom` name lies.

## 9. GC can reap an image asset whose only reference is a graveyard entry — Put back restores a broken image

**note** · P13 ("nothing destroys silently"); the June GC-gate work

`GraveEntry.kinds` can hold `BlockKind::Image { src, .. }`
(captured at cut, document.rs:1042–1049; re-stamped by put_back,
document.rs:1076–1079). But `collect_unreachable_assets`
(store.rs:898–936) builds its reachable set from live
`blocks.asset_refs()` + `History::asset_refs()` — which scans only
the BlockMap element of each undo state, ignoring the tuple's
Graveyard (document.rs:1520–1527) — + checkpoint blocks. Scenario:
insert an image, exile its passage before any checkpoint spans it,
keep writing until the undo stack rotates; the next save GC's the
asset; Put back later restores an Image block with a dangling `src`.
Pre-existing hole, but in-pile Exile (this build) adds a new cut
source. The pile itself is safe by construction — scraps are rope
blocks, already in `blocks.asset_refs()`.

**Resolution:** chain graveyard refs into reachability: a
`Graveyard::asset_refs()` (over entries' `kinds`) added to the live
set, and `History::asset_refs` extended to the Graveyard element of
each persisted undo/redo state (undo of a `grave_delete` can
resurrect an entry). The cheap all-referenced gate (store.rs:917)
keeps working unchanged.
