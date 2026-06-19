# Developing Strop

Strop is a Rust workspace. The engine (`strop-core`) is framework-agnostic; the
shell (`strop-app`) is built on [gpui](https://github.com/zed-industries/zed).

## Build from source

You need a recent Rust (1.85+, for the 2024 edition — [rustup.rs](https://rustup.rs))
and the system libraries gpui depends on. On Debian/Ubuntu:

```sh
sudo apt-get install -y pkg-config libwayland-dev libxkbcommon-dev \
  libxkbcommon-x11-dev libvulkan-dev libgbm-dev libgl1-mesa-dev \
  libasound2-dev libfontconfig-dev libssl-dev libxcb1-dev libx11-dev libxext-dev
```

Then:

```sh
# The first build is long: it compiles a pinned fork of Zed's gpui (wgpu + Wayland).
cargo run --release -p strop-app

# open a document directly:
cargo run --release -p strop-app -- ~/Documents/Strop/draft.strop

# open (and import) a Markdown file:
cargo run --release -p strop-app -- notes.md
```

The built binary is named `strop`. To register `.strop` files for double-click in
your file manager, run `scripts/install-desktop.sh`.

## Set up an AI provider

The diagnosis pass speaks to any **OpenAI-compatible** chat API — OpenAI,
OpenRouter, Poe, a local [Ollama](https://ollama.com), Anthropic-compatible
endpoints, and so on. Nothing is sent anywhere until you run a pass.

Run **"Set Up AI Provider"** from the command palette (`ctrl-shift-p`) to write a
commented `~/.config/strop/config.toml`, then add your endpoint, model, and key.
`STROP_API_KEY` in the environment overrides the file.

## Layout

- **`crates/strop-core`** — the engine: text buffer, document model, typography,
  Markdown, images, storage, and the LLM diagnosis. Never imports the UI.
- **`crates/strop-app`** — the GPUI / Wayland shell.
- **`docs/`** — the reasoning: [`DECISIONS.md`](docs/DECISIONS.md),
  [`editorial-foundations.md`](docs/editorial-foundations.md),
  [`DESIGN.md`](docs/DESIGN.md), [`document-model.md`](docs/document-model.md),
  [`ROADMAP.md`](docs/ROADMAP.md). Keyboard reference:
  [`docs/keymap-baseline.md`](docs/keymap-baseline.md) (or `ctrl-?` in-app).

## Tests and conventions

```sh
cargo test  -p strop-core          # engine, incl. the property suite
cargo clippy -p strop-core -p strop-app --all-targets -- -D warnings
```

This repo is **hand-formatted on purpose** — do **not** run `cargo fmt`; it would
reflow large stretches of untouched code, and there is no format gate in CI. CI
runs the `strop-core` gate (clippy + property tests) on every push; the `strop-app`
job builds the git-pinned gpui and runs headless logic tests.

## Release builds

Guiding principle: **a release is built once (in CI, on a `v*` tag) but the binary
is downloaded and run many times.** So we spend CI compile time freely to make the
artifact smaller and faster — the cost lands on one machine, the benefit on every
user, on every launch.

`.github/workflows/release.yml` builds the **`dist`** Cargo profile (`cargo build
--profile dist`), kept separate from `release` so a local `cargo build --release`
stays quick. The profile mirrors Zed's own gpui release profile:

- `lto = "thin"` — ~all of fat-LTO's win with a parallelizable link (fat's extra
  gain on a tree this size is low single digits for a long serial link tail).
- `codegen-units = 1` for whole-graph optimization, with the one big app crate
  (`strop-app`) overridden back to `16` so it doesn't serialize the compile.
- `opt-level = 3` — Strop is latency-sensitive (GPU text/scroll hot paths), so
  **not** `s`/`z`.
- `strip = "symbols"`, then CI runs a **full** `strip` on the artifact (cargo's
  strip leaves the ~5 MB static symbol table on the pinned toolchain). The strip
  command is per-OS — see the comment in `release.yml` (GNU `--strip-all` on
  Linux; bare `strip` + ad-hoc `codesign` on macOS; skipped on Windows, whose
  symbols live in a separate `.pdb`).

Deliberate non-choices, so they don't get "optimized" back in:

- **Keep `panic = unwind`** (not `abort`). The win is the smallest of all knobs
  and it's the only one that changes runtime semantics — a panic would abort with
  no Drop/cleanup, worse for an editor with unsaved work. Zed keeps unwind too.
  Revisit only alongside a crash reporter.
- **Baseline `target-cpu`** (not `v2`/`v3`). The SIMD-heavy crates
  (wgpu/cosmic-text/swash) already runtime-detect; `v3` (AVX2) SIGILL-crashes on
  older/low-power x86 with no fallback — unacceptable support risk for a
  distributed binary. If ever wanted, set it via CI `RUSTFLAGS`, not the profile.
- **No UPX / executable packing.** It decompresses on *every launch* (startup
  latency — wrong for a run-many app) and trips antivirus / macOS notarization.

Measured size deltas from the 2026-06 pass are recorded in
[`docs/dependency-review-2026-06.md`](docs/dependency-review-2026-06.md) (≈49.9 MB
default-release → 35.8 MB stripped `dist`, −28%).
