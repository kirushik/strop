# The gpui fork (where our gpui-tree patches live)

Strop's `gpui` / `gpui_platform` are pinned in the root `Cargo.toml` to a
**fork** of zed-industries/zed — **<https://github.com/kirushik/zed>**, branch
`strop-patches-on-main` — based at upstream `main` rev `69b602c7` (2026-06-18)
plus the three commits below.

**The fork is the single source of truth** for everything we change inside the
gpui tree: the pinned rev is exactly what compiles, and these commits *are* the
diffs. There is no vendored dependency source in this repo, and no second copy
of the patches to drift out of sync — to read a change, open its commit.

> **2026-06-19 dependency review** (see
> [`dependency-review-2026-06.md`](dependency-review-2026-06.md)): this branch
> replaced the previous `strop-patches` / `f3f236eb` pin. Rebasing onto upstream
> `main` drops the `async-std`/`async-tar` subtree (clears RUSTSEC-2025-0052), and
> a third commit narrows `image` to the formats gpui decodes. The workspace is
> pinned to the tip, `c0a1cafa`.

## Patches (commits on `strop-patches-on-main`, tip `c0a1cafa`)

SHAs change whenever the branch is rebased/re-signed onto a newer base — the
authoritative pinned rev is always the one in the root `Cargo.toml`.

- **gpui_windows: layout-independent letter keys** —
  [`8eadb6bc7a1a8a10ee55e6ebcdcd3c2f6eb95f45`](https://github.com/kirushik/zed/commit/8eadb6bc7a1a8a10ee55e6ebcdcd3c2f6eb95f45).
  `get_key_from_vkey` mapped VK codes through the *active* keyboard layout, so
  letter chords (`Ctrl+Shift+P`, …) stopped matching under non-Latin layouts
  (the physical `P` key yields `з` under Cyrillic). Returns the US-layout letter
  for `VK_A..=VK_Z` directly. Analysis:
  [`UPSTREAM-gpui-windows-keyboard-layout.md`](UPSTREAM-gpui-windows-keyboard-layout.md).
- **gpui_wgpu: fresh ScaleContext per glyph** —
  [`e44a028a9f6507ed0c3846f6e12a54a9dba3eabb`](https://github.com/kirushik/zed/commit/e44a028a9f6507ed0c3846f6e12a54a9dba3eabb).
  Drops the long-lived shared swash `ScaleContext` whose caches make glyph
  rasterization non-deterministic across a window scale change, the Wayland
  scale-change corruption. Analysis:
  [`UPSTREAM-gpui-scale-bug.md`](UPSTREAM-gpui-scale-bug.md).
- **gpui: narrow `image` to the formats gpui decodes** *(the pinned tip)* —
  [`c0a1cafaef4e8d8060fa62e0a66c530433b353ba`](https://github.com/kirushik/zed/commit/c0a1cafaef4e8d8060fa62e0a66c530433b353ba).
  Default `image` features drag in the rav1e AV1 encoder, exr and qoi — none of
  which gpui's `ImageFormat` decodes; narrows the four gpui-family crates that
  declare `image` to png/jpeg/webp/gif/bmp/tiff/ico/pnm. Unlike the two bug
  fixes, this is a Strop-specific trim, not headed upstream. Detail:
  [`dependency-review-2026-06.md`](dependency-review-2026-06.md).

## Re-syncing on a gpui bump

A bump changes the base rev. In the fork: rebase `strop-patches-on-main` onto the
new upstream rev (or branch off it and `git cherry-pick` the three commits
above), resolve any drift, push, and update the rev in both `gpui` and
`gpui_platform` in `Cargo.toml`. `cargo build` refreshes `Cargo.lock` (commit it
— `release.yml` builds `--locked`). Note the fork commits are signed with a FIDO
key, so re-signing/rebasing rewrites their SHAs — the pinned rev is whatever
lands in `Cargo.toml`. If the fork owner ever changes, update `deny.toml`'s
`[sources.allow-org]` too.

## Upstreaming

The two **bug-fix** commits are each their own concern — cherry-pick onto a
`main`-based branch and open a separate PR. The `image` trim is a downstream
narrowing, not an upstream fix, so it stays Strop-only. The branch itself stays
on the fork purely for Strop to pin; don't PR it directly.

## If the fork is ever lost

The build pins the fork's URL + rev, so it can't be rebuilt from this repo
alone — keeping `kirushik/zed` (and its `strop-patches-on-main` branch) reachable
is a real external dependency. To recover: re-fork zed at the base rev, re-create
the two bug-fix commits from the analysis docs above, and re-apply the `image`
trim per [`dependency-review-2026-06.md`](dependency-review-2026-06.md) (each is a
few lines).
