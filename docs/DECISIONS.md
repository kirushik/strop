# Strop — Architecture Decisions

Decisions made 2026-06-10 with Kirill, backed by four research tracks (GPUI
viability, framework alternatives, text-engine layer, typography). Companion
context: `editorial-foundations.md` (editorial theory, scope, voice-
preservation thesis).

## D1. Shell: Rust + GPUI

The default survived a deliberate attack. GPUI is Apache-2.0 (verified — Zed's
GPL covers only their `editor`/`ui` crates, which we must never vendor), on
crates.io since Oct 2025, validated by Zed 1.0 (Apr 2026) on all three
platforms, with a real third-party ecosystem (`gpui-component`, awesome-gpui).
It is the only Rust framework designed around sub-10ms text editing.

Known costs, accepted with open eyes:

- crates.io release (0.2.2) is ~8 months stale; serious users ride
  `zed@main` as a git dep. We start on 0.2 and migrate to a pinned git rev
  when we hit missing APIs (the `gpui`/`gpui_platform` split is git-only).
- Linux text rendering is GPUI's weakest spot (cosmic-text backend, grayscale
  AA, ignores fontconfig, LoDPI blur issues). **Kirill runs Wayland HiDPI/2x —
  GPUI's best Linux case — so this is acceptable.** Revisit if LoDPI support
  ever matters.
- No accessibility (AccessKit not landed). Accepted for a hobby v1; tracked.
- Forces `smol` async, not tokio.

Runners-up, for the record: iced+cosmic-text (Linux-proven via COSMIC Epoch 1,
but the prose canvas would be fully custom anyway), Masonry+Parley (best text
stack design — HarfRust/AccessKit/IME — but beta-on-beta in 2026; the likely
right answer in 2027–28). Rejected: Zig (no GUI story; Ghostty went
native-per-platform), raw Skia shell (JetBrains Fleet discontinued Dec 2025),
Electron/ProseMirror (multi-frame latency floor), Tauri (WebKitGTK on Linux).

**Hedge: everything below the shell lives in framework-agnostic core crates.
The GPUI layer stays thin enough to rewrite.**

## D2. Document model: rich text internal, Markdown as export

Kirill's call, overriding the styled-source recommendation. The document is a
structured rich-text model that *lives* in Strop's own store; Markdown (and
later other formats) is an export/import boundary, not the source of truth.
This kills the Markdown round-tripping fight and the hide/reveal cursor-
jumpiness bug class, at the cost of building a real rich-text widget on GPUI.
Desktop-only; mobile explicitly out of scope.

## D3. Storage/history: rope hot path + Loro durable layer

The "Datomic for documents" requirement (continuous saves, persistent undo,
checkpoint rollbacks) maps onto **Loro** (loro.dev, 1.0 since Oct 2024): rich
text (Peritext-style spans), time travel to any version frontier, persistent
`UndoManager`, shallow snapshots, stable binary format.

Architecture rule: **Loro is never on the keystroke path.** Hot editing goes
through a ropey-backed buffer with immutable snapshots, stable anchors, and
transaction-grouped undo; committed transactions are mirrored into the Loro
doc, which owns durability, cross-session history, checkpoints, and (later)
sync. `.strop` file = Loro snapshot + incremental updates.

Side effects we get for free: future paid cloud sync = Loro's sync protocol;
the full edit history doubles as the author's own voice corpus (editorial-foundations fork
#4 cold-start mitigation).

Rejected: CRDT-as-buffer (xi-editor postmortem), automerge (heavier, JSON-
oriented), CozoDB (interesting later for the annotations index), plain
files-as-truth.

## D4. AI scope: diagnosis-only first

Margin queries naming problems; zero generated prose in v1. Defers the voice-
preservation problem until the voice-distance metric exists (editorial-foundations §3e,
fork §4.3). Interaction model: on-request, discrete, anchored to ranges —
never ghost text (ownership/homogenization findings, editorial-foundations §3d). BYO LLM
provider.

## D5. Typography: the Birman bar, as actually published

Research finding: the bar is NOT justification/Knuth-Plass/optical margins —
Birman: «выравнивание по ширине нефиг использовать на вебе вообще»; Gorbunov
on hanging punctuation in body text: «выпендрёж». The bar IS:

- **Deterministic, overridable, document-language-aware typographic input.**
  Quote style («»/„“ vs “”/‘’) and dash conventions follow the *document*
  language, never the keyboard layout silently. Em dash always in Russian
  ranges (1941—1945); Word's en-dash insertion is the canonical anti-pattern.
  Nbsp after short prepositions, before the dash. Every substitution
  reversible with a single undo that reverts just the substitution.
- **Vertical rhythm over font choice.** All vertical gaps in multiples of the
  line height.
- **v1 metrics: 20px/28px body, ~64ch measure (~660px), ragged right,
  no justification, no hyphenation.** (Birman's own blog runs 20/28; iA's
  default measure is 64ch; Bringhurst's band is 45–75.)
- **Font: the PT superfamily** (ParaType via Google Fonts, OFL), vendored in
  `assets/fonts`: PT Serif body (4 canonical styles), PT Sans Bold headings
  (the families are metrically harmonized for pairing; PT ships no SemiBold,
  so the sans face carries heading contrast), PT Mono code. *Supersedes
  Literata* (2026-06-11): Literata's variable-font-derived statics spanned
  three family names ("Literata"/"Literata SemiBold"/"Literata 36pt") and
  showed migrating glyph corruption in GPUI's shaping/atlas path on lines
  and windows mixing faces — wrong glyphs (small-cap forms) whose location
  shifted when style runs changed. Document bytes were verified clean, so
  the font stack was the experiment variable; PT faces are independently
  drawn, not instanced. If corruption recurs under PT, the bug is GPUI's
  text system itself → file upstream. Do NOT ship iA's fonts as defaults
  (license-legal but reputationally radioactive in this exact product
  category).

## D6. Text-engine commitments

- Buffer: **ropey** (snapshot property is load-bearing: O(1) Arc-shared
  clones). Anchors + transaction undo designed in from day one — AI edits
  run against a snapshot and rebase over interleaved typing (Zed's agentic-
  edit pattern).
- Latency budget: edit→present inside one 120Hz frame (8.3ms); buffer edit
  <0.5ms; verified externally with Typometer, never internal timestamps.
- Markdown (as export/import): pulldown-cmark. No incremental parsing needed
  at the export boundary.

## D7. The door — drafting vs reviewing (2026-06-14, core-loop research)

The seven-dimension research (DESIGN §6; dossiers in `docs/research/`)
converged on one axis: writing alternates between GENERATE and EVALUATE, and
tools fail by ignoring which mode the writer is in. Decisions taken for
Round 1:

- **A manual draft/review gate, not behavioral inference.** Strop was already
  pull-only (the AI never speaks first), but existing cards still linger in
  the margin during a drafting burst. The door (`ctrl-shift-r`) quiets them.
  Inferring the mode from keystroke bursts is deferred: a wrong card fired
  mid-burst is the one unforgivable error, so ambiguity must default to quiet
  and v1 stays manual. A missed surfacing is cheap; a wrongful interruption
  is not.
- **Default = drafting, except the tutorial.** Every document opens to write
  (protects re-entry — the warm-up re-read that slides into editing). The
  tutorial opens the door because showing the margin is its whole point. Not
  persisted (no stored mode; "settings are apologies"). Revisit if returning
  to a manuscript with resting cards feels like hiding.
- **The door quiets the editor, not the writer.** Only AI cards
  (NoteKind::Diagnosis, both diagnosis and believing) collapse; the writer's
  own `ctrl-m` notes always show. Nothing is lost — the rail's count is
  honest and one click reopens.
- **Altitude suppression, surfaced.** Copy-level cards hide while a
  developmental one is open (dev→line→copy is mandatory), but the held count
  shows in the rail — never silent hiding.
- **Red lines for the rounds ahead** (research-drawn): never a "suggested
  rewrite" field (supplying text is the homogenizing anchor); the future
  editorial agreement may only learn what to *stop* flagging, in the open and
  ratified; no auto-merge of competing drafts.

## Open items

- [ ] Rich-text document schema (blocks + inline spans + annotation layer
      anchored to ranges) — design before the editor widget.
- [ ] When to jump from gpui 0.2.2 (crates.io) to a pinned zed@main git rev.
- [ ] Voice-distance metric harness (editorial-foundations §3e) — the artifact that decides
      whether generation ever ships.
- [ ] Diagnostic engine: one engine with mode rulesets vs three (editorial-foundations §4.2).
- [ ] Williams-vs-Klinkenborg stance setting (editorial-foundations §4.1).
