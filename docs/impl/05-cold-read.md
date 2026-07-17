# Impl spec 05 — the cold read (BUILD SPEC, 2026-07-06)

*(Supersedes the deferred stretch-package seed. Design settled across
golden-path rounds 1–4 — the binding-decision register with citations
is `cold-read/design-law.md`; the code map is `cold-read/recon-code.md`;
the typography engineering research is `cold-read/research-linebreak.md`
(the OWNER'S DIRECTIVE: justification+hyphenation is spec-driven from
typography literature, never screenshot-driven — the browser mock's
justification was judged bad); page metrics/face/texture research is
`cold-read/research-page.md`. Branch: `cold-read` off golden-path-impl
@ bb13058. Corner-case adjudications live in
`cold-read/adjudications.md` and rule over this file where they
conflict.)*

## 0. What ships

A writer-invoked, read-only, paged takeover: the manuscript re-typeset
as a book page — bookish face, justified + real hyphenation, real
pagination, numbered flippable pages, subtle irregular paper grain —
centered on a quiet desk. Entry quietly records a fingerprint-guarded
checkpoint. Reactions (`? doubt` / `! alive` / `~ drags` / a few words)
file from page selections as ordinary content-anchored margin notes.
Esc returns to the desk. The same renderer serves the history
"read this version" preview with a Restore chip. No AI first-move, no
churn heat, no invitation to enter, no toast, ever.

## 1. Architecture

- **State:** `Editor.cold_read: Option<ColdRead>` beside
  `history_preview` (the state-on-Editor + render-switch precedent).
  ```
  ColdRead {
    source: ColdReadSource,        // Live | Past { name, created_unix, }
    book: BookLayout,              // the paginated result (§2)
    page: usize,                   // current page index
    words: usize,                  // manuscript word count at entry
    station: Option<String>,       // newest checkpoint name at entry (banner data)
    selection: Option<Range<usize>>, // manuscript-space char range (word-snapped)
    input: Option<Entity<TextField>>,// the reaction input (one line)
    pulse: (Instant-ish, bool),    // banner refusal pulse
  }
  ```
- **Render:** a third custom element sibling (`EditorElement`,
  `StripElement`) is NOT needed — the page is painted by a dedicated
  prepaint/paint path inside the root render switch, the way banners
  and the strip overlay mount. All shaping in prepaint, never in paint
  (the 2026-06-12 law); `Editor.last_frame`/`LayoutKey` untouched —
  the book keeps its own layout (§2.6).
- **Key context:** a `Some("ColdRead")` binding context (the NoteInput
  precedent) owns escape / left / right / space / shift-space /
  pageup / pagedown / home / end while the takeover has focus. The
  reaction input's TextField keeps its own context on top.
  `escape_mode` additionally gets a top guard branch (belt for focus
  loss).
- **Module:** new `crates/strop-app/src/bookpage.rs` for the layout
  engine (pure logic over an abstract measurer — unit-testable without
  gpui) + `hyphen.rs` for dictionary loading/routing. Surface code
  lives in editor.rs beside its siblings.

## 2. The layout engine (Wave A; spec-driven)

The full engineering rationale, parameter sources, and pipeline are
`cold-read/research-linebreak.md` — its §4 table and §8 pipeline are
NORMATIVE. Summary of the binding choices:

**2.1 Fragments.** Paragraph → whitespace-split tokens (U+00A0 never
splits) → fragments carrying: text, style-run slice, measured width
(shaped, cached), space-after width (per-style shaped space; 0 at
paragraph end), break class (Free | AtHyphen | Bound). A token that is
just `—` binds to the preceding word (тире never starts a line);
digit-bearing tokens never hyphenate and their internal hyphens are
not break opportunities; internal-hyphen compounds split at the hyphen
(break allowed there, `\exhyphenpenalty`-charged, nothing inserted).

**2.2 Line breaker: greedy best-fit with TeX-style badness (v1).**
When the next word misses at minimum spacing, enumerate: break before
it, or break at each hyphenation point of the straddling word
(hyphenation looked up only then, on demand). Score `badness =
100·|r|³` with r from the real stretch/shrink windows, plus
`\hyphenpenalty 50`, `\exhyphenpenalty 50`, and a LARGE penalty past
2 consecutive hyphen-ended lines (3 = hard cap). Total-fit DP (same
fragment model, TeX demerits) is the committed v2 "if rivers offend."

**2.3 Justification parameters** (research §4 — key values):
word-space min **0.80×** / preferred max **1.33×** / acceptable max
**2.00×** nominal (the shaped space of the run's own style — never
hardcoded); beyond 2.0× distribute anyway and debug-flag.
**Letterspacing never** (law). Hyphenation: min word 5 chars; edge
minima come from the dictionaries themselves (en 2/3, ru 2/2 —
Russian genuinely differs); never hyphenate a paragraph's last word;
the last line sets natural, never justified. A word wider than the
measure overflows left-aligned, never squeezed.

**2.4 Hyphenation.** `hyphenation` crate 0.8.4 (code Apache-2.0/MIT,
verified). Dictionaries are **runtime-loaded loose files** —
`assets/hyphenation/{en-us,ru}.standard.bincode` (≈130 KB total),
`Standard::from_path`, held in a `OnceLock`, loaded on first entry.
**Never `include_bytes!`**: hyph-ru is LPPL 1.2+ (GPL-incompatible;
mere-aggregation posture, the hunspell precedent); en-US (Kuiken,
permissive) ships the same way for symmetry. Missing file → justify
without hyphenation for that script + log. NOTICE gains Kuiken and
Lebedev/LPPL attributions. Language routing: per word by first
alphabetic char's script — Cyrillic → ru, Latin → en-US, else none.
Soft hyphens (U+00AD) override the dictionary (crate behavior),
stripped from painted text. The slice string is NEVER transformed
(adjudications F2): hyphenation looks up an NFC copy of the single
word and skips the word when its NFC form differs from the raw form.

**2.5 Shaping.** Shape per fragment via
`shape_line(text, size, &[TextRun], None)`; hyphenated fragments are
re-shaped as `prefix + "-"` (U+002D, never U+2010) — the painted
string is the shaped string, always. Paint each fragment at its
computed pen position (`ShapedLine::paint`, TextAlign::Left); gaps
are exact f32 arithmetic, spaces never painted. Paragraphs with RTL
characters set ragged-left, unjustified, unhyphenated (honest
degradation). Style runs crossing word boundaries reslice per
fragment; bold/italic inside one word shapes in one call.

**2.6 Caches & performance.** gpui's LineLayoutCache is frame-scoped
— useless at pagination time. Own immortal width cache
`(fragment string, style key) → width` for the view's lifetime.
**Pre-paginate the whole manuscript synchronously at entry** (page
count must be true immediately); budget < 100 ms for a 5k-word doc
(rig microbench verifies). Re-pagination triggers: window resize
(snap, then reopen at the page containing the previous page-top char)
and nothing else — the view reads a snapshot; filing a reaction bumps
`Document::revision` but must NOT re-paginate (the book is keyed on
the entry snapshot, not live revision).

**2.7 Pagination (page breaking).** Widows/orphans ≥ 2 lines each
side; a heading is never the last block on a page and keeps ≥ 2 lines
of its following paragraph; avoid a hyphen on a page's last line when
one-line movement fixes it. Per-BlockKind: headings set in the book
face's Demi at ~1.15× body with keep-with-next; lists keep their
markers (justified, marker-indented); quotes indent one em both
sides; code blocks set in PT Mono, ragged, never hyphenated; images
scale to the measure, keep whole (move to next page if short; scale
to page if oversized). Footnote refs paint as superscripts (the
fn_marks machinery). *AMENDED 2026-07-17 (supersedes the v1
off-page law; G1's research item is resolved by convention):*
definitions set at the BOTTOM of the page whose body carries their
ref's superscript — print convention: reduced-alpha third-of-measure
rule, `FOOTNOTE_AIR`, 0.85× face with proportionate leading, the
ref-order number as a hanging marker, justified/hyphenated by the
body engine. Pagination reserves the space; a ref whose note cannot
share its page moves forward WITH the note (whole-note move-forward
v1; continued-note splitting only when a single note exceeds a
page's usable area). A definition with no in-text ref sets on the
final page after the anchored notes — the writer's text is never
silently dropped; the unanchored note IS the honest rendering of an
unanchored source line.

**2.8 Unit tests (Wave A gate).** The breaker/paginator run over a
fake measurer: badness candidate choice, hyphen-streak cap, widow/
orphan, dash binding, U+00A0, digit tokens, last-line law, compound
splits, offset realignment (capitalized word, «Ёлка», U+00AD-bearing
word; every break a char AND grapheme boundary), page-top-char resume
after remeasure.

## 3. Typography & assets

**3.1 Face: URW Bookman** (urw-base35 2020-09-10) — a true libre
Bookman with verified full Cyrillic on all styles. Bundle **Light,
Light Italic, Demi** (~300 KB; do not subset). License AGPL-3.0 +
embedding exception — one-way GPLv3-compatible; posture: **runtime
data files** (read at startup + `add_fonts`, NOT `include_bytes!` —
the same posture as the dictionaries), license text beside the fonts,
NOTICE entry. Source: this machine's `fonts-urw-base35` package files
(same upstream). Fallback stack via `font_fallbacks`:
`URW Bookman → Bookman Old Style → Iowan Old Style → Palatino
Linotype → Palatino → Georgia → PT Serif`. If the bundled files are
missing at runtime the page falls back honestly (log; PT Serif
backstop). **Wave A gate: CFF/OTF smoke test on the rig** (strop has
only bundled TTF so far); worst case convert CFF→TTF (license
permits).

**3.2 Page metrics** (research-page §1.5 — NORMATIVE, deliberate
divergences from the lab mock, pre-named for Gate 2):

| token | value (at font_scale 1.0) |
|---|---|
| page width | **570 px** (450 px measure + 2×60) |
| page height | fill window − ≥24 px desk gutter, **cap 855 px** (1.5×w), floored to whole lines |
| margins | 60 sides / 48 top / **64 bottom** (block sits high) |
| body | **16.5 px / 25 px line height (1.52)** — 55 EN / 50 RU chars per line |
| running head | 12.5 px italic, muted, top-right, ellipsized, never wraps |
| folio | `— 2 of 9 —` 12.5 px, muted, centered, drop |
| paper / desk | `#FEFEFC` / `#E4E6E9` (mint tokens PAGE_PAPER, DESK_BG) |
| shadow | the mock's double shadow (2/4 + 12/30) |

All values scale by the existing `font_scale`; line height re-rounds
to whole px. Small windows degrade in steps: gutter→0; below ~12
lines drop once to 15 px/420 px measure; below that, fewer lines =
more pages. Never scroll a page, never continuously rescale glyphs.
Running head content: live read `{doc name} — draft`; history variant
`{doc name}` alone (the banner already carries the checkpoint name —
no duplication).

**3.3 Texture.** One 256×256 grayscale two-octave noise tile
(committed asset; the exact `magick` command is research-page §3),
noise pre-multiplied over the paper color so the page fill IS the
tile — drawn tiled (≤12 clipped quads), zero runtime blending.

## 4. The surface (Wave B)

**4.1 Banner** (strings are LAW, lab v4): a full-width row flush
under the titlebar (the strip-banner pattern: h≈30, `STALE_BG` wash,
hairline below):

> **Reading** · "Draft complete" · 4,120 words · Esc returns

— **Reading** bold, the pulse target; station = newest checkpoint's
name at entry in typographic quotes (absent → segment omitted);
count = manuscript words at entry; the `·` before "Esc returns"
dimmed. History variant:

> **"Draft complete"** · Sun 6 Jul · 4,120 words [Restore] · Esc returns

— name-led bold, real date (computed at render), plain dark
**Restore** chip (routes through `restore_to_state`, which is taught
to exit the takeover and unpark the strip — adjudications F3).
Titlebar: doc name + muted `— reading` / `— viewing "Draft
complete"`; word-count pill and editor button hidden; ⌕, history
clock, ≡ remain (their handlers guard, §4.5).

**4.2 The refusal pulse.** Typing (any text-insert key), a guarded
chord, or a blocked verb pulses the word **Reading** (warm
`--seltint` wash, 180 ms ramp, ~900 ms hold, single-pulse) — the
mode-matrix "one pulse idiom, never a silent swallow". No toast.

**4.3 Flip.** Symmetric 26% left/right full-height zones; hover
shades an inward gradient (`rgba(26,30,38,.035)`) — withdrawn
entirely at the edges (page 1 left / last right = dead zone, no
gradient, click eaten; the folio corroborates). Middle ~48% inert
(selection territory). Keys: → / PageDown / Space forward; ← /
PageUp / Shift+Space back; Home/End first/last; each flip = instant
content swap with ≤120 ms opacity fade on the incoming page;
`reduce_motion` → strictly instant. Every frame passes the
screenshot test. Mouse wheel: nothing in v1 (flip is click/keys;
corner round may overrule).

**4.4 Suppression while open** (predicate `cold_read.is_some()`):
margin lane + rail, narrow pill/panel, flanks, footer chips (fixing
the pre-existing `history_view`-only gap in passing is lawful),
selection popover, editor menu (closed on entry; its rows extend the
H33 inert gate — never diagnose a document the screen isn't
showing), omnibar (closed on entry). The scraps seam, pile, and
graveyard never render — the book consumes `manuscript_slice`
(rebased triple; reactions add `manuscript_base_char()` back when
filing). The door state is untouched; a pass completing mid-read
parks because the parking predicate learns the room
(`typing_burst_live() || cold_read.is_some()` in `deliver_pass` AND
the lull watcher — adjudications F7); exit joins scroll/door/new-pass
as the flush trigger, after surfaces are restored.

**4.5 Chords that pierce the ColdRead context** (App-scoped):
palette (ctrl-shift-p), find (ctrl-f), strip (ctrl-alt-h), passes
(ctrl-shift-d/b), etc. — v1 default: **guard with the pulse** (the
parked-mode precedent guards mutations; we guard entry surfaces
too). Esc remains the one exit. Corner round owns the per-chord
table.

**4.6 Entry & exit.**
- **Verb:** `Read it cold` — commands.rs row, section View, chord
  **ctrl-shift-l**, aliases: cold read, reading, book, свежим
  взглядом, перечитать. (Glossary: carried term; ru translates the
  function.)
- Entry: `save_now()` → `add_checkpoint_if_changed("Cold read",
  false)` (a new `station_rank` arm ranks the name at reflex tier —
  bare tick), snapshot `manuscript_slice`, paginate, set the entry
  page, focus the takeover.
- Entry page (AMENDED 2026-07-10; supersedes "always page 1"): at or
  under **10,000 words** (`CR_RITUAL_MAX_WORDS`; ≈ a 40-minute read —
  one sitting from the top is plausible) the performance starts from
  the top, as research-page §4.6 argues. Over it, the read opens at
  the page of the **current chapter's** start: the nearest heading
  at-or-before the caret at the chapter grain = the shallowest
  heading level with ≥ 2 occurrences (a lone H1 is the title and
  must not swallow the grain), else the shallowest present
  (`bookpage::chapter_start`). The caret survives quit
  (files.rs intents sidecar), so after a rest it still marks the
  last-worked chapter. No grain heading before the caret, a caret
  outside the manuscript, or any Past read → page 1. The threshold
  is in WORDS, not pages — behavior must not flip with window size.
  Dividers are scene grain and never chapter marks. **Home remains
  the one-key return to the ceremonial top.**
- Strip open at now: a Live entry closes it first. Canvas showing
  the PAST (parked strip or history panel): the Live verb guards with
  the pulse (adjudications Time 1). The omnibar/menu/popover close;
  the composer resolves first (F8); a running pass does NOT block
  entry (results park per F7).
- Exit (Esc, two-level): reaction input open → close input (clear);
  else drop the takeover, restore every suppressed surface, return
  caret/scroll exactly as left (nothing moved — the takeover never
  touched them). Esc from a Past read returns to where it was
  invoked (the parked strip stays parked).
- Mutation attempts (paste, format chords, undo…) → pulse, no-op.

**4.7 History variant ("read this version").** Source: a
**materialized checkpoint state** — its OWN manuscript (the state's
own boundary; `checkpoint_state(cp)`). v1 affordance: a quiet `Read`
text-verb in the strip's parked banner beside Restore — rendered only
when the playhead resolves to a checkpoint WITH a materialized state
(adjudications Regions 1). Selection and copy stay LIVE in Past mode;
what Past disables is the reaction INPUT (never raises — you annotate
the present only; adjudications Scopes 6). Restore chip per §4.1.
Strip-scrub arbitrary moments stay in the flat parked preview — the
book binds to checkpoints only (v1).

## 5. Reactions (Wave B)

**5.1 Selection.** Word-box model: pagination emits per-fragment
hit boxes carrying manuscript char ranges; drag selects word-to-word
(union of boxes, warm selection tint via `paint_background`);
click alone = nothing (no caret exists). Selection collapses on
flip/resize/Esc.

**5.2 The input.** On mouseup with a selection: the amber-bordered
floating card (~250 px) under the selection — three chips
`? doubt · ! alive · ~ drags` above one line with placeholder
`…or a few words`. Chip click files immediately; Enter files typed
text; Esc closes. (Its floating-over-text placement is
mockup-approved — S9 stands; any move is a named divergence.)

**5.3 Filing — the body convention (P3: the glyph is text, not
widget state).** A chip files body `"? doubt"` / `"! alive"` /
`"~ drags"`; typed text files as-is. `Document::add_note(range +
manuscript_base_char(), body, now)` — an ordinary writer note: same
undo, same re-anchoring, same margin afterlife (done/dismissed fade;
orphan → Scraps with anchor fragment). Cosmetic rule at card render:
a body starting `[?!~] ` bolds its first glyph. No schema change.

**5.4 The reading lane.** 230 px right of the page (18 px gap; the
page + lane GROUP centers, and the lane is reserved from entry so the
page never moves — adjudications S2), top-aligned with the text
block: this session's reactions as warm note cards in DOCUMENT order
— muted ellipsized quoted anchor line (~42 chars, grapheme-safe),
then body. Overflow: recede-in-place, the margin's own grammar (S6).
**Anchor links:** clicking a lane card flips to the page containing
`range.start` and flashes the anchored words once (ARRIVAL_FLASH
grammar, via the book's own paint). Windows too narrow for the group:
the lane hides; reactions still file (cards await on the desk).

## 6. Decisions taken in this spec (arbitrations within settled law)

1. **No face choice in v1** (O3) — bookish only; the lab cycler was
   lab ground. Settings are apologies.
2. **Nothing at the end of the read** (O6) — no believing-pass
   offer; P2. The last page simply has no next page.
3. **Verb string** (O7): `Read it cold`, ctrl-shift-l.
4. **"Reading" homonym** (O10): accepted — the door's "Reading"
   (editor button) and the banner's "Reading" never co-occur
   on-screen (the button is hidden in the takeover). Recorded, not
   renamed.
5. **Running head suffix** (O11): live `— draft` fixed word; Past
   variant carries no suffix.
6. **Find inside the read** (O12): guarded with the pulse in v1;
   the reading room has no tools in it.
7. **Reaction storage** (recon §4 options): body convention, no
   schema change (P3 argues it; the design-law P3 line states it).
8. **Entry checkpoint name**: `"Cold read"`, manual=false, reflex
   rank via a new station_rank arm (CheckpointMeta kinds live on
   another branch; name-convention is this branch's law).
9. **Metrics diverge from the mock deliberately** (§3.2) — the
   pre-named Gate-2 divergence register is research-page §1.5.
10. **Inline formatting renders faithfully** on the page (it is the
    text): bold→Demi, italic→Light Italic, strike, highlight, code
    spans in PT Mono; find-tints and machine transients never.

## 7. Corner-round agenda (adjudicated before build)

The five lenses, each owning the feature-interaction set:
(a) **regions/scraps** — manuscript_slice edge cases (legacy top-era
docs, textless manuscript, boundary at 0/EOF, empty doc);
(b) **time/persistence** — checkpoint interplay (entry-guard
semantics vs other auto-checkpoints, Past-read of pre-Scraps
checkpoints, restore-while-reading, journal events for
entering/leaving?, strip interaction);
(c) **scopes/verbs** — the per-chord pierce table (palette, find,
strip, passes, export, save-copy-as, scraps travel, review toggle,
font-size chords?), select-all, copy (allowed like parked mode? what
does copy copy — page text with hyphens?!);
(d) **surfaces/attention** — reveal-clock/deferred-pass during read,
door pulse interactions, reduce_motion audit, wheel-flip verdict,
narrow/tiny windows, multi-window/single-instance, the footer-chip
gap fix;
(e) **notes/reactions** — reacting over a span boundary, overlapping
existing notes, reaction on text later edited/parked/exiled,
composer interplay (can the margin composer be open when entering?),
lane overflow (many reactions), duplicate selections.

## 8. Waves & gates

**Wave A (worktree agent): the engine.** bookpage.rs + hyphen.rs +
deps + assets (fonts, dictionaries, texture, NOTICE/licenses) +
unit tests (§2.8) + a rig microbench + the CFF smoke test. Gate:
clippy -D warnings; cargo test --workspace; the golden text fixture
paginates identically across two runs; entry budget measured.

**Wave B (worktree agent): the room.** State/element/key-context,
banner, flip, suppression, reactions, history variant, verbs,
station_rank arm, rig hooks (`coldread:open`, `coldread:flip:N`,
`coldread:react`, dump object `coldread{open,pages,page,source}` +
restored-surfaces bits), wshot goldens (EN page, RU page —
hyphenation visible, narrow window), Gate-2 side-by-sides vs lab
scene 1 (banner strings, reaction input, flip shading, folio must
MATCH; metrics divergences per the register), `seed:legacy` pass,
fresh-litmus real-file verification.

Corridor floor (unchanged): it looks like a book; page-flip by
click/arrow keys; Esc returns to the desk.
