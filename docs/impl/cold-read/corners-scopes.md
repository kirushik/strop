# Corner cases — scopes, verbs, chords, copy (cold read)

Domain: the per-chord pierce table for every App-context binding, the
refusal architecture (one pulse idiom, never a silent swallow —
acceptance.md §3), and what copy means inside a justified, hyphenated,
paged rendering. Code read at `cold-read` @ e6f8bb3; gpui dispatch
verified in the fork at `/home/kirushik/Code/Thirdparty/zed`
(window.rs, keystroke.rs). Law: 05-cold-read.md §4.5 ("v1 default:
guard with the pulse; corner round owns the per-chord table"),
design-law.md L18/L25/L30, the parked-mode precedent (recon §1:
mutations pulse, copy allowed, navigation free), P2/P11/P12/P13.
2026-07-06.

Mechanical ground truth, verified once, used throughout:

- The whole window sits under `key_context("App")` (editor.rs:16347);
  every `Command::global()` chord (commands.rs:42–54) binds to that
  root context (bind_keys, editor.rs:488–508) and therefore fires from
  ANY focus — a `Some("ColdRead")` focus context does not stop them.
- `Editor`-context bindings (all text mutations + raw editing keys,
  editor.rs:509–563) require the editor column on the dispatch path
  (editor.rs:16422 `key_context("Editor")` + `track_focus`); with the
  takeover focused they are simply UNBOUND — a silent swallow, not a
  refusal, unless we bind them ourselves.
- gpui dispatch order (fork window.rs:4746–4860): key BINDINGS match
  and consume first; only unbound keystrokes reach `on_key_down`
  listeners (`finish_dispatch_key_event`). A binding on a deeper
  context outranks a shallower one. Lone modifier presses arrive as
  `ModifiersChangedEvent` (window.rs:4718–4739) and never reach
  key-down listeners at all.
- `Keystroke.key_char: Option<String>` (fork keystroke.rs:18–32) is
  the "this press would have typed text" signal.
- `ctrl-q` → Quit binds with context **None** (main.rs:134) — it
  matches unconditionally, everywhere, always.

---

## 0 · The pierce table (DECIDE — this is the adjudication spec §4.5 delegated)

Rulings use three verdicts: **guard** = refuse + pulse the banner's
bold mode word (L18; one pulse idiom); **allow** = acts normally;
**exit** = the chord IS an exit. "Past" = the history variant (§4.7).
Unless a cell says otherwise, Live and Past rule identically.

### App-context chords (pierce by construction — each needs an explicit ruling)

| chord | verb (commands.rs) | ruling | why |
|---|---|---|---|
| ctrl-shift-p | Open Command Palette | **guard** | the room has no tools (spec §6.6's own logic); a menu over the page breaks P11/L25 |
| ctrl-f | Find in Document | **guard** | settled: spec §6.6 (O12) |
| ctrl-h | Find and Replace | **guard** | find surface + a mutation surface, doubly out |
| ctrl-alt-h | History (strip) | **guard** | in a Past read entered from the parked strip, `toggle_strip` would `close_strip` (editor.rs:2452–2464) UNDER the takeover — killing the "Esc returns to the parked strip" contract (§4.6) and clearing `history_preview` state the room's source may share |
| ctrl-shift-d | Run Editorial Diagnosis | **guard** | H33/I5: never diagnose a document the screen isn't showing; `run_pass` also force-opens the door (`drafting = false`, editor.rs:3548) and flushes a parked pass (:3547) — three mutations from one pierced chord |
| ctrl-shift-b | Run Believing Pass | **guard** | same path (editor.rs:3387–3389). The believing pass is asked AFTER the read (L24), not from inside it |
| ctrl-shift-r | Drafting / Reviewing | **guard** | "the door state is untouched" (spec §4.4); `toggle_door` also flushes a parked pass (editor.rs:3404) — mid-read flush violates the deferred-pass parking law |
| ctrl-shift-e | Export as Markdown | **guard** | an audience surface (L26); would silently write files + journal an Export event (editor.rs:2941–2951) while the writer is performing a read |
| ctrl-shift-s | Save a Copy As… | **guard** | a tool with an OS dialog over the book; `save_now()` inside it (editor.rs:3166) is harmless but the surface is not |
| ctrl-shift-o | Scraps (travel) | **guard** | scraps do not exist in the room (I1); the verb moves the hidden caret + scroll (editor.rs:6033–6044), breaking the "nothing moved" exit contract |
| ctrl-alt-g | Toggle Graveyard | **guard** | scrolls the hidden desk to document end (editor.rs:6484–6491) — same exit-contract break |
| ctrl-alt-s | Name a Checkpoint | **guard** | `add_checkpoint` is NOT fingerprint-guarded (store.rs:559 vs :658): it would stamp a duplicate manual tick right on top of the entry "Cold read" tick. The entry checkpoint already recorded this moment (L3); the room needs no second shutter press |
| ctrl-? | Keyboard Map | **guard** | a cheatsheet of mostly-guarded chords overlaying the page is an inert menu (P12) and a second surface (L25); the banner already names the one live exit |
| f2 | Rename Document… | **allow** | a file act, not a text act — the writer's hands on her own file (case 6) |
| ctrl-n | New Document | **allow** | opens a new window/process (editor.rs:4539–4541); this room is untouched |
| ctrl-o | Open Document… | **allow** | new process (editor.rs:3143–3160); the OS file dialog is not a strop surface |
| ctrl-shift-l | Read it cold | **exit** | toggle precedent (`toggle_strip`, `toggle_palette`): the verb pressed inside its own mode exits it — chord-symmetric P13. Past variant: also exits, to where it was invoked |
| ctrl-q | Quit | **allow** | verified unblocked (case 9) |

Chordless globals (Reveal in Files, Copy Document Path, History panel,
Set Session Goal…, the three Diagnosis Mode rows, Set Up AI Provider…,
Test AI Connection, Cancel AI Run, Open Welcome Guide): reachable only
through the palette, which is guarded — moot. Note `Cancel AI Run` is
therefore unreachable mid-read; that is correct — a running pass
doesn't block entry and parks its result (spec §4.6), it should not be
cancellable from a room that pretends the machine isn't there.

### Editor-context chords (unbound inside the takeover — must be BOUND to the refusal, or they swallow silently)

Undo/redo (ctrl-z, ctrl-shift-z, ctrl-y), formatting (ctrl-b/i/u/e,
ctrl-shift-x/h, ctrl-.), structure (ctrl-1..3, ctrl-alt-1..3,
ctrl-alt-q/c/f, ctrl-shift-7/8), scraps verbs (ctrl-shift-a,
ctrl-shift-g), ctrl-m, cut/paste (ctrl-x, ctrl-v, shift-delete,
shift-insert), enter/backspace/delete, f10 (palette's Editor-scoped
alias, editor.rs:563): **all guard with the pulse.** The mode-matrix
gate demands a pulse for every blocked VERB, and "unbound" is a
silent swallow, not a refusal. Two exceptions:

- **ctrl-c and ctrl-insert: allow — live copy** (case 2). The parked
  precedent is explicit (editor.rs:7760–7781: "Copy MUST work while
  parked").
- **ctrl-a: guard in v1** (case 3).

Optional named depth, not v1: ctrl-m with an active page selection
could open the reaction input (keyboard parity with mouseup — the
ARIA both-modalities rule the popover already honors, editor.rs:7010).
Moot until a keyboard selection exists; recorded so the parity gap is
a known one.

### ColdRead-owned bindings

escape (two-level, I3) · left/right/space/pagedown → flip forward &
back per research-page §4.2 · shift-space/pageup ← back · home/end
first/last. **Add ctrl-home/ctrl-end as aliases of home/end** —
DocStart/DocEnd muscle memory (editor.rs:537–538) will be pressed in
the first minute; up/down stay inert (not verbs, no pulse).

**Severity: DECIDE** (ratify the table). Everything below details the
cases the table's one-liners can't carry.

---

## 1 · The guard architecture: two leak paths make handler-side guards non-optional (BLOCKER)

**Scenario A.** The build guards chords by giving ColdRead its own key
context and assuming focus scoping contains the rest. It doesn't: all
16 App chords bind to the ROOT context and fire from any focus (the
recon flagged this; here is the concrete blood): ctrl-shift-d inside
the read runs a real AI pass on the manuscript, force-opens the door,
and flushes a parked pass — three state mutations while the screen
shows a book.

**Scenario B — the one nobody wrote down.** Field overlays in this app
mount ON THE ROOT, outside the column ("Field overlays mount on this
root", editor.rs:16348–16350). If the reaction input follows that
convention, its dispatch path is `root(App) → NoteInput` with **no
ColdRead node between** — so even a perfect set of ColdRead-context
guard bindings falls off the path the moment the input has focus, and
ctrl-shift-p typed into the reaction input opens the palette over the
book.

**Resolution** (belt and braces, all three):

1. **Registry-driven context guards.** In `bind_keys`, one loop over
   `commands::all()`: every command with `keys` — global or not —
   whose ruling in §0 is "guard" binds its chord in the `ColdRead`
   context to a single `CrRefuse` action whose handler pulses. Deeper
   context outranks "App" (verified, window.rs dispatch), so the App
   action never fires; Editor-scoped chords get their refusal the same
   way. Anti-drift property: a FUTURE command added to the registry is
   guarded by default unless the allow-list (f2, ctrl-n, ctrl-o,
   ctrl-c, ctrl-insert, ctrl-shift-l) names it — the same
   single-source-of-truth argument commands.rs:1–6 makes for the
   keymap.
2. **Handler-side guards** (`if self.cold_read.is_some() { pulse;
   return; }`) on every handler reachable by MOUSE or by a
   non-ColdRead focus path: `find`, `toggle_palette`, `toggle_strip`
   (titlebar clicks, case 7), plus the shared mutation sinks
   `apply_replace` / `cut` / `undo` / `redo` / `scraps_travel` —
   ordered BEFORE their `is_parked` checks (case 8).
3. **Mount the reaction input inside the ColdRead-context element**,
   not on the root — closing scenario B twice over.

Failure if unhandled: a palette, a pass, or the strip operating a
document the screen isn't showing — the exact H33 wound, silently.
Law: acceptance.md §3 (mode matrix), H33, spec §4.5.

## 2 · Copy: dead by default, and corrupt if fixed naively (BLOCKER)

**Scenario.** The writer drags a selection on the page and presses
ctrl-c. Today's plan ships NOTHING: ctrl-c binds in the "Editor"
context only (editor.rs:547) and the editor's `copy` handler reads
`self.selected_range` — the hidden desk's selection, not the book's
(`ColdRead.selection` is its own field, spec §1). Inside the takeover
the chord is unbound → silent swallow. And the obvious quick fix —
concatenate the painted fragments the selection touches — ships
corruption: painted fragments have soft hyphens STRIPPED, an inserted
U+002D at every line break, and **no spaces at all** (gaps are
arithmetic, never painted — research-linebreak §0/§6). Fragment
concatenation of two lines yields
`thequickbrown-fox` — spaceless, hyphen-mangled clipboard text pasted
into some other program days later, unattributable.

**Resolution — the copy-source law.** Bind ctrl-c AND ctrl-insert in
the ColdRead context to `cr_copy`, which slices the **source
snapshot** by the selection's manuscript char range:

- Selection state is already manuscript-space (word-snapped char
  range, spec §5.1: hit boxes carry manuscript char ranges) — so
  `snapshot_text[char_to_byte(start)..char_to_byte(end)]`, nothing
  else. Live read: the entry `manuscript_slice` snapshot (identical
  to the live rope by the read-only invariant, but the snapshot IS
  the page — slice it). Past read: the checkpoint state's own
  manuscript slice.
- Consequences, all correct by construction: a word hyphenated across
  a line break copies whole (its two fragment boxes union to one
  source range); an author's U+00AD copies verbatim (source fidelity
  — the editor's own copy does the same); inter-word spaces and
  paragraph `\n`s come from the source; the bound «слово —» fragment
  contributes its real source text.
- Empty selection: no-op, silent (the editor's own precedent,
  editor.rs:7761–7763). Copy never pulses — it is an allowed verb.
- **Primary-selection parity** (NOTE folded in): page selections must
  ride `publish_primary`'s contract (editor.rs:5667–5684) — PRIMARY
  on Linux + `auto_copy_selection` — with the same source slice, or
  middle-click paste elsewhere carries stale text while ctrl-c
  carries fresh: two clipboards disagreeing about one selection.
- Wave-A test: select across a hyphenated line break; assert the
  clipboard equals the source substring, contains a space between
  words, contains no U+002D that the source lacks.

Law: the parked copy precedent (Bug B, editor.rs:7764–7769 — lifting
words out of a read-only surface is lawful), P3 (what the writer takes
is text, real text), seam-mechanics 1 (copy is source-honest).

## 3 · Select-all inside the read (DECIDE)

**Scenario.** ctrl-a. In the editor it is region-scoped with the
recorded ctrl-A+ctrl-A whole-doc exception "an audience surface
wearing a hand's glove" (editor.rs:7556–7572; adjudications,
seam-mechanics 8). In the book there is no caret and no region — what
does select-all even mean? Whole manuscript (spans pages, mostly
invisible, would raise no input since the input raises on mouseup
only)? Current page?

**Resolution: guard with the pulse in v1.** The room has no tools
(§6.6); whole-piece copy is two keys away through Esc. The recorded
scraps exception does NOT carry into the room — that ruling was about
a caret standing in editable text; the room has no caret to wear the
glove. Name the v2 candidate once, so it isn't re-invented: ctrl-a =
select the current PAGE's words (the page is the object the room
offers, P7), copyable via case 2, never raising the reaction input.
Failure if unhandled: an unbound ctrl-a is a silent swallow (gate
violation). Law: seam-mechanics 8 (the exception's reasoning), P5.

## 4 · What exactly is "typing", mechanically (DECIDE — ratify the rule)

**Scenario.** Spec §4.2: typing pulses. But the takeover registers no
input handler (no caret → no text-input context — correct, and it
also keeps IME preedit from ever engaging: `accepts_text_input` has
nobody to ask, so bindings always win, window.rs:4839–4854). So
"typing" must be detected on raw key events. What fires the pulse?

**Resolution — one rule, mock-parity, verified against dispatch:**
the ColdRead element's `on_key_down` (which, per the verified dispatch
order, sees ONLY keystrokes no binding consumed) pulses iff
`keystroke.key_char.is_some() && !modifiers.control &&
!modifiers.platform`. This is the mock's `e.key.length===1 && !ctrl
&& !meta` rule (ux-lab-2026-07.html:419) translated to gpui's honest
"would have typed" signal (keystroke.rs:26–32). Verified properties:

- **Modifier-only presses cannot fire**: lone modifiers arrive as
  `ModifiersChangedEvent`, never as KeyDown (window.rs:4718–4739) —
  the task's must-hold, guaranteed by the platform layer, but add the
  rig assertion anyway.
- **Space never double-fires**: it is BOUND (flip forward), and bound
  keys are consumed before listeners run — the mock's order-dependent
  `if` chain maps exactly.
- **Dead keys / compose**: the dead press itself carries no key_char
  (no pulse); the composed character arrives as one keystroke with
  key_char (one pulse). Alt is deliberately NOT excluded (option- and
  AltGr-typed characters are typing; the mock excludes neither).
- enter/backspace/delete are bound to the refusal in §0 (their
  key_char behavior is platform-wobbly; binding them is deterministic).
- The mock also pulses on Escape (ux-lab line 419) — a lab-ground
  artifact (the lab's Esc doesn't exit scenes). Product Esc exits.
  Named so Gate 2 doesn't "fix" it backwards.

Law: L18, the mock-fidelity gate with this one named divergence.

## 5 · The reaction input's own chords (DECIDE)

**Scenario.** The input is a `TextField` (NoteInput context: Enter →
FieldCommit, Esc → FieldCancel, editor.rs:564–573; field editing
bindings own ctrl-a/c/x/v inside it, text_field.rs:795+). Corners:

- **Enter on empty text**: mock files nothing and keeps the input
  (line 411 requires `value.trim()`). Adopt: no-op. Chips are the
  empty-input verbs.
- **Esc**: closes the input AND collapses the selection — the input
  and its selection are one raised object (spec §5.1 "collapses on
  Esc" + I3 two-level Esc). Second Esc exits the room.
- **Blur** (writer types three words, then clicks a flip zone):
  **file-if-nonempty, discard-if-empty.** The universal
  commit-on-blur law (the 2026-06-23 round: every field commits on
  blur — the rename field's exact pattern, editor.rs:4585–4601) beats
  "reactions file only deliberately": typed words are the writer's
  material and losing them to a stray click is the worse sin. The
  flip then proceeds — blur-commit first, click-action second, the
  standard order.
- **Guarded chords typed INTO the input** (ctrl-shift-p…): pulse, via
  case 1's architecture (the guard bindings sit between App and
  NoteInput on the dispatch path once the input mounts inside the
  takeover's subtree). The field's own deeper bindings (enter, esc,
  ctrl-a, ctrl-c) win over the guards — correct: inside the field
  they are field verbs.
- **Quit with unfiled text**: ctrl-q pierces (context None) and the
  quit path never blur-commits (main.rs:397–404 saves the document,
  not open fields). The typed reaction dies. Accept as NOTE-grade:
  same fate as any half-typed field at quit; not worth a quit hook.

Law: commit-on-blur (universal, shipped), P3, I3.

## 6 · F2 / doc-title rename inside the room (DECIDE)

**Scenario.** The titlebar remains (L16) with the doc-name chip; F2 is
App-scoped and the chip's click calls `rename_document` directly
(editor.rs:12096–12102) — a mouse path no key context guards.

**Resolution: allow, both entrances, both modes.** Renaming is the
writer's hands on her FILE, not on the text — no analog of the
manuscript is touched (`rename_file`, store path only,
editor.rs:4608–4628). The parked precedent leaves it live. Refusing a
title fix mid-read would be the tool wanting something (P2). The
rename field is a titlebar TextField with commit-on-blur — it coexists
with the takeover exactly as it coexists with the parked strip. Two
consequences to name: the running head reads the LIVE doc name, so it
updates mid-read after a rename — honest, the head is chrome
displaying a record (P1's "record/relocate as data"); and the rename
input's focus path is `root → titlebar field`, outside the ColdRead
subtree — one more reason case 1's handler-side guards exist (a
guarded chord pressed while renaming must still refuse via the App
handler's own guard, and DOES, because the handler guard doesn't care
where focus lives).

## 7 · The ⌕ / ≡ / clock titlebar controls: present-but-guarded is a lie (DECIDE)

**Scenario.** Spec §4.1 keeps ⌕ (omni pill), the history clock, and ≡
(hamburger) visible with guarded handlers. The pill renders as a live
IBeam field labeled "Search · ctrl-f" (editor.rs:12245–12283); the
clock and hamburger wear pointing-hand cursors and hover tints. A
control that looks operable but silently isn't violates P12 — the
control IS the indicator, so an inert control must indicate inertness.

**Resolution: dim all three** — muted ink at reduced opacity, no hover
tint, default cursor, tooltips retained (they name the chord that will
work again on Esc; that is teaching, not lying) — **and their clicks
pulse** (a blocked verb is a blocked verb regardless of input device;
the pulse is the one idiom). The pill's placeholder drops the "ctrl-f"
hint while dimmed (a dimmed control advertising its chord invites the
chord; the pulse would answer, but don't bait it). Window controls
(–/□/×) stay fully live — allow, like quit. The word-count pill and
editor button are hidden per spec §4.1, which is the stronger form of
the same honesty; hiding ⌕/≡/clock instead of dimming is the named
alternative if the dimmed row reads as clutter at Gate 2 — but
dimming preserves "the room still HAS these places, they wait"
(control-is-indicator, both directions).

## 8 · Guard-order law in the shared sinks (NOTE)

**Scenario.** A Past read entered from the parked strip leaves
`strip.is_parked()` true beneath the takeover. Every shared mutation
sink checks `is_parked` first and pulses the STRIP banner
(`pulse_strip`, editor.rs:2775–2800; sinks at 9356, 7786, 7886, 7910)
— which is hidden under the takeover. If any mutation path reaches a
sink (the rig's clipboard shim paste, editor.rs:7800–7808, does so
without a keystroke), the refusal pulses an invisible banner = a
silent swallow with extra steps.

**Resolution.** In every shared sink and in `escape_mode`, the
`cold_read` guard branch comes FIRST, before `is_parked` /
`history_view`; the ColdRead pulse targets the cold-read banner's bold
lead. In the PAST variant the banner has no "Reading" word (L15 is
name-led) — the pulse target is the bold checkpoint name, same idiom,
same wash (one pulse grammar, two banners). Rig: `coldread` dump
object gains a `pulse` bit (the strip's precedent, editor.rs:8627) so
refusal assertions are scriptable chord-by-chord.

## 9 · Quit and window close: verified unblocked (NOTE)

ctrl-q binds with context None (main.rs:134) — matches from any focus
including the takeover and the reaction input. The window ×
(off-macOS) calls `cx.quit()` directly (editor.rs:12531); the platform
close-request path also routes to `cx.quit()` (main.rs:363–384);
`on_app_quit` (main.rs:397–404) saves synchronously — filed reactions
are ordinary notes and persist; the single-instance socket releases
(main.rs:420). Nothing consults cold-read state anywhere on the path.
**Allow; nothing to build** except the case-5 note about unfiled
reaction text. The exit-state caret written at quit
(editor.rs:1794–1795) is the DESK's caret, untouched by the takeover —
correct by the "nothing moved" invariant.

## 10 · The root wheel and mouse handlers leak into the hidden desk (BLOCKER)

**Scenario.** The root `on_scroll_wheel` (editor.rs:16399 →
7935–7959) early-returns for palette/settings/shortcuts only. Inside
the takeover, a wheel twitch anywhere: (a) calls
`flush_deferred_pass` — landing a parked pass's cards mid-read, the
exact thing spec §4.4 promises cannot happen ("results park … reveal
on exit"); (b) scrolls the hidden desk's `scroll_top` — so Esc
returns the writer to a place she never left, breaking §4.6's "return
caret/scroll exactly as left". The same class: `on_middle_click`
pastes PRIMARY (editor.rs:8352–8356, mounted at 16516) if the editor
column is still mounted beneath and a middle click lands; and the
root `light_dismiss` mouse-down (16386–16391) is harmless (everything
it dismisses is already closed) but should be confirmed so.

**Resolution.** `on_scroll_wheel` gains `cold_read.is_some()` in its
early-return set (spec §4.3's "mouse wheel: nothing in v1" means
nothing ANYWHERE, including beneath); the takeover element occludes
mouse events so nothing reaches the suppressed column's handlers; the
in-read wheel does NOT count as "scroll" for the reveal clock (the
deferred pass stays parked — the reveal-clock lens owns the full
rule, this case owns the leak). Rig: wheel during `coldread:open`,
then Esc, assert `scroll_y` unchanged and no cards revealed.
Failure if unhandled: a silent position teleport + a mid-read card
landing — both prior-round blocker classes. Law: spec §4.3/§4.6,
attention-motion §2 (never mid-burst), P6.

## 11 · Past-mode selection and copy: the spec's "selection does nothing" over-shoots (DECIDE)

**Scenario.** Spec §4.7: reactions are disabled in Past mode,
"selection does nothing". But copy's whole precedent is lifting words
OUT of a past state (Bug B, editor.rs:7764: "the writer wants to lift
text out of a past revision"). If selection is dead in the Past book,
ctrl-c has no substrate — and the flat parked strip preview ALLOWS
select+copy of the very same past state: the same gesture on two past
surfaces, opposite behavior.

**Resolution.** Split selection from reaction: word-box selection and
case-2 copy work in BOTH modes; what Past mode disables is the
reaction INPUT (no input raises on mouseup; you annotate the present
only). This amends §4.7's wording — a deliberate divergence to name
at Gate 2. Copy slices the checkpoint state's own manuscript text
(its own boundary — L27). Law: Bug B precedent, L26/L27, P13.

## 12 · Pre-existing silent swallows adjacent to the new surface (NOTE — fix-in-passing candidates)

While building the guard set, two neighbors violate the one-pulse law
today and will read as cold-read bugs the moment testers compare:
`scraps_travel` while parked returns silently (editor.rs:6008–6010,
no pulse), and `set_aside` while parked relies on the deeper
`Document::set_aside` path rather than pulsing at the verb
(editor.rs:6061–6063 guards history_view only). Same class as the
footer-chips gap the spec already licenses fixing in passing
(§4.4). Cheap: route both through `pulse_strip` when parked. Not
cold-read scope; recorded so the mode-matrix audit doesn't blame the
new room for the old holes.

## 13 · Rig coverage for this lens (NOTE)

The smoke grammar already sends raw chords (`Keystroke::parse`,
smoke.rs:530). Add to rig-check: with `coldread:open` — (a)
ctrl-shift-p / ctrl-f / ctrl-alt-h / ctrl-shift-d each assert **no**
overlay bit in the dump AND `coldread.pulse` true; (b) a plain letter
key asserts pulse; (c) a lone shift press asserts NO pulse (case 4's
guarantee, exercised); (d) ctrl-c after a `coldread:select` hook
asserts clipboard equals the source slice (the case-2 golden); (e)
ctrl-q at any point quits cleanly (already implicit in every script's
teardown). The dump object: `coldread{open, pages, page, source,
pulse}`.

---

## Summary

The takeover's key context stops nothing by itself: all sixteen
App-context chords pierce it by construction, every Editor-context
chord dies into silence rather than refusal, and the app's own
convention of mounting field overlays on the root would carry even the
reaction input's focus path around any context-scoped guard — so the
guard must be architectural (registry-driven ColdRead bindings +
handler-side guards + in-subtree input mount), not a fence of
one-off checks. The three blockers are the pierce architecture itself,
copy (dead as spec'd, and clipboard-corrupting if implemented from
painted fragments — which contain no spaces at all; the law is: slice
the source snapshot by manuscript char ranges, both modes, PRIMARY
included), and the root wheel handler, which today would flush a
parked pass and scroll the hidden desk out from under the "nothing
moved" exit contract. The decides are mostly ratifications: the
pierce table (guard everything except rename, new/open, copy, quit,
and the verb's own toggle-exit), select-all refused in v1, Past-mode
selection kept alive for copy while only the reaction input dies,
blur-commit for typed reactions, and dimming the three titlebar
controls whose live faces would otherwise lie (P12). The typing pulse
rule is the mock's, made mechanical and verified against gpui's
dispatch: unbound keydown with a key_char and no ctrl/platform —
modifier-only presses provably cannot fire it, and space can't
double-fire because bindings consume before listeners run.
