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
