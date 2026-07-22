# Changelog

All notable changes to Strop are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); Strop uses
[semantic versioning](https://semver.org/) once it stabilises (pre-1.0, minor
versions may still break things).

## [Unreleased]

## [0.3.0] — 2026-07-22

The shipping release. Strop used to arrive as a bare binary in an archive;
0.3.0 makes it something you install, something that proves where it came
from, and something that keeps itself current. Real packages on every
platform, macOS builds signed and notarized, every artifact carrying a
verifiable build attestation, quiet self-updates checked against a signing
key baked into the app — and, underneath, a draft that backs itself up
before anything else touches it. The writing room moved too: footnotes
reached the book page, the keyboard map got a room of its own, and the
clipboard finally carries structure.

### Added
- **Quiet self-updates** — builds installed from GitHub releases check for
  a new version in the background and say so in one calm line. The update
  manifest is signed (minisign) and verified against a public key compiled
  into the binary — no key on disk, no env var, nothing to misplace.
  Downloads are size-capped, verified by hash, staged, and applied on the
  next start; a failed attempt is remembered and never retried in a loop.
  Package-manager builds never self-update.
- **About** — a real colophon: version and commit, the licenses Strop
  stands on, and a "check for updates" that shows what it's doing. Opens
  from the palette.
- **Real packages** — `.deb` and `.rpm` with desktop entry, icons and file
  association; a signed, notarized macOS `.dmg`; a Windows installer with
  honest uninstall. Runtime assets (the reading-room fonts, hyphenation
  patterns) ship beside the binary in every one of them.
- **Provenance you can check** — every release asset carries a GitHub
  build attestation (`gh attestation verify`), and macOS builds are
  Developer-ID signed and stapled. Windows binaries are unsigned this
  release — SmartScreen will warn, and the README says so plainly instead
  of pretending otherwise.
- **Backup at open** — before a document is touched, the previous on-disk
  state is preserved; the ledger survives torn writes and crashes
  mid-save. The draft is sacred.
- **The keyboard map's own room** — the shortcut map left its overlay and
  became a window you can keep open beside your work.
- **Footnotes on the book page** — the reading room renders footnote
  definitions by print convention, at the foot of the page they belong to.
- **A clipboard that keeps structure** — what you cut is what you paste:
  headings, lists, code and captions survive the round trip, in and out.

### Changed
- **One identity everywhere** — the app is `cc.pimenov.strop` across
  desktop entries, bundles, installers and the single-instance socket.
- **The dock under law** — the editor's dock parks gracefully at either
  rail and rides the history scrub instead of fighting it.
- **Welcome, brought current** — new sections for pictures, scraps and
  the reading room; the strip explained as the seek bar it is.

### Fixed
- Files opened through portals (Flatpak-style document mounts) resolve to
  their real paths at every door — the file stays where you put it, and
  a moved or renamed draft no longer risks saving to a stale location.
- Wrapped lines in the reading room hold their punctuation at the break.
- Footnote reserves respect their bounds; the two-page spread no longer
  copies the book's convergence behaviour where it shouldn't.
- The keyboard-map window earned correct chrome: client-side decorations
  where the compositor offers none, one quit path, the right display.
- Margin chrome shares one geometry; the active band's bottom edge sits
  exactly on the squiggle it belongs to.

## [0.2.0] — 2026-07-15

The interface release. The writing–editing–checkpointing loop that 0.1 kept in
a sidebar got its real shape: a scrubbable history strip, places for text that
isn't (yet, or anymore) manuscript, a formatting toolbar that answers to the
selection, and a book-typography reading room. Underneath it, the engine
stopped stalling on real, long-lived documents. macOS text rendering is fixed —
this is the first release verified by an outside macOS tester. Binaries are
still unsigned, and Linux (Wayland) remains the primary runtime-tested platform.

### Added
- **The history strip** — the history sidebar grew into a scrubbable strip
  along the top edge of the page: drag it like a seek bar to move through the
  document's whole past, at one fixed time scale, with your absences shown as
  quiet recessed wells rather than gaps to feel guilty about. Restoring an old
  version is one click, and always undoable.
- **Compost and graveyard** — two places for words that aren't manuscript. The
  compost rail (left) is where ideas, clippings and todos live until they're
  promoted into the draft; the graveyard (document footer) automatically
  catches every paragraph you cut, so deletion is never a silent loss — "Put
  back" returns it whole, with its own formatting. Both are plain text:
  editable, selectable, formattable.
- **Selection flanks** — select anything (mouse *or* keyboard) and a small
  formatting grid appears beside it — bold, italic, highlight, headings, link —
  with selection actions on the other flank: add a note, set aside, send to
  the graveyard, ask the editor about this. `ctrl-.` jumps into it from the
  keyboard.
- **The editor button** — one titlebar home for everything AI: request a
  believing, developmental or line read, see what the editor is doing in plain
  words — **Reading** or **Away** — and find results counted honestly
  ("1 read ready", "0 new") instead of appearing as mystery badges.
- **The omnibar** — the top-center control is now a real search field: text
  search, heading jump (`@`), commands (`>`), results hanging live from the
  field's own edge.
- **Margin cards, packed and paced** — your own margin notes and the editor's
  cards now share the right margin, visually distinct (warm vs. cool), packed
  to never overlap, and paced: results arriving mid-sentence wait for a lull
  in your typing before they animate in, and an over-full margin recedes cards
  in place to one line instead of hiding them. Off-screen cards show as
  clickable counts. Honest motion: your own material never fades or slides.
- **Cold read** — a reading room for your finished draft: book typography
  (URW Bookman), justified and properly hyphenated, no caret, pages that flip
  instead of scroll. You read, and file reactions in the margin — the room
  keeps you from fiddling with sentences when the job is judging the whole.
- **Inline images, done right** — pasted or dropped images are furniture now:
  ordinary text editing can't merge prose onto them, clone them, or silently
  delete them; removing one is a deliberate, staged act. The caption is real
  prose on the image's own line, not a hidden field.
- **The editor speaks your language** — an editorial read now detects the
  manuscript's dominant language and writes its margin commentary in it
  (quotes stay byte-exact). The reply pipeline recovers valid cards from
  partially malformed model output, runs one bounded repair round for the
  rest, and reports failures truthfully (refusal, length stop, invalid JSON)
  instead of guessing.

### Changed
- **History is instant.** Versions record their full state when sealed instead
  of being replayed on demand: opening history on a 13-checkpoint document
  went from 71 s to 190 µs, and restore is immediate even on old files.
- **Saves got honest.** A save with no changes writes zero bytes; saving runs
  off the UI thread so typing never stalls behind it; a failed final save
  keeps the editor alive and recoverable instead of losing the last words.
- **One color language.** Warm amber is always you, cool blue is always the
  machine, drained neutral is stale, red is reserved for errors, sage for a
  goal reached — across cards, strip, selection and flashes.
- **Highlight and strikethrough now mark a fixed extent** — typing at their
  edge no longer grows them (bold/italic/underline still extend as you type,
  matching every other editor). If highlights seemed to "follow" your typing
  before, that was a bug, not a feature.
- **Every text field behaves the same** — note composer, rename, session goal,
  checkpoint name, reaction input: real caret and selection, commit on a
  deliberate gesture, and no field silently closes because focus blinked
  (Alt-Tab and keyboard-layout switches no longer eat your half-written note).
- **The margin scrolls on its own** when it overflows, without moving the page.

### Fixed
- **macOS: text was invisible** (#10). The build was missing the `font-kit`
  feature on `gpui_platform`, so macOS shaped text but painted no glyphs —
  caret and selection moved over an empty page. One feature flag; verified by
  the reporting tester on a real Mac.
- **Multi-second stalls on real documents**, three separate causes, all
  variants of treating history as a routine read: the idle-save asset scan
  (6.8 s → 24 ms), history-sidebar open (71 s → 190 µs), and a hidden
  formatting-marks cost that taxed open and save (4.7 s for 5.7 KB of prose —
  gone).
- **A 4.8 MB file for 5.7 KB of prose** — idle saves rewrote every channel
  every time; now guarded by change fingerprints, and bloated files from
  0.1.x compact themselves at open (4.77 MB → 82 KB, cold open 5.8 s → 4.9 ms,
  with a `.pre-compact.bak` safety copy).
- **Letter shortcuts under non-Latin keyboard layouts** (e.g. `Ctrl+Shift+P`
  on Cyrillic) — fixed in the gpui fork, now on all platforms.
- **Glyph corruption after a display-scale change** (moving the window to a
  differently-scaled monitor) — fixed in the gpui fork.
- **Editing could corrupt images**: Delete could merge text onto a picture's
  line, Enter could clone the picture onto both halves of a split, Backspace
  could silently swallow it. All walled off by the furniture model.
- **A margin-note draft could leak onto an AI card**, and the composer showed
  only the first line of a multi-line note.
- **Redo wasn't invalidated on every path that should clear it.**
- **U+2028 in imported Markdown discarded block formatting** — the line
  separator is now modeled as a real hard break; import and export agree on
  what a hard break is.

### Internal
- **gpui fork rebased onto Zed stable v1.10.2** (from a June main snapshot),
  carrying four patches: Windows layout-independent letter keys, per-glyph
  scale context (the Wayland scale fix), image-format trimming, and opt-in
  SVG text rendering — the last lets Strop drop `rustybuzz` entirely
  (RUSTSEC-2026-0206). See `docs/gpui-fork.md`.
- A separate `dist` build profile (thin LTO, stripped), a trimmed dependency
  tree, tightened `cargo-deny`, Dependabot now watching actions and cargo,
  and faster CI on all three platforms.
- The undo engine shares side-state structurally (`Arc` + copy-on-write):
  the 5,000-block stress fixture went from ten million map clones to one
  live allocation. In-session undo depth remains uncapped; only the
  persisted cross-session tail was trimmed.

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

[Unreleased]: https://github.com/kirushik/strop/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/kirushik/strop/releases/tag/v0.3.0
[0.2.0]: https://github.com/kirushik/strop/releases/tag/v0.2.0
[0.1.1]: https://github.com/kirushik/strop/releases/tag/v0.1.1
[0.1.0]: https://github.com/kirushik/strop/releases/tag/v0.1.0
