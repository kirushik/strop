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

Decisions 2026-06-10: **highlight is in**. **No autolinking** — links are
created explicitly (TLD heuristics are exactly the kind of guessing the
typograph forswears); margin comments may adopt different conventions when
they exist.

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
| Image {src, alt, caption} | `![alt](src)` | standalone block only; assets stored in-file (§5b) |
| Code block {info} | ``` fenced ``` | monospace, **no syntax highlighting** — by design; doubles as the ASCII-pseudotable escape hatch. The fence info string (` ```rust `) is stored for round-trip fidelity, never acted on |
| Footnote definition {id} | `[^id]: …` | lives in the text stream as a block; rendered in the viewport footnote zone (§4c) |

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

## 4a. Multiplayer-proofing the annotation overlay

Verified against the "human editor leaves Google-Docs-style margin
comments" future:

- **Convergence**: annotations live in Loro containers; concurrent edits
  from multiple replicas converge by CRDT construction, no extra design.
- **Anchors**: Loro `Cursor` binds to character *identity* (OpID), not
  offset — it survives concurrent edits, resolves identically on every
  replica after sync, and degrades to the nearest surviving position when
  the anchored text is deleted. This is precisely the primitive built for
  this use case.
- **Schema reserved now** (cheap to declare, shapes thinking): annotation =
  { id, anchor, kind, **author {id, name}**, **parent_id** (threads/replies),
  **created/modified**, status {open, resolved}, body }. **Body is a
  LoroText**, not a string — editors write formatted comments with links.
- **Suggested edits** (Docs' "suggesting" mode) fit as kind `suggestion`
  with payload {anchored range, proposed text}; accepting one is an
  ordinary transaction. No schema break.

Out of scope until needed: identity/auth, permissions, transport — all
orthogonal to the document model.

## 4c. Footnote presentation (sketch, UX postponed)

Adopted direction (Kirill): footnotes render *as footnotes* — a zone at
the viewport bottom showing the definitions whose anchors are currently
visible. Footnotes are part of the text (bottom zone); margin space stays
reserved for annotations (not part of the text). The semantics align.

Design constraints that make it workable:

- **Numbering is global by document order**, displayed as-is in the zone —
  nothing renumbers on scroll; you simply see "7, 8" when those anchors
  are in view.
- **The zone is an overlay inset, not a layout resize** — text scrolls
  behind it; `max_scroll` and cursor-into-view account for the inset.
  Otherwise scrolling reflows the page, which is disqualifying.
- **Height cap ~⅓ of viewport**, internal scroll past that; long notes
  clamp with expansion. Too-many-footnotes can crowd the zone, never the
  text.
- v1 of the zone: read-only projection of the def blocks, click-to-jump;
  editing in place in the zone is a later, separate decision.

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
  reference the hash. **In-file always** (decided: portability beats file
  size; sidecar directories are how exports break).

### 5b. Image import policy (deterministic, decided 2026-06-10)

- **Stored natively**: PNG, JPEG, WebP — kept byte-identical when within
  caps (recompressing a JPEG is generation loss; PNGs are screenshots
  where lossless text matters).
- **Converted on import**: GIF/BMP/TIFF → PNG if the image has an alpha
  channel, else JPEG q88. (Alpha test is deterministic; "looks
  photographic" is not.) Animated GIF: first frame, with a notice.
- **SVG: refused in v1** — arbitrary SVG is a rendering and security
  surface, not a photo. Revisit if real demand appears.
- **Downscale only when oversized**: long edge > 2400 px → scale to
  2400 px (2× the 660 px column at 2× DPI, with headroom for zoom and
  export). Below that, never touched.
- **EXIF stripped** (GPS in photos is a privacy leak), orientation baked
  into pixels first.
- **Hard caps**: > 8 MB after the pipeline, or > 12000 px either edge
  pre-decode (decompression-bomb guard) → **refused with a message**,
  never silently degraded.
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

## Resolved questions (2026-06-10)

1. Highlight: **in**.
2. Links: **explicit creation only**, no autolink.
3. Images: **in-file**, policy in §5b.
4. Footnotes: **viewport footnote zone**, sketch in §4c; UX details later.
5. Code blocks: **in** — monospace, no highlighting, info string stored
   for round-trip only.
