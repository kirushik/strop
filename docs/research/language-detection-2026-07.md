# Local language detection for editor replies

Research note, 2026-07-13. This evaluates the narrow product question behind
LLM output-language consistency: can Strop resolve a manuscript's dominant
language locally, then tell an English-prompted model which language to use for
generated editorial fields? This is not a general language-identification
benchmark and does not claim that a mixed-language work has one objectively
correct language.

## Product contract

The language decision belongs before the provider request, not inside the
model's guesswork:

1. an explicit document language wins;
2. otherwise detect the whole manuscript, not the selected passage;
3. every passage inherits that result;
4. `quote` remains exact source text in whatever language it uses;
5. generated `problem` and `query` fields use the resolved language;
6. a short or uncertain manuscript uses one stable fallback rather than
   oscillating between calls.

Whole-document detection is important. A quotation, character name, or short
selection can be in a different language from the surrounding work. It is also
the designed answer to inherently ambiguous fragments such as `Мама там.`.

## Candidates

The practical Rust candidates were:

- [Whatlang](https://github.com/greyblake/whatlang-rs), MIT, pure Rust,
  70 languages, one compact trigram model, with confidence and a reliability
  signal;
- [Lingua](https://github.com/pemistahl/lingua-rs), Apache-2.0, pure Rust,
  75 languages, language-selectable models, confidence values, and an explicit
  focus on short text;
- [Whichlang](https://github.com/quickwit-oss/whichlang), which is compact but
  does not support Ukrainian;
- CLD3 bindings, which add C++ and protobuf build requirements that are hard to
  justify for this release.

Lingua's language models can be selected as Cargo features, but enabling all
languages is documented at roughly 300 MB. A release build with only English,
Russian, and Ukrainian was still about 18 MB after stripping in this VM. The
equivalent unrestricted Whatlang probe was about 606 KB. Restricting production
detection to three languages is not acceptable: French, Polish, or any other
supported manuscript would then be confidently forced into the wrong class.

## Small local bakeoff

A temporary Cargo program exercised both libraries on 29 hand-written samples:
English, Russian, and Ukrainian narrative and expository paragraphs; foreign
quotations; language-neutral names; mixed-language dialogue; and deliberately
short or ambiguous fragments. Twenty-five samples had a declared answer.

| Detector | Configuration | Correct |
| --- | --- | ---: |
| Lingua 1.8.0 | English, Russian, Ukrainian | 24/25 |
| Whatlang 0.18.0 | English, Russian, Ukrainian allowlist | 24/25 |
| Whatlang 0.18.0 | all supported languages | 23/25 |

The unrestricted errors were short Russian fragments classified as Bulgarian;
one was `Мама дома.`, and Whatlang marked the result unreliable. All ordinary
paragraphs, dominant-language mixed samples, and foreign-quotation samples were
correct. An extra smoke set correctly identified generated French, German,
Spanish, Polish, and Italian paragraphs. Lingua's own published comparison also
shows that all detectors weaken sharply on single words and word pairs, while
its high-accuracy mode and Whatlang both perform much better on sentences and
longer text. That table is vendor-published evidence, not an independent test.

The probe is intentionally modest. Its samples were generated for this review,
not drawn from consenting Strop manuscripts; 24/25 is not a release-quality
accuracy estimate. It establishes feasibility and exposes the relevant failure
shape. A thousand repeated detections took about 0.19 seconds with the
three-language Lingua build and about 0.01 seconds with Whatlang on this VM, so
neither is an interaction-latency concern when run once per manuscript change.

## Recommendation for 0.2

Whatlang is the proportionate first implementation. It covers the broad
language set a writing tool needs at a small binary cost, is local and
deterministic, and performed correctly on the long Russian/Ukrainian cases that
matter to this policy. Lingua remains the higher-cost candidate if real user
corpora show that Whatlang confuses closely related languages at unacceptable
rates.

Do not gate solely on Whatlang's `is_reliable()`. Several ordinary Russian
paragraphs in the probe had the correct top result but an unreliable flag. Also
do not invent a confidence threshold from 29 synthetic examples. For the first
implementation:

- detect the whole non-empty manuscript with unrestricted Whatlang;
- cache the result by manuscript revision and make selections inherit it;
- accept an explicit language setting without detection;
- retain a stable fallback for empty or very short material;
- log the resolved code, confidence/reliability, and fallback reason, never
  sample text;
- validate the generated fields only as a repair hint: a confident mismatch
  may trigger the existing single repair round, but an uncertain check must not
  discard grounded cards.

The precise short-text cutoff and fallback language need a corpus-backed test,
not another model call. Until then consistency is more important than guessing
an eight-word draft correctly. A small multilingual fixture suite plus opt-in,
redacted tester reports should decide that threshold before it becomes a hard
contract.

Implementation note, 2026-07-14: the unrestricted detector now resolves the
whole manuscript and is cached by document revision plus configured override.
`[ai].language` accepts `auto` or a sanitized language tag independently of the
top-level English/Russian typograph setting. The provisional deterministic
boundary is twenty alphabetic characters; shorter material falls back to
English unless explicitly tagged. Diagnostics record only the code, source,
confidence, and reliability. Corpus-backed adjustment of that boundary remains
evaluation work.

## Release evaluation

Build fixtures at three scales: shared/ambiguous phrases, 1–3 ordinary
paragraphs, and 1,000–10,000-word manuscripts. Include Russian and Ukrainian
without distinctive letters, borrowed vocabulary, English/Cyrillic quotations,
dialogue that code-switches, headings in another language, and equal bilingual
drafts. Record the intended product language separately from linguistic purity.

The release gate is not a headline classifier score. It is:

- repeated passes never change language on an unchanged manuscript;
- an unambiguous Russian or Ukrainian manuscript resolves correctly;
- a passage quotation cannot flip the inherited language;
- a detection or validation failure never loses an otherwise grounded quote;
- diagnostics reveal the decision without revealing the work.
