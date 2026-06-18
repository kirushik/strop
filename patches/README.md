# Patches against the pinned gpui (zed) tree

Strop pins `gpui` / `gpui_platform` to a single zed git rev (see the root
`Cargo.toml`). When a fix has to live *inside* that tree and upstreaming it
isn't on the table yet, we carry it here as a `.patch` and consume it by
pointing the git deps at a **fork of zed** that has the patch applied.

Why a fork and not the alternatives:

- **cargo-patch (the tool)** is build-time codegen: it would need
  `cargo install cargo-patch` + a `cargo patch` prebuild step on every
  (ephemeral) CI runner, plus a bootstrap dance with a `[patch]` entry that
  points at a not-yet-generated `target/patch/` dir. It also doesn't currently
  compile on the pinned toolchain (rotted gitoxide/`time` deps). Wrong shape
  for CI.
- **Vendoring the whole crate** (the `vendor/gpui_wgpu` pattern) works but
  drags ~11.5k lines of the DirectX backend into this repo just to change three.
- **A zed fork** keeps this tree lean (only a rev bump) and is a plain git
  dependency on CI — nothing to install, no prebuild step.

## Patches

### `gpui_windows-keyboard-layout.patch`

Layout-independent keyboard shortcuts on Windows. `gpui_windows`'
`get_key_from_vkey` derives the keystroke key with `MapVirtualKeyW(..,
MAPVK_VK_TO_CHAR)`, which maps through the **active** layout — so under a
Cyrillic layout the physical `P` key yields `з`, and `ctrl-shift-p` (and every
other letter chord) stops matching. The patch returns the US-layout letter
directly for `VK_A..=VK_Z` (whose VK codes *are* the ASCII letters), restoring
layout independence. Analysis: `docs/UPSTREAM-gpui-windows-keyboard-layout.md`.

## Applying (creating / refreshing the fork)

Replace `<FORK>` with your zed fork (e.g. `git@github.com:kirushik/zed.git`)
and `<REV>` with the rev currently pinned in the root `Cargo.toml`.

```sh
git clone <FORK> zed && cd zed
git checkout -b strop-patches <REV>
git apply /path/to/strop/patches/gpui_windows-keyboard-layout.patch
git commit -am "gpui_windows: layout-independent letter keys for shortcuts"
git push origin strop-patches
git rev-parse HEAD          # -> the SHA to pin in Strop
```

Then point Strop at the fork (root `Cargo.toml`), in three places — both git
deps **and** the `[patch]` block must use the fork URL so the transitive
`gpui_wgpu` override still binds:

```toml
gpui          = { git = "https://github.com/<you>/zed", rev = "<NEW_SHA>", … }
gpui_platform = { git = "https://github.com/<you>/zed", rev = "<NEW_SHA>", … }

[patch."https://github.com/<you>/zed"]
gpui_wgpu = { path = "vendor/gpui_wgpu" }
```

Run `cargo check` (or `cargo update -p gpui`) to refresh `Cargo.lock`, since
`release.yml` builds `--locked`.

## Re-syncing on a gpui bump

The bump changes `<REV>`. In the fork: `git checkout -b strop-patches-<new>
<new-rev>`, re-apply (or `git cherry-pick` the previous patch commit / rebase
the `strop-patches` branch onto the new rev), resolve any drift, push, and
update the rev in Strop. If a patch no longer applies cleanly, regenerate the
`.patch` here from the new tree so this folder stays the source of truth.
