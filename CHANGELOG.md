# Changelog

All notable changes to Strop are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); Strop uses
[semantic versioning](https://semver.org/) once it stabilises (pre-1.0, minor
versions may still break things).

## [Unreleased]

## [0.1.1] — 2026-06-19

Windows papercuts and layout-independent shortcuts. Still unsigned, and only
Linux (Wayland) is runtime-tested.

### Fixed
- **Windows: no stray console window** — the GUI release no longer opens a
  black console window beside it.
- **Windows: one title bar, and it drags** — the app's own title bar is no
  longer stacked under a native one, and the window drags from it (not only the
  native strip); the title-bar buttons stay clickable.
- **Shortcuts work from any focus** — `Ctrl+Shift+P` and the other menu
  commands now fire while the command palette, a margin-note field, or the AI
  settings panel is focused — not only when the document has focus. (All
  platforms.)
- **Keyboard-layout-independent shortcuts on Windows** — letter chords like
  `Ctrl+Shift+P` fire under non-Latin layouts (e.g. Cyrillic), matching the
  existing Linux behaviour.
- **Open / reveal work off Linux** — "Reveal in Files", opening `config.toml`,
  and the "Get a key" link use each OS's native handler instead of Linux-only
  `xdg-open` / `gdbus`.

### Internal
- gpui patches — the Windows keyboard fix and the Wayland scale-change glyph
  workaround — are consolidated onto a single zed fork rev. The vendored
  `gpui_wgpu` crate and its `[patch]` override are removed; see
  `docs/gpui-fork.md`.

## [0.1.0] — 2026-06-17

The first rough, early cut. Strop is a writer's editor built on the thesis that
the machine should **diagnose, never prescribe** — you write every sentence, and
the AI reads your draft the way a sharp editor would, naming what isn't working
as a question in the margin.

This is an MVP: a working, deliberately rough demonstration of every necessary
feature, end to end. Only **Linux (Wayland)** is runtime-tested; macOS and
Windows binaries build and pass a headless launch smoke but are otherwise
unverified and unsigned.

### The editorial core
- **Diagnosis pass** (`ctrl-shift-d`): a depth-selectable editorial read
  (developmental / line / copy) that returns named problems as questions in the
  margin and never replacement text. Quotes are anchored against the current
  draft; dismissed diagnoses are never re-raised on the same span; the whole
  pass is one undoable transaction.
- **The door** (`ctrl-shift-r`): a per-session Drafting/Reviewing lens. Drafting
  quiets the editorial margin to a thin rail; Reviewing shows the cards. Your own
  notes are never hidden. Reviewing enforces the developmental → line → copy
  order.
- **Believing mode** (`ctrl-shift-b`): Elbow's believing game as a pass — named
  working moves and the draft's center of gravity, praise-adjectives banned.
- **Bring your own model**: any OpenAI-compatible chat API — OpenAI, OpenRouter,
  Poe, a local Ollama, Anthropic-compatible endpoints. A pass only ever sends
  text to the provider you choose; point it at a local model and nothing leaves
  the machine. First-launch onboarding offers a key-free local path when Ollama
  is present.
- **Margin annotations** (`ctrl-m`): your own notes alongside the AI's, with a
  Google-Docs-style margin solver and bidirectional anchor/card activation.

### Writing surface
- Rich text with a PT superfamily face stack (PT Serif body, PT Sans headings,
  PT Mono code), bold/highlight, and durable formatting persisted as Loro
  Peritext marks.
- Block model: paragraphs, headings 1–3, quotes, lists, dividers, code blocks,
  footnotes — with Markdown-style block commands and keyboard toggles.
- Language-aware typography applied as you type (quotes, em dashes, non-breaking
  spaces), each substitution undoable with a single keystroke.
- Find and replace (`ctrl-f` / `ctrl-h`) with live match highlighting.
- Images: paste and file-drop, privacy-preserving import (EXIF-stripping),
  content-addressed and deduplicated in-file.

### Files & history
- One self-contained `.strop` file per document, auto-saving, carrying the entire
  editing history — undo survives restarts.
- Markdown import and export (byte-exact roundtrip; assets exported alongside).
- Named checkpoints (`ctrl-alt-s`) and a reading-order history view
  (`ctrl-alt-h`) with word-level inline diff and restore-as-forward-edit.
- A descriptive voice-drift indicator (stylometric proxy) between any checkpoint
  and the current draft — drift signals, never an identity claim.

### Platform & project
- Linux (Wayland) desktop shell on a vendored, patched gpui 0.2.2.
- Single-instance-per-file handling; window bounds remembered across launches.
- Configuration via `~/.config/strop/config.toml`.
- GPL-3.0-or-later. Supply-chain gating (cargo-deny) and three-OS CI.

[Unreleased]: https://github.com/kirushik/strop/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/kirushik/strop/releases/tag/v0.1.1
[0.1.0]: https://github.com/kirushik/strop/releases/tag/v0.1.0
