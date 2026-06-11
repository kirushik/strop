# Strop MVP Roadmap

Agreed mode (2026-06-10): Claude follows this plan autonomously, turn after
turn, until MVP-done; Kirill evaluates in big chunks. MVP = "a working,
possibly rough, demonstration of every necessary feature" — the
diagnosis-first writer's editor thesis, end to end, on Linux.

Judgment calls along the way get made, documented (DECISIONS.md /
commit messages), and flagged in summaries rather than asked about,
unless they'd be expensive to reverse.

## Phase A — Rich text, finished

- [x] A1. Bold rendering (static Literata faces; variable axes unsupported
      by GPUI on Linux), highlight verified, run-construction tests.
- [x] A2. **Durable formatting**: SpanSet persists as Loro Peritext marks
      (rebuilt wholesale at save time — durability lives at the disk
      boundary, which sidesteps expand-rule drift; live mirroring waits
      for multiplayer), loaded on open; formatting joined transaction
      history via per-transaction span snapshots, so undo/redo restore
      spans. Selection composites over content backgrounds (Kirill's
      visibility requirement) via alpha blending.
- [x] A3. **BlockMap**: per-block kinds (paragraph, heading 1–3, quote,
      list item, divider, code block, footnote def) over the same text
      stream; block-aware rendering (sizes/leading on the 28px rhythm,
      quote inset, ⁂ divider, PT Mono code blocks); Enter/Backspace
      semantics at block edges; persistence as newline marks in Loro.
      Typography: research PT-family pairings for headings (PT Sans vs
      PT Serif display cuts vs Literata weights) — Kirill's explicit ask.
- [x] A4. Block commands (soft break U+2028 deferred to B-phase): `#`–`###`-space at line start converts (same
      single-undo contract as the typograph; NO inline `*`/`_` rules —
      decided against), ctrl-alt-1/2/3 headings, quote/list/divider
      toggles, shift-enter soft break (U+2028).

## Phase B — Documents & interchange

- [x] B1. **Markdown export/import**: EXPORT DONE (serializer over the
      full schema — heading/quote/list/code-fence merging/divider/footnote
      defs/images, inline nesting with reopen-on-overlap, escaping,
      `<u>` passthrough, soft-break backslash; ctrl-shift-e writes
      doc.md next to doc.strop). IMPORT DONE: pulldown-cmark walker
      (strikethrough+footnotes options, quote/list/item/code-fence
      line-splitting, image hoisting, <u> html, soft/hard breaks),
      byte-exact roundtrip test; opening a .md imports into a sibling
      .strop (existing .strop wins).
- [x] B2. **Footnotes**: refs are FootnoteRef spans over carrier digits
      (markdown roundtrip: marker replaces carrier), ctrl-alt-f inserts
      ref + def block and lands the cursor in the def; viewport bottom
      zone shows defs whose refs are on screen (overlay inset, height
      cap + internal scroll, click jumps to def). Stale-frame offset
      clamps added across frame-data consumers (fixed a delete-all panic).
- [x] B3. **Images**: §5b pipeline in core (header-only bomb check,
      PNG/WebP byte-identical passthrough, JPEG passthrough only when
      EXIF-free — privacy beats fidelity, decided 2026-06-11; alpha-rule
      conversion, CatmullRom downscale, in-crate orientation baking;
      runs on the background executor — 0.3-0.8s for 12MP). blake3
      content-addressed assets in-file with dedupe. Paste (Wayland
      clipboard images verified readable per GPUI source) and file drop
      (ExternalPaths) insert Image blocks; rendering via Arc<gpui::Image>
      -> use_render_image decode-once cache -> Window::paint_image,
      DPI-crisp and column-capped.
- [x] B4. File UX: ctrl-o open dialog -> new window (one document, one
      process — in-place switching backlogged), ctrl-shift-s "save a copy"
      (.md exports markdown, else full-history .strop snapshot; continuous
      save never re-targets), window title from file stem, window bounds
      remembered across launches (XDG state file). Recent-files dropped
      from scope (the OS file manager + dialog recents cover it).
- [x] B5. **Checkpoints & persistent history** (plumbing done 2026-06-11,
      pulled forward on Kirill's ask — "Google-Docs Rewind, local-first,
      self-contained file" resonates with his interviewees):
      cross-session undo/redo (transaction stacks + span/block snapshots
      persisted atomically with the text; one lifecycle for typing AND
      formatting — Kirill's unification principle), auto "Session start"
      checkpoint on open, ctrl-alt-s named checkpoints, Frontiers-based
      time-travel reads (state_at), restore-as-forward-edit semantics.
      Rough rewind UI shipped: titlebar toggle, checkpoint list with
      dates, click-to-restore (one undoable forward edit). Naming input
      still pending (auto names for now).

## Phase C — The thesis: diagnosis margin

- [x] C1. **Annotation overlay UI, no AI** (research: Liveblocks/Docs
      two-pass solver brief in repo history): Annotations in core —
      non-expanding anchors (orphan-on-delete, Hypothesis-style), unified
      undo (own transactions; snapshots now triple spans/blocks/notes),
      persisted with save + loaded. ctrl-m notes selection (or word at
      caret) and opens a composer (minimal IME-capable NoteInput entity;
      in-card when the margin fits, bottom strip when narrow). Margin lane
      at >= column+264px: Docs-style solver (downward sweep; active card
      snaps to anchor, earlier cards push up in reverse), wheat anchor
      tints compositing under selection, bidirectional activation
      (card click <-> anchor click), Done/Dismiss terminal states leave
      the margin but persist. Old persisted-undo JSON drops once
      (state-snapshot format grew a field).
- [x] C2. **LLM plumbing** (research: provider wire-matrix verified):
      ureq-3 blocking client on the background executor (gpui's bundled
      http_client is trait-only — NullHttpClient by default). Bearer auth
      everywhere; max_completion_tokens except ollama-ish base URLs; NO
      response_format (Poe + Anthropic-compat ignore it) — structured
      output is prompt-and-parse with a lenient fence-stripping array
      extractor; OpenRouter errors-inside-200 handled; error matrix
      (Auth/RateLimited/Provider-verbatim/Shape) unit-tested.
- [x] C3. **Diagnosis run** (ctrl-shift-d): selection-or-document scope
      (24k-char cap) -> diagnosis-first prompt (named problems as queries
      to the author, zero rewrites, Gaiman guardrail, voice-is-never-a-
      defect clause, manuscript language matched, empty array honorable,
      <=7 items; levels developmental/line/copy with line default — config
      mode switch still to wire) -> quotes anchored sequentially against
      the CURRENT text (hallucinated quotes dropped, count reported),
      dismissed diagnoses never re-raised on the same span (tested), the
      whole pass is ONE undoable transaction. Margin: diagnosis cards are
      the quiet species (level chip, named problem semibold, query body;
      no composer), anchors are muted underlines promoting to tint when
      active — never red, never wavy. Titlebar shows running/error state.
      NEEDS a live key test (Poe) — all layers below HTTP are unit-tested.
- [x] C4. Settings file (~/.config/strop/config.toml, malformed = warn +
      defaults): [ai] base_url/api_key/model, language = auto|ru|en
      typograph override, auto_copy_selection (Kirill's habit — selection
      also hits the clipboard), font_size (body, headings scale
      proportionally on a 2px-rounded rhythm). Pulled ahead of C2/C3
      since the client consumes it.

## Phase D — MVP polish gate

- [x] D1. Find (ctrl-f): live sage-tinted matches (compositing with
      everything else), Enter cycles with wraparound + count label,
      Escape returns to the text, seeds from the selection.
      Case-insensitive matching added in the backlog round (char-fold,
      exact for RU/EN).
- [x] D2. Latency pass, by measurement (STROP_PERF=1): release prepaint
      of a 59k-char / 121-block document = 0.4-0.9ms — inside the
      8.3ms/120Hz budget; GPUI's frame-to-frame LineLayoutCache already
      dedupes shaping. No paragraph cache needed at MVP scale. Typometer
      verification remains a nice-to-have (external hardware-ish setup).
- [x] D3. Window niceties: size/position remembered, title shows the
      document name (B4). Confirm-quit-on-failed-save judged not worth a
      blocking quit-time prompt: idle-save failures already warn on every
      heartbeat long before quit; revisit if a real data loss ever occurs.
- [x] D4. Docs sweep: README rewritten to match the shipped feature set;
      ROADMAP is the live record. **MVP gate passed 2026-06-11.**

## Backlog (researched properly, not squeezed in)

- [x] **Asset GC** (2026-06-11): save-time reachability sweep — an asset
  survives if the current blocks, any persisted undo/redo state, or any
  checkpoint still references it; otherwise deleted (tested).
- [x] **Markdown export of assets** (2026-06-11): ctrl-shift-e writes
  `<stem>.assets/<hash>.<ext>` files and rewrites links relative.
- **fast_image_resize**: adopt (SIMD, 10-30x) if import latency on large
  photos annoys; image-crate resampler is scalar.
- **Image UX**: selection/deletion affordances on image blocks, alt/caption
  editing, GNOME screenshot-portal paste quirks. Wayland clipboard image
  WRITE is unimplemented in GPUI (copying an image out won't work).
- **In-place document switching** (vs one-window-per-document) + recent
  files, if multi-doc workflows materialize.
- **Margin v2** (from the C1 research): floating card near anchor on
  narrow windows, gutter dots / clean-mode, collapsed icon rail, snap
  animation (~180ms ease on card top), CriticMarkup export of notes.
  (Diagnosis-card anatomy landed with C3; checkpoint rename-in-place and
  image alt editing landed 2026-06-11.)
- **Visual self-verification** (blocked on a one-time user action): the
  agent cannot screenshot GNOME Wayland headlessly — portal screenshot is
  permission-gated (a dialog may be pending; approving it once grants it),
  or `sudo apt install sway grim` enables a nested headless compositor the
  agent fully controls. Either unblocks autonomous visual audit/polish of
  all UI built to date.

- [x] **History & versions visualization** (researched + rebuilt
  2026-06-11): the research verdict — list beats scrubber (Etherpad's
  own maintainers call theirs broken), never show writers a DAG, inline
  diff in reading order, restore-as-forward-edit validated. Shipped:
  history mode (↺ or ctrl-alt-h) takes the editor read-only with an
  inline word-level diff (similar crate, paragraph->word cascade,
  whole-paragraph fallback below 0.4 word-ratio; sage inserts,
  struck-dimmed deletes); Docs-style panel with day groups, word deltas,
  named-only and vs-prev/vs-draft toggles; arrow-key version stepping
  with live preview; Enter or Restore button restores (auto "Before
  restoring" checkpoint, still undoable); idle-gap (15min) session
  sealing replaces interval checkpoints. Deliberately skipped per
  research: scrubber, tree view, side-by-side, selective restore,
  per-author colors. REMAINING (needs hands-on feedback first):
  rename-in-place, "since last session" affordance, snap animation.

## Done in the final loop round (2026-06-11)

- [x] **Voice baseline corpus**: `[voice] corpus = ["~/essays/*.md"]`
  globs (.md via import, .strop via store read, .txt raw; >=200 words
  each, >=3 files) -> per-feature mu/sigma + leave-one-out calibration of
  normal self-variation; the history panel shows "Voice: within / Nsigma
  outside your normal range (M texts)" plus up to 5 nameable per-feature
  flags ("частота тире: +3.1sigma от вашей нормы"). Below 3 docs the v0
  descriptive mode stands. Verified live with a generated corpus.

## Explicitly post-MVP

Voice-distance metric — v0 stylometric proxy DONE 2026-06-11
(strop-core::voice: fixed RU/EN function-word frequencies, sentence-rhythm
CV + Goh-Barabasi burstiness with top billing per research, punctuation
per-1000w rates, MATTR-100; descriptive drift between any checkpoint and
the draft shown in the history panel's vs-draft mode; honest Eder caveat —
drift indicators, never identity; surprisal-based real metric still
post-MVP, baseline-corpus z-flagging awaits multiple documents).
Believing-mode DONE 2026-06-11 (ctrl-shift-b: Elbow's believing game as a
pass — 2-3 named working moves with mechanisms, center of gravity, the
comes-alive sentence, almost-said; quotes mandatory, praise-adjectives and
advice verbs banned, scarcity rule; per research NO shipping tool does
this). Any text generation, sync/multiplayer, mac/Windows,
~~find-replace~~ (done
2026-06-11: ctrl-h adds a replace field to the find bar, Tab hops fields,
Enter replaces-and-advances, All replaces every match), tables (never?),
per-paragraph AI rewrites (thesis says diagnosis only).

## Screenshot-driven fixes round 2 (2026-06-11)

- [x] **Overlay z-order**: history panel (and every other overlay) painted
  UNDER the document text — GPUI paints siblings in tree order; overlays
  now mount after the canvas child. Caught by Kirill from a screenshot;
  invisible to the state-reading smoke harness by construction.
- [x] **Font stack → PT superfamily** (PT Serif body / PT Sans Bold
  headings / PT Mono code), replacing Literata. Motive: migrating glyph
  corruption (small-cap-looking forms whose location shifted when style
  runs changed). Document file audited clean first — one span, exactly
  "Lish", char-indexed end to end (`strop-core --example dump` is the new
  audit tool). Literata's three-family static set was the prime suspect;
  PT faces are independently drawn. **If corruption recurs under PT, it's
  GPUI's shaping/atlas — file upstream with the dump as evidence.**

## Text-pipeline surgery (2026-06-11, после PT-swap screenshots)

Corruption SURVIVED the font swap -> the bug was GPUI's text pipeline,
proven and patched:

- [x] **shape_audit harness** (`strop-app --example shape_audit`):
  headless-ish window shapes the exact corrupted lines + decoy battery in
  two process orders, compares every glyph id against the font's own cmap
  (ttf-parser over the same embedded bytes). Verdict: isolated shaping was
  deterministic and cmap-exact -> bug above/below shaping, not in it.
- [x] **Vendored gpui 0.2.2** (last crates.io release ever; zed stopped
  publishing) into `vendor/gpui` + backported upstream fixes — see
  vendor/gpui/STROP-PATCHES.md: PR #41224 (TextLayout re-measure mangles
  truncated runs -> the history-panel jitter/garble), PR #43856
  (layout_line drops font changes between runs when decorations match),
  PR #48504 adapted (cosmic-text 0.14->0.17.2: style-mismatched fallback
  picked wrong fonts; ASCII fast path emitted wrong glyphs for words with
  incompatible spans -> the canvas corruption class).
- [x] **De-fallback the chrome**: PT has no ○●↑↓↺□✕✓→; those were forcing
  mid-session system-font loads (the trigger surface). History markers,
  zoom button, history toggle are now drawn divs; ✕->×, →->›, ↑/↓->words.
  Voice strings keep σ (no honest replacement; fallback now safe-ish).
- [ ] **Visual confirmation needed (Kirill)**: rebuild, look at the same
  two paragraphs + history panel. If small-caps-style corruption persists,
  next suspects in order: raster path (backport #45423+#46857), AMD/radv
  (test `ZED_PATH_SAMPLE_COUNT=0`), then a zed git pin.
- [ ] Autonomous screenshots remain blocked: GNOME portal denies headless
  capture; `sudo apt install sway grim` would let me run Strop in a nested
  headless compositor and look at pixels myself.
- [ ] Exit strategy: git pin on zed-industries/zed (post gpui/gpui_platform
  split) to retire the vendor patches; #54878 fallback wiring comes free.

## History preview formatting (2026-06-11, Kirill's report)

- [x] **Styling no longer disappears in history mode**: checkpoints carry
  their SpanSet+BlockMap (state_at already computed them; they were being
  discarded), and rebuild_preview projects BOTH diff sides' formatting onto
  the merged view — kept/inserted text gets the newer version's spans and
  block kinds, deleted text the older version's. Built on new
  diff::prose_diff_blocks (per-block provenance: which old/new paragraph a
  merged line renders; segments reconstruct each source byte-exact — the
  flat prose_diff's separator newlines are synthetic, so the flat form
  can't be cursor-walked). Smoke: pspans/pkinds in debug_cursor prove a
  bold span + H3 survive into the preview of a real store's checkpoint.
- [ ] Formatting-only sessions never seal a checkpoint
  (add_checkpoint_if_changed compares text only) — bold/heading work
  without text edits is unreachable by rewind. Compare (text, spans,
  blocks) instead?
