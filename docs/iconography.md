# Strop — the icon plate

*(Companion to `design-principles.md` (the constitution),
`color-language.md` (who owns which hue) and `ux-glossary.md` (what words
reach the chrome). This doc owns the third channel: the few drawn marks.
Decided 2026-07-11 after a five-surface visual audit and a library
research pass; sources at the end.)*

## The verdict

Strop keeps **very few icons, and now they are one hand**: a bespoke
micro-set of ten single-color SVGs in `assets/icons/`, embedded into the
binary and rasterized by gpui's own resvg pipeline (`svg()`), tinted at
render time with the element's text color. Three of the ten borrow their
geometry from Lucide (ISC — `assets/icons/LICENSES/`); the rest are our
own drawings. No icon font, no library dependency, no new crates — resvg
already ships inside gpui.

This replaces the old three-idiom scatter (PT-glyph labels at text
weight, hand-stacked divs at another weight, painted quads at a third)
whose titlebar showed four different stroke weights in one cluster, and
whose history toggle read as a record dot (⊙) — an icon that said
nothing to anyone.

**Why not adopt a library wholesale** (the Obsidian route): Strop's icon
count is ~10 and constitutionally stays low (P2 — most capabilities live
in *worded* controls; P4 — words are the data). At that count a curated
plate costs an afternoon and buys voice control and per-size crispness
that no 24px-grid library gives. This is what iA Writer, Ulysses and
Bear do, and what Zed does with the very same rendering path.

**Why SVG and not the drawn-div idiom**: the divs existed to dodge the
garbled-glyph bug class — any character outside the bundled PT fonts
forces a mid-session system-font fallback load. The SVG path never
consults a font at all, so it is immune by construction — and it can
draw what stacked rectangles cannot: the one icon that matters most
(history) *requires* a curve with an arrowhead.

## The two families

One ink discipline, two formal classes that must never borrow from each
other:

| | **Pictorial** | **Window** |
|---|---|---|
| means | a *document* thing | the *window* (OS verbs) |
| forms | pictures: clock, card, headstone, chain | pure geometry: line, square, saltire |
| caps | round (humanist warmth, pairs with PT's low-contrast stems) | butt (drafting-table neutrality) |
| stroke | 1px, both families (see the pixel-honesty rule) | 1px |
| members | `history`, `menu`, `note`, `grave`, `link`, `dismiss`, `caret-down` (the one filled form) | `win-minimize`, `win-maximize`, `win-close` |

## The pixel-honesty rule

**Each icon is authored on the grid it displays at** — viewBox equals
the canonical display size, so one SVG unit is one logical pixel
(revised 2026-07-11; the first cut used a 24-unit viewBox with tuned
stroke widths and shipped blurry).

Why: gpui rasterizes an SVG at device size × 2 and paints it back at
exactly 2:1 — a phase-aligned box filter. That preserves crispness *if
and only if* the geometry's edges land on device-pixel boundaries, and
a 24-grid shown at 13px puts one grid unit at 2.17 texels — nothing
ever lands. On the display-size grid the discipline is mechanical:

- **Axis-aligned strokes are 1px wide, centered on half-integers**
  (`y="6.5"`): both edges land on device pixels at 1× and 2×.
- **Fills edge on integers or halves** (the headstone's slab).
- **Curves and diagonals float free** — anti-aliasing is correct for
  them; it is the fuzzy *double-gray bar* that reads as blur, never a
  softened circle.
- The old weight distinction between the families (2.25 vs 2.1 on the
  24 grid) could not survive pixel-honesty — a crisp stroke is 1px or
  2px, nothing between. The families now ride form and caps, which
  was always the stronger signal.
- A new display size therefore means a new drawing, not a scale
  (Octicons' per-size discipline). Current grids: 13 (titlebar,
  note, link), 12 (grave), 11 (dismiss), 9 (caret).

The separation answers a real confusion risk: custom-drawn window
controls sit in the same bar as the app's own toggles. Pictures mean the
document; bare geometry means the window. A writer who has never named
this rule still feels it — the same way she feels that "–" is not a
button about her manuscript.

## The ink rules

- **Icons carry form; color stays the element's decision.** At rest an
  icon wears `MUTED_COLOR`; active/open state wears `TEXT_COLOR`; hover
  brightens exactly as the old glyphs did. The one colored mark is the
  link chain in `LINK_COLOR` — blue = machine-side affordance, per the
  color language. No icon ever introduces a hue of its own.
- **The mark is never the only speaker** (P10 corollary, NN/g): every
  icon control keeps its tooltip with the chord chip, and counters/labels
  ride beside the marks that summarize them ("Graveyard · 2").
- **Canonical sizes**: 13px in the titlebar (matches the bar's 13px
  text), 9px for the caret wedge, 11–13px for chip marks. Prefer these;
  a new size is a design decision, not a convenience.

## What is deliberately NOT an icon

- **Format faces stay type specimens**: B, I, U, S, `==`, `{}`, H1–H3,
  and the footnote button's superior "1" are *typography about
  typography* — in a writing tool they beat any pictogram. They stay
  PT glyphs at text weight.
- **The dismiss "×" inside text surfaces stays the type's ×.** A margin
  card is a text surface (P3); its dismiss is typographic, like the
  middot separators. The drawn saltire appears only on chrome (the strip
  panel, the window). At rendered sizes the two are visually identical —
  the rule costs nothing and keeps cards all-type.
- **State dots stay dots**: the cooking/error dot, the held-AI dot, the
  checkpoint ●/○ rows, the sage goal dot. A dot is a *light*, not an
  icon; drawn divs remain the right tool.
- **The palette and menus stay text-only.** No icon column ever grows in
  the omnibar rows or the editor dropdown (P4 — the label IS the
  affordance; icon columns are how editors start looking like IDEs).
- **The strip's fabric** (flecks, wells, threads, veils, thumb) is a
  visualization, not iconography; it keeps its painted quads.
- **The door stays words** ("Reading · Away" — presence grammar, see
  ux-glossary). No icon can carry that meaning without mentoring.

## The plate

| file | depicts | sites | derivation |
|---|---|---|---|
| `history.svg` | clock swept counter-clockwise | titlebar history toggle | Lucide `history`, restroked |
| `menu.svg` | hamburger | titlebar palette toggle | Lucide `menu`, restroked |
| `caret-down.svg` | solid wedge | "Ask the editor" dropdown | ours (successor of the fused-bars ▾) |
| `note.svg` | bordered card, two text bars | narrow-notes pill | ours (the mini-card motif) |
| `grave.svg` | filled headstone on its ground | graveyard footer chip | ours (the historical slab + ground; a stroked arch reads as a bell at 12px, mass is what reads) |
| `link.svg` | chain | left-flank link cell | Lucide `link`, restroked |
| `dismiss.svg` | round-capped saltire | strip close | ours |
| `win-minimize.svg` | line | window controls (non-macOS) | ours |
| `win-maximize.svg` | outline square | 〃 | ours |
| `win-close.svg` | butt-capped saltire | 〃 | ours |

The **history clock** is the load-bearing choice: the
counter-clockwise-swept clock is the one "versions" form non-technical
users already know (Google Docs' version history, Apple Time Machine),
and it honestly signals *backwards in time* — matching the strip's
reversed-Raskin reading. The record-dot it replaces was a shipped
corridor failure (P5): nothing about ⊙ says history.

What we researched and *rejected*: the sparkle for AI (NN/g: users
don't read ✨ as AI, and the magic metaphor is anti-P2 — the editor
button keeps its words); a magnifier for the omnibar (the field already
says "Search" in words — an icon would be decoration); any icon in
palette rows (WordPress's testing showed palette icons don't help).

## Mechanics

- `crates/strop-app/src/icons.rs` — the embedded table (`include_bytes!`
  like the fonts: a missing file is a compile error), the `StropAssets`
  asset source (registered via `with_assets` in main.rs), path constants
  (a typo is a compile error), and the `icon(path, size, color)` helper.
- gpui renders the SVG into the monochrome sprite atlas and tints it
  with `text_color` — set the color **explicitly** on the icon; there is
  no text-color cascade into `svg()`. For hover brightening use
  `.group(...)` on the control and `.group_hover(...)` on the icon (see
  `window_button`).
- SVG sources: viewBox = display size (the pixel-honesty rule above),
  `stroke` kept as strokes (usvg outlines them at bake time; keeping
  them editable is the point), `#000` placeholder color (ignored —
  alpha mask). A comment at the top of each file names its family and
  derivation.
- Licensing: Lucide-derived geometry is ISC — full text in
  `assets/icons/LICENSES/LICENSE-lucide.txt`, pointer in `NOTICE`. Our
  own drawings are GPL-3.0-or-later with the project.

## Sources

Pairing icons with type: Streamline "Choosing the perfect icons for your
typeface" (serif pairings want moderate, humanist strokes; icons and
letters share construction). Small-size practice: GitHub Octicons design
guidelines (draw per size; at 16px every pixel matters); Zed's
`assets/icons` (16-grid, ~1.2px strokes, same resvg path). Icon
comprehension: NN/g "Icon Usability" (only a handful of icons are
universal; always label), NN/g on the AI-sparkle problem; Google Docs /
Time Machine precedent for the ↺-clock. Writing-app precedent: Obsidian
ships Lucide; iA Writer/Ulysses/Bear keep tiny bespoke sets subordinate
to type. Licenses: Lucide ISC (lucide.dev/license); GNU license list for
ISC/MIT/Apache-2.0 GPL-compatibility.
