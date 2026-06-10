# Strop Document Model

Scope agreed 2026-06-10: roughly default Markdown, plus strikethrough,
underline, footnotes; images as standalone blocks (one per paragraph,
limited types and sizes). This doc fixes the full inventory, the storage
mapping, and the edit semantics — the parts that are cheap to argue about
now and expensive to change later.

## 1. Inline spans

| Span | Markdown | Typing expansion* |
|---|---|---|
| Emphasis (italic) | `*…*` | expands |
| Strong | `**…**` | expands |
| Strikethrough | `~~…~~` | expands |
| Underline | `<u>…</u>` on export (no MD syntax; deliberate) | expands |
| Highlight | `==…==` (Obsidian-compatible) | expands |
| Inline code | `` `…` `` | does **not** expand |
| Link `{href}` | `[…](url)` | does **not** expand |
| Footnote ref `{id}` | `[^id]` | zero-width atom |

\* *Expansion* = typing at the span's right edge continues the style
(Peritext semantics). Styles expand; code and links don't — typing after a
link must not grow the link. This matches Loro's mark expand configuration,
so the hot path and the durable layer agree by construction.

**Highlight is my addition** — it's an *authoring* tool (mark-for-later
during drafting), it's cheap, and it pairs naturally with the future
diagnosis layer without being part of it. Cut it if it feels like creep.

Also inline: **soft line break** (Shift+Enter) — required for verse,
addresses, and quoted poetry in essays. The block separator stays `\n`; a
soft break is U+2028 LINE SEPARATOR inside the block (already whitespace to
word motion and the shaper splits on it at render time). Exports as
Markdown's trailing-backslash break.

## 2. Blocks

| Block | Markdown | Notes |
|---|---|---|
| Paragraph | bare text | default |
| Heading {level 1–6} | `#`–`######` | UI exposes 1–3; 4–6 parse/round-trip only |
| Blockquote | `>` | essential for op-ed/criticism; one nesting level |
| List item {bullet/ordered, depth 0–1} | `-` / `1.` | prose lists are flat; two levels max, deliberately |
| Divider / scene break | `***` | first-class for fiction (⁂-class rendering later), not just a rule |
| Image {src, alt, caption} | `![alt](src)` | standalone block only; assets stored in-file (§4); PNG/JPEG/WebP, size-capped |
| Footnote definition {id} | `[^id]: …` | lives in the text stream as a block; UI may render as popover/margin later |

## 3. Explicitly rejected (the "what else" answer, negative half)

- **Tables** — not prose; a formatting tar pit (cursor logic, wrapping,
  export). If a real need appears, revisit as an embedded object, not text.
- **Per-span color/size/font** — word-processor territory and against the
  thesis: voice lives in words, not decoration. Also what keeps the
  document model small enough to diagnose.
- **Task lists, embeds/iframes, Mermaid** — not prose.
- **Math** — not the audience (op-eds/essays/fiction). Reserve an inline
  atom kind in the enum so adding it later isn't a schema break.
- **Heading levels 4–6 in UI** — an essay with H4s has a structure problem
  no menu should encourage. Parsed and preserved, not offered.

## 4. The annotation layer (reserved now, built later)

The product thesis lives here, so the schema reserves it from day one even
though v1 ships none of it:

- **Annotation** = { id, anchor: stable range, kind, body, status }.
  Kinds: `diagnosis` (AI margin query), `note` (author's note-to-self),
  later `checkpoint-comment`.
- Annotations are an **overlay**, never part of the text stream — they
  must survive Markdown export (dropped) and arbitrary edits (anchors).
- Anchoring uses **Loro Cursor** stable positions (they survive edits and
  even time travel), mirrored on the hot path by the same span-adjustment
  math as formatting (`SpanSet::apply_op`).

## 4b. Document metadata

`LoroMap("meta")`: `lang` (explicit typograph-language override; absent =
auto-detect), `title` (absent = first heading, else first line), future
stance settings (Williams-vs-Klinkenborg, handoff §4.1). Deliberately tiny.

## 5. Storage mapping

Hot path (strop-core, framework-free):
- `Rope` — plain text; `\n` separates blocks (as today).
- `SpanSet` — inline spans as char ranges + attr, adjusted on every
  `TextOp` by Peritext expansion rules. This is the anchor machinery
  D6 promised; annotations reuse it.
- `BlockMap` — per-block kind + params, keyed by block index, repaired
  incrementally on edits (a block split/join moves kinds with the text).

Durable layer (Loro):
- One `LoroText("content")` — the same text stream.
- Inline spans = **Loro Peritext marks** with matching expand config.
- Block attrs = marks on the block's trailing `\n` (the Quill/Loro
  rich-text convention).
- `LoroMap("assets")` — image bytes keyed by content hash; image blocks
  reference the hash. Caps enforced at insertion (type allowlist, size cap,
  configurable, default ~8 MB).
- `LoroList("annotations")` — the overlay, with Loro Cursors as anchors.

Mirroring stays op-based as today; formatting changes add mark/unmark ops
alongside TextOps.

## 6. Markdown boundary

Import and export are total functions over this schema (that's the point
of capping it): every block/span row above lists its mapping. Underline
exports as `<u>` (documented HTML passthrough); soft breaks as
trailing-backslash; footnotes as `[^id]`/`[^id]: …` pairs; images export
alongside an assets directory (`doc.assets/<hash>.png`) with relative
links. Anything Markdown can express that this schema can't (tables, raw
HTML beyond `<u>`) imports as literal text — visibly, not silently.

## 7. Migration

Current `.strop` files are plain LoroText — already forward-compatible:
the new model reads them as all-paragraph documents with empty SpanSet.
No format break.

## Open questions for Kirill

1. Highlight (`==…==`): in or out?
2. Links: autolink pasted URLs, or only explicit link creation?
3. Image size cap and whether assets belong in-file (single portable
   `.strop`) vs sidecar directory (smaller files, fragile moves). I lean
   **in-file** — portability is worth megabytes, and Loro handles binary.
4. Footnote UX direction (inline defs at doc end vs margin popovers) —
   affects nothing in the schema, everything in the editor; can wait.
