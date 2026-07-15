# Spec-review ledger — wave 1 (2026-07-05)

*(12 adversarial reviews over specs 00–05: 16 blocking / 30 high / 58
mid / 19 low findings. This ledger is the integration record: every
blocking+high finding got a verdict; the specs are amended where the
verdict says so; implementers treat this file as an addendum to their
spec. Full findings JSON: session scratchpad
`spec-review-findings.json`; the raw texts ride the workflow journal.)*

## Verdicts — blocking

- **Restore breaks reconstruction** (B6/H37) — ADOPTED. The app-level
  restore path materializes an automatic checkpoint of the RESTORED
  state immediately after the swap ("Restored", `manual:false`), plus
  the `Restore` journal event. Reconstruction anchors are then always
  correct; the envelope folds `Restore.len_chars`. Applies to the
  strip's own Restore too (spec 01 §2 already re-bakes on it).
- **Seconds vs milliseconds** (B11/B15) — ADOPTED as a stated law, then
  hardened after the first implementation exposed sub-second double replay:
  every journal comparison is in ms; new checkpoints carry exact
  `created_ms`, while `timestamp_ms()` falls back to `created_unix × 1000`
  for legacy files. Test scrubs to an early t.
- **Stability law vs fabric scroll** (B7) — ADOPTED. The law now
  separates the immutable BAKE from the mutable view offset; the rig
  asserts the `bakes` counter and playhead/readout state only.
- **Journal-as-blob bloat** (B10/B14) — STALE vs code, REAL vs spec
  text: the implementation already persists two append-only Loro list
  containers with per-item pushes and an append-only counter
  (`journal_saved`); spec 00 §3 rewritten to match. Roundtrip +
  append-only tests landed (`journal_persists_appends_only_the_tail`).
- **Graveyard has no fingerprint channel** (B12) — ADOPTED: 5th
  `SavedHashes` channel, seeded at open, guarded like annotations.
- **AsideBoundary as a BlockKind breaks old builds and splice paths**
  (B13 + H42) — ADOPTED with a design change: **no new BlockKind
  variant.** The boundary is an out-of-band block INDEX
  (`BlockMap.aside_boundary: Option<usize>`), persisted as its own
  small key in the blocks map, adjusted inside `BlockMap::on_edit`
  (which already sees every block splice). The boundary LINE in the
  rope is a plain empty paragraph. An older build simply ignores the
  key: compost renders as leading paragraphs, nothing is lost, no
  block kinds reset; a round-trip through an old build drops the
  boundary (compost folds into the manuscript — text preserved,
  documented). B13's kind-duplication trap evaporates.
- **Cross-boundary selection** (B4) — ADOPTED: drag/keyboard selection
  clamps at the boundary; a selection can never span regions.
- **Compost keyboard trap** (B3) — ADOPTED: Esc from a rail caret
  returns to the last manuscript caret; Down at the last compost line
  crosses into the manuscript. The hard edge blocks drift INTO the
  rail, never exit from it.
- **CardFocus dangling on orphan migration** (B5) — ADOPTED: the
  migration resolves/deselects focus first (via resolve_composer /
  deselect path), atomically with the note leaving the margin.
- **Key-cap hints m/a/x/e would type over the selection** (B2/B9) —
  ADOPTED: the hints are dropped; the right-flank rows are mouse-only
  in v1 (matching the editor-button menu). No bare-letter bindings.
- **Packer pin claim unbacked** (B8) — ADOPTED: the right menu is an
  independent overlay that OCCLUDES cards at its y (it is transient);
  the packer is untouched in wave 1.
- **Cold-read reaction loop/margin gaps** (B0/B1) — DEFERRED with the
  package: folded into spec 05 as must-solve items (raise gesture,
  two-level Esc, paged margin model).

## Verdicts — high (grouped)

**Journal/strip mechanics.** Settle the journal at every checkpoint
creation so no run straddles an anchor (H44 — editor calls
`journal_mut().settle()` before `add_checkpoint*`). Reconstruction
carries spans/blocks via `ReplayDoc` — spec 00 §4's "text only"
wording corrected; formatting evolves from the anchor and explicit
mid-window toggles are the stated v1 gap (H29/H39). Replay is
char-indexed (`ReplayDoc`, rope ops) — spec 01 §2 wording fixed
(H38 — stale vs code). The readout's word count tokenizes the
reconstructed rope, never sums run deltas (H30). The bake FREEZES for
the lifetime of an open strip: background pass/checkpoint arrivals do
not re-bake; the one in-session re-bake is the explicit Restore
(H35). Margin lane and rail hide while previewing the past
(H36 — verify the existing checkpoint-preview path already does this;
if not, gate on `history_preview`). The wholesale-pause guard lives in
core `Document` where recording happens (H45 — stale vs code).

**Asides.** The aside verb and orphan migration carry an explicit
graveyard-suppression guard — a MOVE never files a corpse (H41). The
auto-cut trigger is a SINGLE selection-deletion op ≥ threshold
(deterministic, and the editor still holds the deleted text at that
point); backspace runs never auto-file (H24 + H43). Tail appends
insert a separator blank line when compost is non-empty (H23).
Caret-line aside: empty selection asides the caret's paragraph (H25).
Compost rail and outline are mutually exclusive (opening one closes
the other) (H26). AI passes: slice the manuscript, anchor within the
slice, re-offset ranges by the manuscript base (H40).

**Flanks.** Any row's action dismisses both flanks before its result
takes the lane (H20). Both flanks are suppressed while any history
surface is up (H22). Narrow fallback = formatting-only horizontal
popover; the verb set stays reachable via palette (H21).

**Editor button.** `run_pass(bool)` is refactored to a `PassKind`
enum (Believing | Doubting | Diagnostic(mode)) threaded through
deferred/pending state (H27). The doubting row's copy goes
form-neutral: "the strongest case against it" (H34). Button face uses
the glossary word: `Reading · {n} open` (never "Reviewing") and the
face is a priority function over (ai_status, parked, door, count):
NeedsSetup > Error > cooking > read-ready > unacknowledged-empty >
Reading·N > idle (H31,
H32). The button disables its rows while a history preview is up —
the pass must not diagnose a document the screen isn't showing (H33).
End Session is retired WITH the intent question (its only real job —
H28); sealing lives in the existing checkpoint verbs; the
`end_session_input`/`session_goal` survivors are re-audited in P4.

**Cold read (deferred package).** Entry affordances named (palette
verb + history-preview path) (H17); pass-during-read, paged margin
geometry, and visual→rope anchoring (hyphenation offsets) are
must-solve items in spec 05 (H16, H18, H19).

## Mid/low findings

58 mid + 19 low ride with the implementers: each package agent reads
the findings JSON for its spec and treats mids as a checklist —
adopting is default; skipping requires a one-line reason in the
package report. The assembly review re-audits the skips.
