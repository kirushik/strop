# Implementation plan — golden-path build wave 1 (2026-07-05)

*(From the round-4 designs to running code. Specs: `00-journal.md`
… `05-cold-read.md` in this directory. Branch: `golden-path-impl`.
Method per Kirill's brief: specs → adversarial review → architecture
→ fan-out → assemble → review → validate; edge-case polish on the
real document happens with him, after.)*

## 0. Scope call

**Wave 1 builds:** P0 journal · P1 history strip · P2 asides ·
P3 selection flanks · P4 editor button (incl. retiring the shelved
re-entry surfaces). **Deferred:** P5 cold read (the one package
needing new layout machinery — spec'd, reviewed, not rushed).
Quality over coverage: any package that can't reach green
tests + rig by assembly time ships as a reviewed branch commit,
not as a broken feature.

## 1. Architecture facts the packages hang on (from recon)

- The journal records at the `take_ops`→`Store::apply` drain; persists
  as a dedicated **Loro list container** (append-only pushes; blob
  re-insertion would resurrect the 4.8MB bloat class; lists survive
  shallow compaction like checkpoints do).
- Scrub reconstruction = nearest materialized checkpoint state +
  forward replay through a scratch `Document` (`edit_bytes`), so
  spans/blocks/notes ride the same invariant machinery as live
  editing. `state_at`/Loro checkout is never touched (5–7s hangs).
- The strip paints as a custom `Element` from **instanced quads**
  (Scene batches same-type primitives; thousands of flecks = one
  batch); scrub = the selection-drag listener pattern +
  `request_animation_frame` while active.
- Asides: compost = rope region behind a `BlockKind::AsideBoundary`
  sentinel (no new BlockMap invariant); graveyard = a side record like
  `Annotations`. Counts/export/passes scope to the manuscript slice.
- All app-level integration follows the precedent map: bottom-strip
  idiom (strip), outline-panel idiom (rail), selection-popover +
  format_tools (flanks), window_button + narrow-notes-panel anchoring
  (editor button).
- Gates: clippy `-D warnings`; tests (164 green at baseline); the
  headless rig is THE integration harness; hand-formatted tree —
  **never cargo fmt**; commits `--no-gpg-sign`.

## 2. Work packages

| # | Package | Layer | Owner | Depends on |
|---|---|---|---|---|
| P0 | journal.rs + record hook + persistence + pass-id collision fix | core | main loop (me) | — |
| P1 | StripView bake + StripElement + scrub/preview/restore + rig | app | agent, worktree | P0 |
| P2 | aside boundary + region scoping + graveyard + rail UI + verbs | core+app | agent, worktree | — |
| P3 | flank grid + right selection menu + link button | app | agent, worktree | P2 (verbs) |
| P4 | editor button + menu + presence + doubting read + retire intent banner | app | agent, worktree | — |

Parallelism: P0 first (everything reads its API), then P1 ∥ P2 ∥ P4;
P3 follows P2. Integration is centralized: worktree diffs land on
`golden-path-impl` one at a time (P2 → P4 → P1 → P3 by expected
readiness), conflicts in editor.rs resolved by hand at merge — every
package's editor.rs surface is additive (new fields, new render fns,
wiring lines), so conflicts are textual, not semantic.

Each implementation agent must: build clippy-clean, run
`cargo test` green, extend `smoke.rs`/`debug_ui_dump` per its spec,
match the hand-formatting idioms (style notes in
`scratchpad/recon/app-ui.md`), and NEVER run cargo fmt.

## 3. Validation plan

1. Per-package: unit tests (pure logic) + dump-level rig assertions.
2. Assembly: full `cargo test` + extended `scripts/rig-check.sh` run.
3. Multi-lens review workflow over the assembled diff (correctness /
   spec-fidelity vs the design docs / perf / invariant traps from
   recon), findings adversarially verified, then fixed.
4. Visual: wshot screenshots of seeded fixtures (strip open, parked,
   post-restore; rail + flanks; button states) — eyeballed against
   the lab's intent.
5. Kirill's real-document polish pass happens live, after handoff.

## 4. Open decisions parked for Kirill

- Strip vs right history panel: v1 keeps both (ctrl-alt-h → strip;
  panel via palette). The demolition question stands.
- Restore-anxiety microcopy vs hover-preview (round-4 open item) —
  strip ships with neither; the appended-station visual is the
  argument.
- The cut-to-graveyard threshold (80 chars) — tuned on his doc.
