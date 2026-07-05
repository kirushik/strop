# Impl spec 04 — the editor button

*(Design docs: golden-path §9.2–9.3, ux-glossary (presence pair), lab
v4 scene 3. Status: SPEC — pre-review draft.)*

## 0. The consolidation

One titlebar control replaces the bare diagnose-toggle as the
subsystem's single home (`window_button` idiom for the control,
`render_narrow_notes_panel` idiom for the attached menu):

**Button faces** (state lives on the control, P12):
- idle/drafting: `Ask the editor ▾`
- cooking: pulse dot + `Ask the editor ▾` (dot = the existing
  `AiStatus::Running`; hover names the read)
- parked results: `Ask the editor · a read is ready ▾` (sentence form,
  never "1 read waiting")
- reviewing: `Reviewing · {n} open · Ask the editor ▾`

**The menu**, glued flush under the button (right edges aligned, the
lab's fix for the detached-dropdown sin):
- carrier line `Ask the editor for…`
- `A believing read — what's alive here, what it's secretly about`
- `A developmental read — the structure: stakes, turns, the ending`
- `A line read — rhythm, imagery, dialogue`
- `A copy read — slips, typos, repetitions` — GATED while a
  developmental query is open; gate reason as its `when` line
  (`after the structural queries settle`) — the only row that
  explains, because the gate is data. No "usually after…" advice
  anywhere (P2).
- `A doubting read — the strongest case against the argument` (new
  prompt in diagnose.rs: the believing read's mirror; same parser)
- footer: `{open} queries open · {resolved} resolved` + the presence
  verb `Reading ⇄ Away` (= the door: Away ⇔ `drafting`; toggling
  routes through the existing `toggle_review` flush semantics).

Menu rows dispatch through the existing `run_pass` machinery with the
mode pinned per row (believing / mode strings). Selecting a row closes
the menu. Light-dismiss + Esc per the panel idiom.

## 1. What this retires

- The old diagnose-toggle mini-card icon (the button replaces it).
- **The intent banner and `next_intent`** — re-entry is SHELVED
  (golden-path §9.3); the banner render, the field, and the
  End-Session intent question are removed. End Session keeps its
  sealing role only. (`end_session_input` survives where it names the
  session checkpoint.)

## 2. Corner cases

- Pass already cooking + row clicked → the existing single-flight
  guard (`ai_generation`) applies; the row is inert while cooking
  (menu shows the pulse in the button; no queueing in v1).
- `ai.configured()` false → menu rows disabled with the existing
  NeedsSetup affordance routed from `render_ai_status`.
- Door toggling with parked results → existing flush-deferred
  semantics (toggle_review already flushes).
- Copy-gate release: developmental queries all closed → gated row
  livens without a pass re-run.
- Narrow window: the button truncates to `Editor ▾` before the
  titlebar collapses (word-count yields first, matching existing
  titlebar priority).

## 3. Tests & rig

- Dump gains `editor_btn: {face, open, cooking, ready, open_count}`.
- Smoke: `ebtn:open`, `ebtn:door`. Rig asserts the door law (cards
  rest while drafting even with the menu open) and face transitions
  through seed:deliver.
