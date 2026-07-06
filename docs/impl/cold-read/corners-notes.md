# Corner cases — notes, reactions, margin interplay

Lens (e) of impl 05 §7. Against `05-cold-read.md`, `cold-read/design-law.md`
(L19–L22, I4–I5, §2-S9), `research-linebreak.md`, `08-compost-fresh.md` +
`compost-fresh/adjudications.md`, and the code at `cold-read` @ e6f8bb3
(all line anchors verified on this commit). The lab mock's reaction JS
(`docs/mockups/ux-lab-2026-07.html`, scene-1 script) was read as the
approved-design reference; two of its behaviors are named below as mock
bugs not to copy.

## 1. BLOCKER — entering over an open composer: the blur-commit steals focus back into the editor

**Scenario.** The writer is mid-thought in a margin-note composer
(`CardFocus::Composing`) and hits ctrl-shift-l. Spec §4.4's suppression
list (lane, rail, pill, flanks, chips, popover, menu, omnibar) never
names the composer; §4.6's entry steps go straight to
`save_now → checkpoint → snapshot → paginate → focus the takeover`.

**What goes wrong.** The composer's TextField carries an `on_focus_out`
subscription (editor.rs:3297–3313) that fires the moment the takeover
takes focus, and — the composer still being open — calls
`finish_composing → resolve_composer`, whose last act is
`window.focus(&self.focus_handle, cx)` (editor.rs:3251): **the editor's
own focus handle, re-focused after the takeover's**. The reading room is
born deaf: the `ColdRead` key context never has focus, so ←/→/Space go
through Editor bindings — arrows move the *hidden caret*, silently
breaking §4.6's exit promise ("return caret/scroll exactly as left —
nothing moved"). Esc survives only via the `escape_mode` belt branch.
Additionally `set_note_body` pushes an undo snapshot and clears the redo
stack *mid-entry* (document.rs:2326–2333), and if any path leaves the
state `Composing`, the draft heartbeat keeps calling
`set_note_body_draft` every tick during the read
(editor.rs:1763–1776, document.rs:2339–2342).

**Resolution.** Entry's first act, before `save_now`:
`finish_composing(window, cx)` (the SINGLE composer exit — its own doc
comment, editor.rs:3231–3252, demands every focus-changing action call
it first; entering a takeover is exactly such an action). The
`composing_id() == Some(id)` guard then makes the later focus-out a
no-op, so nothing double-commits and nothing re-focuses. The draft is
committed, not lost — the universal commit-on-blur law upheld. Add the
composer (and `CardFocus` generally, case 12) to §4.4's entry-close
list, and a rig token: composer open → `coldread:open` → dump asserts
`focused` is the takeover and the note body holds the draft.

## 2. BLOCKER — "a pass completing mid-read parks" is false against the code: the lull gate does not hold while reading

**Scenario.** Spec §4.4: "a running pass does NOT block entry (results
park)… cards reveal on exit". The writer enters with a pass running (or
with `deferred_pass` already parked from a pre-entry typing burst) and
reads.

**What goes wrong.** `deliver_pass` defers **only** when
`typing_burst_live()` (editor.rs:3753–3765). A reader is by definition
in a lull, so a pass completing mid-read calls `integrate_pass`
immediately; a pass parked *before* entry is flushed by the lull
watcher's 250 ms poll (editor.rs:3784–3799) within a second of the
last pre-entry keystroke. Consequences: diagnoses land in the doc
mid-read (lawful data, wrong moment); cards land in the *hidden*
margin with their `appearing` entrance-fade set and expired before exit
(editor.rs:3703–3725, 1007–1011) — on exit they are simply, silently
there, no arrival, violating the reveal clock's one rule
(attention-motion §2) and the mode-matrix's "never a silent swallow";
and `render_ai_status` paints its floating "N margin queries anchored"
card into the takeover (editor.rs:3728–3741, 15009+ — a surface §4.4
never suppresses). A wheel event reaching the root handler makes it
worse: `on_scroll_wheel` flushes the deferral unconditionally
(editor.rs:7944).

**Resolution.** One-line gate: defer when
`typing_burst_live() || cold_read.is_some()`; exit joins scroll and the
door as an attention-shift flush point (`flush_deferred_pass` on
takeover drop, so exit IS the reveal — the door's own law, I5, and the
margin's deliver_pass single-gate precedent). Suppress `render_ai_status`
under the §4.4 predicate; the status is still there on exit. Entry does
NOT flush (entering is turning away *from* the desk, not back to it).
Rig: `seed:deliver`-style hook mid-read → dump asserts zero open
diagnosis cards until after Esc. *(Shared with lens (d), which owns the
reveal clock; filed here because the failure is cards materializing in
the margin.)*

## 3. BLOCKER — hyphenated word: per-fragment hit boxes yield half-word anchors unless snap resolves to the token

**Scenario.** "difficult" breaks as `dif-` / `ficult` across two lines
(or across a page, §4's last-line-hyphen rule being only best-effort).
The writer starts or ends a reaction drag on either painted fragment.
Spec §5.1 says "pagination emits per-fragment hit boxes carrying
manuscript char ranges; drag selects word-to-word" — as written, each
fragment carries its own *subrange* of the word.

**What goes wrong.** Word-snap over the prefix fragment alone files an
annotation anchored to `dif` — a half-word range persisted into the
annotations container. The lane quote reads `"dif…"`; the margin card
after exit anchors mid-word; and at the next restore,
`Annotations::reanchor` slices the covered substring `dif` and
`diagnose::anchor` matches it to *any* `dif` near the offset
(document.rs:892–912, diagnose.rs:134–150) — the wrong-twin jump made
near-certain by a three-letter needle. Silent, persisted, wrong: the
same class as the prior round's text-teleport blockers. The identical
trap exists for U+00AD-bearing words (breaks are the soft-hyphen
positions; painted text strips U+00AD but the source range must keep
it — research-linebreak §1.2/§6) and for the bound «слово —» fragment.

**Resolution.** The selection unit is the **source token**, not the
painted fragment: every fragment carries, besides its own range, the
full char range of the whitespace-delimited token it came from
(NBSP-joined tokens are one token; the merged dash fragment's token is
the word + dash — honest text, research-linebreak §5); word-snap unions
*token* ranges. Both halves of a hyphenated word share one token range
by construction, so hitting either selects the whole word including any
U+00AD. Wave-A unit test: break "Anfractuous" and a U+00AD word, assert
both fragments' token ranges are equal and char+grapheme-aligned (the
§2.8 offset-realignment fixture already stages this text).

## 4. DECIDE — the reaction input's close-path inventory: Esc is the one discard

**Scenario.** Text sits half-typed in the reaction input when: the
writer clicks a lane card (flip+flash), clicks a flip zone, resizes the
window (re-pagination; §5.1 collapses the selection), or quits. Spec
§5.2 defines Enter (file) and Esc (close) only. The mock leaves the
input *open* on click-away and its flip-zone handlers stay live under
an open input (`ux-lab-2026-07.html`, `crFlip` onclick vs the
key-handler's `ri.style.display` gate) — a mock bug, not a design.

**What goes wrong / undefined.** Four close paths with no ruling. The
app's one shipped law is commit-on-blur — the composer
(editor.rs:3297) and the link field (editor.rs:5405–5409,
`commit_field_on_blur` 2219–2238) both commit; a reaction input that
silently drops typed words on a stray click is the writer's words dying
(P3), while one that lingers over a flipped page anchors to a range the
eye can no longer see.

**Resolution (one rule).** *Esc is the only discard.* Every other close
— blur, lane-card click, flip (click or key), resize — first files
non-empty text exactly as Enter would (trimmed; whitespace-only = plain
close, the mock's own Enter guard), then performs the action. The
captured range is the input's, taken at raise time. Quit mid-input is
the accepted v1 loss (the reaction has no note id yet, so the composer's
draft-heartbeat rescue cannot apply — name it, don't build it). A filed
note is one dismiss from gone (P13, L22) so over-filing is cheaper than
silent loss. The counter-position — a reaction is a verdict, blur-filing
creates accidental judgments — is honest but buys inconsistency with
every other field in the app; adjudicate.

## 5. DECIDE — chip clicked while typed text sits in the field

**Scenario.** The writer types "cut the schedule talk?" then clicks
`~ drags`.

**What goes wrong.** The mock discards the typed text (`crKeep` clears
the input; the chip files only `~ drags`). Yet design-law L21's own
exemplar card body is **"~ drags — cut the schedule talk?"** — glyph
*and* prose in one body, which spec 5.3's chip-OR-text convention cannot
produce. Copying the mock ships silent word loss; following L21 needs a
combination rule.

**Resolution.** Chip with non-empty text files one body:
`"~ drags — {text}"` (em-dash separator, exactly L21's exemplar);
chip with empty text files the bare `"~ drags"`. Still the body
convention — no schema, the glyph is text (P3), the bold-first-glyph
cosmetic (spec 5.3) applies unchanged. Name the divergence from the
mock at Gate 2 (acceptance.md §2: deliberate, not silent).

## 6. DECIDE — undo inside the read: keep the guard; record the steelman and the redo casualty

**Scenario.** A reaction filed, instantly regretted. Spec §4.6 guards
undo (pulse, no-op); the agenda asks whether `add_note` deserves to be
the ONE undoable verb since it is the writer's act in this room.

**Steelman for allowing it.** `add_note` is its own undo atom
(document.rs:2308–2315); P13 wants the inverse in the same grammar,
immediately, where the act happened.

**Steelman for the guard (wins).** `doc.undo()` pops whatever is on
top. Ctrl-Z with *no* reaction filed this session reverts the writer's
last prose edit **under a book that renders the entry snapshot** — the
page keeps showing text the document no longer contains for the rest of
the read (§2.6 forbids re-pagination), the purest "ship a lie" in this
whole feature. Scoping undo to "only if top-of-stack is a this-session
reaction" builds a mode-dependent undo — a bespoke illegal-state
machine for a note that is already one `×` from gone. The margin's own
grammar IS the inverse (L22, P13's "a reaction note dies by the
margin's own dismiss/fade grammar"), and post-exit ctrl-Z removes the
last reaction cleanly (precedented: ctrl-m notes, editor.rs:3209–3229).

**Resolution.** Guard stands (pulse via the ColdRead context — Editor's
own ctrl-z binding never fires under the takeover's focus, so the
context must bind it explicitly or the swallow is silent, violating
L18). Record the casualty: filing a reaction clears `redo_states`
(document.rs:2313) — a writer who undid a cut, entered the read to
check, and reacted has lost the redo path, silently. Precedented
(every ctrl-m does this) — accept, and name it in the tester guide.

## 7. DECIDE — Past-mode "selection does nothing" must mean disabled at mousedown

**Scenario.** In a history read (§4.7) the writer drags across the
page out of habit.

**What goes wrong.** If the disable sits at input-raise, the drag still
paints the warm selection tint — the writer-acting color (L29) on a
surface where the writer's one verb is refused: a color lie, and a
dead-end affordance (P9's inverse). The parked-strip precedent runs the
other way — selection there is live and copy is deliberately allowed
(editor.rs:7760-region; recon §1) — so the two readings genuinely
conflict.

**Resolution.** v1: no drag state is created at all in Past mode —
mousedown on prose is inert, no tint, no input, and the first
mouseup-after-drag-attempt pulses the banner once (the refusal idiom,
L18; a bare click stays silent — clicks are already "nothing" in Live
mode). Named collision for lens (c): if the corner round grants
page-copy (the parked-mode precedent), Past mode needs live selection
after all, and the disable moves to input-raise with the tint's warmth
re-argued. One ruling, recorded either way.

## 8. DECIDE — lane overflow and ordering: many reactions

**Scenario.** A 12-page read files 25 reactions; the 230 px lane holds
6–8 cards.

**Undefined.** Spec 5.4 gives the lane no overflow behavior and no
ordering. The mock appends downward unbounded (`cr-lane.appendChild`).

**Resolution.** Filing order, newest appended at the bottom (the mock's
reading order — a session log, not an anchor pack). Over budget, the
*oldest* cards recede in place to their one quote line — the margin's
own over-budget grammar (the 2026-07-02 recede-in-place reversal:
"every visible squiggle keeps a card" becomes "every reaction keeps a
line"), click still flips to the anchor. Never a scroll surface (a
second scrollable in the reading room fails P11), never a hidden count.
Alternative worth one sentence at adjudication: hard-cap the lane and
let older reactions simply await the desk — lawful by the
narrow-window precedent (5.4's "cards await on the desk"), but it makes
the lane lie about the session. Rig: `coldread:react` ×10 → dump
asserts lane count + receded count sum to notes filed.

## 9. DECIDE — a selection spanning two pages: the drag can't, the word-snap can

**Scenario A (drag).** Mousedown on prose, drag right into the flip
zone, mouseup there. Verify by construction: flip is a *click* on the
zone (down+up on the same element, the gpui contract); a mouseup ending
a text drag fires no zone click, so no flip, and the union clamps to
the current page's boxes. Pin it with a rig drag token ending in the
zone: page index unchanged. Also: flip *keys* pressed mid-drag (the
ColdRead context still has them until the input opens) must be ignored
while the mouse is captured — else the page swaps under a live drag and
the box union goes cross-page garbage.

**Scenario B (word-snap).** When the §4 page-break rule can't avoid a
hyphen on a page's last line, the straddling word's remainder opens the
next page. Word-snap on either fragment (case 3's token rule) yields a
range whose *visual* extent spans two pages.

**Resolution.** The range is honest — file it whole. Tint paints only
the visible fragments (the union rect per line, first-box-start to
last-box-end, so stretched word gaps don't strobe as holes); the input
anchors under the visible portion; a lane-card flip targets **the page
containing `range.start`** (one rule for every anchor link, O2's
"visible anchor links" made concrete). Flash both fragments when both
are on the target page; the off-page tail simply isn't shown —
never auto-flip twice.

## 10. NOTE — overlap and duplication are legal, verified, and precedented

Reacting over a range already covered by a note or diagnosis, or
reacting twice on the same words: `Annotations::add` pushes freely and
sorts by `range.start` (document.rs:736–754); overlap suppression
(`is_suppressed`, :821–829) gates *diagnoses at pass time only*, never
writer notes. Two identical ranges = two cards, stacked by the packer
like any same-anchor pair; the lane shows both. The mock allows it;
the margin's shipped packer handles it. Build-time care only: none.
(Dedup would be the tool having an opinion about her reactions — P2.)

## 11. NOTE — the exit frame: no double home, no spurious motion, pills not cards

The lane renders iff `cold_read.is_some()`; the margin iff the §4.4
predicate is false — one render switch, no frame shows both (screenshot
test P6; add the rig assert). Reactions are writer notes, so they never
enter `appearing` (editor.rs:1007–1011: "your own keystroke is
instant") — at exit they appear in the margin with no entrance fade,
correct by the same law. No spurious slides either:
`update_lane_motion` runs every root render (editor.rs:16309) including
takeover frames, so reaction ids are already in `lane_tops` before exit
and nothing diffs (verify: `moves_started` unchanged across
`coldread:open → react → escape` — the Phase-5 rig grammar). Reactions
whose anchors sit outside the editor's parked viewport surface as
edge pills, not cards (the cull, editor.rs:14518–14539) — lawful, the
margin's own grammar; don't "fix" it by scrolling the editor (§4.6:
nothing moved).

## 12. NOTE — CardFocus across entry/exit: resolve, then Idle; restore nothing

After case 1's `finish_composing`, the state is `Selected(id)` — a
card that would re-raise door-exempt and cull-exempt on exit
(editor.rs:14502–14527), pinning a stale highlight after the room
change. Entry ends with `deselect_card` semantics (`focus =
CardFocus::Idle`); exit restores no selection — Esc-like, and honest:
the writer left that conversation to go read. One line; make it
explicit in §4.6 so nobody "helpfully" round-trips it.

## 13. NOTE — twin anchor text: precedented, and worst for tiny reactions

A reaction on words that appear twice re-anchors at restore via
covered-substring search from the former offset, falling back to
from-zero (`Annotations::reanchor` document.rs:892–912 →
`diagnose::anchor` diagnose.rs:134–150; repeats resolve in document
order). Identical behavior to every ctrl-m note — accepted,
precedented. The cold read only *sharpens* it: chip reactions are
frequently one word (`? doubt` on "the"), and a one-word needle jumps
twins easily. Case 3's whole-token anchors are the floor; anything
stronger (context-widened matching) is out of scope — the orphan/
detached grammar (I4, "· detached" label, editor.rs:14001–14012)
already catches the miss honestly.

## 14. NOTE — filing into a parse-failed annotations container: pre-existing, not worsened

A corrupt `ANNOTATIONS_CONTAINER` loads as `Annotations::default()`
silently (store.rs:463–470); the first note mutation then persists the
fresh one-note list over the old JSON (store.rs:837–848, fingerprint-
gated) — the corruption made permanent. A cold-read reaction rides
`Document::add_note` exactly like ctrl-m: same exposure, zero new
surface. Verified not worsened; the read even *lessens* the blind spot
marginally (the writer sees her margin empty before entering). A
load-failure eprintln→journal breadcrumb is the cheap future fix;
out of this build's scope.

## 15. NOTE — the 42-char anchor quote: graphemes, flattening, and the mock's UTF-16 trap

The mock truncates with JS `sel.toString().slice(0,42)` — UTF-16 code
units, which splits surrogate pairs (any non-BMP emoji) — and the
codebase's own habit is `chars().take(n)` (editor.rs:3097, 4706–4708),
which splits ZWJ emoji sequences and flag pairs even in NFC. The lane
quote (and any receded one-line row, case 8) must truncate on
**grapheme boundaries** (unicode-segmentation, already in the tree —
research-linebreak §0) and append `…` (U+2026). Cross-block ranges
flatten newlines to spaces before slicing — the exact
`anchor_fragment.replace('\n', " ")` precedent from
`move_note_to_scraps` (document.rs:2291). U+00AD in the covered text
rides along invisibly (zero-width in the card — fine; it IS the
source). An RTL quote renders as gpui shapes it, ragged in the card —
same honest degradation as the page (§2.5); no bidi heroics in a
13 px data line.

## 16. NOTE — reactions across block kinds: text is reactable, metadata is not

Headings (Demi), list items, quotes, and code blocks all paginate as
shaped text fragments → hit boxes, reactions, tint all work; nothing
special. Three deliberate holes, all honest: **list markers** are
painted decoration, not source chars — no hit box, no range (a snap
starting on a marker starts on the first real token). **Running
head/folio** are chrome — dead to selection. **Image captions** are
`BlockKind::Image { caption }` metadata (document.rs:252–256), not
rope text — no manuscript range exists, so a caption cannot carry a
reaction or a tint; the drag passes over it. A drag from the paragraph
above an image to the one below files a range that numerically spans
the image block's source chars — legal (annotations span anything;
`apply_op`/`reanchor` cope), and case 15's newline-flattening keeps
its quote sane. Do not invent caption anchoring in v1.

## 17. NOTE — the wheel over the book must be eaten, not merely unused

§4.3 rules "mouse wheel: nothing in v1", but nothing yet *implements*
nothing: the root `on_scroll_wheel` (editor.rs:7935–7959) will scroll
the hidden editor (silently breaking §4.6's exact-return promise) and
flush the deferred pass (:7944, compounding case 2). Precedented fix,
both layers: the takeover surface takes
`on_scroll_wheel(stop_propagation)` like every overlay
(editor.rs:5046, 7084, 7279), and the root handler adds
`cold_read.is_some()` to its §0.6 second-line guard (:7939). Rig:
`wheel:` token mid-read → dump asserts `scroll_y` and page unchanged.

## 18. NOTE — reaction-input geometry at the page's edges

The input floats ~250 px under the selection (S9 stands). A selection
on the page's last line pushes it over the folio and off the page onto
the desk; near the right margin it can reach the lane. Clamp within
the window and flip above the selection when there's no room below —
the shipped CSD-popover overlap fix is the precedent (commit 471b4db
lineage). Never let it cover the words it quotes (P1: chrome must not
wear her sentence); covering the folio or desk is fine — they are not
her words.

## 19. NOTE — filing bumps revision; the book and the guards already absorb it

`add_note` bumps `Document::revision` (document.rs:2308–2315) while
the book renders the entry snapshot — spec §2.6 already keys the book
off the snapshot, not revision (recon risk 8); verify with the rig
that `coldread:react` changes no page geometry. `mark_dirty` mid-read
saves are lawful (annotations channel only; the save fingerprints keep
text writes at 0 bytes). The draft heartbeat is inert once case 1
resolves the composer at entry. No further machinery needed — this
case exists so nobody "fixes" the revision bump by suppressing saves
inside the read.

---

**Summary.** Nineteen cases: **3 BLOCKER, 6 DECIDE, 10 NOTE.** The
blockers are all silent-wrong-state class: (1) entering over an open
composer lets the blur-commit's `window.focus(&self.focus_handle)`
steal focus back from the takeover — the room opens deaf and arrow
keys move the hidden caret, so entry must `finish_composing` first;
(2) spec §4.4's "results park mid-read" is contradicted by
`deliver_pass`, which defers only during typing bursts — a reading
writer is a lull, so passes integrate into the hidden margin (and the
lull watcher flushes a pre-entry parked pass seconds into the read)
unless the gate learns `cold_read.is_some()` and exit becomes the
flush point; (3) per-fragment hit boxes as spec'd yield half-word
annotation anchors on hyphenated and soft-hyphen words — persisted,
wrong, and twin-jumping at every restore — unless word-snap resolves
to the source *token* shared by both painted fragments. The decides
are one-rule arbitrations (Esc-is-the-only-discard close inventory,
chip+text combination per L21's own exemplar, undo stays guarded,
Past-mode selection dies at mousedown, lane overflow recedes in place,
page-spanning anchors flip to `range.start`); the notes are mostly
verified-precedented behaviors (overlap legality, orphan/twin
grammar, parse-failure exposure) plus build-care items with named
precedents (grapheme truncation, wheel-eating, input clamping). The
single most dangerous case is **#2** — it is the one place the build
spec asserts a behavior the shipped code actively does the opposite
of, and its failure mode (AI cards materializing silently behind the
book, status chrome floating into the reading room) would ship
unnoticed by every test that doesn't run a pass mid-read.
