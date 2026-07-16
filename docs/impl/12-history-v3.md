# Impl spec 12 — history strip v3

*(Design doc: `docs/history-strip.md` v3 — LAW; when this brief and
the spec disagree, the spec wins and the divergence is reported, not
coded around. Supersedes spec 01's layout facts where they conflict.
Status: build in progress on branch `history-v3`.)*

## Build order (waves; each lands green before the next starts)

- **A — the record** (strop-core + recording seams): §6 events,
  `del_words`, the shared range-transform, the store API for naming
  an arbitrary moment. No UI.
- **B — sheet, desk, sittings** (strip.rs + painter): §1a geometry,
  §1b session-mark removal, clickable dates. No threads, no margin.
- **C — true threads + the past margin** (needs A): §1e polylines
  (v3 forward path + legacy reverse-walk), CardHistoryIndex, §3b
  projection and margin rendering while parked.
- **D — Name this version** (needs A's store API): §2d center group,
  §3c composer, palette rewire.
- **E — Compare** (needs C, D): §3d chips, reading room, Changes,
  per-side margins.
- **F — rig + aesthetics** (coordinator): new rig scenarios, wshot
  passes at 1× and 2×, review round.

## Laws that bind every wave

- The scrub **stability law** (§3a): the bake is immutable while the
  strip is open; the `bakes` counter is the assertion. The
  CardHistoryIndex is the bake's immutable sibling.
- **Red line 3 extended**: no geometry may assert a coordinate the
  record doesn't prove. Uncertainty is drawn as absence plus the
  diamond boundary, never as softened marks.
- **Restore + CardsRebased are one durable generation.** No persisted
  state may contain the one without the other.
- Estimates stay distinguishable from exact counts, forever
  (`del_words`: legacy runs estimate; the estimate never gains
  expressiveness).
- Vocabulary: no user-visible "checkpoint", "session", "seal";
  writer-facing noun is "version" (`ux-glossary.md`).
- Wire compat: every v0.2 file opens; every new field/event has a
  serde default story; fixture tests prove both directions.

## Acceptance floor (per wave)

1. New targeted tests for the wave's laws.
2. `cargo test --workspace` (pinned toolchain — AGENTS.md).
3. `cargo build -p strop-app && bash scripts/rig-check.sh`.
4. Coordinator wshot review before merge.
