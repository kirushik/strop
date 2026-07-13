# 10 — LLM pipeline remediation

Implementation brief, 2026-07-13. The research and code audit lives in
`docs/research/llm-pipeline-review-2026-07.md`. This brief records the product
decisions made after that review and turns them into ordered, testable work.

## 0 · Constraints

- The editorial intention of the prompts is fixed. Improve the machinery
  around them; do not turn diagnosis into rewriting.
- Poe, OpenAI, OpenRouter, Anthropic-compatible endpoints, and Ollama are
  best-effort providers in 0.2. Arbitrary OpenAI-compatible endpoints remain
  best effort too. A future Strop endpoint will carry the strong contract.
- Provider handling stays one generic path plus small, proven compatibility
  shims. No capability-discovery framework, model catalogue, or provider
  abstraction project.
- Local Ollama models may be too small or have too little context. Strop may
  try; it must report what actually failed rather than blaming model size by
  default.
- There is no scope control in 0.2. Writers want the result, not an AI console
  (P2, P4, P5). A quiet scope clarification in the editor menu is a later UX
  research item, not part of this implementation.
- The AI should recover automatically where recovery is bounded and safe.
  Valid first-round cards may land while a repair consultation continues.
- Diagnostics are local and manually submitted. An exported support log must
  not contain manuscript text, exact quotes, request bodies, API keys, or raw
  completions unless the writer deliberately opts into sharing those in a
  support conversation.

## 1 · Response evidence and truthful failures

Preserve a provider-neutral completion record: content, finish reason,
refusal when exposed, token usage when exposed, latency, retry count, and a
provider request identifier when exposed. A response-shape failure and a
diagnosis-contract failure remain distinct.

Replace “usually a too-small model” with evidence-based copy. Length stops,
empty replies, refusals, invalid JSON, wrong diagnosis fields, and unanchored
quotes must be distinguishable where the provider supplies enough evidence.

Add a separate local diagnostic log. The durable document journal may later
record provider, model, pass, and accepted/rejected counts after its
backward-compatibility implications are specified; it must never store the
manuscript or raw completion.

Acceptance:

- recorded details contain no request authorization or manuscript text;
- an unknown failure is called unknown, never guessed;
- existing provider/auth/rate-limit error behavior remains intact.

## 2 · Deterministic parsing, salvage, and recovery

Replace first-`[`/last-`]` extraction with a bounded ladder:

1. parse the trimmed response as JSON;
2. strip one recognised Markdown JSON fence and parse;
3. scan for the first complete JSON array using JSON parsing, not bracket
   slicing;
4. validate the envelope and each diagnosis independently;
5. keep valid items and report rejected-item reasons;
6. make at most one repair request for rejected/malformed material;
7. validate the repaired reply through the same path and stop.

Hard invariants are pass-specific item caps, allowed levels, non-empty
fields, quote length, and exact anchoring within the target. Prompt-level
editorial preferences remain evaluation criteria, not brittle parsers.

Valid first-round items land as soon as the first reply is validated. If a
repair is needed, the existing running pulse remains visible with a restrained
consultation label; repaired items join the same logical pass. Repair may fix
serialization or generated-field language. It must not invent a replacement
quote without receiving the relevant target source.

Transport recovery is separate: one bounded retry for rate limits, selected
5xx responses, and transient network failure, respecting `Retry-After` when
available. Authentication and stable request errors are never retried.

Acceptance:

- one malformed item cannot discard valid siblings;
- fences/prose around one JSON value recover deterministically;
- multiple arrays or truncated JSON cannot be spliced into invented JSON;
- recovery attempts are capped and visible through the existing progress
  state;
- no recovered card can escape exact target anchoring.

## 3 · Deterministic output language

Resolve one target language before the request. Generated `problem` and
`query` fields use it; `quote` remains byte-for-byte source text.

Policy:

1. an explicit document language wins;
2. otherwise detect the dominant language of the whole manuscript locally;
3. a short or ambiguous selection inherits the whole-manuscript result;
4. low confidence uses a stable fallback, never a fresh model guess.

The local bakeoff in `docs/research/language-detection-2026-07.md` recommends
unrestricted `whatlang` for the first implementation: it covers 70 languages
at a small binary cost and handled ordinary Russian/Ukrainian paragraphs well.
`lingua-rs` remains a fallback candidate if real corpora show unacceptable
closely-related-language errors. Do not restrict detection to English,
Russian, and Ukrainian, and do not make a vendor reliability flag the entire
policy. No detector is expected to classify a shared two-word phrase;
whole-manuscript inheritance is the designed answer.

Acceptance:

- English prompts need no localisation;
- repeated Russian and Ukrainian runs keep generated fields in the resolved
  language;
- foreign quotations do not flip the document language;
- ambiguous selections inherit rather than oscillate.

## 4 · Scope identity and the 10k safety ceiling

Replace the silent 24,000-character prefix with an explicit internal scope
object carrying the source snapshot, exact target document range, context
range, pass kind, language, and whole/selection/local strategy.

Ten thousand words is the initial per-request manuscript-source ceiling,
including TARGET plus all CONTEXT and an explicit select-all. It is a product
cost/quality guard, not a claim about model context. Keep it as one named
constant so evaluation can move it. Never submit a partial prefix or paragraph
and call it the manuscript.

When the writer selects a passage, the selection is the target. Add up to two
complete paragraphs before and after it as read-only context, as the remaining
source budget allows, plus the containing heading where practical. Never trim
the selection to make room for context. The model may read context but may
return quotes only from the target. Validation and anchoring enforce that
boundary.

Without a selection:

- at or below 10k words, whole-piece passes receive the whole manuscript;
- above 10k, every 0.2 pass declines the whole-piece read and asks for a
  shorter explicit selection. No pass silently chooses a caret or chapter
  window because the existing unselected action means a whole-piece read.

For a conservative sanity estimate, budget submitted source as two tokens per
English word and four per non-English word, then leave prompt, output, and
safety reserve. At 10k words this estimates at most 20k or 40k source tokens,
well below the 100k-token planning baseline used for hosted models. This is a
cost fuse and rough forecast, not a tokenizer or a provider guarantee. Small
local Ollama contexts remain best effort and may fail truthfully.

Scope stays automatic in 0.2. A quiet whole-piece/current-chapter indication
inside existing “Ask an editor” action copy is backlog work requiring UX tests.

Acceptance:

- text after character 24,000 is included when the whole piece is eligible;
- oversized manuscripts are never silently truncated;
- a selected result cannot anchor to repeated text outside the selection;
- context can inform a card but cannot become its anchor;
- edits made while a request is in flight cannot move a card to unrelated
  text.

## 5 · Long-document strategies

Do not ship long-document chunking in 0.2. A local window would be honest for
explicitly local line work, but the current no-selection action promises a
whole read. Neighboring chapter summaries improve transitions yet cannot
recover facts, arcs, contrasts, or global superlatives elsewhere in the work.

The post-0.2 direction is not uniform chunking:

- Line: chapter/caret window, paragraph boundaries, one-to-two-paragraph
  overlap, local claims only.
- Copy: the same local mechanics, with global consistency deferred to a
  distinct indexed pass.
- Developmental/believing/doubting: hierarchical evidence dossiers, global
  synthesis, then final card generation from the relevant raw segments.

Intermediate dossiers preserve exact candidate quotes and uncertainty. Final
cards are ranked once across the piece to the original five/seven-card limit;
never emit a full allowance per chunk. This phase follows, rather than blocks,
the 0.2 safety ceiling. It also needs a summary/evidence cache, invalidation on
edits, structural boundaries, source provenance, and retrieval of raw passages
before final card generation. Adjacent-chapter summaries are one input to that
system, not a substitute for it.

## 6 · Provider shims

Keep this phase deliberately small:

- retain the generic OpenAI-compatible chat-completions path;
- preserve only observed request/response differences;
- use schema-constrained output opportunistically where a known endpoint
  supports the exact request;
- fall back immediately to the prompt-and-validate path;
- do not infer model quality or context from model names.

The parser, validator, language contract, scope rules, and diagnostics are the
portable guarantees. Provider-native structured output is an optimisation.

## 7 · Evaluation and release gates

Add deterministic fixtures before live provider testing: bare/fenced/noisy/
truncated JSON, refusals, empty choices, one bad item among good siblings,
wrong levels, repeated quotes, selection collisions, and Russian/Ukrainian/
mixed-language cases.

Later in the product lifecycle, qualify a small rotating model set through Poe
and selected direct/local endpoints. A short-lived, low-quota key lives outside
the repository and never enters chat, fixtures, logs, or commits. This is not a
0.2 provider-profile framework or release blocker.

Measure grounded-card rate, usable-card precision, duplicate rate, target-
language adherence, format/recovery rate, latency, cost, and global-pass
quality with decisive evidence at the opening, middle, and end.

## 8 · Delivery order

1. Truthful errors and response metadata.
2. Robust parsing, partial salvage, and bounded retry/recovery.
3. Range-preserving scope, neighboring passage context, and the 10k-word
   total-source safety ceiling.
4. Deterministic local language detection and enforcement.
5. Partial-result repair delivery after UX review in
   `docs/impl/11-llm-repair-flow.md`.
6. Minimal provider shims and opportunistic structured output.
7. Post-0.2 hierarchical global reads.
8. Evaluation gates and controlled prompt/model tuning.

Dependency-light work comes first. The language detector and any provider-
specific additions each require their own explicit dependency/compatibility
review; neither may turn into an architecture project.
