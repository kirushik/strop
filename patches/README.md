# Patches against the pinned gpui (zed) tree

Strop pins `gpui` / `gpui_platform` to a single zed git rev (see the root
`Cargo.toml`). When a fix has to live *inside* that tree and upstreaming it
isn't on the table yet, we carry it here as a `.patch` and consume it by
pointing the git deps at a **fork of zed** (github.com/kirushik/zed, branch
`strop-patches`) that has the patches applied on top of the pinned rev. Every
patched crate rides one fork rev — a single source of truth, and no vendored
dependency source in this repo.

Why a fork and not the alternatives:

- **cargo-patch (the tool)** is build-time codegen: it needs
  `cargo install cargo-patch` + a `cargo patch` prebuild step on every
  (ephemeral) CI runner, plus a bootstrap dance with a `[patch]` entry that
  points at a not-yet-generated `target/patch/` dir. It also doesn't currently
  compile on the pinned toolchain (rotted gitoxide/`time` deps). Wrong shape
  for CI.
- **Vendoring the crate(s)** (the old `vendor/gpui_wgpu` approach) works but
  copies third-party source into this repo and needs a `[patch]` override per
  crate. Replaced by the fork.
- **A zed fork** keeps this tree to a rev bump and is a plain git dependency on
  CI — nothing to install, no prebuild step.

## Patches (the source-of-truth diffs)

Each is a commit on the fork's `strop-patches` branch; these `.patch` files are
the record, for re-applying after a gpui bump and for the eventual upstream PRs.

- **`gpui_windows-keyboard-layout.patch`** — layout-independent keyboard
  shortcuts on Windows. `get_key_from_vkey` derived the keystroke key via
  `MapVirtualKeyW(.., MAPVK_VK_TO_CHAR)` (the *active* layout), so under a
  Cyrillic layout the physical `P` key yields `з` and `ctrl-shift-p` (and every
  letter chord) stops matching. Returns the US-layout letter for `VK_A..=VK_Z`
  directly. Analysis: `docs/UPSTREAM-gpui-windows-keyboard-layout.md`.
- **`gpui_wgpu-scale-context.patch`** — a fresh swash `ScaleContext` per glyph
  in `render_glyph_image`. The shared context's caches make rasterization
  non-deterministic across a window scale change, poisoning the sprite atlas
  with wrong-size glyphs after a Wayland monitor-to-monitor move. Formerly
  `vendor/gpui_wgpu`. Analysis: `docs/UPSTREAM-gpui-scale-bug.md`.

## Applying (creating / refreshing the fork)

Replace `<REV>` with the rev currently pinned in the root `Cargo.toml`.

```sh
git clone git@github.com:kirushik/zed.git zed && cd zed
git checkout -b strop-patches <REV>
git apply /path/to/strop/patches/gpui_windows-keyboard-layout.patch
git apply /path/to/strop/patches/gpui_wgpu-scale-context.patch
# one commit per patch keeps each cherry-pickable for its own upstream PR
git push origin strop-patches
git rev-parse HEAD          # -> the SHA to pin in Strop
```

Then point Strop at the fork (root `Cargo.toml`) — just the two git deps. No
`[patch]` block: every patched crate now comes from the fork.

```toml
gpui          = { git = "https://github.com/kirushik/zed", rev = "<NEW_SHA>", … }
gpui_platform = { git = "https://github.com/kirushik/zed", rev = "<NEW_SHA>", … }
```

`cargo build` refreshes `Cargo.lock`; commit it, since `release.yml` builds
`--locked`. (A new fork owner also needs adding to `deny.toml`'s
`[sources.allow-org]`.)

## Re-syncing on a gpui bump

The bump changes `<REV>`. In the fork: branch off the new rev, re-apply (or
`git cherry-pick` / rebase `strop-patches`), resolve any drift, push, update the
rev in Strop. If a patch stops applying cleanly, regenerate the `.patch` here
from the new tree so this folder stays the source of truth.

## Upstreaming

Each patch is its own concern — cherry-pick each commit onto a `main`-based
branch and open a separate PR. The `strop-patches` branch stays at the old rev
purely for Strop to pin; don't PR it directly.
