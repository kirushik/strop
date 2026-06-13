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
surface is the discoverable safety net.

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
4. **Drafting/diagnosing as honest modes**: while drafting, no critique
   affordances anywhere; diagnosis is a deliberate register change.
   The AI must never be the first to speak.
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

## Open questions for Kirill

- Selection-popover formatting vs persistent format buttons (can't
  have both as primary — one demotes).
- History side panel: full-height right panel would displace the
  margin while open — acceptable?
