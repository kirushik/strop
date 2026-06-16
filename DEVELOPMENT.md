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
