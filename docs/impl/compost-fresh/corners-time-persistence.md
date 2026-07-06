# Scraps corner cases — time & persistence

Domain: the boundary node across checkpoint states, Restore, the parked
past, the migration transaction, legacy shapes, fingerprints/compaction,
the journal, seals. Against 08-compost-fresh.md (amended) and the code at
e535933. All anchors are current-geometry code (compost at TOP); the
build flips it.

## 1 · The persisted boundary is era-ambiguous — BLOCKER

`BlockMap.aside_boundary: Option<usize>` is a bare index with top-era
semantics baked into its meaning: blocks `0..b` compost, `b` separator,
`b+1..` manuscript (document.rs:98–115). It persists as the `"boundary"`
key in the blocks map (store.rs:844–860), is read back by ONE reader for
the live doc and every materialized checkpoint state alike
(`read_state_of`, store.rs:171–181), and is embedded verbatim in every
`CheckpointState`'s JSON (store.rs:74–80) — which is **immutable once
recorded** (`set_checkpoint_state` refuses overwrite, store.rs:675–702).

After the flip the same integer means the opposite partition. Nothing in
the file says which era a state belongs to, so "historical checkpoints
keep their own era" (08 §2, 07 N3) is currently *unexpressible*:

- New build reads an old checkpoint state → the old compost renders as
  manuscript and the whole manuscript renders as scraps; counts, export,
  AI scope inverted wholesale.
- Old (shipped top-era) build opens a tail-era file → the manuscript's
  head renders as compost. The code's own compat promise is "older
  builds degrade to boundary-dropped, text preserved" (store.rs:840–843,
  document.rs:106–112) — misreading is strictly worse than that promise.

**Resolution.** The tail era gets its own persisted name at both levels.
(1) Live doc: a new blocks-map key (`scrap_line`); the migrated save
writes legacy `"boundary": null`, so a top-era build degrades to
no-boundary — the documented safe path — instead of misreading.
(2) `BlockMap`: a new serde field for the tail seam; `aside_boundary`
becomes a read-only legacy alias meaning top-era. Era = which field is
present; `read_state_of` and `checkpoint_state` interpret per field. Old
states stay byte-identical (immutability holds); an old build reading a
new state sees no known boundary field → safe degradation. Do NOT make
the seam a new `BlockKind` (design-tail §7's sketch): an unknown variant
makes older builds' serde fall back to the token parser and silently
reset every block kind in the file — the recorded reason the boundary is
an index (document.rs:106–112).

## 2 · Seam changes are invisible to the journal — BLOCKER

The journal records text ops only. Seam birth is an *empty* transaction
(`Document::set_aside_boundary`, document.rs:955–965); a park's
delete+insert ops journal as ordinary runs (document.rs:880–890) but the
`set_aside_boundary(Some(new_boundary))` rider does not
(document.rs:1134). `JournalEvent` has Pass/CardClosed/Restore/Export —
no boundary variant (journal.rs:56–67) — and `ReplayDoc` can evolve a
boundary only via `on_edit` (journal.rs:243–276).

Concrete failure: checkpoint anchor at 11:00 (no seam); first park at
12:00; scrub the strip to 12:30 and press the strip's Restore.
`strip_restore` lifts `scratch.replay.blocks` — boundary `None` — and
routes it through `restore_to_state`, which persists (editor.rs:
2668–2697, 2221–2271). The parked text is now manuscript again: counted,
exported, sent to the AI, saved. "Text never teleports across the
boundary by time travel" (08 §2) is violated by shipped machinery on day
one of Scraps, with no writer-visible signal.

**Resolution.** Journal the boundary: a `JournalEvent::Seam { t, at }`
recorded by every seam birth/move/evaporation (park, chord-park,
adoption, undo, migration), applied by `ReplayDoc::advance` interleaved
by timestamp. Pre-Scraps journals carry none and reconstruct against
their own-era anchors (case 1 tags those). Bonus: the strip gets an
honest mark for "the pile was born here" for free, and the seal/label
question (case 6) needs nothing else.

## 3 · Restore across eras: normalize or reproduce — DECIDE

`restore_to_state` installs the entry's BlockMap verbatim, boundary
included (editor.rs:2221–2271 → `restore_state`, document.rs:1429–1471).
Restoring a top-era checkpoint under the tail build would put live,
editable top geometry into an editor whose rail rendering, guards, and
verbs are deleted — every `manuscript_base_char` caller mis-scopes.

Two defensible answers: (a) reproduce literally — keep a read-only
top-geometry rendering path alive forever; (b) **normalize-on-restore**
— the restore transaction applies the flip transform to the incoming
state: membership preserved (07 N3's "never teleports" = no block
changes sides), position normalized; the "Restored" checkpoint
materializes tail-era. Propose (b): (a) pays permanent rent for a dead
geometry, and the preview (case 5) is where the writer *sees* the old
era. Side fix either way: `restore_state`'s length-mismatch fallback
`BlockMap::new(lines)` (document.rs:1461–1465) silently discards any
boundary — it must not swallow a seam over a trailing-newline
disagreement.

## 4 · The migration transaction: what actually breaks — DECIDE

The flip of a live top-era doc = delete `0..manuscript_base_char`,
insert seam+pile at the rope tail. Walked through the real machinery:

- **In-pile anchors don't travel.** `Annotations::apply_op` and
  `Graveyard::apply_op` CLAMP anchors caught inside a deletion
  (document.rs:744–771; test at :2044). A naive op-pair migration
  strands every margin note anchored in the pile (they must "park and
  travel with their block", 08 §2) and pins in-pile graveyard
  `origin_pos` to char 0 — Put back then resurrects text at the document
  head. Resolution: remap anchor ranges and `origin_pos` arithmetically,
  the `restore_state` precedent of preserve-and-reinstall around a
  wholesale op (document.rs:1440–1470) — never through `apply_op`.
- **The persisted undo stack time-travels across eras.** `History`
  carries full BlockMap snapshots (document.rs:1502–1518); after
  migration one cross-session ctrl-Z reinstates a top-era boundary into
  the live doc. Resolution: the migration drops undo/redo stacks, by the
  documented pre-P2 precedent ("the undo STACK is lost for that one
  upgrade… non-destructive", document.rs:1506–1511). Hence the migration
  is NOT a ctrl-Z atom; its inverse is Restore of the "Before migration"
  checkpoint — which case-3 normalization makes idempotent (P13
  satisfied in the restore grammar, the same door restore uses).
- **Honesty of the record.** Un-paused, the move journals as a
  document-sized run — exactly what `restore_state` pauses for
  (document.rs:1448–1456). Resolution: `journal.pause()` + a Seam/
  Migration event + a "Before migration"/"Migrated" checkpoint pair —
  restore's layered record (editor.rs:2209–2268), which is what "one
  honestly-recorded transaction" (08 §2) already means elsewhere.
- **Ordering and crash-safety are already right.** Migration runs after
  `compact_on_open` (inside `Store::open`, store.rs:400–401), before the
  first edit; `single_instance::claim` (single_instance.rs:85) means no
  second process can migrate the same file concurrently; disk changes
  only at the next atomic temp+rename save (store.rs:970–981), so a
  mid-migration crash leaves the top-era file intact and the next open
  re-migrates. First post-migration save rewrites blocks+spans+
  annotations+graveyard once (all fingerprints legitimately move,
  store.rs:353–362; seeded pre-migration at store.rs:444–467) — one-time
  oplog growth ≈ 2× pile size. Acceptable; name it in the commit.

## 5 · The parked past shows no geometry — and its count lies — DECIDE

History/parked mode deliberately hides all asides chrome: `let
aside_boundary = if in_history { None } … }` ("History mode shows
neither", editor.rs:9410), and `PreviewDoc` carries text/kinds only —
the strip's scrub even *holds* the replay boundary and drops it
(editor.rs:2565–2571; panel path 2056–2062). After the flip, previewing
a with-seam state renders the pile as ordinary trailing manuscript: the
parked past asserts the piece ends 3,000 words later than it ever did; a
mid-pile screenshot of the parked past is unclassifiable (P6), and any
count shown while parked scopes to nothing coherent (the live doc's
boundary against the preview's text).

Two answers: (a) status quo — the parked register strips ALL apparatus
(it already drops notes, cards, graveyard) and the count is hidden while
parked; (b) `PreviewDoc` gains `boundary + era`; the preview draws the
seam row read-only (hairline + label, no verbs) and scopes its count to
it. Propose (b): 08 §2 "past states keep their own geometry" is about
what the writer *sees*; the strip path is one field, and the panel's
diff-of-two-geometries takes the newer side's seam, matching the
existing "block style follows the newer side" rule (editor.rs:
2010–2017).

## 6 · A park as the seal's triggering edit; the vestigial jot sentence — NOTE

No park-specific checkpoint meta exists or is needed: the idle seal
(900 s → `add_checkpoint_if_changed("Session")`, editor.rs:1590–1597)
compares text+spans+blocks, and BlockMap's derived PartialEq includes
the boundary (store.rs:644–651) — so a boundary-only session (a pure
adoption gesture on an already-trailing pile) correctly earns its seal,
and a park journals as ordinary runs. One spec wrinkle for the record:
08 §2's undo paragraph still says "a jot in transit can never land in a
checkpoint state as manuscript" — a sentence stranded from the shelved
capture line. Under type-then-park, a 900-second walkaway between typing
the thought and pressing the chord WILL seal the thought as manuscript,
and the post-veto amendment accepts exactly that ("harmless"). The
amendment governs: build no seal-suppression machinery.

## 7 · Length-mismatch fallbacks silently drop the seam — NOTE

`ReplayDoc::new` (journal.rs:246–249) and `restore_state`
(document.rs:1461–1465) replace a mismatched BlockMap with
`BlockMap::new(lines)` — boundary `None`. A foreign or damaged journal,
or an off-by-one state, then reconstructs the pile as manuscript with no
signal — the exact silent-scope-trespass class the seam exists to kill.
Resolution: carry the boundary across the fallback;
`set_aside_boundary`'s clamp (document.rs:175–185) already degrades a
truly out-of-range index to `None`, so reuse it instead of discarding.
