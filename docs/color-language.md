# Strop — the color language

One axis, four meanings, two mechanics. Every semantically-meaningful color
lives in `crates/strop-app/src/theme.rs`; this doc is the *why*. The goal is a
palette that's **habit-forming**: a color means the same thing everywhere, so
the writer learns it once.

Grounded in a 2026-06-23 research pass (conventions, leading-tool precedent,
WCAG/color-blindness math); sources at the end.

## The principle

> **Warm = the human, cool = the machine.** Everything the writer authors or
> touches wears warm amber/gold; everything the AI says wears a calm cool blue
> that never competes with the writer's warm hand. **Confidence rides
> saturation, not a new hue:** a live AI note is a clear cool tint, and as its
> anchor goes stale it *drains toward neutral* (doubt = desaturation). Red is
> reserved strictly for true errors; green marks the one moment of success.
> Because meaning rides a **warm / cool / neutral** axis — never red-vs-green —
> the language stays legible for color-blind readers, and every card is a faint
> tint light enough to keep near-black body text at WCAG AAA.

On the owner's instinct "AI is always blue": adopt the conclusion, reject the
reason. There is *no* industry "AI blue" (Copilot=purple, Claude=terracotta,
OpenAI=near-black, Gemini=four-color). The real reason blue wins is that strop
already owns warm amber for the writer, so the machine voice must be its **cool
complement** — and blue beats purple (least-standardized, blue↔purple is a CVD
confusion) and terracotta (collides with our amber; it's Claude's own brand).

**The axis is ordered** (Scraps token audit, 2026-07-06): warm = living,
cool = machine-live, drained = receding from life (stale → dead), red = error;
labels, rules, and chrome are off-axis. So the writer's living pile wears a
warm wash (`SCRAP_WASH`) in full ink while the graveyard's record drains
(`STALE_BG`), and the one arrival warm (`ARRIVAL_FLASH`) means "something
arrived over there" wherever it blinks. Put-back's returned-paragraph flash is
deliberately the warm `SELECTION_COLOR`, not sage — retrieval arrives selected
(amber = the writer acting); sage stays reserved for answered/resolved
terminals and the reached goal.

## Hue → meaning

| Meaning | Family | Tokens |
|---|---|---|
| Writer's marginalia (human, permanent) | **warm amber/gold** | `NOTE_CARD_BG`, `NOTE_TINT`, `NOTE_TINT_ACTIVE`, `ACTIVE_BORDER`, `FIELD_SELECTION_BG` |
| AI diagnosis (machine, transient) | **cool blue** | `DIAGNOSIS_CARD_BG` (card wash), `AI_ACCENT` (the wavy anchor squiggle + in-card ink) |
| Provisional / stale / unverified | **drained → neutral taupe** | `STALE_BG`, `MUTED_COLOR` |
| Structure (rules, borders, secondary text) | **muted taupe** | `RULE_COLOR`, `MUTED_COLOR` |
| Achieved goal | **sage** | `SAGE_COLOR` |
| Error / destructive (only) | **red** | `ERROR` |
| AI diagnosis, ACTIVE anchor | **cool blue** band | `DIAGNOSIS_TINT_ACTIVE` |
| Text selection (prose & field) | **warm amber** *(the writer acting)* | `SELECTION_COLOR`, `FIELD_SELECTION_BG` |
| Writer's `==highlight==` mark | yellow *(mechanic)* | `HIGHLIGHT_COLOR` |
| Search-match / diff-insert | sage *(mechanic)* | `FIND_MATCH_BG` |
| Hyperlink | cool blue + underline | `LINK_COLOR` |
| Inline code | neutral chip | `CODE_BG_COLOR` |

## Concrete values & contrast (verified)

Body ink `TEXT_COLOR #1A1A18` on page `BG_COLOR #FBFAF8` = 16.7:1. Card washes
sit ~1.05–1.2:1 *off the page* (a perceptible tint, far from chrome) while
keeping body text at **AAA**:

| Token | Hex | Body-text contrast |
|---|---|---|
| `NOTE_CARD_BG` (writer) | `#FAF4E2` | 15.86:1 AAA |
| `DIAGNOSIS_CARD_BG` (live AI) | `#EEF3FB` | 15.64:1 AAA |
| `STALE_BG` (drained) | `#EFEEEA` | 15.01:1 AAA |
| `AI_ACCENT` (AI ink) | `#3D5C8C` | 6.48:1 AA |
| `LINK_COLOR` | `#2156A8` | 6.80:1 AA |
| `ERROR` | `#B23B2E` | 5.66:1 AA |
| `ACTIVE_BORDER` / `NOTE_TINT` amber | `#C8A951` / `#E3B849` | ~2.2:1 — **wash/border only, never a load-bearing mark** |

**Two constraints from the math:**
1. **Amber can't be a contrast-bearing mark** (~2.2:1). The writer-note identity
   lives in the *wash*, not a small amber glyph. Pair any amber mark with
   shape/size.
2. **Stale ↔ live-AI is the subtlest pair** under "tint only." Mitigated by
   making `STALE_BG` the most clearly de-saturated member (drains toward the
   paper's warmth) and, if needed, muting in-card AI text to taupe when stale (a
   text-color change, not chrome) — a second, redundant cue per WCAG 1.4.1.

## Color-blindness

Meaning never rides red-vs-green. The warm/cool/neutral axis is the
color-blind-safe choice (blue/orange is the recommended warm-cool pair). Two
sites that *did* rely on color alone were fixed (2026-06-23): the AI-settings
test feedback now prefixes ✓ / ✗; the history voice-anomaly stopped using red
(it's a descriptive flag, not an error — the "Nσ outside your range" text
carries it).

## Resolved 2026-06-23: selection is warm, blue is the machine's alone

The earlier-open tradeoff is decided. Prose `SELECTION_COLOR` is now **warm amber**
(matching the in-field `FIELD_SELECTION_BG`): selecting text is *the writer acting
on the page*, so it wears the human's warm hand. That frees **cool blue to mean
exclusively "the machine"** — the AI card wash, the squiggle, and the new
`DIAGNOSIS_TINT_ACTIVE` band that an AI-commented anchor now shows when selected
(it used to fall through to the warm amber note tint — the bug that broke the
warm/cool axis exactly when a diagnosis was engaged).

**Why we could just decide it (the "is OS selection blue?" question).** Nothing
reads an OS selection colour, and nothing can: the prose canvas is a fully
self-drawn GPUI element, so its selection rectangle is *our* `SELECTION_COLOR`,
not a native widget's. gpui exposes no system selection/accent colour either —
its `Colors` palette is a hardcoded light/dark default (`selected = 0x2a63d9`),
flipped only by appearance, never sampled from the OS. So there was never an OS
blue to collide with or to read; we own the colour outright and chose warm.

## Deferred (incidental chrome, not yet promoted)

The audit found ad-hoc chrome hexes not yet in `theme.rs`: panel fills
(`#F4F1EA` ×7, `#FCFAF4` ×8), the very-pale warm pills (`#F7F5EF`), the warm
action-button colors (`#E8DFC8`/`#DFD3B0`), and the interactive-state overlays
(`rgba(0x1A1A1812)` selected / `…0A` hover / `…30` modal, ×25). These carry
*state*, not the four content meanings, so they're left for a follow-up
consolidation into named tokens (e.g. `PANEL_BG`, `HOVER_OVERLAY`). Also noted:
`MUTED_COLOR` as small *text* is ~3.5:1 (below AA) — a future `MUTED_TEXT`
(~`#6E6B61`) should split text from rules.

## Sources

Conventions: IBM Carbon (status palette; purple=outliers), Atlassian (info/
success/warning/danger/discovery), Material 3 (error role), Apple HIG, NN/g
(never color alone). Human-vs-AI: GitHub Copilot purple `#8534F3`, Anthropic
terracotta, OpenAI green→black, Gemini four-color, VS Code/Copilot dimmed-grey
ghost text, Google Docs yellow comments, GitHub greyed "outdated". Accessibility:
WCAG 1.4.3/1.4.6/1.4.11/1.4.1; contrast tracks luminance not hue (WebAIM);
color-blindness ~8% of men, avoid red+green (NEI, Colour Blind Awareness).
Paper/editor: Kindle sepia, macOS `#B3D7FF` selection, VS Code theme tokens.
