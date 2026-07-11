# Cold read corner cases — time, persistence, checkpoints, the strip

Lens (b) of the corner round (05-cold-read.md §7). Against the build
spec at e6f8bb3, `cold-read/design-law.md`, `cold-read/recon-code.md`,
08-compost-fresh.md + `compost-fresh/adjudications.md`, and the code on
branch `cold-read` @ e6f8bb3. All file:line anchors verified on this
commit. 2026-07-06.

---

## 1 · Restore pressed inside a Past book: `restore_to_state` neither exits the takeover nor unparks the strip — BLOCKER

**Scenario.** The writer parks the strip at a station, presses the
banner's `Read` verb, the book opens (spec §4.7). She presses the
book banner's **Restore** chip. Spec §4.1 asserts the chip "routes
through `restore_to_state`, which exits the takeover itself."

**What goes wrong.** As shipped, `restore_to_state`
(editor.rs:2369–2419) exits only the *preview* channels: it calls
`exit_history` (editor.rs:2073–2077), which clears `history_view` +
`history_preview` — nothing else. Two independent failures:

- It cannot know about `cold_read` (a field this build introduces).
  Unless the build adds the exit, the takeover survives the restore:
  the book keeps showing the pre-restore checkpoint while the live
  document underneath was just swapped — the screen lies about the
  document, and the Restore chip is still up, armed to fire again
  (another "Before restore"/"Restored" pair per click).
- It does not touch the strip. Only `strip_restore`
  (editor.rs:2856–2883) performs the strip epilogue (`parked=false`,
  `pos_ms=now`, `scratch=None`, `history_preview=None`,
  `strip_bake(now)`). A book Restore that calls `restore_to_state`
  directly leaves `strip.open && strip.parked == true`, so after the
  takeover exits: every mutation guard refuses typing on the live
  restored document (`is_parked()` gates at editor.rs:5354, 5384,
  7665, 7786, 7886, 7910, 9356…), the parked banner
  (`render_strip_banner`, editor.rs:12850) renders over the live doc
  showing a **stale** moment from a bake that predates the restore
  (the stability law: only open and Restore re-bake — editor.rs:2470),
  and the fabric shows a pre-restore history. Recoverable by Esc
  (editor.rs:6997–7003 → `strip_return_to_now`), but the frame it
  ships is a lying banner plus a mutation lockout.

**Resolution.** Make the spec's sentence true in code, both halves:
(1) `restore_to_state` drops the takeover (`cold_read = None`) in the
same breath it drops previews — the one shared restore path stays the
one place every read-only surface dies (I6; P7: Restore is a real
button doing the strip's Restore). (2) When the strip is open at
restore time, run the strip epilogue too — either route the book's
chip through `strip_restore` when `strip.is_parked()`, or hoist
`strip_restore`'s tail (unpark, snap to now, re-bake) into
`restore_to_state` behind `if strip.open`. Caret after: `0..0` per the
shipped restore contract (editor.rs:2387) — the takeover's
"caret/scroll exactly as left" promise (§4.6) applies to Esc, not to
Restore, which is a document swap. Rig: Past read from parked strip →
`strip:restore`-equivalent on the book chip → dump asserts
`coldread.open=false`, `strip.banner=false`, and a keystroke lands.

**Severity: BLOCKER** (ships a lying frame and an input lockout via
the spec's own recommended wiring).

## 2 · A Past read of a Top-era (or pre-Scraps) checkpoint: the state's own manuscript slice does not exist yet — BLOCKER

**Scenario.** "Read this version" on a checkpoint recorded before the
Scraps flip. Its `BlockMap` carries `aside_boundary` —
`BoundaryEra::Top` (document.rs:265–269, 394–397): the pile is the
text *before* the boundary, the manuscript *after* it. A checkpoint
older still carries no boundary at all (whole doc = manuscript).

**What goes wrong.** The live read consumes
`Document::manuscript_slice()` — a method on the live `Document` that
era-branches and returns the rebased `(text, spans, kinds)` triple.
The Past read gets its state from `checkpoint_state(cp)`
(store.rs:693–698) — a raw triple with no slicing. The only
free-standing helper is `manuscript_range_of(&rope, &blocks)`
(document.rs:76–88), which is era-correct but yields a *char range of
text only*. A naive Past-read implementation that paginates the whole
state text — or slices the text but forgets to rebase spans by the
base and truncate/offset kinds — ships a lie: on a Top-era state the
**book opens with the compost pile as its first pages** (Top era: the
pile is at the top), the banner's word count includes it, and
bold/italic runs and BlockKinds land on the wrong paragraphs. This is
the cold-read echo of the compost round's era blocker
(corners-time-persistence.md §1), and it violates L26/L27 ("the read
ends at the scrap line"; "the history-preview variant shows the past
state's own geometry") and adjudications Scopes 4 + 6 verbatim.

**Resolution.** Wave A builds a free function
`manuscript_slice_of(text, spans, blocks) -> (String, SpanSet,
Vec<BlockKind>)` era-branching exactly like
`Document::manuscript_slice` (Top: slice after the separator, rebase
spans by base, kinds from `b+1`; Tail: truncate at the seam; None:
whole), unit-tested on Top-era, Tail-era, no-boundary, and the
degenerate Top-era "everything is compost" state (empty manuscript →
one blank page, `— 1 of 1 —`, no crash; only past states can be empty
this way — the regions lens owns the live-textless page). The Past
banner's word count uses the same range — the precedent is already in
the tree: `strip_word_count_at` counts against the state's own
boundary via `manuscript_range_of` (editor.rs:2824–2849, scopes-search
6). The live path and the Past path must share this one function so
the two books can never disagree about where a piece ends.

**Severity: BLOCKER** (a silent wrong-book on every legacy file; the
`seed:legacy` gate should catch it only if the fixture gains a
Top-era-boundary checkpoint — add one).

## 3 · The banner's station segment can quote a machine name as the writer's words — including "Cold read" itself — DECIDE

**Scenario.** `ColdRead.station` = "newest checkpoint name at entry"
(spec §1). The entry sequence has just run `add_checkpoint_if_changed
("Cold read", false)` — so on any read entered with changes pending,
the newest checkpoint at entry IS "Cold read". On other days it is
"Session", "Restored", or "Checkpoint 3".

**What goes wrong.** The banner renders **Reading** · "Cold read" ·
4,120 words — the mode word duplicated as a quoted station — or
**Reading** · "Session" · …, a system label wearing the writer's
typographic quotes. L14 is explicit: the station name is "the
writer's own words in typographic quotes." P8 bans system prose
posing as data; history-strip.md §2 keeps reflex checkpoints
deliberately unnamed. Capturing the name *before* the entry
checkpoint doesn't fix it (a previous read's "Cold read" checkpoint
may still be newest).

**Resolution.** The station segment shows the newest checkpoint whose
name is the writer's own — i.e. `manual == true` (the strip's own
law: "a manual checkpoint always carries a writer's own name,"
strip.rs:666–670; `rename_checkpoint` sets manual, store.rs:629–653).
Walk the list newest→oldest, take the first manual name; none →
segment omitted (L14's "absent → segment omitted"). All automatics
("Session", "Cold read", "Before restore", "Restored", "Exported",
"Checkpoint N") are skipped by the same one rule, present and future.
Rig: seed a manual "Draft complete" + a later automatic "Session" →
banner shows "Draft complete".

**Severity: DECIDE** (one rule, needed before build — the spec's
literal sentence produces the lie on the very first read).

## 4 · `Read it cold` fired while the canvas shows the PAST (parked strip, history panel): subject switch + destroyed place — DECIDE

**Scenario.** The writer parks the strip at Tuesday and is reading
Tuesday's text in the flat preview. `Read it cold` is App-scoped
(section View → App context, commands.rs `Command::global`), so
ctrl-shift-l fires. Spec §4.6: "If the strip is open (parked or not) a
Live entry closes it first." The history panel (`history_view` +
its preview) is not in the close-first list at all.

**What goes wrong.** Two ways: (i) *subject switch* — the screen
showed Tuesday; the verb checkpoints and opens a book of TODAY's
manuscript. The tree's own precedent forbids operating on a document
the screen isn't showing: the editor-menu rows go inert on exactly
this predicate (editor.rs:15868, review H33), and the spec itself
extends that gate to the menu (§4.4). (ii) *destroyed place* —
`close_strip` (editor.rs:2452–2464) wipes `parked/pos_ms/scratch/
bake`; the strip reopens at now; the writer's Tuesday position is
gone. Closing a strip that is merely open **at now** is harmless
(that is what Esc does); closing a *parked* one throws away where she
was standing.

**Resolution.** Split §4.6's sentence by what the canvas shows. Strip
open at now (not parked) → close it and enter (nothing lost). Canvas
showing the past (`strip.is_parked() || history_preview.is_some()` —
the panel included) → the Live verb **guards with the pulse** (§4.5's
own v1 default: guard entry surfaces too), leaving the writer one Esc
from a Live entry — or one press of `Read` from the book of the state
she is actually looking at. This also makes §4.6's Esc symmetry ("the
parked strip stays parked") reachable from the state it describes.

**Severity: DECIDE** (the spec's current sentence is destructive and
H33-inconsistent; needs the amended rule before Wave B).

## 5 · What the parked banner's `Read` verb binds to; stateless legacy checkpoints must never offer it — DECIDE

**Scenario.** §4.7 puts a quiet `Read` beside Restore in the parked
banner. But the playhead parks *anywhere* on a continuous timeline,
and "the book binds to checkpoints only (v1)."

**What is undefined / goes wrong.**
- *Arbitrary moment:* parked between checkpoints, which state would
  `Read` even open? Nothing in the spec says.
- *The at-station window is label-based:* the banner's station
  detection matches only stations with non-empty labels within 5
  working px (editor.rs:12860–12867). Reflex ticks — "Checkpoint N"
  and the newly-blanked "Cold read" (case 9) — have empty labels and
  can never match, so a label-window gating rule silently denies
  `Read` at exactly the checkpoints reads create.
- *Stateless checkpoints:* on a legacy file mid-backfill,
  `checkpoint_state` falls back to `state_at` (store.rs:693–698) — a
  synchronous historical checkout measured at 5–7 s per checkpoint
  (the history-sidebar hang the materialization work killed). The
  strip's own anchors already exclude stateless checkpoints for this
  reason ("a stateless one can't base a reconstruction,"
  strip.rs:603–613), and the panel's Restore refuses while
  materializing (editor.rs:2343–2345).

**Resolution.** `Read` renders iff the playhead resolves to a
checkpoint **with a materialized state**: nearest station tick within
the banner's existing 5-px working-px window, *regardless of label
rank* — identity travels as `at_ms`/checkpoint index in the bake,
never as the label string (labels are display, not identity; ties —
e.g. "Before restore"/"Restored" seconds apart — take the nearest x).
No qualifying checkpoint → no verb: the control's absence IS the
"this moment has no book" message (P12), exactly like the withdrawn
flip zone at the book's edges. Never call `state_at` from the banner;
legacy files grow the verb as the backfill lands (editor.rs:
2034–2071) — the sidebar's shipped degradation. And the Past entry
runs **no entry-checkpoint sequence**: L3's "every cold read starts
from a recorded state" is about Live reads — a Past read starts from
a checkpoint by construction, and the restore bracket ("Before
restore"/"Restored", editor.rs:2385/2415) already records the present
if she restores.

**Severity: DECIDE** (§4.7 explicitly delegates this to the corner
round).

## 6 · The misfiled reaction has no inverse inside the read (P13) — DECIDE

**Scenario.** The writer selects a phrase, means `! alive`, clicks
`~ drags` — chips file immediately (§5.2). Or she files typed text
with a typo she sees the moment the card lands in the lane.

**What goes wrong.** Undo is guarded inside the read (§4.6 names
undo in the mutation list; the parked precedent at editor.rs:
7885–7889). The note body is editable only on the desk (§5.3's
afterlife). The lane card's only behavior is the anchor flip (§5.4).
So inside the reading room the one verb the room offers — react —
has **no inverse in any grammar** until exit: a P13 violation at the
exact surface where the verb lives, and a real flow wound (the writer
must abandon the read to fix a misclick, then re-enter at page 1 —
research-page §4.6).

**Resolution.** Two candidates: (a) the lane card carries the
margin's own dismiss affordance — the same `set_note_status` the desk
uses; the reaction dies "by the margin's own dismiss/fade grammar,"
which is precisely the inverse P13 already assigns it (design-law
§4-P13, I4/L22: reactions resolve alongside cards with the margin's
shipped grace). (b) A narrow undo pierce: ctrl-z un-guards only when
the top undo atom is a reaction filed this session, else pulse.
Recommend **(a)**: it is the inverse the note keeps for the rest of
its life, needs no carve-out in the refusal law ("one pulse idiom,
never a silent swallow" keeps zero exceptions), and typo-fixes remain
desk-work by §5.3 (dismiss + refile is two gestures, on the page, in
grammar). Named residue: under ~900 px the lane hides (§5.4) and the
only inverse is exit-to-desk — accepted, the lane's absence already
parks reactions to the desk.

**Severity: DECIDE.**

## 7 · Journal events for entering/leaving a read: recommend NONE — DECIDE

**Scenario.** The seam is journaled (`JournalEvent::Seam`,
journal.rs:59–77, adjudications "The seam is journaled"); AI passes
are journaled and drawn as veils. Should a cold read be a
`JournalEvent` too?

**Analysis.** No, on three grounds. (1) *The fabric's quant is
words* (history-strip.md §1): a read adds zero words; "the fabric's
warmth is the standing proof the machine never writes" — every mark
class on the strip is either words, machine reach (veils), card
lifespans, or state ticks. A "writer read" mark is a new class with
no caption (the v2 design gate: a mark you can't caption in one
sentence is a mark you can't ship). (2) *Nothing consumes it*:
`ReplayDoc` reconstruction needs no read events; the entry checkpoint
already leaves the only navigationally useful trace, and only when
the state actually changed — which is the fingerprint law working,
not a gap. The AI-pass veil analogy fails: veils audit the
*machine's* reach, a trust datum; recording the writer's own reading
is the tool watching her read (P2's shadow). (3) *Reactions are
already timestamped records* (`created_unix` on every note) — the
read that produced them is legible from its own artifacts, like every
other sitting. Forward-compat is NOT the obstacle either way:
journal events persist one JSON item per event and unknown kinds are
skipped per-item (`filter_map(.ok())`, store.rs:284–291), so a
future variant degrades gracefully — the decision is pure design, and
the design answer is no event, no strip mark. Record the ruling so a
later analytics impulse must overturn a written adjudication.

**Severity: DECIDE** (an open question the spec's agenda names;
wants one adjudication line).

## 8 · Enter/exit/enter checkpoint arithmetic: last-only dedupe verified sufficient — NOTE

**Scenario walkthrough** against `seal_session_with` (store.rs:
668–688 — full `(text, spans, blocks)` equality vs the LAST
checkpoint only):

- *Edits → enter → exit → enter (no edits between):* first entry
  records "Cold read"; second entry compares equal to it → no
  checkpoint. Total one. Correct.
- *Enter → exit → edit → 900 s idle → enter:* the idle sealer
  records "Session" (editor.rs:1679–1685); entry then compares equal
  to "Session" → no "Cold read" checkpoint. L3's guarantee — "every
  cold read starts from a recorded state" — still holds: the state IS
  recorded, under another name, and the name is load-bearing nowhere
  (both rank as reflex; churn heat is evicted from the read, L23).
- *Reactions filed during a read:* annotations are not part of
  `CheckpointState` (`text, spans, blocks` — store.rs:82–86), so a
  reaction-only read leaves no checkpoint on re-entry. Correct: notes
  have their own persistence and afterlife; a checkpoint is a text
  moment.

**Build care.** Entry should also set `dirty_since_checkpoint =
false` when it seals (mirror the manual handler, editor.rs:
1913–1917); harmless if forgotten (the idle sealer would no-op on
equality) but keeps the flag honest. Rig: `coldread:open`, Esc,
`coldread:open` → checkpoint count grew by exactly one.

**Severity: NOTE** (verified correct; recording the walkthrough so
nobody "fixes" the dedupe into per-name comparison).

## 9 · "Cold read" as a checkpoint name is data forever: old builds label it; the new build needs display AND rank arms, both manual-aware — NOTE

- *Old builds (shipped strip):* `station_rank` falls through to
  `RANK_REFLEX` for unknown non-manual names (strip.rs:694–697) —
  correct rank; but `station_display` blanks only `"Checkpoint "`
  prefixes (strip.rs:703–709), so "Cold read" renders as a visible
  labeled tick, and the parked banner will curly-quote it inside the
  at-station window. No misbehavior, no data damage — **verified
  harmless**; the cost is cosmetic label noise (one per
  edited-between read, losing every collision at rank 6).
- *New build:* the spec's "new `station_rank` arm" (§4.6, decision
  8) is NOT sufficient for L3's "unnamed bare tick" —
  `station_display` must also blank the name, or every read is a
  *labeled* station. Both arms must check `!manual`: a writer is
  free to rename a checkpoint to "Cold read" (`rename_checkpoint`
  sets `manual=true`, store.rs:629–653), and her own words must keep
  writer rank and display. (The existing arms have this latent bug —
  a writer-named "Restored" is demoted today; don't copy it.)
  `station_display` takes only `name` — its signature grows the flag
  or the blanking moves to the caller.
- *Knock-on:* after blanking, parking exactly on a cold-read tick
  shows `format_moment` in the banner, like every reflex tick —
  correct; and the label-window `Read` gating consequence is case 5.

**Severity: NOTE.**

## 10 · Legacy file mid-backfill: entry records no checkpoint — accept the precedented degradation — NOTE

**Scenario.** A pre-materialization file opens;
`backfill_checkpoint_states` starts in the background (editor.rs:
1652–1660). The writer enters a cold read before it lands. The last
checkpoint has `state: None`, so `seal_session_with` returns early
("legacy checkpoint mid-migration — seal next launch," store.rs:
674–677): the entry records nothing.

**Resolution.** Accept: the identical deferral already governs
session sealing; the alternatives are worse (blocking entry on a
multi-second backfill, or `add_checkpoint` unconditionally —
duplicate states, fingerprint economy broken). The book itself is
correct regardless — the snapshot comes from the live doc; only the
record's tick is missing, once, on legacy files, and the next launch
seals. Name the degradation in the build commit. (Post-compaction
files cannot be stateless — `compact_on_open` runs only when every
checkpoint is self-contained, store.rs:207–218 — so this is strictly
the mid-backfill window.)

**Severity: NOTE.**

## 11 · Mid-read autosave / idle-seal machinery: verified harmless; keep the entry's save-then-checkpoint order — NOTE

The 1 s heartbeat (editor.rs:1661–1692) keeps running under the
takeover and every branch is benign: `sync_active_note_draft` no-ops
(entry closes the composer; the reaction input is not the composer);
`save_now` fires only when a filed reaction marked dirty — the
reaction-filing path must call `mark_dirty()` + `bump_activity()`
like `add_note` does (editor.rs:3224–3227) — and a changeless save is
0 bytes by the save-channel fingerprints; the 900 s idle seal
compares equal to the entry state (reactions move no text/spans/
blocks) → no-op. A 40-minute read shows on the strip as a folded gap
seam — honest: no words happened (history-strip §1). System sleep
mid-read: timers resume, `Instant`s are monotonic, nothing depends on
wall-clock continuity except the banner date (case 12). One ordering
care: entry runs `save_now()` BEFORE `add_checkpoint_if_changed` (the
`restore_to_state` pattern, editor.rs:2377–2385) so the checkpoint's
frontiers and materialized state align with settled, saved reality —
keep that order, and budget entry latency as save + fingerprint
compare (an O(doc) equality per entry) + pagination, not pagination
alone.

**Severity: NOTE.**

## 12 · The Past banner's date must be computed at render, never baked at entry — NOTE

`date_label` (strip.rs:844–861) says "Today"/"Yesterday" — relative
words that go stale across midnight. `ColdRead` stores `created_unix`
(good); it must not also store the formatted string: a read crossing
midnight would show yesterday's checkpoint as "Today" until exit.
Format in the render fn from `created_unix` + fresh now (the strip's
own convention, `date_label(t, now)`); a stale frame then
self-corrects on the next notify (flip, pulse). The relative words
themselves are lawful — history-strip §2 fixes "Today"/"Tue 1 Jul" as
the product's real-date grammar; the lab's "Sun 6 Jul" is one
rendering of it, and the year appears automatically on old
checkpoints ("histories never expire").

**Severity: NOTE.**

## 13 · Past read from the parked strip: the strip hides but is not closed, its place survives, and the takeover's refusal must outrank the parked guard — NOTE

While `cold_read.is_some()` the root renders the book: the
StripElement band, thumb, and flat parked preview must not paint (two
chrome layers over one truth; a reachable thumb would scrub
`history_preview` under a book keyed to a different state — a lying
screen). But strip STATE — `open, parked, pos_ms, pin_ms, scratch,
bake, view_offset` — is untouched, so Esc returns the identical
parked frame (§4.6: "the parked strip stays parked"), Compare pin
included. Two build invariants:

- *Guard order.* Every mutation entry checks `strip.is_parked()`
  first and pulses the STRIP banner (pulse_strip, editor.rs:
  2775–2800). Inside a Past-from-parked read that banner is hidden —
  a stray mutation would flash invisible chrome (a silent swallow,
  the exact thing L18 bans). The ColdRead guard must sit ABOVE the
  parked guard at every mutation entry (or the takeover's key
  context + focus must consume those events wholesale so they never
  reach the handlers).
- *Rig.* `strip:scrub:…` → Past read → Esc → dump asserts
  `strip.parked == true` and `pos_ms` unchanged; wshot before/after
  the round trip pixel-identical.

**Severity: NOTE.**

## 14 · Crash mid-read: the takeover's only volatile datum is the un-filed reaction line — NOTE

Inventory, verified: book layout (derived from the snapshot,
recomputed on entry), page index (deliberately unpersisted — always
enter at page 1, research-page §4.6), selection (transient by design,
collapses on flip/resize/Esc, §5.1), the reaction input's typed line
(lost on crash AND on Esc — §4.6 close-and-clear; one line max).
Filed reactions are ordinary notes: `mark_dirty` + the 1 s heartbeat
persists them within ~1 s (editor.rs:1673–1676); a crash inside that
second loses at most the newest note — the same exposure every edit
has. The composer's keystroke-durability heartbeat
(`sync_active_note_draft`) deliberately does NOT extend to the
reaction input: the input creates its note only on commit (unlike
ctrl-m's create-then-compose). Accept the asymmetry — a one-line
reaction is not a composition — and record it so nobody "fixes" it
into a draft-heartbeat that would file half-typed reactions as real
notes after a crash.

**Severity: NOTE.**

## 15 · Re-entrancy and double-fire guards — NOTE

`Read it cold` is App-scoped and pierces the ColdRead key context:
inside a read, ctrl-shift-l must guard with the pulse (§4.5's
default) — a second entry would re-run save/checkpoint/pagination and
snap the reader back to page 1, the "performance starts from the top"
rule weaponized by a stray chord. The book's Restore chip needs an
idempotency guard (first click exits the takeover; a double-click
must not restore twice — the strip chip's `is_parked()` early-return
is the precedent, editor.rs:2857). Same class: `AddCheckpoint`
(ctrl-alt-s) inside a read would record an unconditional duplicate
"Checkpoint N" (store.rs:559 has no fingerprint guard) — the
per-chord table (lens c) should list it guarded. Rig: a doubled
`coldread:open` token asserts one takeover, one checkpoint.

**Severity: NOTE.**

---

## Summary

Fifteen cases: **2 BLOCKER, 5 DECIDE, 8 NOTE**. The blockers are both
"the spec's sentence is not yet true in the tree": (1) the book's
Restore chip routed through `restore_to_state` exits neither the
takeover nor the parked strip — shipping a stale parked banner over
the live restored document plus a full mutation lockout — and (2) no
free-standing era-aware manuscript-slice-of-state exists, so a Past
read of a Top-era checkpoint would open the book with the compost
pile as its first pages, the compost round's era blocker echoed one
feature later. The decides are one filter (the banner's station
segment must never quote a machine name — including "Cold read"
itself, which the entry sequence guarantees is the newest checkpoint),
one guard split (Live entry while the canvas shows the past should
pulse, not close-and-switch-subject — and not destroy the parked
place), one binding rule (`Read` only at materialized checkpoints,
resolved by tick identity not label, never through `state_at`'s
multi-second checkout), one P13 repair (a misfiled reaction needs the
margin's own dismiss inside the read), and one adjudication to write
down (no journal event for reads — words are the quant, a read adds
no words, and the entry checkpoint plus the reactions' own timestamps
are the whole honest record). The notes verify the quiet machinery
holds: last-checkpoint-only dedupe is sufficient for L3 in every
interleaving including reaction-only reads (annotations are outside
`CheckpointState`); "Cold read" as persisted name is harmless on old
builds but needs manual-aware rank AND display arms on this one;
mid-read autosave/idle-seal walk through clean; the parked strip
survives a Past-read round trip only if the takeover's refusal
outranks the parked guard; and a crash can eat at most one un-filed
line by design.
