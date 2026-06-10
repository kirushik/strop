# AI Writer's-Editor — Project Handoff / Context Carryover

> **Purpose of this file.** This is a Claude→Claude handoff, not a human deliverable. It exists so a fresh Claude Cowork session can resume work on Kirill's "AI editor for writers" project without re-deriving scope, re-running settled reasoning, or repeating the research already done. Optimize for fidelity and actionability over polish. The companion deliverable is the synthesized research report (artifact, titled "Research Report" / id `compass_artifact_wf-0e6db338-...`); **do not duplicate it — read it first, then use this doc for the deltas: decisions, untapped leads, and open forks.**

---

## 0. The builder & working contract (calibrate to this)

- **Who:** Kirill — technically strong, founder/CTO type, Berlin-based. Strong on **HCI/UX**; explicitly wants depth on the *more fundamental* layers (editorial theory > human process > HCI) because he can already self-supply the HCI layer.
- **The project:** A "writer's editor with AI editor" — a **hobby project**. Influenced by **lex.page**, which he considers the best existing take, but believes can be beaten decisively **"if designed from solid foundations up."** Central obsession: **avoiding LLM-driven voice homogenization** ("averaging-out the writer's talent and unique style").
- **Working style (apply by default):** Concise. Lead with **counterarguments**, not agreement. Give **opinionated, concrete recommendations**, not hedged option-lists. **Steelman opposing views honestly; don't soften or walk them back.** Push back as a knowledgeable expert when he pushes on a defensible position. **No compliments** (he finds them noise). He pushes back effectively on unsupported claims — expect to defend yours.

---

## 1. Scope — DECIDED (do not relitigate without cause)

**In scope (editorial domains):**
- Essays (literary + argumentative)
- **Opinionated journalism** / op-eds / commentary / criticism (a core domain, currently *under-served* by the research — see §3)
- Short fiction — **lesser** priority

**Out of scope (explicitly, with rationale):**
- **Commercial / marketing copy** — Kirill: driven by "explicit-to-the-level-of-dumbness formal styleguides and policies." Not interesting; don't pull marketing-copy tools (Jasper, Copy.ai) as positive comparanda.
- **Novel-length fiction** — Kirill: "requires a completely different kind of writer's support." Treat novel-craft tooling (Sudowrite Story Bible, Dramatron, etc.) as out-of-domain except as *negative* comparanda / paradigm sources.

**Research weighting (his explicit ask):** theory > process > HCI. **Most valuable = prior art at the *intersections* of those layers.** He'll adapt findings down into the HCI layer himself.

---

## 2. What's already concluded (in the report — reference, don't rebuild)

The report's load-bearing conclusions, so you know what's settled:

1. **Diagnosis-over-prescription is the design north star.** Gaiman's rule (readers are *right that something's wrong, wrong about the fix*) + the editorial "would I write it that way?" guardrail. Default surface = *named problems as queries*; rewrites are secondary, opt-in, diff-gated.
2. **Levels-of-edit (developmental / line / copy / proof) → tool MODES.** Claimed as a likely *open gap* in HCI prior art and the defensible product architecture. (Closest existing adjacent work: **Writor**, which maps writing-center pedagogy → design guidelines. Verify Writor's specifics against primary text — CHI 2026 / In2Writing@ACL2025, arXiv 2602.04047 — before relying on the "7 guidelines" claim.)
3. **Voice preservation as a *measurable* property,** not a vibe. Homogenization is empirically real and **RLHF-specific**: Padmakumar & He (ICLR 2024) found InstructGPT — *not* base GPT-3 — caused statistically significant diversity loss; Agarwal/Naaman/Vashistha (CHI 2025) found AI suggestions homogenize toward Western styles. Implication: it's partly a *model-choice + prompting + accept-friction* problem, not destiny.
4. **Calibrate intervention to writer skill/intent** (Sommers: novices word-swap, experts revise recursively/globally; Gottlieb edits a master with "the slightest hint"). Adaptive intervention level.

Four-stage recommendation in the report: (1) diagnostic-first editor → (2) levels-of-edit mode switch → (3) voice-preservation as regression test → (4) skill/intent calibration. **The pivot condition is stated there: if voice-preservation testing fails even with guardrails, drop generation entirely and ship pure diagnosis.**

---

## 3. UNTAPPED LEADS — named in the research brief but thin/absent in the report

These were in the original research plan and either didn't surface or got compressed. Each is a concrete thread to pull in Cowork. Roughly ordered by expected payoff.

### 3a. Journalism craft (HIGH — it's a core domain and the report under-covers it)
The report leans literary-essay. Op-ed/journalism editing has its own diagnostic vocabulary that should feed the diagnostic engine:
- **Structural primitives:** the *lede* (and "burying the lede"), the *nut graf* (the "why this matters / why now" paragraph), the *inverted pyramid*, the *kicker*. These are nameable, detectable structural problems — strong candidates for *developmental-mode* checks specific to journalism.
- **Op-ed form specifically:** Trish Hall (ex-NYT op-ed editor), *Writing an Op-Ed* / "Op-Ed and You" guidance; the "one idea, argued hard" constraint; the obligation to *concede and rebut*. Op-eds have a tighter rhetorical shape than essays — possibly a distinct mode/template.
- **Institutions/resources:** ACES (American Copy Editors Society), Poynter (Roy Peter Clark lives here — the report has his books but not the journalism-editing angle). Poynter's "writing tools" are journalism-first.
- **Open question this raises:** do essay / op-ed / short-fiction share ONE diagnostic engine with mode-specific rulesets, or are they different enough to need separate analyzers? (See §4 fork.)

### 3b. Editors named but missing/thin in the report
- **Maxwell Perkins** — the canonical "enabler" editor (Wolfe, Fitzgerald, Hemingway). Pairs with Lish as the *other pole* of intervention: Perkins as cutter-in-service-of-the-author vs. Lish as cutter-as-co-author. Useful framing for an "intervention intensity" axis in the UI.
- **Benjamin Dreyer, *Dreyer's English*** — copyediting *with personality*; "the stuff you only notice when it's done wrong." Good source for the copy/proof modes' tone (queries with wit, not robotic flags).
- **Renni Browne & Dave King, *Self-Editing for Fiction Writers*** — short-fiction-relevant: **RUE (Resist the Urge to Explain)**, show-don't-tell mechanics, dialogue/beats, "-ly" adverb flags, point-of-view consistency. Concrete detectable patterns for the fiction mode.
- **Arthur Plotnik, *The Elements of Editing*** — older but the "editor's job" framing; check for anything beyond what Saller/Gottlieb already give.
- **Verlyn Klinkenborg, *Several Short Sentences About Writing*** — **IMPORTANT, because it's a genuine philosophical conflict with the report's spine.** Klinkenborg treats the *sentence* as the unit and is *hostile to "flow"* and transitional connective tissue — the opposite of Williams' cohesion/old-before-new ideology that the report leans on. This is a real design fork (see §4), not a footnote.

### 3c. Process-side, under-developed
- **The workshop model's homogenizing tendency** — the "MFA/workshop voice" critique (e.g. Junot Díaz, "MFA vs POC"; Chad Harbach, "MFA vs NYC"). This is *directly on-theme* for the homogenization obsession: human institutions already average voices toward a house style. Worth mining for: what specifically gets sanded off, and whether the tool should actively *resist* the same tendencies.
- **Paris Review "Art of Editing" interview series** — named in the brief, not mined. Primary-source goldmine for how working editors actually talk about restraint, voice, and the limits of intervention. High signal, low effort to extract.
- **Tracked-changes / query etiquette specifics** — the *mechanics* of suggesting-not-imposing (marginal query vs. inline change conventions) as direct UI affordance sources. Report has the principle (Saller) but not the interaction-level detail.

### 3d. HCI comparanda & foundations not in the report
- **Foundational interaction model:** Horvitz (1999), *Principles of Mixed-Initiative User Interfaces* — the canonical frame for "when should the system act vs. defer." Underpins the diagnosis-vs-prescription decision at the interaction layer.
- **Co-writing systems / datasets worth a look:** CoAuthor (Lee/Liang, Stanford — dataset + the ownership findings), Wordcraft (Google, story writing), Sparks (science-writing sentence suggestions), Metaphoria, TaleBrush, Luminate (Suh et al., structured ideation), Lamplight. Most are *generation*-oriented — useful as the paradigm the project is *reacting against*.
- **Ownership/agency studies:** the recurring finding that *inline completion (ghost-text) reduces sense of ownership and increases homogenization* relative to *on-request, discrete* assistance. This is empirical ammunition for NOT defaulting to Copilot-style autocomplete for prose. (Tie to Kreminski's "dearth of the author," which IS in the report.)

### 3e. The homogenization problem stated computationally (intersection — HIGHEST conceptual payoff)
The report establishes homogenization is real and RLHF-specific. Untapped *mechanistic* framing to develop:
- **Voice ≈ controlled deviation from the expected.** Candidate technical proxy: per-author **perplexity/surprisal distance** from a generic baseline LM. A voice-preserving edit should *not reduce* the writer's surprisal signature toward the mean; a homogenizing edit does. → testable internal metric, beyond Padmakumar/He's lexical-diversity measures.
- **Mode-seeking vs. sampling.** RLHF models collapse toward high-probability continuations (the "mean"). Design levers that fight this: model choice (base vs. instruct vs. RLHF-light), temperature, *retrieval of the author's own corpus* to condition style, and **accept-friction** (Padmakumar/He saw ~70% accept rates — friction that forces engagement should reduce blind convergence).
- **Reframe:** homogenization is *not only* a model property — it's a **UI + prompting + accept-rate** property. That's the optimistic, actionable thesis the whole product can hang on, and it's defensible from the InstructGPT-vs-GPT3 result.

---

## 4. OPEN DESIGN FORKS — decisions a future session must surface, not bury

1. **Flow vs. anti-flow philosophy (Williams vs. Klinkenborg).** The report's diagnostic engine leans on Williams (cohesion, old-before-new, transitions = good). Klinkenborg says transitional connective tissue is mostly padding and the sentence stands alone. **The tool cannot encode both as "correct."** Decision needed: does the tool have a *stylistic-stance setting* (e.g. "flowing" vs. "staccato/sentence-forward"), or does it pick a house philosophy? This connects to Thomas & Turner's "classic style is a *stand*" — maybe the right primitive is *selectable stances*, not universal rules.

2. **One engine or three?** Essay / op-ed / short-fiction share some diagnostics (clutter, hedging) but diverge hard (nut graf vs. RUE vs. argument concession). Fork: unified engine + mode-specific rulesets, or genuinely separate analyzers. Cheaper to start unified; risk is mushy domain-specific value.

3. **Generation at all?** The report's pivot condition: if voice-preservation can't be guaranteed, ship **pure diagnosis, zero generation**. This is a viable standalone product and arguably the *purest* expression of the anti-homogenization thesis. Decision is partly empirical (build the voice-distance regression test first, then decide) and partly positioning. Kirill should weigh in early — it changes the whole build.

4. **Whose voice is the reference?** Voice-matching needs a corpus. Cold-start (new writer, no corpus) vs. warm (writer's existing body of work). Lex does weak voice-matching from in-doc context. Stronger: ingest the author's corpus and bias edits *against* regressing from it. Privacy/local-first implications (Kirill has shown a local-first/offline-first preference in adjacent projects — worth confirming whether that constraint applies here).

5. **Believing-mode vs. doubting-mode (Elbow).** Report proposes offering both (amplify-what-works vs. find-faults). Open: is "believing mode" a real, buildable feature or a framing? It's genuinely novel if real — worth prototyping, since every existing tool only does "doubting."

---

## 5. Verification debts (claims to confirm before any public-facing use)

- **Writor's "seven design guidelines"** — verify against the primary CHI 2026 / arXiv 2602.04047 text. Recent paper; details may be paraphrased loosely in secondary sources.
- **Lex specifics** — Tiptap stack, 100k-writers, track-changes gap: reported via reviews/founder summary, not audited. Funding ($2.75M, True Ventures, Aug 2023) and Every-incubation (fall 2022) are PR-Newswire-confirmed.
- **"Levels-of-edit → tool-modes is an open gap"** — positioning *hypothesis* from a lit scan, not exhaustively proven. Don't assert as fact publicly; frame as a claim.
- **Lish/Carver cut percentages** (50% book, ~70%+ stories, ~78% on "A Small, Good Thing"/"The Bath") — well-sourced (Sklenicka bio, Lilly Library, NYRB Harvey 2010) but the *aesthetic verdict* is contested; always present as a two-sided debate (King: "a cheat"; Harvey: "editorial genius").
- **Stein's "triage" enumerated steps, some Thomas & Turner page-level quotes** — sourced partly from summaries; verify against primary texts.

---

## 6. Suggested next actions for the Cowork session (opinionated)

1. **Read the report artifact in full first.** This doc is deltas only.
2. **Close the journalism gap (§3a) and the Klinkenborg fork (§3b/§4.1) before any build** — they're the two places the report's foundations are actually incomplete/contested, and both bear directly on the diagnostic engine's design.
3. **Build the voice-distance regression test (§3e) as the first artifact**, before committing to generation. It de-risks fork §4.3 empirically rather than by argument.
4. **Draft the levels-of-edit → modes spec** (the claimed differentiator) and pressure-test the "open gap" claim with a focused lit search (search In2Writing, CHI/UIST/CSCW 2023–2026 for any existing levels-of-edit mode mapping) before treating it as novel.
5. Mine the **Paris Review editing interviews** and the **workshop-voice critiques** (§3c) for restraint heuristics and homogenization mechanisms — cheap, high-signal, on-theme.

---

## 7. One-line state summary

Research synthesis complete (see report artifact); scope locked (essays/op-ed/short-fiction; no copy/no novels; theory>process>HCI); core thesis = *diagnosis-first, levels-as-modes, voice-preservation-as-measurable, anti-homogenization-as-positioning*; **next:** close journalism + flow-philosophy gaps, build voice-distance metric to decide the generation-vs-pure-diagnosis fork.
