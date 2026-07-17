# impl/16 — Book typography round (indent, lists, quotes)

*Status: ADJUDICATED 2026-07-16 — sharp-corners round decisions,
hardened by a Bringhurst/Milchin-lens review against the live engine
(bookpage.rs metrics; measure 450px @ scale 1, em = 16.5px,
line_h = 25px). Full rulings in the session record; this is the
shipping table.*

## 1. Book mode: indent replaces gap

- **`PARA_INDENT_EM` = 1.5** (24.75px at body 16.5) — indent ≈ the
  lead (Bringhurst's indent-equals-lead alternative; Milchin 1.5
  кегельных at this measure). 1em is timid against a 450px justified
  Bookman measure.
- **Body→body `para_gap` = 0 exactly.** No hair. Bonus prize: the
  25px line grid is restored — `page_height = lines × line_h`
  becomes literally true. Indent-plus-gap is a word processor's tell.
- **The heading-air trap:** `para_gap` currently doubles as heading
  air in `assemble()` (~line 1366). Split the token before zeroing:
  **`HEAD_AIR_ABOVE` = 1 × line_h (25px), `HEAD_AIR_BELOW` = 8px**
  (asymmetric — a heading belongs to the text below it; the old
  symmetric 17/17 was already a tell).
- **Indent suppression is language-conditional** (the one honest
  EN/RU divergence, keyed to the same document-language oracle as
  quotes, never per-paragraph):
  - **EN:** suppress at document start, after Heading, after
    Divider ⁂, after a writer's blank line (section openings set
    flush).
  - **RU:** suppress nothing — uniform абзацный отступ, per Russian
    book tradition. Flush-first in a Russian book reads as a Western
    import.
- **Last-line demerit** via the existing `endpoint_cost` seam: last
  line ≥ 2× indent; penalize last lines within ~1 indent of full
  measure (with gap 0, the indent and the short last line are the
  only paragraph signals — protect them).
- **Editing door: unchanged** — gap 1em, no indent. Typescript vs
  book is honest garment disagreement (the doors already disagree on
  justification and hyphenation); a block-set draft is the corridor's
  own knowledge of "working draft."
- Engine seam, planned as such: first-line-only indent means the
  first line's `avail` differs — `break_para`/`LineParams` gain a
  first-line width, not a constant tweak.

## 2. Lists

- **Book door:** inter-item spacing is subsumed by gap = 0 — a list
  sets solid, one object on the grid. **`LIST_GROUP_AIR` = line_h/2
  (12px) above and below the group** — the pair sums to one line, so
  the grid recovers below the list.
- **Editing door:** inter-item between consecutive ListItems =
  0.25 × para_gap (~4px); full para_gap frames the group.
- **Shape predicate for Paragraph blocks that look like lists**
  (spacing/flush treatment only — NEVER block-kind promotion, NEVER
  synthesized markers or hanging indents; restyling is as far as
  sniffing goes, P1):
  - Ordered: `^\d{1,2}[.)]\x20` AND an adjacent shaped sibling
    numbered n±1, ≥2 items ("1917. Всё началось…" fails twice).
  - Unordered: `^[-*•]\x20`, ≥2 consecutive. **Em/en dashes are
    excluded — radioactive:** Russian dialogue sets every reply as an
    em-dash paragraph; including — would compress it into a
    pseudo-list.
  - Shaped paragraphs additionally suppress the first-line indent
    (a marker must sit at the margin to function); flush + solid +
    group air = "a list set simply."

## 3. Quotes (typograph)

- **The empty-pair rule, split by tradition:**
  - EN doubles: after “ with nothing following → ” ("" becomes “”).
  - EN singles: after ‘ → ’ (same opens_after bug).
  - RU: **after „ → “; after « → »** (2026-07-17 REVERSAL of the
    original never-after-« ruling, product-owner adjudication on
    live use). The original panel read «„ as correct linear nesting
    («„Ирония судьбы“ вышла…»); in practice the dominant keystroke
    is the fill-in habit (`""` then arrow-left), and the «„ it
    minted has no in-flow recovery. The reversal keeps nesting
    reachable both ways: `"` typed *inside* the fresh «» sees » as
    its suffix (exempted from the empty-pair rule) and nests „; and
    with content already after the caret the suffix guard never
    fires, so the linear-nester-into-existing-text also keeps „.
    The one case that pays: typing «"Ирония… linearly at the end of
    the document now needs the extra arrow-left — judged the rarer
    habit by far.
  - The rule runs BEFORE open/close classification, and requires a
    **suffix guard**: fires only when the next char is
    end-of-paragraph, whitespace, or closing punctuation — the
    prefix-only `typograph::process` API grows a next-char/suffix
    parameter (strop-core signature change, budgeted).
- **Rejected rules** (Birman contract rule 1 — never guess): digit →
  prime ″ (quote-after-digit in prose beats inch marks by frequency);
  quote-repair before a letter (ambiguous at insertion time).
- Apostrophe branch confirmed non-colliding (runs before opening
  classification; an opening quote is not alphabetic).
- Update the enshrining fixture: `«Фильм "` → „ **stays** (it has
  content before the closer — not an empty pair); add empty-pair
  fixtures for both languages and both quote levels.
