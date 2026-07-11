# Cold read — corner-case adjudications (2026-07-06)

Rulings over the five `corners-*.md` files (86 cases: 12 blocker-grade,
~32 decides, the rest notes). Each corner file holds the full analysis
and code anchors; this file states what is LAW for the build. Where a
ruling says ACCEPTED, the corner file's resolution text is the spec
verbatim. This file RULES OVER `05-cold-read.md` where they conflict
(the spec's four false sentences are amended in place as well). Gate 0:
none of these rulings re-opens a product decision; all follow recorded
law.

## The foundation (the blockers, one ruling each)

**F1. One slice function, both books.** ACCEPTED (regions 1 + time 2,
merged): extract `manuscript_slice_of(text, spans, blocks)` as a free
function beside `manuscript_range_of`; `Document::manuscript_slice`
delegates to it; the Live book and the Past book consume ONLY it. The
Past banner's word count goes through `manuscript_range_of` on the
state's own blocks (the `words_at` precedent). The book never feeds
from the panel's diff projection. Unit tests: Top-era state (pile
never enters), Tail, no-boundary, everything-compost. `seed:legacy`
gains a Top-era-boundary checkpoint so the gate can catch regressions.

**F2. The slice string is never transformed.** ACCEPTED (regions 2):
the spec's "NFC guaranteed at the document layer" is struck (false).
Law: hit boxes and selection ranges live in slice space, identical to
document offsets modulo `+base`; hyphenation looks up an NFC COPY of
the single word and, if the NFC form differs from the raw form, skips
hyphenation for that word (the missing-dictionary degradation). NFD
fixture added to the §2.8 offset tests.

**F3. Restore exits everything, everywhere.** ACCEPTED (time 1):
`restore_to_state` drops `cold_read = None` in the same breath it
drops previews, and when `strip.open` it runs the strip epilogue
(unpark, snap to now, re-bake — hoisted from `strip_restore`). The
book's Restore chip gets the `is_parked`-style idempotency guard.
Caret after Restore: `0..0` (the shipped restore contract; the
"nothing moved" promise belongs to Esc, not to a document swap).

**F4. The guard is architectural.** ACCEPTED (scopes 1), all three
belts: (1) registry-driven refusal — every command whose ruling is
"guard" binds its chord in the ColdRead context to one `CrRefuse`
action that pulses; future commands are guarded by default unless the
allow-list names them; (2) handler-side guards (`cold_read.is_some()`
→ pulse + return) on every mouse-reachable handler and shared
mutation sink, ordered BEFORE `is_parked`/`history_view` checks;
(3) the reaction input mounts INSIDE the ColdRead-context element,
never on the root.

**F5. Copy is source-honest and alive.** ACCEPTED (scopes 2): ctrl-c
and ctrl-insert bind in the ColdRead context to `cr_copy`, which
slices the SOURCE snapshot by the selection's manuscript char range —
never the painted fragments (which carry no spaces and invented
hyphens). PRIMARY-selection parity via the `publish_primary`
contract. Empty selection: silent no-op. Wave-A test: selection
across a hyphenated break equals the source substring.

**F6. The wheel is eaten, twice.** ACCEPTED (scopes 10 + surfaces 2 +
notes 17): `cold_read.is_some()` joins the root `on_scroll_wheel`
early-return set AND the takeover surface stops wheel propagation.
The one carve-out: the reading lane consumes the wheel over itself
only if its receded stack overflows (see S6). Rig: wheel mid-read →
`scroll_y` unchanged, no cards revealed, page unchanged.

**F7. The reveal clock holds in the room.** ACCEPTED (surfaces 1 +
notes 2): `deliver_pass` and the lull watcher park when
`typing_burst_live() || cold_read.is_some()`. Exit joins scroll/door/
new-pass as a flush trigger — `flush_deferred_pass` runs AFTER the
suppressed surfaces are restored, so cards land visibly with their
one enter fade and the titlebar note fires when it can be seen. Entry
does NOT flush. Nothing inside the read (flips, reactions, the pulse)
flushes. `render_ai_status` sleeps under the same suppression block
as the margin (surfaces 12); persistent states (Error, NeedsSetup)
survive to exit by construction.

**F8. Entry resolves the composer first.** ACCEPTED (notes 1 +
notes 12): entry's first act is `finish_composing` (draft committed
by the universal blur law; the focus-out no-op guard then holds), and
entry ends with `CardFocus::Idle`; exit restores no card selection.
Rig: composer open → `coldread:open` → focus is the takeover, body
holds the draft.

**F9. The selection unit is the source token.** ACCEPTED (notes 3):
every fragment carries the full char range of the whitespace-
delimited token it came from (both halves of a hyphenated word share
it; NBSP-joined tokens are one token; the bound «слово —» fragment's
token is word + dash); word-snap unions TOKEN ranges. Unit test:
hyphenated + U+00AD words — both fragments carry equal, char- and
grapheme-aligned token ranges.

## Regions & the slice

1. Parked-banner `Read` verb: renders iff the playhead resolves to a
   checkpoint WITH a materialized state within the banner's 5-working-
   px window, by tick identity (index/at_ms), regardless of label
   rank; no qualifying checkpoint → no verb; never `state_at`.
   ACCEPTED (regions 3 + time 5, merged). Past entry runs NO entry-
   checkpoint sequence (a Past read starts from a checkpoint by
   construction).
2. The empty book: entry always succeeds; pages = max(1, computed);
   one honest blank page (paper, grain, running head, `— 1 of 1 —`,
   banner `0 words`); no Scraps hint anywhere. Blank in-slice
   paragraphs paginate as blank lines. ACCEPTED (regions 4).
3. Banner station segment: newest checkpoint with `manual == true`
   (writer-tier), walked newest→oldest; none → segment omitted; all
   automatics ("Cold read", "Session", "Restored"…) skipped by the
   one rule. ACCEPTED (time 3, subsuming regions 5).
4. Inverse rebase: file = slice + base; display = doc − base
   (saturating, out-of-range drops the link); the session lane keeps
   its slice-space ranges. Round-trip unit test. ACCEPTED (regions 6).
5. One accounting: `ColdRead.words = manuscript_word_count()` at
   entry, formatted by `format_thousands`; Past = the `words_at`
   analog on the state's own blocks. ACCEPTED (regions 7).
6. Giant paragraph: a page always makes progress (≥1 line);
   keep-rules yield in stated order when a paragraph fills whole
   pages. ACCEPTED (regions 8; the full relaxation order is S10).
7. Keep rules see the SLICE's block sequence only; a slice-final
   heading sets on the last page, rule vacuous. ACCEPTED (regions 9).
8. The slice triple is the book's only formatting source; clipped-
   span invariant tests. ACCEPTED (regions 10).
9. Footnotes: refs superscript-numbered by ref order WITHIN the
   slice; `FootnoteDef` blocks in-slice are SKIPPED by the paginator
   (definitions stay off-page even when they live in the manuscript);
   banner counts the region, not the pages — accepted asymmetry.
   The region-blind `insert_footnote` append (lands in the pile on
   every seamed doc) is a PRE-EXISTING DEFECT flagged for its own fix
   outside this build. ACCEPTED (regions 11).
10. Images: missing assets render the editor's missing-image
    degradation at deterministic size (same input → same pagination);
    checkpoint-state images verified GC-safe. ACCEPTED (regions 12).
11. Legacy mid-backfill: the entry checkpoint's silent deferral is
    accepted and named (rig must not assert a checkpoint on the
    legacy fixture's first launch). ACCEPTED (regions 13 + time 10;
    the `Read`-verb gate is ruling 1).
12. Whole-doc fingerprint stands; two-ticks/one-book after pile-only
    edits is intended. ACCEPTED (regions 14). Blank-region unit tests
    on `manuscript_slice_of`. ACCEPTED (regions 15).

## Time & the strip

1. Live entry while the canvas shows the past (`strip.is_parked() ||
   history_preview.is_some()`): GUARD with the pulse. Strip open at
   now: close it and enter. ACCEPTED (time 4; amends spec §4.6).
2. Misfiled reaction: the lane card carries the margin's own dismiss
   (`set_note_status` Dismissed) — the inverse the note keeps for
   life; no undo carve-out. ACCEPTED (time 6, option a).
3. No journal event for reads — words are the quant; the entry
   checkpoint and the reactions' own timestamps are the record.
   RULED (time 7). A later analytics impulse must overturn this line.
4. Checkpoint arithmetic verified (last-only dedupe sufficient;
   reaction-only reads leave no checkpoint — correct); entry mirrors
   `dirty_since_checkpoint = false`; entry order stays save → guard →
   checkpoint → snapshot → paginate. ACCEPTED (time 8 + 11).
5. "Cold read" name: `station_rank` AND `station_display` gain
   manual-aware arms (bare unnamed tick; a writer-renamed "Cold read"
   keeps writer rank/display). Old builds verified harmless.
   ACCEPTED (time 9).
6. Past banner date: computed at render from `created_unix` + fresh
   now (the strip's `date_label` convention), never baked. ACCEPTED
   (time 12).
7. Past-from-parked: strip hides (element unpainted) but state
   survives untouched; Esc returns the identical parked frame; the
   ColdRead guard outranks the parked guard at every sink (Past
   pulse target = the bold checkpoint name — one idiom, two banners).
   ACCEPTED (time 13 + scopes 8).
8. Crash inventory accepted; the reaction input deliberately has NO
   draft heartbeat (a one-line reaction is not a composition).
   ACCEPTED (time 14).
9. ctrl-shift-l inside the read: TOGGLE-EXIT (the strip/palette
   precedent; P13 chord symmetry) — overrules time 15's guard
   proposal; scopes table stands. Restore idempotency + ctrl-alt-s
   guarded: ACCEPTED (time 15 remainder).

## Scopes & chords

1. **The pierce table is RATIFIED as written** (scopes 0): guard
   everything except — f2/rename (allow, both entrances), ctrl-n,
   ctrl-o, ctrl-c/ctrl-insert (live copy), ctrl-q (allow),
   ctrl-shift-l (toggle-exit). All Editor-context chords BIND to the
   refusal (unbound = silent swallow = banned). ColdRead owns
   escape/arrows/space/shift-space/pageup/pagedown/home/end +
   ctrl-home/ctrl-end aliases; up/down inert without pulse.
2. Select-all: guarded in v1; the v2 candidate (current page) is
   named once here and not built. ACCEPTED (scopes 3).
3. The typing-pulse rule: unbound keydown with `key_char.is_some()`
   && !ctrl && !platform; modifier-only presses provably can't fire;
   Space can't double-fire; product Esc exits (the mock's Esc-pulse
   is a named non-adopted lab artifact). RATIFIED (scopes 4).
4. Reaction-input chords: Enter on empty = no-op; Esc closes input +
   collapses selection; guarded chords typed into the input pulse
   (via F4's architecture); quit with unfiled text loses the line —
   accepted, named. ACCEPTED (scopes 5; the close-path law is N1).
5. Titlebar ⌕/≡/clock: DIMMED (muted, no hover tint, default cursor,
   tooltips kept, pill drops its "ctrl-f" hint), clicks pulse.
   Window controls fully live. ACCEPTED (scopes 7).
6. Past-mode selection: LIVE for selection + copy in both modes; what
   Past disables is the reaction INPUT (never raises; no input, no
   filing). Overrules notes 7's dead-at-mousedown variant — the
   parked-copy precedent wins; the warm tint is honest because copy
   is a live writer verb. Amends spec §4.7's "selection does
   nothing"; named divergence at Gate 2. (scopes 11 over notes 7.)
7. Pre-existing silent swallows (`scraps_travel`/`set_aside` while
   parked): fix-in-passing LAWFUL, one line each through
   `pulse_strip`. ACCEPTED (scopes 12). Rig coverage per scopes 13,
   dump gains `coldread{open,pages,page,source,pulse}`.

## Surfaces & motion

S1. Flip zones: geometry kept (26%, over the page) but CLICK-ONLY and
    drag-transparent — mousedown arms a flip, drag past threshold
    converts to selection, mouseup-in-place flips; hover gradient
    paints UNDER the text ink; input and lane cards stop propagation.
    ACCEPTED (surfaces 3).
S2. Centering: the GROUP (page + 18px gap + 230px lane) centers, per
    the mock; Live reserves the lane from entry (empty lane = empty
    desk space); the Past book has no lane and truly centers — named
    lawful difference. The page NEVER moves while the takeover is up.
    ACCEPTED (surfaces 4).
S3. Wheel-flip: v1 = eaten-nothing, upheld; the ±1-page detent flip
    is the pre-named v2 candidate, not built. ACCEPTED (surfaces 5).
S4. Banner pulse: the `pulse_strip` idiom — one Instant, alpha a
    decaying function, retrigger resets, INSTANT onset (the <100ms
    feedback law overrules the mock's 180ms ramp — named divergence),
    ~900ms decay; the listener lives in the ColdRead context only
    (reaction-input keystrokes never re-pulse). ACCEPTED (surfaces 7).
S5. Rapid flips: a flip within ~250ms of the previous swaps
    instantly; only a flip from rest fades. ACCEPTED (surfaces 8).
S6. The lane: **document order** (marginalia, not a log — surfaces 6
    over notes 8's filing order); mid-list insertion displaces
    instantly (writer material: no fades, no slides, ever); overflow
    = the margin's own recede-in-place grammar (oldest-filed recede
    to their one quote line first; click flips + expands) — no second
    scroll surface, no hidden count (notes 8 over surfaces 6's
    private scroll). Pathological overflow past even the receded
    stack clips at the lane top — accepted papercut-tier. Filing
    keeps the new card visible by construction (it lands at its
    document position; if off-lane it recedes a neighbor, never
    scrolls the page).
S7. font_scale width clause: page width = min(570·scale, window);
    clamped → measure shrinks, margins hold at 60·scale; below the
    justification floor (~45 EN / 40 RU chars) the block sets
    ragged-right unhyphenated; the <12-line emergency step's values
    scale with font_scale. ACCEPTED (surfaces 9).
S8. Tiny windows: the paginator guarantees progress (≥1 line/page);
    relaxation order when capacity < 8 lines: drop hyphen-avoidance →
    widow/orphan 1/1 → drop keep-with-next. Deterministic, unit-
    tested, rig-covered at 600×300. Page-height floor = margins + 2
    lines; shorter windows clip the page, not the text. ACCEPTED
    (surfaces 10 + regions 8).
S9. Resize: same-metrics re-break every frame (snap); the type step
    carries hysteresis (<12 drop, ≥13 return) and defers to
    resize-end; page-top-char resume after every re-pagination.
    ACCEPTED (surfaces 11).
S10. Machine states sleep with the one shared suppression block
    (margin + rail + render_ai_status); Error/NeedsSetup surface
    intact at exit. ACCEPTED (surfaces 12).
S11. The transient rule, written down: never spend a one-shot
    transient (fade mark, pulse, flash, status fade) while its
    surface is suppressed — park the event, not just the pixels.
    EditorElement stays MOUNTED under the desk (reuse fast-path);
    optional `cursor_visible = true` clamp at entry. ACCEPTED
    (surfaces 13).
S12. The reduce_motion table (surfaces 14) is RATIFIED: the flip fade
    is the only motion with an off-switch; everything else is already
    its own reduced form. One rig assertion under `reduce:motion`.
S13. HiDPI grain coarsening at scale 2: accept for v1, flagged to the
    taste round; goldens captured at a named scale with
    STROP_TEST_STILL. ACCEPTED (surfaces 15).
S14. Selection tint: per-line continuous runs (extend each selected
    fragment's box across its following gap when the next is
    selected); tint painted before ink. ACCEPTED (surfaces 16).
S15. Reaction input: flips ABOVE the selection when below-space <
    height + 8px; clamps within the window; never covers the words it
    quotes; eats mouse + wheel. ACCEPTED (surfaces 17 + notes 18).
S16. Page height counts from the banner's hairline (BAR_HEIGHT + 30);
    at gutter→0 the page abuts the hairline; one formula, both
    variants. ACCEPTED (surfaces 18).
S17. Entry atomicity: compute the full BookLayout BEFORE setting
    `cold_read` + notify — no bookless-desk frame exists; entry/exit
    are instant scene swaps. Past-variant golden seeds a backdated
    checkpoint so the date string freezes. ACCEPTED (surfaces 19).
S18. Single-instance activation must not steal ColdRead focus —
    manual checklist item (rig line if cheap). ACCEPTED (surfaces 20).
S19. Anchor-link receipt: flip (fade rules) + 420ms word-box flash =
    the two-station grammar; same-page click flashes only; the flash
    rides the book's own paint, never the editor's arrival_flash/
    LayoutKey. ACCEPTED (surfaces 21).
S20. Lane top aligns to the text block (48·scale); the 18px gap is a
    named token; lane cards don't scale with font_scale in v1
    (flagged, not built). ACCEPTED (surfaces 22).

## Notes & reactions

N1. The close-path law: **Esc is the only discard.** Blur, lane-card
    click, flip (click or key), resize — each files non-empty trimmed
    text exactly as Enter would, then acts; whitespace-only = plain
    close. The captured range is the raise-time range. ACCEPTED
    (notes 4, agreeing with scopes 5).
N2. Chip + typed text combine: `"~ drags — {text}"` (em-dash, L21's
    own exemplar); bare chip files `"~ drags"`. Named mock divergence.
    ACCEPTED (notes 5).
N3. Undo stays guarded inside the read (the top-of-stack revert-
    under-a-snapshot lie is worse than the two-gesture fix); the
    redo-stack casualty of filing is accepted and goes in the tester
    guide. ACCEPTED (notes 6).
N4. Page-spanning word-snap ranges file whole; tint paints visible
    fragments as per-line unions; lane flip targets the page
    containing `range.start`; flip keys are ignored while the mouse
    is captured mid-drag. ACCEPTED (notes 9).
N5. Overlap/duplication of reactions: legal, unlimited, undeduped
    (P2). ACCEPTED (notes 10).
N6. Exit frame: lane and margin render on one mutually-exclusive
    switch; reactions never carry `appearing`; `moves_started`
    unchanged across open→react→escape; off-viewport reactions
    surface as edge pills — lawful. ACCEPTED (notes 11).
N7. Twin-anchor jumps: precedented, floored by F9's whole-token
    anchors; the detached grammar catches misses. ACCEPTED (notes 13).
N8. Parse-failed annotations container: pre-existing exposure, not
    worsened; breadcrumb fix out of scope. ACCEPTED (notes 14).
N9. The anchor quote: grapheme-boundary truncation at 42 + U+2026;
    newlines flattened to spaces (the `move_note_to_scraps`
    precedent); U+00AD rides invisibly; RTL renders as shaped.
    ACCEPTED (notes 15).
N10. Block-kind holes are deliberate: list markers and running
    head/folio dead to selection; image captions carry no ranges (no
    caption anchoring in v1); cross-image drags file legally.
    ACCEPTED (notes 16).
N11. Filing bumps revision; the book keys off the snapshot — verified
    absorbed; `mark_dirty` + `bump_activity` on filing. ACCEPTED
    (notes 19 + time 11).

## Spec amendments (made in 05-cold-read.md in the same commit)

- §2.4: the NFC sentence struck; F2's law written in.
- §4.1: "which exits the takeover itself" → "which is taught to exit
  the takeover and unpark the strip (F3)".
- §4.4: the parking claim now cites F7's predicate.
- §4.6: the strip-close sentence split per Time 1.
- §4.7: "selection does nothing" → selection + copy live; the input
  never raises (Scopes 6).
- §5.4: lane threshold re-derived from group centering (S2); lane
  reserved from entry.

## Build order

Wave A gains: `manuscript_slice_of` (F1) + its tests, the token-range
fragment field (F9), the NFC-skip rule (F2), the progress guarantee +
relaxation order (S8), width clause (S7). Wave B gains everything
else. The two pre-existing defects flagged for SEPARATE fixes (not
this build): region-blind `insert_footnote` appends into the pile
(regions 11); the parked-mode silent swallows are fix-in-passing
(scopes 12).
