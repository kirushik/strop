# Strop

A writer's editor with an editor inside.

Strop diagnoses prose the way a good human editor does — naming problems
as queries to the author, never rewriting a word — and is built to resist
the voice homogenization that LLM writing tools cause. Documents live in
single portable `.strop` files carrying their full edit history.

## What works (MVP, Linux/Wayland)

- **Typing-first canvas**: PT Serif 20/28 on a 64ch measure, cursor
  affinity, goal-x vertical motion, the full GTK keyboard/mouse baseline
  (see docs/keymap-baseline.md), PRIMARY selection, drag-by-word.
- **Typograph as you type**: document-language-aware «ёлочки»/"curly"
  quotes, em dashes with NBSP binding, RU short-word NBSP, ellipsis —
  deterministic, and one ctrl-z always restores exactly what you typed.
- **Rich text**: bold/italic/underline/strike/highlight/inline code
  (ctrl-b/i/u, ctrl-shift-x/h, ctrl-e), headings (`# ` shortcut or
  ctrl-alt-1..3, PT Sans Bold on the 28px rhythm), quotes, lists,
  dividers, code blocks (PT Mono), footnotes with a viewport bottom zone
  (ctrl-alt-f), images (paste/drop; EXIF-stripped, size-capped, stored
  in-file content-addressed).
- **History**: word-coalesced undo that survives restarts; named
  checkpoints (ctrl-alt-s) with a rewind panel (↺) — Google-Docs Rewind
  in a local-first file; every keystroke retained in the Loro op log.
- **Markdown**: byte-exact roundtrip import/export (`strop notes.md`
  imports; ctrl-shift-e exports next to the file).
- **The margin**: ctrl-m author notes; **ctrl-shift-d runs an editorial
  diagnosis** through any OpenAI-compatible provider (Poe/OpenAI/
  OpenRouter/ollama/Anthropic-compat) — named problems anchored in the
  right margin as questions, never replacement text, at a chosen depth
  (developmental / line / copy — the levels-of-edit switch). Dismissed
  diagnoses are never re-raised. The margin teaches its own setup,
  shows run status, and names failures actionably.
- **The shell**: ctrl-shift-p command palette (every command, fuzzy,
  EN+RU aliases, recent documents) — the menu of a chrome-minimal
  editor; ctrl-? keyboard map; first run opens a live tutorial document
  with pre-seeded margin cards.
- **Files, visible from birth**: documents live in ~/Documents/Strop;
  ctrl-n new window, F2/titlebar-click renames file and all, Reveal in
  Files, recents; scripts/install-desktop.sh registers .strop
  double-click.
- **Settings**: ~/.config/strop/config.toml — "Set Up AI Provider"
  writes a commented template; re-read before every pass; STROP_API_KEY
  overrides the file.

```sh
cargo run --release -p strop-app [file.strop|file.md]
```

- `crates/strop-core` — framework-agnostic engine (buffer, document,
  typograph, markdown, images, store, llm, diagnose). Never imports UI.
- `crates/strop-app` — the GPUI shell.
- `docs/DECISIONS.md`, `docs/document-model.md`, `docs/ROADMAP.md` — why
  everything is the way it is, and what's next.
