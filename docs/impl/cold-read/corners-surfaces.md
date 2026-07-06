# Cold read corner cases — surfaces, attention, motion

Lens: what the takeover shows, what it hides, every animation it adds,
and every animation the rest of the app keeps running underneath it.
Spec = `docs/impl/05-cold-read.md`; law = `cold-read/design-law.md`
(L-numbers), `docs/attention-motion.md`, `docs/margin-card-dynamics.md`
§12–14, `docs/impl/compost-fresh/adjudications.md`. Code @ e6f8bb3;
every line anchor below re-verified against the tree. 2026-07-06.

Verdict up front: **2 blockers, 10 decides, 10 notes.** Both blockers
are one-line predicates the spec *asserts* but no code path expresses —
the same class as the compost round's "every park is mid-burst" find.

---

## 1. The reveal clock does NOT hold during a read — the spec's parking claim is false as coded — BLOCKER

**Scenario.** The writer asks for a doubting read, then enters the cold
read while it cooks (spec §4.6: "a running pass does NOT block entry
(results park)"). Twelve seconds in, the pass completes.

**What goes wrong.** The one arrival gate is `deliver_pass`
(editor.rs:3753–3765): it parks results **only** behind
`typing_burst_live()` (3642–3645: last buffer edit < 1s). Inside the
read nobody is typing, so the pass **integrates immediately** into the
hidden margin. Even if the writer entered mid-burst, the lull watcher
(3784–3806) polls only `typing_burst_live` and flushes 1s later. Then
every consequence fires invisibly:

- `integrate_pass` marks the new cards `appearing` and clears the marks
  on a **timer** (`CARD_APPEAR` + 150ms, editor.rs:3707–3724) — the
  once-only enter fade is *spent* behind the opaque desk. On exit the
  populated lane is simply there, static: the exact change-blindness
  swallow attention-motion.md principle 3 bans ("a change with no
  transient is not perceived at all").
- The announce has no channel: the editor button (the Ready face) is
  hidden by spec §4.1, `render_ai_status` rides the same suppression
  block as the margin (editor.rs:16575–16589) so the titlebar note is
  hidden too, and its 6s fade (`schedule_status_fade`, 3608–3624)
  expires while suppressed. The writer exits and is never told a pass
  landed — violating the two-clock model's t=0 announce
  (attention-motion §3) *and* the mode-matrix "never a silent swallow".
- Spec §4.4 states "a pass completing mid-read parks (the deferred-pass
  lull gate holds)". It does not hold. Building Wave B literally per
  §4.4 ships a lie about its own reveal clock.

**Resolution.** Extend the ONE rule, don't add a second clock
(margin-card-dynamics §13: "deliver_pass is the single gate"): the
parking predicate becomes `typing_burst_live() || cold_read.is_some()`,
in `deliver_pass` **and** in the lull watcher's flush condition. Exit
of the takeover joins scroll/door/new-pass as an explicit attention
shift: `flush_deferred_pass` in the exit path, after the suppressed
surfaces are restored — cards then land in a *visible* lane with their
250ms enter fade, and `integrate_pass`'s `AiStatus::Note` fires when
the titlebar can show it. The read itself is none of the flush triggers
(flipping pages, filing reactions, the banner pulse — none may flush).
Rig: `coldread:open` → `seed:deliver` → assert `ai_deferred:true` and
`appearing:0` while open; `escape` → assert cards landed + fade marked.

## 2. The wheel falls through to the hidden editor — exit restores a scroll the writer never made — BLOCKER

**Scenario.** Spec §4.3: "Mouse wheel: nothing in v1." The writer
idly wheels over the page (or an inertial trackpad coasts after the
entry click).

**What goes wrong.** "Nothing" is not what happens. The root
`on_scroll_wheel` (editor.rs:16399 → 7935–7958) has an overlay guard
for palette/ai-settings/shortcuts only (7939–7941). A ColdRead state
that merely paints over the editor lets every wheel event through to
it, where it (a) **scrolls the hidden document** (`scroll_top` moves,
7952–7954), (b) **flushes the deferred pass** (7944 — "scrolling is the
writer looking around", except she isn't looking at the manuscript at
all), (c) clears exit-fade ghosts and the selection popover. Exit then
violates §4.6's promise verbatim — "return caret/scroll exactly as left
(nothing moved — the takeover never touched them)" — the desk reopens
somewhere else, the silent-teleport class the prior corner round
existed to catch. With case 1 fixed, (b) would also secretly reveal
parked cards mid-read.

**Resolution.** Two belts, both precedented: `cold_read.is_some()`
joins the blocking-overlay guard at 7939 (§0.6 law 1, second line of
defense), and the takeover's root div eats wheel events
(`on_scroll_wheel(stop_propagation)` — the narrow-panel idiom,
editor.rs:16177). The reading lane alone may consume the wheel for its
own overflow (case 6). Rig: `coldread:open` → `wheel:` → dump asserts
`scroll_y` unchanged and `ai_deferred` still true; exit → `scroll_y`
identical to entry.

## 3. The flip zones sit ON the text — 40% of the measure can't be selected for a reaction — DECIDE

**Scenario.** The writer wants to react to the first words of a line —
"the ferry" at the left edge of the text block.

**What goes wrong.** The mock's `.flip` is `position:absolute` INSIDE
`.page`, `width:26%`, `z-index:2` (lab CSS lines 74–75): the zones are
26% **of the page**, layered **over the prose**. At 570px that is
148px per side; the side margin is 60px, so ~88px of text (≈11
characters) on each side of every line lies under a click-eating flip
zone — the mousedown that should start a word selection turns the
page. Spec §4.3's "middle ~48% inert (selection territory)" makes the
conflict explicit: the middle 48% of the page is 274px, but the
measure is 450px — 40% of the text is outside "selection territory".
This collides with the build's own cited law: "pointer real estate
near text must not page-turn out from under an aimed click"
(research-page §4.1, the margin-click caret-guard precedent) and P1
(the flip gradient shading her words on hover is chrome worn by text).

**Resolution.** Keep the zones' geometry (mock-approved, L13) but make
them **click-only, drag-transparent**: mousedown in a zone arms a flip;
movement past the drag threshold before mouseup converts to a word
selection (we own the hit map — recon §2 note 7); mouseup-in-place
flips. The hover gradient still shades the zone (P9: hover expands an
affordance that works without it), but paints **under** the text ink
(gradient first, fragments after — trivially ours since we paint every
fragment). Any narrowing of the zones to margin-plus-desk instead is a
named divergence from the mock (acceptance.md §2). Reaction-input and
lane cards stop propagation so they never flip (mock z-index 5
precedent).

## 4. Who is centered — the page, or the page+lane group? The mock says group; the spec's own numbers disagree — DECIDE

**Scenario.** A live read in a 1120px window; then the same read at
2560px; then the first reaction of the session files.

**What is undefined.** The mock centers the **flex group** (page +
18px gap + 230px lane, `.cr-stage` justify-center, lab line 66) — the
page sits ~124px left of window center. Spec §0 and L7 say the page is
"centered on a quiet desk"; spec §5.4 hangs the lane "230 px right of
the page". Three consequences nobody wrote down:

- If the page is window-centered, the lane fits only when
  `(W − 570·scale)/2 ≥ 248` → **W ≥ ~1066px** at scale 1.0 — the
  spec's "narrow windows (< ~900 px): the lane hides" threshold is
  arithmetic from the *group*-centered layout and is unreachable under
  page-centering.
- If the group is centered and the lane is *born with the first
  reaction*, filing it **shifts the whole page 124px left mid-read** —
  the anchor object teleporting because a card appeared, against P11
  and the NO-JUMP centred-column precedent.
- At 2560px both layouts put the lane page-hugging (the 18px gap),
  never at the window edge — but the gap itself is unnamed in the spec.

**Resolution.** Adopt the mock's group centering (it is the approved
design and keeps the ~900px threshold honest), with one hard rule:
**lane presence is constant for the whole read** — Live mode reserves
the 230px lane from entry (empty lane = empty desk space, no chrome),
so the page never moves while the takeover is up; the Past variant has
no reactions, no lane, and is truly centered (a named, lawful
difference). Resize across the fit threshold re-centers with the
resize snap (the established snap rule, §2.6). Name the 18px gap as a
token beside the lane width.

## 5. Wheel-flip and trackpad swipes — the verdict — DECIDE

**Steelman for wheel-flip:** every writer's hand knows wheel-down =
"more text"; Kindle's desktop app pages on wheel; a dead wheel reads
as a hung app; the flip keys are invisible until tried.
**Steelman against:** inertial trackpads emit dozens of events per
gesture — honest paging needs detent accumulation, thresholds, and a
cooldown (a gesture engine, in v1, for an input the corridor test
doesn't need); a misfired double-flip while reading is exactly the
motion-noise the room bans; the hover gradients + folio already teach
clicking; and "the page never scrolls" (L7/§3.2) is best taught by the
wheel doing *visibly nothing*.

**Resolution: uphold spec v1 = nothing — where "nothing" means EATEN
(case 2), never fall-through.** Trackpad swipe events arrive as the
same ScrollWheelEvent stream and get the same fate. One carve-out: the
wheel scrolls the reading lane when the pointer is over it and it
overflows (case 6) — contained, stop-propagation, the shipped
margin-lane-scroll precedent (2026-06-23 round). V2, if testers ask:
±1 page per accumulated detent with a ~250ms cooldown, honoring the
edge dead zones. Record it as the pre-named divergence candidate, not
built.

## 6. The reading lane: order, overflow, and the writer's-material no-fade rule — DECIDE

**Scenario.** A 6,000-word read; the writer files 30 reactions in one
sitting, several of them on page 3 after flipping back from page 9.

**What is undefined.** (a) Lane order: the mock's `crKeep` appends
(filing order) — but the mock only ever files while reading forward,
where filing order *is* document order; the two diverge the moment she
flips back. (b) Overflow: ~13 cards fit beside an 855px page; card 14
silently leaves the window with no pill, no count — the margin packer
and its off-screen grammar do NOT run here. (c) Motion: does a filed
card fade in? Does the stack slide when one lands mid-list?

**Resolution.** (a) **Document order** — the lane is marginalia
(L21), not a log; top-to-bottom mirrors the book's own order, and the
anchor-link grammar reads sanely. (b) The lane owns a private scroll:
wheel-over-lane scrolls it (case 5's carve-out); filing auto-scrolls
the new card into view (the writer's own act moves her own viewport —
lawful). No pills, no cull, no receipt grammar — a scrollbar-less
clipped stack that answers to the wheel matches "the desk is quiet".
(c) **No enter fade, no slides**: reactions are the writer's material —
"the writer's own notes never fade (your keystroke is instant; only
the arriving voice eases in)" (margin-card-dynamics §13). Mid-list
insertion displaces lower cards instantly (writer-initiated, discrete,
her eyes are already there). reduce_motion: identical (nothing moves
that isn't already instant).

## 7. The banner pulse: retrigger law, onset ramp, and the reaction-input leak — DECIDE

**Scenario.** The writer forgets herself and types a sentence at the
page — fifteen keys in four seconds. Later she types a ten-word
reaction into the open input.

**What goes wrong.** Three sub-cases: (a) The mock's `crPulse` (lab JS
line 390) stacks bare `setTimeout` removals — under sustained typing
the FIRST timeout strips the tint at t=900ms while keys are still
coming, the next key re-adds it: a ~1Hz on-off flicker, exactly the
strobe WCAG 2.3.1's single-pulse rule exists to prevent. Copying the
mock's JS behavior ships it. (b) The mock/spec say "180 ms ramp"
(L18, §4.2) — but a refusal is *feedback*, and feedback's first frame
must land <100ms (L28); a ramp makes the block feel mushy.
(c) Printable keys while the **reaction input** has focus are legal
typing — if the pulse listens at the wrong level, every reaction
keystroke re-pulses "Reading" and the banner sits lit the whole time
she types, a standing lie ("refused") over an accepted input.

**Resolution.** Adopt the shipped `pulse_strip` idiom, not the mock's
JS: one `Option<Instant>`, alpha a pure decaying function of it
(editor.rs:2775–2800, 12872–12875), **retrigger = reset the instant**
— under sustained typing the tint holds steady and decays 900ms after
the last attempt; no gap, no strobe, monotone luminance. Onset
instant (first frame full tint — the <100ms feedback law wins over the
mock's CSS-transition artifact; name it in the Gate-2 divergence
register). The pulse listener lives in the ColdRead key context only —
keys consumed by the TextField context on top never reach it (the
NoteInput precedent, §1 of the spec). reduce_motion: unchanged — a
single luminance pulse is already the reduced form (L28's own idiom).

## 8. Held-down Space: the flip fade retriggers into an unreadable page — DECIDE

**Scenario.** The writer holds → or Space to skim back to her place.
Key repeat delivers a flip every ~33ms; the incoming-page fade is
≤120ms from alpha 0.

**What goes wrong.** Every flip restarts the fade, so during the whole
held-key skim the page text never exceeds ~30% opacity — a strobing,
unreadable book precisely when the writer is *looking for something*.
Fails P6 (a 30%-alpha page is no true still) and the flip fade's own
justification ("the user caused it" — she caused *paging*, not
shimmer).

**Resolution.** One rule: **a flip within ~250ms of the previous flip
swaps instantly; only a flip from rest fades.** (Equivalently: never
start a fade while page-flip input is repeating.) This also covers
click-spamming a flip zone. reduce_motion already makes all flips
instant (§4.3). Rig: `coldread:flip` ×3 rapid + a capture — assert the
final frame is full-ink.

## 9. font_scale extremes: the 1140px page has no width story — DECIDE

**Scenario.** `font_size` maps to font_scale 0.6–2.0
(editor.rs:10317–10321). At 0.6 the page is 342×(≤513) — fine, a small
book. At 2.0 the page is **1140px wide** in, say, a 1000px window.

**What is undefined.** §3.2's degradation ladder is height-only
(gutter→0, the <12-line type drop, fewer lines). Nothing says what
happens when the *scaled width* exceeds the window: page clipped both
sides? Centered overflow? The one thing every law agrees on is what
must NOT happen — horizontal scrolling or continuous glyph rescale.
Also unresolved: does the <12-line emergency step ("15 px/420 px")
scale by font_scale (30px/840px at 2.0) or is it absolute?

**Resolution.** Extend the ladder with a width clause, same shape as
the height one: page width = `min(570·scale, window_width)`; when
clamped, the measure shrinks with it (margins hold at 60·scale). Below
the measure's justification floor (~45 EN/40 RU chars — Bringhurst's
own "go ragged below this", research-page §1.1) the block sets
ragged-right unhyphenated — the honest-degradation arm the engine
already has for RTL (§2.5). The emergency step's values scale with
font_scale (they are type metrics, not window metrics). Every clamp
re-paginates by the resize rule, never mid-frame.

## 10. Tiny windows: below ~4 lines the paginator's keep-rules are unsatisfiable — DECIDE

**Scenario.** A 300px-tall window: 300 − 36 (BAR_HEIGHT,
editor.rs:225) − 30 (banner) = 234px of desk; gutter→0; margins
48+64 leave 122px → **4 text lines** per page. At 200px, one line or
none.

**What goes wrong.** Widows/orphans ≥2/2 (§2.7) cannot both hold when
splitting a 4-line paragraph across 4-line pages; "a heading keeps ≥2
lines of its paragraph" needs 3 lines minimum; "avoid a hyphen on the
page's last line" fights both. A naive constraint loop either
oscillates (infinite re-break = a hang, which IS shippable damage) or
panics; a silent best-effort with no defined order ships
nondeterministic pagination (breaking the two-runs-identical Wave A
gate).

**Resolution.** Wave A law, unit-tested: **the paginator guarantees
progress — every page consumes ≥1 line, unconditionally.** Relaxation
order when capacity < 8 lines: drop hyphen-on-last-line avoidance →
relax widow/orphan to 1/1 → drop keep-with-next. Deterministic, stated,
and rig-covered by a `WSHOT_MODE=600x300` capture: a 2-line page with
a true folio ("— 7 of 41 —") is *correct* output ("below that, accept
it", §3.2). Page-height floor = margins + 2 lines; a window shorter
than that clips the page bottom against the desk (the window is broken,
the book isn't).

## 11. Resize mid-read: per-frame re-pagination is cheap for height, but the type step needs hysteresis — DECIDE

**Scenario.** The writer drags the window edge through the ~12-line
boundary, wobbling ±10px.

**What goes wrong.** Height-only resize re-breaks pages from cached
lines (no re-shaping — the measure didn't change): microseconds, safe
per-frame. But crossing the <12-line step swaps 16.5px→15px type =
full re-shape (~10–80ms, research-linebreak §7) — and a drag that
wobbles across the boundary thrashes full re-paginations and *strobes
the type size*, glyphs visibly re-rasterizing each frame — violating
"never continuously rescale glyphs" (§3.2) in spirit.

**Resolution.** Two-part rule: (a) same-metrics re-break runs on every
resize frame (snap, the margin-lane precedent); (b) the type-step
transition carries **hysteresis** (drop below 12 lines, return at ≥13)
and defers to resize-end (~150ms quiet — research-linebreak §7's own
"debounced resize-end"). While the step is pending, the current layout
stays, bottom lines clipping into the shrinking margin momentarily —
honest, still. Page-top-char resume (§2.6) applies after every
re-pagination including step changes.

## 12. Machine states inside the room: Running at entry, Error mid-read — DECIDE

**Scenario.** (a) A pass is cooking when she enters — where does
"Running…" live for the next 40 seconds? (b) The provider 500s
mid-read.

**What is undefined / goes wrong.** `render_ai_status` renders the
Running card, the Error card, and the NeedsSetup card into the margin
lane's slot or a bottom strip (editor.rs:15009–15046). It already
rides the same render block the suppression predicate will gate
(16575–16589) — so gating that block hides all of them. Is that
right? Running: yes — the room has no tools, the machine works
silently (P2; the cooking dot's home, the editor button, is hidden by
§4.1 anyway — and the dot is a static color cue, editor.rs:12318–12322,
no animation loop to leak). Error: `AiStatus::Error` has **no fade**
(set at 3851 with no `schedule_status_fade`) — it is persistent state,
so hiding it costs nothing; it surfaces intact on exit. But nothing in
the spec *says* any of this, and a builder who suppresses the margin
without the shared block (or vice versa) ships a cool machine card
floating over the desk — L24's "no AI cards materialize in the reading
lane", violated by a status card instead of a note card.

**Resolution.** Name it in the spec: the ColdRead suppression predicate
gates the one shared block (margin + rail + `render_ai_status`
together, the code's existing shape), so Running/Error/NeedsSetup/idle-
hint all sleep with the lane. Persistent states (Error, NeedsSetup,
Ready face) survive to exit by construction; transient Notes are
covered by case 1's parking (they fire at exit-flush, visible). Rig:
error-path smoke → `coldread:open` → dump has no status overlay bit →
exit → Error card present.

## 13. Hidden-surface animation audit: what keeps ticking under the desk — NOTE

Checked every spawn loop for wasted frames / state leaks while the
takeover covers its surface:

- **Cursor blink heartbeat** (editor.rs:5449–5485): wakes each 530ms
  but notifies only within 10s of last input — entering the read
  within 10s of typing burns ≤19 no-op-ish repaints, then silence.
  Harmless; optionally clamp `cursor_visible = true` at entry (the
  in_history precedent, 10449 region).
- **Editor-button cook dot**: static fill, no loop (12318–12322) —
  nothing runs under the hidden button. The strip's pulse loop
  (2775–2800) self-terminates ~900ms; chip_pulse and arrival_flash
  cannot fire mid-read (all mutations and travel verbs are guarded).
- **`update_lane_motion`** (14716+) runs in root render regardless;
  with the lane suppressed it diffs against a stale `last_frame` —
  cheap and self-healing (exit resize/scroll counts as snap). Verify
  exit doesn't fire spurious slides: new cards are appear-not-move by
  construction (plan_lane_moves only moves persisting ids).
- **The general rule this build should write down** (case 1 is its
  instance): *never spend a one-shot transient (fade mark, pulse,
  flash, status fade) while its surface is suppressed* — park the
  event, not just the pixels.
- **EditorElement under the desk**: keep it mounted (the strip/preview
  precedent) — `last_frame` consumers stay sane and the reuse
  fast-path makes the hidden frame ~10µs; do NOT unmount, or every
  `last_frame.as_ref()` site needs a None audit.

## 14. reduce_motion audit — every motion this build adds, with its reduced form — NOTE

Per L28 ("every translate/scale degrades to an equal-duration opacity
cross-fade, never a teleport") and the config switch
(config.rs:20–25, rig `reduce:motion`, smoke.rs:386–395):

| motion | full form | reduce_motion form |
|---|---|---|
| page flip | instant swap + ≤120ms opacity fade on incoming | **strictly instant** (spec §4.3; a fade is itself the reduced form of nothing — dropping it is lawful because the flip is a pop, not travel) |
| banner refusal pulse | instant-on seltint, 900ms decay, retrigger resets (case 7) | **unchanged** — single luminance pulse, no travel (WCAG 2.3.1-safe by construction) |
| anchor-link arrival flash | one 420ms `ARRIVAL_FLASH` background blink on the anchored word boxes (theme.rs:166 grammar) | **unchanged** — the blink IS the shipped reduce_motion form of the pulse family (editor.rs:12657 precedent) |
| flip preceding the flash | fade per row 1 | instant + blink — two transients, one gesture; lawful as the two-station receipt grammar (adjudications, Surfaces 4) |
| reaction input appear/close | instant (the writer's own act; popover precedent — no fade at all) | unchanged |
| reaction card lands in lane | instant, no fade (case 6; writer material never fades) | unchanged |
| lane auto-scroll on filing | instant placement | unchanged |
| flip-zone hover gradient | static hover state, no transition | unchanged (P9) |
| entry/exit of the takeover | instant scene swap, both directions (O9's "instant is lawful", extended to entry) | unchanged |
| re-pagination on resize | snap (no tween) | unchanged |

Nothing in this build translates, scales, springs, or loops — the
whole surface is already reduced-motion-safe except the flip fade,
whose off-switch is named. Assert `reduce:motion` + flip in the rig
once, for the record.

## 15. HiDPI: the paper grain doubles at scale 2, and goldens are per-scale — NOTE

The 256×256 tile (§3.3) drawn at logical px becomes 512 physical px at
scale 2 — each noise grain is a 2×2 blot, bilinear-blurred: the "fine
irregular grain" reads coarser/softer exactly on the displays that
show it best. Options: accept (v1 — consistent with every other
logical-px asset; the effect is ≤5% alpha and marginal), or draw the
tile at 128 logical px when scale ≥ 1.5 (constant physical frequency,
one branch). Recommend accept + flag for the taste round. Rig
consequences (VISUAL-RIG.md): text rasterization already differs per
scale factor, so the EN/RU page goldens are captured at a *named*
scale (`wshot.sh out.png 1`), "capture twice, keep the second", with
`STROP_TEST_STILL=1`; run one `wflip.sh`-style scale-flip over a
rendered page before shipping — the book page is a brand-new glyph
surface for the known upstream swash/ScaleContext bug class
(docs/UPSTREAM-gpui-scale-bug.md), and its goldens would be the first
to catch a regression.

## 16. Selection tint on the paper — verified, plus the gap-sliver trap — NOTE

`SELECTION_COLOR` 0xC8A951 @ 40% (theme.rs:143) over `#FEFEFC`
composites to ≈ #E8DCB8; text ink over it ≈ 12:1 — AAA holds, warm =
writer acting (color-language.md) is preserved on the brighter paper;
the ~5% grain underneath moves it negligibly. The build trap is
*shape*, not color: §5.1's "union of boxes" painted per-fragment
leaves unpainted slivers at every justified gap — a selected phrase
reads as separate amber pills, not one selection. Paint per-line
continuous runs: extend each selected fragment's box across its
following gap when the next fragment is also selected (exact f32
arithmetic, same as the pen positions). Also tint under ink: 
`paint_background` before fragments, never over.

## 17. The reaction input near the page bottom — flip above, own shadow, eats its events — NOTE

The ~250px card floats *under* the selection (S9 stands). On the last
lines of a page the input extends past the page edge over the desk —
fine (it carries its own border+shadow, mock z-index 5) — but near a
short window's bottom it would clip at the window edge, hiding the
text line and Enter target. Rule: when space below the selection <
input height + 8px, the card opens **above** the selection (the
popover-flip convention). It stops mouse/wheel propagation so it can
never trigger the flip zone it overlaps (case 3) and never scrolls the
hidden editor (case 2). TextField's caret is solid (no blink loop in
text_field.rs) — no determinism or idle-frame cost in goldens.

## 18. Desk gutter arithmetic: the banner is part of the ceiling — NOTE

Available page height = window − BAR_HEIGHT (36, editor.rs:225) −
banner row (30, the strip-banner pattern §4.1) − 2×gutter. The ≥24px
gutter is measured from the **banner's hairline**, not the titlebar or
window top — the two shipped banner rows both mount
`absolute().top(px(BAR_HEIGHT)).h(px(30.))` (editor.rs:12898–12907),
so a builder using window height alone overlaps the page's shadow into
the banner rule. At the gutter→0 degradation step the page top abuts
the hairline exactly (shadow 2/4 + 12/30 will paint over the banner's
last pixels — accept; the shadow is soft) and the bottom edge abuts
the window. The history variant has the same 30px row; no variant is
banner-less, so one formula serves.

## 19. Entry atomicity and the entry screenshot — NOTE

The <100ms pagination budget is spent **inside the verb's handler**:
compute the full `BookLayout` first, only then set
`Editor.cold_read = Some(...)` + notify — never flip the state and
paginate lazily in prepaint, or one frame renders a bookless desk (a
blank-desk flash that fails P6 on the single most important transition
this build has). Frame N = editor, frame N+1 = complete book with true
folio; entry and exit are instant scene swaps (row 14). Rig: the
`coldread:open` token asserts `pages ≥ 1` in the same dump as
`open:true` (no intermediate state is observable by construction);
wshot the entry frame itself. For the Past-variant golden the banner
carries a real date — seed via `debug_push_checkpoint` (store.rs:600
backdater) so the golden's date string is frozen, and keep
`STROP_TEST_STILL` for the timestamp freeze.

## 20. Single-instance rendezvous while a read is up — NOTE

A second `strop file.md` on the same file activates the existing
window through the single-instance socket (single_instance.rs — the
foreground drain "activates" on a timer). Activation must be
focus-only: verify the drain path never touches editor state (it
doesn't today — it raises the window), and that window focus arriving
does not steal focus from the ColdRead context (gpui re-focus lands on
the window's focused handle — the takeover's handle while open).
Failure would be quiet: a background `strop` invocation drops the
writer's read back to the desk. One rig line if cheap; otherwise a
manual check on the checklist.

## 21. Anchor-link flash + flip stacking — grammar check — NOTE

Clicking a lane card mid-read fires flip (≤120ms fade) then the 420ms
word-box flash — two transients for one gesture. Lawful: it is exactly
the adjudicated two-station receipt (origin = her click on the card,
destination = the anchored words; adjudications, Surfaces 4), and both
are single-shot luminance events. Same-page click (no flip): flash
only. Edge case: the anchor's page is the current page and the anchor
is already under the pointer — still flash (the receipt confirms the
*link*, not the travel). reduce_motion: instant flip + same blink
(row 14). The flash must ride the book's own paint (word boxes from
the hit map), not `arrival_flash`/LayoutKey (10342) — that key belongs
to the editor layout and would force a hidden-editor rebuild.

## 22. Lane top alignment and the unnamed gap — NOTE

L21/lab set the lane `padding-top:52px` = the mock's page top padding;
the reconciled metrics moved the page's top margin to 48px (§3.2). The
lane aligns with **the text block's first baseline area** (page top +
48·scale), not the mock's 52 — one more row for the Gate-2 divergence
register, or the side-by-side check flags a 4px drift as a bug.
Likewise name the page↔lane gap (18px, case 4) and that lane cards do
not scale with font_scale in v1 (they are chrome-adjacent marginalia
at 13px serif; if the page scales for eyesight, the lane arguably
should too — flag, don't build).

---

## Summary

The build's two structural risks are both "the spec asserts a behavior
no predicate implements": the reveal clock's parking gate is
typing-based and simply doesn't hold inside a read — a pass completing
mid-read integrates into the hidden margin, spends its once-only enter
fade and its 6s announce invisibly, and the writer exits to a silently
populated lane (case 1); and the root wheel handler underneath the
takeover both scrolls the hidden document and flushes the parked pass,
so "nothing moved" at exit is false the first time a trackpad coasts
(case 2) — each is a one-line predicate plus a rig assertion. The
decide-tier clusters around geometry the mock resolved one way and the
spec's prose another (flip zones layered over 40% of the measure,
group-centering vs page-centering and the lane-fit arithmetic), motion
rules that exist app-wide but have no owner inside the room (pulse
retrigger semantics, rapid-flip fade starvation, the lane's
no-fade/no-packer order-and-overflow story), and the degradation
ladders nobody extended to width, font_scale 2.0, or 4-line pages —
the last hiding a genuine hang risk (the paginator needs a stated
progress guarantee and relaxation order). The notes are mostly
reassuring: no animation loop leaks under the hidden button, the whole
surface is reduced-motion-safe by construction once the flip fade's
off-switch is honored, and the selection tint passes AAA on the new
paper — but the per-scale texture grain, the transient-on-suppressed-
surface rule, and the entry-frame atomicity each need one deliberate
sentence in the spec before Wave B writes them by accident.
