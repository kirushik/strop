# Research: justified + hyphenated line breaking for the cold read

*(2026-07-06. Engineering spec research for impl spec 05 — the
book-page renderer. Spec-driven from typography literature and the
actual crate/gpui APIs, not from the browser mockup, whose
justification the owner already judged bad. Everything below was
checked against live sources on the date above; §9 lists the few
items that still need verification against our gpui fork or a unit
test before code is written.)*

## 0. Recommendations at a glance

| What | Choice | Version | License |
|---|---|---|---|
| Hyphenation | `hyphenation` crate | 0.8.4 (latest; Aug 2021) | Apache-2.0/MIT |
| — commons | `hyphenation_commons` | 0.8.4 | Apache-2.0/MIT |
| — en dictionary | `en-us.standard.bincode`, runtime-loaded file | 90,932 B | Kuiken notice-preservation (permissive) |
| — ru dictionary | `ru.standard.bincode`, runtime-loaded file | 42,531 B | LPPL 1.2-or-later (Lebedev) |
| Word segmentation | `unicode-segmentation` | already in Cargo.lock | MIT/Apache-2.0 |
| Line breaker v1 | **our own greedy best-fit with badness** (~100 lines) | — | — |
| Line breaker v2 | total-fit DP with TeX demerits; `textwrap` 0.16.2 (MIT) `wrap_optimal_fit` or `paragraph-breaker` 0.4.4 (MIT) as fallback impls | — | — |
| Shaping/painting | gpui `shape_line` + `ShapedLine::paint` at computed origins | our fork | — |

Justification is done by *positioning* whole shaped fragments (words
and hyphenated word-pieces) — spaces are never painted, gaps are
arithmetic. Letterspacing is never used. Parameter table in §4.

## 1. The `hyphenation` crate

### 1.1 Version and maintenance status

0.8.4 is still the latest (published 2021-08-19; checked crates.io
2026-07-06). The crate is dormant — last release almost five years
ago — but it is a pure function over static TeX pattern data with
three small deps (`bincode` 1.3.3, `serde`, optional
`unicode-normalization`). Dormancy here is stability, not rot. Pin
the exact version; vendor the two dictionary files we ship.

### 1.2 API surface (verified against docs.rs and the source)

Loading, embedded: cargo features are `embed_all` (all 121
dictionaries, ~2.8 MB in the binary) and `embed_en-us`. **There is
no `embed_ru` feature** — embedding Russian means embedding
everything. Moot for us: we runtime-load (see §1.4).

Loading, runtime (the calls we use):

```rust
use hyphenation::{Language, Standard, Load, Hyphenator};

let en = Standard::from_path(Language::EnglishUS, path_en)?; // bincode file
let ru = Standard::from_path(Language::Russian,  path_ru)?;
// also available: Standard::any_from_reader(&mut impl Read)
```

Hyphenating (from `src/hyphenator.rs`):

```rust
fn hyphenate<'t>(&self, word: &'t str) -> Word<'t, Self::Opportunity>;

pub struct Word<'t, Break> {
    pub text:   &'t str,
    pub breaks: Vec<Break>,   // for Standard: Vec<usize>
}
```

Facts that matter to us, each verified in the source or README:

- **Breaks are byte offsets into the ORIGINAL word.** Internally the
  word is case-folded before pattern matching, and the computed
  opportunities are realigned to the original string's byte
  positions (`realign(o, shifts)` in `hyphenator.rs`). So mixed-case
  and capitalized words (proper names, sentence-initial) hyphenate
  correctly with no caller-side folding. Add the §9 unit test anyway.
- **Soft hyphens win.** "Soft hyphens take priority over dictionary
  hyphenation; if the word contains any, they will be returned as
  the only breaks available." So an author's U+00AD is an override
  mechanism for free. The renderer must strip U+00AD from painted
  text (it is zero-width until used) and paint `-` when breaking at
  one.
- **Minimum lengths are built in, per dictionary.** `boundaries()`
  returns no breaks when the word's char count is below
  `l_min + r_min` from the dictionary parameters; opportunities
  never fall closer to the edges than the pattern file's
  `lefthyphenmin`/`righthyphenmin` (en-US: 2/3; ru: 2/2 — see §4).
- **Segmentation is the caller's job.** The crate hyphenates one
  word; the README's own example splits text with
  `unicode_segmentation` first. Punctuation handling is ours:
  - strip leading/trailing non-alphabetic chars from a token before
    hyphenating the alphabetic core (keep the offset shift);
  - tokens containing an internal `-` (compounds, «какой-то»):
    split at the hyphen and treat "after the existing hyphen" as a
    free break opportunity (no glyph inserted; see §5); hyphenate
    each part separately;
  - internal apostrophes (`don't`, `l'objet`): pass the token
    through whole; the apostrophe simply never matches a pattern
    character, so opportunities near it are suppressed — safe, just
    conservative;
  - tokens containing digits (`1-го`, `40%-ный`, `v0.8.4`): never
    hyphenated, and their internal hyphens are NOT break
    opportunities (Мильчин's rule; also just prudent).
- **Normalization.** The crate offers mutually-exclusive `nfc`/
  `nfd`/`nfkc`/`nfkd` features that normalize input to match the
  dictionary; the bundled patterns are in whatever form their
  authors chose (mostly NFC; "most often they don't" cover multiple
  forms, per the README). Recommendation: enable **no** crate
  feature and guarantee NFC at the document layer instead — strop's
  text is our own store, and en/ru NFC text is what the en-US and ru
  patterns expect. In NFC, «ё» is a single scalar (U+0451), not
  е + U+0308, and the ru patterns cover ё explicitly (§5).

### 1.3 Dictionary sizes

From the crate repo `dictionaries/` (GitHub contents API,
2026-07-06): `en-us.standard.bincode` = 90,932 bytes,
`ru.standard.bincode` = 42,531 bytes. Both together ≈ 130 KB on
disk; deserialization at first use is a few ms. Load once per
language, lazily, on first cold-read entry; hold in a `OnceLock`.

### 1.4 Licenses — the honest analysis

Code: `hyphenation` and `hyphenation_commons` are both declared
`Apache-2.0/MIT` (crates.io metadata for 0.8.4, both crates checked
individually). Clean for a GPL-3.0-or-later app; passes deny as
configured. The recon note's "verify hyphenation_commons before
merge" is hereby closed: **Apache-2.0/MIT, verified 2026-07-06.**

Pattern data — this is where nuance lives. The crate bundles
compiled forms of the hyph-utf8 TeX patterns, "© their respective
owners":

- **hyph-en-us.tex** header (fetched from the hyph-utf8 repo):
  "Copyright (C) 1990, 2004, 2005 Gerard D.C. Kuiken. Copying and
  distribution of this file, with or without modification, are
  permitted in any medium without royalty provided the copyright
  notice and this notice are preserved." Note this is *Kuiken's*
  extended US patterns (the ushyphmax lineage), not Knuth/Liang's
  original `hyphen.tex` with its "may not be modified" clause. The
  Kuiken notice is a simple permissive license, GPL-compatible.
  **English could legally be embedded in the binary.**
- **hyph-ru.tex** header: author Alexander I. Lebedev (the ruhyphen
  patterns, generated with patgen from a ~990k-word list and
  manually corrected), license **LPPL 1.2-or-later**. The FSF
  classifies the LPPL as a free license that is **not
  GPL-compatible** (its file-renaming/distribution conditions are
  restrictions the GPL does not permit adding). Embedding the ru
  dictionary via `include_bytes!` would make our distributed binary
  a single combined work carrying both GPL-3.0-or-later and
  LPPL-1.2+ material — neither license can absorb the other's
  terms. That is the murky position to avoid.

What runtime loading actually buys: the dictionary ships as a
**separate file** in our package, next to the binary. That is "mere
aggregation" (GPLv3 §5, final paragraph) — each work keeps its own
license, and *using* the data at runtime is unrestricted by the
LPPL (only distribution terms matter, and we distribute it as an
intact independent work with its notice). This is exactly the
posture of every GPL application that ships hunspell dictionaries
or fonts under non-GPL free licenses. Residual honesty notes:

- The `.bincode` is a format-shifted (arguably "modified") form of
  the patterns. LPPL 1.2 wants modified components renamed — the
  file *is* renamed (`ru.standard.bincode`), and we keep the
  hyph-ru.tex attribution + a pointer to the source patterns in
  NOTICE. Good faith is satisfiable; total pedantic certainty is
  not available with LPPL 1.2's TeX-centric wording. The practical
  risk is nil (this data is shipped this way by half the free
  software world).
- Consequence for packaging: **the dictionaries must NOT go through
  gpui's embedded AssetSource** (`include_bytes!`-based). They ship
  as loose files (assets dir next to the binary, or XDG data dir
  located via the `directories` crate already in the tree) and are
  read with `Standard::from_path`. If a dictionary file is missing
  at runtime, degrade gracefully: justify without hyphenation for
  that script (and log).
- Load *both* languages from files, even though en-US could be
  embedded: symmetry, ~91 KB off the binary, and users can drop in
  additional hyph-utf8 dictionaries later for free.
- cargo-deny never sees the dictionaries (they are data files, not
  crates); the compliance surface is our NOTICE file.

## 2. Language routing without language tags

Real systems do not detect language — they are told. CSS `hyphens`
only acts when the element has a `lang` attribute; InDesign and
LibreOffice carry a language attribute per character run. We have
no tags, so we adopt the simplest honest rule:

**Per word, sniff the script of the first alphabetic character:
Cyrillic block → ru dictionary; Basic Latin/Latin-1 letters → en-US
dictionary; anything else (Greek, CJK, digits-only, mixed) → no
hyphenation.** A word is atomic — never route halves of one word to
different dictionaries; for hyphen-joined mixed compounds
(«GPUI-архитектура») the §1.2 hyphen-split already yields per-part
routing.

Known, accepted misfires: French/German words get English patterns
(occasional wrong break — but such words are rare in strop docs and
a wrong *English* break in a French word is the status quo of most
justified English books quoting French); Ukrainian/Bulgarian
Cyrillic gets Russian patterns. Escape hatch if it ever matters: a
document-level default-language setting that overrides the Latin
branch — v2, config-only, no new machinery. Unknown scripts justify
fine without hyphenation; they just stretch more.

## 3. Line-breaking algorithm

### 3.1 What exists in Rust, mid-2026

- **`textwrap` 0.16.2** (MIT, maintained): `wrap_algorithms` module
  exposes `wrap_first_fit` (greedy) and `wrap_optimal_fit` — a
  Knuth–Plass-family total-fit optimizer over a `Fragment` trait
  (`width`, `whitespace_width`, `penalty_width`, all **f64**), with
  `Penalties { nline_penalty: 1000, overflow_penalty: 2500,
  short_last_line_fraction: 4, short_last_line_penalty: 25,
  hyphen_penalty: 50 }` and an `OverflowError`. Linear-time
  (SMAWK), documented as ~4× slower than greedy. Fits our
  "own fragments, own widths" architecture directly. Gaps for our
  use: no `doublehyphendemerits` analogue (cannot penalize
  *consecutive* hyphenated lines), and its cost is squared *gap to
  target width* — a ragged-right model with no shrink concept, so
  it cannot represent "fit one more word by compressing spaces
  to 0.8×". Usable as a v2 engine, but only as a proxy.
- **`paragraph-breaker` 0.4.4** (MIT, 2021): a faithful Knuth–Plass
  (total-fit + standard-fit) over box/glue/penalty items — i.e. it
  *does* model stretch and shrink. Widths are `i32`, so we would
  scale Pixels to fixed-point (×64). Dormant but small; readable.
- **parley** (Linebender): has justification (`Alignment::Justify`,
  renamed from `Justified` to match CSS; a justification
  space-count bug was fixed in 2025; HarfRust shaping since
  Aug 2025) but **no hyphenation** as of mid-2026, and adopting it
  means a second text stack beside gpui's. Rejected.
- **cosmic-text** (gpui's Linux shaping backend): no justified
  alignment at the buffer level (`Align` has no Justify; alignment
  API churn is ongoing per issues #343/#420). Nothing to reuse.
- **gpui** (our fork, verified): `TextAlign` = Left/Center/Right
  only; `shape_line(text, font_size, &[TextRun], force_width:
  Option<Pixels>) -> ShapedLine` (asserts no `\n` in the text) and
  `ShapedLine::paint(origin, line_height, align, align_width,
  window, cx)` — paint at arbitrary origin with `TextAlign::Left`
  is our substrate, already exercised by the footnote-superscript
  code (`editor.rs`).

### 3.2 Recommendation: greedy best-fit v1, total-fit v2, one fragment model

**v1 — greedy with badness (our own, ~100 lines).** Standard
first-fit, with one refinement: when the next word does not fit at
minimum spacing, enumerate the candidates — (a) break before the
word, (b) break at each hyphenation point of the straddling word
(hyphenation is looked up *only now*, on demand) — and score each
resulting line by TeX-style badness plus penalties, taking the
minimum:

```
r = slack / total_stretch      (if slack ≥ 0; total_stretch = Σ gap·0.33)
r = slack / total_shrink       (if slack < 0;  total_shrink  = Σ gap·0.20)
infeasible if r < −1
badness  = 100·|r|³
cost     = badness
         + 50    if line ends in an inserted hyphen        (TeX \hyphenpenalty)
         + 50    if line ends at a pre-existing hyphen      (TeX \exhyphenpenalty)
         + LARGE if the previous 2 lines also ended hyphenated (hard cap, §4)
```

This is one-line lookahead — no global optimization — but it always
picks the best available end for *this* line, uses shrink as well
as stretch, and respects the consecutive-hyphen cap.

Why this is acceptable at our measure: the cold read is a book page
at roughly 60–66 characters per line (10–12 words). Knuth & Plass
(*Breaking Paragraphs into Lines*, Software—Practice & Experience
11, 1981) demonstrate that the first-fit/best-fit quality gap grows
as the measure narrows — their dramatic failure cases are
newspaper-width columns; at book measure, with hyphenation ON and a
space that can shrink to 0.8× and stretch to 1.33× (§4), best-fit
stays inside the tolerance window on the large majority of lines,
and the failures are "a loose line now and then", not "gruesome
gaps". Butterick's floor is the real cliff: "If you're using
justified text, you must also turn on hyphenation to prevent
gruesomely large spaces between words" — hyphenation is the 90% of
the quality, the optimizer is the last 10%. What greedy cannot do:
trade a slightly-worse line now for a much better paragraph — so it
cannot avoid a loose line directly above a tight one (spacing
flicker), cannot minimize rivers, cannot fix runts. This is
precisely why InDesign ships both a Single-line Composer and a
Paragraph Composer and defaults to the latter. Hence:

**v2 — total-fit, triggered "if rivers offend"** (recon note's own
criterion). Swap only the chooser: a ~150-line DP over the same
fragments with TeX's demerit structure (\linepenalty 10,
\hyphenpenalty 50, \exhyphenpenalty 50, \doublehyphendemerits
10000, \finalhyphendemerits 5000, \adjdemerits 10000 — the TeXbook
defaults), feasibility from the real [min, max] gap windows. If we
would rather not hand-roll: `paragraph-breaker` models glue
correctly; `textwrap::wrap_optimal_fit` is the better-maintained
but shrink-less proxy.

The fragment model is shared by both, so design it once (§8): v1 →
v2 replaces one function.

## 4. Justification parameters — the spec table

Sources: the TeXbook's plain-TeX defaults and Computer Modern's
interword glue (cmr10 fontdimens: space 3.33333 pt, stretch
1.66666 pt (+50%), shrink 1.11111 pt (−33%)); Bringhurst, *The
Elements of Typographic Style*, on word space ("a typical value …
is a quarter of an em, M/4"; justified minimum "a fifth of an em",
"a reasonable maximum … M/2, and if it can be held to M/3, so much
the better"); InDesign's shipped justification defaults (word
spacing 80% / 100% / 133%, letter spacing 0/0/0, glyph scaling
100%); Butterick, *Practical Typography*; CSS Text Level 4
(`hyphenate-limit-chars: auto` ≡ `5 2 2`; `hyphenate-limit-lines`
initial `no-limit`; `hyphenate-limit-last` values incl. `always`
and `page`); hyph-utf8 pattern file headers; Мильчин, «Справочник
издателя и автора» (as summarized by ru.wikipedia «Перенос
(типографика)» and gramota.ru).

| Parameter | Spec value | Rationale / source |
|---|---|---|
| Word space, nominal | the font's shaped space advance for the run's style (measure `" "` via `shape_line`; ≈ M/4 for book faces) | Bringhurst M/4; never hardcode — italic and bold spaces differ |
| Word space, min | **0.80 × nominal** (hard floor) | InDesign default 80%; equals Bringhurst's M/5 when space = M/4; tighter than TeX cmr's −33% because shrink hurts more than stretch on screen |
| Word space, preferred max | **1.33 × nominal** | InDesign default 133%; ≈ TeX cmr stretch at badness 100 |
| Word space, acceptable max | **2.00 × nominal** | Bringhurst's "reasonable maximum" M/2; beyond preferred-max cost rises cubically (badness) |
| Beyond 2.0× | emergency: distribute the slack anyway (spaces only), flag the line in debug | TeX \emergencystretch analogue; a visibly loose line beats every alternative below |
| Letterspacing for justification | **0. Never. Law.** | "A man who would letterspace lower case would steal sheep" (Goudy, via Bringhurst); InDesign default 0/0/0 |
| Glyph scaling | none in v1 (and probably ever) | InDesign default 100%; micro-typography is out of scope |
| Min word length to hyphenate | **5 chars** | CSS Text 4 `hyphenate-limit-chars: auto` ≡ 5 2 2; crate additionally enforces l_min+r_min |
| Min chars before / after break | **en-US: 2 / 3; ru: 2 / 2** | stated in the pattern files themselves (hyph-en-us.tex: left 2 right 3; hyph-ru.tex: left 2 right 2 — Russian norm allows two-letter tails, «пе-ре-нос»); the crate enforces these from the dictionary, we do nothing |
| Max consecutive hyphenated lines | **2 preferred; 3 hard cap** | book practice 2–3; TeX discourages pairs via \doublehyphendemerits=10000; Мильчин tolerates up to 5 in book work (up to 7 in Soviet-era practice) so 3 is safely inside every norm; v1 enforces the cap via the LARGE penalty, v2 via doublehyphendemerits |
| Final word of a paragraph | never hyphenated | TeX \finalhyphendemerits; CSS `hyphenate-limit-last: always` semantics; cheap: don't request opportunities for the last word |
| Last line | set at natural width, ragged right; **never justified** | universal; the single non-negotiable of justified setting |
| Runts (single-word / very short last line) | v1: accept, except never a bare hyphenated fragment (already guaranteed by the row above); v2: penalize last line < ~25% of measure | textwrap `short_last_line_fraction: 4`; K-P handles via looseness; not worth greedy post-passes in v1 |
| Word wider than the measure (even hyphenated) | paint at natural width, left-aligned, overflowing into the outer margin; never squeeze or letterspace; debug-log | TeX's overfull box, minus the black rule |
| Rivers | no detection in v1 (note only) | total-fit reduces incidence as a side effect (Knuth–Plass 1981); detection is research-grade, not v1 |
| Widows/orphans (page breaks) | **≥ 2 lines of a paragraph on each side of a page break** | CSS `orphans`/`widows` initial value 2; Russian tradition bans висячие строки outright — 2/2 satisfies both |
| Headings at page end | a heading is never the last block on a page; keep it with ≥ 2 lines of its paragraph | standard book make-up rule |
| Hyphen on a page's last line | avoid if fixable by moving one line (i.e. treat as a soft page-break penalty) | CSS Text 4 `hyphenate-limit-last: page` precedent; Мильчин: перенос с полосы на полосу нежелателен; a reader flipping a page mid-word is the worst hyphen there is |

## 5. Russian specifics

- **ё**: hyph-ru.tex carries explicit ё patterns (а1вё, ё1ка, …) —
  NFC input hyphenates correctly with zero extra work. Texts that
  spell ё as е are the author's problem, not ours.
- **ь, ъ, й, single letters, prefixes**: all encoded in Lebedev's
  patterns (patgen over a 990k-word list, hand-corrected). Trust
  the dictionary; do not add rule code.
- **Тире (em dash).** The rule (gramota.ru; Мильчин): a dash must
  not begin a line, *except* the paragraph-initial dash of direct
  speech/dialogue. Implementation: forbid the break before «—» by
  merging — a token that is just `—` (spaced dash) binds to the
  preceding word as one fragment (`слово —`); breaking *after* the
  dash is fine. Paragraph-initial «—» needs nothing special (it is
  not preceded by a break opportunity). Respect existing U+00A0:
  **never break at a non-breaking space** — that single rule also
  lets authors bind whatever else they care about.
- **Дефис in compounds** («какой-то», «во-первых»): breaking at the
  existing hyphen is allowed; the hyphen stays on the first line
  and nothing is inserted (modern practice does not double the
  hyphen). Charge \exhyphenpenalty=50 so it is not *preferred* over
  a clean inter-word break.
- **Initials, numbers + units** («А. С. Пушкин», «1 км», «№ 5»):
  proper handling needs a smarter tokenizer — **v2**, except the
  two freebies we already have: U+00A0 is honored (authors can
  bind), and digit-bearing tokens are never hyphenated (§1.2).

## 6. Shaping interaction pitfalls (gpui)

- **Shape per fragment, not per line.** Justification repositions
  every word anyway, so per-line shaping buys nothing. What
  per-word shaping loses: kerning pairs across a word space (fonts
  define almost none, and justification is about to jitter that gap
  by ±20–33% regardless — genuinely negligible for book faces) and
  ligatures across spaces (do not exist in en/ru). Within a word,
  nothing is lost: a fragment carries its `&[TextRun]` slice, and
  `shape_line` accepts multiple runs, so a bold/italic boundary
  inside a word shapes correctly in one call.
- **Hyphenated fragments must be re-shaped as `prefix + "-"`.**
  Never compute `width(prefix within whole word) + width('-')`:
  splitting can break ligatures («dif-» out of “difficult” kills
  the ffi ligature) and changes kerning at the new edge, and the
  hyphen itself kerns against the last letter. The painted string
  is the shaped string, always. (Same for the remainder fragment
  starting the next line.)
- **Hyphen character: paint U+002D hyphen-minus**, not U+2010.
  U+2010 is typographically "correct" but missing from many fonts
  (fallback-glyph risk); every font has U+002D and browsers render
  automatic breaks with the same glyph. U+00AD in source text is
  never painted (strip it from fragment text; it is a break
  opportunity marker only — and per §1.2 the crate already treats
  it as overriding the dictionary).
- **Offsets:** the crate returns byte offsets into the original
  word (§1.2), so slicing fragment text and re-slicing its style
  runs is plain byte arithmetic on run lengths.
  `debug_assert!(word.is_char_boundary(b))` plus a grapheme-
  boundary assert (unicode-segmentation is already in the tree) —
  in NFC en/ru a combining mark can't be orphaned, but the assert
  is free.
- **Bidi/RTL: out of scope.** If a paragraph contains RTL
  characters, set it ragged-left (no justification, no
  hyphenation) rather than produce confidently wrong output.
- `shape_line` debug-asserts the text contains no `\n` (fork,
  text_system.rs:404) — fragments are intra-paragraph by
  construction, but keep the invariant in mind for the pipeline's
  paragraph splitter.
- Paint call in our fork: `ShapedLine::paint(origin, line_height,
  TextAlign::Left, None, window, cx)` (line.rs:83). We compute the
  x of every fragment ourselves; gaps are exact f32 pixels —
  distribute slack proportionally with no rounding and let gpui's
  subpixel glyph positioning handle the rest.

## 7. Caching and performance

- **gpui's LineLayoutCache is frame-scoped.** `finish_frame()`
  swaps previous/current maps and clears (fork,
  line_layout.rs:497–509): an entry survives only while it is
  painted every frame. Pagination-time shaping therefore gets no
  durable reuse from gpui — **our own width cache is mandatory**,
  and it is also what makes painting cheap (the visible page's
  fragments re-enter gpui's frame cache naturally).
- **Cache shape:** `HashMap<(SharedString, StyleKey), Pixels>`
  where StyleKey = (font id/weight/style, size) resolved per run
  signature; hyphenated fragments cache under their painted string
  (`"dif-"`) like any other. The cold read is a read-only view over
  a snapshot (entry creates the reflex checkpoint), so the cache is
  immortal for the view's lifetime: no invalidation logic at all.
- **Cost model** (to be confirmed by a rig microbench, §9): a
  5,000-word doc has ~1.5–2.5k unique (word, style) keys (Zipf).
  Cold `shape_line` of a short word through cosmic-text/HarfBuzz-
  class shaping ≈ 5–50 µs ⇒ **~10–80 ms one-time** at entry, upper
  bound. Hyphenation is looked up only for line-straddling words
  (~1 per line, ~400–600 lines) at ~1–5 µs each — noise. Greedy
  breaking is O(words); page assembly is trivial. Entry pagination
  total: well under 100 ms — acceptable for a modal view entry
  whose page count must be shown immediately.
- **Pre-paginate the whole document at entry, synchronously.** Page
  numbers must be stable and the total count displayed, so lazy
  per-page layout is disqualified by the product requirement, and
  the cost above doesn't justify background trickery.
- **Re-pagination triggers:** with a fixed em-based measure (a book
  page that rescales/re-centres in the window rather than reflowing)
  — **none during normal use**. If the design ends up scaling font
  size with the window, re-paginate on debounced resize-end and on
  font-size config change. Content never changes while the view is
  open (read-only snapshot), so there is no other trigger.
- Dictionary load: `from_path` once per language on first entry
  (~130 KB bincode total, a few ms), held in a `OnceLock`.

## 8. Pipeline

```
paginate(doc_snapshot, measure, page_height):
  for block in doc_snapshot.blocks():           # headings, paragraphs
    paras += split_paragraph(block)             # carries style runs

  for para in paras:
    tokens = split_on_whitespace(para)          # U+00A0 not a splitter
    frags  = []
    for tok in tokens:
      for part in split_at_internal_hyphens(tok):   # digit tokens: no split
        frags.push(Fragment {
          text: part, runs: reslice(para.runs, part.range),
          width: shaped_width_cached(part, style),
          space_after: space_width(style) | 0 at para end,
          break_after: Free | AtHyphen | Bound,     # Bound: before «—», at U+00A0
        })
    lines += break_para(frags, measure)

  pages = break_pages(lines, page_height)       # §4 widow/orphan rules
  return pages                                  # stable numbering, total count

break_para(frags, measure):                     # v1 greedy best-fit
  line = []; hyphen_streak = 0
  for each next fragment f:
    if fits_at_min_spacing(line + f): line.push(f); continue
    candidates = [ break_before(f) ]
    if f is alphabetic and dict = route_by_script(f):   # §2
      w = dict.hyphenate(core(f))               # on-demand, byte offsets
      for b in w.breaks (respecting §4 limits, skip if last word of para):
        candidates += break_inside(f, b)        # reshape "prefix-" cached
    pick candidate minimizing badness+penalties (§3.2), emit line,
    update hyphen_streak, start next line with the remainder
  emit final line (natural width, never justified)

paint_page(page, origin):
  for line in page.lines:
    gaps = justify(line, measure)               # spaces only; last line natural
    x = origin.x
    for frag in line:
      frag.shaped.paint(point(x, y), line_height, TextAlign::Left, None, …)
      x += frag.shaped.width + gaps.next()
```

## 9. Verify before building

1. Fork API signatures (checked 2026-07-06 against the local fork,
   re-check at build time): `shape_line` at
   gpui/src/text_system.rs:397; `ShapedLine::paint` at
   text_system/line.rs:83; `finish_frame` cache clearing at
   text_system/line_layout.rs:497.
2. Unit test the crate's offset realignment: `hyphenate` on
   "Anfractuous" (capitalized), «Ёлка», a word containing U+00AD
   (breaks = soft-hyphen positions only), and assert every break is
   a char AND grapheme boundary of the original string.
3. bincode compatibility: the crate pins bincode 1.3.3 — our
   Cargo.lock must not unify it upward past 1.x for that dep.
4. Space advance per style: measure via shaping `" "` per StyleKey
   (italic/bold spaces differ); confirm gpui returns a sane width
   for a whitespace-only ShapedLine (else measure "x x" − 2·"x").
5. Rig microbench: actual `shape_line` µs/word on the dev machine;
   confirm the < 100 ms entry-pagination budget for a 5k-word doc.
6. Packaging: dictionaries ship as loose files reachable by
   `from_path` in every distribution format we produce (NOT via
   gpui embedded assets); NOTICE gains the Kuiken and Lebedev/LPPL
   attributions.
7. If v2 adopts `textwrap::wrap_optimal_fit`: verify how
   `hyphen_penalty` detects hyphenated fragments (penalty_width >
   0?) and accept the no-shrink limitation consciously — or prefer
   `paragraph-breaker` / hand-rolled DP.

## 10. Sources

- hyphenation crate: crates.io API (0.8.4, 2021-08-19, Apache-2.0/
  MIT; hyphenation_commons idem); docs.rs front page; README and
  src/hyphenator.rs at github.com/tapeinosyne/hyphenation;
  dictionaries/ sizes via GitHub contents API.
- Pattern licenses: hyph-en-us.tex and hyph-ru.tex headers at
  github.com/hyphenation/tex-hyphen (hyph-utf8 master files);
  FSF license list on LPPL/GPL incompatibility (gnu.org/licenses/
  license-list).
- textwrap 0.16.2: docs.rs wrap_algorithms module + Penalties
  struct (defaults quoted in §3.1).
- paragraph-breaker: crates.io (0.4.4, MIT); Knuth & Plass,
  "Breaking Paragraphs into Lines", Software—Practice & Experience
  11 (1981) 1119–1184.
- parley: linebender.org "This Month in Linebender" (Alignment::
  Justify rename, justification space-count fix, HarfRust
  migration, parley 0.5.0); cosmic-text issues #151/#343/#420.
- Typography: Bringhurst, *The Elements of Typographic Style*
  (word-space M/5–M/4–M/2, letterspacing lore); Butterick,
  *Practical Typography*, "Justified text"; Adobe InDesign
  justification defaults (80/100/133, letter 0, glyph 100);
  the TeXbook (plain-TeX penalty/demerit defaults, cmr10
  fontdimens, \emergencystretch); CSS Text Level 4 + MDN
  (`hyphenate-limit-chars` auto = 5 2 2, `hyphenate-limit-lines`,
  `hyphenate-limit-last`); CSS 2.1 (`orphans`/`widows` initial 2).
- Russian norms: Мильчин, «Справочник издателя и автора» /
  «Издательский словарь-справочник» via ru.wikipedia «Перенос
  (типографика)»; gramota.ru справка № 214376 (тире в начале
  строки); hyph-ru.tex (lefthyphenmin 2 / righthyphenmin 2, ё
  patterns).
