# Research — the book page: metrics, face, texture, flip UX

*(Cold read, impl spec 05. Design law: golden-path §9.1–9.3 + D3 —
BOOKISH face wins, justified + real hyphenation, irregular noise
texture, running head + centered folio, flip zones. This document
reconciles the browser mock's numbers against book typography and
settles the bundled typeface with verified licensing and Cyrillic
coverage. Researched 2026-07-06. No code touched.)*

Method note: everything checkable locally was checked locally, not
taken from the web — Cyrillic coverage via `fc-query` language sets
and fontTools cmap probes on the actual font files; character
widths measured from `hmtx` advances; candidate fonts downloaded
from the google/fonts repo and measured; static instances cut with
`fontTools.varLib.instancer`; the gpui fallback API read from the
fork source. Web sources are cited inline and listed at the end.

---

## 1. Page metrics, reconciled

### 1.1 The measure (the mock is too narrow — for Russian)

Bringhurst: 45–75 characters per line (incl. spaces) is
satisfactory for a single-column page; "the 66-character line is
widely regarded as ideal" (*Elements of Typographic Style* §2.1.2).
Hochuli (*Detail in Typography*) is tighter: 50–60 characters,
8–10 words. Justified setting is the demanding case: below ~45
characters even good hyphenation can't stop gappy word-spacing, and
Bringhurst's own advice is to go ragged when the measure is short.

Measured average advances of URW Bookman Light (fontTools, from
`hmtx`, realistic mixed-case sentences including spaces):

- English: **0.493 em** → 8.14 px per char at 16.5 px
- Russian: **0.545 em** → 8.99 px per char at 16.5 px

Cyrillic runs ~10% wider than Latin in the same face — a Russian
line always holds fewer characters, and Russian words are longer.
Russian is first-class here, so the measure must be sized for the
Russian column, not the English one.

The mock's measure: 520 − 2×58 = **404 px** → **~50 EN / ~45 RU**
characters per line. English sits at the comfortable low end;
Russian sits exactly on the justification floor. Verdict: widen.

| measure @16.5px | EN chars | RU chars | note |
|---|---|---|---|
| 404 px (mock) | 50 | 45 | RU at the justified floor |
| **450 px** | **55** | **50** | recommended; Hochuli's band both ways |
| 490 px | 60 | 54 | fine, page starts to dominate |
| 537 px | 66 | 60 | Bringhurst's ideal EN needs this much |

66 EN chars would need a 537 px measure (657 px page) — visually
dominant on the desk and pushing RU to 60, wider than Hochuli
likes. **Recommendation: 450 px measure at 16.5 px** (55 EN /
50 RU) — squarely inside every authority's band in both languages.

### 1.2 Leading (the mock is too loose)

Bookman-class faces are wide, open-countered and low-contrast; the
classic rule (Bringhurst §2.2.1) is that dark, wide and
large-x-height faces want *more* lead than delicate ones — but book
norms for Bookman-class text are 1.25–1.45 (e.g. 10/13, 11/14.5).
Screens read a bit looser than paper; e-reader defaults land around
1.4–1.5. The mock's 1.66 is web-article air: it compensated for
ragged-right and browser rendering. A justified, hyphenated block
has an even texture and doesn't need it; at 1.66 the "book" reads
like a blog post.

Measured relevant metrics: URW Bookman Light x-height 0.485 em
(PT Serif: 0.500) — the face is *wide* (avg advance +10% vs
PT Serif) rather than tall. Wide + low contrast → generous but not
extreme leading.

**Recommendation: line-height 25 px at 16.5 px type (1.52).**
Acceptable band 1.45–1.6; put 1.52 vs 1.58 through the visual rig
once real pages render — this is a taste call inside a settled
range. Whole-pixel line height keeps pagination arithmetic exact.

### 1.3 Page proportion and window behavior

Print pages run 1.4–1.62 (ISO √2 = 1.414; classic octavo 2:3 =
1.5; 5×8" trade = 1.6). The mock's 614/520 = **1.18** is squat —
a browser-viewport artifact, not a design.

What screens actually do: Kindle hardware is 4:3-ish (Paperwhite
1236×1648 = 1.33); the Kindle and Apple Books *apps* fill the
window/screen entirely and reflow — no fixed page aspect anywhere
in shipping readers. Strop's page is different: it is an *object on
the desk* (estrangement wants a visible page, not a filled
viewport), so it needs both a real proportion and respect for the
window.

**Recommendation — one rule, width from type, height from window:**

- Page **width is fixed at 570 px** (450 px measure + 2×60 px
  margins). It never tracks the window.
- Page **height fills the available window height** minus a desk
  gutter (≥ 24 px above/below), **capped at 1.5× width (855 px)**,
  and floored so the text block is a whole number of 25 px lines.
- Result at common windows: a 800 px-tall window gives a ~1.3
  page (squarer, like an e-ink device); a 1000 px window hits the
  1.5 cap and stops (a real book page). Both read as "book".

| window height | page (W×H) | ratio | text lines |
|---|---|---|---|
| 700 px | 570×652 | 1.14 | 21 |
| 800 px | 570×752 | 1.32 | 25 |
| 900 px | 570×852 | 1.49 | 29 |
| 1100 px | 570×855 (cap) | 1.50 | 29 |

(At 55 chars ≈ 9 words/line, 29 lines ≈ 260 words — a 3000-word
draft paginates to ~12 pages. "— 2 of 9 —" grammar stays honest.)

**Small windows:** never scroll a page and never rescale glyphs
continuously — both break the book fiction. Degrade in steps:
(1) shrink the desk gutter to zero (page absorbs the window,
e-reader posture); (2) below ~12 text lines, drop once to 15 px
type / 420 px measure (chars/line preserved); (3) below ~8 lines,
accept it — fewer lines per page just means more pages. Pagination
is recomputed on window resize (snap, no tween — same rule as the
margin lane's resize snap).

### 1.4 Margins, running head, folio

The mock's padding 52 top / 58 sides / 48 bottom puts the *larger*
margin on top — upside-down by book convention. Classical canons
(Van de Graaf / Tschichold, inner:top:outer:bottom = 2:3:4:6) make
the bottom margin the largest so the text block sits high; for a
single centered screen page, symmetric left/right is correct, and
only the top:bottom weighting carries over.

**Recommendation: 60 sides / 48 top / 64 bottom** (top:bottom =
3:4). The block sits high; the folio gets room to breathe.

**Running head** — content: the draft's name (history-preview
variant: the checkpoint's name, per §9.1 verdict 7 — name is the
payload). Books set running heads in the text size or slightly
smaller, in small caps or italic, well clear of the block. The
mock's 11 px (0.67× body) is UI-chrome scale; use **12.5 px
(~0.75×) italic**, muted ink, top-right, baseline ~26 px from the
page top (inside the 48 px top margin). Italic, not small caps:
URW Bookman has no small-caps feature, and fake small caps are the
kind of fake the cold read is sworn against. Truncate long names
with an ellipsis; the running head never wraps.

**Folio** — "— 2 of 9 —" centered at the foot is a classic drop
folio, endorsed as-is. Set it 12.5 px (same muted ink as the
running head), centered, ~26 px from the page bottom. Kindle shows
"Page 2 of 9" when print-mapped pages exist; Apple Books shows "N
of M" — the grammar is settled reader language, and our pages are
*real* pages of this rendering, so no locations/percentages.

### 1.5 Reconciled metrics table (GPUI build)

| token | mock (browser) | reconciled | why |
|---|---|---|---|
| page width | 520 px | **570 px** | 50-char RU floor → 450 px measure |
| page height | min 614 px | **fill − gutter, cap 855 px, whole lines** | screen page fills, print cap 1.5 |
| side margins | 58 px | **60 px** | round; symmetric is right on screen |
| top margin | 52 px | **48 px** | block sits high |
| bottom margin | 48 px | **64 px** | bottom largest (canon 3:4) |
| body size | 16.5 px | **16.5 px** | verified comfortable; keep |
| line height | 1.66 (27.4 px) | **25 px (1.52)** | book-honest for justified Bookman |
| measure | 404 px ≈ 50/45 chars | **450 px ≈ 55/50 chars** | Hochuli band in EN and RU |
| running head | italic 11 px | **italic 12.5 px** | 0.75× body, book scale |
| folio | 12 px centered | **12.5 px centered** | drop folio, keep |
| alignment | ragged (browser) | **justified + hyphenated** | design law; we own layout |

---

## 2. The bookish face

### 2.1 What "Bookman-class" means

The lineage: A. C. Phemister's *Old Style Antique* (Miller &
Richard, Edinburgh, c. 1858) → American foundry copies renamed
*Bookman* → ITC Bookman (Ed Benguiat, 1975): very wide, round,
large apparent x-height, low stroke contrast, big open counters —
the friendly, sturdy paperback voice. *Bookman Old Style*
(Monotype, Ong Chong Wah) is the Windows/Office incarnation,
tempered back toward the 19th-century models. *URW Bookman* (née
URW Bookman L) is URW++'s libre, metrically ITC-compatible cut
from the ghostscript base-35 set. The class signature the taste
test picked: roundness + width + low contrast = warm and unhurried,
maximally distant from PT Serif's crisp transitional neutrality —
which is exactly the estrangement stimulus D3 wants.

### 2.2 Candidates — verified matrix

All coverage claims below verified locally (fc-query language sets
and/or fontTools cmap probe for U+0410–044F, Ё/ё); sizes are actual
file sizes; "static" = instance cut from the variable font.

| face | class fit | Cyrillic | license | reg + italic size | verdict |
|---|---|---|---|---|---|
| **URW Bookman** (urw-base35 2020-09-10) | IS Bookman | **yes** — ru uk be bg sr kk + el, all 4 styles | AGPL-3.0 + PS/PDF-embed exception | **98K + 103K** (OTF; Demi +97K, DemiItalic +102K) | **primary** |
| TeX Gyre Bonum (GUST/GFL) | IS Bookman (same metrics) | **NO** (U+0412 absent; no ru in fc langs) | GFL (LPPL-derived) | 143K + 147K | disqualified as primary |
| Literata (OFL) | warm-contemporary, made for e-books | yes | OFL 1.1 | 264K + 253K static | **OFL alternate** |
| Vollkorn (OFL) | warm dark old-style | yes | OFL 1.1 | 355K + 278K static | bookish, not Bookman |
| Alegreya (OFL) | literary but calligraphic, narrow (0.408 em) | yes | OFL 1.1 | 262K static | wrong energy |
| Gentium Book Plus (OFL) | humanist book face | yes | OFL 1.1 | 800K + 862K | files huge; not Bookman |
| Bitter (OFL) | contemporary slab | yes | OFL 1.1 | 320K var | that's the "slab" grade, a different stimulus |
| Source Serif 4 (OFL) | neutral transitional | yes | OFL 1.1 | 1181K var | neutral, not warm |
| Noto Serif (OFL) | neutral workhorse | yes | OFL 1.1 | 1842K var | neutral; huge |
| PT Serif (OFL) | the *editor* face | yes | OFL 1.1 | already bundled | zero estrangement — excluded by definition |

Width check (avg EN advance, em): URW Bookman 0.493 — the widest
of the whole table (Literata 0.482, Bitter 0.456, Vollkorn 0.438,
Gentium 0.425, Alegreya 0.408). The measured geometry agrees with
the taste test: nothing OFL actually replaces Bookman's roundness;
Literata is the closest spiritually (commissioned by Google for
Play Books, TypeTogether 2015; Cyrillic by Vera Evstafieva) but
reads contemporary-neutral-warm, not round-friendly.

URW Bookman ships **Light, Light Italic, Demi, Demi Italic** — no
"Regular"; Light *is* the classic ITC Bookman text weight. That is
authentic, not a gap.

### 2.3 System availability per OS (verified)

- **Windows 11 (base):** Bookman Old Style is **not** in the
  shipped font list (verified against the full Microsoft Learn
  list). It arrives only with Microsoft 365 as a cloud/Office
  font. Base serif faces: Cambria, Constantia, Georgia, Palatino
  Linotype, Sitka, Times New Roman. Georgia and Palatino Linotype
  carry Cyrillic.
- **macOS Sequoia:** Bookman Old Style, Iowan Old Style and
  Athelas sit in the **document-support** list — "available only
  to documents that already use the font, **or to apps that
  request the font by name**" (Apple's wording). Palatino and
  Georgia are in the main installed list. Iowan Old Style is
  Apple Books' own default reading face — the closest
  system-resident bookish serif on the Mac — but it is Latin-only.
  ITC Bookman itself has not shipped with macOS in the modern era.
- **Linux:** URW Bookman comes from `fonts-urw-base35`
  (Debian/Ubuntu) / `urw-base35-fonts` (Fedora), pulled in by
  ghostscript — very commonly present (this machine has it three
  formats deep), but not guaranteed on a minimal install.

Conclusion: **no OS can be trusted to have a Cyrillic-complete
Bookman**. Bundling is mandatory, as the design law assumed.

### 2.4 Recommendation

**Bundle URW Bookman Light + Light Italic (+ Demi if pages ever
need bold) — ~200 KB for two styles, ~300 KB for three.** For
scale: the app already ships 2.9 MB of PT fonts; this is noise.
Reactions and the running head use the italic; it carries full
Cyrillic (verified per-style). **Do not subset**: Russian+Latin
+Greek is the point, the files are already ~100 KB each, and
subsetting would break the offline any-language posture for a
saving that doesn't matter.

**License posture (AGPL-3.0 + exception, in a GPL-3.0-or-later
app):** AGPLv3 and GPLv3 are expressly one-way compatible — §13 of
each license permits combining/conveying the combined work, with
each part keeping its own license. The font exception (verbatim,
from upstream LICENSE and Debian's copyright file) additionally
permits embedding in PostScript/PDF documents regardless of the
document's license — irrelevant to us (strop doesn't embed fonts
in exports), but nice for users who print. Two implementation
choices keep the story maximally clean:

1. Load the OTFs as **runtime data files** (read from assets at
   startup and `add_fonts`), not `include_bytes!` — the same
   "runtime data" posture already adopted for the LPPL Russian
   hyphenation dictionary (ux-glossary appendix). Mere-aggregation
   readings don't get cleaner than a file on disk.
2. Ship the license text next to the fonts (as with
   `PTSerif-OFL.txt`) and add a NOTICE entry. `cargo-deny` audits
   crates, not assets — no CI change needed.

Debian and Fedora have shipped these exact files under this exact
license for years; this is a well-trodden path. **If the owner
vetoes AGPL-anything on principle: Literata (OFL) is the
alternate** — designed for long-form e-reading, Cyrillic-complete,
517 KB for regular+italic statics — at the cost of the actual
Bookman look the taste test picked.

**Fallback stack (per-OS, behind the bundled primary):**

```
"URW Bookman" (bundled)
  → "Bookman Old Style"   (macOS doc-support by-name; Office-era Windows)
  → "Iowan Old Style"     (macOS; Apple Books' face; Latin-only)
  → "Palatino Linotype"   (Windows base, has Cyrillic)
  → "Palatino"            (macOS installed)
  → "Georgia"             (both, has Cyrillic)
  → "PT Serif"            (bundled; guaranteed Cyrillic backstop)
```

gpui supports this directly — verified in the fork:
`TextStyle.font_fallbacks: Option<FontFallbacks>` where
`FontFallbacks(Arc<Vec<String>>)` is an ordered family-name list,
plus per-glyph system fallback below that. Because the primary is
bundled and Cyrillic-complete, the stack is a safety net for
exotic codepoints (emoji, CJK), not a rendering path Russian ever
takes — which is the whole argument for bundling.

**Two flags for the code recon:**
- URW ships OTF (CFF outlines); strop currently bundles only TTF.
  cosmic-text (Linux), CoreText and DirectWrite all rasterize CFF,
  but smoke-test a shaped Cyrillic page on the rig before
  committing. (Worst case: fontTools converts CFF→TTF; the AGPL
  permits modification.)
- Confirm `add_fonts`-registered families shadow same-name system
  fonts — Linux machines with `fonts-urw-base35` installed will
  have two "URW Bookman"s. (Same upstream file, so a mismatch is
  cosmetic, but determinism matters for wshot goldens.)

---

## 3. Paper texture without a browser

The endorsed look is the mock's feTurbulence fractal noise
(baseFrequency 0.75, 2 octaves, ~5% alpha) — i.e. fine-grained
*irregular* noise with no visible pattern, which is exactly what a
pre-baked grayscale noise tile gives at near-zero runtime cost.
Serious readers ship flat pages (Apple killed the skeuomorphic
texture in iOS 7; Kindle is flat), but the design round explicitly
endorsed subtle irregular grain, so: **one 256×256 grayscale PNG
tile (~47 KB, generated once, checked into assets), tiled across
the page quad at 4–5% opacity** — or, cheaper still, pre-multiplied
into the paper color so the page background is a single textured
tile draw with no runtime blending. High-frequency noise has no
low-frequency features, so a 256 px tile shows no visible
repetition (the mock's own tile was 140 px); 512 px doubles the
asset size for nothing. Generated and verified locally with:

```sh
magick \( -size 256x256 xc:gray50 +noise Gaussian \) \
       \( -size 128x128 xc:gray50 +noise Gaussian -resize 200% \) \
       -compose blend -define compose:args=60,40 -composite \
       -colorspace Gray -level 38%,62% png8:paper-noise-256.png
```

(The half-resolution second layer reproduces feTurbulence's second
octave — a faint clumpiness that reads "paper" rather than "TV
static". Output: 47 KB, mean 49% gray, σ≈18%; tiles seamlessly
because per-pixel noise has no cross-edge correlation. Composite
over `#FEFEFC` at 4–5% alpha and freeze it as the page fill.)

---

## 4. Page-flip and pagination UX

### 4.1 Flip zones

Shipping-reader geometry, verified: KOReader defaults (from
`defaults.lua`): **backward = left 25% full-height, forward = the
remaining 75%, menu = top ⅛**. Kindle hardware: a narrow ~0.5"
left strip for back, everything center-right for forward, top
strip for the toolbar. Apple Books: margin taps plus swipes.
Readers are asymmetric because thumbs favor "forward" hundreds to
one; a desktop mouse has no such bias, and strop's page flip is
announced by the hover gradient (control-is-indicator — the zone
*shows itself* before the click). **Keep the mock's symmetric
26% / 26% full-height zones.** The middle ~48% stays inert:
clicking prose is where reaction-marks live, and the margin-click
caret guard precedent says pointer real estate near text must not
page-turn out from under an aimed click.

### 4.2 Keyboard

Settled across Kindle/Apple Books/KOReader/PDF viewers, adopt
verbatim: **→ / PageDown / Space = next page; ← / PageUp /
Shift+Space = previous; Home / End = first / last; Esc = the
desk.** (Impl spec 05 already promises arrows + Esc; Space is the
one addition readers have trained into everyone.)

### 4.3 Folio grammar

Confirmed fine. Kindle shows "Page 2 of 9" where print-mapped
pages exist (its locations/percent modes exist because reflowable
pages are unstable across font settings); Apple Books shows
"N of M". Strop's pages are the *actual* pages of this rendering,
so "— 2 of 9 —" is both honest and idiomatic. No percentages, no
time-left, no locations.

### 4.4 Flip animation

Apple Books offers Curl / Slide / None (Slide became the iOS 16
default; the skeuomorphic curl was restored by demand in 16.4 as
an *option*). Kindle e-ink flips are instant; KOReader ships
animation off. Strop's motion law (attention-motion.md) is
decisive here: **animate moves, not pops** — and a page turn is a
pop (in-place content replacement), so curl and slide are ruled
out as kitsch-by-law, not just taste. **Recommendation: instant
content swap, at most a 100–120 ms opacity cross-fade on the
incoming page** (the enter-fade precedent: new margin cards fade
250 ms; a page can be brisker since the user caused it).
`reduce_motion` ⇒ strictly instant — the config switch and the
"reduced motion is not no motion, but travel becomes fades" rule
already exist; a fade *is* the reduced form, so this feature may
simply use the fade for everyone or drop to instant under
reduce_motion. The hover gradient on the flip zone is the
anticipatory cue; feedback-by-animation is not needed.

### 4.5 Edges (first/last page)

No bounce — a bounce is motion-noise on a pop. **Dead zone with
the affordance withdrawn**: on page 1 the left flip zone shows no
hover gradient and eats the click; likewise the right zone on the
last page (control-is-indicator: the control disappearing IS the
"you are at the edge" message, corroborated by "— 9 of 9 —").
Kindle's past-the-end "book finished" interstitial is a
storefront behavior; strop wants nothing there.

### 4.6 Position memory across exit/re-entry

E-readers remember position aggressively (Kindle syncs it across
devices) — because a novel is read over weeks and losing your
place is the cardinal sin. The cold read is the opposite artifact:
a *ritual read-through* of a short draft, entered from a quiet
checkpoint (D3), re-paginated on every entry (page identity is not
stable across edits or window sizes, so a restored "page 4" is a
lie waiting to happen). **Always enter at page 1.** The cost of
the rule is a few Space presses after an accidental Esc; the gain
is that entering the reading room always means the same thing —
the performance starts from the top, as estrangement demands. If
tester feedback later surfaces real pain, the cheap concession is
session-local only (restore within the same sitting, never
persisted), but do not build it speculatively.

---

## Sources

Book typography: [webtypography.net §2.1.2 (Bringhurst on
measure)](http://webtypography.net/2.1.2) ·
[Kaplan-Moss, Typography: Rhythm & Proportion (Bringhurst
summary)](https://jacobian.org/2008/nov/21/typography-rhythm-proportion/) ·
[Wikipedia: Line length](https://en.wikipedia.org/wiki/Line_length) ·
[Wikipedia: Canons of page construction (Van de Graaf /
Tschichold)](https://en.wikipedia.org/wiki/Canons_of_page_construction)

Faces & licenses: [ArtifexSoftware/urw-base35-fonts
(GitHub)](https://github.com/ArtifexSoftware/urw-base35-fonts) ·
[upstream LICENSE (AGPL-3.0 + font
exception)](https://raw.githubusercontent.com/ArtifexSoftware/urw-base35-fonts/master/LICENSE) ·
Debian `fonts-urw-base35` copyright file (local, verbatim
exception text) · [Fedora
urw-base35-bookman-fonts](https://packages.fedoraproject.org/pkgs/urw-base35-fonts/urw-base35-bookman-fonts/) ·
[Bookman Old Style — Microsoft
Typography](https://learn.microsoft.com/en-us/typography/font-list/bookman-old-style) ·
[Cloud fonts in Office](https://support.microsoft.com/en-us/office/fonts/cloud-fonts-in-office) ·
[Windows 11 font list — Microsoft
Learn](https://learn.microsoft.com/en-us/typography/fonts/windows_11_font_list) ·
[Fonts included with macOS Sequoia — Apple
Support](https://support.apple.com/en-us/120414) ·
[Literata — TypeTogether](https://www.type-together.com/literata-book) ·
[Literata — Wikipedia](https://en.wikipedia.org/wiki/Literata) ·
[googlefonts/literata](https://github.com/googlefonts/literata) ·
[Iowan Old Style — Wikipedia (Apple Books
default)](https://en.wikipedia.org/wiki/Iowan_Old_Style) ·
[Bookman typeface history — Fontesk](https://fontesk.com/bookman-typeface/) ·
[GUST Font License](https://www.gust.org.pl/projects/e-foundry/licenses)

Reader UX: [KOReader defaults.lua (tap
zones)](https://github.com/koreader/koreader/blob/master/defaults.lua) ·
[Kindle Paperwhite touchscreen zones —
dummies.com](https://www.dummies.com/article/technology/electronics/tablets-e-readers/kindles/how-to-use-the-touchscreen-on-your-kindle-paperwhite-168729/) ·
[Kindle page numbers — How-To
Geek](https://www.howtogeek.com/715778/how-to-see-a-books-page-number-on-amazon-kindle/) ·
[Apple Books page-turn options —
MacRumors](https://www.macrumors.com/how-to/re-enable-page-turning-animation-apple-books/) ·
[iOS 16.4 Books curl restoration — Good
e-Reader](https://goodereader.com/blog/e-book-news/ios-16-4-introduces-page-turn-animations-to-apple-books)

Local verifications (this machine, 2026-07-06): fc-query language
sets (URW Bookman ru/uk/be/bg/sr/kk/el on all four styles; TeX
Gyre Bonum none); fontTools cmap probe (Bonum missing U+0412);
hmtx advance measurements at 16.5 px; varLib.instancer static
cuts; gpui fork `crates/gpui/src/text_system/font_fallbacks.rs`;
ImageMagick texture generation + tile test.
