# Phase E — The operational shell (road to dogfooding)

> Compiled 2026-06-12 after Kirill's course correction: "we have a passable
> text editor — and that's all, at least on the UX level. How do I run my
> AI editors? How do I export to Markdown? How do I locate my current
> .strop file?" Those questions are symptoms of one class: **every
> capability was built engine-first and left without an operational
> surface.** This plan fixes the class, not the symptoms.
> `docs/ROADMAP.md` remains the historical log of phases A–D.

## E0. Gap analysis (the audit, 2026-06-12)

Cross-checked against `ai-writers-editor-handoff.md` and docs/.

**Invisible capabilities** — exist, work, untestable by a user who hasn't
read the source:

| Capability | Today's only surface |
|---|---|
| Editorial diagnosis (THE thesis) | ctrl-shift-d, undocumented in-app |
| Believing pass (Elbow) | ctrl-shift-b, same |
| Levels-of-edit modes (THE differentiator per handoff §2.2) | **hardcoded `"line"`** — developmental/copy prompts exist in core, no switch anywhere |
| Markdown export | ctrl-shift-e, silent success |
| Open file | ctrl-o (gpui prompt), no menu/palette |
| Save a copy | ctrl-shift-s |
| New document | **does not exist** |
| Current file location | **not shown anywhere**; default doc hides in ~/.local/share/strop/scratch.strop |
| Recent files | **does not exist** |
| Named checkpoint | ctrl-alt-s |
| Author note | ctrl-m |
| Footnote | ctrl-alt-f |
| Find / Replace | ctrl-f / ctrl-h |
| History / rewind | ↺ button (unlabeled ring glyph) or ctrl-alt-h |
| Headings, lists, quotes, code blocks | ctrl-alt-1..3, ctrl-shift-7/8, ctrl-alt-q/c |
| AI provider setup | hand-edit ~/.config/strop/config.toml; no validation, no in-app surface |
| AI run status / errors | `last_ai_error` string, barely rendered; no in-progress state |
| Voice baseline corpus | config-file globs only |

**Thesis-surface gaps** (handoff cross-check):
- §2.2 levels-of-edit → modes: core-complete, UX-absent (the one thing the
  handoff calls a likely open gap in the field — currently invisible).
- §4.1 Williams-vs-Klinkenborg stance: unsurfaced (acceptable post-MVP,
  but the mode picker must leave room for a stance setting).
- §3e accept-friction: diagnosis cards already require explicit
  dismissal — good; never weaken this while smoothing UX.
- First-run experience teaches nothing about diagnosis-first editing; the
  sample document is filler prose, not an introduction.

**Verdict**: the engine outran the product. No new engine work until the
shell catches up.

## Design principles for the shell

1. **The palette is the menu.** Chrome stays minimal; one searchable
   surface lists every action with its binding. No menu bar, no toolbar
   growth (the MS-Visual-Studio™ fear stays policy).
2. **Nothing requires the config file.** TOML remains the storage, but
   every setting a writer needs (AI key/model, mode, language, font size)
   gets an in-app surface.
3. **The AI must explain itself**: unconfigured → teaches setup; running →
   visible; failed → names the cause; empty result → says so honorably.
4. **Files are not a mystery.** The document's name and place are always
   one glance away; new/open/recent are first-class.
5. **Teach by document.** First run opens a tutorial .strop that
   demonstrates marks, history, and a pre-seeded diagnosis — the document
   IS the onboarding.

## Stages

Each stage: research applied → implement → smoke/tests → commit → mark
here. Sequenced so dogfooding unblocks as early as possible.

- [x] **E1. Command palette + action registry** (shipped 2026-06-12):
  crates/strop-app/src/commands.rs is the single source of truth — one
  table (label, aliases incl. Russian, section, chord, action) drives the
  keymap (KeyBinding::load) and the palette, so chords can never drift
  from what the UI claims. ctrl-shift-p / F10 / drawn-hamburger titlebar
  button; empty query lists all commands grouped by section (the palette
  IS the menu); fuzzy subsequence + substring scoring with word-boundary
  bonuses (unit-tested, RU aliases verified); Enter/click dispatches the
  real action with focus returned to the document first; up/down navigate
  via a PaletteInput key context. Polish later: scroll-selected-into-view,
  recents-on-top.
- [x] **E2. Document lifecycle** (shipped 2026-06-12): visible-from-birth
  files — bare launch reopens the most recent document, first launch ever
  migrates the old hidden scratch into `$XDG_DOCUMENTS_DIR/Strop/` (xdg
  user-dirs parsed, localized) or starts Untitled.strop there; `--new` /
  ctrl-n / palette "New Document" opens a fresh Untitled in its own
  window (one window per document, one process per window); titlebar
  shows the document name — click or F2 renames in place and renames the
  file on disk (collision-refusing); Reveal in Files (FileManager1 D-Bus,
  xdg-open fallback) and Copy Document Path in the palette; app-private
  recents (~/.local/state/strop/recents.json, deduped, existing-only)
  appear as palette rows that open in new windows;
  scripts/install-desktop.sh registers application/x-strop + .desktop so
  double-click works. Deviations from research, deliberate: no start
  screen (recents-in-palette + reopen-last covers it with zero new UI;
  revisit after dogfood), recently-used.xbel deferred, single-instance
  raise-if-open deferred. New documents seed empty; the first-ever
  document keeps the demo text until E4's tutorial replaces it.
- [x] **E3. AI surface** (shipped 2026-06-12, per E3-research): guided
  config-file flow, no settings panel — "Set Up AI Provider…" writes a
  commented template (Poe/OpenRouter/Ollama examples, STROP_API_KEY env
  precedence documented and implemented) and opens it via xdg-open;
  every pass re-reads config.toml, so edit→save→retry needs no restart.
  AiStatus state machine rendered where results land (margin lane top,
  floating card on narrow windows): NeedsSetup teaching card with the
  privacy line + Open config + Test connection; Running card with
  UI-level Cancel (generation counter drops stale responses); success
  Note that names kept/dropped counts (0-anchored is said out loud) and
  fades; Error cards with named causes (key rejected / rate limited /
  unreachable / unusable reply / not-diagnosis-format) + Open config /
  Retry (repeats the same pass kind) / Dismiss. "Test AI Connection" =
  1-token chat that moves 401s to setup time; on provider errors it
  GETs /models and lists the first 8 ids — that IS the model picker.
  **Levels-of-edit mode switch shipped** (the thesis surface, handoff
  §2.2): Diagnosis Mode Developmental/Line/Copy palette commands +
  [ai].mode config default + idle margin hint showing the current mode;
  debug_cursor reports ai=/mode= for smoke. Deferred: ticking elapsed
  display, margin-header mode chips (post-dogfood).
- [x] **E4. First-run tutorial + keyboard map** (shipped 2026-06-12):
  first launch (and palette "Open Welcome Guide" / `--welcome`) seeds
  "Welcome to Strop.strop" — built at runtime from markdown, real
  formatting, literal chords inline, three pre-seeded margin cards (two
  diagnoses on planted flaws incl. an agreement slip the copy mode would
  catch, one author note) and a "Fresh tutorial" checkpoint so rewind has
  something to show; the demo SAMPLE text is retired. ctrl-? (GNOME
  convention) opens the keyboard map: every registry command grouped by
  section plus the text-editing baseline, esc/click-out closes —
  rendered from the same table as the palette and keymap, so it can
  never lie.
- [ ] **E5. Dogfood gate**: Kirill writes something real; every friction
  point becomes a tracked item; first live-key diagnosis run on real
  prose. Exit criterion: he reaches for Strop instead of his current
  editor for one full piece.

## Research debts feeding this plan

- E1-research: palette binding conventions vs GTK/GNOME + our keymap
  (ctrl-k conflict: GTK delete-to-EOL vs web palette convention).
- E2-research: GNOME Text Editor / TextEdit / iA Writer lifecycle
  patterns; freedesktop recents; default-folder conventions.
- E3-research: BYO-key setup flows (Zed/Obsidian-plugins/TypingMind);
  status/error surfacing patterns; model-field UX for Poe/OpenRouter/
  Ollama.
- Each lands as a section appended below when the agent reports.

## E1-research (landed 2026-06-12)

Verdicts (full agent report in session log): palette on **ctrl-shift-p**
(cross-editor standard, zero GNOME/GTK/keymap collisions; ctrl-k stays
reserved for insert-link — the writing-app convention, Google Docs/Bear);
**ctrl-? shortcuts overlay** (GNOME-native: libadwaita apps all answer
ctrl-?; Linear built one even with a world-class palette — palette is for
doing, overlay for learning); **one titlebar menu button** (F10, GNOME
primary-menu placement) opening the palette — the day-zero affordance;
**empty-query palette state lists every action grouped by section** (the
palette IS the menu bar; Obsidian/VS Code convention); verb-first labels
with aliases (Superhuman pattern: "Export as Markdown" + alias "save as",
RU aliases for the bilingual user); binding rendered right-aligned on
every row; fuzzy subsequence matching; **slash-commands rejected** ('/'
is legitimate prose; Lex itself chose cursor-anchored palette over '/');
tutorial document validated (Typora Quick Start.md pattern: 1-2 screens,
in-format, literal keybindings inline, reopenable from the palette).

## E2-research (landed 2026-06-12)

Verdicts: **explicit-file model with visible-from-birth autosave** — the
GNOME Text Editor hidden-drafts pattern is the documented anti-pattern
(Ctrl.blog) and caused exactly Kirill's complaint; Apple HIG adopted
(autosave content, file stays a user-manageable object). Untitled docs
materialize immediately as `$XDG_DOCUMENTS_DIR/Strop/Untitled N.strop`
(xdg user-dirs, localized — use the dirs crate); titlebar filename click
→ popover with full path / Copy path / Reveal in Files
(org.freedesktop.FileManager1.ShowItems D-Bus, xdg-open fallback) /
rename field; silent scratch.strop migrated visibly; app-private recents
(~/.local/state/strop/recents.json, ~8 shown) in palette (+ xbel later);
auto-title from first heading only as an explicit offer, never silent;
one window per document (ghostwriter/Apostrophe convention), palette
in-window switching later; .desktop + MIME (application/x-strop)
registration script now, single-instance raise-if-open later.

## E3-research (landed 2026-06-12)

Verdicts: **no settings panel in v1** — a GPUI settings UI fights the
thesis; Continue.dev proves guided-config-file is credible when the app
(1) writes a commented template on demand (filled [ai] examples: Poe
api.poe.com/v1, OpenRouter, Ollama localhost:11434/v1), (2) opens the
config in Strop itself, (3) reloads config on window focus so
edit→alt-tab→retry needs no restart. STROP_API_KEY env precedence (Zed
pattern) answers plaintext-on-disk. **The empty margin teaches** (Zed's
empty-Agent-Panel move): unconfigured ctrl-shift-d → margin card "needs a
model" with privacy line ("your text goes directly to the endpoint you
configure, only when you run a pass") + Create-config + Test-connection
actions; muted hint line in empty margin so the AI is visible BEFORE the
chord is known; a titlebar diagnosis button (buttons teach chords).
**Test connection** = 1-token chat call, moves 401s to setup-time
(BoltAI/Cursor Verify pattern); on 404 fetch {base}/models and list ids —
that IS the model picker, no dropdown. **Running state lives where
results will land**: pinned margin card "Diagnosing… {model} · {N}s" with
UI-level cancel (generation counter, ignore stale responses); success
card "{kept} queries anchored" (0-anchored must be said — silent success
is the second invisibility bug). **Error taxonomy** on the same card:
401→key rejected [Open config]; 404→model not found [List models];
429→rate limited [Retry]; network→can't reach host [Retry]; parse→model
too small [Retry]; raw error as expandable detail, never alone.

## Phase F — the designed shell (from docs/DESIGN.md, 2026-06-13)

Execution of the design architecture; each stage cites its DESIGN.md
section. Ordering: root-fix verification first, then highest-leverage.

- [x] **F0. gpui migration landed** (2026-06-13): pinned to zed tag
  v0.233.10 (gpui facade + gpui_platform; wgpu renderer, current
  cosmic-text, per-span fallback wiring — every fix class the vendored
  0.2.2 hotpatched around). 22 mechanical API changes; smoke output was
  byte-identical to the old binary down to glyph x-positions; shape
  audit clean; vendor/gpui deleted. First launch pays ~20s wgpu shader
  warm-up once; ExternalPaths clipboard paste deliberately ignored.
  REMAINING VERIFICATION: Kirill's eyes on the corrupted screens
  (footnote line, headings) — raster output can't be asserted headless.
- [x] **F1. Titlebar diet + selection popover** (shipped 2026-06-11,
  DESIGN §2-toolbar): persistent B I U S H {} stripped from the titlebar
  (zero category precedent across the seven surveyed minimal editors);
  final bar = title · live word count (cached usize, recomputed in
  sync_mutations — never per frame) · history ring · hamburger · window
  controls. Selection popover: in-surface GPUI overlay (never an
  xdg_popup), B I S {} + H1 H2, shown on mouse-up over a non-empty
  selection, summonable via ctrl-. ("Format Selection…" in the registry,
  the ARIA-toolbar keyboard path), anchored above the selection start
  (below when the titlebar is in the way; clamped to window edges),
  dismissed by mousedown/typing/scroll/escape, mounted after the canvas
  and before the palette in the overlay chain. ctrl-1..3 promoted in the
  registry (palette + keyboard map show them); ctrl-alt-1..3 stay as
  silent literal aliases in bind_keys. Rig note: shot.sh now captures
  twice — unfocused XWayland surfaces present stale frames on the first
  grab.
- [x] **F2. History side panel** (shipped 2026-06-12, DESIGN §2-history):
  the dropdown is dead. Full-height right panel, PUSH not overlay — the
  center row gets pr(panel_w) so the column re-centers and re-wraps; the
  panel shrinks before the document does (180..320px, prose keeps
  ~400px), margin lane + AI card + footnote zone stand down while open.
  Slim mode banner in the column's 56px top padding (never over prose):
  "Viewing: name · date · [Restore] · Esc exits" — Restore lives there
  now, once. Two-tier list: named checkpoints first-class (bold, filled
  dot), runs of autos collapse into "N auto-checkpoints show/hide" rows;
  arrow-stepping into a collapsed run unfolds it (selection is never
  hidden); day headers, rename-in-place, word deltas, named-only filter
  all kept. Per-row voice-drift scalar ("+2.1σ", capped at ">10σ") when
  a baseline exists and assess() flags, gated on the 200-word corpus
  floor. vs-prev/vs-draft is a segmented control pinned at the panel
  bottom (Docs' "Show changes" position); the vs-draft voice block sits
  above it. Bug fixed en route: Baseline::assess with a doc-detected
  language different from the corpus language indexed out of bounds
  (per-language function-word vectors differ in size) — Baseline now
  exposes lang() and all assess() signatures use it.
- [x] **F3. Footnote completion** (shipped 2026-06-12, DESIGN
  §2-footnotes): the in-text ref is a painted superior figure — the
  carrier digit keeps its advance (caret, hit-testing, selection) but
  inks transparent; paint() draws the number at 65% of the block size,
  baseline raised 35% of the font size, accent ink, pill dropped (size
  signals "footnote", color "interactive"; PT ships no sups, so we
  paint our own with the list-marker machinery). Painted numbers
  derive from ref order in the text — stored ids stay stable internal
  labels (the Pandoc architecture); orphan defs take the following
  numbers; insert mints max(label)+1 so a label is never reused after
  a deletion. Bidirectional jumps: click a ref → caret at its def's
  text start; click the def's "N." gutter or a zone row's marker →
  caret just after the ref (geometry hit-test on the span's glyph band
  — a caret placed *beside* the ref must not teleport). The zone is an
  edit surface now: clicking row text lands the caret at the matching
  offset in the def line (Word notes-pane niche; row bounds captured
  by a canvas child, click x re-shaped against the row's own text).
  Stacking policy: >3 visible refs → the 3 nearest the viewport center
  + a muted "+N more" row. Rig: the smoke harness gained `click:X,Y`
  (synthetic clicks through the full GPUI dispatch path, div listeners
  included) and `fn-geo` (window-space click targets for refs, def
  gutters, zone rows).
- [x] **F4. AI settings panel** (shipped 2026-06-12, DESIGN §2-ai —
  Kirill's mandate: provider setup is the core onboarding task): a
  centered in-surface overlay (keyboard-map pattern: backdrop, 520px
  panel, esc/click-out closes, mounted last) on OpenAiSettings — the
  registry's "Set Up AI Provider…" and every AI-card "Open config"
  button now point here; "Edit config file…" inside the panel keeps the
  old xdg-open flow. Three NoteInput fields under a "SettingsInput" key
  context (tab cycles via note_tab, enter commits, ctrl-enter saves,
  escape closes): Base URL with the three example endpoints as helper
  text, API key MASKED (dots except last 4 — display only, paste/IME
  untouched), Model as free text that live-filters the picker. [Test] =
  the 1-token chat against the *typed* values, inline on the panel
  ("testing…" → "OK · Nms" green / warm-red error), auto-fetching
  /models on success; [List models] → background list_models() →
  scrollable list, up/down+enter or click picks. Save writes through
  toml_edit (workspace dep): [ai] values set in place, comments and
  unknown keys survive (unit-tested against an injected temp path —
  env vars are process-global and lifecycle_in_isolated_home already
  owns them), config reloaded, fading "AI configured: model via host"
  note; with STROP_API_KEY set the key field says it's ignored and is
  never written. Verified live against a local python stub (/models +
  /chat/completions) and the connection-refused error path;
  debug_cursor gained ai_panel=/ai_models= tags.
- [ ] **F5. Structure & finishing layer** (DESIGN §1.6, §4): the beat
  strip (outline at point of performance) + close-time "Next session I
  will ___" ritual + session word-progress. The research's verdict:
  this, not chrome, is what stands between the writer and "The End".
- [ ] **F6. Explorability passes** (DESIGN §3): tutorial invites
  breaking things; solution-reveal whispers (1/session); palette
  hit-frequency ordering.
