# Corner cases — the seam and the pile as text mechanics

Domain: the boundary node's every text interaction; the blank-line item
model; the pile/graveyard hard edge. Against `docs/impl/08-compost-fresh.md`
(amended), `design-tail.md` §3, `asides.md`, P1–P13, and the code at
`e535933`. Ground truth reminder: today the boundary is an out-of-band
block index (`BlockMap::aside_boundary`, `document.rs:99-115`) marking a
real blank separator line at the TOP; the build flips it to the tail.

## 1. BLOCKER — the boundary index cannot say which side is scraps

`aside_boundary: Option<usize>` is persisted bare in three places: the
Loro blocks map (`store.rs:176-181`), every materialized
`CheckpointState { text, spans, blocks }` (`store.rs:74-76, 547`), and
every undo/redo snapshot incl. cross-session `History`
(`document.rs:922-929, 1512-1518`). Its *meaning* is about to invert:
today "blocks `0..b` are scraps", after the flip "blocks `b+1..` are
scraps". Nothing records which reading a given state uses.

**User-visible failure:** restore a pre-flip checkpoint
(`restore_state`, `document.rs:1429-1471` installs the old `BlockMap`
verbatim), or ctrl-Z across the one-transaction migration, or reopen a
pre-flip file's persisted undo stack — and the regions invert silently:
the manuscript becomes the uncounted/unexported region and the old
compost becomes the piece. The next save persists the loss. This is the
wave-1 "restoring any version silently merged the compost into the
manuscript" bug (comment at `store.rs:171-175`), resurrected at higher
stakes.

**Spec clause:** 08 §2 "past states keep their own geometry… Restore
appends a state with *its* seam"; migration: "historical checkpoints
keep their own era and restoring one never teleports text (07 N3)".
As written this is *impossible* with the current representation.

**Proposed resolution:** an era tag serialized beside the index — a
`BlockMap` field with serde default = `Top` (so all existing files,
checkpoints, and history states decode as old-era), a new key in the
Loro blocks map, riding `CheckpointState` for free. Region math is
already centralized (`manuscript_base_char` / `manuscript_char_range` /
`manuscript_slice`, `document.rs:967-1016`; `caret_region_bytes`,
`editor.rs:5953-5965`): branch there. Restoring or undoing into a
Top-era state renders the region at the top (in-flow face only; the
rail panel stays deleted) — never converts; conversion is exclusively
the writer's one recorded migration transaction. Decide before build:
every other case below sits on this representation.

## 2. DECIDE — legalize spanning selections and define the four verbs

The spec demands "selections span it but never capture it"; the code
*forbids* spanning by construction: `clamp_to_region`
(`editor.rs:5967-5984`) clamps keyboard and drag, and at least five
sites assume single-region input — `auto_cut_qualifies`
(`editor.rs:1396-1408`, trigger at 8306-8325), `set_aside`'s refusal
(`document.rs:1146-1151`), `send_to_graveyard` (`editor.rs:5622-5632`),
`flank_gate` (`editor.rs:833-853`), `caret_region_bytes`. Deleting the
clamp without defining the verbs hands the semantics to
`adjust_boundary`'s never-panic fallback (`document.rs:219-241`), which
silently reclassifies remnants when a delete merges through the
separator line.

**Proposed resolution.**
- **Copy:** clipboard = above-fragment + `"\n"` + below-fragment. The
  seam's own blank separator line is stripped (the seam never enters
  the clipboard, not even as its whitespace shadow); the pile's
  item-separating blank lines inside the selection stay — they are
  honest text (asides §1: a clipping with a blank line arrives as two
  items).
- **Delete / type-over:** one transaction, two edits — above and below
  the separator, the separator line untouched, "the seam between the
  remnants" (design-tail §3). Replacement text lands on the manuscript
  side (caret = selection start). If the below-edit empties the region,
  evaporation rides the same atom (case 7).
- **Cut** = copy + delete. Graveyard filing per case 3.
- **Region verbs** (Set aside / Move to the manuscript / Exile) are not
  offered on a spanning selection — they are region verbs; formatting
  still rises (case 11).

Severity *decide*: the spec pins span-not-capture and remnant-seam, but
the clipboard payload shape and the verb gating each have a second
defensible form (keep the blank line as honest text; offer Set aside on
the manuscript fragment only).

## 3. DECIDE — graveyard capture inside the pile; how a spanning deletion files

Today the pile is exempt: `file_cut` refuses compost selections
("compost selection — not a cut", `editor.rs:5619-5632`);
`auto_cut_qualifies` requires `start_char >= manuscript_base`. But 08
says "Exile works (a deleted scrap falls one level, into the record)"
and design-tail assumption 8 + scorecard F6 extend *substantial-
deletion capture* to wherever the writer deletes, including Scraps —
one downward gradient, and the trust promise ("cuts survive") should
not stop at the seam.

**Proposed resolution:** auto-capture applies inside the pile; a
seam-spanning deletion files **two entries**, one per region fragment,
each with its own `origin_pos` — a single merged entry would make Put
back (`document.rs:1064-1087`, one insertion point) teleport scrap text
into the manuscript. The counter-position (the pile stays a private box;
deleting a jot shouldn't corpse) is defensible and asides §0's
move-not-departure sentence was written under the old law — the
maintainer adjudicates. Unhandled failure: a writer who prunes her pile
finds either no insurance record at all, or a Put back that resurrects
scraps as prose.

## 4. NOTE — the caret must never rest on the seam line

Deletion at the three boundary edges is guarded (`at_manuscript_start`
/ `at_compost_tail` / `at_separator_start`, `editor.rs:5989-6015`;
guards at 5483-5521) but *motion is not*: arrows and clicks land the
caret ON the separator line, and typing there inserts into the seam's
own line — text belonging to the wrong region with no visible change
(the trap the `editor.rs:6003-6008` comment documents for
forward-delete only; extraction papercut). After the flip the seam row
renders as hairline + label + live count: a caret blinking inside that
ornament fails P6/P12, and words typed "into the seam" vanish from the
count.

**Resolution (spec pins it):** "arrows across it like a paragraph
break" — the seam line is zero-width for caret purposes. Vertical and
horizontal motion skip it; click maps by y to manuscript-end or
first-scrap-start; insertion at the seam is impossible by construction;
the two edge-deletion guards stay no-ops.

## 5. NOTE — park must move spans, kinds, and notes; today it strips them

`Document::set_aside` moves *plain text*: `rope.slice(s..e).to_string()`
(`document.rs:1153`), and `insert_into_compost` resets every inserted
block to `Paragraph` and re-adds no spans (`document.rs:1126-1130`) —
while `cut_to_graveyard`/`put_back` carefully capture and re-stamp both
(`document.rs:1035-1049, 1071-1083`). Park a bold heading → it arrives
a plain paragraph; Move to the manuscript returns stripped text —
"departs losslessly" (08) and P13's verbatim inverse both break.
Also: notes on the parked range are today handled *outside* the park
transaction — diagnoses dismissed after the fact, writer notes migrated
as orphans in a **second** undo atom (`reconcile_dead_anchors` →
`migrate_note_to_compost`, `editor.rs:5612-5616, 5880-5928`,
`document.rs:1187-1191`). Under the amended gating law notes must
instead **re-anchor and travel with their block**, and card retirement
must ride the park atom (case 6). Build care; the spec is unambiguous.

## 6. NOTE — the park/jot undo atom: exact contents

The atom largely exists: `set_aside` is one transaction — snapshot +
grouped delete + grouped insert, the boundary move riding the `BlockMap`
snapshot (`document.rs:1146-1166`), so undo already evaporates a
just-born seam (`document.rs:1382-1404`). The build must keep it one
atom while adding: (a) span/kind transfer and note re-anchoring (case
5); (b) diagnosis-card retirement as a notes mutation *inside* the open
transaction — notes are in the snapshot tuple, so ctrl-Z resurrects the
retired card for free (08: "parking a diagnosed passage retires its
card"); (c) the caret return value flips: `s + payload_len`
(`document.rs:1165`) is correct only while the pile is above the caret —
after the flip the caret stays at `s`; (d) the jot form (chord, no
selection) takes the caret's paragraph *plus one adjoining newline*, so
no empty block strands at the join and undo restores the Enter-typed
shape verbatim.

## 7. DECIDE — seam evaporation: structural empty vs textless empty

Today the boundary dissolves only when its blocks are structurally
removed (`adjust_boundary` → `None` at 0, `document.rs:236-240`).
Deleting all the pile's *text* leaves empty blocks and a standing seam
reading "Scraps · 0" — and since backspace at the region edge is a
guarded no-op, **no editing gesture can ever remove it**: the seam the
writer cannot evaporate reads as the tool disputing her page (08 §5's
named risk verbatim).

**Proposed resolution:** textless = empty. When a deletion leaves the
region with no characters, the same transaction removes the leftover
blank blocks and the boundary (undo restores both, P13). Guard the
retype race: if the caret is *inside* the region at that moment, keep
the seam (count honestly reads 0) and evaporate when the caret leaves —
otherwise deleting a scrap's text to retype it lands the next keystroke
in the manuscript. The simpler immediate-evaporation rule is defensible;
adjudicate.

## 8. DECIDE — two provenances merge when the separating blank line dies

(The corner 08 explicitly kicks to this pass.) **Proposed resolution:
provenance is a range-anchored side record** — reuse the `Annotation`
anchoring machinery the design already names — never item metadata.
Then the merge needs no rule: both records survive, each pinned to its
own text inside the merged scrap; the margin one-liner shown is the one
whose range contains the resting caret (two caret positions show two
honest origins); Put back returns its own anchored range's current
text; deleting a fragment kills its provenance the way note anchors
die; a typed blank line *splitting* a parked scrap needs no rule either
— each half keeps the provenance its text carries. Identity stays
geometry (asides §5); one anchor grammar everywhere (P8). Unhandled
failure: a merged scrap shows the wrong origin, or Put back returns a
jot it never parked.

## 9. DECIDE — select-all under the one-sentence scope law

`select_all` is region-scoped to the caret (`editor.rs:6697-6703`,
review #110). The amended law reads the other way: select-all is the
writer's hands, so it should cross. But ctrl-A + ctrl-C is how a piece
leaves for another program — an audience surface in disguise; crossing
would leak the pile into every whole-document copy, the exact failure
class the seam exists to kill. **Proposed resolution: keep select-all
region-scoped** (a bulk scope, like Replace All); optionally let a
second ctrl-A widen to the whole document (precedented widget
behavior, P7-lawful). The law-purity reading is defensible; adjudicate
and record the exception, or the one-sentence law erodes per-verb
exactly as the tail critique warned.

## 10. NOTE — the pile/graveyard hard edge

Graveyard clicks already never become carets (`editor.rs:7325-7348`,
`grave_section_top` guard). After the flip: (a) `put_back`'s clamp must
invert — `origin_pos.clamp(base, len)` (`document.rs:1064-1068`,
review #62) becomes clamp-*below*-the-seam, or every corpse whose
origin sits past the seam char (common: cuts near the piece's end)
resurrects INTO the pile; (b) ctrl-End and click-below-last-line land
at the last scrap's end, not the record — correct, worth a rig
assertion; (c) the 22px slop band above the section header
(`editor.rs:7335`) now steals clicks from the last scrap's final line —
retune, editable text abuts the slab directly.

## 11. NOTE — formatting across the seam; the seam under layout

Formatting spanning the seam is lawful (writer's hands): spans are
seam-agnostic char ranges and `toggle_format` already works over any
range in one transaction (`document.rs:1291-1302`); block-kind commands
stamp both sides but must skip the seam line itself (it renders as the
seam regardless — no kind may make it "a tiny grey heading", the
extraction bug). Layout: the seam is a block row in the one text column
and reflows with the measure; its live word count must invalidate with
pile edits that change no line count (the prepaint layout-reuse cache
keys on revision — the seam row must also key on the region count; cf.
`compost_header`, `editor.rs:1245-1254, 9453-9462`). The cool lane ends
at the seam via the existing manuscript-slice rebase
(`editor.rs:3478-3499`) — after the flip that rebase becomes a
clamp-below-seam; `flank_gate`'s blanket right-menu suppression
(`editor.rs:846-851`) flips to offering Move to the manuscript, and
writer note cards below the seam become lawful lane citizens. Failure
if missed: a stale seam count, or a cool card anchored below the seam —
the lane visibly failing to end.
