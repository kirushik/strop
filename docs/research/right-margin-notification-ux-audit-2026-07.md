# Right-margin status and notification UX audit

Audit date: 2026-07-14. Code reviewed on `llm-pipeline-remediation` at
`c5912f5`. Status: **discussion document — no UI changes made**.

## Executive judgment

The complaint is correct. The right margin is a good home for the writer's
anchored notes and the editor's anchored queries. It is not a good generic
notification area merely because an LLM caused an event.

The current `AiStatus` surface combines four different jobs:

- an idle feature advertisement;
- a persistent prerequisite/recovery task;
- running-state and cancellation;
- transient success and informational receipts.

Those jobs have different loci and attention contracts. Giving them one
card-shaped renderer makes harmless facts look like editorial material, makes
unanchored system prose compete with anchored conversation, and creates real
layout collisions. Of the five current `AiStatus::Note` producers, none needs
to remain a margin card. The actionable setup and failure cases are defensible
as an interim exception, but their actions need to be specific to the failure.

The useful replacement rule is:

> Put content at its object, state on its control, validation in its form,
> receipts at their changed destination, and exceptional recovery at the
> smallest persistent surface that contains the next action.

This follows the constitution rather than inventing another notification
system:

- P1: the margin is prose-adjacent and must not become a generic overlay lane;
- P2: the tool does not initiate coaching or make maintenance into a task;
- P4: passive explanations are not interface content;
- P8: parallel meanings need parallel forms;
- P11: the editor control, not a competing status card, is the AI subsystem's
  anchor object;
- P12: the control that starts a read should carry its state;
- the attention brief: the writer moves information between center and
  periphery; the system does not push it inward.

## What the margin means

`docs/DESIGN.md` and `docs/margin-card-dynamics.md` give the lane a precise
ontology:

- writer notes are owned text about an anchored passage;
- editorial cards are machine-authored queries about an anchored passage;
- both preserve a spatial relationship between passage and card;
- the editor button is the AI subsystem's stateful control;
- the lane's motion and density budget exist to preserve that relationship.

An unanchored message such as “AI configured” or “Opened config.toml” has none
of those properties. Card styling falsely says “this concerns the nearby
passage,” and putting it before the real cards spends the lane's highest-
priority position on process chrome.

The nearby precedent is already good. AI settings test results are inline in
the AI settings panel. Save failure lives beside the document name. History
and cold-read state live in their mode banners. Scraps and Graveyard receipts
occur at the destination controls and passages. These surfaces show the right
general grammar.

## Complete `AiStatus` inventory

### One renderer, two geometries

`render_ai_status` in `crates/strop-app/src/editor.rs` renders every variant:

| Variant | Wide window | Narrow window | Lifetime |
| --- | --- | --- | --- |
| `None` | passive one-line margin hint when the lane is empty | nothing | persistent condition |
| `NeedsSetup` | top-of-margin action card | bottom action strip | persistent |
| `Running` | top-of-margin state card with Cancel | bottom state strip | request lifetime |
| `Note` | neutral top-of-margin information card | neutral bottom strip | six seconds |
| `Error` | error top-of-margin card with actions | error bottom strip | persistent |

The same state also changes `EditorFace`: setup, running, ready, error, and
open-query state are already reflected by the `Ask the editor` control. Notes
are the exception; they exist only as the transient card.

### Idle hint

Producer: the `None` branch inside `render_ai_status`.

Current copy: `Margin: ctrl-shift-d — {mode} read`.

It is not an event, but it occupies the same surface. It was added when the
core feature lacked a visible entry point. The always-visible `Ask the editor`
button and attached menu now provide that entry point and teach the relevant
actions. The hint is therefore duplicate capability advertising. It conflicts
with P2/P4 and should be removed.

### Setup prerequisite

Producers:

- an attempted read without a configured provider;
- `Test AI Connection` without a configured provider;
- the local Ollama probe updates the same card when it discovers a model.

Current card actions:

- run with the detected local model;
- open the AI settings panel;
- test the connection;
- dismiss.

This is not passive information: the writer explicitly requested a read and a
missing prerequisite blocked it. It is the strongest case for a temporary
action card. Still, the natural long-term locus is the AI settings panel: it
already owns provider choice, model discovery, inline validation, and the
pending-read continuation. Opening that panel immediately would remove a
choice surface before the real choice surface.

Conservative 0.2 judgment: keeping this actionable card is acceptable while
the direct-to-panel flow is designed and tested. Do not generalize from it to
success notifications.

### Running state

Producers:

- every editorial read;
- the palette-level `Test AI Connection` command.

Current card: `Running: {read/model}…` plus Cancel.

The editor button simultaneously wears a cooking dot, identifies the held read
in its attached menu, and makes all read rows inert. This is direct P12
duplication: state is displayed in the margin but controlled from the titlebar,
except Cancel, which lives only on the status card.

Preferred direction: the editor button remains the indicator; its attached
menu exposes the active read and a visible `Cancel this read` action. Connection
testing belongs in the settings panel and uses its existing inline Running/OK/
Failed state. Once Cancel has a lawful home, the running card can disappear.

Conservative 0.2 fallback: if the running card is retained, it needs measured
layout ownership and must not coexist at the same unreserved coordinate as
anchored cards.

### Read-completion note

Producer: every successful `integrate_pass`, including reads that landed real
cards.

Current titles:

- `Pass complete — the editor found nothing to flag`;
- `Pass complete — no quote matched the current text`;
- `{N} margin queries anchored`.

Optional details expose malformed and stale dropped-item counts.

This is the largest mismatch:

- with accepted cards, the cards and the editor control already are the
  outcome; the status card announces visible objects;
- malformed-sibling counts are backend maintenance, not a writer task;
- `Pass` and `anchored` are internal vocabulary leaking into chrome;
- the generic card arrives at the same moment as the anchored cards and can
  cover them;
- a six-second disappearance is a poor completion contract for the only case
  that truly needs an explicit outcome: a valid empty read.

Recommended outcomes:

| Result | Writer-facing outcome |
| --- | --- |
| one or more grounded cards | cards land; open count/control update; no status message |
| valid empty result | editor control carries a stable `nothing flagged` last-result state until the next read or deliberate acknowledgement |
| accepted cards plus rejected siblings | show accepted cards; record counts in diagnostics; do not assign repair work to the writer |
| all reply items invalid | actionable failed-read state with Retry and setup/details as applicable |
| quotes became unresolvable after in-flight edits | neutral blocked-result state such as `Draft changed before the read landed`, with `Read again`; do not call it provider failure |

Candidate copy needs a real UI pass. The table establishes semantics, not
final wording.

### Opened-config note

Producer: `Open AI Config` after launching the external editor.

Current copy explains which fields to fill and that Strop re-reads the file.

Opening the file is its own visible receipt; the template contains the
instructions. The card is both redundant and a six-second manual (P4). Remove
the success note. If launching fails, keep the user on an actionable error
surface. Longer term, the in-app settings panel is the normal path and the
config file is an advanced escape hatch.

### Connection-test success note

Producer: successful palette-level `Test AI Connection`.

Current copy: `OK — {model} via {host} · {latency}ms`.

The AI settings panel already implements the correct pattern: Running, OK with
latency, Failed, and model discovery all render inline. Route the palette
command to that panel/test path or retire the duplicate command. Do not emit a
margin card.

### Settings-saved note

Producer: successful save from the AI settings panel when no read is queued.

Current copies:

- `AI configured: {model} via {host}` plus `Run a pass with ctrl-shift-d.`;
- `AI settings saved (provider still incomplete)`.

Closing a successfully saved form and changing the editor button out of
`needs setup` is sufficient feedback. The invitation to run a pass violates
P2 and duplicates the visible editor control. An incomplete provider should
not close as a successful flow and then explain the incompleteness elsewhere;
keep the panel open with inline validation.

### Diagnosis-mode note

Producer: each palette command that changes the default diagnosis mode.

Current card names the mode and explains its purpose for six seconds.

The attached editor menu now lets the writer request developmental, line, or
copy reads directly, and visibly marks the default chord target. The older
mode commands are a parallel control grammar and the card is a passive lesson.
Either retire those commands, or let the menu's marked default row change as
the control's own feedback. No notification is needed.

### Error states

Producers:

- a selection outside the manuscript (for example, Scraps);
- a whole piece or selection over the source ceiling;
- provider authentication, rate-limit, provider, network, or response-shape
  failures;
- model refusal, output truncation, or unreadable diagnosis contract;
- failure to save the detected local provider;
- palette-level connection-test failure.

The persistence and recovery intent are right, and the user has explicitly
accepted a failure card with remediation actions as a possible margin use.
The current actions are not actually contextual: every error gets `Set up…`,
`Retry`, and `Dismiss`.

Examples of the resulting mismatch:

- `This is too long for one editor read` offers provider setup, which cannot
  shorten the selection, and Retry repeats the same failure;
- selecting Scraps offers provider setup instead of returning the selection to
  the manuscript;
- a local-config write failure's Retry runs the last editorial pass rather than
  retrying the write;
- rate limiting and authentication need different next actions.

Before treating the error card as an approved primitive, make recovery typed:

- authentication/setup: `Set up` and Retry after configuration;
- rate limit/network/5xx: Retry and Dismiss;
- over ceiling: `Use selection`/Dismiss, with Retry disabled until scope
  changes;
- non-manuscript selection: Dismiss and a manuscript-selection direction;
- truncation: Retry only if the request shape changes or output budget can
  change;
- invalid reply: Retry, plus diagnostics discoverable for support;
- local save failure: Retry save or return to settings.

Longer-term preference: error state lives on the editor control and expands in
its attached menu, because that is the subsystem's anchor object. For 0.2, an
actionable persistent margin/bottom card is a defensible conservative choice
if its buttons are truthful and its geometry is fixed.

## Layout and mechanics defects

The semantic mismatch already causes mechanical problems.

### Wide: status and cards share one coordinate

The status card is absolutely placed at `BAR_HEIGHT + 8`. The margin packer
also starts every real card at a floor of `BAR_HEIGHT + 8`; it does not receive
or measure status height. Therefore a completion/running/error card can overlap
the first writer note or diagnosis card.

The selection flank knows only that “top furniture” exists and adds a fixed
30-pixel allowance. Setup and error cards can be many lines tall, so the flank
can overlap them too. This exact hazard is already recorded in
`docs/impl/11-llm-repair-flow.md §4`.

### Medium: a status changes document geometry

`lane_has_content` treats any `ai_status` as lane content. At widths where the
lane barely fits, a transient six-second success note can shift the prose
column left, then let it shift back when the timer expires. A process receipt
therefore moves the writer's page even when no anchored card exists.

### Narrow: several bottom owners

The same status becomes `bottom: 0` at narrow widths. The note composer strip,
image-alt strip, history strip, footer chips, and other later-painted surfaces
also use the lower edge. Rendering order decides which one obscures another;
there is no shared measured stack or mutual-exclusion contract. Persistent
setup/error actions can become unreachable underneath a writer-owned field.

### Timers can spend a transient while hidden

History preview and cold read suppress the entire margin/status render block,
but the six-second Note timer continues. The cold-read design review extracted
the correct general law: never spend a one-shot transient while its surface is
suppressed. Removing transient Notes avoids most of this class instead of
adding more parking machinery.

## Adjacent informational-popup inventory

The margin card is not the only relevant transient. The shared bottom-right
`chord_whisper` surface has two producers:

1. after the first palette execution of a chorded command, it says
   `Chord: {keys} does this directly`;
2. after Replace All, it reports replaced and untouched counts.

It renders for six seconds in the bottom-right corner. Its own code comment
acknowledges that at narrow widths it can graze the last clipped prose line;
translucency does not satisfy the directive that status never covers prose.

The chord lesson directly conflicts with the newer constitution's P2 and P4:
it is a software-initiated teaching prompt. Remove it. Shortcuts already live
in the palette rows and tooltips, at the moment the writer asks for them.

The Replace All report is legitimate action-result data but has the wrong
locus. Keep the result in the omnibar/replace control that initiated the
operation, including the important `scraps untouched` count, until the next
query/action or Esc. That makes the control the indicator and avoids a global
popup.

## Nearby surfaces that should not be collapsed into “notifications”

These were inspected because they also appear and disappear, but their current
primitive is generally sound:

- AI settings test/model messages: inline validation in the owning form;
- save failure: persistent red state beside the document name while automatic
  retry continues;
- history, parked-history, and cold-read banners: visible mode state with
  relevant exit/restore actions;
- Scraps/Graveyard chip pulses and paragraph flashes: one-shot receipts at the
  changed destination;
- margin edge counts and narrow-note pill: persistent representations of real
  hidden anchored objects;
- note composer and alt-text strips: writer-owned fields, not status;
- diagnosis and writer cards: anchored content, the lane's actual purpose.

Their details can have independent bugs, but replacing them with a unified
toast/status system would make the grammar worse.

## Proposed surface matrix

| Meaning | Primary surface | Persistence | Why |
| --- | --- | --- | --- |
| anchored writer/editor content | margin card or narrow notes drawer | until writer resolves | spatial conversation |
| read is running | editor button + attached menu; Cancel in menu | while running | P12, one subsystem anchor |
| read produced cards | cards + open-count state | until resolved | outcome is already visible |
| valid empty read | last-result state on editor control/menu | until next read/acknowledgement | must not disappear before it is seen |
| partial malformed siblings | diagnostic log, normally no chrome | durable support evidence | AI maintenance is not writer work |
| blocked/failed read | typed actionable recovery surface | until action/dismiss | next action is required |
| provider setup/test | AI settings panel | while task is open | form validation at its locus |
| settings saved | changed control state; otherwise silent | immediate | action confirms itself |
| default read changed | marked row/control state | persistent | control is indicator |
| replace-all result | omnibar/replace row | until next operation/Esc | action result at initiating control |
| mode/takeover state | existing banner | for mode lifetime | no hidden modes |
| material moved/restored | destination chip/paragraph receipt | one quiet event | object constancy and trust |

## Recommended remediation order

### Step 1 — remove plainly wrong informational cards

- Stop emitting a Note for reads that landed one or more cards.
- Remove the opened-config, settings-saved, and diagnosis-mode Notes.
- Keep connection-test state inline in the settings panel.
- Remove the now-obsolete idle margin hint.
- Remove the shortcut-coaching whisper; move Replace All outcome into the
  omnibar.

This is mostly deletion and routing to already-existing states.

### Step 2 — make the editor control a complete run surface

- Add visible active-read detail and Cancel to the attached editor menu.
- Design a persistent valid-empty outcome there.
- Decide the neutral in-flight-edit/no-match outcome and its `Read again`
  action.
- Do not rely on hover for failure meaning (P9).

Only after this step should the Running margin card be removed.

### Step 3 — type recovery actions

Replace the universal `Set up / Retry / Dismiss` row with a failure-specific
action model. Verify that every button changes the condition named by the
message. Retain the margin/bottom recovery card for 0.2 if moving failure
details into the editor menu needs a deeper design round.

### Step 4 — fix geometry for every retained status surface

If any top-of-lane card remains, measure it and give the packer and selection
flank one shared top-furniture height. At narrow widths, define one bottom-
surface owner or a measured stack; never rely on paint order. Add overlap and
action-reachability rig assertions.

### Step 5 — revisit repair UX only after this grammar lands

`docs/impl/11-llm-repair-flow.md` currently assumes the running status card is
the detailed state and Cancel surface. This audit rejects that premise as the
default. Any later repair flow should use the settled editor-control grammar,
deliver valid cards through the existing reveal clock, and keep serialization
maintenance out of writer-facing chrome unless no usable result survives.

## Validation plan

### Required stable frames

Capture wide, medium, and narrow frames for:

1. idle editor with an empty margin;
2. read running, menu closed and open;
3. cancellation available without a status card;
4. read landing several cards;
5. valid empty read;
6. partial accepted/rejected reply;
7. authentication failure;
8. rate limit/network failure;
9. oversized whole-piece read;
10. selection in Scraps;
11. unconfigured provider with and without local Ollama;
12. note composer or alt field open while an AI failure persists.

For every frame assert: no prose overlap, no card overlap, one anchor object,
all stated actions reachable, and no state whose meaning exists only on hover,
color, or motion.

### Corridor tasks

Without explaining the state machine, ask testers to:

- start and cancel a read;
- tell when it completed with cards and with no cards;
- recover from a bad API key, a temporary network failure, and an oversized
  manuscript;
- change which kind of read the shortcut invokes;
- test and save a provider;
- run Replace All where Scraps also contains matches.

Ask what changed, where they looked first, and what each offered action would
do before they click it. The key release risks are not preference ratings but
false beliefs: “the status card comments on this passage,” “Retry will fix the
length,” “my read is still running,” or “the setup was complete when it was
not.”

### Instrumentable gates

- Zero informational `AiStatus::Note` producers remain.
- A successful read with cards creates no unanchored status surface.
- A valid empty read stays discoverable until superseded or acknowledged.
- Running always has a visible Cancel path at wide and narrow widths.
- Every failure class maps to an explicit allowed action set.
- No bottom-right whisper paints over prose.
- Retained status and anchored cards never share unreserved geometry.
- Cold read/history never consume a hidden transient's timer.

## Decisions requested before implementation

Decision record (2026-07-14):

- For 0.2, retain the actionable setup/failure card as an explicitly temporary
  exception, type its actions, and fix its geometry. Revisit the surface before
  0.3.
- For 0.2, a valid empty read shows `0 new` on the editor button without a
  timeout. Closing its attached menu acknowledges the marker; the menu keeps a
  quiet last-result record until another read supersedes it. This records
  interaction exposure, not inferred gaze. Revisit and user-test before 0.3.
- Retire the three palette diagnosis-mode commands now.

The resulting 0.2 implementation removes all `AiStatus::Note` producers and
the idle/running margin surfaces; adds active-read/Cancel and last-empty data
to the editor menu; routes connection testing into the settings panel; types
every retained recovery action; gives recovery, rail, cards, and the selection
menu one shared wide-lane floor; parks narrow recovery under writer-owned
bottom surfaces; retires the diagnosis-mode commands and chord whisper; and
keeps Replace All counts inline in the omnibar. Rig states cover running,
empty acknowledgement, and recovery/card geometry. The two temporary product
decisions are also recorded as a pre-0.3 gate in `docs/ROADMAP.md`.

1. For 0.2, should actionable setup/error details remain as the conservative
   margin/bottom exception, or should this pass move them into the attached
   editor menu too? My recommendation is **retain them for 0.2, but type their
   actions and fix their geometry**; test the menu-only recovery design later.
2. For a valid empty read, should the editor button carry a persistent short
   result until the next read, or should the attached menu alone record the
   last result? My recommendation is **a restrained persistent button state**:
   zero cards otherwise looks indistinguishable from a request that vanished.
3. Are the three palette diagnosis-mode commands still worth keeping now that
   the editor menu requests each read directly? My recommendation is **retire
   them** rather than support a second sticky-default grammar.

Everything else in Steps 1–4 follows from existing principles and observed
mechanics rather than requiring a new product decision.
