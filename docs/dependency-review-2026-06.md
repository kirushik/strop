# Dependency review — 2026-06 (v0.2.0)

A full pass over Strop's dependency tree: duplicates, version freshness, feature
bloat, reimplementation candidates, supply-chain surface, build size. Done as an
open-ended audit; this is the record of what was found, what changed, and what
was deliberately left alone.

## TL;DR

| Metric | Before (pin `f3f236eb`) | After | Δ |
| --- | --- | --- | --- |
| `Cargo.lock` packages | 826 | 783 | **−43 (−5.2%)** |
| Distinct duplicated crate names | 44 | 40 | −4 |
| `async-std` / `async-tar` | present | **gone** | clears RUSTSEC-2025-0052 |
| Release binary (`target/release/strop`) | 50.9 MB | 49.9 MB | −1.0 MB |
| Tests | 135 pass | 135 pass | no regression |
| Clippy | clean | clean | — |

Both the baseline and the after-figures are measured against the **git** pin
(apples-to-apples). All changes were validated by building Strop against the
rebased fork (`cargo build`/`test`/`clippy`/`build --release`, all green).

> **On the image trim and the package count.** The −43 lock-count win is the
> async-std cluster (from the rebase) plus version unification. The `image` trim
> is a **compile-time and binary-size** win, not a lock-count one: it stops
> *compiling* the avif (`ravif`/`rav1e`/`av1-grain`/`avif-serialize`) and `exr`
> codecs (verified — the enabled `image` features are only
> png/jpeg/webp/gif/bmp/tiff/ico/pnm + rayon), but Cargo still **version-pins**
> those codecs in `Cargo.lock` as `image`'s unused optional deps. They are never
> downloaded or built for any target. Re-resolving the lock (`cargo
> generate-lockfile`) does not remove them.

## The shape of the tree

826 packages, but Strop's own manifests are lean. The duplicate/version sprawl
is **almost entirely inherited from gpui** (the Zed fork drags wgpu, naga,
cosmic-text, accesskit/atspi, zbus, resvg, …). Examples before this pass:
`itertools` ×4, `hashbrown` ×4, `bitflags` ×2 — all transitive, none ours. Our
direct deps already keep a single version each, and the only "two versions of X"
that touched our own choices (`toml_edit` 0.22 vs 0.25) turned out to be a
transitive build-dep artifact we don't control.

Conclusion: **the leverage is in gpui's feature/dependency surface, not in
Strop's manifests.** Since the gpui pin is a *writable* fork
(`github.com/kirushik/zed`, local checkout at `../../Thirdparty/zed`), that's
where the big cuts were made.

## Changes made

### Fork (`kirushik/zed`, branch `strop-patches-on-main`, tip `96bebcc2db`)

1. **Rebased the two gpui-tree patches onto upstream `main`** (origin/main @
   `69b602c7`, 2026-06-18 — 122 commits past the old base). Both patch files are
   byte-identical on the new base, so the rebase was conflict-free. This is the
   single biggest win and it is *free of any code change*: upstream landed a
   refactor that moves `async-fs`/`async-tar`/`sha2`/`tempfile`/`util` behind a
   new optional `github-download` feature on `http_client`. `gpui` /
   `gpui_linux` depend on `http_client` **without** that feature (only Zed's
   `languages`/`project` crates enable it), so the rebase deletes the whole
   `async-tar → async-std → async-channel v1` cluster — exactly the
   RUSTSEC-2025-0052 "async-std discontinued" advisory we suppress in
   `deny.toml`. It also unified `rustix`/`nix`/`linux-raw-sys`/`event-listener`/
   `async-channel`/`futures-lite` down to single versions.

2. **Narrowed `image` to the formats gpui actually decodes.** gpui's
   `ImageFormat` (platform.rs) only handles png/jpeg/webp/gif/bmp/tiff/ico/pnm;
   `image`'s default features additionally pull `avif` (→ `ravif` → `rav1e`, a
   ~50-crate AV1 *encoder*), `exr`, `qoi`, `hdr`, `dds`, `tga`. Set
   `default-features = false` + the explicit eight formats on the four
   gpui-family crates that declare `image` (gpui, gpui_linux, gpui_macos,
   gpui_windows). The workspace `image` default is untouched, so the wider Zed
   app crates keep their formats — only gpui (and thus Strop) slims down. This
   stops those codecs from being **compiled** (binary + cold-build win); they
   remain version-pinned in `Cargo.lock` as `image`'s optional deps but are never
   built — see the note under TL;DR.

   Note: this could not be expressed as `image = { workspace = true,
   default-features = false }` — Cargo forbids overriding `default-features` on a
   `workspace = true` inherit — so the gpui-family crates declare `image` with an
   explicit `version` instead.

### Strop repo

3. **Dropped `anyhow`** from `strop-app` — it was declared but had zero
   references anywhere in the source.
4. **Narrowed `similar` to `["unicode"]`** — the `inline` feature was enabled but
   no inline-diff API (`iter_inline_changes`/`InlineChange`) is used; the diff
   code only needs `from_unicode_words` + core ops.

## Deliberately NOT changed (discipline cuts both ways)

- **`toml` + `toml_edit` kept as a pair.** They split cleanly: `toml` for
  deserialization (`toml::from_str` in config.rs), `toml_edit` for the
  comment-preserving config writer. Consolidating onto `toml_edit::de` would save
  ~1 crate but requires enabling `toml_edit`'s `serde` feature — which would *add*
  serde plumbing to the `toml_edit` already shared with gpui's
  `proc-macro-crate` → zbus chain. Net ≈ wash, with real churn in a delicate
  writer. Not worth it.
- **`image`'s `rayon` feature kept.** Tempting to drop (Strop's image work is
  decode + re-encode, which rayon doesn't accelerate), but `rayon` is
  load-bearing in gpui via `sum_tree` regardless — so the feature is *free*, and
  dropping it would only risk image-op latency for no supply-chain gain.
- **Reimplementation candidates declined.** The single-use small deps
  (`blake3` = content hashing, `glob` = `**` matching, `directories` =
  cross-platform XDG/mac/Win paths, `interprocess` = local-socket single-instance)
  are each either a correctness/crypto concern or genuinely cross-platform.
  Hand-rolling any of them trades a few transitive crates for a maintenance and
  correctness liability. Not worth it.

## Deferred follow-ups (need fork *code* surgery, higher risk)

These are the remaining large inherited subtrees. All are **hard** — gpui uses
them unconditionally — so they need feature-gating work inside the fork (ideally
upstreamed), not a manifest tweak. Rough sizes in Strop's current tree:

| Subtree | ~crates | Why it's hard | Notes |
| --- | --- | --- | --- |
| SVG: `resvg`/`usvg`/`tiny-skia`/`fontdb`/`svgtypes`/`roxmltree` | ~70 | gpui renders icons through `svg_renderer.rs` + `ImageFormat::Svg` unconditionally | **Strop never calls `gpui::svg()`** (verified) — highest-value cut if gpui gained a `svg` feature gate. |
| Accessibility: `accesskit`/`accesskit_unix`/`atspi*` | ~25 | hard deps in gpui + gpui_linux, woven through window/div/text | `zbus` would stay (see below). |
| Keyring: `oo7` (+ `aes`/`cipher`) | ~10–15 | hard dep in gpui_linux, 3 call sites in `linux/platform.rs` | `zbus` stays — `ashpd` needs it for the file dialogs Strop *does* use. |

## Finalize (the parts this review couldn't push)

The review session has write access to the local fork checkout but **cannot
push** to `kirushik/zed` (FIDO-key signing). To land:

1. **Push the fork branch:** in `../../Thirdparty/zed`,
   `git push origin strop-patches-on-main` (commits are `--no-gpg-sign`).
2. **Swap Strop's pin:** in this repo's root `Cargo.toml`, replace the TEMP
   local-path override with the `git + rev = 96bebcc2db…` lines already written
   just above it, then `cargo build` to refresh `Cargo.lock` (commit the lock).
3. **Drop the cleared advisory:** remove the `RUSTSEC-2025-0052` line from
   `deny.toml`'s `ignore` list (async-std is gone). The other four ignores stay.
4. Optional: rename `strop-patches-on-main` → `strop-patches` if you prefer the
   stable branch name (the rev pin is by SHA, so the name doesn't matter to the
   build).

Until step 2, the root `Cargo.toml` carries an absolute local-path override
(marked TEMP) so the trimmed tree builds locally; that override must not be
committed.

## How to re-measure

`cargo`/`rustc` on PATH are broken snap shims in this environment; use the real
toolchain and set `RUSTC`:

```sh
TC=$HOME/.rustup/toolchains/1.96.0-x86_64-unknown-linux-gnu/bin
export RUSTC=$TC/rustc PATH=$TC:$PATH
$TC/cargo tree --duplicates | grep -E '^[a-z]' | awk '{print $1}' | sort -u | wc -l  # dup names
grep -c '^\[\[package\]\]' Cargo.lock                                                # package count
$TC/cargo tree -i async-std                                                          # advisory subtree
```
