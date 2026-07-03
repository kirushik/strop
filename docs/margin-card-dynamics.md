# Strop — Margin card dynamics

Companion to `DESIGN.md §7`. The canon (the two-layer model and its decided
behaviour) lives there; this note holds the *reasoning*: how cards behave over
time, the dynamic failure modes that motivated the redesign, and the build
order. Written 2026-06-19 with Kirill, off the 2026-06-14 margin-annotation
research (Google Docs / Hypothes.is / Notion / PAVA packing) and a dynamic pass
over the existing implementation.

## 1. The reframe (why two layers)

The current code packs one `MarginCard` type. But the margin actually hosts two
objects with different physics over time:

- **Layer A — writer's marginalia** (`ctrl-m`): authored, owned, GENERATE-side,
  mode-agnostic, long-lived, the stable layout spine.
- **Layer B — editorial review** (AI diagnoses): metadata, EVALUATE-side,
  mode-gated, batched per pass, decays with edits, resolve-to-clear, yields
  around Layer A.

This single split (validated independently by Kirill's craft instinct and by
the 2026-06-14 research, which parked it as "build authored marginalia as a
separate, simpler system; don't run it through the same packer") organises
almost every dynamic below.

## 2. Lifecycle — the transitions ARE the dynamics

A static packer places a snapshot; the bugs live in the *events* between
snapshots.

```
birth ─▶ drift ─▶ (stale?) ─▶ activation ─▶ resolution ─▶ (reappears?)
        (edits)   (Layer B)    (click)       (done/dismiss)  (re-pass)
```

- **birth** — Layer A: one at a time, at caret/selection. Layer B: in a pass
  (≤7), bursty. Both witnessed by the writer (no async source).
- **drift** — `Annotations::apply_op` rides anchors through edits; ranges can
  grow, shrink, or collapse to zero-width.
- **staleness** (Layer B only) — the anchored text changed since emission, so
  the diagnosis may now be wrong → grey out + deprioritise. Never auto-dismiss.
- **activation** — z-raise + expand-in-place + glow the anchor highlight; only
  minimal repack (highlight-only attribution means we don't need to shove the
  lane to prove which card is live).
- **resolution** — done / dismiss = a `NoteStatus` change, not a delete;
  reversible through history (principle 2).
- **reappears** — a later pass must not re-flag a dismissed span (see D2).

## 3. Dynamic failure-mode catalogue

Symptom → cause → fix. These are what made the bugs "hard to describe": they
only manifest over time.

- **D1 · Stale-anchor lies.** A diagnosis persists unchanged after you fixed the
  prose it flagged → it now lies. *Fix:* fade on edit-to-anchor; a new pass
  re-validates or the writer retires it.
- **D2 · Dismissal amnesia.** A new pass re-flags what you dismissed last pass →
  nagging. *Fix:* dismissals suppress re-flagging of the same span+kind (the
  minimal seed of the Editorial Agreement — learn only what to *stop* saying).
- **D3 · Notification debt.** Layer-B cards are cheap to emit, free to ignore →
  the lane becomes wallpaper. *Fix:* make resolution the path of least
  resistance; consider a document-wide soft cap / "N open — review" action;
  never let generation outrun resolution silently.
- **D4 · Cluster bloom.** Many cards on one hot paragraph all want the same y;
  with highlight-only attribution the overflow stack drifts far from its anchor.
  *Fix:* cap visible cards per anchor-cluster; collapse the rest into a
  "+N here" badge *on the anchor* (cluster-scoped, anchor-attached).
- **D5 · Edge flicker on scroll.** Culling pops cards in/out at viewport edges.
  *Fix:* small over-scan band + fade/slide at edges; smooth `▲N/▼N` counts.
- **D6 · Orphans under culling.** A card orphaned mid-session (restore) has no
  on-screen anchor, so it can't live in a viewport-attributed lane. *Fix:* it
  migrates to an `N detached` holder, not floating in the lane.
- **D7 · Altitude reveal surprise.** Resolving the open developmental card makes
  suppressed copy cards pop in. *Fix:* the rail already counts them; reveal
  gently, don't burst.
- **D8 · RETIRED.** Originally "what's new since I left?" (read/unread on
  re-entry). Does not hold: single-user + local files means **no asynchronous
  card source** — every birth is witnessed, nothing arrives while away. So no
  read-state, no "since last session"; re-entry shows the doc exactly as left.
  The only freshness axis is *which pass* (session-independent). Reopens only if
  cross-device Loro sync ever lands.

## 4. Implementation sequence

Ordered so the live bugs die first and each phase stands alone. Phases 1–2 need
no model change (they use the existing `NoteKind`).

1. **Packer + culling** (`strop-app`, `Editor::margin_cards`). Replace the three
   fighting passes with one PAVA/isotonic pass + per-card weights; cull to the
   viewport; `▲N/▼N` edge chips; `N detached` holder. *Fixes overlap + scroll
   pile-up — the reported bugs.* Measure card heights instead of estimating
   (`CPL=30`), and feed the active composer its *live* input length (the
   "overlap while typing" cause).
2. **Two-layer treatment.** Layer-A priority weight (pins; doesn't reshuffle on
   door toggle); visual distinction by treatment + AI-provenance mark, not
   colour; confirm Layer A is never gated by the door.
3. **Model** (`strop-core`). Add `pass_id` + a `ReviewPass` metadata record
   (id, started_unix, mode/scope, model used) to the annotation store; a
   staleness signal (flag flipped when an edit intersects a Layer-B range, or a
   stored anchor-text hash). Best-effort migrate legacy cards to a synthetic
   `pass 0`; breaking changes acceptable at 0.1.0.
4. **Aging + reconciliation UI.** Grey out stale Layer-B cards; pass left-edge
   tab (2–3 muted tones); new-pass reconciliation (stale carried cards grey,
   re-validation deferred per the open question); dismissal-suppression (D2).
5. **Motion polish.** Eased transitions; composer grows downward from a pinned
   top; stable-by-`id` reorder; minimal activation movement.

## 5. Deferred / open

- Re-validating carried-over Layer-B cards on a new pass (an AI call per stale
  card) — deferred to the §7 open question; v1 may just grey + let the writer
  judge.
- Document-wide soft cap on open Layer-B cards (D3) — revisit if debt shows.
- Cross-device sync resurrecting D8 — out of scope until Loro sync exists.

## 6. Resolved after the critique round (2026-06-19)

A three-lens adversarial review (principles / minimalism / dynamic-coherence)
found two composite contradictions and several over-built pieces. Outcomes:

**Staleness criteria.** A Layer-B card is **unverified** (not "wrong", not
"old") when a *committed edit intersects its anchor range*. Binary — no age
threshold ("N passes ago" conflates age with wrongness; an untouched old
diagnosis is still true) and no edit-magnitude threshold (a magic number, and a
poor proxy: one deleted word can invalidate a claim a 50% reword leaves true).
Cleared only by re-validation, writer action (done/dismiss), or Loro revision-id
returning to pre-edit state (so undo un-greys; no per-keystroke or history-scrub
flicker). Sensitivity is modulated by the existing `level`: copy/line cards
unverify on any intersecting edit; developmental cards (sticky, structural) only
on a material gutting of their span. NO separate LLM "stickiness" signal —
`level` proxies it (a new construct would violate principle 12). Grey reads
"unverified since your edit."

**Immutable-height caching (the real keystone).** AI card content is immutable
(writer can't edit a diagnosis), the lane is fixed-width, the font fixed → an AI
card's height is a pure function of fixed inputs → **measure once, cache, never
re-estimate**. Diagnoses render the same height active/inactive. Manual notes
re-measure at composer commits; only the actively-composing note needs live
height (grows down from a pinned top). This *eliminates* the `CPL=30` estimation
that caused the overlaps — the packer runs on real heights. Cache key =
(content, lane width, font size, display scale); invalidate on any change (note
the narrow-window panel is a second width regime). Disk-persist is optional
cold-open polish with the same invalidation. Measurement is cached; *placement*
still recomputes each frame (cheap, O(n)). A brand-new card uses the old
estimate for one frame, replaced by the measured value next frame (invisible
under eased auto-reveal).

**Packer simplification.** Drop the PAVA/isotonic formalism — at ≤10 cards a
single anchor-seeking pass with bidirectional overlap resolution is identical.
Layer-A notes and the active card are **hard positional constraints (true
pins), not soft weights** — a weight lets an active Layer-B card shove the
"stable spine"; a hard pin can't. If an active card has no free gap by its
anchor, it offsets and draws a short tick to its highlight (the only sanctioned
leader line, active-card-only).

**Packer = `place_margin_cards` (pure, proptested).** Shipped Phase 1. The
placement math is a pure function (`crates/strop-app/src/editor.rs`): floor +
downward no-overlap sweep, then a bottom-up pull-up that COMPRESSES the movable
run above each pin into its internal slack — a rigid slide left loose gaps
unused and stranded the selected card off the bottom edge (caught by eye, then
by proptest). The selected card's anchor is clamped UP so the whole card fits
above the viewport bottom. Three invariants are proptested: (1) no two cards
overlap; (2) every card sits at/below the floor; (3) the selected card lies
fully within `[floor, viewport_bottom]` whenever the stack fits the lane (over-
crowding past that is the Phase 4 visible-cap's job). Heights feed in MEASURED
(`shape_text`, cached by content). **Not yet test-covered:** the height
measurement itself (needs a `Window`) and viewport culling (needs a frame) —
those still ride visual verification.

**Auto-reveal, completed.** Announce-loud-once + reveal-on-attention: on
completion fire a transient rail state ("N ready · review", reusing the door
grammar) plus a momentary pip in the caret's margin (in-gaze for keyboard
triggers); surface cards into the lane only when the door is open AND the writer
next pauses/turns to the margin — never into a closed door or a live drafting
burst. Never scroll or steal focus. Honors the door × pass-completion seam (was
unspecified; now an explicit invariant).

**Visible-budget cap.** Cap *total visible* Layer-B cards at ~7 (not per-pass —
accumulation defeats the ≤7 rate-limit); a new pass rests older-pass cards
behind the door rail. Layer-A (writer notes) uncapped — working memory, not
judgments. This also bounds the "more cards than fit the viewport" overflow.

**Count grammar.** One idiom, mutually-exclusive buckets with precedence
(detached > off-screen ▲N/▼N > cluster); each card in exactly one. Door closed ⇒
rail only (no edge chips). Cut: per-cluster "+N here" badge, separate detached
*holder* (orphans pin in place, greyed), animated edge fades, the per-pass
colored tab (deferred behind staleness-grey — revisit only if testers report
batch confusion), and the elapsed-escalation prose ("taking a while" → just
spinner + bare elapsed seconds).

**Edge counts pulled forward (2026-06-19, with the Phase 2 squiggle).** The
off-screen `▲N / ▼N` pills shipped early, ahead of the rest of the count
grammar: a tester (Kirill) selected the second-to-last card, which pinned high
and pushed its neighbour off the bottom edge with NO trace — a direct violation
of principle 2 ("nothing can be lost, and the writer must FEEL that"), so the
honest indicator couldn't wait for Phase 5. `margin_cards` now returns a
`MarginLayout { cards, above, below }`; `above`/`below` count both anchor-culled
and packing-pushed-off cards (door-held cards stay the rail's job). Still
deferred to Phase 5: clickable jump-to-hidden, the bucket precedence with
detached/cluster, narrow-drawer count semantics (`cull = false` there).

**Diagnosis anchor mark.** Wavy/dotted squiggle (spellcheck idiom), never a
straight underline — coexists with the writer's `ctrl-u` and avoids resurrecting
the mark §2 banished.

**Caret-margin slot (future, banked).** The momentary caret-margin pip proves a
real channel: a guaranteed attention anchor. Treat it as SCARCE — anything
landing there must justify it; a principled visual language for "what may appear
at the caret margin" is deferred (n=1 today, too early). Do not let it accrete.

**Dismissal memory.** Defer the "learning"; v1 ships a dumb dedupe (don't
re-create a card matching an open-or-dismissed same-(kind, content-hash) on a
new pass). Not a hidden adaptive system; revisit as the surfaced, revocable
Editorial Agreement seed later.

## 7. Implementation status (2026-06-19, branch `better_card_placement`)

**Shipped (committed, tested):**
- Phase 1 — measured/cached heights, viewport culling, pure proptested packer
  `place_margin_cards` (`0b46b43`).
- Off-screen edge counts `▲N / ▼N` — pulled forward from Phase 5 because silent
  disappearance violates principle 2 (`c4c8a81`).
- Phase 2 — diagnosis anchor as a wavy squiggle (coexists with `ctrl-u`) +
  corner-shape layer distinction (notes rounder, AI crisper) (`c4c8a81`,
  `233cdcc`).
- Phase 3 — `pass_id` + staleness latch (`unverified` when the flagged text is
  edited; notes never decay; never auto-dismissed) + re-run dedupe
  (`is_suppressed`) + grey treatment (`c2230b5`).
- Perf — asset-GC idle-save stall fixed, 6.8 s → 24 ms (`8629b4f`); oplog bloat
  (`rebuild_marks`) diagnosed in `docs/perf-save-stall-2026-06.md`.
- Phase 6 (partial) — `card_slot` + `note_surfaces` extracted as pure tested
  functions (`c9f1231`).

**Deferred — need the running app to tune the FEEL** (building subtle visual/
timing UX blind risks regressing the "nothing vanishes / no interruption"
properties without a way to catch it). The door + culling + edge-counts already
cover most of the attention/honesty goal; what remains is presentation:
- Phase 4 — titlebar working-state + elapsed (move the in-flight card out of the
  lane), caret-margin pip, auto-reveal timing (announce-loud-once +
  reveal-on-pause), visible-cap (~7, rest older passes behind the rail via
  `pass_id`).
- Phase 5 — clickable jump-to-hidden on the edge pills, bucket precedence
  (detached/cluster), eased motion / stable-by-id reorder / composer grow-down,
  per-pass aging tab.
- Phase 6 remaining — GC-gate regression test; GPUI headless integration tests
  for the height-measurement + culling paths.

**Open decisions (Kirill):** the oplog-bloat persistence fix (perf doc, options
1–3); whether to compact the existing 2.86 MB file (destructive).

## 8. The composer interaction FSM (2026-06-20)

Three reported bugs turned out to be one structural defect, and the fix is the
general lesson for this whole subsystem.

**Symptoms.** (a) Press Enter on a note → the card renders blank chrome until
deselected. (b) Click away mid-edit → the input AND the text label both render,
same text. (c) Earlier: the draft mirror leaked a note's text onto clicked AI
cards, persisted.

**Root cause.** The card-interaction state lived in three fields —
`active_note` + `composing_note` + `note_input` — mutated by several handlers
that didn't keep them consistent, while the render read the composer from one
field and the body label from another. Every place two of those booleans
*disagreed* was a visible bug. We had been hand-policing an implicit state
machine.

**Fix — make the illegal states unrepresentable.**
```
enum CardFocus { Idle, Selected(id), Composing { id, input } }
```
The composer's id and its `NoteInput` are one variant's two fields, so
"composing but not active", "active-committed but blank", and "draft on the
wrong card" cannot be constructed. Every focus change funnels through
`resolve_composer` — the single exit from `Composing` — which commits the live
draft to the note it actually edits, then demotes to `Selected`. The render's
body region is one exhaustive `match` (`card_body → Composer | Text`): exactly
one of input-or-text, never both, never neither. Commits `5cb4dbb` (fix) on top
of `dec5c4b` (the earlier draft-leak patch the enum subsumes).

**Why an enum and not lifetimes / typestate (the question asked).** Lifetimes
scope borrows, not lifecycle-over-time — wrong tool. Typestate (a phantom-typed
`Editor<State>`) is actively wrong here: the editor is ONE long-lived
retained-mode entity re-rendered every frame; you can't swap its type per
interaction. The right Rust tool is a data-carrying enum + exhaustive match +
funneled transitions: low ceremony, and a new interaction state forces every
match to be updated.

**Where types still don't save you.** The other half of this subsystem is pixel
geometry (reserved height == painted height). Types can't catch a wrong
constant; that stays a measured-equals-painted discipline (the `CARD_*` /
`COMPOSER_*` constants, shared by measurement and render). The multi-line
composer (`02426a0`) needed exactly that care: it wraps at `COMPOSER_INNER_W`
and reserves the box's chrome so the growing field never clips/overlaps.

**Residual, NOT bugs (left deliberately):** abandoning an empty note (ctrl-m
then click away with no text) leaves an `(empty note)` placeholder card — the
writer dismisses it with `×`, consistent with "only the user dismisses". Could
auto-remove an empty note on resolve if it ever feels like litter. The
narrow-window composer strip reuses the same (now multi-line) input; functional,
not visually tuned.

**Tests (strop-app 39 → 43):** `card_body` total+exclusive; `CardFocus`
accessors for Idle/Selected; composing-implies-active over the id projection.
The entity-bearing `Composing` variant is correct by construction; a full
transition test would need gpui `test-support` (deliberately off).

> Note (2026-06-23): the composer's field is now a `TextField`, not `NoteInput`
> — `NoteInput` was extracted and deleted (see §9). `CardFocus::Composing` now
> holds an `Entity<TextField>`; everything above about the FSM is unchanged.

## 9. One text-field widget (`TextField`, 2026-06-23)

**Why.** Every small box of letters in the app (margin-note composer, command
palette, AI-settings fields, rename) had been the `NoteInput` entity — grown
incrementally from an append-only field to a caret/selection field. It still
fell short of what "a box you can type in" implies: mouse selection was
click-to-place + shift-click only, so the reflex *double-click a word, then type
to replace it* was busted; motion stepped char boundaries, not graphemes; the
prose canvas (a separate, mature text element) had a full mouse model the fields
didn't share. The fields were "almost a text field" in four different places.

**What.** `NoteInput` is gone, replaced by one reusable `TextField`
(`crates/strop-app/src/text_field.rs`). It is the full contract:

- *Pure core* (unit-tested, no GPUI): grapheme-cluster motion (UAX#29 via
  `unicode-segmentation`) so a caret never splits an emoji ZWJ run; word motion
  with the SAME semantics as the prose canvas (`previous/next_word_boundary`);
  `word_range_at` / `line_range_at` for click-unit selection; the utf16/char
  conversions for the OS IME boundary.
- *The widget*: caret + selection paint, the whole keyboard editing set, IME
  preedit, clipboard (masked fields never copy out), single-line scroll vs
  multi-line soft-wrap — all ported verbatim from `NoteInput`'s proven paint/IME
  code, so the regression surface is the new parts only.
- *Full mouse* (the gap that motivated this): click-to-place, drag-select,
  double-click-word, triple-click-line, word/line-snapped drag-extend,
  shift-click. `click_count` picks a `DragUnit`; `begin_select` fixes a
  `selection_origin`; `drag_to` unions the unit under the pointer with it. This
  mirrors the prose canvas's model rather than sharing state with it — the main
  editor is Loro-backed and multi-block; coupling the two would re-introduce the
  fragile shared-mutable-state class this whole subsystem keeps fighting.

**Migration.** All nine field sites switched to `TextField::{single, multiline,
palette, settings}`; the `note_input` action set became `text_field`'s `Field*`
actions; the field-editing keybindings moved into the module. `content` and
`focus_handle` are `pub(crate)` (the parent editor reads them back); a
`debug_caret()` accessor feeds the rig instead of exposing internals.

**Verification (rig, not eyeball).** Driven through real GPUI dispatch via
`STROP_SMOKE` (`scripts/wrun.sh`), asserting `dump:ui`'s `field_sel` char range:
- `f10 …type "selectme"… click:X,Y,2` → `field_sel [1,9]` (the word, excluding
  the `>` command prefix); then typing `x` → `>x` (replace-on-type — the exact
  workflow that was broken).
- `…click:X,Y,3` → `[0,9]` (whole line).
- `…click drag:` → a multi-char range tracks the drag.
Selection *rendering* confirmed by screenshot (`scripts/wshot.sh`): the gold
band spans the selection. Pure core: 6 unit tests; suite 46 → 48.

**Deferred (explicitly cut, not forgotten):** per-field undo/redo (the prose
canvas has it; fields don't yet) and edge-hold autoscroll during drag (single-
line fields autoscroll via the existing caret-follow; only a held drag past the
edge is unserved). Both are additive on the same widget.

## 10. Backlog (deferred from the 2026-06-23 field work)

- **Per-field undo/redo** in `TextField` (coalesced, selection-restoring). The
  prose canvas has it; the small fields do not yet.
- **Edge-hold drag autoscroll** in `TextField`: single-line fields autoscroll
  via caret-follow, but *holding* a drag past the edge without moving doesn't
  scroll. Needs the prose canvas's `autoscroll_tick` timer pattern.

## 11. Review round (2026-06-23): navigation, packer, focus — and the harness

A five-lens adversarial review (a workflow: textfield / margin / focus / gpui /
color, each finding then independently verified) turned up 14 real defects on
this branch. Several were the *same* off-screen-card bug seen from different
angles. Each non-obvious one was fixed as a CLASS, with the cheapest test
abstraction that makes the whole class discoverable — per Kirill's standing rule.

- **One source of truth for off-screen cards.** The pill COUNT, the navigation
  TARGET, and the RENDERED set were computed from three different filters, so a
  pill could read "1 below" yet do nothing, or scroll to a door-suppressed
  non-card. `MarginLayout` now carries `above/below: Vec<OffscreenRef>` (id +
  content-anchor-y + `anchor_culled`); the pill count is `.len()` and
  `reveal_offscreen` navigates that exact list. Divergence is now impossible by
  construction.
- **Two reveals, by how the card hid.** Anchor scrolled off-screen → scroll it to
  the NEAR edge (`reveal_scroll`, pure + proptested: lands the anchor
  `REVEAL_INSET` from the edge, never a page away — the "pagination feel" fix).
  Anchor on-screen but packing pushed the card out → SELECT it, so Pass 3 forces
  it in. Either way the pill always acts.
- **The active card wins the lane (packer Pass 3) — by DISPLACING, not
  overlapping.** A tall writer note pinned in the slack above a selected diagnosis
  used to shove it off the bottom while `card_slot` still reported it `Shown` —
  invisible AND uncounted (principle 2 violation). Pass 3 re-clamps the active
  card fully into view, then shoves the run above it UP to stay clear; a card
  pushed past the floor is culled into the honest `above` count (becomes "N
  above") — never overlapped, never painted over the titlebar. (First shipped as
  a sanctioned active-card *overlap*; Kirill flagged that it broke the
  never-overlap rule, so it became displacement.) `card_slot` lost its `active`
  special case — pure geometry now, so it can't lie. INV3 proptest strengthened
  to include competing note pins; INV1 is back to no-overlap-EVER.
- **The active card is door-exempt.** Selecting a copy-level diagnosis suppressed
  under an open developmental one lit the anchor but rendered no card.
  `margin_cards` now surfaces the active card regardless of the door (mirroring
  its anchor-cull exemption).
- **Anchor hit-test trailing edge.** A click on the trailing half of an anchor's
  last glyph snaps to `c == end` and missed the strict `< end` test (dead zone).
  Extracted `note_at_char` (pure, unit-tested): strict-contain first, then accept
  the trailing boundary — back-to-back anchors never double-claim.
- **Composer exit always restores focus.** `select_card`/`set_note_status` (lane
  clicks) resolved the composer WITHOUT re-focusing the document, stranding the
  keyboard. Focus restoration moved INTO `resolve_composer` — the one documented
  exit from `Composing` (§8) — so every exit handles it by construction.
- **Unified scroll.** One `on_scroll_wheel` on the document root, not per-element,
  so the whole surface (gutters, lane, whitespace) scrolls, not just the prose.
- **TextField hardening** (`text_field.rs`): the single-line newline policy moved
  to the one splice point `replace` (so dictation / IME commit can't inject a
  `\n` into a filename, not just paste); masked `text_for_range` returns dots (the
  IME/a11y read path was leaking the API key past the copy/cut guard);
  `character_index_for_point` localizes the window point like the mouse path.
- **Bold-title height.** Diagnosis titles are painted bold but were measured
  normal-weight, under-reserving a row at the wrap boundary → overlap. Measure
  with the paint weight.

**Test abstractions chosen, by class:** pure proptests
(`reveal_scroll_lands_at_the_near_edge`, strengthened `selected_card_stays_fully_in_view`,
geometry-only `card_visibility_is_honest`) and pure unit tests (`note_at_char`,
`single_line_field_flattens_newlines`) for the algorithmic classes;
correct-by-construction structure (single source of truth, single focus-restoring
exit, one scroll handler, masked read-path) where a value test would be brittle;
visual rig (`wrun`/`wshot`) for the integration colours and scroll. Still on the
Phase 6 list: GPUI headless integration tests for the height-measurement +
culling paths (the bold-title and masked/hit-test fixes ride reasoning until then).

## 12. The budget recedes, it never hides (2026-07-02)

A design reversal, one release-candidate old. Commit `e40e215` shipped the
Phase-4 "visible cap" as *hide*: over `VISIBLE_DIAGNOSIS_CAP`, the oldest
passes were dropped from the lane entirely and tallied in the rail as
"N resting · review". Reviewed against the product's first principle — *you
just type and it works; nothing needs operating* — that shape was wrong three
ways, one of them a plain bug:

- **It institutionalized a reported bug.** A capped diagnosis kept its squiggle
  in the text but had no card in the margin — exactly the "the sentence
  highlights but no card shows" state reported as a bug (and fixed) earlier the
  same week. A margin that disagrees with the text isn't calm, it's broken.
- **It was document-global.** The held set was computed over every open
  diagnosis in the file, then the lane culled to the viewport. A crowded page
  three sections away could empty the margin *here* — squiggled sentences
  beside an empty lane with plenty of room. The budget solved a per-viewport
  problem (clutter) at per-document scope.
- **It made the writer learn a policy.** "Why does THIS sentence have a card
  and THAT one not?" had no answer visible on the page — the answer was an
  internal sort by pass recency, surfaced only as a third rail-label grammar
  ("resting · review") next to two other grammars. Invisible policy is the
  opposite of a mental model.

**The replacement — recede, don't hide.** On paper, dense marginalia get
smaller; they don't get filed in a drawer. Over the full-size budget
(`FULL_DIAGNOSIS_CAP`, still 5, still newest-passes-first via the same pure
`oldest_beyond_cap`), older diagnoses now render as **one-line cards at their
anchor** — title only, muted, `COLLAPSED_CARD_H` tall, clickable. Clicking
selects, and the selected card is budget-exempt, so it expands in place. The
budget is counted **lane-local** (among the cards that survived the anchor
cull), so it answers the only question that matters: "is *this page's* margin
too dense?" The rail returns to door-only semantics; the "resting · review"
label is gone; nothing about the budget needs explaining, because everything
it does is visible where it does it.

**The invariant this buys** (new, rig-asserted): *every flagged passage in the
viewport has a card in the lane* — full or one-line, but present at its
anchor's height. `seed:many` (8 diagnoses, 2 passes) drives it against a real
frame in `rig-check.sh`: 8 visible, 3 receded, no overlap; click a receded
card → it expands, 2 receded, still no overlap.

## 13. Phase 4 shipped: one clock, two fades (2026-07-02)

The Phase-4 plan of record (announce-loud-once + reveal-on-attention across
four triggers + caret pip) was re-reviewed against the same first principle as
§12 and built in a pared form. What shipped:

**The reveal clock is one rule.** A completed pass arriving mid-typing-burst
parks (`deferred_pass`) and lands the moment the prose has been still for
`TYPING_LULL` (1s) — or immediately on any explicit attention shift: scroll,
the door (shortcut, rail, narrow pill, squiggle-reach), or asking for another
pass. That's the whole model. The four-trigger machinery (margin-gaze
detection, a 15s idle timer, a "N ready · review" rail state) was cut before
building: strop already HAS the writer-controlled attention gate — the door —
and gaze/idle triggers are haunted-house UX (cards materialize because your
mouse drifted). Parked results stay un-anchored so quotes anchor against the
text as it stands at reveal; they carry their generation, so cancel/re-run
staleness needs no new mechanism. The **caret pip is cut** (the research's own
weakest-supported bet; the titlebar note is the announcement) — it returns
only if testers report not knowing a pass finished (tester guide §3 asks).

**Motion only when it means something.** Two fades, both opacity-only:
- *Enter* (`CARD_APPEAR`, 250ms decelerate): only GENUINELY new cards — a card
  scrolled back into view never re-announces itself, and the writer's own
  notes never fade (your keystroke is instant; only the arriving voice eases
  in). One fade per landed pass, marks cleared right after (`appearing`).
- *Exit* (`CARD_RESOLVE`, 150ms accelerate): the model commits instantly; only
  the card's ghost lingers, painted under the live lane, dropped on scroll.

**Deliberately not built:** a `reduce_motion` flag — both animations are
already the reduced-motion-safe form (short, run-once, opacity only). The flag
lands together with re-pack move-tweening (Phase 5), the first real
translation there'd be anything to reduce. *(It did — §14.)*

**Harness (the §11 discipline, continued):** `deliver_pass` is the single
arrival gate shared by the real LLM path and the `seed:deliver` rig hook, so
rig-check.sh drives the actual clock end-to-end in a real window: type → pass
parks, nothing surfaces; lull → lands; wheel → lands immediately. New smoke
tokens `wait:MS` and `resolve:first`; the dump gains `ai_deferred`,
`appearing`, `departing`, and both fade lifecycles are asserted (marked at the
event, cleared right after — nothing can ever re-fade).

## 14. Phase 5 shipped: the slide, its off-switch, one honest bucket (2026-07-03)

The last phase of this arc closes the motion story and the count grammar.

**Re-pack slides (`CARD_MOVE`, 200ms standard, 40ms stagger capped at 4).**
When a discrete re-pack moves surviving cards — a card resolves in a crowded
lane, a landed pass displaces neighbours, a selection expands a receded card —
each survivor now SLIDES from its old slot to its new one instead of
teleporting. This is the one place motion buys object constancy (Heer &
Robertson; attention-motion.md verdict 2): an instant jump makes the eye
re-find every card, a short slide keeps each one the same object. Mechanics
worth recording:
- *One rule decides tween vs. snap* (`update_lane_motion`, a render pre-pass
  that diffs the packed lane between frames): a discrete re-pack in a still
  lane slides; any CONTINUOUS cause — scroll, the live composer growing, a
  typing burst reflowing anchors — tracks 1:1 and clears all motion. The lane
  never animates against the writer's own movement, and never mid-burst.
- *Content-space deltas*: a move stores its offset in content space and the
  render applies it to the CURRENT frame's target, so a scroll landing
  mid-slide still moves every card exactly 1:1 with the text.
- *Re-target, never snap back*: a further re-pack mid-flight redirects the
  card from its currently-displayed position toward the new slot.
- The open-time import of a .md file stamps a text edit, so the first second
  after opening a fresh file rightly snaps — a rig trap encoded in
  rig-check.sh (`wait:1100`), not a bug.

**`reduce_motion` (config.toml).** Travel becomes an opacity cross-fade of the
same duration: the live card fades in at its new slot while a non-interactive
ghost of it fades out at the old one — "reduced motion is not no motion", a
teleport breaks felt continuity as badly as a violent slide (attention-motion
§4). The entrance/exit fades were already reduced-safe and are unchanged. GPUI
exposes no OS reduce-motion query, so this is the writer's own switch; the
`reduce:motion` smoke token flips it for a rig run.

**Count grammar closed (the §6 precedence, audited).** With the detached
*holder* cut (orphans pin in place at best-effort anchors) and cluster badges
cut (D4 never materialised), the mutually-exclusive buckets reduce to: the
lane, the ▲/▼ edge counts, and the rail — and the audit confirms each open
note lands in exactly one (door-held cards never reach the lane; the active
card is exempt from door AND cull, so it can't double-count; rig-asserted:
4 seeded cards scroll away → 1 active visible + 3 above). One real drift
found and fixed: a receded one-liner LOST its "· detached" marker — an
orphaned card must never read as confidently anchored, however small it
renders. The pill contract survives orphans as-is: an off-screen detached
card counts in ▲/▼ honestly because its card really is at the best-effort
anchor the pill scrolls to.

**Not built, still:** the per-pass aging tab (§6 cut list) — waits for testers
to actually report batch confusion, per the tester guide.

**Harness:** smoke gains `resolve:last` (resolve the newest note — the bottom
full-size card in the seeded lane, whose departure deterministically expands a
receded card and shifts the run below; `resolve:first` hits an already-receded
one-liner whose departure legitimately moves nothing — the check that taught
us that is in rig-check.sh) and `reduce:motion`; the dump gains `moving` and
`reduce_motion`. rig-check asserts: survivors slide then settle, scroll snaps
all motion instantly, cross-fade mode registers the same moves, and the
one-bucket exclusivity above. The tween-vs-snap policy and the staggered
easing are pure functions (`plan_lane_moves`, `staggered_ease`) with unit
tests — the timing contract is code, not feel.
