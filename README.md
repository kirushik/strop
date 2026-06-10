# Strop

A writer's editor with an editor inside.

Strop diagnoses prose the way a good human editor does — naming problems and
leaving the fix to the author — and is built to *resist* the voice
homogenization that LLM writing tools cause. Rich-text documents with full
persistent history; Markdown as an export format; typography a Bureau-school
designer could live with.

Status: v0 scaffold. A GPUI window proving the typographic canvas
(Literata 20/28, 64ch measure). No editing yet.

- `crates/strop-core` — framework-agnostic engine: buffer, history, typograph,
  diagnostics. Never imports the UI.
- `crates/strop-app` — thin GPUI shell.
- `docs/DECISIONS.md` — why everything is the way it is.
- `ai-writers-editor-handoff.md` — the editorial-theory research this is built on.

```sh
cargo run -p strop-app
```
