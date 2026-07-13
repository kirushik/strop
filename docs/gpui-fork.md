# The gpui fork (where our gpui-tree patches live)

Strop's `gpui` / `gpui_platform` are pinned in the root `Cargo.toml` to a
**fork** of zed-industries/zed — **<https://github.com/kirushik/zed>**, branch
`strop-patches-on-main` — based at upstream stable tag `v1.10.2`, rev
`adc60ccf` (2026-07-10), plus the four commits below.

**The fork is the single source of truth** for everything we change inside the
gpui tree: the pinned rev is exactly what compiles, and these commits *are* the
diffs. There is no vendored dependency source in this repo, and no second copy
of the patches to drift out of sync — to read a change, open its commit.

> **2026-07-13 stable rebase.** The branch moved from the June upstream-main
> snapshot to stable `v1.10.2`. The three standing patches still applied
> cleanly; the macOS renderer-fingerprint diagnostic commit was dropped because
> it was never pinned and that investigation is complete. A fourth patch makes
> SVG text/font support opt-in, allowing Strop to remove `rustybuzz` while Zed's
> default GPUI behavior remains unchanged.
>
> The original **2026-06-19 dependency review** (see
> [`dependency-review-2026-06.md`](dependency-review-2026-06.md)): this branch
> replaced the previous `strop-patches` / `f3f236eb` pin. Rebasing onto upstream
> `main` drops the `async-std`/`async-tar` subtree (clears RUSTSEC-2025-0052), and
> a third commit narrows `image` to the formats gpui decodes.

## Patches (commits on `strop-patches-on-main`, tip `994cdfd1`)

SHAs change whenever the branch is rebased/re-signed onto a newer base — the
authoritative pinned rev is always the one in the root `Cargo.toml`.

- **gpui_windows: layout-independent letter keys** —
  [`f729778df66e3f251833062c81a814715677ae09`](https://github.com/kirushik/zed/commit/f729778df66e3f251833062c81a814715677ae09).
  `get_key_from_vkey` mapped VK codes through the *active* keyboard layout, so
  letter chords (`Ctrl+Shift+P`, …) stopped matching under non-Latin layouts
  (the physical `P` key yields `з` under Cyrillic). Returns the US-layout letter
  for `VK_A..=VK_Z` directly. Analysis:
  [`UPSTREAM-gpui-windows-keyboard-layout.md`](UPSTREAM-gpui-windows-keyboard-layout.md).
- **gpui_wgpu: fresh ScaleContext per glyph** —
  [`6dfc9a0ec798d84ca3173b06c64e833b19b66745`](https://github.com/kirushik/zed/commit/6dfc9a0ec798d84ca3173b06c64e833b19b66745).
  Drops the long-lived shared swash `ScaleContext` whose caches make glyph
  rasterization non-deterministic across a window scale change, the Wayland
  scale-change corruption. Analysis:
  [`UPSTREAM-gpui-scale-bug.md`](UPSTREAM-gpui-scale-bug.md).
- **gpui: narrow `image` to the formats gpui decodes** —
  [`b95b46dac53757849bb3c0d4bdf81df992228037`](https://github.com/kirushik/zed/commit/b95b46dac53757849bb3c0d4bdf81df992228037).
  Default `image` features drag in the rav1e AV1 encoder, exr and qoi — none of
  which gpui's `ImageFormat` decodes; narrows the four gpui-family crates that
  declare `image` to png/jpeg/webp/gif/bmp/tiff/ico/pnm. Unlike the two bug
  fixes, this is a Strop-specific trim, not headed upstream. Detail:
  [`dependency-review-2026-06.md`](dependency-review-2026-06.md).
- **gpui: make SVG text rendering optional** *(the pinned tip)* —
  [`994cdfd1a274030628911b607fcee96cedbce059`](https://github.com/kirushik/zed/commit/994cdfd1a274030628911b607fcee96cedbce059).
  Moves resvg's text, system-font and mmap-font features behind GPUI's
  `svg-text` feature. GPUI enables it by default, preserving full Zed behavior;
  Strop uses `default-features = false` and deliberately omits it. SVG shapes
  and raster images continue to render, while the unused rustybuzz shaping
  stack disappears from Strop's graph.

## Re-syncing on a gpui bump

A bump changes the base rev. In the fork: rebase `strop-patches-on-main` onto the
new upstream rev (or branch off it and `git cherry-pick` the four commits
above), resolve any drift, push, and update the rev in both `gpui` and
`gpui_platform` in `Cargo.toml`. `cargo build` refreshes `Cargo.lock` (commit it
— `release.yml` builds `--locked`). Note the fork commits are signed with a FIDO
key, so re-signing/rebasing rewrites their SHAs — the pinned rev is whatever
lands in `Cargo.toml`. If the fork owner ever changes, update `deny.toml`'s
`[sources.allow-org]` too.

## Upstreaming

The two **bug-fix** commits are each their own concern — cherry-pick onto a
`main`-based branch and open a separate PR. The `image` trim is a downstream
narrowing. The `svg-text` feature boundary is independently upstreamable, but
Strop does not depend on that happening. The combined branch stays on the fork
purely for Strop to pin; don't PR it directly.

## If the fork is ever lost

The build pins the fork's URL + rev, so it can't be rebuilt from this repo
alone — keeping `kirushik/zed` (and its `strop-patches-on-main` branch) reachable
is a real external dependency. To recover: re-fork zed at the base rev, re-create
the two bug-fix commits from the analysis docs above, re-apply the `image` trim
per [`dependency-review-2026-06.md`](dependency-review-2026-06.md), and restore
the `svg-text` boundary described above.
