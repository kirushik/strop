# Acceptance gates — what "done" must survive

Born 2026-07-05, from the litmus-file round: a wave that was green on
every gate we had (204 tests, clippy, 48 rig assertions, wshots) shipped
a history strip that rendered a real document's June as one thin line,
an invisible read-only mode, a graveyard that truncated a buried section
to three lines, and a compost with no visible presentation at all. Every
one of those was invisible to the gates because the gates only ever saw
*fresh rig documents* and never compared a surface against its approved
mockup. These three gates close that class. They are additive: the old
gates (cargo test, clippy -D warnings, scripts/rig-check.sh) still run.

## 1 · The legacy-document gate

Every feature that reads persisted state must be exercised against a
document with a *past* — not only the fresh `mktemp` docs the rig seeds.
The reference shape is the litmus file ("Welcome to Strop 4"): weeks of
checkpoints that predate the journal, margin notes, thousands of words.

- The rig carries `seed:legacy` (checkpoints at past timestamps,
  materialized states, an empty journal). New surfaces that touch
  history, checkpoints, annotations, or persistence MUST add an
  assertion block against it.
- Before declaring a wave done: copy the litmus file (never open the
  original — opening mutates the store; never commit any `.strop`),
  wshot the new surfaces on the copy, and READ the shots. A fresh copy
  per run — smoke tokens type into the document when they miss.

## 2 · The mockup-fidelity gate

The lab (docs/mockups/ux-lab-2026-07.html) is the approved design; specs
are its lossy projection. Before a surface is done, put its wshot beside
the lab scene and check every element the scene draws:

| Surface | Lab scene | Look for |
|---|---|---|
| Cold read | 1 | page, banner strings, reaction input, Esc returns |
| Compost | 2 | rail typography, hairlines, anchor-quote items, tail mark |
| Graveyard | 2 | footer bar strings, FULL-text entries, receded one-liners, whisper verbs |
| Flanks | 2 | 2×4 grid + seam left; verb rows + chord chips right; opacity |
| Editor button | 3 | border, shape, arrow, state labels, glued menu |
| History strip | 4 | rail/fabric/envelope, stations, readout, Now, Restore |

A deliberate divergence is allowed; a *silent* one is the failure mode.
Divergences are named in the wave report, with the reason.

## 3 · The mode-matrix gate

Modal states must be visible and uniform. For any state that gates
input or hides UI (parked history, drafting door, cold read):

- There is an always-visible indicator *in the writer's field of view*
  (not only in the panel that created the state).
- Every blocked verb reacts the same way (one pulse idiom — never a
  silent swallow), and the exits are named on screen.
- The rig asserts: enter state → dump bit set; leave state → the
  hidden surfaces come back (e.g. `margin_hidden` false again).
