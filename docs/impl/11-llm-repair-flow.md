# 11 — LLM partial-result and repair flow

UX/architecture proposal, 2026-07-13. Status: **PROPOSAL — design review
required before implementation**. This applies the constitution in
`design-principles.md`, the shipped attention decisions in
`attention-motion.md`, the editor-button grammar in `impl/04-editor-button.md`,
and the margin mechanics in `margin-card-dynamics.md` to one question: what
should happen when an LLM reply contains some valid cards and some material
that needs the single bounded repair round?

## 0 · Design judgment

The repair is not a second editor pass and must not look like one. It is the
unfinished tail of the writer's original request. Therefore:

- the editor button remains the subsystem's one anchor object (P11);
- its existing cooking pulse remains the running indicator (P12);
- the attached editor menu is the detailed state and Cancel surface; no
  running card enters the prose-adjacent margin;
- valid first-round cards enter the existing reveal clock immediately;
- no toast, modal, completion pip, badge, or new notification channel appears;
- repaired cards join the same logical pass and use the same card grammar.

This is the smallest flow that tells the truth without placing process chrome
over the prose (P1), teaching the writer about JSON (P4), or making AI
maintenance into a task the writer must manage (P2).

## 1 · State and event contract

One run reserves one pass ID before its first provider request. Initial and
repaired cards, diagnostics, cancellation, and the terminal journal event all
share it.

| State/event | Cards | Editor control | Status surface |
| --- | --- | --- | --- |
| Requested | none yet | cooking pulse | running label + Cancel |
| First reply, all valid | normal reveal-clock delivery | returns idle when delivery is recorded | clears |
| First reply, partly valid | valid cards enter reveal clock | keeps cooking | read kind + ready/rejected counts + Cancel |
| First reply, zero valid | none | keeps cooking | restrained checking label + Cancel |
| Repair returns valid items | additions join the same pass and reveal clock | returns idle at terminal | clears after terminal delivery |
| Repair fails after partial success | landed/pending valid cards remain | normal ready/Reading face, never Error | existing neutral `Note`, then its normal fade |
| Repair fails with zero valid | none | error face | existing error card |
| Cancel during repair | already landed cards remain; pending repaired work stops | returns idle | clears without rollback |

Candidate copy should be reviewed in the real surface at narrow and wide
widths. It must describe what is still true without anthropomorphising a panel
of editors or suggesting that a second substantive opinion is being
commissioned. A counted direction such as `Line read · 3 ready · checking 2`
is data and explains the continuing pulse. If the reply cannot be counted,
`Line read · checking the reply` is enough. “Another consultation” makes a
serialization recovery sound editorially meaningful and should not ship.

## 2 · Reveal and attention rules

First-round validity does not override the shipped attention contract.

- If the writer is Reading or the normal lull condition has fired, valid cards
  may land immediately while the pulse continues.
- If the writer is in an active typing burst or cold read, valid cards remain
  in the existing deferred batch. Repair additions merge into that batch; one
  later reveal presents the pass coherently.
- If the first batch has already landed, repaired additions are genuinely new
  cards and use the existing 250 ms appearance treatment. Existing cards do
  not reanimate or repack merely to advertise backend work.
- Completion uses the title-bar/status state already shipped. There is no new
  pip: `attention-motion.md` explicitly cut that channel pending evidence.

This preserves the writer-controlled center/periphery movement and prevents a
malformed sibling from delaying useful feedback without converting partial
delivery into an interruption.

## 3 · Identity, edits, and journal semantics

Every landing phase re-resolves the original target snapshot against the
current manuscript. A selected target that changed or became ambiguous fails
closed. A whole-piece result follows the current whole manuscript only under
the existing scope rule. Repair can never anchor outside the original target.

Between phases the writer may edit, resolve, or dismiss an initial card. The
repair phase must re-run duplicate and suppression checks against current
annotations. It cannot resurrect a dismissal, stack an equivalent query, or
attach to a quote that only exists in context.

Record one terminal pass event after the repair succeeds, fails, or is
cancelled. It carries initial-valid, repair-valid, rejected, and dropped
counts; it must not make two reads appear in history. Diagnostics may record
phase timings and provider request IDs, but never source or raw completions.

## 4 · Layout hazard

Partial cards and an active repair may coexist. The 0.2 AI UX pass established
the ownership contract before repair implementation: the editor button/menu
owns running state and Cancel; only persistent setup/failure recovery owns the
shared wide-lane floor, and narrow recovery yields to writer-owned bottom
surfaces. A later repair must reuse that contract. It may not restore a running
status card, move Cancel into hover, or place a banner across the manuscript
(P1, P6, P9, P12).

## 5 · Failure severity

Partial success is not a failed reading. If at least one grounded card survives
and the repair tail fails, keep those cards and record the tail failure only in
the redacted diagnostic log. The cards are already the visible result; a second
status surface would turn backend maintenance into writer work. The normal
editor button face is determined by the landed/deferred cards. Error styling
and Retry are reserved for a run that yields no usable result or a provider/
authentication failure requiring action. Color is never the only distinction
(P10).

The writer should never need to retry malformed JSON manually. A retry action
is appropriate only after the bounded recovery is exhausted and there were no
usable cards, or after an actionable provider failure.

## 6 · Required frames and adversarial review

The implementation is not reviewable from a happy-path video. Capture and
inspect these stable frames:

1. requested, no cards yet;
2. partial cards visible while repair runs, wide margin;
3. partial cards deferred during active typing;
4. repair additions merged into a deferred reveal;
5. repair additions landing while already Reading;
6. partial success after repair failure;
7. cancellation after initial cards landed;
8. narrow bottom strip with partial cards, status, and Cancel;
9. reduced-motion behavior for every changed layout.

Adversarial questions:

- Does the continuing pulse read as “still checking” or as a stuck duplicate
  pass?
- Do early cards pull the writer into evaluation before their chosen
  breakpoint?
- Can a later card appear without an understandable relationship to the
  original request?
- Does cancellation falsely imply that already delivered cards were revoked?
- Can status furniture obscure the very passage or card it describes?
- Does any state depend on red, animation, or hover for meaning?

Only after this review should neutral user tests compare candidate status copy.
Ask what participants believed was happening and what they expected Cancel to
do; do not explain “repair” first or ask whether the design feels reassuring.
