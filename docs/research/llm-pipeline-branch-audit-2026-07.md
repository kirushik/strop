# LLM pipeline branch audit

Audit date: 2026-07-13. Audited branch:
`llm-pipeline-remediation`, at `a8c14a4`.

No—the mixed-language title is a real bug, and the second agent's review is
substantially correct. I would not merge the branch in its present form.

## Remediation status (2026-07-14)

The branch surgery below has now been applied. The speculative semantic repair
turn is removed; valid first-round siblings are delivered without a second
request. CI lint defects, credential redaction, known-object parsing, scope
fuses, paragraph context, dense scripts, transport retry, request evidence,
and opportunistic structured output are corrected and covered by regression
tests.

AI reply language is now independent of the top-level English/Russian
typograph setting. `[ai].language` accepts `auto` or any sanitized language
tag. Auto detection runs locally over the whole manuscript with unrestricted
Whatlang, is cached by document revision, and is inherited by selections. Both
the prompt and structured schema explicitly require the card title (`problem`)
and body (`query`) in that resolved language. This prevents the old literal
“dominant language of TARGET” guess from being made anew by every provider
request; provider adherence still needs corpus and live-model evaluation.

The repair design remains deferred until its separate UX proposal is approved
and it can return with immediate partial delivery, a remaining-cap invariant,
no manuscript resend, and state-machine tests.

## Why the card title stays English

The model returns:

- `problem` → card title;
- `query` → card body.

That mapping happens in `crates/strop-core/src/diagnose.rs`. Both prompts ask
for the manuscript language, but `Auto` currently sends the literal instruction
`the dominant language of TARGET` from `crates/strop-app/src/editor.rs`. There
is no local language detection and no language validation.

Consequently, a model can produce:

```json
{
  "problem": "Buried lead",
  "query": "Почему главное появляется только в третьем абзаце?"
}
```

The parser sees two non-empty strings and accepts it. The title/body discrepancy
observed during testing is therefore expected from the current implementation,
not something introduced by card rendering.

The intended fix remains correct: resolve the whole manuscript's language
locally and once, pass an explicit code such as `ru` or `uk`, let selections
inherit it, and fail consistently on ambiguity. English prompts do not need
localization.

The current `Language::{Auto, Ru, En}` also cannot represent an explicit
Ukrainian choice. Automatic Ukrainian detection could still return `uk`, so
that enum alone does not make automatic Ukrainian impossible, but the
explicit-language side needs a more general representation eventually.

## Findings confirmed

| Report | Verdict |
| --- | --- |
| CI blocked by two Clippy errors | Confirmed. The exact CI command exits 101 at `diagnose.rs` lines 323 and 340. |
| Surplus valid items trigger repair | Confirmed. Items rejected only because the cap was already reached increment `rejected`, which starts repair. |
| Repair resends the manuscript | Confirmed in `repair_user_prompt`. |
| Repair can exceed the item cap | Confirmed. The merge has no remaining-cap check and deduplicates only exact quote + problem pairs. |
| Repair uses temperature 0.3 | Confirmed in `llm.rs`. |
| Length-truncated output is repaired with the same budget | Confirmed. `finish_reason` decorates an error but does not control repair. |
| Refusal is logged as `diagnosis_contract` | Confirmed. It becomes `AiFailure::Parse`. |
| Valid first-round cards wait for repair | Confirmed. Both calls occur inside one blocking background closure before delivery. |
| No repair-orchestration tests | Confirmed. Tests cover core parsing and anchoring, not the app-side merge/state machine. |
| Retry omits 5xx and `Retry-After` | Confirmed. Only network errors and 429s retry after a fixed 300 ms. |
| Provider credentials can enter diagnostics | Confirmed. `https://user:secret@host/v1` becomes `user:secret@host`. |
| Object-wrapped diagnosis arrays fail | Reproduced: `{"diagnoses":[...]}` returns `JSON response root is not an array`. |
| Whitespace bypasses both fuses | Reproduced: a 4,000,010-character target containing four million newlines was accepted as two words. |
| “After” context is one paragraph short | Reproduced. Requesting two produced `"\nafter one\n"`. |
| Context calculation is quadratic | Confirmed structurally: it repeatedly recounts progressively longer prefixes. I did not independently repeat the quoted timings. |
| Tibetan, Bopomofo, and decomposed Hangul are absent | Confirmed from the dense-script ranges. |

One additional incompleteness: the client preserves a provider request ID, but
the diagnostics discard it, and latency is not recorded at all despite delivery
item 1 asking for both.

The earlier 408-test result is still meaningful, but it does not make CI green:
CI runs Clippy first and never reaches the tests.

## Structured output

The strategic recommendation is sound, with updated provider nuance:

- OpenAI supports strict JSON Schema structured output.
- OpenRouter supports `response_format: json_schema` for compatible models and
  has optional response healing. Its own documentation notes that healing
  cannot repair truncation
  ([structured outputs](https://openrouter.ai/docs/guides/features/structured-outputs),
  [response healing](https://openrouter.ai/docs/guides/features/plugins/response-healing)).
- Current Ollama documentation says its OpenAI-compatible chat endpoint accepts
  `response_format`; the reported compatibility gap appears outdated
  ([structured outputs](https://docs.ollama.com/capabilities/structured-outputs),
  [OpenAI compatibility](https://docs.ollama.com/api/openai-compatibility)).
- Poe Chat Completions explicitly ignores `response_format`, although its newer
  Responses endpoint supports schemas through `text.format`
  ([Poe compatibility documentation](https://creator.poe.com/docs/external-applications/openai-compatible-api)).

So the right design is still “opportunistic structured output, always
validate,” with a few small host shims—not a provider-capability architecture.

Object envelopes also need first-class parser support because
schema-constrained APIs commonly work most naturally with an object like
`{"diagnoses":[...]}`.

## Recommended branch surgery

I agree with cutting the repair round. My order would be:

### 1. Stabilize delivery items 1–3

- Remove the second-call repair orchestration and unused repair prompts.
- Keep independent item salvage and immediately deliver valid cards.
- Fix the two Clippy failures.
- Strip URL userinfo from diagnostic provider names and add the missing
  credential test.
- Accept a known `diagnoses` object envelope without weakening the one-array
  parser.
- Include total source characters in the estimator, for example a global
  `total_chars / 4` floor. That closes the whitespace hole while bounding raw
  request size.
- Correct and linearize both paragraph-context walkers.
- Add Tibetan, Bopomofo, and Hangul Jamo ranges.
- Add regression tests for each case.

### 2. Implement deterministic language resolution before 0.2

- Detect the whole manuscript locally and cache the result by revision.
- Let an explicit language win.
- Make selections inherit the manuscript result.
- Use a stable fallback for short or ambiguous manuscripts.
- Log only resolved code and confidence metadata.
- Test both `problem` and `query` on repeated Russian and Ukrainian runs.

### 3. Add structured-output and retry shims

Add minimal opportunistic structured-output shims and truthful transport retry
behavior. Continue validating every reply locally because structured output is
not universal and does not establish exact quote grounding.

### 4. Reintroduce repair after UX review

The later repair implementation should have:

- immediate delivery of first-round valid cards;
- temperature zero;
- remaining-cap enforcement;
- no repair for surplus items, refusals, or length truncation;
- no whole-manuscript resend for serialization repair;
- explicit phase and state-machine tests.

## Verification performed for this audit

- The required core Clippy command was run and exited 101 with the two reported
  `clippy::filter_map_bool_then` failures.
- A disposable Cargo example reproduced the object-envelope rejection,
  whitespace-fuse bypass, and short after-context behavior; it exited 0.
- The disposable example and cold-build artifacts were removed afterward.
- No product-code changes were retained as part of the audit.
