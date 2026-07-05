# The UX glossary — what words reach the chrome

*(Round 4 opened this: our vocabulary was born in design sessions
steeped in English-language craft literature, and it shows. The test
for every term below is a concrete persona: **a writer drafting a blog
post in Russian, who has never read a book on creative writing.** If a
term fails her, it either stays internal, gets a carrier sentence, or
gets a plainer word. Governed by design-principles.md P4 (words are
data or actionable labels) and P8 (grammar).)*

## The three registers

1. **Internal vocabulary** — spec and codebase words. Never printed in
   the UI. It is not a failure for these to be jargon; it is a failure
   for them to leak.
2. **Carried terms** — craft words the UI may use, but only inside a
   carrier sentence that teaches them in place ("Ask the editor for a
   **line read**"). Never bare on a badge or button.
3. **Plain labels** — words that pass the persona test naked.

## The audit

| term | register | verdict |
|---|---|---|
| **the editor** | plain | The word that sets the whole relationship; translates perfectly (редактор). Never "AI", never "assistant", never a product-name mascot. Standing rule: **the product never calls itself an editor in any UI string** — Strop *is* a текстовый редактор in both languages, and the word belongs to the person. |
| **a read** | carried | Fine inside sentences: "Ask the editor for a read", "a read is ready". Never bare, and never numeral+noun ("3 reads" on a badge is arithmetic wearing a fig leaf; the cards in the margin are the count). Sentence forms only. |
| **the door** | internal | The concept name stays in specs. The UI shows the *state* on the editor control itself (P12), in **presence grammar** — the pair every messenger already taught the corridor: **"Reading" / "Away"**, RU **«Читает» / «Не смотрит»** (parallel, gender-free; the rejected "Working alone" both mixed action-and-state kinds and forced Russian to gender the writer). Presence also quietly states P2's promise: someone who is merely *present* never initiates. |
| **seal** | internal | Round 4 flagged it, correctly. The writer's act is naming a checkpoint in her own words ("Draft complete"); the gate it opens is machinery. No UI string says "seal". |
| **pass** (believing/line/copy) | internal | Already displaced by "read" in UI sentences. |
| **checkpoint / station** | internal | The strip shows *names*, not the category. Where a category noun is unavoidable (palette verb), use **"version"** — the one history word every tool taught everyone (Восстановить версию). Verb: "Name this version". |
| **cold read** | carried | Keep — but localization translates the *function*, not the metaphor ("холодное чтение" means a mentalist's trick in Russian; «свежим взглядом» carries the function). Rule for all metaphor terms: function over metaphor at the language border. |
| **compost / graveyard** | carried | Strong, warm, self-deprecating — and they label *sections of the writer's own text*, so they sit next to their contents, which teaches them. **Open item: the Russian pair is untested.** «Компост» has no craft resonance (purely agricultural — comedy risk); «кладбище» lands heavier than "graveyard" (no "kill your darlings" wryness to soften it). Corridor-test both in Russian; hold functional fallbacks ready («вырезанное» — plainly "what was cut"). Our own rule — function over metaphor at the border — may come for our favorites. |
| **aside / set aside** | carried | The *verb* translates cleanly (отложить) and is the only form the UI uses. The noun stays internal — Russian has no scrap-noun for "an aside" («реплика в сторону» is theater); the pile the verb feeds is named by "compost". |
| **margin note** | plain | — |
| **carry this forward** | — | Dead. The control is **"Restore"** (v2 strip): the common word, honored-plus — our restore appends and destroys nothing. |
| **put back / Restore** | plain | Two verbs, lawful under P8 because the objects differ: **Put back** returns a *fragment to its place* (graveyard entries and the post-exile affordance — which formerly said "undo"; one action, one verb); **Restore** brings back a *whole document state* (the strip). Fixed Russian now, before translators improvise: **Put back = «Вернуть»**, **Restore = «Восстановить»**. |
| **unanchored / orphaned** | forbidden | System taxonomy that frightens. The behavior (note → compost with quoted anchor) needs no name at all. |
| **reveal / recede / lane / altitude / envelope / fleck / veil / thread / seam** | internal | Fabric and motion vocabulary; specs only. |

## Standing rules extracted

- **A metaphor may live in the UI only where its referent is visible**
  (graveyard labels the graveyard; door labels nothing visible → door
  stays home).
- **Localization translates function, not metaphor.** Every carried
  term ships with a function gloss for translators.
- **No bare jargon on badges, counters, or buttons** — carriers only.
- The glossary is a gate: a new UI string containing a new term needs a
  row here first.

## Appendix: hyphenation languages for the cold read

The justified, book-honest cold read needs Knuth–Liang hyphenation.
Verified state of the Rust ecosystem (2026-07):

- **The `hyphenation` crate covers 80 language variants** — Russian
  and Ukrainian included, German in all three forms (1901, 1996,
  Swiss), GB/US English split, Greek, Church Slavonic… Code is
  MIT/Apache-2.0; dormant since 2021 but patterns don't rot. The
  alternative, Typst's actively-maintained `hypher` (48 languages,
  no_std, zero-alloc, ~1.1 MiB all-in), covers Russian too but only
  reform-1996 German and a single English.
- **Russian quality is first-class**: both crates ship Alexander
  Lebedev's `ruhyphen` patterns (patgen-trained on a 990k-word list,
  hand-corrected) — the de-facto Russian TeX standard for 20+ years.
- **One licensing note**: the Russian pattern *data* is LPPL-only
  (FSF: free, GPL-incompatible for code; gray for inert data). The
  clean posture for our GPL-3.0-or-later binary is loading the ru
  dictionary at runtime as a data file — which `hyphenation` supports
  natively and `hypher` (compile-time FSAs) does not. That plus the
  German-variant coverage decides it: **`hyphenation`, with runtime
  dictionaries.**
- **Fundamental limits** (not crate gaps): CJK doesn't hyphenate —
  line breaking is per-character with kinsoku prohibition rules
  (UAX #14 territory, which justification must handle separately);
  Thai/Lao/Khmer need dictionary word-segmentation, not patterns;
  Arabic justifies by kashida elongation, not hyphens. So: the
  justified cold read with hyphens serves the alphabetic world —
  Russian emphatically included; CJK justification is a different,
  later engine feature.
