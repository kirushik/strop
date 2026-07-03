# The writing–editing–checkpointing flow: an assembly brief

*(2026-07-03, draft for Kirill's reaction — nothing here is built. Companion
to DESIGN.md §6 Round 2 and the versioning dossier in
`docs/research/writer-core-loop-2026-06-14.md` §4a; citations live there.)*

## 0. The defect, stated against ourselves

Strop has every building block of a version story and no version story. An
inventory of what ships today, with the seams named:

| Block | Shipped as | What it knows |
|---|---|---|
| The door | draft/review gate, altitude order, rail | which mode — held in memory, recorded nowhere |
| Sessions | open-seal ("Session start"), 15-min idle-gap seal, end-session ritual | wall-clock boundaries |
| Intents | "Next session I will ___" → banner at next open | the writer's own words — consumed by the banner, then GONE |
| Goals | "+50 words" live progress | reached or not — never recorded |
| Checkpoints | named (ctrl-alt-s), autos, materialized states, restore-as-forward + Before-restoring safety | name, time, and (since 025a1e0) the full document state |
| History panel | push panel, canvas preview, vs-prev/vs-draft, word delta, voice-drift glyph | text deltas and one stylometric scalar |
| Passes | pass_id + staleness + dismiss-suppression | which cards a pass raised — never linked to any version |

Four independent clocks tick over this pile: persistence (idle-save 1s),
session (open/idle-gap/end), attention (typing lull, the door), and meaning
(passes, manual checkpoints, intents). None reads another. The concrete
absurdity: the single most meaningful moment in the loop — the writer
explicitly closing a session and *typing what matters next in their own
words* — leaves no mark in history at all. The intent string feeds a banner
and evaporates. Meanwhile a 15-minute coffee break gets a permanent
checkpoint.

That is the disassembly. The dossier says what it costs: writers backtrack
asking "where was this still good and why did I change it," and every tool —
now including ours — answers with a wall of timestamps (§4a; MTSU). Our rows
are better-dressed timestamps (delta + drift glyph), but the *why* layer is
absent, and the why layer is the entire gap the research says nobody serves.

## 1. The organizing claim

**A checkpoint should be the written record of a core-loop transition, not a
timer artifact.** The consolidated loop model (DESIGN §6) already names the
transitions: IGNITION → DRAFTING → HANDOFF → RE-ENTRY → ALTITUDE DESCENT.
Every one of them already has a shipped mechanical trace — open-seal,
idle-gap, end-session, run-pass, restore. The assembly work is not new
machinery; it is making the existing trace *land in the same record*, so that
history reads as the story of the work instead of a clock.

What a checkpoint record becomes (all derived, zero ceremony — the dossier's
hardest finding is that naming-discipline does not exist; Darwin's "eight
files all called Final"):

```
CheckpointMeta {
  kind:      SessionStart | SessionEnd | IdleGap | PrePass | BeforeRestore | Named,
  intent:    Option<String>,       // the writer's own sentence, at SessionEnd
  goal:      Option<(target, hit)> // "+50 words" and whether it landed
  cards:     Vec<CardStub>,        // open diagnoses AT THIS MOMENT (title, level, pass)
  resolved:  usize,                // cards resolved since the previous record
  passes:    Vec<u64>,             // pass_ids run since the previous record
}
```

The enabler is already on this branch: materialized `CheckpointState` made
checkpoints self-contained documents; metadata is one more field in the same
list item, with serde defaults for legacy files. This was infeasible in June
(anything per-checkpoint cost a 5–7s time-travel) and is ~free now.

## 2. Counterarguments first

**"The session is the writer's clock, not the story's."** True and decisive
against session-as-THE-unit: a chapter spans nine sessions, a session may
touch three chapters. So sessions do not replace named checkpoints — they
organize the *automatic* record between them. Named checkpoints stay the
first-class annotations they already are (DESIGN §5 tension 7); the session
records are the two-tier collapsed layer underneath (the Figma answer,
already spec'd, never built). The intent string is the exception that makes
session records worth reading: it is the one semantic label with zero
discipline cost, because the ritual already captures it.

**"Cards on checkpoints turn history into a guilt ledger."** The dossier's
own warning: some backtracking is discouragement, not a text request (Hardy),
and a row reading "11 problems open" feeds the spiral. So the row leads with
motion, not debt — "+412 words · resolved 3 · 'tighten the ferry scene'" —
and the open-card stubs surface only in the preview detail, as *orientation*
("this is the version before I over-controlled the dialogue"), never as a
count in the list. The parade-of-small-rewards framing is load-bearing, not
decoration.

**"Pre-pass checkpoints are spam."** Running three passes in a review hour
must not mint three identical versions. `add_checkpoint_if_changed` already
exists; a PrePass record seals only when edits happened since the last
record. The payoff is the dossier's sharpest proposal made real by
derivation: a version labeled by the problems the editor found in it —
"the version pass 4 read."

**"More history UI for a surface writers rarely open."** The rarity is the
symptom, not the baseline: history is opened rarely because it answers
questions nobody asks (what changed at 14:32?). The folk final.FINAL.v2
system is the demand signal — writers *want* semantic labels and pay
filename-chaos prices for them (§4a, xkcd 1459). We hold the unique raw
material (intents, cards, passes) that no competitor has; the assembly is
cheap; the bet is bounded.

**"Restore already works; leave it."** Restore-as-forward with the
Before-restoring safety checkpoint is correct and shipped — and silent. The
dossier is explicit that the *fear* is the failure mode (defensive duplicate
files; Scrivener's foresight paradox), and fear is killed by saying the
safety out loud, not by having it. One sentence in the restore affordance
("a view, not a destruction" — the explorability layer's own language) is
the whole build.

## 3. The assembly, phased for rounds

**Round 2a — the record (model only).** `CheckpointMeta` on `Checkpoint`,
populated at the five existing call sites: open-seal, idle-gap, end-session
(which starts sealing a checkpoint — the hole closed), pre-pass, restore.
End-session's intent and goal-outcome land in the record *and* keep feeding
the banner. Legacy checkpoints: meta absent, rows render as today. Tests:
meta round-trips, if-changed gating, intent lands.

**Round 2b — history reads as narrative.** The two-tier list from DESIGN §2:
named checkpoints and session-end records first-class; idle-gap/pre-pass
autos collapsed between them behind a count ("· 4 more moments"). Row =
kind-glyph · name-or-intent · word delta · voice glyph. Preview detail gains
the card stubs of that moment. No timestamps as the leading text — time is
the sort order, not the label (the anti-wall).

**Round 2c — revert psychology, two sentences of build.** (1) The restore
button says what it does: "restores as a new version — everything since
stays in history." (2) The comparison frame stops presenting current-as-
winner: vs-draft mode's header names both sides symmetrically ("then / now"),
because existing text carries unearned authority (Darwin) and the UI must
not add to it. Optionally later: a diagnosis-styled observation on the pair
— framed as a question, never a verdict (red line: police problems, not
style).

**Round 3 dependency banked, not built — the Cuttings drawer.** Cut text
(especially cut in response to a card) exiles to a searchable drawer
anchored to its removal site; permission-to-cut is the function, resurrection
is the writer's alone. Red line inherited whole: the machine NEVER
re-suggests an exiled darling — preservation only. This brief only reserves
the model seam (a cut record referencing the checkpoint it happened after),
so Round 3 doesn't need a migration.

**Deferred, gated on the baseline actually existing:** the voice-trajectory
sparkline as history's primary navigation (§4a proposal 1). MATTR-100 on
sub-session deltas is noise (the Eder caveat already in ROADMAP); it earns
its place only with a real corpus baseline and per-checkpoint drift that
survives its own error bars. The drift glyph we already show is the honest
v1 of this.

## 4. What this refuses

No branch DAG, no undo-tree, no merge — restore linearizes (dossier §4a.5,
"disorienting even to programmers"). No auto-naming by an LLM (derivation
beats generation: derived labels are facts, generated ones are opinions the
writer must audit). No read/unread, no "new since last session" (DESIGN §7 —
no async source). No checkpoint ceremony added anywhere: every record in
§3 exists because a shipped mechanism already fires at that moment.
