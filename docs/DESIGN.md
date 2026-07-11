# Strop Design Architecture

> Started 2026-06-13 after Kirill's diagnosis: "all those nits and
> papercuts are the result of our UX never being _designed_, only
> _evolving as it went_." This document is the cure: the cohesive
> UI/UX architecture every surface must answer to. PLAN.md tracks
> execution; this tracks intent. Research provenance: three agent
> reports (HCI fundamentals; surface conventions; AI-panel patterns),
> landing 2026-06-13.

## 0. Directives already decided (the user's law)

These came from use, not theory; they outrank any pattern below.

1. **Universal gestures stay universal.** Any chord that works on text
   anywhere works on text everywhere — ctrl-backspace in a query field,
   ctrl-arrows in a rename box. No surface gets a dumber text model
   because it's small. (Implication: NoteInput must grow into a real
   single-line editor — cursor, word motion, selection — or be replaced
   by a shared field widget.)
2. **Status never covers prose.** Cards, popups, toasts — nothing sits
   on top of the user's words. Margins, strips, panels: yes. Overlap:
   no. Everything transient must be dismissable.
3. **AI provider setup is the core onboarding task** and gets a real
   UI: form, async validation, live model list from the API, visible
   feedback. The config file remains the storage (UI writes through
   toml_edit, comments preserved; hand edits stay respected).
4. **No MS Visual Studio™.** Chrome stays minimal — but minimal is a
   budget to spend deliberately, not an excuse to spend it by accident
   (the current titlebar is "almost accidental").
5. **Enough support, not neutrality.** The target user includes an
   ADHD-minded aspiring fiction writer who needs the tool's help to
   *finish*. Strop should lean toward Bryce-school explorability —
   the software invites trying things — without costume-party UI.

### §0.6 Layer discipline (2026-06-12, second papercut round)
Every transient surface (palette, AI settings, shortcuts, selection popover, find/replace, inline composers, history takeover) lives on one conceptual stack. Laws:
1. The topmost layer owns every input channel: keystrokes, paste, scroll, click. Nothing leaks to the surfaces beneath it.
2. Esc always dismisses exactly the topmost layer, regardless of where keyboard focus happens to be. An open layer with no obvious way to close it is a bug by definition.
3. Light-dismiss layers (palette, selection popover, shortcuts) also close on any click outside them. Form layers (AI settings, rename, end-session) survive stray clicks; they close on Esc, their own buttons, or their backdrop.
4. Closing a layer restores focus to the surface beneath it — in practice, the editor.
Rationale: Raskin, *The Humane Interface*, ch. 2–3 — input must follow the user's locus of attention; a paste that lands behind the visible dialog violates the most basic expectation a user has of a computer. These laws are mechanically enforced by scripts/contracts.sh (H6).

### §0.7 Chrome language (2026-06-12, second papercut round)
UI chrome is English-only in v1 — one language, consistently, until real i18n (string tables, not literals sprinkled through the render tree). Russian stays only where it is content or affordance rather than chrome: palette search aliases (RU synonyms are a discoverability feature, not a translation), the tutorial and sample prose, and of course the user's own documents. A status line that switched languages based on the *document's* language — the original "Поля редактора" / "Голос:…" bug — is the anti-pattern: chrome speaks the UI's language, never the text's.

## 1. Design principles (ranked; provenance in the research report)

The canon converges on one spine: **safety enables exploration,
exploration enables finishing** (Shneiderman's easy-reversal golden
rule, Raskin's First Law, the CST principles, Krause's "padded cell",
Scrivener's Snapshots — same idea at different altitudes). Strop's CRDT
keystroke history is not a storage feature; it is the load-bearing wall
of the design, and its psychological value is mostly unextracted today.

1. **The text is the only permanent citizen; everything else is a
   guest.** (Shneiderman direct manipulation; Canon Cat; Typora.) Every
   new surface must justify why it isn't a transient popover at the
   locus of attention. VS isn't bad because it has chrome — its chrome
   has *tenure*.
2. **Nothing can be lost, and the writer must FEEL that.** (Krause: "the
   first day you come into the program, you can't hurt yourself.")
   Corollary (Raskin): universal undo replaces confirmation — Strop
   ships zero "are you sure?" dialogs, ever.
3. **The AI converses; it never types.** (iA: "Using AI in the editor
   replaces thinking. Using AI in dialogue increases thinking";
   homogenization findings: Arnold IUI'20, Padmakumar & He ICLR'24,
   Agarwal CHI'25.) HONESTY CLAUSE: those papers study *insertion*
   interfaces. "Insertion homogenizes" is established; "commentary
   doesn't" is Strop's falsifiable hypothesis — the voice-drift
   instrument exists to test it, and the docs say so.
4. **One way to do each thing; one place to find everything.**
   (Raskin's monotony × Nielsen's recognition, synthesized by the
   palette: recognition surface + shortcut teacher in one.) Discipline:
   refuse redundant toolbars later.
5. **No hidden modes; visible state at the caret.** Escape always
   returns to the document — an invariant, already true, now law.
6. **Externalize what working memory can't hold.** (Barkley:
   point-of-performance representations; ADHD writing gaps concentrate
   in planning/revision; Scrivener's real insight = synopsis-per-chunk.)
   A glanceable outline/beat strip is Strop's biggest structural gap.
7. **Reward arrives during the session or not at all.** (ADHD
   delay-aversion meta-analysis; Locke & Latham.) Per-session progress,
   never lifetime totals as the headline.
8. **Externalize time; the writer's clock is broken.** (Time-perception
   meta-analysis; hyperfocus.) Ambient elapsed time, dismissible exit
   ramps, zero mid-sprint interruptions.
9. **Opinionated defaults are the product; settings are apologies.**
   (iA: "works without settings… the open secret of its success.")
10. **The diagnosis names the problem; the writer performs the edit.**
    (Hemingway-app model; iA Style Check. Contrast Lex: preaches
    trainer-not-ghostwriter, ships one-click accept — the incoherence
    Strop names and refuses. Lex's one stealable idea: user-authored
    checks.)
11. **Playfulness quarantined to ephemeral layers; the text surface
    stays dead serious.** (Figma: whimsy in cursors, never the pen
    tool; Linear: craft felt, not seen.)
12. **Honest instruments only.** Drift is "coarse statistics, never
    identity"; no unvalidated-construct branding (no "RSD mode"); AI
    provenance stays visible.

## 2. The capability map (what surfaces exist and why)

Current inventory: titlebar (title/rename, format buttons, history
ring, hamburger, window controls), command palette, keyboard map,
margin (notes/diagnoses/AI status), footnote zone, find/replace strip,
history dropdown + inline diff, bottom strips (narrow-window variants).

Resolved against the surfaces research (2026-06-13):

**Toolbar.** None of the seven surveyed minimal editors keeps persistent
formatting buttons (even Bear collapses; iA/Typora/ghostwriter have
none; persistent chrome budget goes to panel toggles, export, stats,
modes). DECIDED: strip the titlebar to title/rename · word count ·
history button · hamburger · window controls. Formatting moves to a
**selection popover** shown on mouse-up only, keyboard-summonable (ARIA
toolbar rule), rendered as an in-surface GPUI overlay — NEVER a Wayland
xdg_popup (Zed's documented popup fragility under wlroots compositors).
Headings: ctrl-1..3 becomes the promoted chord (the iA/Bear/Typora
convention), `# ` autoformat stays, ctrl-alt-1..3 remain as silent
aliases. No undo/redo buttons — zero category precedent; the history
surface is the discoverable safety net. **One exception, shipped
2026-06-13: a Diagnose button** (E3-research's deferred "titlebar
diagnosis button — buttons teach chords"). The product's reason to
exist had no always-visible seam on first run — the margin idle hint
is suppressed exactly when demo cards exist, i.e. in the tutorial. The
button is drawn as a little margin card (the shape a diagnosis takes,
not a stock glyph), sits just left of the margin side (results-adjacent),
and its tooltip teaches `ctrl-shift-d`. It clears the "60-second
capability enumeration" test for the core feature, which prose alone
did not.

**The bar's order (2026-07-11 ordering round; four critic lenses over
a scenario map).** Left to right: `name · count [· ↺ on macOS-left
platforms nothing changes]` — `≡ · Search` — `↺ · Ask the editor ▾ —
moat — – □ ×`. The decisions, each earned against a critique round:
(1) *The menu button rides the omnibar's left flank* — the palette IS
the field's ">" mode, so its button lives on the field and the command
list blooms under the click (P7 attachment; it used to sit across the
bar in the window-controls corner, opening a menu in the middle of the
screen). Its own button beside the field, never inside it — a field's
interior belongs to the caret (P7). The field cedes the button's 31px
so the ensemble claims exactly the old third (`OMNI_MENU_W`).
(2) *History stays top-right* — the ↺-clock's corridor legibility was
learned at that corner (Docs, Time Machine; iconography.md) — but the
editor button and a moat of pure drag surface (56px, stepping to 24px
in a narrow bar) now stand between it and the OS verbs. The extracted
law: **no app verb adjacent to OS window controls.** (Rejected:
footer placement — the strip covers the footer when open, hiding the
control exactly when it must wear its open state (P12), and the footer
lane is the writer's warm territory (P3); left-cluster placement — ↺
beside the document name reads as "revert this document", and on macOS
it would move into the traffic lights' shadow.)
(3) *The count chip counts the PIECE, always* (owner's call, same day,
superseding the round's first cut): the caret-scoped "piece · N" /
"scraps · N" label spent always-visible bar space on a fact the page
already shows — the Scraps delineation header carries the pile's own
live count — and it hid the session goal whenever the caret wandered
below the seam. One region, one instrument, no emphasis states. The
doc name alone wears full ink at rest — the bar's anchor object (P11).
(4) *The session goal is set INLINE in the count chip* (the rename
idiom — P8, one grammar): the field swaps in beside the still-visible
count, prefilled with the current goal selected; **erasing it erases
the goal** (P13 — the inverse lives in the verb's own grammar), so the
old bottom strip and its "enter sets · 0 clears · esc cancels" legend
(a P4 conviction) are dead.
(5) *Situational popups exclude each other* (§0.6): the omnibar
dropdown and the editor menu never stand together; the menu's right
edge anchors by arithmetic (moat + window cells), not a captured paint
edge — deterministic on the first frame, immune to the one-frame lag
that let it drift.
(6) *Dashed underline = "this text is clickable"* — the put-back
idiom generalized (`inline_action`): the omnibar's empty-state hint
sigils type what they name ("> for commands" types ">"), and the
door's inactive pole wears the dash (state in ink, action dashed).

H3 completes the popover into three hairline-divided groups: inline
marks `[B I S {} ==]` | headings `[H1 H2 H3]` | footnote `[¹]`. Each
label demonstrates its own mark (B bold, I italic, S struck, {} mono,
== a highlit chip) so the bar teaches without text; every button carries
a name+chord tooltip; active marks/headings tint. **Underline is
deliberately absent** from the toolbar and from the mark set exposed to
writers: Markdown has no underline, manuscript convention used it only
as a typewriter-era stand-in for italics, and on screens an underline
reads as a hyperlink (Butterick, *Practical Typography*: "underlining —
absolutely not"). `SpanKind::Underline` stays in core for import
fidelity only; `ctrl-u` still toggles it for users who insist, but it
gets no button.

**History.** The Docs/Figma hybrid, not panel-vs-mode: right side panel
(PUSH, not overlay — single-document app, reflow is cheap), document
stays the diff canvas (our existing strength), slim mode banner on top
("Viewing checkpoint X · Restore · Esc"), read-only takeover,
restore-as-forward-edit (validated verbatim by Figma's semantics).
List: two-tier — named checkpoints first-class, auto-checkpoints
collapsed between them (Figma's exact answer to autosave noise),
named-only filter, each row = name · time · word delta · voice-drift
glyph (scalars in the list; prose diff on the canvas). vs-prev/vs-draft
stays (exceeds mainstream precedent) as a segmented control at panel
bottom. The current dropdown dies; at most a "edited Nm ago" teaser.

**Footnotes.** The bottom zone is Strop's most defensible original move
(print-faithful; satisfies Gwern's zero-effort criterion better than
popovers) — keep it, complete it: marks become painted superior
figures (~65% size, raised, accent ink — size signals "footnote",
color signals "interactive"; PT lacks sups/⁴⁺ glyphs so we paint our
own, same machinery as list markers), click ref → jump to def, click
def/zone marker → jump back, the zone becomes the primary edit surface
(scroll-synced Word-Notes-pane niche, essentially unoccupied), no hover
popovers (the zone already beats them), stacking policy: show all up to
3, then collapse with count. Numbering stays painted-by-order over
stable labels (the universal Word/Pandoc architecture — already ours).

*One visual home (H4).* A footnote body renders in exactly one place at
a time. The bottom zone is the page-bottom reading surface: it shows a
footnote iff its reference is in the viewport **and** its definition
block is not. The definition blocks at the document end render as a
visible "Footnotes" section — a hairline rule above the first def, set
~0.9× body size — so when the writer scrolls there, the zone stands
down. Never both at once (the H4 papercut: the same note appearing twice
when ref and section were both on screen).

**AI settings panel** (Kirill's mandate + partial research): dedicated
in-app panel; form = base_url · key (masked, paste-friendly) · model;
async test-call validation with inline states; fetch /models into a
pickable, filterable list (Open WebUI is the closest flow model);
writes config.toml through toml_edit so comments and hand edits
survive; config file remains the storage and stays hand-editable.

*Onboarding pass (2026-06-13).* The panel led with three blank fields —
the BYO-key cliff sat directly on the path to the one feature that
justifies the app. Closed with four moves (provenance: the onboarding
audit this section heads):
- **Provider picker** (principle 9, "defaults are the product"): one
  chip per opinionated provider — Local (Ollama) · OpenRouter · Poe ·
  OpenAI · Custom — prefills base_url; a "Get a key →" link `xdg-open`s
  the provider's key page; the chip lights up even for a hand-typed URL
  (substring match), so the file stays authoritative. Free-text fields
  remain for Custom and power users.
- **Local auto-detect** (the cliff-killer; local-first thesis made
  literal): an unconfigured pass fires a background `/models` probe at
  Ollama's default port (connection-refused returns instantly when
  absent). On a hit, the NeedsSetup card upgrades to a one-click,
  key-free, fully-private first pass — no account, nothing leaves the
  machine.
- **Setup→run continuity**: the pass that *triggered* setup is queued
  (`pending_pass`) and runs the moment a provider exists — Save reads
  "Save & run", the local one-click runs it directly. No "now press the
  chord again" dead end.
- **Bottom-strip robustness**: the default-sized window renders AI status
  as the bottom strip (margin doesn't fit); it now stacks title · detail ·
  actions so a long privacy line can never push the setup buttons off the
  edge.

## 3. The explorability layer (the Bryce verdict, suit on)

What aged well in Krause's school: the padded cell and live feedback
loops. What aged badly: hidden, unlabeled chrome (HN: "beautiful, but
unusable"; the Corridor pros were disoriented by cryptic chrome, then
delighted within minutes once feedback loops closed). Keep safety and
loops; discard secret doors. Maeda's Law 5: complexity demonstrably
NEARBY — visible seams.

Mechanisms (each reuses the four primitives — selection, command,
preview, undo — never new nouns):
1. **Visible-tether time travel**: history as a scrubber with live
   inline diff, labeled with consequence-free language ("a view, not a
   destruction").
2. **Preview-before-commit everywhere**: restore-preview and tinted
   find-matches exist; the pattern is the rule for all future verbs.
3. **The palette as piano lid**: fuzzy discovery + inline chords;
   add hit-frequency ordering so it becomes *your* instrument.
4. **Teach by document**: the tutorial is a Papert microworld — extend
   it to *invite* breaking things ("delete this paragraph, then press
   ctrl-alt-h and watch it come back").
5. **Solution reveal, post-hoc, opt-in**: after a clumsy manual
   operation, one dismissible whisper "that's ctrl-shift-x" — max once
   per session (VimGolf's engine; Bederson's flow rules forbid more).
6. **First diagnosis on the house**: the tutorial ships with margin
   queries pre-seeded — the first encounter with the thesis is reading,
   not invoking (low floor on the core feature). Shipped in E4.
7. **Wide walls: user-authored checks** ("flag my crutch words", "mark
   where the POV slips") — Lex's novel mechanic, recast without the
   accept button.
8. Resnick test: no capability reachable by only one modality; Raskin
   test: none with two equally-promoted modalities.

## 4. The finish-your-story layer (evidence-ranked)

1. **If-then session ritual — the strongest card in the deck.** On
   close, one question: "Next session I will ___." On open: that
   sentence, the document, the caret restored, nothing else.
   (Implementation intentions d=0.65 across 94 tests, Gollwitzer &
   Sheeran 2006; tested in ADHD samples. Mechanized Hemingway: stop
   mid-sentence, resume mid-sentence.)
2. **Tiny session goals with live progress**: "50 words" / "finish this
   beat", a bar that fills NOW (Locke/Latham; delay-aversion; Boice '83
   — moderate confidence, flagged).
3. **Pinned beat list / outline strip that ticks off** — externalized
   structure at the point of performance, scene/beat granularity. THE
   gap (see principle 6).
4. **Drafting/diagnosing as honest modes — "the door"** (SHIPPED 2026-06-14,
   core-loop research round). The single most-cited craft rule and tool
   failure is the same: evaluation fired into the generate window kills both
   momentum and voice (Elbow: premature editing "damps out the voice"). So
   Strop keeps a **door** between drafting and reviewing (`ctrl-shift-r`).
   *Drafting* (closed, the default — a document opens to write, not to be
   judged): the editorial margin goes quiet; open diagnosis/believing cards
   collapse to a thin **rail** ("3 resting · open") whose count is honest and
   whose click reopens — nothing is lost, the door is one keystroke away.
   *Reviewing* (open): cards surface, and copy-level cards stay suppressed
   while any developmental card is open (the mandatory altitude order —
   don't polish prose the structural edit may cut). The writer's own `ctrl-m`
   notes are NEVER hidden — the door quiets the *editor*, not the writer.
   Running a pass, or reaching for a resting anchor, opens the door (you
   asked to evaluate). The tutorial opens it (the demo cards are its point).
   **No behavioral inference in v1** (deferred, high-regret): a wrong guess
   that fires a card mid-burst is the one unforgivable error, so the mode is
   always manual — a missed surfacing is cheap, a wrongful interruption is
   not. The AI must never be the first to speak.
5. **Sprint timer + ambient elapsed time + one dismissible exit ramp.**
6. **Repairable momentum, never chains**: "wrote on 9 of last 14 days";
   broken-streak framing measurably depresses the behavior (Silverman &
   Barasch JCR 2023).
7. **Body doubling**: community-validated, evidence-pending; offer late,
   label honestly.

**Refused by name**: consecutive-day streaks; loss-framed nudges;
public metrics; punishment mechanics (Write-or-Die); XP/levels;
configurable planning systems (setup choices are an initiation tax);
the untraceable "3x more likely if you track" statistic.

**Invariant**: opening Strop lands you in the document, caret restored,
within one second, with zero questions asked. Scaffolds prompt at
CLOSE, when activation is cheap.

## 5. Component language

_To fill: the shared vocabulary — field widget, card, strip, panel,
popover; spacing/rhythm rules (28px), color roles, type roles — so new
surfaces are assembled, not invented._

## 4b. Tensions, resolved

1. Invisible vs discoverable → one always-visible seam (palette button),
   everything else exactly one level behind it. Test: a novice can
   enumerate Strop's capabilities in 60 seconds without docs.
2. Raskin's monotony vs Shneiderman's redundancy → monotony of
   *promotion* (one taught path: the palette), silent acceleration
   (inline chords). Never two visible buttons for one act.
3. Playful vs trustworthy → play in *mechanics* (fearless rewind,
   preview-everything, sandbox tutorial), never in *aesthetics*.
4. Help vs judgment → diagnosis is pull-only, mode-gated, query-phrased,
   rate-limited (≤7 cards), never auto-triggered.
5. Hyperfocus vs exit ramps → writer-set threshold, single dismissible
   ambient nudge, never modal.
6. Scaffolding vs zero-overhead start → scaffolds optional-and-sticky;
   prompts at close, never at open (see invariant above).
7. "Nothing lost" vs checkpoint ceremony → auto-checkpoints carry the
   safety; named ones are *annotations on* history, not its mechanism.
   Never let the writer believe unsaved = unsafe.

**The over-indexed fear**: anti-Visual-Studio is currently winning too
hard — every shipped surface is already transient and palette-gated.
The real gap is structure-holding (principle 6): nothing in Strop holds
the story's shape for the writer. That, not chrome restraint, stands
between an ADHD first-timer and "The End".

## 6. The core-loop arc (2026-06-14)

A seven-dimension web-research round (writer day-to-day loop · version &
backtracking psychology · competing drafts & merging · the editor
relationship · instant/on-demand AI feedback · the style-sheet/editorial-
agreement · tooling gaps; full dossiers + citations in
`docs/research/writer-core-loop-2026-06-14.md`). The load-bearing finding:
**writing alternates between a GENERATE mode and an EVALUATE mode, and almost
every tool failure is a failure to respect which mode the writer is in.**
Strop's whole thesis is best read as *mode discipline made into a product* —
silent during generate, a named-problem-and-a-question editor during
evaluate, never supplying the replacement text that homogenizes voice.

The consolidated core-loop model (design against the **transitions**, not the
stages): IGNITION (cold start) → DRAFTING bursts → the CARDINAL GUARD
(drafting⇄evaluating mid-burst — the involuntary crossing that flattens
voice) → HANDOFF (stop where you know what comes next) → RE-ENTRY (warm-up
re-read that becomes line-editing) → ALTITUDE DESCENT (dev→line→copy).

**The prioritized arc** (leverage × effort × regret; one round at a time, so
Kirill stays in the loop between them):

| Round | Opportunity | Lev | Eff | Regret | Status |
|---|---|---|---|---|---|
| 1 | **The Door** (draft/review gate) + altitude suppression + card-grammar | High | M | Low | **shipped 2026-06-14** |
| 2 | History by *meaning* (checkpoints labeled by the cards open then; revert-as-forward made loud; defuse recency bias) | High | M–L | Med | next |
| 3 | "Try it both ways" — in-place A/B fork + Cuttings drawer, **no merge** ("I don't assemble stories, I write them") | Med–High | M | Med | |
| 4 | Strengths-first developmental **editorial letter** (whole-manuscript altitude) | High | M | Med | |
| 5 | **Editorial Agreement** — Continuity Canon (enables drift cards) + Voice Charter (suppresses cards), born empty, infer-and-ratify, linter-style governance | High | L | **High** | |
| later | voice-trajectory sparkline · re-entry on-ramp · behavioral mode inference · anti-tinker nudge | Med | S–L | mixed | |

**Red lines the research drew** (hold across every round): never a "suggested
rewrite" field (supplying text *is* the homogenizing anchor — Doshi & Hauser,
Cornell/CHI'25); the editorial agreement may only ever learn what to *stop*
flagging, surfaced in the open and ratified, never silent acceptance-driven
adaptation; no auto-merge of forks; police *problems*, not *style*, by
default (a born-empty Voice Charter must not flag intentional comma splices
as errors — T1); behavioral mode-inference defaults ambiguous states to
**quiet** (T6). The one finding that pushes against a Strop principle and
*wins*: the agreement wants a seed but the open forbids questions → resolved
as born-empty + an optional, skippable, post-landing "tell me a quirk", never
a gate.

## 7. The margin card system (two layers, in motion)

The editorial margin hosts two kinds of object that look alike and behave
nothing alike. Conflating them into one `MarginCard` is the root of the
placement bugs (overlap, scroll pile-up) and the dynamic confusion. They split
by **behaviour over time**, not appearance. (Companion analysis + the dynamic
failure-mode catalogue + the build order: `docs/margin-card-dynamics.md`.)

**Layer A — the writer's marginalia** (`ctrl-m` notes). Authored, owned, part
of GENERATE: where ideas, loose self-feedback, and TODOs land (principle 6,
externalize working memory). Mode-agnostic — the door (§4.4) never touches
them; present while drafting *and* reviewing. Long-lived; only the writer
resolves them. In layout they are the **stable spine**: high priority, pinned
to their anchor, and they do **not** reshuffle when the editorial layer toggles.

**Layer B — the editorial review** (AI diagnoses). Metadata *about* the work,
part of EVALUATE. Mode-gated (reviewing only). Born in **passes** (≤7 per pass,
Tension 4; multiple passes accumulate). Transient — created to be resolved and
cleared. **Decays**: an edit to the anchored text can make a diagnosis wrong, so
a stale card is greyed and deprioritised — never auto-dismissed; only the writer
permanently dismisses (principle 10). In layout they **yield around** Layer A.

One column. The layers are told apart by **treatment + priority + visible AI
provenance** (principle 12), never by a second column or a wash of card-body
colour (which would wreck the calm paper field).

**No asynchronous source — so no "unread."** Single-user, local files: the
writer makes every Layer-A note by hand, and Layer-B cards appear only inside a
pass the writer explicitly ran (the AI never speaks first, §4.4). Every card
birth is witnessed; nothing arrives while you're away. So there is **no
read/unread state and no "new since last session"** — re-entry shows the
document exactly as you left it (itself the principle-2 safety the writer must
*feel*). The only freshness axis that exists is **which pass** a card came from,
session-independent. (Reopens only if cross-device Loro sync ever lands;
deferred.)

**Layout — one PAVA pass, culled to the viewport.** Replace the three fighting
passes (downward sweep · reverse up-push · floor re-sweep) with a single
isotonic/PAVA solver carrying per-card weights — active highest (pins to
anchor), Layer-A high, Layer-B default — so overlapping cards pool into a
cluster centred on their anchors with no sag and one settled position. **Cull to
viewport**: a card occupies the lane only while its anchor is on (or just off)
screen; off-screen cards roll up into honest `▲N / ▼N` edge chips (the door's
rail grammar — nothing vanishes silently). **Orphaned** cards (anchor lost in a
restore) leave the lane for an `N detached` holder, never floating untethered.
**Connector: highlight-only** — attribution is the anchor highlight + proximity
+ bidirectional hover-emphasis, no leader lines; this is *why* culling and tight
anchor-tracking are load-bearing, not optional.

**Lifecycle.** birth → drift (anchors ride edits via `apply_op`) → staleness
(Layer B only: edited anchor → greyed/deprioritised) → activation (z-raise +
expand-in-place + glow the anchor; minimal repack) → resolution (done/dismiss =
status, not delete; reversible through history). Across passes, Layer-B cards
carry a `pass_id` + pass metadata; a new pass reconciles carried-over cards
(stale ones grey out) and dismissals **teach suppression** so the tool stops
re-flagging what you waved off — the minimal seed of the Editorial Agreement
(Round 6/§6 Round 5: learn only what to *stop* saying).

**Motion discipline** (principle 11 — craft felt, not seen): one settled
position per frame; the active composer grows *downward* from a pinned top
(never reflows upward per keystroke); identity is stable by `id` through reorder
(slide, don't flicker); activation barely moves the lane; transitions ease.

**Aging cue** (the distinction for accumulated passes): by **pass**, not
wall-clock, and correlated with **trust** — a stale (edited-anchor) card faded
reads as both "older" and "verify me." Carried on a thin left-edge tab in 2–3
muted tones (the sticky-note-pack metaphor) or a two-tier crisp/settled,
**never** a full-card colour wash.

**Decided 2026-06-19 (with Kirill):** one column; `pass_id` + per-pass metadata
added to the model (breaking change acceptable at 0.1.0 — testers tolerant, no
non-tester usage; best-effort migrate legacy cards, don't overspend); decay =
grey-out + deprioritise, the writer is the sole permanent dismisser. **Refused:**
auto-dismiss of stale cards (harsh, confusing); per-card colour washes (noise);
leader lines; a second margin column; read/unread state (no async source).

## Open questions for Kirill

- Selection-popover formatting vs persistent format buttons (can't
  have both as primary — one demotes).
- History side panel: full-height right panel would displace the
  margin while open — acceptable?
- The door's open-time default is *drafting* everywhere except the tutorial
  (protects re-entry). Revisit if returning to a manuscript with resting
  cards behind the rail feels like hiding rather than focus.
- Round 2 vs Round 3 ordering: history-by-meaning is the dependency-correct
  next step, but "try it both ways" is the more demo-able answer to the
  competing-edits question. Say which you want first.
- Margin §7: re-validating carried-over Layer-B cards on a new pass wants an AI
  call per stale card. Worth the cost/latency, or is grey-out + let-the-writer-
  judge enough for v1 (re-validation deferred)?
