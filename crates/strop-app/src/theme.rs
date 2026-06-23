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
//! `SELECTION_COLOR` (blue) and `HIGHLIGHT_COLOR` (yellow) are plain editor
//! mechanics in the prose canvas; `LINK`/`CODE_BG` are structural. Incidental
//! chrome (panel fills, hover overlays) is not yet promoted here — see the doc's
//! "deferred" list. When you reach for a new color, first ask which meaning it
//! belongs to; add a new meaning only deliberately.
//!
//! Values are `0xRRGGBB` or `0xRRGGBBAA` (alpha when translucent), the form
//! gpui's `rgb()` / `rgba()` take. Card tints are verified AAA against
//! `TEXT_COLOR`; see the doc for ratios.

// --- Surface & ink -------------------------------------------------------

/// The page: a warm off-white, not pure white (easier on the eye, sets the
/// "paper" baseline the card tints are read against).
pub const BG_COLOR: u32 = 0xFBFAF8;
/// Primary ink: near-black with a hint of warmth, never pure `#000`.
pub const TEXT_COLOR: u32 = 0x1A1A18;
/// Secondary / provisional / quiet text: stale diagnoses, labels, captions.
pub const MUTED_COLOR: u32 = 0x8A8678;
/// Hairline rules, borders, dividers — structure that shouldn't draw the eye.
pub const RULE_COLOR: u32 = 0xE8E4DC;

// --- Card fills (the writer/AI distinction, DESIGN two-layer model) -------

/// Generic card chrome (AI-status card, edge-count pills, the narrow-notes
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

// --- Cool blue: the AI / machine voice -----------------------------------

/// The AI's ink: the wavy diagnosis-anchor squiggle and in-card accents (AA
/// text, 6.48:1 vs page / 6.06:1 on the AI wash). The legible "machine voice"
/// against the writer's warm amber.
pub const AI_ACCENT: u32 = 0x3D5C8C;

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

/// Prose text selection (the OS-idiom blue), translucent.
pub const SELECTION_COLOR: u32 = 0xB4D5FE88;
/// Find / search match highlight (yellow), translucent.
pub const HIGHLIGHT_COLOR: u32 = 0xF9E29CAA;
/// Inline-code background tint.
pub const CODE_BG_COLOR: u32 = 0x1A1A1814;
/// Hyperlink ink (AA text, 6.80:1) — cool, shares the machine-voice family
/// ("points outward, not your warm words"); the underline carries the meaning.
pub const LINK_COLOR: u32 = 0x2156A8;
