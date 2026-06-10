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

- [ ] B1. **Markdown export/import** (pulldown-cmark in, serializer out)
      per the document-model mapping table; ctrl-shift-e export, opening
      a `.md` imports. Underline as `<u>`, footnotes `[^id]`, soft break
      as trailing backslash.
- [ ] B2. **Footnotes**: insert/ref/def plumbing + the viewport bottom
      zone (global numbering, overlay inset, ~1/3 height cap, read-only
      projection, click-to-jump).
- [ ] B3. **Images**: paste/drop import pipeline per §5b policy (types,
      2400px downscale, EXIF strip, 8MB refusal), assets in Loro, block
      rendering via gpui img.
- [ ] B4. File UX: ctrl-o open (.strop/.md via xdg portal dialog), recent
      files on the bar?, save-as. Keep minimal.
- [~] B5. **Checkpoints & persistent history** (plumbing done 2026-06-11,
      pulled forward on Kirill's ask — "Google-Docs Rewind, local-first,
      self-contained file" resonates with his interviewees):
      cross-session undo/redo (transaction stacks + span/block snapshots
      persisted atomically with the text; one lifecycle for typing AND
      formatting — Kirill's unification principle), auto "Session start"
      checkpoint on open, ctrl-alt-s named checkpoints, Frontiers-based
      time-travel reads (state_at), restore-as-forward-edit semantics.
      REMAINING: the rewind UI (checkpoint list, preview, restore button)
      and an in-app naming input.

## Phase C — The thesis: diagnosis margin

- [ ] C1. **Annotation overlay UI first, no AI**: author notes-to-self
      (ctrl-m) anchored to ranges, rendered in the right margin, surviving
      edits (SpanSet math + Loro cursors), resolve/delete. Proves the
      margin interaction.
- [ ] C2. **LLM plumbing**: BYO-key config (~/.config/strop/config.toml)
      as an **OpenAI-compatible chat-completions client with configurable
      base_url/key/model** — one client covers Poe (Kirill's subscription,
      explicitly requested), OpenAI, OpenRouter, ollama/llama.cpp, and
      Anthropic's compat endpoint. Background thread + channel into GPUI's
      executor (mind the smol-vs-tokio trap), non-streaming first.
- [ ] C3. **Diagnosis run**: document/selection scope -> diagnosis-first
      prompt (named problems as queries, zero rewrites, Gaiman guardrail;
      levels-of-edit as a mode switch: developmental/line/copy) ->
      anchored margin annotations with dismiss/done. The handoff doc's
      §2 conclusions become running code here.
- [ ] C4. Settings file: provider/model/key, document language override,
      auto-copy-selection-to-clipboard (Kirill's habit), font size.

## Phase D — MVP polish gate

- [ ] D1. Find (ctrl-f), minimal: highlight matches, n/N navigation.
- [ ] D2. Latency sanity pass: profile full-document reshape-per-frame,
      fix the obvious (cache shaped paragraphs across frames keyed by
      content+width), verify with Typometer if feasible.
- [ ] D3. Window niceties: remember size/position, confirm-quit only if
      save failed. Title shows document name.
- [ ] D4. Docs sweep: README quickstart, DECISIONS/document-model updated
      to match reality.

## Explicitly post-MVP

Voice-distance metric (the generation-gate experiment), any text
generation, believing-mode, sync/multiplayer, mac/Windows, find-replace,
tables (never?), per-paragraph AI rewrites (thesis says diagnosis only).
