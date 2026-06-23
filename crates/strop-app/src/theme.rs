//! The color language. One place for every semantically-meaningful color, so
//! the palette stays consistent and *habit-forming* for the writer: a color
//! means the same thing everywhere it appears.
//!
//! The vocabulary (four content meanings + a few editor mechanics):
//!
//! - **Warm amber = the writer / active engagement.** Your own marks and the
//!   thing you're touching right now: your margin notes (`NOTE_CARD_BG`,
//!   `NOTE_TINT`), the active/selected card border (`ACTIVE_BORDER`), the
//!   selection inside a field (`FIELD_SELECTION_BG`). Habit: *warm = mine.*
//! - **Cool neutral = the tool (AI).** AI diagnosis surfaces lean cool and
//!   slightly grey (`DIAGNOSIS_CARD_BG`) so they read as the assistant's output
//!   sitting *over* the page rather than ink *on* it. Habit: *cool = the tool.*
//! - **Muted taupe = provisional / quiet / structural.** Unverified diagnoses,
//!   stale anchors, secondary labels, hairline rules (`MUTED_COLOR`,
//!   `RULE_COLOR`). Habit: *greyed = not sure / not load-bearing.*
//! - **Sage = achieved.** The reached-goal dot, used sparingly, no celebration
//!   (`SAGE_COLOR`). Habit: *sage = done.*
//!
//! `SELECTION_COLOR` (blue) and `HIGHLIGHT_COLOR` (yellow) are plain editor
//! mechanics in the prose canvas, not part of the semantic vocabulary; `LINK`
//! and `CODE_BG` are likewise structural. Incidental chrome (pure white field
//! fills, panel backgrounds) is not promoted here — only colors that *carry
//! meaning* live in this module. When you reach for a new color, first ask which
//! of the four meanings it belongs to; add a new meaning only deliberately.
//!
//! Values are `0xRRGGBB` or `0xRRGGBBAA` (alpha when translucent), the form
//! gpui's `rgb()` / `rgba()` take.

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
/// Layer A — the WRITER'S own note: a warm cream wash. Reads as ink *on the
/// page*, and rhymes with the amber note-anchor tint. The "paper-tint"
/// differentiation chosen 2026-06-23 (margin-card-dynamics §11).
pub const NOTE_CARD_BG: u32 = 0xFCF6E9;
/// Layer B — an AI DIAGNOSIS card: a faintly cool, grey-leaning off-white.
/// Subtly *not warm*, so it reads as the tool's output over the page. Pairs
/// with the lifecycle difference (AI cards are gated by the door; notes aren't).
pub const DIAGNOSIS_CARD_BG: u32 = 0xF5F6F8;

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

// --- Editor mechanics (not part of the semantic vocabulary) --------------

/// Prose text selection (the OS-idiom blue), translucent.
pub const SELECTION_COLOR: u32 = 0xB4D5FE88;
/// Find / search match highlight (yellow), translucent.
pub const HIGHLIGHT_COLOR: u32 = 0xF9E29CAA;
/// Inline-code background tint.
pub const CODE_BG_COLOR: u32 = 0x1A1A1814;
/// Hyperlink ink.
pub const LINK_COLOR: u32 = 0x1A56A0;
