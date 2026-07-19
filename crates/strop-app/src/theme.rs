//! The color language. One place for every semantically-meaningful color, so
//! the palette stays consistent and *habit-forming* for the writer: a color
//! means the same thing everywhere it appears. Full rationale, sources, and
//! the accessibility math live in `docs/color-language.md`.
//!
//! **The one axis: warm = the human, cool = the machine.** Everything the
//! writer authors or touches wears warm amber/gold; everything the AI says
//! wears a calm cool blue that never competes with the writer's warm hand.
//! **Confidence rides saturation, not a new hue:** a live AI note is a clear
//! cool tint; as its anchor goes stale the color *drains toward neutral*
//! (`STALE_BG`) — doubt = desaturation, not red. Red (`ERROR`) is reserved for
//! true errors only. Green (`SAGE_COLOR`) marks the one moment of success.
//! Because meaning rides a warm / cool / neutral axis — never red-vs-green —
//! the language stays legible for color-blind readers, and every card tint is
//! faint enough to keep near-black body text at WCAG AAA.
//!
//! The vocabulary:
//! - **Warm amber = the writer / active engagement.** Your margin notes
//!   (`NOTE_CARD_BG`, `NOTE_TINT`), the active/selected border (`ACTIVE_BORDER`),
//!   the in-field selection (`FIELD_SELECTION_BG`). Habit: *warm = mine.*
//! - **Cool blue = the AI tool.** Diagnosis card fills (`DIAGNOSIS_CARD_BG`) and
//!   the AI's marks/ink (`AI_ACCENT`: the wavy anchor squiggle, in-card accents).
//!   Habit: *cool = the assistant.*
//! - **Drained neutral / muted taupe = provisional / quiet / structural.** Stale
//!   or unverified cards (`STALE_BG`), secondary labels (`MUTED_COLOR`), hairline
//!   rules (`RULE_COLOR`). Habit: *greyed = not sure / not load-bearing.*
//! - **Sage = achieved** (`SAGE_COLOR`); **red = error** (`ERROR`), nothing else.
//!
//! `SELECTION_COLOR` is warm amber (selecting text is the *writer* acting — see
//! its note); `HIGHLIGHT_COLOR` (yellow) is the writer's `==highlight==` mark and
//! `FIND_MATCH_BG` (sage) the search/diff preview; `LINK`/`CODE_BG` are
//! structural. When you reach for a new color, first ask which meaning it
//! belongs to; add a new meaning only deliberately.
//!
//! **What is NOT here (deliberately, 2026-06-23).** Incidental *chrome* colors —
//! panel/drawer fills, hover/selected/modal overlays, pill and action-button
//! fills — are left inline at their use sites, NOT promoted to tokens. They
//! encode interaction *state* (a surface, a hover), not one of the four content
//! meanings above, so pulling them here would dilute the vocabulary. Roster, so
//! the next person knows where they live and why: panel fills `0xF4F1EA`/
//! `0xFCFAF4`, pills `0xF7F5EF`, action buttons `0xE8DFC8`/`0xDFD3B0`, and the
//! interactive overlays `rgba(0x1A1A18, 12/0A/30)` (selected / hover / modal
//! scrim). Promote them to e.g. `PANEL_BG`/`HOVER_OVERLAY` only if a second
//! regime forces a real choice. Also pending: `MUTED_COLOR` as *small text* is
//! ~3.5:1 (below AA); a future `MUTED_TEXT` (~`0x6E6B61`) should split text from
//! rules. Full rationale + contrast math: `docs/color-language.md`.
//!
//! Values are `0xRRGGBB` or `0xRRGGBBAA` (alpha when translucent), the form
//! gpui's `rgb()` / `rgba()` take. Card tints are verified AAA against
//! `TEXT_COLOR`; see the doc for ratios.

// --- Surface & ink -------------------------------------------------------

/// The page: a warm off-white, not pure white (easier on the eye, sets the
/// "paper" baseline the card tints are read against).
pub const BG_COLOR: u32 = 0xFBFAF8;
/// Auxiliary windows (keymap, About — everything in the aux_window shell):
/// a step warmer and dimmer than the page, so a window floating OVER the
/// manuscript can never be mistaken for the manuscript. Furniture, not
/// paper. Was the keymap window's local hardcode; promoted when About
/// arrived in page-white and read as the page it covered.
pub const AUX_BG: u32 = 0xF6F4EF;
/// Primary ink: near-black with a hint of warmth, never pure `#000`.
pub const TEXT_COLOR: u32 = 0x1A1A18;
/// Secondary / provisional / quiet text: stale diagnoses, labels, captions.
pub const MUTED_COLOR: u32 = 0x8A8678;
/// Hairline rules, borders, dividers — structure that shouldn't draw the eye.
pub const RULE_COLOR: u32 = 0xE8E4DC;

// --- Card fills (the writer/AI distinction, DESIGN two-layer model) -------

/// Generic card chrome (AI recovery, edge-count pills, the narrow-notes
/// panel): a neutral near-white, no warm/cool lean — it carries no layer
/// meaning, it's just "a small surface".
pub const CARD_BG: u32 = 0xFFFDF6;
/// Layer A — the WRITER'S own note: a warm cream wash (AAA, 15.86:1). Reads as
/// ink *on the page*, and rhymes with the amber note-anchor tint. The
/// "paper-tint" differentiation chosen 2026-06-23 (margin-card-dynamics §11).
pub const NOTE_CARD_BG: u32 = 0xFAF4E2;
/// Layer B — a live AI DIAGNOSIS card: a faint cool BLUE wash (AAA, 15.64:1).
/// Cool = the machine voice (the complement to the writer's warm), so it reads
/// as the tool's output over the page. Pairs with the lifecycle difference
/// (AI cards are gated by the door; notes aren't).
pub const DIAGNOSIS_CARD_BG: u32 = 0xEEF3FB;
/// A stale / unverified card: the cool tint DRAINED toward the paper's own
/// warmth (AAA, 15.01:1). Doubt = desaturation, "fading back into the page" —
/// never red (staleness is not an error).
pub const STALE_BG: u32 = 0xEFEEEA;
/// The Scraps region wash (08 §2): the writer's living pile at the document
/// tail, painted over the column measure + ~14px bleed — never viewport-wide.
/// This is `NOTE_CARD_BG` at ~45% over the page, baked solid (~15.5:1 vs
/// TEXT_COLOR, AAA holds). Distinctness from the note-card cream is carried
/// by CHROMA (same value, roughly half the saturation) plus FORM (a
/// borderless region wash at full measure vs a bordered rounded card; wash
/// text is 0.8-size full ink) — no page-legible wash can be "visibly distant
/// in value" from cream, so the distance here is chroma-not-value. Flagged
/// for the taste round, where the writer parked the warm-family revisit.
pub const SCRAP_WASH: u32 = 0xFAF7EE;

// --- Cool blue: the AI / machine voice -----------------------------------

/// The AI's ink: the wavy diagnosis-anchor squiggle and in-card accents (AA
/// text, 6.48:1 vs page / 6.06:1 on the AI wash). The legible "machine voice"
/// against the writer's warm amber.
pub const AI_ACCENT: u32 = 0x3D5C8C;
/// The in-text highlight band of an ACTIVE diagnosis anchor: a translucent cool
/// blue, the machine-voice counterpart to the writer's warm `NOTE_TINT_ACTIVE`.
/// A resting diagnosis wears only the squiggle; selecting/activating it promotes
/// to this blue band (matching its card + squiggle) — so blue is, consistently,
/// "the AI is pointing here", never the warm amber the writer's own notes use.
pub const DIAGNOSIS_TINT_ACTIVE: u32 = 0x86B0E64D;

// --- Warm amber: the writer & active engagement --------------------------

/// The active/selected card border, and field-focus accents — the gold that
/// says "this is the thing you're engaging with right now".
pub const ACTIVE_BORDER: u32 = 0xC8A951;
/// The selection highlight inside a text field: the active-card gold, made
/// translucent so the glyphs read through it.
pub const FIELD_SELECTION_BG: u32 = 0xC8A95155;
/// A note's in-text anchor tint (resting): wheat/amber ~15% — the Docs-trained
/// "there's a comment here" intuition.
pub const NOTE_TINT: u32 = 0xE3B84926;
/// The same anchor tint at ~30%, when its card is active.
pub const NOTE_TINT_ACTIVE: u32 = 0xE3B8494D;

// --- Sage: achievement ---------------------------------------------------

/// The reached-goal dot (DESIGN §4.2): subtle, no celebration.
pub const SAGE_COLOR: u32 = 0x7D8C66;

// --- Red: errors ONLY (never "stale") ------------------------------------

/// True errors / destructive outcomes only (AA text, 5.66:1). Reserved: doubt
/// and staleness desaturate (`STALE_BG`); they do not turn red. Pair with a
/// non-color cue (icon/text) so it's legible without color (WCAG 1.4.1).
pub const ERROR: u32 = 0xB23B2E;

// --- Editor mechanics (not part of the semantic vocabulary) --------------

/// Prose text selection: warm amber, translucent — the SAME family as the
/// in-field selection (`FIELD_SELECTION_BG`) and the active-note tint, because
/// selecting text is *the writer acting on the page* (warm = human). This is a
/// deliberate departure from the OS-idiom blue: it frees cool blue to mean
/// *exclusively* "the machine" (AI cards, squiggles, `DIAGNOSIS_TINT_ACTIVE`).
/// We can do this freely because nothing reads an OS selection color — the prose
/// canvas is fully self-drawn and gpui exposes no system selection/accent color
/// (its `Colors` is a hardcoded light/dark default, not OS-derived).
pub const SELECTION_COLOR: u32 = 0xC8A95166;
/// Markdown `==highlight==` inline-attribute background (yellow), translucent —
/// the writer's own emphasis mark. NOT the find/search color (that's
/// `FIND_MATCH_BG`); the two were conflated in an earlier docstring.
pub const HIGHLIGHT_COLOR: u32 = 0xF9E29CAA;
/// Find / search match (and history diff-insert) tint: a translucent sage,
/// deliberately distinct from the warm wheat of selection/notes so a live search
/// preview never reads as a selection or an annotation anchor.
pub const FIND_MATCH_BG: u32 = 0x7FB8A455;
/// Inline-code background tint.
pub const CODE_BG_COLOR: u32 = 0x1A1A1814;
/// Hyperlink ink (AA text, 6.80:1) — cool, shares the machine-voice family
/// ("points outward, not your warm words"); the underline carries the meaning.
pub const LINK_COLOR: u32 = 0x2156A8;

// --- Scraps & the graveyard ----------------------------------------------

/// The arrival blink (08 §2; token audit A3): the active gold made translucent
/// (~0.33 alpha) — ONE warm for "something arrived over there", shared by the
/// landed-block flash in the pile, the graveyard section's exile blink, and
/// the footer chips' pulse/blink. A single quiet flash, then gone. (Unified
/// from the old COMPOST_FLASH + the graveyard bar's inline 0xEFE6D0; the
/// COMPOST_TAIL anchor mark died with the rail.)
pub const ARRIVAL_FLASH: u32 = 0xC8A95154;
