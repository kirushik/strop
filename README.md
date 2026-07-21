# Strop

> **strop** — a strip of leather you rub a razor on to make the blade sharp.
> 
> **strop** *(UK informal)* — a bad mood, especially one in which a person will not do as they are asked.
> 
> <sub>— [Cambridge Dictionary](https://dictionary.cambridge.org/dictionary/english/strop)</sub>

**In the human–AI centaur, don't be the horse.**

<p align="center">
  <img src="assets/screenshot.png" width="820"
       alt="Strop reading a short story: four editorial diagnoses anchored in the right margin — named problems asked as questions, never a rewrite — with the active card's passage tinted in the text and a highlighted phrase glowing amber.">
</p>

In the *advanced chess* Garry Kasparov championed, a human paired with a machine
beats both the best grandmaster and the best engine — as long as the human stays
in charge of the ideas. Most AI writing tools invert that: the machine drafts, you
approve, and your voice dissolves into a language model's median style.

Strop keeps you in charge. You write every sentence. The machine is the editor in
the margin — it reads your draft the way a sharp editor would, names what isn't
working as a question, and never rewrites a line.

## What it does

- **An editor that diagnoses, never prescribes.** One button (or a shortcut) asks the editor for a read at the depth you choose — developmental, line, or copy. It returns named problems as questions in the margin ("Is this the real beginning?"), never replacement text; a believing read tells you what already works instead. Dismiss a card and it stays gone. The commentary comes in your manuscript's language.
- **Your voice, not the model's.** Bring your own LLM — any OpenAI-compatible API, including a local Ollama. A read only ever sends text to the provider you pick; point it at a local model and nothing leaves your machine.
- **A calm canvas.** Typing-first, with typography handled as you write — language-aware quotes, em dashes, the right non-breaking spaces — and any substitution undone with a single keystroke. Select something and the formatting toolbar comes to the selection; images sit in the text as furniture ordinary editing can't corrupt.
- **History you own.** `.strop` files auto-save and carry the whole editing history — Undo works after a restart. A scrubbable strip along the top of the page moves through the document's past like a seek bar; named versions restore in a click, and restoring is itself undoable.
- **Nothing is lost.** A compost rail keeps the ideas and clippings that aren't manuscript yet; every paragraph you cut lands in the graveyard at the document's foot, one "Put back" from returning whole.
- **A room for reading.** When the draft is done, a cold read lays it out in book typography — justified, hyphenated, paged — with no caret to fiddle with; you read, and file reactions in the margin.
- **Plain, portable files.** One `.strop` file per document; Markdown in and out.

## Install

Grab the [latest release](https://github.com/kirushik/strop/releases/latest):

- **Windows** — a per-user installer (no admin rights) or a portable zip. The binaries are not yet Authenticode-signed, so SmartScreen asks once — *More info → Run anyway*; every artifact carries a verifiable [build-provenance attestation](https://github.com/kirushik/strop/attestations) instead.
- **macOS** (Apple silicon) — a DMG, signed and notarized.
- **Linux** — `.deb` and `.rpm` packages; the launcher entry, icons, and the `.strop` file association wire up on install.

Installed builds keep themselves current, carefully: each release's manifest is signed with the project's [minisign key](minisign.pub) and verified by the binary itself before a byte is trusted. Updates stage quietly in the background and apply on the next launch — no prompts, no interruptions — and "About Strop" always says what you're running and what just changed. Package-manager builds never self-update.

## Where it's at

Strop is early, and built in the open. **[v0.3.0](https://github.com/kirushik/strop/releases/tag/v0.3.0)** is the third cut — the release that leaves the workshop: installers, file associations, an updater that answers to the project's own signing key, and an About box that owes nobody anything; see the [changelog](CHANGELOG.md). **Linux (Wayland)** is the primary runtime-tested platform; macOS and Windows build and pass launch smokes in CI, with outside testers filling the runtime gap. Building from source remains fully supported: see **[DEVELOPMENT.md](DEVELOPMENT.md)**.

## License

[GPL-3.0-or-later](COPYING). Built on Zed's
[gpui](https://github.com/zed-industries/zed); bundled PT fonts are © ParaType
under the SIL Open Font License, and URW Bookman ships under AGPL-3.0 with the
font-embedding exception. Attribution in [NOTICE](NOTICE).
