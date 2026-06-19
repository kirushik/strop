# The gpui fork (where our gpui-tree patches live)

Strop's `gpui` / `gpui_platform` are pinned in the root `Cargo.toml` to a
**fork** of zed-industries/zed — **<https://github.com/kirushik/zed>**, branch
`strop-patches` — based at upstream rev `992f395c` (tag v0.233.10) plus the two
commits below.

**The fork is the single source of truth** for everything we change inside the
gpui tree: the pinned rev is exactly what compiles, and these commits *are* the
diffs. There is no vendored dependency source in this repo, and no second copy
of the patches to drift out of sync — to read a change, open its commit.

> **2026-06-19 dependency review:** the patches were rebased onto upstream `main`
> (origin/main @ `69b602c7`) on branch **`strop-patches-on-main`** (tip
> `96bebcc2db`), and a third commit narrows `image` to the formats gpui decodes.
> The rebase alone drops the `async-std`/`async-tar` subtree (clears
> RUSTSEC-2025-0052). The pin swap to the new rev is pending a push of that
> branch — see [`dependency-review-2026-06.md`](dependency-review-2026-06.md) for
> the metrics and the finalize checklist.

## Patches (commits on `strop-patches`)

- **gpui_windows: layout-independent letter keys** —
  [`fc1a0cc814ee1f810ecf62943ab4e2cc7eb7976d`](https://github.com/kirushik/zed/commit/fc1a0cc814ee1f810ecf62943ab4e2cc7eb7976d).
  `get_key_from_vkey` mapped VK codes through the *active* keyboard layout, so
  letter chords (`Ctrl+Shift+P`, …) stopped matching under non-Latin layouts
  (the physical `P` key yields `з` under Cyrillic). Returns the US-layout letter
  for `VK_A..=VK_Z` directly. Analysis:
  [`UPSTREAM-gpui-windows-keyboard-layout.md`](UPSTREAM-gpui-windows-keyboard-layout.md).
- **gpui_wgpu: fresh ScaleContext per glyph** *(the pinned tip)* —
  [`f3f236eb58663cf0e43f866be3d25833918c4452`](https://github.com/kirushik/zed/commit/f3f236eb58663cf0e43f866be3d25833918c4452).
  Drops the long-lived shared swash `ScaleContext` whose caches make glyph
  rasterization non-deterministic across a window scale change, the Wayland
  scale-change corruption. Analysis:
  [`UPSTREAM-gpui-scale-bug.md`](UPSTREAM-gpui-scale-bug.md).

## Re-syncing on a gpui bump

A bump changes the base rev. In the fork: branch off the new upstream rev,
`git cherry-pick` the two commits above (or rebase `strop-patches` onto it),
resolve any drift, push, and update the rev in both `gpui` and `gpui_platform`
in `Cargo.toml`. `cargo build` refreshes `Cargo.lock` (commit it —
`release.yml` builds `--locked`). If the fork owner ever changes, update
`deny.toml`'s `[sources.allow-org]` too.

## Upstreaming

Each commit is its own concern — cherry-pick it onto a `main`-based branch and
open a separate PR. The `strop-patches` branch stays at the old base rev purely
for Strop to pin; don't PR it directly.

## If the fork is ever lost

The build pins the fork's URL + rev, so it can't be rebuilt from this repo
alone — keeping `kirushik/zed` (and its `strop-patches` branch) reachable is a
real external dependency. To recover: re-fork zed at the base rev and re-create
the two commits from the analysis docs above (each is a few lines).
