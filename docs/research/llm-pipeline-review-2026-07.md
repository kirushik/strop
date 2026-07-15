# Strop's LLM pipeline — review and iteration plan (2026-07-13)

This is a code-informed review of how Strop prompts, calls, parses, and
applies LLM output as of the 0.2.0 release candidate. It accepts the
editorial intention in `docs/editorial-foundations.md` as a constraint. The
question here is whether the machinery expresses that intention reliably
across providers, models, languages, document sizes, and failure modes.

The review covers `crates/strop-core/src/llm.rs`,
`crates/strop-core/src/diagnose.rs`, the pass lifecycle in
`crates/strop-app/src/editor.rs`, relevant tests and repository history, and
current provider guidance. Provider documentation changes; external claims
and links below were checked on 2026-07-13.

## Executive judgment

The editorial prompts are unusually coherent. They define an editor's role,
protect voice, require grounded quotes, prohibit rewrites, allow silence, and
make the result usable as margin queries. Those are real strengths.

At the start of this review, the surrounding reliability layer was prototype-
grade. One generic
OpenAI-compatible request shape is sent to every endpoint; JSON is requested
only in prose; the response parser throws away completion metadata; one bad
item invalidates the whole reply; the advertised retry does not exist; the
language rule is underspecified for mixed-language text; and document scope
is silently reduced to the first 24,000 characters. Error messages therefore
guess at a cause when the program has not collected enough evidence to know
it.

The right move is not a wholesale prompt rewrite. It is to make the
contract around the prompts explicit and observable:

1. explicit, range-preserving document scope and a visible safety fuse;
2. a bounded parsing, validation, salvage, and repair ladder;
3. truthful provider evidence and privacy-safe diagnostics;
4. an explicit local language policy;
5. small provider shims and schema-constrained output where proven useful;
6. fixtures and evaluation cases that measure these contracts.

Implementation began on branch `llm-pipeline-remediation`. Commit `4c9139e`
already replaces the silent prefix, carries exact target/context scope,
salvages valid siblings, bounds one repair turn, preserves response metadata,
uses truthful failure copy, and writes a privacy-safe local log. The remaining
0.2 work is local language resolution and, after design review, partial-result
delivery during repair. Hierarchical long-document reads and model/provider
qualification remain later work.

## What exists now

The audit-start run path was compact:

```text
selection, otherwise manuscript
        |
        v
first 24,000 Unicode scalar values
        |
        v
English system prompt + "The manuscript:" + source text
        |
        v
OpenAI-compatible /chat/completions, temperature 0.3,
2,048 output tokens
        |
        v
first choice.message.content
        |
        v
substring from first '[' through last ']'
        |
        v
deserialize the entire Vec<Diagnosis>
        |
        v
exact-quote search in the current whole manuscript
        |
        v
drop unanchored items; add the rest as cards
```

The client intentionally targets Poe, OpenAI, OpenRouter, Ollama, and an
Anthropic compatibility endpoint through one request shape. It switches only
between `max_completion_tokens` and `max_tokens`, based on whether the base
URL resembles Ollama. It does not request a structured response format.

The three prompt families are:

- diagnostic, with developmental, line, and copy altitudes;
- the believing game, for precise strengths and their mechanisms;
- the doubting game, for the strongest honest case against the draft.

All require exact source quotes and JSON-array output. The diagnostic prompt
allows zero to seven items. Believing and doubting prescribe a small taxonomy
of item levels and a maximum of five.

## What is already good

### The prompts provide a stable editorial identity

The system prompts do more than ask for “feedback.” They specify role,
epistemic stance, prohibited behavior, output shape, priority order, and the
relationship between observation and authorial choice. This is close to the
right division of labor: the prompt carries editorial policy; code should
carry mechanical guarantees.

The strongest details are:

- “diagnose, never rewrite” is stated repeatedly and operationally;
- unusual voice is not itself a defect;
- every card must cite a short verbatim anchor;
- questions are framed as one editor's reading, not a verdict;
- an empty result is legitimate;
- output counts are capped, limiting margin spam;
- altitude is explicit instead of being inferred from the manuscript.

### The source-grounding design is sound

Exact quotes are a useful anti-hallucination gate. Strop refuses to create a
card when its quote cannot be found in the current manuscript. That is much
safer than trusting model-supplied offsets or placing free-floating advice.
The application also suppresses already-dismissed, overlapping diagnoses.

### Several provider failures are already distinguished

Authentication, rate limiting, provider, network, response-shape, and
diagnosis-parse failures are separate internal cases. HTTP errors embedded in
a successful OpenRouter response are checked. Error previews are safe at
Unicode boundaries. These are good foundations; the missing part is richer
evidence and actionable recovery.

## Principal reliability findings

### 1. Structured output is treated as a prompt preference

“Respond with ONLY a JSON array” improves the odds of JSON but cannot enforce
it. Models can add fences, explanations, refusals, truncated syntax, subtly
wrong field types, or valid JSON that violates Strop's semantic contract.

Current APIs can do better. OpenAI documents schema-constrained
[Structured Outputs](https://developers.openai.com/api/docs/guides/structured-outputs),
Anthropic documents
[structured outputs](https://platform.claude.com/docs/en/build-with-claude/structured-outputs),
and Ollama documents
[structured outputs](https://docs.ollama.com/capabilities/structured-outputs).
The exact request field and schema subset differ, and compatibility gateways
may ignore or reject them. Therefore the right abstraction is not “always
send `response_format`.” Keep the generic prompt-and-validate path and add only
small, proven endpoint shims for schema support. A dynamic capability framework
is not justified for 0.2.

For capable endpoints, define a schema for an object such as:

```json
{
  "items": [
    {
      "quote": "...",
      "problem": "...",
      "query": "...",
      "level": "line"
    }
  ]
}
```

An object root is preferable to a bare array because it leaves room for
versioning and metadata. Keep the runtime validator even with constrained
generation: provider claims are not a substitute for application checks.

### 2. The lenient extractor is both helpful and unsafe

`extract_json_array` takes the first `[` through the last `]`. It rescues a
common fenced-response failure, but it is not a JSON-aware scanner. Prose,
multiple arrays, bracketed citations, or a trailing example can cause it to
select an invalid or unintended span.

Use a deterministic parse ladder instead:

1. parse the whole trimmed response;
2. strip one recognized Markdown fence and parse again;
3. scan for the first complete JSON value with a real JSON parser;
4. validate the application schema and every semantic invariant;
5. if necessary, perform one constrained repair request;
6. otherwise preserve a redacted diagnostic and show an actionable failure.

Do not add increasingly imaginative local string surgery. Missing quotes,
wrong meanings, or truncated content cannot be repaired safely with commas
and brackets.

### 3. One malformed item discards every good item

The response is deserialized directly to `Vec<Diagnosis>`. A wrong type or
missing required field in one element fails the entire run. This creates the
appearance that the model “returned malformed JSON” even when six of seven
items were usable.

Parse the envelope first, then validate items independently. Accept valid
items, record rejection reasons for invalid ones, and surface a partial
success such as “5 queries added; 2 replies could not be read.” An invalid
`level` should be rejected or normalized only by an explicit pass-specific
rule. Quotes should never be fuzzily rewritten just to make them anchor.

### 4. “Autofix” should be bounded and observable

A safe automated repair path is possible:

- syntax cleanup: whole-value parsing and fence removal;
- item salvage: keep independently valid items;
- one semantic repair turn: provide the invalid response and schema, ask for
  corrected JSON only, at temperature zero or the lowest supported setting;
- final validation: run the same schema and quote checks again;
- stop after one repair attempt.

The repair turn does not need the full manuscript merely to correct JSON. If
it is asked to repair or replace an ungrounded quote, it does need the relevant
source segment; otherwise it should drop that item. This distinction avoids
letting a “format repair” silently invent new editorial content.

Transport retries are separate. Retry transient network failures, 429s, and
selected 5xx responses with bounded exponential backoff and jitter, respecting
`Retry-After`. Do not retry authentication errors or stable 4xx request
errors. The `LlmError` comments currently say rate limits are surfaced after
“our single retry,” but `chat()` performs no retry. Either implement the
policy or correct the comment.

### 5. The program cannot explain malformed replies accurately

The UI currently says a diagnosis parse failure is “usually a too-small
model.” The evidence available at that point is only a Serde error string.
Other plausible causes include:

- output truncation;
- a refusal or safety response;
- provider incompatibility;
- prose or Markdown around otherwise valid JSON;
- one item with a missing or wrong-typed field;
- prompt injection from manuscript text;
- an endpoint returning a nonstandard response shape;
- a model that followed the editorial task but not the serialization task.

The chat-response struct discards `finish_reason`, refusal fields, usage,
choice index, provider request IDs, and most raw-response structure. Those are
exactly the signals needed to distinguish truncation from refusal from schema
failure.

Capture a small, provider-neutral result record:

```text
provider and model
request shape or proven compatibility shim
HTTP status and provider request ID
latency and retry count
input/output token usage when available
finish/stop reason
response shape selected
parse stage reached
per-item validation and anchoring counts
```

Logs must redact the API key and should not retain manuscript content or raw
model output by default. A user-triggered “copy technical details” action can
include a short redacted preview and correlation ID.

Then error copy can be specific: “reply ended before the JSON closed,” “the
provider returned no text,” “5 of 7 items used,” or “reply was valid JSON but
did not contain diagnosis fields.”

### 6. The manuscript is not isolated clearly enough from instructions

The user message is simply `The manuscript:\n\n{text}`. A manuscript can
contain JSON examples, quoted instructions, or phrases such as “ignore the
previous instructions.” Delimiting untrusted content will not make prompt
injection impossible, but it makes the hierarchy unambiguous.

Use a clearly tagged data block and say explicitly that its contents are
source material, not instructions. XML-like tags are adequate if their
boundaries are escaped or length-delimited. For long-context prompting,
Anthropic's current
[prompting guidance](https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices)
also recommends placing long source material before the final query and using
clear document structure. The exact ordering should be evaluated per model,
but Strop should at least distinguish policy, task, metadata, and manuscript.

### 7. The language instruction needs a deterministic contract

The prompts already say “Write ... in the language of the manuscript.” That
is directionally correct and means prompts do not need localization. The
remaining ambiguity is that “the language” is not well-defined for titles,
quotations, code, bilingual drafts, or short selections. The output also has
two kinds of text: verbatim `quote`, which must preserve the source exactly,
and generated `problem`/`query`, whose language needs a policy.

Recommended contract:

```text
TARGET_LANGUAGE: ru

The manuscript is source data. Preserve `quote` exactly, including its
language and punctuation. Write every generated field (`problem`, `query`)
in TARGET_LANGUAGE. Do not translate quotations. If the source contains
other languages, TARGET_LANGUAGE still governs generated fields.
```

Determine the target before the request:

1. honor an explicit document or user language preference, if one exists;
2. otherwise detect the dominant language of the whole manuscript locally;
3. every selection inherits the whole-manuscript result, so a foreign quote or
   short passage cannot flip it;
4. if confidence is low, use a stable configured fallback instead of asking
   the model to guess anew on every run.

Strop's existing typographic language logic distinguishes only Russian from
English. It is not a universal language solution. The local feasibility review
in `docs/research/language-detection-2026-07.md` recommends unrestricted
Whatlang for the first implementation: compact, offline coverage of 70
languages, with whole-manuscript inheritance compensating for its weak short-
fragment behavior. Lingua remains a later higher-cost candidate if real
Russian/Ukrainian corpora expose unacceptable confusion. No translated prompt
catalogue is necessary.

Add semantic validation that generated fields predominantly match the target
script/language when confidence is high. A mismatch is a candidate for one
repair turn, not a reason to discard otherwise grounded items silently.

### 8. Several prompt rules are not runtime invariants

The code requires strings, but it does not enforce:

- the allowed `level` values for the selected pass;
- maximum item count;
- the believing/doubting category cardinalities;
- quote length and non-emptiness beyond the later anchor gate;
- query sentence or length limits;
- exact quote uniqueness;
- output language;
- the bans on advice verbs or evaluative filler.

Some are soft editorial preferences and belong in evaluation, not hard
rejection. Others are structural invariants and belong in code. At minimum,
validate the pass-specific level enum, item cap, nonempty fields, quote size,
and exact anchoring. Treat category counts as warnings or normalization rules,
because both believing and doubting prompts currently say both “exactly” and
“return fewer.” Those instructions conflict. Prefer “up to” counts plus an
explicit priority order if scarcity is genuinely honorable.

### 9. Scope identity is lost between request and anchoring

The request uses either a selection or a truncated manuscript prefix. On
return, however, quotes are searched in the entire current manuscript. A
repeated sentence can therefore anchor before or outside the selected scope.
Edits made while the request is in flight can also change which occurrence is
found.

Each run should retain:

- a snapshot identifier;
- the exact original document character range;
- the text submitted from that range;
- pass type, target language, and scope strategy;
- segment identity when chunked.

Anchor first inside the submitted snapshot and original range, then map the
range through intervening edits if possible. If the exact text no longer
exists inside that scope, mark or drop it; do not search unrelated parts of
the manuscript as a fallback.

## The 24,000-character limit

### Provenance

Repository history gives a clear origin but no quantitative rationale.
Commit `62c5e88` (2026-06-11), which introduced the LLM client and editorial
pass, added:

> Scope: selection if one else whole document (capped — a 24k-char window is
> plenty for an editorial pass).

The same commit is the only history match for `24_000` and “24k-char cap.” Its
commit message discusses selection/document scope but does not name a target
context window, tokenizer, provider limit, latency target, or cost budget. The
pre-existing roadmap only called for document/selection scope. The later
writing-lifecycle audit explicitly lists “whole-manuscript altitude” as
missing because passes cap at 24k characters and cards are span-local.

Therefore the historical answer is: there is no indication that 24,000 was
derived from a measured requirement. It was a prototype judgment. A plausible
inference is that roughly 24,000 English characters, plus a 2,048-token
completion and prompts, was intended to fit conservatively near an 8k-token
context. That is an inference, not recorded intent.

There is also no intended automatic workflow past the cap in current code.
The program silently submits the first 24,000 characters. It does not notify
the writer, choose a chapter, make an N+1 request, overlap chunks, summarize,
or aggregate results. The only way to inspect later text is to select it
manually.

### Is the threshold reasonable?

As a temporary circuit breaker, yes. As a universal document policy, no.

Characters are not the unit models limit. Token ratios vary by tokenizer,
language, script, whitespace, and content. Russian and other non-English text
can have materially different token-per-character behavior from English. A
fixed character cap therefore gives different effective budgets to different
writers—the exact opposite of the desired language consistency.

The cap is simultaneously too large and too small:

- Ollama's current
  [context-length documentation](https://docs.ollama.com/context-length)
  says its default depends on available VRAM and can be 4k tokens on machines
  below 24 GiB. A 24k-character source plus 2,048 output tokens may overflow
  that configuration.
- Current hosted models often accept far more, so the cap needlessly hides
  most of an ordinary long essay or chapter.

All request content and generated output consume the context window; this is
described explicitly in Anthropic's
[context-window documentation](https://platform.claude.com/docs/en/build-with-claude/context-windows).
Exact token counting is model-dependent; OpenAI provides a current
[token-counting guide](https://developers.openai.com/api/docs/guides/token-counting).
Its general [token explainer](https://help.openai.com/en/articles/4936856-what-are-tokens-and-how-to-count-them)
uses roughly four English characters or three-quarters of an English word per
token while warning that non-English text often has a higher token-to-character
ratio. Google's [Gemini token guide](https://ai.google.dev/gemini-api/docs/tokens)
gives a similar four-character approximation and makes clear that input and
output share the model limits. These are useful sanity checks, not universal
tokenizers.

Even when a document fits nominally, “send everything” is not automatically
best. The primary study
[Lost in the Middle](https://arxiv.org/abs/2307.03172) found that models can
use information less reliably when it appears in the middle of long context.
Long-document behavior therefore needs evaluations, not just a larger limit.

The replacement policy should budget tokens:

```text
usable source budget = model context limit
                     - system/task tokens
                     - schema and scope metadata
                     - reserved output tokens
                     - safety margin
```

Exact tokenizers and provider counting endpoints may improve later telemetry,
but they are not required for the 0.2 safety policy. Use a deliberately rough
local estimate: at least two tokens per English-like whitespace run, four per
other spaced-language run, one per Unicode scalar in a densely written script,
and a character-derived floor for unusually long runs. The useful accuracy is
the conservative factor, not a false provider-specific digit. Cut context only
at paragraph or structural boundaries and never cut the selected target.

Product decision after review: apply both a 10,000 whitespace-word ceiling and
a 40,000 estimated-source-token ceiling to TARGET plus CONTEXT. The latter
prevents an unsegmented manuscript from counting as one “word.” These are
deliberate cost/quality fuses against accidentally sending book-scale text, not
provider or model-context claims. Typical admitted English and other spaced
text estimates around 20k and 40k source tokens respectively. A 100k hosted-
model context is only a reference envelope showing substantial room for prompt,
output, and estimation error; it is neither the operating target nor a provider
guarantee. Local Ollama remains best effort even when its configured context is
smaller. Under both fuses, send the whole piece whenever the pass implies global
visibility. Over either in 0.2, visibly decline a no-selection read and ask for
a shorter selection—never take a prefix or silently substitute a local window.

### Backlog: manuscript length in the writer's own unit

The safety estimate and the writer's sense of a work's length are different
products. Preliminary publishing evidence rejects a single “non-whitespace
language” rule:

- Chinese literary calls specify `字数`, including China Writers Association
  calls that set fiction ranges in tens of thousands of characters
  ([example](https://www.chinawriter.com.cn/n1/2024/1105/c403988-40354413.html));
- Japanese fiction prizes commonly specify the equivalent number of
  400-character manuscript sheets
  ([Kawade](https://www.kawade.co.jp/smp/bungei_award.html),
  [Oita Prefecture](https://www.pref.oita.jp/soshiki/10310/bungakusyo-bosyu.html));
- Korean publishers commonly use 200-character manuscript sheets
  ([Hankyoreh Publishing](https://www.hanibook.co.kr/jsp/intro/cusnotice_view.jsp?centergb=HANIBOOK&s_menu_lcode=0000000001&s_menucd=CM&sa=v&sno=50),
  [Changbi](https://www.changbi.com/contest?type=2));
- Thai-language submission guidance may specify words
  ([Journal of Thai Studies](https://so04.tci-thaijo.org/index.php/TSDJ/authorguidelineinthai))
  or formatted pages, so orthographic spacing alone does not determine the
  convention.

Post-0.2 work should survey creative-writing submissions and writing tools per
target language, settle whether spaces and punctuation count, and test the
labels with writers. Strop can then expose a native `words | characters |
manuscript pages` work-length value based on resolved document language, with a
stable fallback and perhaps a later explicit override. That value is UI data;
the provider safety fuse remains the separate conservative token estimate.

### Is the cap supported by surrounding code?

At audit time, only mechanically. `chars().take(24_000)` was Unicode-boundary-
safe and avoided a byte-slice panic. Beyond that, support was weak:

- truncation is silent;
- the first 24k is always favored, regardless of caret or pass;
- no scope metadata reaches the model;
- no scope identity returns with the result;
- output still reserves a fixed 2,048 tokens, irrespective of provider;
- a later manual selection is anchored against the whole manuscript;
- there are no boundary, N+1, deduplication, or global-ranking tests.

Implementation note, 2026-07-13: the first remediation removed that prefix,
added an exact target/context scope and range-aware anchoring, and visibly
declines an oversized whole read. The follow-ups cap TARGET plus CONTEXT at
10,000 words and 40,000 estimated source tokens, dropping only whole neighboring
passages when either context budget is tight. The fixed output reserve and
post-0.2 hierarchical workflow remain open work.

Audit follow-up, 2026-07-14: the estimator now applies a whole-source
`characters / 4` floor, including whitespace, so the token fuse also bounds raw
request size. Paragraph context walks linearly and really includes the promised
two passages on either side. Tibetan, Bopomofo, and decomposed Hangul Jamo join
the dense-script table. The prematurely implemented semantic repair call was
removed: valid siblings land after one response, while a later repair phase
remains gated on the UX and state-machine review in `impl/11-llm-repair-flow.md`.

The cold-reading behavior does not fill this gap. For sufficiently long
manuscripts, cold read opens at the caret's current chapter. Its chapter
heuristic finds the shallowest heading level that occurs at least twice, then
the nearest such heading at or before the caret. This changes the opening page
of a whole-manuscript reading snapshot. It does not change what an LLM pass
sees, and the diagnosis path does not call it.

The heuristic may become a useful structural primitive for later AI scope.
“Where cold read opens” and “what the model receives” remain separate product
decisions; in 0.2 the former must not silently narrow the latter.

### Can the second half be sent without context?

Only for genuinely local work, and only if the prompt tells the truth about
the scope.

The current prompts repeatedly call their input “the manuscript” or “this
draft.” Sending only the second half makes the model reasonably infer that the
half is the whole. No instruction can recover omitted evidence. The effect by
pass is:

| Pass | Independent second-half reliability | Why |
| --- | --- | --- |
| Line | Often adequate with adjacent overlap | Clarity, momentum, and dead weight are mostly local; transitions and references need neighbors. |
| Copy | Partly adequate | Usage and mechanics are local; consistency of names, facts, terminology, and timeline is global. |
| Developmental | Not reliable | Structure, argument, and what the piece wants to be require the whole arc. |
| Believing | Not reliable | Center of gravity, the “alive” sentence, and repeated latent material are comparative whole-piece judgments. |
| Doubting | Not reliable | “Strongest case,” “most load-bearing,” and “weakest” are global superlatives. |

Thus a raw N+1 chunk could later produce local line observations if it is
explicitly requested and labelled `segment N+1 of M`, includes neighboring
paragraphs and section context, and prohibits whole-piece claims. That is not
the 0.2 behavior. Running the existing developmental, believing, or doubting
prompts unchanged over each chunk will produce
confident but incomparable local answers. Returning five or seven cards per
chunk would also destroy the prompts' intended scarcity.

### Recommended long-document workflow

For 0.2, prefer full text whenever it fits both product fuses. Strop targets
essays, talks, and chapters, so many normal documents fit without chunking.
Above either, visibly decline a no-selection read and retain the hard boundary
until a separately designed long-document system is ready.

After 0.2, measured model capability and pass quality can inform a movable cap.
When a document does not fit, use pass-specific strategies.

For line editing:

1. choose the selected range, or a chapter/window centered at the caret;
2. treat a selection as the target and include one or two complete paragraphs
   before and after as context; returned quotes may anchor only in the target;
3. include the containing heading and `segment N of M` metadata;
4. state that the task is local and forbid whole-manuscript conclusions;
5. preserve exact document offsets and deduplicate overlapping results.

For copy editing, use the same local pass and add a distinct global
consistency pass over a compact index of headings, names, dates, numbers, and
terminology. This keeps mechanics local without pretending consistency is.

For developmental, believing, and doubting reads, use hierarchical evidence:

1. segment at real headings/chapters, preserving source ranges;
2. produce a compact structured dossier per segment: claims, narrative moves,
   open threads, and exact candidate quotes—no final margin cards yet;
3. synthesize the dossiers, outline, opening, and ending globally to identify
   the most important questions and relevant source segments;
4. generate final cards with the global synthesis plus the relevant raw
   segments in context;
5. anchor every quote to the original snapshot, deduplicate, and rank once
   globally to the original five/seven-card cap.

This is not ordinary map/reduce summarization. Intermediate summaries can
erase voice and subtle evidence, so dossiers should retain exact quotes and
explicit uncertainty. Opening/middle/ending sampling is acceptable only as an
honestly labelled partial read, not as a substitute for global visibility.
Previous/next-chapter summaries can help a local segment understand transitions
and open threads, but they are only one dossier input. They cannot represent an
arbitrary fact, image, or arc elsewhere, and summary generation introduces its
own provenance, invalidation, and compounding-error problems.

A scope object should make all of this concrete:

```text
PassScope
  snapshot_id
  original_document_range
  submitted_segments[] { id, range, text, overlap }
  whole_document_chars/tokens
  target_language
  pass_kind
  strategy: whole | selection | local_window | hierarchical
  truncated_or_partial
```

Product decision after review: no scope control in 0.2. Scope should be
automatic—the writer wants the hole, not the drill. A quiet clarification in
the existing “Ask an editor” action copy may eventually distinguish current
chapter, whole piece, or partial read, but that belongs on the UX-research
backlog and must not block the reliability work.

## Evaluation before architectural change

Do not optimize against anecdotes alone. Build a small, versioned fixture set
that runs through request construction, stored provider replies, parsing,
validation, language checks, and anchoring without network calls.

### Deterministic contract fixtures

Include:

- valid bare JSON and fenced JSON;
- prose before/after JSON and multiple bracketed regions;
- truncated JSON with a length stop reason;
- refusal, empty choice, non-text content, and HTTP-200 error bodies;
- one malformed item among valid items;
- unknown and pass-inappropriate levels;
- over-limit counts and missing fields;
- exact, repeated, moved, edited, and hallucinated quotes;
- a selection containing text repeated outside the selection;
- Cyrillic and emoji near diagnostic preview boundaries;
- a document just below, at, and above every scope boundary.

### Language fixtures

Use at least:

- all-Russian prose;
- Russian prose with an English title or epigraph;
- English prose quoting Russian;
- short ambiguous selections inside a Russian manuscript;
- code-switching dialogue;
- non-RU/EN samples once detection supports them.

Measure exact quote preservation separately from generated-field language.
The desired result is deterministic policy, not a claim that mixed-language
writing has one objectively correct language.

### Editorial quality set

Create a small consented or synthetic corpus with expert judgments for each
pass. Evaluate:

- grounded-card rate;
- usable-card precision rather than raw card count;
- rewrite/advice violations;
- voice-as-defect violations;
- duplicate rate;
- target-language adherence;
- stability across repeated runs;
- global-pass quality by evidence position and document length;
- latency and token cost.

For long documents, place known evidence near the opening, middle, and end.
Compare full-context, local-window, and hierarchical strategies. This directly
tests the failure mode identified by long-context research and tells Strop when
chunking actually helps.

Use a fixed model/provider matrix for release qualification, but treat unknown
OpenAI-compatible endpoints as best-effort. Exact model IDs change; record the
provider-reported model and prompt/schema version with every evaluation run.

## Iteration plan

### Phase 0 — truthful failures and baselines

- Replace the too-small-model assertion with neutral, evidence-based copy.
- Preserve finish reason, usage, provider request ID, and parse stage.
- Add redacted diagnostics and fixture-based tests for current behavior.
- Record baseline format, anchor, language, latency, and failure rates.

Success criterion: every observed failure maps to a known stage; unknowns are
labelled unknown rather than guessed.

### Phase 1 — robust parsing and validation

- Introduce a versioned response envelope and pass-specific enums.
- Implement the deterministic parse ladder.
- Validate items independently and report partial success.
- Enforce hard invariants; measure soft editorial constraints.
- Add opportunistic provider-side JSON Schema and a separate bounded transport
  retry policy. Keep semantic repair deferred until its reviewed UX exists.

Success criterion: malformed presentation no longer loses valid siblings,
surplus output never causes another paid call, and transport retries are
bounded.

### Phase 2 — explicit language

- Add target-language resolution with confidence and inheritance.
- Put the resolved tag in every prompt.
- Preserve quotes exactly and validate generated fields separately.
- Add a stable low-confidence fallback and mixed-language fixtures.

Success criterion: repeated Russian runs produce Russian `problem` and `query`
fields at the agreed threshold, independent of English system prompts.

Implementation note, 2026-07-14: unrestricted Whatlang resolution now runs on
the whole manuscript and is cached by document revision. `[ai].language`
accepts `auto` or a sanitized language tag independently of the narrow
English/Russian typograph setting. The prompt names `problem` and `query`
separately so the title cannot be mistaken for schema chrome.

### Phase 3 — minimal provider shims

- Keep one generic OpenAI-compatible path and encode only proven differences
  in token fields, structured-output requests, and response metadata.
- Use native schema enforcement opportunistically; retain the same validator
  and prompt-only fallback everywhere.
- Do not build dynamic discovery, a provider framework, or a model catalogue.

Success criterion: known endpoint differences do not break the portable
contract, and unsupported capabilities degrade explicitly rather than
silently—with little provider-specific code.

### Phase 4 — correct scope

- Introduce `PassScope` and range-preserving anchoring.
- Replace the character cap with the 10,000-word and 40,000-estimated-token
  total-source safety fuses.
- Run whole-text passes under it; visibly decline unselected reads above it.
- Keep selected context to complete paragraphs inside the same source budget.
- Backlog local line/copy windows and hierarchical global passes rather than
  silently changing the meaning of the current action.
- Revisit quiet scope clarification only through the UX process.

Success criterion: text after the old 24k boundary is never silently ignored;
selected results cannot anchor outside the selection; global-pass quality does
not collapse when decisive evidence is in a later segment.

### Phase 5 — release gates and controlled tuning

- Version prompts, schemas, and scope strategies independently.
- Run the deterministic suite on every change.
- Run the editorial set for material prompt/model/scope changes.
- Compare variants on grounded usefulness, not aesthetic preference.
- Keep rollback possible for each compatibility shim.

Success criterion: a prompt or provider change ships only with measured
non-regression on format, grounding, language, and editorial usefulness.

## Priority summary

The most important defects, in order, are silent partial scope, insufficient
failure evidence, prompt-only serialization, and ambiguous target language.
The 24,000-character cap is not a designed long-document workflow; it is an
undocumented prototype guard whose first-prefix behavior is particularly
unsafe for whole-piece passes.

The smallest coherent improvement was not “increase 24,000.” It was to carry
an explicit scope and language contract from request through validation and
anchoring, while collecting enough response metadata to tell truncation,
refusal, provider incompatibility, and schema failure apart. The word/token
fuses now supply the honest 0.2 boundary. Later whole-document and overflow
strategies should be introduced only when their evidence, summary provenance,
and UI meaning can be judged with fixtures rather than intuition.
