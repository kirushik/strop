//! The prose canvas: a multi-paragraph editable text element built directly
//! on GPUI's IME-capable input plumbing (`EntityInputHandler`).
//!
//! Cursor positions at soft-wrap boundaries are ambiguous (end of the upper
//! visual line vs start of the lower); we resolve them with an explicit
//! affinity bit, set by the motion that produced the position.
//!
//! v0 scope: plain text, cursor/selection/mouse, word ops, clipboard, undo.
//! Not yet: scrolling, drag-extends-by-word after double-click, cursor blink.

use std::ops::Range;
use std::time::{Duration, Instant};

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui::{
    Animation, AnimationExt,
    AnyView, App, Bounds, BoxShadow, ClipboardEntry, ClipboardItem, Context, Corners, CursorStyle,
    Decorations, Element, ElementId, ElementInputHandler, ExternalPaths, Hsla, RenderImage,
    Entity, EntityInputHandler, FocusHandle, Focusable, FontStyle, FontWeight, GlobalElementId,
    KeyBinding, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad,
    Pixels, Point, ResizeEdge, ScrollHandle, ScrollWheelEvent, SharedString, StrikethroughStyle,
    Style, TextAlign, TextRun, Tiling, UTF16Selection, UnderlineStyle, Window, WindowControlArea,
    WrappedLine, actions, div, fill, point, prelude::*, px, relative, rgb, rgba, size,
};
use strop_core::document::{
    Annotation, Annotations, BlockKind, BlockMap, Document, InlineAttr, NoteKind, NoteStatus,
    SpanSet,
};
use strop_core::{Store, typograph};

use crate::config::{Config, Language};
use crate::draw_guard::{DrawGuard, EntityUpdateExt as _, capture_canvas};
use crate::strip::{self, Strip, StripBake};
use crate::text_field::{
    FieldBackspace, FieldBackspaceWord, FieldCancel, FieldCommit, FieldPaste, FieldTab, TextField,
    TextFieldEvent,
};
use unicode_segmentation::UnicodeSegmentation;

// The semantic color language lives in `theme`; everything below is layout.
pub use crate::theme::{BG_COLOR, TEXT_COLOR};
use crate::theme::{
    ACTIVE_BORDER, AI_ACCENT, CARD_BG, CODE_BG_COLOR, COMPOST_FLASH, COMPOST_TAIL,
    DIAGNOSIS_CARD_BG, DIAGNOSIS_TINT_ACTIVE, ERROR, FIND_MATCH_BG, HIGHLIGHT_COLOR, LINK_COLOR,
    MUTED_COLOR, NOTE_CARD_BG, NOTE_TINT, NOTE_TINT_ACTIVE, RULE_COLOR, SAGE_COLOR, SELECTION_COLOR,
    STALE_BG,
};

const MARGIN_WIDTH: f32 = 248.;
const MARGIN_GAP: f32 = 16.;
/// The omnibar's widest width (06 §1, S4): the empty runway IS the type-here
/// affordance. ONE width for the field and its dropdown — equal boxes, equal
/// centres, so all four edges agree (the width-coherence papercut) — computed
/// by `omni_field_width` from the live window so a narrow bar never squeezes
/// the window controls.
const OMNI_FIELD_W: f32 = 400.;

/// The one omnibar width, shared by the titlebar field and its dropdown: a
/// third of the window, capped. Both surfaces call this — they can never
/// disagree.
fn omni_field_width(window: &Window) -> f32 {
    (f32::from(window.viewport_size().width) / 3.).clamp(160., OMNI_FIELD_W)
}
/// Margin-card box metrics, shared by the height MEASUREMENT
/// (`refresh_card_heights`) and the RENDER (`render_margin`) so a card's packed
/// extent equals its painted one. Text wraps at the card's inner width; the
/// line-height is pinned so one shaped row equals one painted row.
pub(crate) const CARD_LINE_H: f32 = 18.;
const CARD_CHROME_H: f32 = 36.; // vertical padding (16) + border (2) + header row (18)
const CARD_INNER_W: f32 = MARGIN_WIDTH - 8. - 16.; // card width (MARGIN_WIDTH−8) − p(8) both sides
/// The composer's text wraps slightly narrower than committed body text: the
/// in-card input box adds its own horizontal padding (6 each side) + border
/// (1 each side). Measuring the live card at this width keeps the reserved
/// extent equal to what the wrapped composer actually paints (no clipping, no
/// overlap with the next card).
pub(crate) const COMPOSER_INNER_W: f32 = CARD_INNER_W - 14.;
/// The composer box's own vertical chrome: py(2) top+bottom + border(1) both.
const COMPOSER_BOX_CHROME: f32 = 6.;
/// Band kept around the viewport when culling cards to it: a card whose anchor
/// sits within this many px of a visible edge still renders.
const CARD_OVERSCAN: f32 = 120.;
/// Gap kept below the selected card when it is clamped to fit the viewport, so
/// it never sits flush against (or past) the bottom edge.
const CARD_BOTTOM_MARGIN: f32 = 8.;
/// How far inside the near edge `reveal_offscreen` lands a revealed anchor —
/// enough to show the card, small enough that the pill reveals "one more card",
/// not a whole page (the pagination-feel fix). See `reveal_scroll`.
const REVEAL_INSET: f32 = 120.;
/// The lull that ends a typing burst: once the prose has been still this long,
/// a pass that completed mid-burst may land (deferred_pass). ~1s sits inside
/// the natural pause a writer takes at a sentence boundary, so in practice the
/// hold is a breath, not a wait — and the research's caveat applies: these are
/// SELF-requested cards, so err eager, never build a longer artificial hold
/// (attention-motion.md §2, the two-clock verdict pared to its one real rule).
const TYPING_LULL: Duration = Duration::from_millis(1000);
/// A freshly-landed card's entrance: one opacity fade, decelerating (the
/// "enter" easing token, attention-motion.md §3) — the AI voice arrives
/// gently instead of popping. Opacity ONLY (no slide/scale/spring), which is
/// also exactly the prefers-reduced-motion-safe form, and only for cards that
/// are genuinely NEW — a card scrolled back into view is old content and must
/// never re-announce itself (motion = information, or it's noise).
const CARD_APPEAR: Duration = Duration::from_millis(250);
/// A resolved card's exit: a brief accelerating fade of its ghost (the model
/// commits instantly; only the light lingers). Short and ease-in — leaving
/// asks less attention than arriving (attention-motion.md §3, "exit" token).
const CARD_RESOLVE: Duration = Duration::from_millis(150);
/// A surviving card's slide when a re-pack moves it (a card resolves in a
/// crowded lane, a pass lands, a selection expands): the "standard" in-place
/// token — 200ms, cubic in-out (attention-motion.md §3). This is the ONE place
/// motion buys object constancy: an instant jump makes the eye re-find every
/// card; a short slide keeps each one the same object (Heer & Robertson).
const CARD_MOVE: Duration = Duration::from_millis(200);
/// Stagger between cards that start a shared re-pack move — staggering lowers
/// tracking error vs. everything moving at once (attention-motion.md §3).
/// Capped (`MOVE_STAGGER_CAP`) so a long train never turns into a slow wave.
const MOVE_STAGGER: Duration = Duration::from_millis(40);
const MOVE_STAGGER_CAP: u32 = 4;

/// One card's re-pack move in flight (see `Editor::moving` and
/// `update_lane_motion`). Offsets are CONTENT-SPACE (viewport top + scroll):
/// the render applies `delta` to the CURRENT frame's target, so a scroll
/// mid-tween moves the card 1:1 with the text — the slide never fights the
/// writer's own navigation.
#[derive(Clone)]
struct CardMove {
    /// Where the card came from, relative to where it is going: at t=0 the
    /// card renders at `to + delta`, at t=1 at `to`.
    delta: f32,
    /// The content-space top this move heads to. A further re-pack mid-flight
    /// compares against this and re-targets from the currently-displayed spot
    /// (never a snap back to the start).
    to: f32,
    start: Instant,
    /// This card's stagger share within its re-pack round.
    delay: Duration,
    /// Bumped on re-target: keys the element animation, so a new target gets
    /// a fresh animation run instead of resuming the old one's clock.
    generation: u32,
    /// reduce_motion only: a snapshot of the card at its OLD slot, cross-faded
    /// out while the live card fades in at the new one — travel becomes a fade
    /// of the same duration, never a teleport ("reduced motion is not no
    /// motion"; attention-motion.md §4). None in the sliding mode.
    ghost: Option<MarginCard>,
}

impl CardMove {
    fn total(&self) -> Duration {
        self.delay + CARD_MOVE
    }

    /// Progress through the ease at `elapsed`, delay honoured (0 before it).
    fn eased(&self, elapsed: Duration) -> f32 {
        let total = self.total().as_secs_f32();
        let d = self.delay.as_secs_f32() / total;
        staggered_ease((elapsed.as_secs_f32() / total).min(1.), d)
    }
}

/// A completed pass parked until the typing lull (see `Editor::deferred_pass`).
/// Carries the RAW diagnoses (quotes not yet anchored) and the generation that
/// produced them — a cancel or a newer run bumps `ai_generation`, and a stale
/// deferral is dropped at flush by that check alone (one place, by construction).
struct DeferredPass {
    diagnoses: Vec<strop_core::diagnose::Diagnosis>,
    generation: u64,
}
/// At most this many AI (Layer-B) cards render FULL-SIZE at once; past the
/// budget, older passes RECEDE to a one-line card at their anchor — smaller,
/// never hidden (dense marginalia shrink; they don't get filed in a drawer).
/// The honesty invariant this preserves: every flagged passage you can see has
/// a visible card in the margin — a squiggle with no card is indistinguishable
/// from a bug (and was reported as one). FIVE, not seven — Miller's 7±2 is a
/// RECALL span, not a limit on persistent on-screen items (Cowan's ~4 applies
/// to un-chunkable items); ~5 is the researched resting count
/// (docs/attention-motion.md §6). Counted LANE-LOCAL (among the cards actually
/// in this viewport), so a crowded page elsewhere never empties this one. The
/// writer's OWN notes are never budgeted (working memory, not judgments), and
/// the selected card always renders full.
const FULL_DIAGNOSIS_CAP: usize = 5;
/// Fixed height of a receded (collapsed) diagnosis card: one 11px title row
/// plus padding and border. The render forces exactly this height so the
/// packer's no-overlap math and the painted card can never disagree.
const COLLAPSED_CARD_H: f32 = 24.;
/// The prose column's capped width — everything else (centering, the note
/// lane, the narrow-width left-shift) is measured against it.
const COL_MAX_WIDTH: f32 = 660.;
/// Horizontal room the note lane needs to the right of the column: the gap,
/// the lane itself, and the card's 8px inset. The lane is reserved on the
/// right at all times (see `column_frame`) so a note appearing never moves the
/// column; the margin renders inline while this much space exists past it.
const NOTE_LANE_TOTAL: f32 = MARGIN_GAP + MARGIN_WIDTH + 8.;
/// The column is CENTRED at rest (it rhymes with the centred omnibox), and
/// stays centred as long as the right margin can still host the note lane.
/// Only when narrowing past that does it shift left — continuously, no
/// breakpoint — to keep the lane, until it hits this minimum margin and the
/// notes fall back to the pill. With no notes it is always centred. The column
/// x is otherwise a pure function of width: panels overlay, they never push it.
const COL_LEFT_MIN: f32 = 24.;
/// History side panel (DESIGN §2-history): push, not overlay. The panel
/// shrinks before the document does — prose keeps DOC_MIN_WIDTH.
const HISTORY_PANEL_WIDTH: f32 = 320.;
/// The compost rail panel (06 §2): left, push — mirrors the history panel.
const RAIL_PANEL_WIDTH: f32 = 200.;
const DOC_MIN_WIDTH: f32 = 400.;
const CODE_FONT: &str = "PT Mono";
const BAR_HEIGHT: f32 = 36.;
/// The parked-banner refusal pulse (Bug B): the moment label flashes seltint
/// and fades out over this window (the mockup's `.pulseme.hot`, ~900ms). A
/// handful of re-notify frames make the decay visible without a render loop.
const STRIP_PULSE_MS: u64 = 900;
const STRIP_PULSE_FRAMES: u64 = 9;
/// Client-side decorations (H2): thickness of the invisible resize band
/// laid along each window edge/corner. GNOME Wayland grants no server-side
/// borders, so without these strips Strop cannot be resized at all. Strips
/// sit flush on top of the content (not as a reserved inset — that would
/// shift every overlay's window-origin coordinates); the top band doubling
/// as a resize handle over the titlebar is the conventional CSD behavior.
const RESIZE_INSET: f32 = 8.;
/// Client-side decoration shadow gutter (docs/research/window-decorations-csd.md):
/// on Wayland CSD the compositor draws no shadow, so the window blends into the
/// desktop. We reserve a transparent margin on each untiled edge and paint our
/// own soft shadow + rounded corners + hairline border into it.
/// `set_client_inset` keeps hit-testing and overlay geometry correct.
///
/// The gutter must be WIDER than the largest shadow extent, or the blur is
/// clipped at the surface edge and reads as a hard slab (the first cut's bug:
/// a single 0.35-alpha blur whose 10px radius exactly equalled the 10px gutter).
/// A convincing window shadow is layered, not one blur — a tight contact layer
/// that grounds the window plus softer cast/ambient layers biased DOWNWARD (light
/// from above). Values below are restrained, GNOME/libadwaita-scale.
const CSD_GUTTER: f32 = 22.;
const CSD_ROUNDING: f32 = 10.;
/// Auto-cut threshold (docs/impl/02-asides.md §4): a single SELECTION-deletion
/// op removing at least this many chars of manuscript prose files itself in the
/// graveyard automatically — deterministic, and the editor still holds the
/// deleted text at that point. A backspace RUN (many one-char deletes) never
/// reaches this in one op, so it never auto-files (reviews H24 + H43).
const AUTO_CUT_MIN_CHARS: usize = 80;

/// A small hover tooltip: a control's name and, optionally, its chord in a
/// mono chip (DESIGN §0 — every titlebar control should teach its shortcut).
struct Tip {
    label: SharedString,
    chord: Option<SharedString>,
}

impl Render for Tip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .bg(rgb(0xFCFAF4))
            .border_1()
            .border_color(rgb(RULE_COLOR))
            .rounded(px(5.))
            .shadow_md()
            .px(px(8.))
            .py(px(3.))
            .flex()
            .items_center()
            .gap(px(8.))
            .font_family("PT Sans")
            .text_size(px(12.))
            .text_color(rgb(TEXT_COLOR))
            .child(self.label.clone())
            .when_some(self.chord.clone(), |d, chord| {
                d.child(
                    div()
                        .font_family(CODE_FONT)
                        .text_size(px(11.))
                        .text_color(rgb(MUTED_COLOR))
                        .child(chord),
                )
            })
    }
}

/// Build a `.tooltip(…)` closure: hovering the element reveals its name and
/// shortcut. `chord` is the canonical key string from commands.rs, verbatim.
fn tip(
    label: impl Into<SharedString>,
    chord: Option<&'static str>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let label = label.into();
    let chord = chord.map(SharedString::from);
    move |_window, cx| {
        let (label, chord) = (label.clone(), chord.clone());
        cx.new(|_| Tip { label, chord }).into()
    }
}

/// A hairline vertical rule separating button groups in the selection
/// popover (H3): inline marks | headings | footnote.
fn popover_divider() -> gpui::Div {
    div().w(px(1.)).h(px(16.)).mx(px(3.)).bg(rgb(RULE_COLOR))
}

/// One client-side resize handle (H2): an invisible edge/corner strip that
/// starts an interactive resize on press. Static cursor — no per-frame
/// hover tracking, no draw-pass mutation. The caller positions it.
fn resize_strip(
    id: &'static str,
    edge: ResizeEdge,
    cursor: CursorStyle,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .absolute()
        .cursor(cursor)
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            cx.stop_propagation();
            window.start_window_resize(edge);
        })
}

/// The eight client-side resize handles for the current tiling state (H2).
///
/// These ride the OUTER backdrop (whose edge is the window surface), but the
/// VISIBLE window border sits `CSD_GUTTER` in — inside the transparent shadow
/// gutter. The grab band STRADDLES that visible border (reaching `RESIZE_INSET`
/// to each side of it), it does NOT extend out to the external shadow edge:
/// otherwise the whole transparent gutter grabs Strop's resize and hijacks
/// drags meant for a window behind it (Kirill). So the outer shadow stays
/// grab-free and only the edge you can see is draggable, the normal way. The
/// TOP reaches only OUTWARD from its border (never inward) so it can't steal
/// clicks from the titlebar/window controls just inside. A tiled (snapped) edge
/// gets no gutter and no handle.
fn resize_handles(tiling: Tiling) -> Vec<gpui::AnyElement> {
    let out = px(CSD_GUTTER - RESIZE_INSET); // band's outer edge, from the surface
    let thick = px(2. * RESIZE_INSET); // straddle: RESIZE_INSET each side of the border
    let thin = px(RESIZE_INSET); // top: outward-only (border to outer edge)
    let mut v: Vec<gpui::AnyElement> = Vec::new();
    if !tiling.top {
        v.push(
            resize_strip("rz-top", ResizeEdge::Top, CursorStyle::ResizeUpDown)
                .top(out)
                .left(out)
                .right(out)
                .h(thin)
                .into_any_element(),
        );
    }
    if !tiling.bottom {
        v.push(
            resize_strip("rz-bottom", ResizeEdge::Bottom, CursorStyle::ResizeUpDown)
                .bottom(out)
                .left(out)
                .right(out)
                .h(thick)
                .into_any_element(),
        );
    }
    if !tiling.left {
        v.push(
            resize_strip("rz-left", ResizeEdge::Left, CursorStyle::ResizeLeftRight)
                .left(out)
                .top(out)
                .bottom(out)
                .w(thick)
                .into_any_element(),
        );
    }
    if !tiling.right {
        v.push(
            resize_strip("rz-right", ResizeEdge::Right, CursorStyle::ResizeLeftRight)
                .right(out)
                .top(out)
                .bottom(out)
                .w(thick)
                .into_any_element(),
        );
    }
    if !tiling.top && !tiling.left {
        v.push(
            resize_strip("rz-tl", ResizeEdge::TopLeft, CursorStyle::ResizeUpLeftDownRight)
                .top(out)
                .left(out)
                .w(thick)
                .h(thin)
                .into_any_element(),
        );
    }
    if !tiling.top && !tiling.right {
        v.push(
            resize_strip("rz-tr", ResizeEdge::TopRight, CursorStyle::ResizeUpRightDownLeft)
                .top(out)
                .right(out)
                .w(thick)
                .h(thin)
                .into_any_element(),
        );
    }
    if !tiling.bottom && !tiling.left {
        v.push(
            resize_strip(
                "rz-bl",
                ResizeEdge::BottomLeft,
                CursorStyle::ResizeUpRightDownLeft,
            )
            .bottom(out)
            .left(out)
            .w(thick)
            .h(thick)
            .into_any_element(),
        );
    }
    if !tiling.bottom && !tiling.right {
        v.push(
            resize_strip(
                "rz-br",
                ResizeEdge::BottomRight,
                CursorStyle::ResizeUpLeftDownRight,
            )
            .bottom(out)
            .right(out)
            .w(thick)
            .h(thick)
            .into_any_element(),
        );
    }
    v
}

actions!(
    editor,
    [
        Backspace, Delete, DeleteWordLeft, DeleteWordRight, Left, Right, Up, Down, WordLeft,
        WordRight, ParagraphUp, ParagraphDown, SelectLeft, SelectRight, SelectUp, SelectDown,
        SelectWordLeft, SelectWordRight, SelectParagraphUp, SelectParagraphDown, SelectAll, Home,
        End, SelectToHome, SelectToEnd, DocStart, DocEnd, SelectToDocStart, SelectToDocEnd,
        PageUp, PageDown, SelectPageUp, SelectPageDown, Newline, Copy, Cut, Paste, Undo, Redo,
        ToggleStrong, ToggleEmphasis, ToggleUnderline, ToggleStrikethrough, ToggleHighlight,
        ToggleCode, Heading1, Heading2, Heading3, ToggleQuoteBlock, ToggleCodeBlock,
        ToggleBulletList, ToggleOrderedList, AddCheckpoint, ExportMarkdown, InsertFootnote,
        OpenFile, SaveCopyAs, AddNote, RunDiagnosis, RunBelieving, Find, Replace, EscapeMode,
        ToggleHistory, ToggleStrip, TogglePalette, TogglePopover, PaletteUp, PaletteDown, NewDocument,
        RenameDocument, RevealInFiles, CopyDocumentPath, OpenAiConfig, TestAiConnection,
        CancelAiRun, DiagnosisModeDevelopmental, DiagnosisModeLine, DiagnosisModeCopy,
        ShowShortcuts, OpenWelcome, OpenAiSettings, SettingsUp, SettingsDown, SaveAiSettings,
        ToggleOutline, SetSessionGoal, ToggleReview, SetAside, SendToGraveyard,
        ToggleGraveyard,
    ]
);

pub fn bind_keys(cx: &mut App) {
    let ctx = Some("Editor");
    // Commands (anything a menu would list) bind from the registry, so the
    // palette and the keymap can never disagree about a chord.
    let editor_ctx: std::rc::Rc<gpui::KeyBindingContextPredicate> =
        gpui::KeyBindingContextPredicate::parse("Editor").unwrap().into();
    // App-global commands (Command::global) bind to the root "App" context so
    // their chords fire from any focus — palette, note field, settings — not
    // just when the document is focused. Document mutations keep "Editor", so
    // e.g. ctrl-b typed into a field can't bold the document behind it. The
    // matching handlers live on both the root and the editor column (render);
    // the deeper one wins when the document is focused, so neither double-fires.
    let app_ctx: std::rc::Rc<gpui::KeyBindingContextPredicate> =
        gpui::KeyBindingContextPredicate::parse("App").unwrap().into();
    cx.bind_keys(crate::commands::all().iter().filter_map(|cmd| {
        let keys = cmd.keys?;
        let predicate = if cmd.global() {
            app_ctx.clone()
        } else {
            editor_ctx.clone()
        };
        Some(
            KeyBinding::load(
                keys,
                (cmd.make)(),
                Some(predicate),
                false,
                None,
                &gpui::DummyKeyboardMapper,
            )
            .expect("bad chord in command registry"),
        )
    }));
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, ctx),
        // GTK binds this too, "to help with mis-typing" during shift-selection.
        KeyBinding::new("shift-backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        KeyBinding::new("ctrl-backspace", DeleteWordLeft, ctx),
        KeyBinding::new("ctrl-delete", DeleteWordRight, ctx),
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("up", Up, ctx),
        KeyBinding::new("down", Down, ctx),
        KeyBinding::new("ctrl-left", WordLeft, ctx),
        KeyBinding::new("ctrl-right", WordRight, ctx),
        KeyBinding::new("ctrl-up", ParagraphUp, ctx),
        KeyBinding::new("ctrl-down", ParagraphDown, ctx),
        KeyBinding::new("ctrl-shift-up", SelectParagraphUp, ctx),
        KeyBinding::new("ctrl-shift-down", SelectParagraphDown, ctx),
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new("shift-up", SelectUp, ctx),
        KeyBinding::new("shift-down", SelectDown, ctx),
        KeyBinding::new("ctrl-shift-left", SelectWordLeft, ctx),
        KeyBinding::new("ctrl-shift-right", SelectWordRight, ctx),
        KeyBinding::new("ctrl-a", SelectAll, ctx),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("shift-home", SelectToHome, ctx),
        KeyBinding::new("shift-end", SelectToEnd, ctx),
        KeyBinding::new("ctrl-home", DocStart, ctx),
        KeyBinding::new("ctrl-end", DocEnd, ctx),
        KeyBinding::new("ctrl-shift-home", SelectToDocStart, ctx),
        KeyBinding::new("ctrl-shift-end", SelectToDocEnd, ctx),
        KeyBinding::new("pageup", PageUp, ctx),
        KeyBinding::new("pagedown", PageDown, ctx),
        KeyBinding::new("shift-pageup", SelectPageUp, ctx),
        KeyBinding::new("shift-pagedown", SelectPageDown, ctx),
        KeyBinding::new("enter", Newline, ctx),
        KeyBinding::new("shift-enter", Newline, ctx),
        KeyBinding::new("ctrl-c", Copy, ctx),
        KeyBinding::new("ctrl-x", Cut, ctx),
        KeyBinding::new("ctrl-v", Paste, ctx),
        // CUA legacy, still alive in every Linux toolkit.
        KeyBinding::new("ctrl-insert", Copy, ctx),
        KeyBinding::new("shift-delete", Cut, ctx),
        KeyBinding::new("shift-insert", Paste, ctx),
        // Redo's CUA alias; the primary chord comes from the registry.
        KeyBinding::new("ctrl-y", Redo, ctx),
        // Silent legacy heading aliases (DESIGN §2-toolbar): ctrl-1..3 is
        // the promoted chord; these keep working but the UI never shows them.
        KeyBinding::new("ctrl-alt-1", Heading1, ctx),
        KeyBinding::new("ctrl-alt-2", Heading2, ctx),
        KeyBinding::new("ctrl-alt-3", Heading3, ctx),
        KeyBinding::new("escape", EscapeMode, ctx),
        // GNOME's menu key opens the palette — it IS the menu.
        KeyBinding::new("f10", TogglePalette, ctx),
        KeyBinding::new("enter", FieldCommit, Some("NoteInput")),
        KeyBinding::new("escape", FieldCancel, Some("NoteInput")),
        KeyBinding::new("backspace", FieldBackspace, Some("NoteInput")),
        KeyBinding::new("ctrl-backspace", FieldBackspaceWord, Some("NoteInput")),
        KeyBinding::new("tab", FieldTab, Some("NoteInput")),
        // DESIGN §0.6 law 1: the focused field owns the paste chord. The
        // deeper context outranks "Editor", so ctrl-v can no longer fall
        // through a field into the document behind it.
        KeyBinding::new("ctrl-v", FieldPaste, Some("NoteInput")),
        KeyBinding::new("shift-insert", FieldPaste, Some("NoteInput")),
        // The multi-line note composer: Enter commits (the fast jot gesture),
        // shift/ctrl-enter add a line break, up/down walk caret rows. The base
        // chords match NoteInput; the extras are bound below via the helper.
        KeyBinding::new("enter", FieldCommit, Some("NoteComposer")),
        KeyBinding::new("escape", FieldCancel, Some("NoteComposer")),
        KeyBinding::new("backspace", FieldBackspace, Some("NoteComposer")),
        KeyBinding::new("ctrl-backspace", FieldBackspaceWord, Some("NoteComposer")),
        KeyBinding::new("tab", FieldTab, Some("NoteComposer")),
        KeyBinding::new("ctrl-v", FieldPaste, Some("NoteComposer")),
        KeyBinding::new("shift-insert", FieldPaste, Some("NoteComposer")),
        // The palette's query field: same editing actions, plus row motion.
        KeyBinding::new("enter", FieldCommit, Some("PaletteInput")),
        KeyBinding::new("escape", FieldCancel, Some("PaletteInput")),
        KeyBinding::new("backspace", FieldBackspace, Some("PaletteInput")),
        KeyBinding::new("ctrl-backspace", FieldBackspaceWord, Some("PaletteInput")),
        KeyBinding::new("ctrl-v", FieldPaste, Some("PaletteInput")),
        KeyBinding::new("shift-insert", FieldPaste, Some("PaletteInput")),
        KeyBinding::new("up", PaletteUp, Some("PaletteInput")),
        KeyBinding::new("down", PaletteDown, Some("PaletteInput")),
        // Find mode: tab hops to the replace field, ctrl-h summons it. Both
        // bubble to the root's on_action handlers (the omnibox lives outside
        // the Editor key context, like the bottom strips it replaced).
        KeyBinding::new("tab", FieldTab, Some("PaletteInput")),
        KeyBinding::new("ctrl-h", Replace, Some("PaletteInput")),
        // The AI settings panel's fields (F4): tab cycles, enter commits
        // (test, or pick from the model list), up/down walk the list,
        // ctrl-enter saves, escape closes.
        KeyBinding::new("enter", FieldCommit, Some("SettingsInput")),
        KeyBinding::new("escape", FieldCancel, Some("SettingsInput")),
        KeyBinding::new("backspace", FieldBackspace, Some("SettingsInput")),
        KeyBinding::new("ctrl-backspace", FieldBackspaceWord, Some("SettingsInput")),
        KeyBinding::new("ctrl-v", FieldPaste, Some("SettingsInput")),
        KeyBinding::new("shift-insert", FieldPaste, Some("SettingsInput")),
        KeyBinding::new("tab", FieldTab, Some("SettingsInput")),
        KeyBinding::new("up", SettingsUp, Some("SettingsInput")),
        KeyBinding::new("down", SettingsDown, Some("SettingsInput")),
        KeyBinding::new("ctrl-enter", SaveAiSettings, Some("SettingsInput")),
    ]);
    // Universal text-field editing (Kirill's rule — universal gestures stay
    // universal): caret movement, selection, delete-forward, copy/cut, applied
    // to EVERY field at once. Palette/Settings keep up/down for list-nav, so
    // only the multi-line composer gets vertical caret rows + line breaks.
    for ctx in ["NoteInput", "NoteComposer", "PaletteInput", "SettingsInput"] {
        cx.bind_keys(crate::text_field::field_editing_bindings(ctx));
    }
    cx.bind_keys(crate::text_field::composer_only_bindings("NoteComposer"));
}


/// How the writer is engaging the margin right now. There is one keyboard
/// focus and one composer, so a card can be in exactly one of these states at
/// a time — and only ONE card can be selected or composed at once. Encoding
/// that as a single enum (instead of the old `active_note` +
/// `composing_note` + `note_input` trio) makes the desync states that caused
/// real, persisted bugs **unrepresentable**:
///
/// - a composer floating on a card that is no longer selected,
/// - a committed note rendering blank (composer gone, body still suppressed),
/// - a draft leaking onto whatever card was clicked instead of the one being
///   edited.
///
/// Every one of those was two booleans that drifted apart. Here they cannot:
/// the composer's identity and its `NoteInput` live in the same variant, and
/// the SINGLE exit from `Composing` (`resolve_composer`) persists the draft to
/// the note it actually belongs to. New interaction states force every
/// `match` below to be updated (exhaustiveness), so this class can't silently
/// regrow. See `card_body` for the render-side counterpart.
enum CardFocus {
    /// No card selected.
    Idle,
    /// A card is highlighted and raised to the top, but not editable. AI
    /// diagnoses only ever reach this (their bodies are immutable); a note
    /// lands here after its composer resolves.
    Selected(u64),
    /// A note's body is open in the composer. The draft mirror and the
    /// composer render read the target id from here, so neither can ever
    /// address a different card.
    Composing { id: u64, input: Entity<TextField> },
}

impl CardFocus {
    /// The selected/composed card, if any — what gets the highlight and the
    /// top z-order. (`active_note` of old.)
    fn active_id(&self) -> Option<u64> {
        match self {
            CardFocus::Idle => None,
            CardFocus::Selected(id) | CardFocus::Composing { id, .. } => Some(*id),
        }
    }

    /// The note whose composer is open, if one is. (`composing_note` of old.)
    fn composing_id(&self) -> Option<u64> {
        match self {
            CardFocus::Composing { id, .. } => Some(*id),
            _ => None,
        }
    }

    /// The open composer's input entity, if composing. (`note_input` of old.)
    fn input(&self) -> Option<&Entity<TextField>> {
        match self {
            CardFocus::Composing { input, .. } => Some(input),
            _ => None,
        }
    }
}

/// What a card paints in its body region: exactly one of a composer or the
/// note's text — never both (the "double" bug), never neither (the "blank
/// committed card" bug). A two-variant enum makes "exactly one" structural,
/// where the old code used two independent `.when()` conditions that could
/// both fire or both stay silent. Pure and total: see `card_body`.
enum CardBody {
    /// The note is being composed here — paint the input.
    Composer,
    /// Paint the committed text (or the empty-note placeholder for a blank
    /// writer's note; diagnoses are never blank-placeheld).
    Text,
}

/// Decide a single card's body region from whether its own composer is open.
/// Trivial by construction — that is the point: the bug existed because the
/// choice was spread across two booleans that could disagree. One input, one
/// of two outputs, no way to render both or neither.
fn card_body(composing_here: bool) -> CardBody {
    if composing_here {
        CardBody::Composer
    } else {
        CardBody::Text
    }
}

/// Which read a pass runs (impl 04 §0, review H27). The old `run_pass(bool)`
/// could name only believing-vs-diagnosis; the doubting read is neither, and a
/// `"doubting"` *mode string* silently produced the LINE prompt (`system_prompt`
/// maps every unknown mode to line). A bool also can't survive the
/// deferred/pending round-trips — `pending_pass`/`last_pass` collapse doubting
/// onto believing. So the pass identity is an enum, threaded end to end.
///
/// `Diagnostic` carries its levels-of-edit mode WITH it, which is the fix for
/// the sticky-mode trap (review mid): a menu row pins its depth into the run
/// itself rather than mutating the persistent `diagnosis_mode` session override
/// (which would silently re-aim the next ctrl-shift-d).
#[derive(Clone, PartialEq, Eq, Debug)]
enum PassKind {
    /// Elbow's believing game — strengths named as mechanisms.
    Believing,
    /// The believing read's mirror — the strongest case against it.
    Doubting,
    /// The Perkins diagnosis at a levels-of-edit depth
    /// (developmental / line / copy — the string `effective_mode` yields).
    Diagnostic(String),
}

impl PassKind {
    /// The journal Pass event's mode string ("believing" / "doubting" /
    /// mode) — recorded by `integrate_pass`, drawn as the strip's veil label.
    fn mode_str(&self) -> &str {
        match self {
            PassKind::Believing => "believing",
            PassKind::Doubting => "doubting",
            PassKind::Diagnostic(mode) => mode,
        }
    }

    /// The Running-card label stem: "{} · {model}" is completed by `run_pass`.
    /// Speaks the MENU's grammar ("a read") — the writer clicked "A line
    /// read", so the running card names the same thing; "pass"/"diagnosis"
    /// are internal registers (ux-glossary) and never print.
    fn run_label(&self) -> String {
        match self {
            PassKind::Believing => "believing read".to_owned(),
            PassKind::Doubting => "doubting read".to_owned(),
            PassKind::Diagnostic(mode) => format!("{mode} read"),
        }
    }
}

/// The editor button's face (impl 04 §0, review H31/H32). The real state is
/// multi-dimensional — AI lifecycle × a parked read × the door — so the face is
/// a PRIORITY over it, not four exclusive states: a cooking pass while the door
/// is open must resolve to ONE face, and Error/NeedsSetup can't fall through to
/// a bare idle button while `render_ai_status` shows the failure. The door word
/// is the glossary's presence pair (`Reading`), never the internal "Reviewing".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum EditorFace {
    /// No provider configured — routes to setup (the card lives on render_ai_status).
    NeedsSetup,
    /// The last read failed — hover carries the message; the card stays too.
    Error,
    /// A read is cooking — pulse dot; hover names the read.
    Cooking,
    /// A completed read is parked behind the reveal clock ("a read is ready").
    Ready,
    /// The door is open — "Reading · {n} open".
    Reading,
    /// Drafting, nothing pending — the bare "Ask the editor ▾".
    Idle,
}

impl EditorFace {
    /// The rig token (dump `editor_btn.face`), so a smoke test can assert the
    /// priority transitions without eyeballing the pixels.
    fn token(self) -> &'static str {
        match self {
            EditorFace::NeedsSetup => "needs_setup",
            EditorFace::Error => "error",
            EditorFace::Cooking => "cooking",
            EditorFace::Ready => "ready",
            EditorFace::Reading => "reading",
            EditorFace::Idle => "idle",
        }
    }
}

/// The inputs the face is a pure function of. `count` shapes only the Reading
/// LABEL, never the priority, so it is not an input here.
struct FaceInputs {
    needs_setup: bool,
    error: bool,
    cooking: bool,
    ready: bool,
    door_open: bool,
}

/// The priority function itself, pure and table-tested. Order is the spec's:
/// NeedsSetup > Error > cooking > read-ready > Reading·N > idle.
fn face_for(i: &FaceInputs) -> EditorFace {
    if i.needs_setup {
        EditorFace::NeedsSetup
    } else if i.error {
        EditorFace::Error
    } else if i.cooking {
        EditorFace::Cooking
    } else if i.ready {
        EditorFace::Ready
    } else if i.door_open {
        EditorFace::Reading
    } else {
        EditorFace::Idle
    }
}

/// The gutter-float grid's fixed width (docs/impl/03-flanks.md §0.1): wide
/// enough for two toggle columns AND the three-wide heading block row. The
/// gutter-fit threshold keys off it (finding 90 — recomputed for the 2-column
/// grid, wider than the old 1×N stack), so a window whose left gutter can't hold
/// it falls back to the formatting-only horizontal popover instead.
const FLANK_GRID_W: f32 = 100.;
/// Estimated grid height (six rows + two seams), to keep the float on-screen.
const FLANK_GRID_H: f32 = 210.;

/// A right-flank verb (docs/impl/03-flanks.md §0.2). Carried by value into the
/// row's click listener so the four rows share one dispatch, each routing to an
/// existing path — note composer, the P2 aside/graveyard verbs, the scoped ask.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SelVerb {
    Note,
    Aside,
    Graveyard,
    Ask,
}

/// The LEFT (formatting) flank's form (docs/impl/03-flanks.md §1, review H21).
/// It always rises for a live selection — only its shape narrows: the closed-set
/// grid in a real gutter, the formatting-only horizontal popover where the
/// window is too narrow to float one (the verbs stay palette-reachable there).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FlankLeft {
    /// Suppressed — a history surface is up (the past is read-only, review H22).
    None,
    /// The closed-set grid, floating in the left gutter (the resting form).
    Grid,
    /// The formatting-only horizontal popover above the line (narrow fallback).
    Horizontal,
}

/// Which flanks rise, as one decision (docs/impl/03-flanks.md §1-2). Both are a
/// consequence of chirality: tools left, the conversation right (asides.md §4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct FlankGate {
    left: FlankLeft,
    /// The right verb menu. Manuscript-only, needs a live lane, no history —
    /// see `flank_gate`.
    right: bool,
}

/// The single gating predicate (docs/impl/03-flanks.md §1-2, reviews H21/H22/B8,
/// findings 57/91/108). Pure over booleans so the test sweeps every
/// region × history × width case without a live frame.
fn flank_gate(history_up: bool, in_compost: bool, lane_available: bool, gutter_ok: bool) -> FlankGate {
    if history_up {
        // A history surface claims the right side AND the past is not editable —
        // neither flank may offer live mutation over a read-only state (H22).
        return FlankGate {
            left: FlankLeft::None,
            right: false,
        };
    }
    FlankGate {
        // The formatting flank always rises; only its FORM narrows.
        left: if gutter_ok { FlankLeft::Grid } else { FlankLeft::Horizontal },
        // The verb menu needs the right lane to occlude into (B8) and is
        // MANUSCRIPT-only — a compost-rail selection is the writer's private
        // scrap box, no verb menu (spec §1, finding 108). Its fallback keys on
        // the LANE's own fit (`margin_fits`), never the left gutter (finding 57):
        // there is a width band where the gutter is gone but the lane remains,
        // and vice versa.
        right: lane_available && !in_compost,
    }
}

/// The per-frame flank geometry + gating, computed once (`flank_layout`) so the
/// left popover, the right menu, and the rig dump all agree on presence and y.
/// The two y bases differ ON PURPOSE (review finding 89): `left_top` co-registers
/// with the prose column (CSD-corrected content space), `right_top` co-registers
/// with the margin CARDS (raw frame y, no inset) — sharing one would drift the
/// menu a gutter (22px) off the cards it means to occlude on floating windows.
struct FlankLayout {
    gate: FlankGate,
    /// Content-space top of the selection's first visual line — the LEFT flank.
    left_top: f32,
    /// LANE-space top (raw frame y) — the RIGHT menu, co-registered with cards.
    right_top: f32,
    line_h: f32,
    /// Caret x within its line (for centring the horizontal fallback).
    x: f32,
    /// The prose column's left edge, content space.
    col_left: f32,
    /// Width the flanks live in (viewport minus CSD gutters) and its height.
    vw: f32,
    vh: f32,
    rail_w: f32,
    /// The note lane's left edge, content space (the right menu pins here).
    lane_left: f32,
}

pub struct Editor {
    focus_handle: FocusHandle,
    /// Text + formatting with unified transaction history. Marks persist
    /// via Store::save_with_marks at save time.
    doc: Document,
    /// Sticky caret formatting: toggles made with an empty selection apply
    /// to the next typed text. (attr, on) overrides what the position would
    /// inherit; cleared by any caret motion.
    caret_attrs: Vec<(InlineAttr, bool)>,
    /// Selection in UTF-8 byte offsets; the cursor is `end` unless reversed.
    selected_range: Range<usize>,
    selection_reversed: bool,
    /// When the cursor offset sits exactly on a soft-wrap boundary: false =
    /// end of the upper visual line, true = start of the lower one.
    cursor_affinity_down: bool,
    /// Preferred x for consecutive vertical moves. Cleared by any other motion.
    goal_x: Option<Pixels>,
    marked_range: Option<Range<usize>>,
    is_selecting: bool,
    /// Drag-selection granularity, set by click count (GTK SELECT_* modes).
    select_granularity: SelectGranularity,
    /// The unit selected by the initiating double/triple click; drag unions
    /// the unit under the pointer with this.
    selection_origin: Option<Range<usize>>,
    /// Document-space offset of the viewport top. Clamped at prepaint.
    scroll_top: Pixels,
    /// When set, the next prepaint scrolls the cursor into view. Set by any
    /// cursor-moving input; never by wheel scrolling (scroll never steals
    /// the caret, the caret never blocks scrolling).
    autoscroll_request: bool,
    /// Last pointer position during a drag, for edge autoscrolling.
    drag_point: Option<Point<Pixels>>,
    autoscroll_active: bool,
    cursor_visible: bool,
    last_input: Instant,
    /// Durable layer; edits mirror into it, a background task saves when idle.
    store: Option<Store>,
    store_dirty: bool,
    /// History mode (rewind v2): list + read-only diff preview.
    history_view: Option<HistoryView>,
    history_preview: Option<PreviewDoc>,
    /// A legacy-checkpoint backfill is in flight (one at a time; the result
    /// persists, so it can never need to run twice for the same file).
    history_backfill_running: bool,
    /// Edits since the last checkpoint — idle-gap session sealing.
    dirty_since_checkpoint: bool,
    /// Encoded image assets by id; Arc<gpui::Image> handles feed GPUI's
    /// decode-once cache via use_render_image.
    image_assets: HashMap<String, Arc<gpui::Image>>,
    /// User settings (config.toml), loaded at startup.
    pub config: Config,
    /// The writer's current engagement with the margin: nothing, a selected
    /// card, or a note open in the composer. One field for what used to be
    /// `active_note` + `composing_note` + `note_input`, so they cannot drift
    /// out of sync (see `CardFocus`).
    focus: CardFocus,
    /// AI pass lifecycle, rendered as the margin's first card (PLAN.md
    /// E3): teaches setup, shows progress, names failures actionably.
    ai_status: Option<AiStatus>,
    /// Bumped on every run and on cancel: an in-flight response from an
    /// older generation is silently dropped.
    ai_generation: u64,
    /// Monotonic review-pass counter; each successful diagnosis pass bumps it
    /// and stamps its new cards' `pass_id`, so a later pass can rest older ones.
    diagnosis_pass: u64,
    /// Session override of the levels-of-edit depth; None = config's
    /// [ai].mode (the thesis switch, editorial-foundations §2.2).
    diagnosis_mode: Option<String>,
    /// What the last pass was, so an error card's Retry can repeat it exactly
    /// (believing / doubting / a diagnosis at its pinned depth).
    last_pass: PassKind,
    /// A pass the writer asked for before a provider existed. Finishing setup —
    /// the panel's Save, or the one-click local-model path — consumes it and
    /// runs THIS pass, so the request that *triggered* setup is the request
    /// that gets answered (no "now press ctrl-shift-d again" dead end). Carries
    /// the full `PassKind`, so a doubting/menu request survives the detour.
    pending_pass: Option<PassKind>,
    /// A completed pass whose results arrived MID-TYPING-BURST: integrating
    /// would pop squiggles into the very sentence being typed and re-pack the
    /// lane — an involuntary peripheral onset (attention-motion.md §2). Held
    /// un-anchored (raw `Diagnosis` quotes, so anchoring runs against the
    /// text as it stands at reveal, not at arrival) until the burst ends
    /// (`TYPING_LULL`) or the writer turns away (scroll, door). This is the
    /// WHOLE reveal clock: one rule, no gaze tracking, no idle timers.
    deferred_pass: Option<DeferredPass>,
    /// When the prose text last changed (set at the `sync_mutations`
    /// chokepoint — real buffer ops only, not caret moves or card clicks).
    /// `deferred_pass` reads it to tell a live typing burst from a lull.
    last_text_edit: Option<Instant>,
    /// Cards from the pass that JUST landed (`integrate_pass`), still inside
    /// their entrance fade (`CARD_APPEAR`). Cleared by a timer right after the
    /// fade completes, so a later scroll-out/in can never replay the entrance.
    /// Writer notes never enter here — your own keystroke is instant.
    appearing: std::collections::HashSet<u64>,
    /// Ghosts of just-resolved cards, mid exit-fade (`CARD_RESOLVE`): the
    /// rendered snapshot + when it started. Painted UNDER the live lane,
    /// non-interactive, dropped by a timer (and by any scroll — the snapshot
    /// is viewport-frozen). The note itself resolved instantly; this is
    /// presentation only, so nothing here can leak back into the model.
    departing: Vec<(MarginCard, Instant)>,
    /// Re-pack moves in flight, card id → its slide (`CardMove`). Fed by
    /// `update_lane_motion` (the render pre-pass that diffs the lane between
    /// frames); read by `render_margin`, which offsets each moving card from
    /// its CURRENT target. Presentation only — the packer's output is always
    /// the truth; this is just how the eye is walked from the old truth to
    /// the new one. Cleared wholesale on any snap frame (scroll, composer
    /// growth, typing burst): motion yields to the writer's own movement.
    moving: std::collections::HashMap<u64, CardMove>,
    /// Each visible card's last settled CONTENT-SPACE top (viewport top +
    /// scroll) — what `update_lane_motion` diffs the current pack against to
    /// tell a discrete re-pack (tween it) from scroll (already 1:1).
    lane_tops: std::collections::HashMap<u64, f32>,
    /// The scroll offset `lane_tops` was recorded at; a change marks the
    /// frame as a scroll frame (snap, never tween).
    lane_scroll: f32,
    /// The viewport size ditto: a resize drag reflows the prose and re-clamps
    /// the lane continuously — snap through it, never chase it with tweens.
    lane_viewport: (f32, f32),
    /// Session-monotonic count of re-pack slides ever started — the rig
    /// asserts "a resolve moved the survivors" against this instead of racing
    /// the 200ms window with a live `moving` snapshot (a cold first launch
    /// made that dump miss the flight and flake).
    moves_started: u64,
    /// The door (DESIGN §4.4; core-loop research: separate GENERATE from
    /// EVALUATE). `true` = drafting, door closed: the editorial margin goes
    /// quiet so a writing burst is never pulled into evaluation by its own
    /// earlier diagnoses (King's "door closed"; Elbow — premature editing
    /// "damps out the voice"). `false` = reviewing, door open: diagnosis
    /// cards surface. Default closed; a document opens to write. Running a
    /// pass, reaching for an anchor, or the rail opens it. The writer's own
    /// ctrl-m notes are NEVER hidden — the door quiets the editor, not the
    /// writer's marginalia. In-memory (per session), no stored mode.
    drafting: bool,
    /// The omnibox (DESIGN §2-omnibox, PLAN.md E1): one summoned-not-mounted
    /// top-centre field that finds (plain), runs commands (`>`) or jumps to
    /// headings (`@`). `palette_*` is its backing state across all modes.
    palette_input: Option<Entity<TextField>>,
    palette_selected: usize,
    /// The omni-list's scroll state (`render_omni`'s "omni-list" div),
    /// persisted across frames so it survives the per-frame rebuild. Mouse
    /// wheel scrolling drives it directly (GPUI's normal `track_scroll`
    /// wiring); keyboard navigation additionally nudges it via
    /// `omni_scroll_into_view` so the selected row can't wander past the
    /// visible window — a `scroll_to_item` request is a no-op when the row
    /// is already on-screen, so it never fights a manual wheel scroll.
    omni_scroll: ScrollHandle,
    /// Mirror of the palette query (debug_cursor has no `cx` to read the
    /// input entity); maintained by the palette's observe hook.
    palette_query: String,
    /// The selection the omnibar opened on (06 §1, critique S3/P13): the
    /// find preview walks `selected_range` across matches as you type, so
    /// Esc must walk it home — cancel restores this, Enter-executed jumps
    /// don't (travel was the point), and click-away doesn't either (the
    /// click placed a new caret on purpose).
    omni_return: Option<Range<usize>>,
    /// Per-command execution counts (DESIGN §3.3, hit-frequency
    /// ordering): loaded from disk at palette open, written through on
    /// every palette execution — the palette becomes *your* instrument.
    palette_freq: HashMap<String, u32>,
    /// The chord whisper (DESIGN §3.5, VimGolf's engine): after a palette
    /// execution of a chorded command, one muted bottom-right one-liner
    /// names the chord, then fades. Bederson's flow rules cap it at once
    /// per session (the bool); the generation guards the fade timer.
    chord_whisper: Option<String>,
    chord_whisper_shown: bool,
    chord_whisper_generation: u64,
    /// In-titlebar document rename (PLAN.md E2).
    doc_rename_input: Option<Entity<TextField>>,
    /// The keyboard-map overlay (PLAN.md E4, ctrl-?).
    shortcuts_open: bool,
    /// The AI settings panel (DESIGN §2-ai, F4): form + async test +
    /// /models picker; saves write through toml_edit.
    ai_settings: Option<AiSettings>,
    /// Bumped on every panel-spawned request and on close, so a stale
    /// test/list response from a closed panel is dropped silently.
    ai_settings_generation: u64,
    /// Replace field (ctrl-h adds it under the omnibox query): Enter on it
    /// replaces the current match; the All button replaces every match (one
    /// undo). The current-match index is `palette_selected` (find mode).
    replace_input: Option<Entity<TextField>>,
    /// Rename-in-place for a history row: (entry index, composer).
    rename_input: Option<(usize, Entity<TextField>)>,
    /// Alt-text composer for an image block: (block index, composer).
    alt_input: Option<(usize, Entity<TextField>)>,
    /// Self-baseline from the [voice] corpus (None until >=3 docs load).
    pub voice_baseline: Option<strop_core::voice::Baseline>,
    /// Narrow-window notes drawer (DESIGN §narrow-margin): below ~932px even
    /// a left-shifted column can't host the 248px lane, so the cards would
    /// vanish. Instead a top-right pill shows the count (never silent) and
    /// this toggles the drop-down panel that lists them.
    narrow_notes_open: bool,
    /// The editor button's dropdown (impl 04 §0): the subsystem's single home,
    /// glued flush under the titlebar control. A bool-toggled overlay, light-
    /// dismissed like the narrow-notes panel it borrows its anchoring from.
    editor_menu_open: bool,
    /// The editor button's PAINTED right edge (window coords), written by a
    /// bounds canvas inside the control each frame (the zone_row_bounds
    /// idiom — a plain shared cell, never an entity write mid-draw). The
    /// dropdown reads it to stay glued right-edge-to-right-edge with its
    /// button: the fixed-width estimate it replaced assumed the chrome right
    /// of the button never changes, but those controls flex-shrink in a
    /// narrow bar, which left the menu floating ~75px off its control.
    editor_btn_right: std::rc::Rc<std::cell::Cell<Option<Pixels>>>,
    /// Selection popover (DESIGN §2-toolbar): formatting rides the
    /// selection. Shown on mouse-up over a selection or via ctrl-.;
    /// dismissed by mousedown, typing, scrolling, or escape.
    selection_popover: bool,
    /// The left flank's link argument-field (docs/impl/03-flanks.md §0.1,
    /// review B2 + findings 59/88): the URL editor opened from the grid's link
    /// cell. It CAPTURES the target CHAR range at open — never a re-read of
    /// `selected_range` at commit, which could have drifted (Link is
    /// non-expanding, `expands()` == false). While it is `Some` the field owns
    /// the flank and keyboard focus, so "typing dismisses" (an EDITOR-focus rule)
    /// can't fire over the URL; commit applies `doc.set_link`, an empty commit
    /// removes the link.
    link_input: Option<(Range<usize>, Entity<TextField>)>,
    /// Titlebar word count, recomputed on mutation — never per frame.
    word_count: usize,
    /// The compost rail (docs/impl/06-omnibar.md §2): the toggleable left
    /// panel listing the compost's items, session-only. The outline it
    /// replaced is gone — heading navigation lives in the palette's `@` mode.
    rail_open: bool,
    /// The graveyard tail section (docs/impl/02-asides.md §4, Bug B): entries
    /// the writer clicked to expand out of their one-line receded form. The
    /// newest entry is always full; older ones recede until expanded. The old
    /// drop-up overlay + `graveyard_open` toggle are gone — the record now lives
    /// in the scroll flow at the document tail. Session-only.
    grave_expanded: Vec<u64>,
    /// The last caret byte offset while it was in the MANUSCRIPT — so Esc from
    /// a compost-rail caret returns exactly there (review B3; asides.md §2.1).
    last_manuscript_caret: usize,
    /// One-shot blink of the compost rail edge on an arrival (aside / orphan
    /// migration) — the same "something arrived over there" grammar the
    /// graveyard bar uses. `None` at rest.
    rail_flash: Option<Instant>,
    /// One-shot blink of the graveyard bar on an exile (a fresh cut filed).
    grave_flash: Option<Instant>,
    /// One-shot pulse of the parked history banner's moment label (Bug B): an
    /// edit attempt while previewing the past does NOTHING to the text and
    /// flashes this instead — the mockup's `.pulseme.hot` (seltint, ~900ms
    /// fade-out). The read-only mode is visible and its refusal is legible
    /// (P2/P4). `None` at rest; the fade alpha is a pure function of elapsed.
    strip_pulse: Option<Instant>,
    /// One-shot blink of a manuscript paragraph (the `originflash` idiom): the
    /// block containing this char offset tints briefly. Set by "show origin"
    /// (reveal where a cut came from) and by Put back (the passage returned).
    /// Painted LIVE (read in `paint`), so it clears on the next frame without a
    /// layout rebuild. `None` at rest.
    para_flash: Option<(usize, Instant)>,
    /// store_dirty was set at least once this session — a plain "did the writer
    /// touch this document" flag (the rig's `edits=` tag). Re-entry via the
    /// intent question was retired (impl 04 §1); this now only reports activity.
    session_had_edits: bool,
    /// Session word goal (DESIGN §4.2): (target, word_count at set time).
    /// Session-only — per-session progress, never lifetime totals.
    session_goal: Option<(usize, usize)>,
    /// The "Set Session Goal…" composer (bottom strip).
    goal_input: Option<Entity<TextField>>,
    /// Painted bounds of each footnote-zone row's text area (captured by a
    /// canvas child at paint time), so a click on the mirror maps to the
    /// same offset in the def line (DESIGN §2-footnotes, the Word
    /// notes-pane behavior).
    /// Written by the zone rows' bounds-capture canvas during prepaint.
    /// MUST be an Rc<RefCell>, not entity state: mutating the Editor
    /// entity from inside a draw pass (handle.update in a canvas closure)
    /// re-dirties the window mid-frame, and under Wayland's frame-callback
    /// scheduling that tore the renderer's per-frame sprite bookkeeping —
    /// the cross-surface glyph corruption of 2026-06-12.
    zone_row_bounds: std::rc::Rc<std::cell::RefCell<HashMap<usize, Bounds<Pixels>>>>,
    /// Measured margin-card heights, keyed by content hash (see
    /// `refresh_card_heights`). A diagnosis's content is immutable and a note's
    /// changes only at a composer commit, so a card's real shaped height is
    /// measured once at the lane width and cached — replacing the char-count
    /// estimate that under-sized tall cards and let them overlap. Read by
    /// `margin_cards`; refreshed in `render`, where the text system is in hand.
    card_heights: HashMap<u64, f32>,
    /// The actively-composed note's live height — its composer text changes
    /// every keystroke, so it can't ride the content-hash cache. Measured each
    /// frame in `refresh_card_heights`; `None` when no note is composing.
    active_card_height: Option<f32>,
    last_frame: Option<TextFrame>,
    /// The history strip (P1 — docs/impl/01-history-strip.md): the bottom
    /// seek-bar surface. `strip.open` gates its overlay; `strip.parked` drives
    /// the read-only preview + the Restore/Now controls. The immutable bake
    /// and the mutable scrub state live apart inside it (the stability law).
    strip: Strip,
    /// The strip's painted rail geometry (WINDOW coords), captured every paint
    /// so the scrub mouse handlers map a pointer x → working px → pos_ms
    /// against the SAME bounds the fabric drew — CSD-inset-safe without any
    /// re-derivation. `Rc<RefCell>`, never entity state: it is written from
    /// inside the draw pass (the 2026-06-12 corruption rule, like
    /// `zone_row_bounds`).
    strip_rail: std::rc::Rc<std::cell::RefCell<Option<StripGeom>>>,
}

#[derive(Clone, Copy, PartialEq)]
enum SelectGranularity {
    Char,
    Word,
    Paragraph,
}

/// Geometry of the last painted frame, for mouse, IME, and vertical-motion
/// mapping. Rebuilt on every paint.
struct TextFrame {
    bounds: Bounds<Pixels>,
    line_height: Pixels,
    scroll_top: Pixels,
    content_height: Pixels,
    paragraphs: Vec<ParagraphLayout>,
    /// The graveyard tail section's pre-shaped rows (Bug B): painted after the
    /// last paragraph, inside the scroll flow. Carries each row's click targets,
    /// so `on_mouse_down` hit-tests the verbs (show origin / put back / delete /
    /// expand) directly. Empty when the graveyard is.
    grave_lines: Vec<GraveLine>,
    /// Doc-space top of the graveyard section header (`None` = no section), for
    /// the footer bar's hide-when-visible gate (`grave_tail_on_screen`).
    grave_section_top: Option<Pixels>,
    /// The key the paragraphs were laid out for; the next prepaint reuses them
    /// when its key matches (see `LayoutKey`).
    layout_key: LayoutKey,
}

/// A verb on the graveyard tail section (Bug B). Click targets carry one.
#[derive(Clone, Copy)]
enum GraveAction {
    ShowOrigin(u64),
    PutBack(u64),
    Delete(u64),
    /// Expand/collapse a receded entry in place.
    Expand(u64),
}

/// One pre-shaped row of the graveyard tail section: segments laid left-to-right
/// (each with its x from the frame origin), the row's doc-space top, and any
/// click targets (doc-space rects, so `on_mouse_down` tests them verbatim).
struct GraveLine {
    /// (x from `bounds.origin.x`, shaped segment) — header/whisper/receded rows.
    segments: Vec<(Pixels, gpui::ShapedLine)>,
    /// A wrapped body paragraph of the full cut text (dimmed, ruled): `(x, line)`.
    /// When set, `segments` is empty. `WrappedLine` isn't `Clone`, so grave_lines
    /// are moved (never cloned) on the reuse fast-path.
    body: Option<(Pixels, gpui::WrappedLine)>,
    /// Doc-space top (relative to `bounds.origin`, pre-scroll).
    top: Pixels,
    height: Pixels,
    line_height: Pixels,
    /// Draw the 3px stale left-rule beside this row (the full cut-text body).
    left_rule: bool,
    /// The section-header row: draws the tombstone slab (+ a flash tint live).
    header: bool,
    /// Click targets on this row, doc-space (origin relative to `bounds.origin`).
    hits: Vec<(Bounds<Pixels>, GraveAction)>,
}

struct ParagraphLayout {
    line: WrappedLine,
    /// Byte range in the document, excluding the trailing newline.
    range: Range<usize>,
    /// Paragraph-local byte indices where soft-wrapped visual lines start.
    boundaries: Vec<usize>,
    /// Y offset of the paragraph top, relative to `TextFrame::bounds` origin.
    top: Pixels,
    height: Pixels,
    /// Per-block metrics (headings, code, quotes differ from body).
    line_height: Pixels,
    indent: Pixels,
    /// Kind-derived decorations, resolved at prepaint.
    bg: Option<gpui::Rgba>,
    quote_rule: bool,
    /// First block of the trailing footnote-definition run (H4): paints a
    /// hairline "Footnotes" section rule above itself.
    section_rule: bool,
    /// Compost decorations (asides.md §1, Bug A — the compost was invisible).
    /// An empty compost/boundary block draws a hairline at its midline (the
    /// item separator grammar); the boundary block also draws the tail anchor
    /// bar (P11). `compost_header` carries the "COMPOST" whisper painted above
    /// the first compost block; `compost_flash` blinks the arrived item's
    /// background once (asides.md §2.3).
    compost_rule: bool,
    compost_tail: bool,
    compost_header: Option<gpui::ShapedLine>,
    compost_flash: bool,
    marker: Option<gpui::ShapedLine>,
    /// Painted superior footnote figures (DESIGN §2-footnotes):
    /// (paragraph-local byte offset of the invisible carrier, label).
    /// Pre-shaped in prepaint: shaping in the PAINT phase poisons the
    /// frame's text-layout/sprite bookkeeping on scale-change redraws
    /// (the 2026-06-12 multi-monitor corruption). Paint only draws.
    fn_marks: Vec<(usize, gpui::ShapedLine)>,
    /// The block's font size — superior figures scale from it.
    font_size: Pixels,
    /// Decoded image for Image blocks, with its display size.
    image: Option<(Arc<RenderImage>, gpui::Size<Pixels>)>,
    /// The input runs this line was shaped with. Compared on the next rebuild
    /// to decide whether the shaped line can be reused verbatim (per-block
    /// layout reuse) instead of re-shaping — a few `TextRun`s per block.
    runs: Vec<TextRun>,
}

impl ParagraphLayout {
    fn len(&self) -> usize {
        self.range.end - self.range.start
    }

    fn line_count(&self) -> usize {
        self.boundaries.len() + 1
    }

    /// Paragraph-local byte index where visual line `line` starts.
    fn line_start(&self, line: usize) -> usize {
        if line == 0 { 0 } else { self.boundaries[line - 1] }
    }

    /// Paragraph-local byte index where visual line `line` ends.
    fn line_end(&self, line: usize) -> usize {
        self.boundaries.get(line).copied().unwrap_or(self.len())
    }

    /// Which visual line a paragraph-local index sits on, given affinity.
    fn line_of(&self, local: usize, affinity_down: bool) -> usize {
        self.boundaries
            .partition_point(|&b| if affinity_down { b <= local } else { b < local })
    }

    /// X position of a local index within its visual line, in frame
    /// coordinates (block indent included).
    fn x_for(&self, local: usize, line: usize) -> Pixels {
        let layout = &self.line.unwrapped_layout;
        self.indent + layout.x_for_index(local) - layout.x_for_index(self.line_start(line))
    }

    fn position(&self, local: usize, affinity_down: bool) -> (usize, Pixels) {
        let line = self.line_of(local, affinity_down);
        (line, self.x_for(local, line))
    }

    /// Closest local index to frame-x `x` on visual line `line`, with the
    /// affinity that renders the cursor on that same line.
    fn index_at(&self, line: usize, x: Pixels) -> (usize, bool) {
        let line = line.min(self.line_count() - 1);
        let y = self.line_height * (line as f32) + self.line_height / 2.;
        let local_x = (x - self.indent).max(px(0.));
        let ix = self
            .line
            .closest_index_for_position(point(local_x, y), self.line_height)
            .unwrap_or_else(|ix| ix);
        (ix, line > 0 && ix == self.line_start(line))
    }
}

impl TextFrame {
    fn doc_len(&self) -> usize {
        self.paragraphs.last().map_or(0, |p| p.range.end)
    }

    /// Maximum scroll offset: one blank line of breathing room past the end.
    fn max_scroll(&self) -> Pixels {
        (self.content_height + self.line_height - self.bounds.size.height).max(px(0.))
    }

    /// Window point -> document-space point.
    fn doc_point(&self, window_point: Point<Pixels>) -> Point<Pixels> {
        window_point - self.bounds.origin + point(px(0.), self.scroll_top)
    }

    /// (paragraph index, visual line, x) of a byte offset.
    fn cursor_position(&self, offset: usize, affinity_down: bool) -> Option<(usize, usize, Pixels)> {
        let par_ix = self.paragraphs.iter().position(|p| offset <= p.range.end)?;
        let par = &self.paragraphs[par_ix];
        let (line, x) = par.position(offset - par.range.start, affinity_down);
        Some((par_ix, line, x))
    }

    /// Position of a byte offset, relative to `bounds` origin.
    fn position_of(&self, offset: usize, affinity_down: bool) -> Option<Point<Pixels>> {
        let (par_ix, line, x) = self.cursor_position(offset, affinity_down)?;
        let par = &self.paragraphs[par_ix];
        Some(point(x, par.top + par.line_height * (line as f32)))
    }

    /// Byte offset (and cursor affinity) closest to a point relative to
    /// `bounds` origin. Points in inter-paragraph gaps snap to the nearest
    /// paragraph edge.
    fn index_for_point(&self, p: Point<Pixels>) -> (usize, bool) {
        if p.y < px(0.) {
            return (0, false);
        }
        let x = p.x.max(px(0.));
        for (i, par) in self.paragraphs.iter().enumerate() {
            if p.y < par.top && i > 0 {
                let prev = &self.paragraphs[i - 1];
                let prev_bottom = prev.top + prev.height;
                let (target, line) = if p.y - prev_bottom <= par.top - p.y {
                    (prev, prev.line_count() - 1)
                } else {
                    (par, 0)
                };
                let (ix, aff) = target.index_at(line, x);
                return (target.range.start + ix, aff);
            }
            if p.y < par.top + par.height {
                let line = ((p.y - par.top) / par.line_height) as usize;
                let (ix, aff) = par.index_at(line, x);
                return (par.range.start + ix, aff);
            }
        }
        (self.doc_len(), false)
    }
}

/// 1234 -> "1,234" for the titlebar count.
fn format_thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The auto-cut predicate (docs/impl/02-asides.md §4): a single
/// SELECTION-deletion op — an EMPTY replacement over a non-empty selection —
/// of at least `AUTO_CUT_MIN_CHARS` chars, entirely inside the manuscript
/// (`start_char >= manuscript_base`). Pure, so both sides of the threshold and
/// the move-vs-cut distinction are unit-tested.
fn auto_cut_qualifies(
    new_text: &str,
    range_chars: usize,
    start_char: usize,
    manuscript_base: usize,
) -> bool {
    new_text.is_empty() && range_chars >= AUTO_CUT_MIN_CHARS && start_char >= manuscript_base
}

/// A drawn headstone for the graveyard bar: a small muted slab with rounded
/// top corners. The ⚰/⚱ glyphs are outside the bundled PT fonts, so the icon
/// is divs, never a glyph (the garbled-glyph bug class, editor comment at the
/// titlebar-controls note).
fn tombstone_icon() -> impl IntoElement {
    div()
        .w(px(9.))
        .h(px(12.))
        .rounded_t(px(4.5))
        .bg(rgb(MUTED_COLOR))
}

/// Wall clock in unix seconds (matching checkpoints and card raise-times).
fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Whitespace-delimited word count over text chunks (rope chunks may split
/// mid-word, so track the in-word state across them).
fn count_words<'a>(chunks: impl Iterator<Item = &'a str>) -> usize {
    let mut words = 0;
    let mut in_word = false;
    for chunk in chunks {
        for c in chunk.chars() {
            if c.is_whitespace() {
                in_word = false;
            } else if !in_word {
                in_word = true;
                words += 1;
            }
        }
    }
    words
}

/// `seed:legacy` prose: exactly `words` words of placeholder text, broken into
/// ~40-word paragraphs so a checkpoint state reads like a real draft. Its word
/// count is `words` and its char count is deterministic — the strip's legacy
/// axis (extent = |Δwords|, envelope = chars) is reproducible in the rig.
fn seed_prose(words: usize) -> String {
    const POOL: [&str; 13] = [
        "the", "ferry", "held", "its", "line", "against", "the", "dark", "water", "and", "the",
        "far", "shore",
    ];
    let mut out = String::new();
    for i in 0..words {
        out.push_str(POOL[i % POOL.len()]);
        if (i + 1) % 40 == 0 && i + 1 < words {
            out.push_str("\n\n");
        } else if i + 1 < words {
            out.push(' ');
        }
    }
    out
}

impl Editor {
    pub fn new(cx: &mut Context<Self>, text: &str, spans: SpanSet, blocks: BlockMap) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            word_count: count_words([text].into_iter()),
            doc: Document::new(text, spans, blocks),
            caret_attrs: Vec::new(),
            selected_range: 0..0,
            selection_reversed: false,
            cursor_affinity_down: false,
            goal_x: None,
            marked_range: None,
            is_selecting: false,
            select_granularity: SelectGranularity::Char,
            selection_origin: None,
            scroll_top: px(0.),
            autoscroll_request: false,
            drag_point: None,
            autoscroll_active: false,
            cursor_visible: true,
            last_input: Instant::now(),
            store: None,
            store_dirty: false,
            history_view: None,
            history_preview: None,
            history_backfill_running: false,
            dirty_since_checkpoint: false,
            image_assets: HashMap::new(),
            config: Config::default(),
            focus: CardFocus::Idle,
            ai_status: None,
            pending_pass: None,
            deferred_pass: None,
            last_text_edit: None,
            appearing: std::collections::HashSet::new(),
            departing: Vec::new(),
            moving: std::collections::HashMap::new(),
            lane_tops: std::collections::HashMap::new(),
            lane_scroll: 0.,
            lane_viewport: (0., 0.),
            moves_started: 0,
            // Door closed by default: every document opens to write, not to
            // be judged (protects re-entry — the warm-up re-read that slides
            // into line-editing). The tutorial opens it (main.rs); so does a
            // pass.
            drafting: true,
            ai_generation: 0,
            diagnosis_pass: 0,
            diagnosis_mode: None,
            // Never read before the first pass sets it (Retry only exists after
            // a pass ran); a plain line diagnosis is the sensible default.
            last_pass: PassKind::Diagnostic("line".to_owned()),
            palette_input: None,
            palette_selected: 0,
            omni_scroll: ScrollHandle::new(),
            palette_query: String::new(),
            omni_return: None,
            palette_freq: HashMap::new(),
            chord_whisper: None,
            chord_whisper_shown: false,
            chord_whisper_generation: 0,
            doc_rename_input: None,
            shortcuts_open: false,
            ai_settings: None,
            ai_settings_generation: 0,
            replace_input: None,
            rename_input: None,
            alt_input: None,
            voice_baseline: None,
            card_heights: HashMap::new(),
            active_card_height: None,
            narrow_notes_open: false,
            editor_menu_open: false,
            editor_btn_right: std::rc::Rc::default(),
            selection_popover: false,
            link_input: None,
            rail_open: false,
            grave_expanded: Vec::new(),
            last_manuscript_caret: 0,
            rail_flash: None,
            grave_flash: None,
            strip_pulse: None,
            para_flash: None,
            session_had_edits: false,
            session_goal: None,
            goal_input: None,
            zone_row_bounds: std::rc::Rc::default(),
            last_frame: None,
            strip: Strip::default(),
            strip_rail: std::rc::Rc::default(),
        }
    }

    /// Attach the durable store and start the idle-save heartbeat: edits
    /// mirror into Loro immediately, the snapshot hits disk once typing
    /// pauses for a second (and on quit, via `save_now`).
    pub fn attach_store(&mut self, store: Store, cx: &mut Context<Self>) {
        let legacy = !store.checkpoints_materialized();
        self.store = Some(store);
        // Heal a legacy file without waiting for the writer to open history:
        // materialize its checkpoint states in the background now, so the
        // sidebar is instant whenever it IS opened — and the next open of
        // the file can compact the oplog (Store::open) it no longer needs.
        if legacy {
            self.backfill_checkpoint_states(cx);
        }
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(1000))
                    .await;
                let alive = this.update(cx, |editor: &mut Editor, cx| {
                    // Keystroke-durability for the open note composer: mirror
                    // its live draft onto the note every tick so a crash never
                    // loses what's been typed. Every other field in the app is
                    // already keystroke-durable; the composer was the lone
                    // RAM-only one (it wrote to the doc only on Enter-commit).
                    editor.sync_active_note_draft(cx);
                    if editor.store_dirty && editor.last_input.elapsed() >= Duration::from_secs(1)
                    {
                        editor.save_now();
                    }
                    // Idle gap seals a writing session — checkpoints are
                    // navigation markers for "a sitting", not safety.
                    if editor.dirty_since_checkpoint
                        && editor.last_input.elapsed() >= Duration::from_secs(900)
                        && let Some(store) = &editor.store
                    {
                        store.add_checkpoint_if_changed("Session", false);
                        editor.dirty_since_checkpoint = false;
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// The single "the document changed, it must be saved" chokepoint. Every
    /// mutation site routes through here instead of poking `store_dirty`
    /// directly, so no field can silently skip persistence the way the note
    /// composer once did. Keep this the ONLY place that sets the flag true.
    fn mark_dirty(&mut self) {
        self.store_dirty = true;
    }

    /// Fan buffer changes out to every offset-tracking consumer (formatting
    /// spans, durable store). Must run after every mutation.
    /// Words in the MANUSCRIPT only — the compost rail is excluded (asides.md
    /// §1: "the accounting does not bleed"). Slicing by char range avoids
    /// cloning the text; `manuscript_char_range` is the whole doc when there is
    /// no rail.
    fn manuscript_word_count(&self) -> usize {
        count_words(self.doc.rope().slice(self.doc.manuscript_char_range()).chunks())
    }

    fn sync_mutations(&mut self) {
        let ops = self.doc.take_ops();
        if ops.is_empty() {
            return;
        }
        // Real buffer ops only — this stamp is what tells a live typing burst
        // from a lull (deferred_pass), so caret moves must never touch it.
        self.last_text_edit = Some(Instant::now());
        self.word_count = self.manuscript_word_count();
        // Apply to the store before releasing the borrow, so the dirty-flag
        // chokepoint (mark_dirty) can take &mut self.
        match &self.store {
            Some(store) => store.apply(&ops),
            None => return,
        }
        self.mark_dirty();
        self.dirty_since_checkpoint = true;
        self.session_had_edits = true;
    }

    /// Mirror the open note composer's draft onto the note so the idle-save
    /// heartbeat persists it like any other keystroke. No-op (and no dirty
    /// flag) while the body is unchanged, so an idle composer doesn't force a
    /// save every tick; undo boundaries stay on the Enter-commit path.
    fn sync_active_note_draft(&mut self, cx: &mut Context<Self>) {
        // Mirror the live composer onto the note IT edits — the id and the
        // input come from the same `Composing` variant, so the draft can never
        // follow a clicked AI card or another note's anchor (that once leaked
        // the note's text onto AI cards and persisted it).
        let CardFocus::Composing { id, input } = &self.focus else {
            return;
        };
        let (id, body) = (*id, input.read(cx).content.clone());
        if self.doc.note_body(id).is_some_and(|cur| cur != body.as_str()) {
            self.doc.set_note_body_draft(id, body);
            self.mark_dirty();
        }
    }

    /// Re-enter the document where the last session left it (DESIGN §4
    /// invariant: caret restored, zero questions asked, within a second). The
    /// re-entry intent question was retired (impl 04 §1); the caret-resume half
    /// of the ritual stays — it is the only thing the sidecar now carries.
    pub fn restore_session(&mut self, entry: crate::files::SessionEntry) {
        if let Some(caret) = entry.caret {
            // Clamp to the document and snap to a char boundary (the doc
            // may have changed under us since the caret was recorded).
            let byte = self
                .doc
                .char_to_byte(self.doc.rope().byte_to_char(caret.min(self.doc.len_bytes())));
            self.selected_range = byte..byte;
            self.autoscroll_request = true;
        }
    }

    /// Quit-time bookkeeping: remember the caret so the next open resumes
    /// mid-sentence. The only thing quit records now — the intent question
    /// (and its End Session verb) is gone (impl 04 §1).
    pub fn record_exit_state(&self) {
        let Some(store) = &self.store else { return };
        crate::files::record_caret(store.path(), self.cursor_offset());
    }

    /// Load the [voice] corpus and build the self-baseline. Synchronous —
    /// essays are small; runs once at startup.
    pub fn load_voice_corpus(&mut self) {
        if self.config.voice.corpus.is_empty() {
            return;
        }
        let mut texts: Vec<String> = Vec::new();
        for pattern in &self.config.voice.corpus {
            let expanded = if let Some(rest) = pattern.strip_prefix("~/") {
                crate::paths::home_dir().join(rest).to_string_lossy().into_owned()
            } else {
                pattern.clone()
            };
            let Ok(paths) = glob::glob(&expanded) else {
                eprintln!("strop: bad corpus glob: {pattern}");
                continue;
            };
            for path in paths.flatten() {
                let text = match path.extension().and_then(|e| e.to_str()) {
                    Some("md") => std::fs::read_to_string(&path)
                        .ok()
                        .map(|md| strop_core::markdown::from_markdown(&md).0),
                    Some("strop") => Store::open(&path)
                        .ok()
                        .and_then(|(_, loaded)| loaded.map(|l| l.text)),
                    _ => std::fs::read_to_string(&path).ok(),
                };
                if let Some(text) = text
                    && text.split_whitespace().count() >= 200
                {
                    texts.push(text);
                }
            }
        }
        let lang = match self.config.language {
            Language::Ru => typograph::Lang::Ru,
            Language::En => typograph::Lang::En,
            Language::Auto => typograph::detect_lang(texts.join(" ").chars().take(4000)),
        };
        let n = texts.len();
        self.voice_baseline = strop_core::voice::baseline(&texts, lang);
        if self.voice_baseline.is_some() {
            eprintln!("strop: voice baseline from {n} corpus texts");
        } else if n > 0 {
            eprintln!("strop: voice corpus has {n} usable texts; need 3+");
        }
    }

    /// Restore persisted cross-session undo/redo.
    pub fn restore_history(&mut self, history: strop_core::document::History) {
        self.doc.import_history(history);
    }

    pub fn restore_annotations(&mut self, annotations: Annotations) {
        // The pass counter is transient but pass_id values persist on cards:
        // without re-seeding, a new session's pass 1 would collide with a
        // loaded card's pass_id and rest the wrong cards behind the rail.
        let max_pass = annotations.notes().iter().map(|n| n.pass_id).max();
        self.diagnosis_pass = self.diagnosis_pass.max(max_pass.unwrap_or(0));
        self.doc.set_notes(annotations);
    }

    /// Install the persisted edit-run journal at load.
    pub fn restore_journal(&mut self, journal: strop_core::journal::Journal) {
        self.doc.set_journal(journal);
    }

    /// Install the persisted graveyard at load, then recompute the manuscript
    /// word count (a loaded doc may carry a compost boundary that scopes it).
    pub fn restore_graveyard(&mut self, graveyard: strop_core::document::Graveyard) {
        self.doc.set_graveyard(graveyard);
        self.word_count = self.manuscript_word_count();
    }

    /// Record a named version snapshot in the document file.
    fn add_checkpoint(&mut self, _: &AddCheckpoint, _: &mut Window, cx: &mut Context<Self>) {
        // Save first: the checkpoint materializes its state from the store
        // (Checkpoint::state), and the store's spans/blocks/annotations are
        // only as fresh as the last save. Checkpointing implying durability
        // is the right property anyway.
        self.save_now();
        if let Some(store) = &self.store {
            let name = format!("Checkpoint {}", store.checkpoints().len() + 1);
            store.add_checkpoint(&name, true);
            self.dirty_since_checkpoint = false;
            self.mark_dirty();
            eprintln!("strop: recorded \"{name}\"");
        }
        cx.notify();
    }

    /// Build the rewind list from the checkpoints' MATERIALIZED states —
    /// word deltas between consecutive states and (when a self-baseline
    /// exists) per-checkpoint voice drift. Cheap (stored states + word
    /// diffs, microseconds each), but only once every checkpoint carries a
    /// state: a legacy one would fall back to a multi-second historical
    /// checkout — the old sidebar hang (71 s measured for 13 checkpoints).
    /// Callers gate on `checkpoints_materialized`.
    fn build_history_entries(&self) -> Vec<HistoryEntry> {
        let Some(store) = &self.store else {
            return Vec::new();
        };
        let mut entries: Vec<HistoryEntry> = Vec::new();
        let mut prev_text = String::new();
        for cp in store.checkpoints() {
            let Some((text, spans, blocks)) = store.checkpoint_state(&cp) else {
                continue;
            };
            let delta = strop_core::diff::word_delta(&strop_core::diff::prose_diff(
                &prev_text, &text,
            ));
            // Flag-only scalar: shown when assess() puts the checkpoint
            // outside the writer's normal range. Gated on the corpus floor
            // (200 words) — shorter states are statistical noise. The
            // signature must use the BASELINE's language (the vectors are
            // per-language and differently sized).
            let drift_sigma = self.voice_baseline.as_ref().and_then(|b| {
                if text.split_whitespace().count() < 200 {
                    return None;
                }
                let report = b.assess(&strop_core::voice::signature(&text, b.lang()));
                (report.overall_sigma > 2.).then_some(report.overall_sigma)
            });
            prev_text = text.clone();
            entries.push(HistoryEntry {
                name: cp.name.clone(),
                created_unix: cp.created_unix,
                manual: cp.manual,
                text,
                spans,
                blocks,
                delta,
                drift_sigma,
            });
        }
        entries
    }

    /// Open the rewind list. With materialized checkpoint states this is
    /// instant. A LEGACY file (checkpoints recorded before states were
    /// stored) instead shows the shell immediately — names, dates,
    /// structure — while a background pass materializes the states once,
    /// persists them into the file, and then fills in texts, deltas and the
    /// preview. The old behaviour materialized inline on the main thread.
    fn enter_history(&mut self, cx: &mut Context<Self>) {
        // The strip and the panel never open together (spec §0).
        if self.strip.open {
            self.close_strip(cx);
        }
        let Some(store) = &self.store else {
            return;
        };
        let materializing = !store.checkpoints_materialized();
        let entries: Vec<HistoryEntry> = if materializing {
            store
                .checkpoints()
                .into_iter()
                .map(|cp| HistoryEntry {
                    name: cp.name,
                    created_unix: cp.created_unix,
                    manual: cp.manual,
                    text: String::new(),
                    spans: SpanSet::default(),
                    blocks: BlockMap::default(),
                    delta: (0, 0),
                    drift_sigma: None,
                })
                .collect()
        } else {
            self.build_history_entries()
        };
        if entries.is_empty() {
            return;
        }
        let selected = entries.len() - 1;
        // You land on the newest checkpoint: if it's automatic, its run
        // starts unfolded so the selected row is visible.
        let mut expanded = HashSet::new();
        if !entries[selected].manual {
            expanded.insert(auto_group_start(&entries, selected));
        }
        self.history_view = Some(HistoryView {
            entries,
            selected,
            named_only: false,
            compare_current: false,
            expanded,
            materializing,
        });
        self.rebuild_preview();
        if materializing {
            self.backfill_checkpoint_states(cx);
        }
        cx.notify();
    }

    /// Materialize legacy checkpoints' states OFF the main thread and write
    /// them back — once per file, ever. Snapshot bytes go to a background
    /// task with its own private doc (the live one never blocks); the states
    /// come back, persist into the checkpoint records (so every later open
    /// is instant), and the sidebar, if still open, fills in.
    fn backfill_checkpoint_states(&mut self, cx: &mut Context<Self>) {
        if self.history_backfill_running {
            return;
        }
        let Some(store) = &self.store else { return };
        let Ok(bytes) = store.export_bytes() else { return };
        self.history_backfill_running = true;
        let task = cx
            .background_executor()
            .spawn(async move { Store::materialize_checkpoint_states(&bytes) });
        cx.spawn(async move |this, cx| {
            let states = task.await;
            this.update(cx, |editor: &mut Editor, cx| {
                editor.history_backfill_running = false;
                {
                    let Some(store) = &editor.store else { return };
                    for (ix, state) in states {
                        store.set_checkpoint_state(ix, state);
                    }
                }
                editor.mark_dirty();
                if editor.history_view.as_ref().is_some_and(|hv| hv.materializing) {
                    let entries = editor.build_history_entries();
                    if !entries.is_empty() {
                        if let Some(hv) = &mut editor.history_view {
                            hv.selected = hv.selected.min(entries.len() - 1);
                            hv.entries = entries;
                            hv.materializing = false;
                        }
                        editor.rebuild_preview();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn exit_history(&mut self, cx: &mut Context<Self>) {
        self.history_view = None;
        self.history_preview = None;
        cx.notify();
    }

    fn rebuild_preview(&mut self) {
        use strop_core::diff::DiffOp;
        let Some(hv) = &self.history_view else {
            self.history_preview = None;
            return;
        };
        // States still assembling in the background: nothing to diff yet
        // (the canvas keeps showing the live document until they land).
        if hv.materializing {
            self.history_preview = None;
            return;
        }
        let entry = &hv.entries[hv.selected];
        let empty_spans = SpanSet::default();
        let empty_blocks = BlockMap::default();
        let current_text;
        let (old, old_spans, old_blocks, new, new_spans, new_blocks) = if hv.compare_current {
            current_text = self.doc.text();
            (
                entry.text.as_str(),
                &entry.spans,
                &entry.blocks,
                current_text.as_str(),
                self.doc.spans(),
                self.doc.blocks(),
            )
        } else {
            let (old, old_spans, old_blocks) = match hv.selected.checked_sub(1) {
                Some(i) => {
                    let prev = &hv.entries[i];
                    (prev.text.as_str(), &prev.spans, &prev.blocks)
                }
                None => ("", &empty_spans, &empty_blocks),
            };
            (
                old,
                old_spans,
                old_blocks,
                entry.text.as_str(),
                &entry.spans,
                &entry.blocks,
            )
        };

        // Byte offsets of each '\n'-separated paragraph, plus char->byte
        // span conversion for each source (live spans are char-indexed).
        let par_offsets = |text: &str| {
            let mut offs = vec![0usize];
            offs.extend(text.match_indices('\n').map(|(b, _)| b + 1));
            offs
        };
        let spans_to_bytes = |text: &str, spans: &SpanSet| {
            let mut idx: Vec<usize> = text.char_indices().map(|(b, _)| b).collect();
            idx.push(text.len());
            let b = |ci: usize| idx.get(ci).copied().unwrap_or(text.len());
            spans
                .spans()
                .iter()
                .map(|s| (b(s.range.start)..b(s.range.end), s.attr.clone()))
                .collect::<Vec<_>>()
        };
        let old_offs = par_offsets(old);
        let new_offs = par_offsets(new);
        let old_spans_b = spans_to_bytes(old, old_spans);
        let new_spans_b = spans_to_bytes(new, new_spans);

        let mut text = String::new();
        let mut inserts = Vec::new();
        let mut deletes = Vec::new();
        let mut spans_bytes: Vec<(Range<usize>, InlineAttr)> = Vec::new();
        let mut kinds: Vec<BlockKind> = Vec::new();
        for (i, b) in strop_core::diff::prose_diff_blocks(old, new).iter().enumerate() {
            if i > 0 {
                text.push('\n');
            }
            // Block style follows the newer side when it exists there.
            kinds.push(
                b.new_par
                    .and_then(|p| new_blocks.kinds().get(p))
                    .or_else(|| b.old_par.and_then(|p| old_blocks.kinds().get(p)))
                    .cloned()
                    .unwrap_or(BlockKind::Paragraph),
            );
            // Within the block, Delete segments concatenate to the old
            // paragraph and Same+Insert to the new one (byte-exact, by
            // prose_diff_blocks' contract) — walk both cursors and project
            // each source's spans onto the merged text.
            let mut old_b = b.old_par.map_or(0, |p| old_offs[p]);
            let mut new_b = b.new_par.map_or(0, |p| new_offs[p]);
            for seg in &b.segs {
                let start = text.len();
                let len = seg.text.len();
                let (src_spans, src_start) = match seg.op {
                    DiffOp::Delete => (&old_spans_b, old_b),
                    _ => (&new_spans_b, new_b),
                };
                for (r, attr) in src_spans {
                    let s = r.start.max(src_start);
                    let e = r.end.min(src_start + len);
                    if s < e {
                        spans_bytes
                            .push((start + (s - src_start)..start + (e - src_start), attr.clone()));
                    }
                }
                text.push_str(&seg.text);
                match seg.op {
                    DiffOp::Insert => {
                        inserts.push(start..text.len());
                        new_b += len;
                    }
                    DiffOp::Delete => {
                        deletes.push(start..text.len());
                        old_b += len;
                    }
                    DiffOp::Same => {
                        old_b += len;
                        new_b += len;
                    }
                }
            }
        }
        self.history_preview = Some(PreviewDoc {
            text,
            inserts,
            deletes,
            spans_bytes,
            kinds,
        });
    }

    /// Wire click-away-commits for a single-line field: the instant it loses
    /// focus, emit `Commit` so the field's own subscriber saves and tears it
    /// down — it becomes a label immediately, not at some later stray click
    /// (the low-latency rule; matches the doc-rename field). `still` re-fetches
    /// the live field, so a blur that races an Enter/Escape (already gone) is a
    /// no-op. NOT for end-session, whose Commit quits — blur there must not.
    fn commit_field_on_blur(
        &self,
        input: &Entity<TextField>,
        window: &mut Window,
        cx: &mut Context<Self>,
        still: impl Fn(&Editor) -> Option<Entity<TextField>> + 'static,
    ) {
        let handle = input.read(cx).focus_handle.clone();
        let weak = cx.entity().downgrade();
        window
            .on_focus_out(&handle, cx, move |_, _window, cx| {
                let Some(editor) = weak.upgrade() else { return };
                editor.update_checked(cx, |editor, cx| {
                    if let Some(field) = still(editor) {
                        let text = field.read(cx).content.clone();
                        field.update_checked(cx, |_, fcx| fcx.emit(TextFieldEvent::Commit(text)));
                    }
                });
            })
            .detach();
    }

    fn edit_image_alt(&mut self, block: usize, window: &mut Window, cx: &mut Context<Self>) {
        let BlockKind::Image { src, alt, caption } = self.doc.blocks().kind(block).clone()
        else {
            return;
        };
        let input = cx.new(|cx| TextField::single(cx, alt));
        cx.subscribe_in(
            &input,
            window,
            move |editor, _, event: &TextFieldEvent, window, cx| {
                if let TextFieldEvent::Commit(new_alt) = event {
                    editor.doc.set_block_kind(
                        block,
                        BlockKind::Image {
                            src: src.clone(),
                            alt: new_alt.clone(),
                            caption: caption.clone(),
                        },
                    );
                    editor.mark_dirty();
                }
                editor.alt_input = None;
                window.focus(&editor.focus_handle, cx);
                cx.notify();
            },
        )
        .detach();
        self.commit_field_on_blur(&input, window, cx, move |e| {
            e.alt_input
                .as_ref()
                .filter(|(b, _)| *b == block)
                .map(|(_, f)| f.clone())
        });
        let input_focus = input.read(cx).focus_handle.clone();
        window.focus(&input_focus, cx);
        self.alt_input = Some((block, input));
        cx.notify();
    }

    fn start_rename(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(hv) = &self.history_view else { return };
        let seed = hv.entries[ix].name.clone();
        let input = cx.new(|cx| TextField::single(cx, seed));
        cx.subscribe_in(
            &input,
            window,
            move |editor, _, event: &TextFieldEvent, window, cx| {
                if let TextFieldEvent::Commit(name) = event
                    && !name.trim().is_empty()
                {
                    if let Some(store) = &editor.store {
                        store.rename_checkpoint(ix, name.trim());
                        editor.mark_dirty();
                    }
                    if let Some(hv) = &mut editor.history_view
                        && let Some(e) = hv.entries.get_mut(ix)
                    {
                        e.name = name.trim().to_owned();
                        e.manual = true;
                    }
                }
                editor.rename_input = None;
                window.focus(&editor.focus_handle, cx);
                cx.notify();
            },
        )
        .detach();
        self.commit_field_on_blur(&input, window, cx, move |e| {
            e.rename_input
                .as_ref()
                .filter(|(i, _)| *i == ix)
                .map(|(_, f)| f.clone())
        });
        let input_focus = input.read(cx).focus_handle.clone();
        window.focus(&input_focus, cx);
        self.rename_input = Some((ix, input));
        cx.notify();
    }

    fn history_select(&mut self, ix: usize, cx: &mut Context<Self>) {
        if let Some(hv) = &mut self.history_view {
            hv.selected = ix.min(hv.entries.len() - 1);
            // Arrow-stepping into a collapsed auto run unfolds it: the
            // selected row must be visible (no hidden modes, no hidden
            // selection).
            if !hv.entries[hv.selected].manual {
                hv.expanded
                    .insert(auto_group_start(&hv.entries, hv.selected));
            }
            self.rebuild_preview();
            self.scroll_top = px(0.);
            cx.notify();
        }
    }

    /// Restore the selected checkpoint: auto-checkpoint the present first
    /// (the rail narrates what happened), restore as an undoable forward
    /// edit, exit history. Reads the entry's own materialized state — the
    /// old `state_at` re-fetch here was another multi-second historical
    /// checkout for data the sidebar already held.
    fn restore_selected(&mut self, cx: &mut Context<Self>) {
        let Some(hv) = &self.history_view else { return };
        if hv.materializing {
            return; // states still assembling; the row can't restore yet
        }
        let entry = &hv.entries[hv.selected];
        let (text, spans, blocks) = (
            entry.text.clone(),
            entry.spans.clone(),
            entry.blocks.clone(),
        );
        let from_unix = entry.created_unix;
        self.restore_to_state(text, spans, blocks, from_unix, cx);
        cx.notify();
    }

    /// The shared restore mechanism (review B6, spec §2), reused by both the
    /// history panel's Restore and the strip's own. Restore is an ordinary
    /// UNDOABLE forward edit that appends nothing to the past — it destroys
    /// nothing (design §3). Its record is deliberately layered: a "Before
    /// restoring" checkpoint of the present (so this restore is itself one
    /// Restore away from undone), the wholesale swap (journal-suppressed — it
    /// would be one document-sized run), an honest `Restore` journal event,
    /// and a POST-restore "Restored" checkpoint that materializes the restored
    /// state as the reconstruction anchor (without it, `text_at(now)` would
    /// rebuild the PRE-restore document). `from_unix` is the moment copied
    /// from, in SECONDS (matching `Checkpoint.created_unix`). The caller
    /// notifies.
    fn restore_to_state(
        &mut self,
        text: String,
        spans: SpanSet,
        blocks: BlockMap,
        from_unix: i64,
        cx: &mut Context<Self>,
    ) {
        // Save first so the "Before restore" checkpoint materializes the
        // present exactly as it stands (see add_checkpoint). Its name is a
        // bare automatic: inlining the source version's title composed
        // writer text into a system label — P8's born-from bug class. The
        // sage arc already points at the source (P10: words don't repeat
        // what the arc says).
        self.save_now();
        let Some(store) = &self.store else { return };
        store.add_checkpoint("Before restore", false);
        self.doc.restore_state(&text, spans, blocks);
        self.selected_range = 0..0;
        self.selection_reversed = false;
        self.cursor_affinity_down = false;
        self.goal_x = None;
        self.marked_range = None;
        self.caret_attrs.clear();
        // Drops any preview (panel `history_view` OR the strip's
        // `history_preview`) so the live restored document is what shows.
        self.exit_history(cx);
        self.sync_mutations();
        // A restore can orphan writer notes (reanchor sets `orphaned` when a
        // passage vanished): migrate their text to the compost (spec §3).
        self.migrate_orphans_after_restore(cx);
        // The restore's own record: the journal suppressed the wholesale
        // swap (it would be one document-sized run), so the honest event
        // carries the fact — and a POST-restore checkpoint materializes the
        // restored state as the reconstruction anchor (review B6: without
        // it, text_at(now) would rebuild the PRE-restore document).
        let len_chars = self.doc.rope().len_chars();
        self.doc
            .journal_mut()
            .record_event(strop_core::journal::JournalEvent::Restore {
                t: strop_core::journal::now_ms(),
                from_unix,
                len_chars,
            });
        self.save_now();
        if let Some(store) = &self.store {
            store.add_checkpoint("Restored", false);
        }
        self.mark_dirty();
        self.bump_activity();
    }

    // -- The history strip (P1 — docs/impl/01-history-strip.md) ---------------

    /// ctrl-alt-h / the titlebar clock: toggle the strip (the new first history
    /// surface). The right-side panel stays reachable via the palette ("History
    /// panel"); the two never open together (spec §0).
    fn toggle_strip(&mut self, _: &ToggleStrip, window: &mut Window, cx: &mut Context<Self>) {
        if self.strip.open {
            self.close_strip(cx);
        } else {
            self.open_strip(window, cx);
        }
    }

    fn open_strip(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.strip.open {
            return;
        }
        // Strip and panel never coexist (spec §0): the panel yields.
        if self.history_view.is_some() {
            self.exit_history(cx);
        }
        let now = strop_core::journal::now_ms();
        self.strip.open = true;
        self.strip.parked = false;
        self.strip.pin_ms = None;
        self.strip.pos_ms = now;
        self.strip.view_offset = 0.;
        self.strip_bake(now);
        cx.notify();
    }

    fn close_strip(&mut self, cx: &mut Context<Self>) {
        self.strip.open = false;
        self.strip.parked = false;
        self.strip.scrubbing = false;
        self.strip.pin_ms = None;
        self.strip.scratch = None;
        self.strip.bake = None;
        self.strip_pulse = None;
        // Drop any preview so the live document returns.
        self.history_preview = None;
        *self.strip_rail.borrow_mut() = None;
        cx.notify();
    }

    /// Build the immutable bake from `(journal, checkpoints, cards)` and bump
    /// the session-monotonic `bakes` counter (the stability-law assertion —
    /// only open and Restore ever call this; scrubbing never does). Snapshots
    /// the checkpoint metadata so the strip never re-reads the store mid-scrub.
    fn strip_bake(&mut self, now_ms: i64) {
        let stations = self.strip_stations();
        let cards = self.strip_cards();
        let seed_len = self.strip_seed_len();
        let bake = StripBake::build(self.doc.journal(), &stations, &cards, seed_len, now_ms);
        self.strip.bake = Some(bake);
        self.strip.bakes += 1;
        self.strip.scratch = None; // re-anchor against the new bake
    }

    /// Checkpoints reduced to draw/anchor metadata (created_unix SECONDS ×1000
    /// at the boundary — the unit law). Word/char counts come from the ALREADY
    /// materialized state (a cheap in-memory read — no time-travel; a stateless
    /// legacy checkpoint reports 0/0 and sizes no span, per Bug A).
    fn strip_stations(&self) -> Vec<strip::StationSnap> {
        let Some(store) = &self.store else {
            return Vec::new();
        };
        store
            .checkpoints()
            .into_iter()
            .map(|cp| {
                let (words, chars) = cp
                    .state
                    .as_ref()
                    .map(|s| (count_words(std::iter::once(s.text.as_str())), s.text.chars().count()))
                    .unwrap_or((0, 0));
                strip::StationSnap {
                    created_ms: cp.created_unix * 1000,
                    name: cp.name,
                    manual: cp.manual,
                    has_state: cp.state.is_some(),
                    words,
                    chars,
                }
            })
            .collect()
    }

    /// Each margin card's lifespan for its thread: raise time (the note's
    /// `created_unix` SECONDS ×1000) → its `CardClosed` event, else open to now.
    fn strip_cards(&self) -> Vec<strip::CardSnap> {
        use strop_core::journal::JournalEvent;
        let closes: HashMap<u64, (i64, bool)> = self
            .doc
            .journal()
            .events
            .iter()
            .filter_map(|e| match e {
                JournalEvent::CardClosed { t, id, resolved } => Some((*id, (*t, *resolved))),
                _ => None,
            })
            .collect();
        let doc_len = self.doc.rope().len_chars().max(1) as f32;
        self.doc
            .notes()
            .notes()
            .iter()
            .map(|n| {
                let closed = closes.get(&n.id);
                strip::CardSnap {
                    raised_ms: n.created_unix * 1000,
                    closed_ms: closed.map(|(t, _)| *t),
                    depth: (n.range.start as f32 / doc_len).clamp(0., 1.),
                    resolved: closed.map_or(n.status == NoteStatus::Done, |(_, r)| *r),
                }
            })
            .collect()
    }

    /// The document's char length at journal start (the earliest materialized
    /// checkpoint ≤ the first run), so the envelope is seeded for a doc that
    /// already held content when journaling began (else 0 — born empty).
    fn strip_seed_len(&self) -> usize {
        let Some(store) = &self.store else {
            return 0;
        };
        let Some(first) = self.doc.journal().runs.first().map(|r| r.t0) else {
            return 0;
        };
        store
            .checkpoints()
            .iter()
            .rfind(|cp| cp.created_unix * 1000 <= first && cp.state.is_some())
            .and_then(|cp| cp.state.as_ref())
            .map(|s| s.text.chars().count())
            .unwrap_or(0)
    }

    /// A materialized checkpoint's state, found by its ms anchor time. Called
    /// only on re-anchor (a scrub crossing a checkpoint), never per frame.
    fn checkpoint_state_at_ms(&self, ms: i64) -> Option<(String, SpanSet, BlockMap)> {
        let store = self.store.as_ref()?;
        let cps = store.checkpoints();
        // .rev(): same-second neighbors exist by design — "Before restore"
        // and "Restored" are written moments apart, and the LATER one is the
        // truthful anchor (wave-1 review: anchoring on the earlier twin
        // rebuilt the pre-restore text right after a restore).
        let cp = cps
            .iter()
            .rev()
            .find(|cp| cp.created_unix * 1000 == ms && cp.state.is_some())?;
        store.checkpoint_state(cp)
    }

    /// Park the playhead at a rail x (window coords) and begin a continuous
    /// scrub; `pin` (shift-click) drops/clears the Compare playhead instead.
    /// Is there any PAST to view? False for a document with no journal and no
    /// checkpoints beyond the present (a zero-width axis) — parking there would
    /// preview "now" and needlessly enter read-only mode (P13: no view without
    /// content). `total_work == 0` captures both the truly-empty doc and the
    /// only-a-now-checkpoint one (its extend-to-now span has no width).
    fn strip_has_past(&self) -> bool {
        self.strip
            .bake
            .as_ref()
            .is_some_and(|b| b.timeline.total_work > 0.)
    }

    fn strip_park_at_x(&mut self, x: f32, pin: bool, cx: &mut Context<Self>) {
        let Some(pos) = self.strip_pos_at_x(x) else {
            return;
        };
        // No history → never park (the reported guard).
        if !self.strip_has_past() {
            return;
        }
        if pin {
            self.strip_toggle_pin(pos, cx);
            return;
        }
        self.strip.parked = true;
        self.strip.scrubbing = true;
        self.strip_scrub_to(pos, cx);
    }

    /// Map a rail x (window coords) → working px → wall-clock ms. The thumb
    /// travels `min(total_work, rail_width)` (design §1): when the history fits,
    /// the rail and the fixed-scale fabric coincide 1:1; when it overflows, the
    /// thumb compresses the whole duration into the rail while the fabric
    /// auto-scrolls. Both this input map and the painted thumb use `strip_travel`.
    fn strip_pos_at_x(&self, x: f32) -> Option<i64> {
        let geom = (*self.strip_rail.borrow())?;
        let bake = self.strip.bake.as_ref()?;
        let travel = strip_travel(geom.rail_x1 - geom.rail_x0, bake.timeline.total_work);
        let frac = ((x - geom.rail_x0) / travel).clamp(0., 1.);
        Some(bake.timeline.wall_at(frac * bake.timeline.total_work))
    }

    /// One scrub step: move the playhead, reconstruct the past, and auto-scroll
    /// the fabric to keep the playhead in view (the mutable half of the
    /// stability law — the bake never moves; only these do).
    fn strip_scrub_to(&mut self, pos_ms: i64, cx: &mut Context<Self>) {
        self.strip.pos_ms = pos_ms;
        self.strip_reconstruct(pos_ms);
        let view = {
            let geom = *self.strip_rail.borrow();
            match (geom, self.strip.bake.as_ref()) {
                (Some(g), Some(b)) => {
                    let rail_w = (g.rail_x1 - g.rail_x0).max(1.);
                    let work = b.timeline.work_at(pos_ms);
                    Some((work - rail_w / 2.).clamp(0., (b.timeline.total_work - rail_w).max(0.)))
                }
                _ => None,
            }
        };
        if let Some(v) = view {
            self.strip.view_offset = v;
        }
        cx.notify();
    }

    /// Reconstruct the document as it stood at `pos_ms` (spec §2). Anchors on
    /// the latest materialized checkpoint ≤ t (the ms law), replays the journal
    /// forward through a bare `ReplayDoc` (no store — a scrub never journals),
    /// and shows it read-only through the EXISTING `history_preview` path so
    /// the margin lane hides and the canvas renders the past. Rightward drags
    /// advance the cached scratch incrementally; a leftward jump or a new
    /// anchor rebuilds it.
    fn strip_reconstruct(&mut self, pos_ms: i64) {
        let anchor_ms: Option<i64> = self.strip.bake.as_ref().and_then(|b| {
            strip::anchor_index(&b.anchor_ms, pos_ms).map(|i| b.anchor_ms[i])
        });
        let anchor_key = anchor_ms.unwrap_or(i64::MIN);
        let runs_until = self.doc.journal().runs_until(pos_ms);
        let reuse = self
            .strip
            .scratch
            .as_ref()
            .is_some_and(|s| s.anchor_ms == anchor_key && runs_until >= s.replay.applied);
        if !reuse {
            // A leftward drag within the SAME anchor rebuilds from the
            // scratch's own cached state; only a genuinely new anchor reads
            // the store (whose checkpoint list parse is per-call).
            let cached = self
                .strip
                .scratch
                .as_ref()
                .filter(|s| s.anchor_ms == anchor_key)
                .map(|s| s.anchor_state.clone());
            let (text, spans, blocks) = cached.unwrap_or_else(|| match anchor_ms {
                Some(a) => self
                    .checkpoint_state_at_ms(a)
                    .unwrap_or_else(|| (String::new(), SpanSet::default(), BlockMap::default())),
                None => (String::new(), SpanSet::default(), BlockMap::default()),
            });
            let applied = anchor_ms.map_or(0, |a| self.doc.journal().runs_until(a));
            let replay =
                strop_core::journal::ReplayDoc::new(&text, spans.clone(), blocks.clone(), applied);
            self.strip.scratch = Some(strip::ScrubDoc {
                replay,
                anchor_ms: anchor_key,
                anchor_state: (text, spans, blocks),
            });
        }
        // Advance forward to pos_ms (disjoint field borrows: scratch vs doc).
        // Unchanged replay on a reused scratch = the projection below would
        // rebuild an identical PreviewDoc — skip the O(doc) work per frame.
        let mut advanced = false;
        if let Some(scratch) = self.strip.scratch.as_mut() {
            advanced = scratch.replay.advance(self.doc.journal(), pos_ms);
        }
        if reuse && !advanced {
            return;
        }
        // Project the reconstructed rope + spans + blocks into a PreviewDoc.
        let Some(scratch) = self.strip.scratch.as_ref() else {
            return;
        };
        let text = scratch.replay.text();
        let words = count_words(std::iter::once(text.as_str()));
        let spans_bytes: Vec<(Range<usize>, InlineAttr)> = {
            let mut idx: Vec<usize> = text.char_indices().map(|(b, _)| b).collect();
            idx.push(text.len());
            let b = |ci: usize| idx.get(ci).copied().unwrap_or(text.len());
            scratch
                .replay
                .spans
                .spans()
                .iter()
                .map(|s| (b(s.range.start)..b(s.range.end), s.attr.clone()))
                .collect()
        };
        let kinds = scratch.replay.blocks.kinds().to_vec();
        // No diff coloring — the strip shows the past AS IT WAS, not a diff.
        self.history_preview = Some(PreviewDoc {
            text,
            inserts: Vec::new(),
            deletes: Vec::new(),
            spans_bytes,
            kinds,
        });
        self.strip.words_at = words;
    }

    /// Esc / the Now chip: drop the preview, return to the present, keep the
    /// strip open (spec §2). Also clears any Compare pin (review: every state
    /// needs an exit).
    fn strip_return_to_now(&mut self, cx: &mut Context<Self>) {
        let now = self
            .strip
            .bake
            .as_ref()
            .map_or_else(strop_core::journal::now_ms, |b| b.now_ms);
        self.strip.parked = false;
        self.strip.scrubbing = false;
        self.strip.pin_ms = None;
        self.strip.pos_ms = now;
        self.strip.scratch = None;
        self.history_preview = None;
        cx.notify();
    }

    /// An edit gesture arrived while parked in the past (Bug B): the past is
    /// read-only, so the text is untouched and the banner's moment label
    /// pulses instead — the ONE uniform refusal for every mutation (insert,
    /// delete, format, block, undo, redo, link, paste, cut). No restore is
    /// ever committed by a keystroke (P2: the tool never wants). The fade
    /// alpha is a pure function of `Instant`; a short spawn loop re-notifies
    /// across the ~900ms so the flash actually decays on screen.
    fn pulse_strip(&mut self, cx: &mut Context<Self>) {
        self.strip_pulse = Some(Instant::now());
        cx.spawn(async move |this, cx| {
            for _ in 0..STRIP_PULSE_FRAMES {
                cx.background_executor()
                    .timer(Duration::from_millis(STRIP_PULSE_MS / STRIP_PULSE_FRAMES))
                    .await;
                let cont = this.update(cx, |editor: &mut Editor, cx| {
                    let done = editor
                        .strip_pulse
                        .is_none_or(|t| t.elapsed() >= Duration::from_millis(STRIP_PULSE_MS));
                    if done {
                        editor.strip_pulse = None;
                    }
                    cx.notify();
                    done
                });
                // Stop once the fade finished (Ok(true)) or the entity is gone
                // (Err); keep re-notifying only while Ok(false).
                if !matches!(cont, Ok(false)) {
                    break;
                }
            }
        })
        .detach();
    }

    /// Compare (shift-click): a second faint playhead whose delta folds into
    /// the readout's single line. A second shift-click on/near it clears it.
    fn strip_toggle_pin(&mut self, pos_ms: i64, cx: &mut Context<Self>) {
        let clear = self.strip.pin_ms.is_some_and(|p| {
            // Near the existing pin, in working px — a toggle-off.
            self.strip.bake.as_ref().is_some_and(|b| {
                (b.timeline.work_at(p) - b.timeline.work_at(pos_ms)).abs() < 8.
            })
        });
        if clear {
            self.strip.pin_ms = None;
        } else {
            self.strip.pin_ms = Some(pos_ms);
            self.strip.pin_words = self.strip_word_count_at(pos_ms);
        }
        cx.notify();
    }

    /// Word count of the reconstruction at `pos_ms`, computed on a THROWAWAY
    /// `ReplayDoc` (never touching the cached scratch or the live preview) —
    /// used once when a Compare pin is set, so the readout's folded delta is
    /// free thereafter.
    fn strip_word_count_at(&self, pos_ms: i64) -> usize {
        let Some(bake) = self.strip.bake.as_ref() else {
            return 0;
        };
        let anchor_ms = strip::anchor_index(&bake.anchor_ms, pos_ms).map(|i| bake.anchor_ms[i]);
        let (text, spans, blocks) = match anchor_ms {
            Some(a) => self
                .checkpoint_state_at_ms(a)
                .unwrap_or_else(|| (String::new(), SpanSet::default(), BlockMap::default())),
            None => (String::new(), SpanSet::default(), BlockMap::default()),
        };
        let applied = anchor_ms.map_or(0, |a| self.doc.journal().runs_until(a));
        let mut replay = strop_core::journal::ReplayDoc::new(&text, spans, blocks, applied);
        replay.advance(self.doc.journal(), pos_ms);
        count_words(std::iter::once(replay.text().as_str()))
    }

    /// The strip's own Restore (spec §2): build a `CheckpointState`-shaped value
    /// from the reconstructed scratch and route through the SAME restore path
    /// as the panel (undoable forward edit, notes reanchor, the "Before
    /// restoring" and "Restored" checkpoints, the `Restore` event). Then the one
    /// lawful in-session re-bake (data changed) and a return to the new present.
    fn strip_restore(&mut self, cx: &mut Context<Self>) {
        if !self.strip.is_parked() {
            return;
        }
        let Some(scratch) = self.strip.scratch.as_ref() else {
            self.strip_return_to_now(cx);
            return;
        };
        let text = scratch.replay.text();
        let spans = scratch.replay.spans.clone();
        let blocks = scratch.replay.blocks.clone();
        let from_unix = self.strip.pos_ms / 1000;
        self.restore_to_state(text, spans, blocks, from_unix, cx);
        // Data changed — refresh the bake, snap the playhead to the new present
        // (the restore IS the new now; the fabric never re-scales under the eye
        // mid-view, only here — design §1).
        let now = strop_core::journal::now_ms();
        self.strip.parked = false;
        self.strip.scrubbing = false;
        self.strip.pin_ms = None;
        self.strip.pos_ms = now;
        self.strip.scratch = None;
        // Drop the preview even if the restore degraded (no store): the strip
        // returns to the live document at now.
        self.history_preview = None;
        self.strip_bake(now);
        cx.notify();
    }

    /// Wheel over the strip pans the FABRIC only (never the thumb, never the
    /// document — design §3); the view offset varies, the bake stays frozen.
    fn strip_pan(&mut self, d: f32, cx: &mut Context<Self>) {
        let max = {
            let geom = *self.strip_rail.borrow();
            match (geom, self.strip.bake.as_ref()) {
                (Some(g), Some(b)) => {
                    Some((b.timeline.total_work - (g.rail_x1 - g.rail_x0)).max(0.))
                }
                _ => None,
            }
        };
        let Some(max) = max else { return };
        self.strip.view_offset = (self.strip.view_offset - d).clamp(0., max);
        cx.notify();
    }

    /// Export next to the .strop file (doc.strop -> doc.md).
    fn export_markdown(&mut self, _: &ExportMarkdown, _: &mut Window, cx: &mut Context<Self>) {
        let Some(store) = &self.store else {
            eprintln!("strop: no document file to export next to");
            return;
        };
        // Export the MANUSCRIPT only — the compost rail is the writer's private
        // scrap box, never part of the exported document (asides.md §1). The
        // slice rebases span/block offsets to 0 (review H40, TRAP 14).
        let (mtext, mspans, mblocks) = self.doc.manuscript_slice();
        let mut md = strop_core::markdown::to_markdown(&mtext, &mspans, &mblocks);
        let path = store.path().with_extension("md");
        // Materialize in-file assets as a sidecar dir with relative links
        // (document-model §6).
        let asset_ids: Vec<String> = self
            .doc
            .blocks()
            .asset_refs()
            .map(str::to_owned)
            .collect();
        if !asset_ids.is_empty() {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("doc")
                .to_owned();
            let dir = path.with_file_name(format!("{stem}.assets"));
            if let Err(e) = std::fs::create_dir_all(&dir) {
                eprintln!("strop: export assets dir: {e}");
            } else {
                for id in asset_ids {
                    let Some(bytes) = store.get_asset(&id) else { continue };
                    let file = id.trim_start_matches("asset:").to_owned();
                    let rel = format!("{stem}.assets/{file}");
                    if let Err(e) = std::fs::write(dir.join(&file), bytes) {
                        eprintln!("strop: export asset {file}: {e}");
                    }
                    md = md.replace(&format!("]({id})"), &format!("]({rel})"));
                }
            }
        }
        match std::fs::write(&path, md) {
            Ok(()) => {
                eprintln!("strop: exported {}", path.display());
                // A ship-shaped moment — the strip marks it.
                self.doc
                    .journal_mut()
                    .record_event(strop_core::journal::JournalEvent::Export {
                        t: strop_core::journal::now_ms(),
                    });
                self.mark_dirty();
            }
            Err(e) => eprintln!("strop: export failed: {e}"),
        }
        cx.notify();
    }

    /// Insert a footnote: a ref atom at the cursor, a def block at the end,
    /// cursor lands in the def. (Two transactions; two undos remove it.)
    fn insert_footnote(&mut self, _: &InsertFootnote, _: &mut Window, cx: &mut Context<Self>) {
        // Fresh internal label, Pandoc-style: never reused (counting defs
        // collides after a deletion). The PAINTED number derives from ref
        // order at paint time; the stored id is identity only.
        let n = self
            .doc
            .blocks()
            .kinds()
            .iter()
            .filter_map(|k| match k {
                BlockKind::FootnoteDef { id } => id.parse::<u64>().ok(),
                _ => None,
            })
            .chain(self.doc.spans().spans().iter().filter_map(|s| match &s.attr {
                InlineAttr::FootnoteRef(id) => id.parse::<u64>().ok(),
                _ => None,
            }))
            .max()
            .unwrap_or(0)
            + 1;
        let id = n.to_string();
        let sel = self.selected_range.clone();
        self.doc.edit_bytes(sel.start..sel.end, &id);
        let char_range = {
            let rope = self.doc.rope();
            rope.byte_to_char(sel.start)..rope.byte_to_char(sel.start + id.len())
        };
        self.doc
            .format_in_current_tx(char_range, InlineAttr::FootnoteRef(id.clone()), true);
        let len = self.doc.len_bytes();
        self.doc.edit_bytes(len..len, "\n");
        let def_block = self.doc.block_of_byte(self.doc.len_bytes());
        self.doc
            .set_block_kind_in_current_tx(def_block, BlockKind::FootnoteDef { id });
        let end = self.doc.len_bytes();
        self.selected_range = end..end;
        self.selection_reversed = false;
        self.cursor_affinity_down = false;
        self.goal_x = None;
        self.caret_attrs.clear();
        self.sync_mutations();
        self.mark_dirty();
        self.bump_activity();
        cx.notify();
    }

    /// Footnotes whose refs are visible in the viewport, as zone rows,
    /// plus the count of rows collapsed by the stacking policy (DESIGN
    /// §2-footnotes: all up to 3, then the 3 nearest the viewport center
    /// and a "+N more" row). Derived from the last painted frame.
    fn visible_footnotes(&self) -> (Vec<ZoneNote>, usize) {
        // Common case: a document with no footnote refs surfaces no zone notes.
        // Skip the whole-viewport paragraph scan and per-ref block search that
        // otherwise run unconditionally on every render (incl. idle blinks).
        if !self
            .doc
            .spans()
            .spans()
            .iter()
            .any(|s| matches!(s.attr, InlineAttr::FootnoteRef(_)))
        {
            return (Vec::new(), 0);
        }
        let Some(frame) = self.last_frame.as_ref() else {
            return (Vec::new(), 0);
        };
        let top = frame.scroll_top;
        let bottom = top + frame.bounds.size.height;
        let mut lo = usize::MAX;
        let mut hi = 0usize;
        for par in &frame.paragraphs {
            if par.top + par.height > top && par.top < bottom {
                lo = lo.min(par.range.start);
                hi = hi.max(par.range.end);
            }
        }
        // The frame may be one paint behind the document (compositor
        // throttling, big edits): clamp its byte ranges to the live rope.
        let len = self.doc.len_bytes();
        let (lo, hi) = (lo.min(len), hi.min(len));
        if lo >= hi {
            return (Vec::new(), 0);
        }
        let rope = self.doc.rope();
        let (clo, chi) = (rope.byte_to_char(lo), rope.byte_to_char(hi));
        let numbers = {
            let refs: Vec<(usize, &str)> = self
                .doc
                .spans()
                .spans()
                .iter()
                .filter_map(|s| match &s.attr {
                    InlineAttr::FootnoteRef(id) => Some((s.range.start, id.as_str())),
                    _ => None,
                })
                .collect();
            footnote_numbers(&refs, self.doc.blocks().kinds())
        };
        // (row, byte offset of the ref's start — for ordering/stacking)
        let mut out: Vec<(ZoneNote, usize)> = Vec::new();
        for span in self.doc.spans().spans() {
            let InlineAttr::FootnoteRef(id) = &span.attr else {
                continue;
            };
            if span.range.start >= chi || span.range.end <= clo {
                continue;
            }
            let no = match numbers.get(id) {
                Some(n) => *n,
                None => continue,
            };
            if out.iter().any(|(seen, _)| seen.no == no) {
                continue;
            }
            let Some(block) = self
                .doc
                .blocks()
                .kinds()
                .iter()
                .position(|k| matches!(k, BlockKind::FootnoteDef { id: d } if d == id))
            else {
                continue;
            };
            let start = rope.line_to_byte(block);
            let end = if block + 1 < rope.len_lines() {
                rope.line_to_byte(block + 1).saturating_sub(1)
            } else {
                rope.len_bytes()
            };
            // One visual home (H4): if the definition block is itself on
            // screen — the reader has scrolled down to the Footnotes
            // section — don't also mirror it in the page-bottom zone. The
            // zone is for footnotes whose def is off-screen above/below.
            if start < hi && end > lo {
                continue;
            }
            let full = self.doc.slice_bytes(start..end);
            let (def, def_len) = if full.chars().count() > 110 {
                let cut: String = full.chars().take(110).collect();
                let cut_len = cut.len();
                (cut + "…", cut_len)
            } else {
                let full_len = full.len();
                (full, full_len)
            };
            out.push((
                ZoneNote {
                    no,
                    def,
                    def_start: start,
                    def_len,
                    ref_byte: rope.char_to_byte(span.range.end),
                },
                rope.char_to_byte(span.range.start),
            ));
        }
        // Reading order, whatever order the span set stores.
        out.sort_by_key(|(_, ref_start)| *ref_start);
        let total = out.len();
        if total > 3 {
            // Keep the 3 nearest the viewport center; reading order stays.
            let center = f32::from(top + frame.bounds.size.height / 2.);
            let mut by_dist: Vec<usize> = (0..total).collect();
            by_dist.sort_by(|&a, &b| {
                let d = |i: usize| {
                    frame
                        .position_of(out[i].1, false)
                        .map_or(f32::MAX, |p| (f32::from(p.y) - center).abs())
                };
                d(a).total_cmp(&d(b))
            });
            let keep: HashSet<usize> = by_dist.into_iter().take(3).collect();
            out = out
                .into_iter()
                .enumerate()
                .filter(|(i, _)| keep.contains(i))
                .map(|(_, n)| n)
                .collect();
        }
        (out.into_iter().map(|(n, _)| n).collect(), total.saturating_sub(3))
    }

    /// One document, one window, one process: opening spawns a sibling
    /// instance (in-place document switching is backlogged).
    fn open_file(&mut self, _: &OpenFile, _: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open".into()),
        });
        cx.spawn(async move |_, _| {
            if let Ok(Ok(Some(paths))) = rx.await {
                let Some(path) = paths.first() else { return };
                let Ok(exe) = std::env::current_exe() else { return };
                if let Err(e) = std::process::Command::new(exe).arg(path).spawn() {
                    eprintln!("strop: open in new window: {e}");
                }
            }
        })
        .detach();
    }

    /// Save a copy: .md exports markdown, anything else a full .strop
    /// snapshot (history included). The open document keeps its own path —
    /// continuous save never re-targets.
    fn save_copy_as(&mut self, _: &SaveCopyAs, _: &mut Window, cx: &mut Context<Self>) {
        self.save_now();
        let Some(store) = &self.store else {
            eprintln!("strop: no document to copy");
            return;
        };
        let dir = store
            .path()
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let suggested = format!(
            "{} copy.strop",
            store.path().file_stem().and_then(|s| s.to_str()).unwrap_or("document")
        );
        let rx = cx.prompt_for_new_path(&dir, Some(&suggested));
        let md = strop_core::markdown::to_markdown(
            &self.doc.text(),
            self.doc.spans(),
            self.doc.blocks(),
        );
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                if path.extension().is_some_and(|e| e == "md") {
                    if let Err(e) = std::fs::write(&path, md) {
                        eprintln!("strop: save copy: {e}");
                    }
                } else {
                    this.update(cx, |editor: &mut Editor, _| {
                        if let Some(store) = &editor.store
                            && let Err(e) = store.save_copy_to(&path)
                        {
                            eprintln!("strop: save copy: {e}");
                        }
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    /// ctrl-m: note on the selection (or the word at the caret), then
    /// open the composer for its body.
    fn add_note(&mut self, _: &AddNote, window: &mut Window, cx: &mut Context<Self>) {
        let range = if self.selected_range.is_empty() {
            self.word_range_at(self.cursor_offset())
        } else {
            self.selected_range.clone()
        };
        if range.is_empty() {
            return;
        }
        let rope = self.doc.rope();
        let char_range = rope.byte_to_char(range.start)..rope.byte_to_char(range.end);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let id = self.doc.add_note(char_range, String::new(), now);
        self.mark_dirty();
        self.open_composer(id, String::new(), window, cx);
        self.bump_activity();
        cx.notify();
    }

    /// The SINGLE exit from `Composing`: persist the open composer's current
    /// text onto the note it edits, then demote that card to `Selected`. Every
    /// focus-changing action calls this first, so a composer is never stranded
    /// on a deselected card and its draft is never committed to a card the
    /// writer merely clicked. No-op unless a composer is actually open.
    fn resolve_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (id, body) = match &self.focus {
            CardFocus::Composing { id, input } => (*id, input.read(cx).content.clone()),
            _ => return,
        };
        self.doc.set_note_body(id, body);
        self.focus = CardFocus::Selected(id);
        self.mark_dirty();
        // The composer field just left the tree; hand keyboard control back to
        // the document so the next keystroke edits prose, not nothing. This is
        // the SINGLE place a composer exit restores focus — because EVERY exit
        // funnels through here, a lane click (selecting another card, done/×)
        // can no longer strand the keyboard the way it did when only
        // finish_composing refocused. Callers that want a *different* target
        // (open_composer focuses the new field) just re-focus after us.
        window.focus(&self.focus_handle, cx);
    }

    /// Leave the composer (Enter, Escape, or any focus loss). The draft is
    /// already the note's text and focus is restored by `resolve_composer`, so
    /// the card just stays selected, now showing what was written.
    fn finish_composing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.resolve_composer(window, cx);
        cx.notify();
    }

    /// Highlight a card without editing it (AI diagnoses; a note clicked via
    /// its anchor). Resolves any open composer first so the previous note's
    /// draft is saved and its composer never lingers.
    fn select_card(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        self.resolve_composer(window, cx);
        self.focus = CardFocus::Selected(id);
    }

    /// Drop all card selection (a click that hits no anchor). Resolves any
    /// open composer first.
    fn deselect_card(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.resolve_composer(window, cx);
        self.focus = CardFocus::Idle;
    }

    fn open_composer(
        &mut self,
        id: u64,
        body: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Switching composers: commit the one we're leaving before opening this.
        self.resolve_composer(window, cx);
        let input = cx.new(|cx| TextField::multiline(cx, body));
        cx.subscribe_in(
            &input,
            window,
            // Enter and Escape both end composing through the one exit; the live
            // text is already the note's body, so the event payload is moot.
            move |editor, _, _event: &TextFieldEvent, window, cx| {
                editor.finish_composing(window, cx);
            },
        )
        .detach();
        // Click-away commits immediately (low-latency: the input becomes a label
        // the instant focus leaves, not when some later click happens to resolve
        // it). Guarded on THIS composer still being open — switching to another
        // card or clicking the document already resolved it through its own path,
        // so the stale handle must not double-commit.
        let handle = input.read(cx).focus_handle.clone();
        let weak = cx.entity().downgrade();
        window
            .on_focus_out(&handle, cx, move |_, window, cx| {
                let Some(editor) = weak.upgrade() else { return };
                editor.update_checked(cx, |editor, cx| {
                    if editor.focus.composing_id() == Some(id) {
                        editor.finish_composing(window, cx);
                    }
                });
            })
            .detach();
        let input_focus = input.read(cx).focus_handle.clone();
        window.focus(&input_focus, cx);
        self.focus = CardFocus::Composing { id, input };
    }

    fn set_note_status(
        &mut self,
        id: u64,
        status: NoteStatus,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Commit any open draft first (the click may be on this card's own
        // done/×, or on another card while this one composes).
        self.resolve_composer(window, cx);
        // A resolved card leaves with a brief exit fade rather than blinking
        // out: snapshot its rendered slot BEFORE the model change (afterwards
        // it has no card to snapshot). The model itself commits immediately —
        // only the light lingers (departing), never the data.
        if matches!(status, NoteStatus::Done | NoteStatus::Dismissed)
            && let Some(card) =
                self.margin_cards(true).cards.into_iter().find(|c| c.id == id)
        {
            self.departing.push((card, Instant::now()));
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(CARD_RESOLVE + Duration::from_millis(50))
                    .await;
                this.update(cx, |editor: &mut Editor, cx| {
                    editor
                        .departing
                        .retain(|(_, since)| since.elapsed() < CARD_RESOLVE);
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        self.doc.set_note_status(id, status);
        // Thread terminals on the strip: when a card left and by which door.
        if matches!(status, NoteStatus::Done | NoteStatus::Dismissed) {
            self.doc
                .journal_mut()
                .record_event(strop_core::journal::JournalEvent::CardClosed {
                    t: strop_core::journal::now_ms(),
                    id,
                    resolved: status == NoteStatus::Done,
                });
        }
        if self.focus.active_id() == Some(id) {
            self.focus = CardFocus::Idle;
        }
        self.mark_dirty();
        self.bump_activity();
        cx.notify();
    }

    /// The thesis, running: an editorial pass that names problems as
    /// queries in the margin and never rewrites a word.
    fn run_diagnosis(&mut self, _: &RunDiagnosis, _: &mut Window, cx: &mut Context<Self>) {
        // ctrl-shift-d honours the session depth (`diagnosis_mode`, else config);
        // the mode is resolved HERE and pinned into the run, so a later menu row
        // can pin a different depth without disturbing this default.
        self.run_pass(PassKind::Diagnostic(self.effective_mode()), cx);
    }

    fn run_believing(&mut self, _: &RunBelieving, _: &mut Window, cx: &mut Context<Self>) {
        self.run_pass(PassKind::Believing, cx);
    }

    /// The door (DESIGN §4.4): flip between drafting (margin quiet) and
    /// reviewing (cards surface). The deliberate "register change" the
    /// research asks for — never inferred, never automatic in v1, so a
    /// drafting burst can never be interrupted by a wrong guess.
    fn toggle_review(&mut self, _: &ToggleReview, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_door(cx);
    }

    /// The door's flush-and-flip, shared by the ctrl-shift-r action, the editor
    /// menu's presence verb (`Reading ⇄ Away`), and the rig hook. Touching the
    /// door is an explicit attention shift — a parked pass lands right now,
    /// mid-burst or not.
    fn toggle_door(&mut self, cx: &mut Context<Self>) {
        self.flush_deferred_pass(cx);
        self.drafting = !self.drafting;
        cx.notify();
    }

    /// Open the door from outside the editor (the tutorial, whose whole
    /// point is to show the margin demo cards).
    pub fn enter_reviewing(&mut self) {
        self.drafting = false;
    }

    /// Open diagnosis/believing cards the closed door is currently holding
    /// back (the rail's count). Notes (the writer's own) are never counted —
    /// the door quiets the editor, not the writer.
    fn resting_diagnoses(&self) -> usize {
        self.doc
            .notes()
            .open()
            .filter(|n| n.kind == NoteKind::Diagnosis)
            .count()
    }

    /// Copy-level diagnoses suppressed while a developmental one is still
    /// open — the mandatory altitude order (don't polish prose that the
    /// structural edit may cut; Reedsy, Sommers). Zero unless both exist.
    fn suppressed_copy(&self) -> usize {
        let has_dev = self
            .doc
            .notes()
            .open()
            .any(|n| n.kind == NoteKind::Diagnosis && n.level == "developmental");
        if !has_dev {
            return 0;
        }
        self.doc
            .notes()
            .open()
            .filter(|n| n.kind == NoteKind::Diagnosis && n.level == "copy")
            .count()
    }

    /// The effective levels-of-edit depth: session override, else config,
    /// else "line" (Perkins' default register).
    fn effective_mode(&self) -> String {
        let mode = self
            .diagnosis_mode
            .clone()
            .unwrap_or_else(|| self.config.ai.mode.clone());
        match mode.as_str() {
            "developmental" | "copy" => mode,
            _ => "line".to_owned(),
        }
    }

    /// The editor button's live face inputs (impl 04 §0). `configured` gates
    /// NeedsSetup even before any pass runs (ai_status is None until one does),
    /// so an unconfigured provider is announced on the button, not just when a
    /// pass is attempted.
    fn face_inputs(&self) -> FaceInputs {
        FaceInputs {
            needs_setup: !self.config.ai.configured()
                || matches!(self.ai_status, Some(AiStatus::NeedsSetup { .. })),
            error: matches!(self.ai_status, Some(AiStatus::Error { .. })),
            cooking: matches!(self.ai_status, Some(AiStatus::Running { .. })),
            ready: self.deferred_pass.is_some(),
            door_open: !self.drafting,
        }
    }

    fn editor_face(&self) -> EditorFace {
        face_for(&self.face_inputs())
    }

    /// Open editorial queries (open diagnosis cards) — the button's "{n} open"
    /// and the footer's "{open} queries open". `resting_diagnoses` already
    /// counts exactly these, regardless of the door.
    fn open_query_count(&self) -> usize {
        self.resting_diagnoses()
    }

    /// Queries the writer RESOLVED (marked Done) in this document — the footer's
    /// "{resolved} resolved". Deliberately Done-ONLY: Dismissed cards are
    /// permanent suppression tombstones (`is_suppressed` leans on them), not work
    /// done, so counting them would inflate the tally with silenced nags (review
    /// mid). Bounded by the document's real annotation set.
    fn resolved_query_count(&self) -> usize {
        self.doc
            .notes()
            .notes()
            .iter()
            .filter(|n| n.kind == NoteKind::Diagnosis && n.status == NoteStatus::Done)
            .count()
    }

    /// The copy row's gate (impl 04 §0): while any developmental query is open,
    /// polishing prose the structural edit may cut is premature (the altitude
    /// order — Reedsy, Sommers). This is the REQUEST-side complement of
    /// `suppressed_copy`'s RESULT-hold — both key off the same signal (an open
    /// developmental diagnosis), so the button and the lane never disagree. It
    /// releases the moment the last developmental query closes, no pass re-run.
    fn copy_gated(&self) -> bool {
        copy_gate_active(self.doc.notes().open().map(|n| (n.kind, n.level.as_str())))
    }

    /// Toggle the editor button's dropdown (impl 04 §0). A pass in flight leaves
    /// the rows inert (see the menu render), but the button still opens so the
    /// pulse and the door verb stay reachable.
    fn toggle_editor_menu(&mut self, cx: &mut Context<Self>) {
        self.editor_menu_open = !self.editor_menu_open;
        cx.notify();
    }

    fn close_editor_menu(&mut self, cx: &mut Context<Self>) {
        if self.editor_menu_open {
            self.editor_menu_open = false;
            cx.notify();
        }
    }

    fn run_pass(&mut self, kind: PassKind, cx: &mut Context<Self>) {
        if matches!(self.ai_status, Some(AiStatus::Running { .. })) {
            return;
        }
        // Re-read the config: edit → save → retry must work without a
        // restart (the guided config-file flow's contract).
        self.config = crate::config::load();
        let ai = self.config.ai.clone();
        if !ai.configured() {
            // Remember what was asked: once a provider exists, THIS exact
            // pass (believing/doubting/depth) runs without the writer re-issuing.
            self.pending_pass = Some(kind);
            self.ai_generation += 1;
            self.ai_status = Some(AiStatus::NeedsSetup { local_model: None });
            self.probe_local_model(self.ai_generation, cx);
            cx.notify();
            return;
        }
        self.last_pass = kind.clone();
        // You asked to evaluate — open the door, so results land in a margin
        // the writer is actually looking at (and any earlier resting cards
        // come back into view alongside them). Asking again is also the most
        // explicit attention shift there is: a still-parked earlier pass
        // lands now rather than racing the run it just triggered.
        self.flush_deferred_pass(cx);
        self.drafting = false;
        // Scope: the selection if there is one, else the MANUSCRIPT (never the
        // compost — the rail is the writer's private scrap box, asides.md §1).
        // Capped — a 24k-char window is plenty for an editorial pass.
        let scope = if self.selected_range.is_empty() {
            self.doc.manuscript_slice().0
        } else {
            self.doc.slice_bytes(self.selected_range.clone())
        };
        let scope: String = scope.chars().take(24_000).collect();
        self.ai_generation += 1;
        let generation = self.ai_generation;
        self.ai_status = Some(AiStatus::Running {
            label: format!("{} · {}", kind.run_label(), ai.model),
        });
        cx.notify();

        cx.spawn(async move |this, cx| {
            let api_key = ai.resolved_api_key();
            let base_url = ai.base_url.clone();
            let model = ai.model.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    let client = strop_core::llm::LlmClient::new(&base_url, &api_key, &model);
                    // The pinned kind picks the prompt; a Diagnostic carries its
                    // own depth (so `system_prompt` never falls through to line).
                    let system = match &kind {
                        PassKind::Believing => strop_core::diagnose::believing_system_prompt(),
                        PassKind::Doubting => strop_core::diagnose::doubting_system_prompt(),
                        PassKind::Diagnostic(mode) => strop_core::diagnose::system_prompt(mode),
                    };
                    let user = strop_core::diagnose::user_prompt(&scope);
                    client
                        .chat(&system, &user, 2048)
                        .map_err(AiFailure::Llm)
                        .and_then(|response| {
                            strop_core::diagnose::parse(&response).map_err(AiFailure::Parse)
                        })
                })
                .await;
            this.update(cx, |editor: &mut Editor, cx| {
                if editor.ai_generation != generation {
                    return; // cancelled or superseded — drop silently
                }
                match result {
                    Ok(diagnoses) => editor.deliver_pass(diagnoses, generation, cx),
                    Err(failure) => {
                        editor.ai_status =
                            Some(failure.into_status(&editor.config.ai.base_url, &editor.config.ai.model));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Success notes fade; errors and setup cards stay until acted on.
    fn schedule_status_fade(&mut self, generation: u64, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(6))
                .await;
            this.update(cx, |editor: &mut Editor, cx| {
                if editor.ai_generation == generation
                    && matches!(editor.ai_status, Some(AiStatus::Note { .. }))
                {
                    editor.ai_status = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn cancel_ai_run(&mut self, _: &CancelAiRun, _: &mut Window, cx: &mut Context<Self>) {
        // UI-level cancel: the response of the abandoned generation is
        // ignored when it lands (no need to abort the request itself).
        // A parked deferral dies with its generation too (flush checks it),
        // but drop it eagerly so nothing lingers.
        self.ai_generation += 1;
        self.ai_status = None;
        self.pending_pass = None;
        self.deferred_pass = None;
        cx.notify();
    }

    /// Is the writer inside a live typing burst right now? True while the
    /// last real buffer edit is younger than `TYPING_LULL`. This one predicate
    /// is the entire "when may AI results land" model — no gaze tracking, no
    /// idle timers, no modes: prose recently changed ⇒ hold; still ⇒ land.
    fn typing_burst_live(&self) -> bool {
        self.last_text_edit
            .is_some_and(|t| t.elapsed() < TYPING_LULL)
    }

    /// Integrate a completed pass into the document NOW: anchor the quotes
    /// against the current text, add the cards, and say out loud what stuck.
    /// The single landing site for both the direct path (results arrive in a
    /// lull) and the deferred path (flushed after a burst).
    fn integrate_pass(
        &mut self,
        diagnoses: Vec<strop_core::diagnose::Diagnosis>,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let count = diagnoses.len();
        // Anchor against the text as it is NOW — quotes
        // that no longer match are dropped.
        self.diagnosis_pass += 1;
        let pass_id = self.diagnosis_pass;
        // Anchor within the MANUSCRIPT slice, then rebase every range back by
        // the manuscript base — so a card can never land in the compost, even
        // when the writer set aside an earlier draft of the very sentence
        // (review H40, TRAP 4). Existing notes are rebased into the slice too,
        // so suppression still matches. When there is no rail the base is 0 and
        // this is identical to anchoring against the whole document.
        let base = self.doc.manuscript_base_char();
        let mtext = self.doc.manuscript_slice().0;
        let mut existing = Annotations::default();
        for n in self.doc.notes().notes() {
            if n.range.start >= base {
                let mut n = n.clone();
                n.range = (n.range.start - base)..(n.range.end - base);
                existing.push(n);
            }
        }
        let mut annotations =
            strop_core::diagnose::to_annotations(&mtext, diagnoses, &existing, now, pass_id);
        for a in &mut annotations {
            a.range.start += base;
            a.range.end += base;
        }
        let kept = annotations.len();
        self.doc.add_diagnoses(annotations);
        // The strip's veil: which read landed, when, how many queries.
        self.doc
            .journal_mut()
            .record_event(strop_core::journal::JournalEvent::Pass {
                t: strop_core::journal::now_ms(),
                mode: self.last_pass.mode_str().to_owned(),
                cards: kept as u32,
            });
        self.mark_dirty();
        // The landed cards get one entrance fade (CARD_APPEAR); the marks
        // clear right after it finishes so nothing ever re-fades. Their ids
        // are exactly the open notes stamped with this pass.
        if kept > 0 {
            self.appearing = self
                .doc
                .notes()
                .open()
                .filter(|n| n.pass_id == pass_id)
                .map(|n| n.id)
                .collect();
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(CARD_APPEAR + Duration::from_millis(150))
                    .await;
                this.update(cx, |editor: &mut Editor, cx| {
                    editor.appearing.clear();
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        // Silent success is the second invisibility bug:
        // 0 anchored must be said out loud.
        self.ai_status = Some(AiStatus::Note {
            title: match kept {
                0 if count == 0 => {
                    "Pass complete — the editor found nothing to flag".to_owned()
                }
                0 => "Pass complete — no quote matched the current text".to_owned(),
                n => format!("{n} margin quer{} anchored", if n == 1 { "y" } else { "ies" }),
            },
            detail: if count > kept && kept > 0 {
                format!("{} dropped (stale quotes)", count - kept)
            } else {
                String::new()
            },
        });
        self.schedule_status_fade(generation, cx);
        cx.notify();
    }

    /// The arrival gate — the ONE decision of the reveal clock. Mid-typing-
    /// burst, results WAIT: landing now would pop squiggles into the sentence
    /// being typed and re-pack the lane under the writer's eyes. The lull
    /// watcher integrates them the moment the burst ends; scroll or the door
    /// flush sooner. Held UN-anchored on purpose — quotes anchor against the
    /// text as it stands at reveal, not at arrival. In a lull (the common
    /// case: the writer asked and is waiting), results land immediately.
    fn deliver_pass(
        &mut self,
        diagnoses: Vec<strop_core::diagnose::Diagnosis>,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        if self.typing_burst_live() {
            self.deferred_pass = Some(DeferredPass { diagnoses, generation });
            self.watch_for_lull(cx);
        } else {
            self.integrate_pass(diagnoses, generation, cx);
        }
    }

    /// Land the parked pass, if its generation still stands. Called by the
    /// lull watcher and by the explicit attention shifts (scroll, the door) —
    /// any of them may fire first; the `take()` makes the flush idempotent.
    fn flush_deferred_pass(&mut self, cx: &mut Context<Self>) {
        let Some(d) = self.deferred_pass.take() else {
            return;
        };
        if d.generation != self.ai_generation {
            return; // cancelled or superseded while parked
        }
        self.integrate_pass(d.diagnoses, d.generation, cx);
    }

    /// Poll (4×/s) until the typing burst ends, then flush the parked pass.
    /// Exits when the deferral is gone — flushed here, flushed by a scroll or
    /// the door, or dropped by a cancel — so at most one watcher does work,
    /// and a re-armed deferral just rides the loop that is already running.
    fn watch_for_lull(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let done = this.update(cx, |editor: &mut Editor, cx| {
                    if editor.deferred_pass.is_none() {
                        return true;
                    }
                    if editor.typing_burst_live() {
                        return false;
                    }
                    editor.flush_deferred_pass(cx);
                    true
                });
                if done.unwrap_or(true) {
                    break;
                }
            }
        })
        .detach();
    }

    /// Background probe for a local OpenAI-compatible model (Ollama's
    /// default port). Reuses `list_models` — connection-refused returns
    /// instantly when nothing is listening, so the cost on machines
    /// without it is negligible. On success it upgrades the live
    /// NeedsSetup card to offer a key-free, fully-local first pass.
    fn probe_local_model(&mut self, generation: u64, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let found = cx
                .background_executor()
                .spawn(async move {
                    let client = strop_core::llm::LlmClient::new(LOCAL_OLLAMA_URL, "ollama", "");
                    client.list_models().ok().and_then(pick_local_model)
                })
                .await;
            let Some(model) = found else { return };
            this.update(cx, |editor: &mut Editor, cx| {
                // Only if the writer is still looking at the same unfulfilled
                // setup card (not cancelled, not already configured/running).
                if editor.ai_generation == generation
                    && matches!(editor.ai_status, Some(AiStatus::NeedsSetup { .. }))
                {
                    editor.ai_status = Some(AiStatus::NeedsSetup {
                        local_model: Some(model),
                    });
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// One-click local path: point the config at Ollama, then run the pass
    /// the writer already asked for. No key, no signup, no leaving the app —
    /// and the manuscript never leaves the machine.
    fn use_local_model(&mut self, model: String, cx: &mut Context<Self>) {
        match crate::config::save_ai(LOCAL_OLLAMA_URL, None, &model) {
            Ok(_) => {
                self.config = crate::config::load();
                self.ai_status = None;
                self.run_pending_pass(cx);
            }
            Err(e) => {
                self.ai_status = Some(AiStatus::Error {
                    title: "Couldn't save the local provider".to_owned(),
                    detail: e,
                });
                cx.notify();
            }
        }
    }

    /// Run whatever pass was queued while the provider was being set up;
    /// defaults to a diagnosis if the queue is empty (the writer reached
    /// setup some other way — running the core feature is the right guess).
    fn run_pending_pass(&mut self, cx: &mut Context<Self>) {
        let kind = self
            .pending_pass
            .take()
            .unwrap_or_else(|| PassKind::Diagnostic(self.effective_mode()));
        self.run_pass(kind, cx);
    }

    /// Guided config-file flow: ensure the commented template exists,
    /// open it in the system editor, and say what happens next.
    fn open_ai_config(&mut self, _: &OpenAiConfig, _: &mut Window, cx: &mut Context<Self>) {
        let path = crate::config::write_template_if_missing();
        crate::files::open_external(&path);
        self.ai_generation += 1;
        let generation = self.ai_generation;
        self.ai_status = Some(AiStatus::Note {
            title: "Opened config.toml in your editor".to_owned(),
            detail: "Fill [ai] base_url / api_key / model, save, and run the pass again — \
                     Strop re-reads the file every time."
                .to_owned(),
        });
        self.schedule_status_fade(generation, cx);
        cx.notify();
    }

    /// A 1-token chat call: moves 401s from run-time to setup-time. On a
    /// provider error it also fetches /models — that list IS the picker.
    fn test_ai_connection(&mut self, _: &TestAiConnection, _: &mut Window, cx: &mut Context<Self>) {
        self.config = crate::config::load();
        let ai = self.config.ai.clone();
        if !ai.configured() {
            self.ai_generation += 1;
            self.ai_status = Some(AiStatus::NeedsSetup { local_model: None });
            self.probe_local_model(self.ai_generation, cx);
            cx.notify();
            return;
        }
        self.ai_generation += 1;
        let generation = self.ai_generation;
        self.ai_status = Some(AiStatus::Running {
            label: format!("testing {} · {}", host_of(&ai.base_url), ai.model),
        });
        cx.notify();
        cx.spawn(async move |this, cx| {
            let api_key = ai.resolved_api_key();
            let base_url = ai.base_url.clone();
            let model = ai.model.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    let client = strop_core::llm::LlmClient::new(&base_url, &api_key, &model);
                    let started = std::time::Instant::now();
                    match client.chat("Reply with exactly: ok", "ok?", 16) {
                        Ok(_) => Ok(started.elapsed().as_millis() as u64),
                        Err(e) => {
                            let models = match &e {
                                strop_core::llm::LlmError::Provider(_) => {
                                    client.list_models().ok()
                                }
                                _ => None,
                            };
                            Err((e, models))
                        }
                    }
                })
                .await;
            this.update(cx, |editor: &mut Editor, cx| {
                if editor.ai_generation != generation {
                    return;
                }
                editor.ai_status = Some(match result {
                    Ok(ms) => {
                        editor.schedule_status_fade(generation, cx);
                        AiStatus::Note {
                            title: format!(
                                "OK — {} via {} · {ms}ms",
                                ai.model,
                                host_of(&ai.base_url)
                            ),
                            detail: String::new(),
                        }
                    }
                    Err((e, models)) => {
                        let mut status = AiFailure::Llm(e).into_status(&ai.base_url, &ai.model);
                        if let (AiStatus::Error { detail, .. }, Some(list)) = (&mut status, models)
                            && !list.is_empty()
                        {
                            let shown: Vec<String> = list.into_iter().take(8).collect();
                            detail.push_str(&format!("\nAvailable models: {}", shown.join(", ")));
                        }
                        status
                    }
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// The AI settings panel (F4): the in-app surface for the core
    /// onboarding task. Prefilled from config.toml; Save writes back
    /// through toml_edit, so the file stays the storage and hand edits
    /// stay respected (DESIGN §0 directive 3).
    fn open_ai_settings(&mut self, _: &OpenAiSettings, window: &mut Window, cx: &mut Context<Self>) {
        if self.ai_settings.is_some() {
            return;
        }
        let cfg = crate::config::load();
        let base_url = cx.new(|cx| TextField::settings(cx, cfg.ai.base_url.clone(), false));
        let api_key = cx.new(|cx| TextField::settings(cx, cfg.ai.api_key.clone(), true));
        let model = cx.new(|cx| TextField::settings(cx, cfg.ai.model.clone(), false));
        for input in [&base_url, &api_key, &model] {
            cx.subscribe_in(
                input,
                window,
                |editor, _, event: &TextFieldEvent, window, cx| match event {
                    TextFieldEvent::Commit(_) => editor.ai_settings_commit(cx),
                    TextFieldEvent::Cancel => editor.close_ai_settings(window, cx),
                },
            )
            .detach();
        }
        // Typing in the model field re-filters the list live.
        cx.observe(&model, |editor, _, cx| {
            if let Some(panel) = &mut editor.ai_settings {
                panel.selected = 0;
            }
            cx.notify();
        })
        .detach();
        cx.observe(&base_url, |_, _, cx| cx.notify()).detach();
        cx.observe(&api_key, |_, _, cx| cx.notify()).detach();
        let focus = base_url.read(cx).focus_handle.clone();
        window.focus(&focus, cx);
        self.ai_settings = Some(AiSettings {
            base_url,
            api_key,
            model,
            test: AiSettingsTest::Idle,
            models: Vec::new(),
            selected: 0,
            models_note: None,
        });
        cx.notify();
    }

    fn close_ai_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ai_settings = None;
        self.ai_settings_generation += 1; // drop in-flight test/list replies
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    /// (base_url, api_key, model) as currently typed, trimmed.
    fn ai_settings_values(&self, cx: &Context<Self>) -> Option<(String, String, String)> {
        let panel = self.ai_settings.as_ref()?;
        Some((
            panel.base_url.read(cx).content.trim().to_owned(),
            panel.api_key.read(cx).content.trim().to_owned(),
            panel.model.read(cx).content.trim().to_owned(),
        ))
    }

    /// Models matching the model field (case-insensitive substring);
    /// empty field = the whole list.
    fn ai_settings_filtered(&self, cx: &Context<Self>) -> Vec<String> {
        let Some(panel) = self.ai_settings.as_ref() else {
            return Vec::new();
        };
        let query = panel.model.read(cx).content.trim().to_lowercase();
        panel
            .models
            .iter()
            .filter(|m| query.is_empty() || m.to_lowercase().contains(&query))
            .cloned()
            .collect()
    }

    /// Enter in any panel field: pick from the visible model list if it
    /// still offers something new; otherwise run the test call.
    fn ai_settings_commit(&mut self, cx: &mut Context<Self>) {
        let filtered = self.ai_settings_filtered(cx);
        let Some(panel) = &mut self.ai_settings else {
            return;
        };
        if let Some(pick) = filtered.get(panel.selected.min(filtered.len().saturating_sub(1))) {
            let already = panel.model.read(cx).content.trim() == pick.as_str();
            if !already {
                let pick = pick.clone();
                panel.model.update_checked(cx, |input, cx| {
                    input.content = pick;
                    cx.notify();
                });
                cx.notify();
                return;
            }
        }
        self.ai_settings_test(cx);
    }

    fn settings_up(&mut self, _: &SettingsUp, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(panel) = &mut self.ai_settings {
            panel.selected = panel.selected.saturating_sub(1);
            cx.notify();
        }
    }

    fn settings_down(&mut self, _: &SettingsDown, _: &mut Window, cx: &mut Context<Self>) {
        let len = self.ai_settings_filtered(cx).len();
        if let Some(panel) = &mut self.ai_settings
            && len > 0
        {
            panel.selected = (panel.selected + 1).min(len - 1);
            cx.notify();
        }
    }

    /// [Test]: the same 1-token chat as Test AI Connection, but against
    /// the values typed in the form (not yet saved), reported inline.
    fn ai_settings_test(&mut self, cx: &mut Context<Self>) {
        let Some((base_url, key, model)) = self.ai_settings_values(cx) else {
            return;
        };
        let Some(panel) = &mut self.ai_settings else {
            return;
        };
        if base_url.is_empty() || model.is_empty() {
            panel.test = AiSettingsTest::Failed {
                message: "base URL and model are both needed for a test call".into(),
            };
            cx.notify();
            return;
        }
        panel.test = AiSettingsTest::Running;
        self.ai_settings_generation += 1;
        let generation = self.ai_settings_generation;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let api_key = resolved_key(&key);
            let result = cx
                .background_executor()
                .spawn(async move {
                    let client = strop_core::llm::LlmClient::new(&base_url, &api_key, &model);
                    let started = std::time::Instant::now();
                    client
                        .chat("Reply with exactly: ok", "ok?", 16)
                        .map(|_| started.elapsed().as_millis() as u64)
                })
                .await;
            this.update(cx, |editor: &mut Editor, cx| {
                if editor.ai_settings_generation != generation {
                    return;
                }
                let (base_url, model) = match editor.ai_settings_values(cx) {
                    Some((b, _, m)) => (b, m),
                    None => return,
                };
                match result {
                    Ok(ms) => {
                        if let Some(panel) = &mut editor.ai_settings {
                            panel.test = AiSettingsTest::Ok { ms };
                        }
                        // A provider that just answered can also tell us
                        // what it serves: refresh the picker for free.
                        editor.ai_settings_list_models(cx);
                    }
                    Err(e) => {
                        let AiStatus::Error { title, detail } =
                            AiFailure::Llm(e).into_status(&base_url, &model)
                        else {
                            unreachable!("into_status always errors")
                        };
                        if let Some(panel) = &mut editor.ai_settings {
                            panel.test = AiSettingsTest::Failed {
                                message: if detail.is_empty() {
                                    title
                                } else {
                                    format!("{title} — {detail}")
                                },
                            };
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// A provider chip was clicked: drop its base URL into the field, clear
    /// stale test state, and — for the local provider, which needs no key —
    /// list its models straight away so the writer can pick and go.
    fn ai_settings_pick_provider(&mut self, base_url: &'static str, list: bool, cx: &mut Context<Self>) {
        let Some(panel) = &mut self.ai_settings else {
            return;
        };
        panel.test = AiSettingsTest::Idle;
        if !base_url.is_empty() {
            panel.base_url.update_checked(cx, |input, cx| {
                input.content = base_url.to_owned();
                cx.notify();
            });
        }
        if list {
            self.ai_settings_list_models(cx);
        }
        cx.notify();
    }

    /// [List models]: GET {base}/models on the background executor; the
    /// result is the pickable, filterable list (Open WebUI's flow).
    fn ai_settings_list_models(&mut self, cx: &mut Context<Self>) {
        let Some((base_url, key, _)) = self.ai_settings_values(cx) else {
            return;
        };
        let Some(panel) = &mut self.ai_settings else {
            return;
        };
        if base_url.is_empty() {
            panel.models_note = Some("base URL is needed to list models".into());
            cx.notify();
            return;
        }
        panel.models_note = Some("fetching model list…".into());
        self.ai_settings_generation += 1;
        let generation = self.ai_settings_generation;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let api_key = resolved_key(&key);
            let result = cx
                .background_executor()
                .spawn(async move {
                    let client = strop_core::llm::LlmClient::new(&base_url, &api_key, "");
                    client.list_models()
                })
                .await;
            this.update(cx, |editor: &mut Editor, cx| {
                if editor.ai_settings_generation != generation {
                    return;
                }
                let Some(panel) = &mut editor.ai_settings else {
                    return;
                };
                match result {
                    Ok(models) if models.is_empty() => {
                        panel.models = Vec::new();
                        panel.models_note = Some("the provider returned an empty list".into());
                    }
                    Ok(models) => {
                        panel.models = models;
                        panel.selected = 0;
                        panel.models_note = None;
                    }
                    Err(e) => {
                        panel.models_note = Some(format!("couldn't list models: {e}"));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// [Save] / ctrl-enter: write through toml_edit (comments and unknown
    /// keys survive), reload the live config, close, confirm in the margin.
    fn save_ai_settings(&mut self, _: &SaveAiSettings, window: &mut Window, cx: &mut Context<Self>) {
        let Some((base_url, key, model)) = self.ai_settings_values(cx) else {
            return;
        };
        // STROP_API_KEY wins over the file; never write a key the
        // environment is already supplying (the panel says so too).
        let key_from_env = env_key_set();
        let api_key = (!key_from_env).then_some(key.as_str());
        match crate::config::save_ai(&base_url, api_key, &model) {
            Ok(_) => {
                self.config = crate::config::load();
                self.close_ai_settings(window, cx);
                self.ai_generation += 1;
                // The whole point of setup was to run a pass — so if one was
                // queued (the writer pressed ctrl-shift-d, hit the wall, and
                // came here), answer it now instead of making them re-ask.
                if self.config.ai.configured() && self.pending_pass.is_some() {
                    self.run_pending_pass(cx);
                } else {
                    let generation = self.ai_generation;
                    self.ai_status = Some(AiStatus::Note {
                        title: if self.config.ai.configured() {
                            format!("AI configured: {model} via {}", host_of(&base_url))
                        } else {
                            "AI settings saved (provider still incomplete)".to_owned()
                        },
                        detail: if self.config.ai.configured() {
                            "Run a pass with ctrl-shift-d.".to_owned()
                        } else {
                            String::new()
                        },
                    });
                    self.schedule_status_fade(generation, cx);
                }
            }
            Err(e) => {
                if let Some(panel) = &mut self.ai_settings {
                    panel.test = AiSettingsTest::Failed { message: e };
                }
            }
        }
        cx.notify();
    }

    fn set_diagnosis_mode(&mut self, mode: &str, cx: &mut Context<Self>) {
        self.diagnosis_mode = Some(mode.to_owned());
        self.ai_generation += 1;
        let generation = self.ai_generation;
        self.ai_status = Some(AiStatus::Note {
            title: format!("Diagnosis mode: {mode}"),
            detail: match mode {
                "developmental" => "Structure and argument — what the piece wants to be.",
                "copy" => "Consistency, usage, mechanics — nothing stylistic.",
                _ => "Clarity, momentum, dead weight — the default register.",
            }
            .to_owned(),
        });
        self.schedule_status_fade(generation, cx);
        cx.notify();
    }

    fn mode_developmental(
        &mut self,
        _: &DiagnosisModeDevelopmental,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_diagnosis_mode("developmental", cx);
    }

    fn mode_line(&mut self, _: &DiagnosisModeLine, _: &mut Window, cx: &mut Context<Self>) {
        self.set_diagnosis_mode("line", cx);
    }

    fn mode_copy(&mut self, _: &DiagnosisModeCopy, _: &mut Window, cx: &mut Context<Self>) {
        self.set_diagnosis_mode("copy", cx);
    }

    /// The omnibox (DESIGN §2-omnibox): one top-centre field, prefix-
    /// dispatched — plain text finds, `>` runs a command, `@` jumps to a
    /// heading. `palette_input` is its field; the `palette_*` machinery is
    /// shared across all three modes. `initial` seeds the query (and so the
    /// mode): "" = find, ">" = command, "@" = heading.
    fn open_omni(&mut self, initial: String, window: &mut Window, cx: &mut Context<Self>) {
        // A fresh field every open (the old entity drops); PaletteInput
        // context gives it up/down row motion and the editing chords.
        let input = cx.new(|cx| TextField::palette(cx, initial.clone()));
        cx.observe(&input, |editor, input, cx| {
            let q = input.read(cx).content.clone();
            editor.palette_query = q.clone();
            editor.palette_selected = 0; // query changed: selection restarts
            // Find previews live — the match scrolls into view as you type,
            // behind the omnibox (this is what dissolves the old bottom-strip
            // "match hidden under the find field" bug).
            if matches!(omni_mode(&q).0, OmniMode::Find) {
                editor.omni_apply_match(0, cx);
            } else {
                // Selection restarted at row 0 (above): the list should
                // follow it back to the top, not stay scrolled wherever the
                // previous query's browsing left it.
                editor.omni_scroll_into_view(0);
                if let Some(sel) = editor.omni_return.clone() {
                    // Leaving find mode (a `>` or `@` prefix) would park the
                    // preview selection on the last match — and a command
                    // executed next would act on it (the cross-mode selection
                    // leak, extraction audit #23). Walk it home; deleting the
                    // prefix re-enters find and re-previews.
                    let max = editor.doc.len_bytes();
                    editor.selected_range = sel.start.min(max)..sel.end.min(max);
                    editor.selection_reversed = false;
                }
            }
            cx.notify();
        })
        .detach();
        cx.subscribe_in(
            &input,
            window,
            |editor, input, event: &TextFieldEvent, window, cx| match event {
                TextFieldEvent::Commit(_) => {
                    let q = input.read(cx).content.clone();
                    // Find: Enter cycles to the next match, bar stays open.
                    // Command/heading: Enter runs the selected row and closes.
                    if matches!(omni_mode(&q).0, OmniMode::Find) {
                        editor.omni_find_next(cx);
                    } else {
                        editor.execute_palette_entry(&q, editor.palette_selected, window, cx);
                    }
                }
                TextFieldEvent::Cancel => {
                    // Esc restores the selection the omnibar opened on (S3,
                    // P13): the find preview walked `selected_range` across
                    // matches; cancel walks it home and scrolls it back in.
                    if let Some(sel) = editor.omni_return.take() {
                        let max = editor.doc.len_bytes();
                        editor.selected_range = sel.start.min(max)..sel.end.min(max);
                        editor.selection_reversed = false;
                        editor.autoscroll_request = true;
                    }
                    editor.close_palette(window, cx)
                }
            },
        )
        .detach();
        let input_focus = input.read(cx).focus_handle.clone();
        window.focus(&input_focus, cx);
        self.omni_return = Some(self.selected_range.clone());
        self.palette_input = Some(input);
        self.replace_input = None;
        self.palette_selected = 0;
        self.palette_query = initial.clone();
        // The handle outlives any one open/close cycle (it lives on the
        // Editor); a fresh open must not inherit wherever the last session
        // left the list scrolled.
        self.omni_scroll_into_view(0);
        // Read once per open, not per keystroke; executions write through to
        // disk AND to this copy, so the session stays self-consistent.
        self.palette_freq = crate::files::load_palette_freq();
        if !initial.is_empty() && matches!(omni_mode(&initial).0, OmniMode::Find) {
            self.omni_apply_match(0, cx);
        }
        cx.notify();
    }

    /// ctrl-f / the titlebar search pill: the omnibox in find mode, seeded
    /// with the selection so "select, ctrl-f" searches for it.
    fn find(&mut self, _: &Find, window: &mut Window, cx: &mut Context<Self>) {
        let seed = if self.selected_range.is_empty() {
            String::new()
        } else {
            self.doc.slice_bytes(self.selected_range.clone())
        };
        self.open_omni(seed, window, cx);
    }

    /// The find-mode match ranges for the current query (empty otherwise).
    /// Case-insensitive (first-lowercase-char folding — exact for RU/EN,
    /// approximate for ß-class expansions).
    fn omni_match_ranges(&self) -> Vec<Range<usize>> {
        match omni_mode(&self.palette_query) {
            (OmniMode::Find, rest) => self.find_matches(rest),
            _ => Vec::new(),
        }
    }

    /// Move the document selection to the ix-th find match and scroll it into
    /// view (live preview for type / arrow / Enter / click). No-op off find
    /// mode or with no matches.
    fn omni_apply_match(&mut self, ix: usize, cx: &mut Context<Self>) {
        let matches = self.omni_match_ranges();
        if matches.is_empty() {
            return;
        }
        let ix = ix.min(matches.len() - 1);
        self.palette_selected = ix;
        self.omni_scroll_into_view(ix);
        self.selected_range = matches[ix].clone();
        self.selection_reversed = false;
        self.bump_activity();
        cx.notify();
    }

    /// Enter in find mode: advance to the next match, wrapping.
    fn omni_find_next(&mut self, cx: &mut Context<Self>) {
        let matches = self.omni_match_ranges();
        if matches.is_empty() {
            return;
        }
        let next = (self.palette_selected + 1) % matches.len();
        self.omni_apply_match(next, cx);
    }

    /// Tab hops between the omnibox query field and the replace field (the
    /// action bubbles up from the PaletteInput/NoteInput context to here),
    /// and cycles the AI settings panel's fields.
    fn note_tab(&mut self, _: &FieldTab, window: &mut Window, cx: &mut Context<Self>) {
        // AI settings panel: tab cycles base URL → key → model → base URL.
        if let Some(panel) = &self.ai_settings {
            let fields = [&panel.base_url, &panel.api_key, &panel.model];
            let focused = fields
                .iter()
                .position(|f| f.read(cx).focus_handle.is_focused(window))
                .unwrap_or(2);
            let next = fields[(focused + 1) % fields.len()].read(cx).focus_handle.clone();
            window.focus(&next, cx);
            cx.notify();
            return;
        }
        let (Some(omni), Some(rep)) = (self.palette_input.clone(), self.replace_input.clone())
        else {
            return;
        };
        if omni.read(cx).focus_handle.is_focused(window) {
            let h = rep.read(cx).focus_handle.clone();
            window.focus(&h, cx);
        } else {
            let h = omni.read(cx).focus_handle.clone();
            window.focus(&h, cx);
        }
        cx.notify();
    }

    /// ctrl-shift-p / F10 / the titlebar menu button: the omnibox in command
    /// mode — every command the app has, searchable, chord on the row (the
    /// menu bar of a chrome-minimal editor, PLAN.md E1). Already in command
    /// mode → close; in another mode → switch to it; closed → open.
    fn toggle_palette(&mut self, _: &TogglePalette, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette_input.is_some()
            && matches!(omni_mode(&self.palette_query).0, OmniMode::Command)
        {
            self.close_palette(window, cx);
            return;
        }
        self.open_omni(">".into(), window, cx);
    }

    fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette_input = None;
        self.replace_input = None;
        self.omni_return = None;
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    /// One window per document, one process per window: simple, and two
    /// windows can never fight over the same CRDT file.
    fn new_document(&mut self, _: &NewDocument, _: &mut Window, _: &mut Context<Self>) {
        crate::files::new_window_blank();
    }

    fn reveal_in_files(&mut self, _: &RevealInFiles, _: &mut Window, _: &mut Context<Self>) {
        if let Some(store) = &self.store {
            crate::files::reveal(store.path());
        }
    }

    fn copy_document_path(&mut self, _: &CopyDocumentPath, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(store) = &self.store {
            cx.write_to_clipboard(ClipboardItem::new_string(
                store.path().display().to_string(),
            ));
        }
    }

    /// F2 or clicking the titlebar name: rename the document in place —
    /// the file on disk is renamed too (visible-from-birth contract).
    fn rename_document(&mut self, _: &RenameDocument, window: &mut Window, cx: &mut Context<Self>) {
        let Some(store) = &self.store else { return };
        if self.doc_rename_input.is_some() {
            return;
        }
        let stem = store
            .path()
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_owned();
        let input = cx.new(|cx| TextField::single(cx, stem));
        cx.observe(&input, |_, _, cx| cx.notify()).detach();
        cx.subscribe_in(
            &input,
            window,
            |editor, _, event: &TextFieldEvent, window, cx| match event {
                TextFieldEvent::Commit(title) => editor.finish_rename(title.clone(), window, cx),
                TextFieldEvent::Cancel => {
                    editor.doc_rename_input = None;
                    window.focus(&editor.focus_handle, cx);
                    cx.notify();
                }
            },
        )
        .detach();
        // Click-away commits the rename (the title is a real edit, like the
        // note composer's body — losing focus should save it, not drop it).
        // Guarded on the field still being open so an Enter-then-blur, which
        // already finished, doesn't rename a second time.
        let handle = input.read(cx).focus_handle.clone();
        let weak = cx.entity().downgrade();
        window
            .on_focus_out(&handle, cx, move |_, window, cx| {
                let Some(editor) = weak.upgrade() else { return };
                editor.update_checked(cx, |editor, cx| {
                    if let Some(field) = editor.doc_rename_input.clone() {
                        let title = field.read(cx).content.clone();
                        editor.finish_rename(title, window, cx);
                    }
                });
            })
            .detach();
        let input_focus = input.read(cx).focus_handle.clone();
        window.focus(&input_focus, cx);
        self.doc_rename_input = Some(input);
        cx.notify();
    }

    fn finish_rename(&mut self, title: String, window: &mut Window, cx: &mut Context<Self>) {
        self.doc_rename_input = None;
        window.focus(&self.focus_handle, cx);
        let Some(stem) = crate::files::stem_from_title(&title) else {
            cx.notify();
            return;
        };
        if let Some(store) = &mut self.store {
            let old = store.path().to_owned();
            let new_path = old
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .join(format!("{stem}.strop"));
            match store.rename_file(new_path) {
                Ok(()) => {
                    crate::files::replace_recent(&old, store.path());
                    window.set_window_title(&format!("{stem} — Strop"));
                }
                Err(e) => eprintln!("strop: rename: {e}"),
            }
        }
        cx.notify();
    }

    /// Commands first (ranked), then recent documents that match — the
    /// palette is both the menu and the door to the other essays. The
    /// empty state opens with a Frequent section (DESIGN §3.3): the five
    /// most-executed commands, which repeat in their home sections below.
    /// The rows for the current query, dispatched by prefix mode: find
    /// matches (with a line snippet), commands + frequent + recents, or
    /// headings filtered by the same fuzzy matcher.
    fn omni_rows(&self, query: &str) -> Vec<OmniRow> {
        let (mode, rest) = omni_mode(query);
        match mode {
            OmniMode::Find => self
                .find_matches(rest)
                .into_iter()
                .take(100)
                .map(|range| {
                    let line = self.doc.rope().byte_to_line(range.start.min(self.doc.len_bytes()));
                    OmniRow::Match {
                        line,
                        snippet: self.omni_line_snippet(line),
                    }
                })
                .collect(),
            OmniMode::Heading => self
                .outline_items()
                .into_iter()
                .filter(|(_, _, text, _)| {
                    rest.is_empty() || crate::commands::score_text(rest, text).is_some()
                })
                .map(|(_, level, text, byte)| OmniRow::Heading { byte, level, text })
                .collect(),
            OmniMode::Command => {
                let mut rows: Vec<OmniRow> = Vec::new();
                if rest.trim().is_empty() {
                    rows.extend(
                        crate::commands::frequent(&self.palette_freq)
                            .into_iter()
                            .map(OmniRow::Frequent),
                    );
                }
                rows.extend(
                    crate::commands::ranked_with_freq(rest, &self.palette_freq)
                        .into_iter()
                        .map(OmniRow::Cmd),
                );
                let current = self.store.as_ref().map(|s| s.path().to_owned());
                for p in crate::files::recents() {
                    if Some(&p) == current.as_ref() {
                        continue;
                    }
                    let name = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    if rest.trim().is_empty()
                        || crate::commands::score_text(rest.trim(), name).is_some()
                    {
                        rows.push(OmniRow::Recent(p));
                    }
                }
                rows
            }
        }
    }

    /// A trimmed, length-capped preview of a document line, for find rows.
    fn omni_line_snippet(&self, line: usize) -> String {
        let rope = self.doc.rope();
        if line >= rope.len_lines() {
            return String::new();
        }
        let raw: String = rope.line(line).chars().take(160).collect();
        let trimmed = raw.trim();
        let mut s: String = trimmed.chars().take(80).collect();
        if trimmed.chars().count() > 80 {
            s.push('…');
        }
        s
    }

    fn execute_palette_entry(
        &mut self,
        query: &str,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let rows = self.omni_rows(query);
        let Some(row) = rows.get(ix) else {
            return;
        };
        match row {
            OmniRow::Cmd(cmd) | OmniRow::Frequent(cmd) => {
                let cmd = *cmd;
                let action = (cmd.make)();
                // Frequency writes through on every execution (DESIGN
                // §3.3) — disk and the session's in-memory copy together.
                let count = crate::files::bump_palette_freq(cmd.label);
                self.palette_freq.insert(cmd.label.to_owned(), count);
                self.maybe_whisper_chord(cmd, cx);
                // Close first: focus returns to the document, so the action
                // lands exactly as if its chord had been pressed there.
                self.close_palette(window, cx);
                window.dispatch_action(action, cx);
            }
            OmniRow::Recent(path) => {
                let path = path.clone();
                self.close_palette(window, cx);
                crate::files::open_in_new_window(&path);
            }
            OmniRow::Match { .. } => {
                // Clicking/selecting a find row jumps to it and keeps the
                // omnibox open (find is iterative — Esc commits).
                self.omni_apply_match(ix, cx);
            }
            OmniRow::Heading { byte, .. } => {
                let byte = *byte;
                self.close_palette(window, cx);
                self.set_cursor(byte.min(self.doc.len_bytes()), false, cx);
            }
        }
    }

    /// The solution reveal, post-hoc and opt-out-by-ignoring (DESIGN
    /// §3.5): a palette execution of a chorded command earns one muted
    /// "that chord exists" whisper — at most once per app session
    /// (VimGolf's engine; Bederson's flow rules forbid more), fading on
    /// the same timer pattern as AI status notes. Chord-less commands
    /// never whisper: there is nothing faster to reveal.
    fn maybe_whisper_chord(&mut self, cmd: &crate::commands::Command, cx: &mut Context<Self>) {
        let Some(keys) = cmd.keys else { return };
        if self.chord_whisper_shown {
            return;
        }
        self.chord_whisper_shown = true;
        self.chord_whisper = Some(format!("Chord: {keys} does this directly"));
        self.chord_whisper_generation += 1;
        let generation = self.chord_whisper_generation;
        cx.spawn(async move |this, cx| {
            // Same fade window as schedule_status_fade's success notes.
            cx.background_executor()
                .timer(std::time::Duration::from_secs(6))
                .await;
            this.update(cx, |editor: &mut Editor, cx| {
                if editor.chord_whisper_generation == generation {
                    editor.chord_whisper = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn palette_up(&mut self, _: &PaletteUp, _: &mut Window, cx: &mut Context<Self>) {
        let sel = self.palette_selected.saturating_sub(1);
        // Find mode previews the row live (selection + scroll); the others
        // just move the highlight.
        if matches!(omni_mode(&self.palette_query).0, OmniMode::Find) {
            self.omni_apply_match(sel, cx);
        } else {
            self.palette_selected = sel;
            self.omni_scroll_into_view(sel);
            cx.notify();
        }
    }

    fn palette_down(&mut self, _: &PaletteDown, _: &mut Window, cx: &mut Context<Self>) {
        let len = self
            .palette_input
            .as_ref()
            .map_or(0, |i| self.omni_rows(&i.read(cx).content).len());
        if len == 0 {
            return;
        }
        let sel = (self.palette_selected + 1).min(len - 1);
        if matches!(omni_mode(&self.palette_query).0, OmniMode::Find) {
            self.omni_apply_match(sel, cx);
        } else {
            self.palette_selected = sel;
            self.omni_scroll_into_view(sel);
            cx.notify();
        }
    }

    /// Nudge the omni-list to keep row `ix` visible (the fold bug this
    /// closes: keyboard `palette_up`/`palette_down` used to move the
    /// highlight past the visible window without scrolling — the writer
    /// navigated blind). `ScrollHandle::scroll_to_item` requests the
    /// *minimal* correction and is a no-op when `ix` is already on-screen,
    /// so this never fights a manual mouse-wheel scroll that hasn't been
    /// invalidated by a fresh selection.
    fn omni_scroll_into_view(&self, ix: usize) {
        let rows = self.omni_rows(&self.palette_query);
        if rows.is_empty() {
            return;
        }
        let ix = ix.min(rows.len() - 1);
        if ix == 0 {
            // A direct reset, not an item-based scroll: on the very first
            // frame the omnibox ever opens, GPUI hasn't measured this div's
            // own viewport bounds yet (`ScrollHandle::bounds` lags a frame
            // behind the child bounds `scroll_to_item` math needs), so it
            // can undershoot by a row on a cold open. Setting the offset
            // straight to zero sidesteps that — and also reaches row 0's
            // section header in grouped mode (bare `>`), which `scroll_to_
            // item(0)` alone would only promise is "somewhere visible".
            self.omni_scroll.set_offset(point(px(0.), px(0.)));
            return;
        }
        let (mode, rest) = omni_mode(&self.palette_query);
        let grouped = mode == OmniMode::Command && rest.trim().is_empty();
        self.omni_scroll.scroll_to_item(omni_child_index(&rows, grouped, ix));
    }

    fn render_omni(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let field_w = omni_field_width(window);
        let input = self.palette_input.clone().expect("omnibox open");
        let query = input.read(cx).content.clone();
        let (mode, rest) = omni_mode(&query);
        let rows = self.omni_rows(&query);
        let selected = self.palette_selected.min(rows.len().saturating_sub(1));
        // Sections group only the command empty-state (Frequent / File / …);
        // find and heading rows are flat.
        let grouped = mode == OmniMode::Command && rest.trim().is_empty();
        let home = crate::paths::home_dir().to_string_lossy().into_owned();
        let mut list = div()
            .id("omni-list")
            .track_scroll(&self.omni_scroll)
            .max_h(px(420.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .pb(px(6.));
        let mut last_section = "";
        for (ix, row) in rows.iter().enumerate() {
            if grouped {
                let section = omni_row_section(row);
                if section != last_section {
                    last_section = section;
                    list = list.child(
                        div()
                            .px(px(12.))
                            .pt(px(10.))
                            .pb(px(2.))
                            .text_size(px(10.))
                            .text_color(rgb(MUTED_COLOR))
                            .child(section.to_uppercase()),
                    );
                }
            }
            // Left label, optional right detail, optional leading gutter
            // (find's line number / heading's level indent).
            let (label, right, lead): (String, Option<String>, Option<String>) = match row {
                OmniRow::Cmd(cmd) | OmniRow::Frequent(cmd) => {
                    (cmd.label.to_owned(), cmd.keys.map(|k| k.to_owned()), None)
                }
                OmniRow::Recent(p) => {
                    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("?").to_owned();
                    let dir = p
                        .parent()
                        .map(|d| d.display().to_string().replace(&home, "~"))
                        .unwrap_or_default();
                    (stem, Some(dir), None)
                }
                OmniRow::Match { line, snippet, .. } => {
                    (snippet.clone(), None, Some(format!("{}", line + 1)))
                }
                OmniRow::Heading { level, text, .. } => {
                    let indent = "  ".repeat((*level as usize).saturating_sub(1));
                    (format!("{indent}{text}"), Some(format!("H{level}")), None)
                }
            };
            list = list.child(
                div()
                    .id(("omni-row", ix))
                    .px(px(12.))
                    .py(px(4.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .cursor(CursorStyle::PointingHand)
                    .when(ix == selected, |d| d.bg(rgba(0x1A1A1812u32)))
                    .hover(|d| d.bg(rgba(0x1A1A180Au32)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |editor, _: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            let q = editor
                                .palette_input
                                .as_ref()
                                .map(|i| i.read(cx).content.clone())
                                .unwrap_or_default();
                            editor.execute_palette_entry(&q, ix, window, cx);
                        }),
                    )
                    .when_some(lead, |d, lead| {
                        d.child(
                            div()
                                .w(px(28.))
                                .flex_shrink_0()
                                .text_size(px(11.))
                                .text_color(rgb(MUTED_COLOR))
                                .child(lead),
                        )
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_size(px(13.))
                            .text_color(rgb(TEXT_COLOR))
                            .child(label),
                    )
                    .when_some(right, |d, right| {
                        d.child(
                            div()
                                .flex_shrink_0()
                                .text_size(px(11.))
                                .text_color(rgb(MUTED_COLOR))
                                .max_w(px(220.))
                                .truncate()
                                .child(right),
                        )
                    }),
            );
        }
        if rows.is_empty() {
            let msg = match mode {
                OmniMode::Find if rest.is_empty() => {
                    "Type to find · > for commands · @ for headings"
                }
                OmniMode::Find => "No matches",
                OmniMode::Heading => "No headings",
                OmniMode::Command => "No matching command",
            };
            list = list.child(
                div()
                    .px(px(12.))
                    .py(px(8.))
                    .text_size(px(13.))
                    .text_color(rgb(MUTED_COLOR))
                    .child(msg),
            );
        }
        // The query input lives in the TITLEBAR now (06 §1) — the card is the
        // field's dropdown: results (+ the replace row, a sanctioned distinct
        // function, S6), never a second query input. The match counter rides
        // the titlebar field. A mousedown on card chrome that isn't a row
        // refocuses the query (H2): the card must never look active while
        // keystrokes route to the prose behind it.
        let query_focus = input.read(cx).focus_handle.clone();
        div()
            .absolute()
            .top(px(BAR_HEIGHT + 2.))
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .child(
                // The dropdown wears the field's own width (06 §1): equal
                // boxes on the same centre line — every edge agrees, and the
                // card reads as the field's shadow, not a second object.
                div().w(px(field_w)).child(
                    div()
                        .w(px(field_w))
                        .bg(rgb(0xFCFAF4))
                        .border_1()
                        .border_color(rgb(RULE_COLOR))
                        .rounded(px(8.))
                        .shadow_lg()
                        .font_family("PT Serif")
                        .flex()
                        .flex_col()
                        // §0.6: clicks inside the omnibox stay in it (rows
                        // handle their own); clicks outside reach the
                        // document's handler, which light-dismisses it. The
                        // wheel is contained the same way.
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            cx.stop_propagation();
                            window.focus(&query_focus, cx);
                        })
                        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                        .when_some(self.replace_input.clone(), |d, rep| {
                            d.child(
                                div()
                                    .p(px(8.))
                                    .border_b_1()
                                    .border_color(rgb(RULE_COLOR))
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .text_size(px(13.))
                                    .child(div().flex_shrink_0().text_color(rgb(MUTED_COLOR)).child("Replace"))
                                    .child(div().flex_1().min_w(px(0.)).child(rep))
                                    .child(
                                        div()
                                            .id("replace-all")
                                            .flex_shrink_0()
                                            .px(px(8.))
                                            .py(px(1.))
                                            .rounded(px(4.))
                                            .cursor(CursorStyle::PointingHand)
                                            .bg(rgb(0xE8DFC8))
                                            .hover(|d| d.bg(rgb(0xDFD3B0)))
                                            .text_size(px(12.))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                                                    cx.stop_propagation();
                                                    editor.replace_all(cx);
                                                }),
                                            )
                                            .child("All"),
                                    ),
                            )
                        })
                        .child(list),
                ),
            )
    }

    /// ctrl-h: open the omnibox in find mode (if not already there) and add
    /// the replace field beneath the query.
    fn replace(&mut self, _: &Replace, window: &mut Window, cx: &mut Context<Self>) {
        let in_find = self.palette_input.is_some()
            && matches!(omni_mode(&self.palette_query).0, OmniMode::Find);
        if !in_find {
            self.find(&Find, window, cx);
        }
        if self.replace_input.is_some() {
            return;
        }
        let input = cx.new(|cx| TextField::single(cx, String::new()));
        cx.observe(&input, |_, _, cx| cx.notify()).detach();
        cx.subscribe_in(
            &input,
            window,
            |editor, _, event: &TextFieldEvent, window, cx| match event {
                TextFieldEvent::Commit(replacement) => {
                    editor.replace_current(replacement.clone(), cx);
                }
                TextFieldEvent::Cancel => editor.close_palette(window, cx),
            },
        )
        .detach();
        // Already searching? Move focus to the new field so the replacement
        // types straight in. Opened fresh (no query yet) → keep the query
        // focused so the search term comes first.
        if in_find {
            let h = input.read(cx).focus_handle.clone();
            window.focus(&h, cx);
        }
        self.replace_input = Some(input);
        cx.notify();
    }

    /// Replace the current match and advance to the next.
    fn replace_current(&mut self, replacement: String, cx: &mut Context<Self>) {
        let (OmniMode::Find, query) = omni_mode(&self.palette_query) else {
            return;
        };
        let query = query.to_owned();
        let matches = self.find_matches(&query);
        if matches.is_empty() {
            return;
        }
        let ix = self.palette_selected % matches.len();
        let target = matches[ix].clone();
        self.doc.edit_bytes(target.clone(), &replacement);
        let cursor = target.start + replacement.len();
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.sync_mutations();
        self.mark_dirty();
        // Land on what is now the match at the same index.
        let matches = self.find_matches(&query);
        if !matches.is_empty() {
            self.palette_selected = ix % matches.len();
            self.omni_scroll_into_view(self.palette_selected);
            self.selected_range = matches[self.palette_selected].clone();
        }
        self.bump_activity();
        cx.notify();
    }

    /// Replace every match in one undoable step (reverse order keeps
    /// earlier offsets valid; the buffer coalesces nothing here — each
    /// edit is its own transaction, so group manually via restore… no:
    /// edits run back-to-front so a single undo per edit would be tedious —
    /// instead snapshot once by funnelling through one transaction).
    fn replace_all(&mut self, cx: &mut Context<Self>) {
        let Some(rep) = self.replace_input.clone() else {
            return;
        };
        let (OmniMode::Find, query) = omni_mode(&self.palette_query) else {
            return;
        };
        let query = query.to_owned();
        let replacement = rep.read(cx).content.clone();
        let matches = self.find_matches(&query);
        if matches.is_empty() {
            return;
        }
        for m in matches.iter().rev() {
            self.doc.edit_bytes(m.clone(), &replacement);
        }
        let count = matches.len();
        self.selected_range = 0..0;
        self.selection_reversed = false;
        self.sync_mutations();
        self.mark_dirty();
        self.palette_selected = 0;
        self.omni_scroll_into_view(0);
        eprintln!("strop: replaced {count} matches");
        self.bump_activity();
        cx.notify();
    }

    fn find_matches(&self, query: &str) -> Vec<Range<usize>> {
        if query.is_empty() {
            return Vec::new();
        }
        let text = self.doc.text();
        let fold = |c: char| c.to_lowercase().next().unwrap_or(c);
        let needle: Vec<char> = query.chars().map(fold).collect();
        let hay: Vec<(usize, char)> = text
            .char_indices()
            .map(|(b, c)| (b, fold(c)))
            .collect();
        let mut out = Vec::new();
        if needle.len() > hay.len() {
            return out;
        }
        let mut i = 0;
        while i + needle.len() <= hay.len() {
            if hay[i..i + needle.len()]
                .iter()
                .map(|(_, c)| *c)
                .eq(needle.iter().copied())
            {
                let start = hay[i].0;
                let end = hay
                    .get(i + needle.len())
                    .map(|(b, _)| *b)
                    .unwrap_or(text.len());
                out.push(start..end);
                i += needle.len();
                if out.len() >= 500 {
                    break;
                }
            } else {
                i += 1;
            }
        }
        out
    }

    fn render_alt_strip(&self) -> Option<impl IntoElement> {
        let (_, input) = self.alt_input.clone()?;
        Some(
            div()
                .absolute()
                .bottom_0()
                .left_0()
                .right_0()
                .bg(rgb(0xF4F1EA))
                .border_t_1()
                .border_color(rgb(RULE_COLOR))
                .px(px(28.))
                .py(px(8.))
                .flex()
                .items_center()
                .gap(px(8.))
                .font_family("PT Serif")
                .text_size(px(13.))
                .child(div().text_color(rgb(MUTED_COLOR)).child("Alt text:"))
                .child(div().flex_1().child(input)),
        )
    }

    pub fn save_now(&mut self) {
        let perf = std::env::var_os("STROP_PERF").map(|_| std::time::Instant::now());
        self.sync_mutations();
        // Settle the journal's tail run: persisted runs are immutable once
        // written, so the store only ever pushes finished items. Saves fire
        // at ≥1s idle, a natural run boundary anyway.
        self.doc.journal_mut().settle();
        if let Some(store) = &self.store {
            match store.save_with_state(
                self.doc.spans(),
                self.doc.blocks(),
                // 50, not 200: each persisted undo entry snapshots the FULL
                // (SpanSet, BlockMap, Annotations) — measured 1.58 MB of JSON
                // at 200 on a card-heavy document, rewritten into the
                // append-only oplog on every editing save. Fifty cross-session
                // undo steps is still deep (in-session undo is unaffected);
                // the persisted tail is what must stay proportionate.
                &self.doc.export_history(50),
                self.doc.notes(),
                self.doc.journal(),
                self.doc.graveyard(),
            ) {
                Ok(()) => self.store_dirty = false,
                Err(e) => eprintln!("strop: failed to save {}: {e}", store.path().display()),
            }
        }
        if let Some(t) = perf {
            eprintln!("strop-perf: save_now {:?}", t.elapsed());
        }
    }

    /// Would text typed at the caret inherit `attr` from the existing spans?
    /// Mirrors `SpanSet::apply_op` insertion rules: strictly inside any
    /// span, or at the right edge of an expanding one.
    fn caret_inherits(&self, attr: &InlineAttr) -> bool {
        let pos = self
            .doc
            .rope()
            .byte_to_char(self.selected_range.start);
        self.doc.spans().spans().iter().any(|s| {
            s.attr == *attr
                && (s.range.start < pos && pos < s.range.end
                    || s.range.end == pos && s.attr.expands())
        })
    }

    /// Is `attr` active at the current selection/caret (for toggle logic
    /// and toolbar state)?
    fn attr_active(&self, attr: &InlineAttr) -> bool {
        if self.selected_range.is_empty() {
            if let Some((_, on)) = self.caret_attrs.iter().find(|(a, _)| a == attr) {
                return *on;
            }
            self.caret_inherits(attr)
        } else {
            let rope = self.doc.rope();
            let range = rope.byte_to_char(self.selected_range.start)
                ..rope.byte_to_char(self.selected_range.end);
            self.doc.spans().covers(range, attr)
        }
    }

    /// Toggle an inline attribute: over a selection, fully-covered removes
    /// and anything less applies; at a bare caret, sets a sticky attr for
    /// the next typed text (the universal rich-editor convention).
    fn toggle_span(&mut self, attr: InlineAttr, cx: &mut Context<Self>) {
        // Read-only while previewing the past: formatting the past is
        // undefined. The strip pulses its banner (the uniform refusal, Bug B);
        // the panel just no-ops.
        if self.strip.is_parked() {
            self.pulse_strip(cx);
            return;
        }
        if self.history_view.is_some() {
            return;
        }
        if self.selected_range.is_empty() {
            let target = !self.attr_active(&attr);
            self.caret_attrs.retain(|(a, _)| a != &attr);
            self.caret_attrs.push((attr, target));
            cx.notify();
            return;
        }
        let rope = self.doc.rope();
        let range =
            rope.byte_to_char(self.selected_range.start)..rope.byte_to_char(self.selected_range.end);
        self.doc.toggle_format(range, attr);
        self.mark_dirty();
        cx.notify();
    }

    /// Open the left flank's link argument-field (docs/impl/03-flanks.md §0.1).
    /// CAPTURES the selection's CHAR range now and applies to THAT on commit —
    /// never a re-read of `selected_range`, which could drift (Link doesn't
    /// expand under the caret). Pre-filled from the current target so editing a
    /// link shows it. The rename-input idiom: Enter commits, Esc cancels, a
    /// click-away blur commits (`commit_field_on_blur`).
    fn open_link_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Read-only while previewing the past (mirrors toggle_span).
        if self.strip.is_parked() {
            self.pulse_strip(cx);
            return;
        }
        if self.history_view.is_some() || self.selected_range.is_empty() {
            return;
        }
        let rope = self.doc.rope();
        let range = rope.byte_to_char(self.selected_range.start)
            ..rope.byte_to_char(self.selected_range.end);
        let seed = self.doc.link_over(range.clone()).unwrap_or_default();
        let input = cx.new(|cx| TextField::single(cx, seed));
        cx.subscribe_in(
            &input,
            window,
            move |editor, _, event: &TextFieldEvent, window, cx| match event {
                TextFieldEvent::Commit(url) => editor.commit_link(url.clone(), window, cx),
                TextFieldEvent::Cancel => editor.cancel_link(window, cx),
            },
        )
        .detach();
        // Blur commits (e.g. the palette steals focus): `still` re-checks the
        // field is the one open, so a stale handle never double-commits.
        self.commit_field_on_blur(&input, window, cx, |e| {
            e.link_input.as_ref().map(|(_, f)| f.clone())
        });
        let handle = input.read(cx).focus_handle.clone();
        window.focus(&handle, cx);
        self.link_input = Some((range, input));
        cx.notify();
    }

    /// Commit the link over its CAPTURED range: a non-empty URL sets it, an empty
    /// one removes it (spec §0.1). Focus returns to the document; the flank stays
    /// up so the writer can apply another mark.
    fn commit_link(&mut self, url: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some((range, _)) = self.link_input.take() else {
            return;
        };
        let url = url.trim().to_owned();
        self.doc
            .set_link(range, if url.is_empty() { None } else { Some(url) });
        self.mark_dirty();
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    /// Esc from the link field cancels just the argument and returns to the flank
    /// (the two-level Esc, finding 59) — the selection and popover stay.
    fn cancel_link(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.link_input = None;
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    /// Tear down BOTH flanks (review H20): acting on any right-menu row clears
    /// them before its result (a composer card, a pass, an aside) takes the lane,
    /// so only one pinned object ever sits at the selection's y. Also drops a
    /// half-typed link argument.
    fn dismiss_flanks(&mut self, cx: &mut Context<Self>) {
        self.selection_popover = false;
        self.link_input = None;
        cx.notify();
    }

    /// Start the cursor-blink heartbeat. GNOME-style: solid while typing,
    /// blinking when idle, solid again (and quiet — no repaints) after 10s.
    ///
    /// STROP_TEST_STILL (the visual harness, scripts/wflip.sh) suppresses
    /// the heartbeat entirely: captures are byte-compared, and a blinking
    /// cursor is nondeterminism. `cursor_visible` starts true and nothing
    /// ever toggles it, so the caret stays solid in every frame.
    pub fn start_blink(&self, cx: &mut Context<Self>) {
        if std::env::var("STROP_TEST_STILL").is_ok() {
            return;
        }
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(530))
                    .await;
                let alive = this.update(cx, |editor: &mut Editor, cx| {
                    let idle = editor.last_input.elapsed();
                    let visible = if idle < Duration::from_millis(530)
                        || idle > Duration::from_secs(10)
                    {
                        true
                    } else {
                        !editor.cursor_visible
                    };
                    if visible != editor.cursor_visible {
                        editor.cursor_visible = visible;
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// Any cursor-affecting input: reset blink and schedule scroll-to-cursor.
    fn bump_activity(&mut self) {
        self.last_input = Instant::now();
        self.cursor_visible = true;
        self.autoscroll_request = true;
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    /// Collapse the selection to `offset`. Keeps `goal_x`; non-vertical
    /// callers go through `move_to`, which clears it.
    fn set_cursor(&mut self, offset: usize, affinity_down: bool, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        self.cursor_affinity_down = affinity_down;
        self.caret_attrs.clear();
        // Remember the manuscript caret so Esc from a compost caret returns
        // exactly here (review B3). A no-rail doc is all manuscript.
        if offset >= self.doc.char_to_byte(self.doc.manuscript_base_char()) {
            self.last_manuscript_caret = offset;
        }
        self.bump_activity();
        cx.notify();
    }

    /// Extend the selection's moving end to `offset`. Keeps `goal_x`.
    fn extend_cursor(&mut self, offset: usize, affinity_down: bool, cx: &mut Context<Self>) {
        // Never let a selection straddle the boundary (review B4): clamp the
        // moving end into the fixed anchor's region.
        let anchor = if self.selection_reversed {
            self.selected_range.end
        } else {
            self.selected_range.start
        };
        let offset = self.clamp_to_region(anchor, offset);
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        self.cursor_affinity_down = affinity_down;
        self.caret_attrs.clear();
        self.bump_activity();
        self.publish_primary(cx);
        cx.notify();
    }

    /// Linux PRIMARY-selection contract: any selection (mouse or keyboard)
    /// is published; middle-click pastes it. With auto_copy_selection
    /// (config), the regular clipboard gets it too — Kirill's habit.
    fn publish_primary(&self, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            let text = self.doc.slice_bytes(self.selected_range.clone());
            if self.config.auto_copy_selection {
                cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
            }
            // The PRIMARY selection is an X11/Wayland concept; gpui exposes it
            // only on Linux/BSD. macOS and Windows have no PRIMARY — the
            // regular clipboard above is the only target there.
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            cx.write_to_primary(ClipboardItem::new_string(text));
            #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
            let _ = text;
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.goal_x = None;
        self.set_cursor(offset, false, cx);
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.goal_x = None;
        self.extend_cursor(offset, false, cx);
    }

    // -- Boundary helpers (byte offsets) ------------------------------------

    /// Start/end of the *paragraph* (rope line) containing `offset`.
    fn paragraph_bounds(&self, offset: usize) -> (usize, usize) {
        let rope = self.doc.rope();
        let line_ix = rope.byte_to_line(offset);
        let start = rope.line_to_byte(line_ix);
        let end = if line_ix + 1 < rope.len_lines() {
            rope.line_to_byte(line_ix + 1).saturating_sub(1)
        } else {
            rope.len_bytes()
        };
        (start, end)
    }

    /// Start/end of the *visual* line under the cursor, with the affinity
    /// that keeps the cursor on it. Falls back to paragraph bounds when no
    /// frame exists yet.
    fn visual_line_bounds(&self, offset: usize) -> ((usize, bool), (usize, bool)) {
        if let Some(frame) = self.last_frame.as_ref()
            && let Some((par_ix, line, _)) =
                frame.cursor_position(offset, self.cursor_affinity_down)
        {
            let par = &frame.paragraphs[par_ix];
            let start = par.range.start + par.line_start(line);
            let end = par.range.start + par.line_end(line);
            return ((start, line > 0), (end, false));
        }
        let (start, end) = self.paragraph_bounds(offset);
        ((start, false), (end, false))
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        if offset == 0 {
            return 0;
        }
        let (start, _) = self.paragraph_bounds(offset);
        if offset == start {
            return offset - 1; // step over the newline
        }
        let line = self.doc.slice_bytes(start..offset);
        line.grapheme_indices(true)
            .next_back()
            .map_or(start, |(ix, _)| start + ix)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        let len = self.doc.len_bytes();
        if offset >= len {
            return len;
        }
        let (_, end) = self.paragraph_bounds(offset);
        if offset == end {
            return offset + 1; // step over the newline
        }
        let line = self.doc.slice_bytes(offset..end);
        line.grapheme_indices(true)
            .nth(1)
            .map_or(end, |(ix, _)| offset + ix)
    }

    fn previous_word_boundary(&self, offset: usize) -> usize {
        previous_word_boundary(&self.doc, offset)
    }

    fn next_word_boundary(&self, offset: usize) -> usize {
        next_word_boundary(&self.doc, offset)
    }

    /// GTK/Windows paragraph motion: to start of current paragraph, or of
    /// the previous one when already at a start.
    fn previous_paragraph_boundary(&self, offset: usize) -> usize {
        let (start, _) = self.paragraph_bounds(offset);
        if offset > start {
            start
        } else if start > 0 {
            self.paragraph_bounds(start - 1).0
        } else {
            0
        }
    }

    /// To end of current paragraph, or of the next one when already at an end.
    fn next_paragraph_boundary(&self, offset: usize) -> usize {
        let (_, end) = self.paragraph_bounds(offset);
        let len = self.doc.len_bytes();
        if offset < end {
            end
        } else if end < len {
            self.paragraph_bounds(end + 1).1
        } else {
            len
        }
    }

    /// Word-bound segment containing `offset` (for double-click selection).
    fn word_range_at(&self, offset: usize) -> Range<usize> {
        let (start, end) = self.paragraph_bounds(offset);
        if start == end {
            return start..end;
        }
        let local = (offset - start).min(end - start - 1);
        let line = self.doc.slice_bytes(start..end);
        for (ix, seg) in line.split_word_bound_indices() {
            if ix <= local && local < ix + seg.len() {
                return start + ix..start + ix + seg.len();
            }
        }
        start..end
    }

    // -- Vertical movement ----------------------------------------------------

    fn vertical_by(&mut self, direction: i64, select: bool, cx: &mut Context<Self>) {
        let cursor = self.cursor_offset();
        let Some(frame) = self.last_frame.as_ref() else {
            return;
        };
        let Some((par_ix, line_ix, x)) = frame.cursor_position(cursor, self.cursor_affinity_down)
        else {
            return;
        };
        let x = self.goal_x.unwrap_or(x);

        let par = &frame.paragraphs[par_ix];
        let target = if direction > 0 {
            if line_ix + 1 < par.line_count() {
                Some((par_ix, line_ix + 1))
            } else if par_ix + 1 < frame.paragraphs.len() {
                Some((par_ix + 1, 0))
            } else {
                None
            }
        } else if line_ix > 0 {
            Some((par_ix, line_ix - 1))
        } else if par_ix > 0 {
            let prev = &frame.paragraphs[par_ix - 1];
            Some((par_ix - 1, prev.line_count() - 1))
        } else {
            None
        };

        let (offset, affinity) = match target {
            Some((p, l)) => {
                let par = &frame.paragraphs[p];
                let (ix, aff) = par.index_at(l, x);
                (par.range.start + ix, aff)
            }
            // First line up -> document start; last line down -> document end.
            None if direction > 0 => (frame.doc_len(), false),
            None => (0, false),
        };

        self.goal_x = Some(x);
        if select {
            self.extend_cursor(offset, affinity, cx);
        } else {
            self.set_cursor(offset, affinity, cx);
        }
    }

    /// GTK/Windows page motion: move the caret by a viewport (minus one line
    /// of overlap), preserving goal-x.
    fn page_by(&mut self, direction: i64, select: bool, cx: &mut Context<Self>) {
        let Some(frame) = self.last_frame.as_ref() else {
            return;
        };
        let cursor = self.cursor_offset();
        let Some((par_ix, line_ix, x)) = frame.cursor_position(cursor, self.cursor_affinity_down)
        else {
            return;
        };
        let x = self.goal_x.unwrap_or(x);
        let page = (frame.bounds.size.height - frame.line_height).max(frame.line_height);
        let par = &frame.paragraphs[par_ix];
        let y = par.top + par.line_height * (line_ix as f32) + par.line_height / 2.;
        let target = point(x, y + page * (direction as f32));
        let (offset, affinity) = frame.index_for_point(target);

        self.goal_x = Some(x);
        if select {
            self.extend_cursor(offset, affinity, cx);
        } else {
            self.set_cursor(offset, affinity, cx);
        }
    }

    // -- Actions -------------------------------------------------------------

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            // The boundary line is removed only via the aside machinery:
            // backspace at manuscript start is a no-op, like backspace at 0.
            if self.at_manuscript_start(self.cursor_offset()) {
                return;
            }
            // At the start of a styled block, the first backspace strips
            // the block kind instead of merging (Notion/Docs convention).
            let cursor = self.cursor_offset();
            let (par_start, _) = self.paragraph_bounds(cursor);
            if cursor == par_start {
                let block = self.doc.block_of_byte(cursor);
                if *self.doc.blocks().kind(block) != BlockKind::Paragraph {
                    self.doc.set_block_kind(block, BlockKind::Paragraph);
                    self.mark_dirty();
                    self.bump_activity();
                    cx.notify();
                    return;
                }
            }
            let prev = self.previous_boundary(cursor);
            self.select_to(prev, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
        self.migrate_orphaned_writer_notes(window, cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            // Forward-delete at the compost tail must not merge the boundary.
            if self.at_compost_tail(self.cursor_offset())
                || self.at_separator_start(self.cursor_offset())
            {
                return;
            }
            let next = self.next_boundary(self.cursor_offset());
            self.select_to(next, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
        self.migrate_orphaned_writer_notes(window, cx);
    }

    fn delete_word_left(&mut self, _: &DeleteWordLeft, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            if self.at_manuscript_start(self.cursor_offset()) {
                return;
            }
            let prev = self.previous_word_boundary(self.cursor_offset());
            self.select_to(prev, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
        self.migrate_orphaned_writer_notes(window, cx);
    }

    fn delete_word_right(
        &mut self,
        _: &DeleteWordRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_range.is_empty() {
            if self.at_compost_tail(self.cursor_offset())
                || self.at_separator_start(self.cursor_offset())
            {
                return;
            }
            let next = self.next_word_boundary(self.cursor_offset());
            self.select_to(next, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
        self.migrate_orphaned_writer_notes(window, cx);
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx);
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.end, cx);
        }
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(hv) = &self.history_view {
            let ix = hv.selected.saturating_sub(1);
            self.history_select(ix, cx);
            return;
        }
        self.vertical_by(-1, false, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(hv) = &self.history_view {
            let ix = hv.selected + 1;
            self.history_select(ix, cx);
            return;
        }
        self.vertical_by(1, false, cx);
    }

    fn toggle_history(&mut self, _: &ToggleHistory, _: &mut Window, cx: &mut Context<Self>) {
        if self.history_view.is_some() {
            self.exit_history(cx);
        } else {
            self.enter_history(cx);
        }
    }

    /// The outline rail (DESIGN §1.6): session-only, no config.
    pub(crate) fn toggle_rail(&mut self, _: &ToggleOutline, _: &mut Window, cx: &mut Context<Self>) {
        self.rail_open = !self.rail_open;
        cx.notify();
    }

    // -- Asides: the compost rail and the graveyard -------------------------

    /// `Set aside` (docs/impl/02-asides.md §2): move the selection — or, with
    /// an empty selection, the caret's paragraph (review H25) — into the
    /// compost. A MOVE, never a cut: `file_cut` is the only path that
    /// files, so the graveyard is suppressed by construction (review H41).
    /// The verb NEVER closes the rail (the writer issued a command and must
    /// see compliance, not a vanishing panel — 06 §2): the arrival blinks
    /// both the in-document region and, when the rail is open, its new row.
    /// The caret stays at the collapse point — the writer parked a thought,
    /// she did not travel.
    fn set_aside(&mut self, _: &SetAside, _: &mut Window, cx: &mut Context<Self>) {
        if self.history_view.is_some() {
            return;
        }
        let range = if self.selected_range.is_empty() {
            let (start, end) = self.paragraph_bounds(self.cursor_offset());
            start..end
        } else {
            self.selected_range.clone()
        };
        let first_birth = self.doc.aside_boundary().is_none();
        let Some(caret_char) = self.doc.set_aside(range) else {
            return; // empty, or the range is compost already
        };
        // The compost's first birth opens the rail once (S2/N1) — the writer
        // must SEE where the passage went; a relocation that shows nothing
        // reads as destruction. Later arrivals only blink: the toggle when
        // the rail is closed, the newest row when it's open.
        if first_birth {
            self.rail_open = true;
        }
        self.rail_flash = Some(Instant::now());
        self.sync_mutations();
        let caret = self.doc.char_to_byte(caret_char.min(self.doc.rope().len_chars()));
        self.set_cursor(caret, false, cx);
        self.schedule_flash_clear(cx);
        // The asided passage may have carried a margin note / diagnosis; its
        // anchor just died. Migrate the note to the compost, close a dead
        // diagnosis (Bug C).
        self.reconcile_dead_anchors(cx);
    }

    /// `Send to the graveyard` (selection menu / palette): file the selection
    /// as a cut, any size. Only a manuscript selection is a cut — a compost
    /// selection would be a move, and the boundary clamp keeps selections
    /// single-region anyway.
    fn send_to_graveyard(&mut self, _: &SendToGraveyard, _: &mut Window, cx: &mut Context<Self>) {
        if self.history_view.is_some() || self.selected_range.is_empty() {
            return;
        }
        let start_char = self.doc.rope().byte_to_char(self.selected_range.start);
        if start_char < self.doc.manuscript_base_char() {
            return; // compost selection — not a cut
        }
        let range = self.selected_range.clone();
        self.file_cut(range, cx);
    }

    /// File `byte_range` in the graveyard and collapse the caret to the cut
    /// point. Shared by the auto-cut trigger and the explicit verb — the one
    /// site that ever files a corpse.
    fn file_cut(&mut self, byte_range: Range<usize>, cx: &mut Context<Self>) {
        let quote = self.origin_quote_before(byte_range.start);
        self.doc.cut_to_graveyard(byte_range.clone(), quote, now_unix());
        self.selected_range = byte_range.start..byte_range.start;
        self.selection_reversed = false;
        self.cursor_affinity_down = false;
        self.goal_x = None;
        self.marked_range = None;
        self.grave_flash = Some(Instant::now());
        self.sync_mutations();
        self.bump_activity();
        self.schedule_flash_clear(cx);
        cx.notify();
        // The cut passage may have carried a margin note / diagnosis; its anchor
        // just collapsed. Migrate the note to the compost, close a dead
        // diagnosis (Bug C) — the one site that ever files a corpse is also the
        // one that must tidy the anchors the corpse leaves behind.
        self.reconcile_dead_anchors(cx);
    }

    /// A trailing fragment of the paragraph immediately before `byte_pos` — the
    /// entry's origin quote (never the whole context). When the cut begins right
    /// at a block boundary (the common case — whole paragraphs are cut), step
    /// back over the boundary newline so the quote is the PRIOR paragraph's tail
    /// rather than empty (the graveyard whisper reads "cut from after …" —
    /// `GraveEntry::origin_quote`). Stops at the next line break above that.
    fn origin_quote_before(&self, byte_pos: usize) -> String {
        let rope = self.doc.rope();
        let mut at = rope.byte_to_char(byte_pos.min(self.doc.len_bytes()));
        if at > 0 && rope.char(at - 1) == '\n' {
            at -= 1; // the cut sits at a line start: name the paragraph above it
        }
        let end = at;
        let mut from = at;
        let mut taken = 0;
        while from > 0 && taken < 48 {
            if rope.char(from - 1) == '\n' {
                break;
            }
            from -= 1;
            taken += 1;
        }
        rope.slice(from..end).to_string().trim().to_owned()
    }

    /// The graveyard verb (menu / key): scroll to the tail section (Bug B — the
    /// record lives in the scroll flow now, there is nothing to "open"). Same
    /// gesture as clicking the footer bar.
    fn toggle_graveyard(&mut self, _: &ToggleGraveyard, _: &mut Window, cx: &mut Context<Self>) {
        self.scroll_to_graveyard(cx);
    }

    /// Reveal the graveyard tail section by scrolling to the document end (the
    /// section sits at the tail). The footer bar "unsticks into the section
    /// header" once it is on screen (asides.md §3).
    fn scroll_to_graveyard(&mut self, cx: &mut Context<Self>) {
        if self.doc.graveyard().is_empty() {
            return;
        }
        if let Some(frame) = self.last_frame.as_ref() {
            self.scroll_top = frame.max_scroll();
            cx.notify();
        }
    }

    /// Everything that changes the graveyard tail section's GEOMETRY beyond the
    /// document `revision` (which already covers the entries themselves): which
    /// receded entries the writer expanded. Keyed into `LayoutKey` so an expand
    /// forces a rebuild (the section grows) rather than a stale reuse.
    fn grave_layout_fingerprint(&self) -> u64 {
        let mut h = 1469598103934665603u64; // FNV-1a offset basis
        for id in &self.grave_expanded {
            h = (h ^ *id).wrapping_mul(1099511628211);
        }
        h
    }

    /// Is the graveyard tail section's header on screen? The sticky footer bar
    /// hides when it is (it "unsticks into the section header" — asides.md §3).
    /// Reads the last painted frame's recorded section top (doc-space).
    fn grave_tail_on_screen(&self) -> bool {
        self.last_frame.as_ref().is_some_and(|f| {
            f.grave_section_top
                .is_some_and(|top| top - f.scroll_top < f.bounds.size.height)
        })
    }

    /// The graveyard verb whose painted hit rect contains a window point, if any
    /// (Bug B click routing). Doc-space rects were recorded on the last frame.
    fn grave_action_at(&self, pos: Point<Pixels>) -> Option<GraveAction> {
        let frame = self.last_frame.as_ref()?;
        if frame.grave_lines.is_empty() {
            return None;
        }
        let doc = frame.doc_point(pos);
        frame
            .grave_lines
            .iter()
            .flat_map(|gl| gl.hits.iter())
            .find(|(rect, _)| rect.contains(&doc))
            .map(|(_, action)| *action)
    }

    /// Put an entry back into the manuscript (one verb everywhere — the entry
    /// action and the post-cut footer affordance). Re-anchored and clamped into
    /// the manuscript by the core; here we place the caret, flash the returned
    /// paragraph, and drop the now-stale expanded flag.
    fn put_back_entry(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(caret_char) = self.doc.put_back(id) else {
            return;
        };
        self.sync_mutations();
        let caret = self.doc.char_to_byte(caret_char.min(self.doc.rope().len_chars()));
        self.grave_expanded.retain(|e| *e != id);
        // Flash the paragraph the passage returned to (P13's visible inverse).
        self.para_flash = Some((caret_char.saturating_sub(1), Instant::now()));
        self.autoscroll_request = true;
        self.set_cursor(caret, false, cx);
        self.schedule_flash_clear(cx);
    }

    /// Move the caret to an entry's re-anchored origin (clamped into the
    /// manuscript) and flash that line — "show origin" reveals where the cut
    /// came from without re-inserting anything (the `originflash` idiom).
    fn show_grave_origin(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(e) = self.doc.graveyard().get(id) else {
            return;
        };
        let base = self.doc.manuscript_base_char();
        let at = e.origin_pos.clamp(base, self.doc.rope().len_chars());
        let byte = self.doc.char_to_byte(at);
        self.para_flash = Some((at, Instant::now()));
        self.autoscroll_request = true;
        self.set_cursor(byte, false, cx);
        self.schedule_flash_clear(cx);
    }

    /// Delete an entry (the journal still holds the record). Undoable in core.
    fn delete_grave_entry(&mut self, id: u64, cx: &mut Context<Self>) {
        self.doc.grave_delete(id);
        self.grave_expanded.retain(|e| *e != id);
        self.mark_dirty();
        self.bump_activity();
        cx.notify();
    }

    /// Toggle a receded graveyard entry between its one-line and full forms
    /// (Bug B — click expands in place; the newest entry is always full).
    fn toggle_grave_entry(&mut self, id: u64, cx: &mut Context<Self>) {
        if let Some(ix) = self.grave_expanded.iter().position(|e| *e == id) {
            self.grave_expanded.remove(ix);
        } else {
            self.grave_expanded.push(id);
        }
        cx.notify();
    }

    /// Has this WRITER note lost its anchor? Either `Annotations::reanchor`
    /// flagged it `orphaned` on a restore, or an ordinary edit collapsed the
    /// anchor to a zero-width point. Diagnoses never qualify — machine cards
    /// are not writer material and stay in the card lifecycle.
    fn note_is_doomed(&self, id: u64) -> bool {
        self.doc.notes().get(id).is_some_and(|n| {
            n.kind == NoteKind::Note
                && n.status == NoteStatus::Open
                && (n.orphaned || n.range.start == n.range.end)
        })
    }

    /// The ordinary-edit path (spec §3): after a deletion, migrate any writer
    /// note whose anchor collapsed. Has a Window, so an active doomed card is
    /// resolved-and-deselected properly first (review B5).
    fn migrate_orphaned_writer_notes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.focus.active_id().is_some_and(|id| self.note_is_doomed(id)) {
            self.deselect_card(window, cx);
        }
        self.perform_orphan_migrations(cx);
    }

    /// The restore path (spec §3): `reanchor` set `orphaned`. No Window here
    /// (history mode has no live composer), so a stray active card is simply
    /// cleared — there is no in-progress draft to commit.
    fn migrate_orphans_after_restore(&mut self, cx: &mut Context<Self>) {
        if self.focus.active_id().is_some_and(|id| self.note_is_doomed(id)) {
            self.focus = CardFocus::Idle;
        }
        self.perform_orphan_migrations(cx);
    }

    /// Move every doomed writer note's text to the compost tail. The caller has
    /// already resolved focus for any active doomed card.
    fn perform_orphan_migrations(&mut self, cx: &mut Context<Self>) {
        let doomed: Vec<u64> = self
            .doc
            .notes()
            .notes()
            .iter()
            .filter(|n| self.note_is_doomed(n.id))
            .map(|n| n.id)
            .collect();
        if doomed.is_empty() {
            return;
        }
        let first_birth = self.doc.aside_boundary().is_none();
        let mut migrated = false;
        for id in doomed {
            let anchor = self
                .doc
                .notes()
                .get(id)
                .map(|n| self.doc.char_to_byte(n.range.start.min(self.doc.rope().len_chars())))
                .unwrap_or(0);
            let quote = self.origin_quote_before(anchor);
            if self.doc.migrate_note_to_compost(id, &quote) {
                self.rail_flash = Some(Instant::now());
                migrated = true;
            }
        }
        // The compost's first birth opens the rail once (S2/N1): the writer
        // must SEE where the note went, not deduce it. Later arrivals only
        // blink — the toggle when the rail is closed, the row when open.
        if migrated && first_birth {
            self.rail_open = true;
        }
        self.sync_mutations();
        self.schedule_flash_clear(cx);
        cx.notify();
    }

    /// After a structural removal — a graveyard cut or a set-aside — reconcile
    /// the margin with the anchors that just died (Bug C). Both paths bypassed
    /// this before, so a cut annotated paragraph left the writer's own margin
    /// note floating, anchored to nothing.
    ///
    /// A WRITER note whose anchor is gone migrates to the compost tail wearing
    /// its anchor quote (spec §3, the designed provenance — the note simply
    /// changed address). A DIAGNOSIS whose anchor is gone is CLOSED instead — a
    /// machine card is never writer material and must not linger pointing at
    /// nothing: it is dismissed and the strip records its `CardClosed` terminal
    /// (the existing close idiom). Windowless, so it also serves the IME
    /// auto-cut path; it mirrors `migrate_orphans_after_restore`'s no-composer
    /// discipline (a cut is not a composer interaction — clear focus directly,
    /// commit no draft).
    fn reconcile_dead_anchors(&mut self, cx: &mut Context<Self>) {
        // Preserve the caret across any migration: an orphan-note migration
        // inserts at the compost tail, shifting every manuscript byte by the
        // same delta, so the caret's DISTANCE FROM THE DOCUMENT END is
        // invariant. The writer cut/parked a thought — she must not travel (P2).
        let len_before = self.doc.len_bytes();
        let tail = len_before.saturating_sub(self.cursor_offset().min(len_before));

        let doomed_diagnoses: Vec<u64> = self
            .doc
            .notes()
            .notes()
            .iter()
            .filter(|n| {
                n.kind == NoteKind::Diagnosis
                    && n.status == NoteStatus::Open
                    && (n.orphaned || n.range.start == n.range.end)
            })
            .map(|n| n.id)
            .collect();
        for id in &doomed_diagnoses {
            self.doc.set_note_status(*id, NoteStatus::Dismissed);
            self.doc
                .journal_mut()
                .record_event(strop_core::journal::JournalEvent::CardClosed {
                    t: strop_core::journal::now_ms(),
                    id: *id,
                    resolved: false,
                });
            if self.focus.active_id() == Some(*id) {
                self.focus = CardFocus::Idle;
            }
        }
        if !doomed_diagnoses.is_empty() {
            self.mark_dirty();
            self.bump_activity();
            cx.notify();
        }

        // Then the writer notes — their text migrates to the compost tail.
        if self.focus.active_id().is_some_and(|id| self.note_is_doomed(id)) {
            self.focus = CardFocus::Idle;
        }
        self.perform_orphan_migrations(cx);

        if self.doc.len_bytes() != len_before {
            let caret = self.doc.len_bytes().saturating_sub(tail);
            self.set_cursor(caret, false, cx);
        }
    }

    /// Clear the one-shot rail/graveyard arrival blinks after a beat.
    fn schedule_flash_clear(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(420))
                .await;
            this.update(cx, |editor: &mut Editor, cx| {
                editor.rail_flash = None;
                editor.grave_flash = None;
                editor.para_flash = None;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // -- Region geometry (byte offsets) -------------------------------------

    /// Byte bounds of the region the caret is in: the manuscript, the compost,
    /// or the whole document when there is no rail. Ctrl+A scopes to this so a
    /// select-all in prose never nukes the compost (spec §1, review #110).
    fn caret_region_bytes(&self) -> (usize, usize) {
        let len = self.doc.len_bytes();
        let Some(b) = self.doc.aside_boundary() else {
            return (0, len);
        };
        let compost_edge = self.doc.rope().line_to_byte(b);
        let manuscript_base = self.doc.char_to_byte(self.doc.manuscript_base_char());
        if self.cursor_offset() >= manuscript_base {
            (manuscript_base, len)
        } else {
            (0, compost_edge)
        }
    }

    /// Clamp a selection's moving end so it never crosses the boundary from the
    /// anchor's region (review B4): every verb's input is single-region by
    /// construction. Applies to keyboard AND drag selection (both go through
    /// `extend_cursor`).
    fn clamp_to_region(&self, anchor: usize, target: usize) -> usize {
        let Some(b) = self.doc.aside_boundary() else {
            return target;
        };
        let compost_edge = self.doc.rope().line_to_byte(b);
        let manuscript_base = self.doc.char_to_byte(self.doc.manuscript_base_char());
        if anchor >= manuscript_base {
            target.max(manuscript_base)
        } else if anchor < compost_edge {
            target.min(compost_edge)
        } else {
            target
        }
    }

    /// Is `byte` the first manuscript position (just after the boundary line)?
    /// Backspace there must be a no-op (the boundary is removed only via the
    /// aside machinery — spec §1 guard).
    fn at_manuscript_start(&self, byte: usize) -> bool {
        self.doc.aside_boundary().is_some()
            && byte == self.doc.char_to_byte(self.doc.manuscript_base_char())
    }

    /// Is `byte` the end of the last compost line (the newline before the
    /// boundary line)? Forward-delete there would merge the boundary away.
    fn at_compost_tail(&self, byte: usize) -> bool {
        match self.doc.aside_boundary() {
            Some(b) => byte == self.doc.rope().line_to_byte(b).saturating_sub(1),
            None => false,
        }
    }

    /// Caret at the START of the boundary's own empty line. Forward-delete
    /// here would eat the separator's newline and merge block b into b+1 —
    /// the boundary index stays pinned at b, so the first MANUSCRIPT
    /// paragraph silently becomes compost (excluded from export, the count,
    /// and passes) with no visible change to the text. Wave-1 review,
    /// correctness/high: the separator's newline is removable only by the
    /// aside machinery, like the other two edges.
    fn at_separator_start(&self, byte: usize) -> bool {
        match self.doc.aside_boundary() {
            Some(b) => byte == self.doc.rope().line_to_byte(b),
            None => false,
        }
    }

    /// "Set Session Goal…" (DESIGN §4.2): a number, a live delta in the
    /// titlebar, a quiet sage dot at the finish line. Session-only.
    fn set_session_goal(&mut self, _: &SetSessionGoal, window: &mut Window, cx: &mut Context<Self>) {
        if self.goal_input.is_some() {
            return;
        }
        let input = cx.new(|cx| TextField::single(cx, String::new()));
        cx.subscribe_in(
            &input,
            window,
            |editor, _, event: &TextFieldEvent, window, cx| {
                match event {
                    TextFieldEvent::Commit(text) => {
                        // A number sets, 0 clears, anything else is
                        // ignored gracefully — the strip just closes.
                        match text.trim().replace([',', ' '], "").parse::<usize>() {
                            Ok(0) => editor.session_goal = None,
                            Ok(goal) => {
                                editor.session_goal = Some((goal, editor.word_count));
                            }
                            Err(_) => {}
                        }
                    }
                    TextFieldEvent::Cancel => {}
                }
                editor.goal_input = None;
                window.focus(&editor.focus_handle, cx);
                cx.notify();
            },
        )
        .detach();
        cx.observe(&input, |_, _, cx| cx.notify()).detach();
        self.commit_field_on_blur(&input, window, cx, |e| e.goal_input.clone());
        let input_focus = input.read(cx).focus_handle.clone();
        window.focus(&input_focus, cx);
        self.goal_input = Some(input);
        cx.notify();
    }

    fn render_goal_strip(&self) -> Option<impl IntoElement> {
        let input = self.goal_input.clone()?;
        Some(
            div()
                .absolute()
                .bottom_0()
                .left_0()
                .right_0()
                .bg(rgb(0xF4F1EA))
                .border_t_1()
                .border_color(rgb(RULE_COLOR))
                .px(px(28.))
                .py(px(8.))
                .flex()
                .items_center()
                .gap(px(8.))
                .font_family("PT Serif")
                .text_size(px(13.))
                .child(div().text_color(rgb(MUTED_COLOR)).child("Session goal, words:"))
                .child(div().flex_1().child(input))
                .child(
                    div()
                        .text_color(rgb(MUTED_COLOR))
                        .text_size(px(11.))
                        .child("enter sets · 0 clears · esc cancels"),
                ),
        )
    }

    /// DESIGN §0.6 law 2: Esc dismisses exactly the TOPMOST layer, one per
    /// press, regardless of where keyboard focus sits. This is the Editor-
    /// context half; a focused field's own Esc goes through FieldCancel,
    /// which closes the same layer. Order: AI settings → palette →
    /// shortcuts → selection popover → find/replace → history takeover.
    fn escape_mode(&mut self, _: &EscapeMode, window: &mut Window, cx: &mut Context<Self>) {
        if self.ai_settings.is_some() {
            self.close_ai_settings(window, cx);
            return;
        }
        if self.palette_input.is_some() {
            // The known bug: palette open + focus on the editor + Esc
            // previously fell through to the (empty) fallback.
            self.close_palette(window, cx);
            return;
        }
        if self.shortcuts_open {
            self.shortcuts_open = false;
            window.focus(&self.focus_handle, cx);
            cx.notify();
            return;
        }
        if self.editor_menu_open {
            self.editor_menu_open = false;
            cx.notify();
            return;
        }
        // The link argument-field is the innermost flank layer (finding 59's
        // two-level Esc): its own FieldCancel handles Esc while it holds focus;
        // this covers the edge case where the editor has focus with a field open.
        if self.link_input.is_some() {
            self.cancel_link(window, cx);
            return;
        }
        if self.selection_popover {
            self.selection_popover = false;
            cx.notify();
            return;
        }
        // Esc from a compost caret returns to the last manuscript caret (review
        // B3): the rail is a hard edge to drift INTO, soft to leave.
        if let Some(b) = self.doc.aside_boundary() {
            let compost_edge = self.doc.rope().line_to_byte(b);
            if self.cursor_offset() < compost_edge {
                let target = self.last_manuscript_caret.min(self.doc.len_bytes());
                self.move_to(target, cx);
                return;
            }
        }
        // (No second palette check here: the one at the top of this fn already
        // returned — the duplicate was unreachable, extraction audit #24.)
        // The strip: parked → return to now (drop the preview); already at now →
        // close it (spec §2 / review mid — restores the app-wide Esc-closes rule).
        if self.strip.open {
            if self.strip.parked {
                self.strip_return_to_now(cx);
            } else {
                self.close_strip(cx);
            }
            return;
        }
        if self.history_view.is_some() {
            self.exit_history(cx);
        }
    }

    /// ctrl-.: summon the selection popover by keyboard (the ARIA-toolbar
    /// requirement — no capability reachable by only one modality).
    fn toggle_popover(&mut self, _: &TogglePopover, _: &mut Window, cx: &mut Context<Self>) {
        self.selection_popover = !self.selection_popover && !self.selected_range.is_empty();
        cx.notify();
    }

    fn show_shortcuts(&mut self, _: &ShowShortcuts, _: &mut Window, cx: &mut Context<Self>) {
        self.shortcuts_open = !self.shortcuts_open;
        cx.notify();
    }

    fn open_welcome(&mut self, _: &OpenWelcome, _: &mut Window, _: &mut Context<Self>) {
        crate::files::open_welcome_window();
    }

    /// The keyboard map (GNOME's ctrl-? convention): every command from
    /// the registry plus the text-editing baseline, at a glance. The
    /// palette is for doing; this is for learning.
    fn render_shortcuts(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut sections: Vec<(&'static str, Vec<(String, String)>)> = Vec::new();
        for cmd in crate::commands::all() {
            let keys = cmd.keys.map_or_else(|| "palette".to_owned(), |k| k.to_owned());
            match sections.iter_mut().find(|(s, _)| *s == cmd.section) {
                Some((_, rows)) => rows.push((cmd.label.to_owned(), keys)),
                None => sections.push((cmd.section, vec![(cmd.label.to_owned(), keys)])),
            }
        }
        sections.push((
            "Text editing",
            [
                ("Move by word / paragraph", "ctrl-arrows"),
                ("Select by word / paragraph", "ctrl-shift-arrows"),
                ("Document start / end", "ctrl-home / ctrl-end"),
                ("Select all", "ctrl-a"),
                ("Copy / Cut / Paste", "ctrl-c / x / v"),
                ("Markdown headings", "# ## ### + space"),
                ("Escape any mode", "escape"),
            ]
            .into_iter()
            .map(|(a, b)| (a.to_owned(), b.to_owned()))
            .collect(),
        ));
        div()
            .absolute()
            .inset_0()
            .bg(rgba(0x1A1A1830u32))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    editor.shortcuts_open = false;
                    // §0.6 law 4: closing a layer restores focus beneath it.
                    window.focus(&editor.focus_handle, cx);
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("shortcuts-panel")
                    .w(px(700.))
                    .max_h(px(560.))
                    .overflow_y_scroll()
                    .bg(rgb(0xFCFAF4))
                    .border_1()
                    .border_color(rgb(RULE_COLOR))
                    .rounded(px(8.))
                    .shadow_lg()
                    .p(px(18.))
                    .font_family("PT Serif")
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .pb(px(10.))
                            .flex()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(TEXT_COLOR))
                                    .child("Keyboard map"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(rgb(MUTED_COLOR))
                                    .child("esc closes · ctrl-shift-p runs any of these by name"),
                            ),
                    )
                    .child(div().flex().flex_wrap().gap(px(16.)).children(
                        sections.into_iter().map(|(section, rows)| {
                            div()
                                .w(px(320.))
                                .child(
                                    div()
                                        .pt(px(6.))
                                        .pb(px(3.))
                                        .text_size(px(10.))
                                        .text_color(rgb(MUTED_COLOR))
                                        .child(section.to_uppercase()),
                                )
                                .children(rows.into_iter().map(|(label, keys)| {
                                    div()
                                        .flex()
                                        .justify_between()
                                        .gap(px(10.))
                                        .py(px(1.))
                                        .text_size(px(12.))
                                        .child(div().text_color(rgb(TEXT_COLOR)).child(label))
                                        .child(
                                            div()
                                                .text_color(rgb(MUTED_COLOR))
                                                .text_size(px(11.))
                                                .child(keys),
                                        )
                                }))
                        }),
                    )),
            )
    }

    /// The AI settings panel (DESIGN §2-ai, F4): centered in-surface
    /// overlay like the keyboard map — backdrop, esc/click-out closes.
    /// Form + async test with inline states + pickable /models list; the
    /// config file stays the storage (Save writes through toml_edit).
    fn render_ai_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let panel = self.ai_settings.as_ref().expect("ai settings open");
        let key_from_env = env_key_set();
        let filtered = self.ai_settings_filtered(cx);
        let selected = panel.selected.min(filtered.len().saturating_sub(1));
        let has_models = !panel.models.is_empty();
        let models_note = panel.models_note.clone();

        let label = |text: &'static str| {
            div()
                .text_size(px(10.))
                .text_color(rgb(MUTED_COLOR))
                .child(text.to_uppercase())
        };
        let helper = |text: String| {
            div()
                .text_size(px(10.5))
                .text_color(rgb(MUTED_COLOR))
                .child(text)
        };
        let button = |id: &'static str, text: &'static str| {
            div()
                .id(id)
                .px(px(10.))
                .py(px(3.))
                .rounded(px(4.))
                .border_1()
                .border_color(rgb(RULE_COLOR))
                .cursor(CursorStyle::PointingHand)
                .hover(|d| d.bg(rgba(0x1A1A180Au32)))
                .text_size(px(12.))
                .child(text)
        };
        // Inline feedback, never a margin card while the panel is open.
        let status_line = match &panel.test {
            AiSettingsTest::Idle => None,
            AiSettingsTest::Running => Some(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(MUTED_COLOR))
                    .child("testing…"),
            ),
            // ✓ / ✗ glyphs carry the pass/fail distinction without relying on
            // color (WCAG 1.4.1); failure wears the reserved ERROR red.
            AiSettingsTest::Ok { ms } => Some(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(0x4F7A4A))
                    .child(format!("✓ OK · {ms}ms")),
            ),
            AiSettingsTest::Failed { message } => Some(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(ERROR))
                    .child(format!("✗ {message}")),
            ),
        };
        let mut model_list = div()
            .id("ai-models-list")
            .max_h(px(170.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .border_1()
            .border_color(rgb(RULE_COLOR))
            .rounded(px(4.));
        for (ix, m) in filtered.iter().enumerate() {
            let model_id = m.clone();
            model_list = model_list.child(
                div()
                    .id(("ai-model-row", ix))
                    .px(px(8.))
                    .py(px(3.))
                    .text_size(px(12.))
                    .cursor(CursorStyle::PointingHand)
                    .when(ix == selected, |d| d.bg(rgba(0x1A1A1812u32)))
                    .hover(|d| d.bg(rgba(0x1A1A180Au32)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |editor, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            if let Some(panel) = &editor.ai_settings {
                                let m = model_id.clone();
                                panel.model.update_checked(cx, |input, cx| {
                                    input.content = m;
                                    cx.notify();
                                });
                            }
                            cx.notify();
                        }),
                    )
                    .child(m.clone()),
            );
        }
        let focus_field = |which: usize| {
            cx.listener(move |editor: &mut Editor, _: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                if let Some(panel) = &editor.ai_settings {
                    let field = [&panel.base_url, &panel.api_key, &panel.model][which];
                    let handle = field.read(cx).focus_handle.clone();
                    window.focus(&handle, cx);
                }
            })
        };

        div()
            .absolute()
            .inset_0()
            .bg(rgba(0x1A1A1830u32))
            .flex()
            .items_center()
            .justify_center()
            .on_action(cx.listener(Self::settings_up))
            .on_action(cx.listener(Self::settings_down))
            .on_action(cx.listener(Self::save_ai_settings))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    editor.close_ai_settings(window, cx);
                }),
            )
            .child(
                div()
                    .id("ai-settings-panel")
                    .w(px(520.))
                    .max_h(px(600.))
                    .bg(rgb(0xFCFAF4))
                    .border_1()
                    .border_color(rgb(RULE_COLOR))
                    .rounded(px(8.))
                    .shadow_lg()
                    .p(px(18.))
                    .font_family("PT Serif")
                    .text_color(rgb(TEXT_COLOR))
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::BOLD)
                                    .child("AI provider"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(rgb(MUTED_COLOR))
                                    .child("esc closes · tab moves between fields"),
                            ),
                    )
                    // Provider picker (DESIGN principle 9): one click prefills
                    // the endpoint and points at where to get a key. Local
                    // first — no account, no key, fully private.
                    .child({
                        let current = provider_for(&panel.base_url.read(cx).content);
                        let mut row = div().flex().flex_wrap().gap(px(5.));
                        for p in PROVIDERS {
                            let active = match current {
                                Some(c) => std::ptr::eq(c, p),
                                None => p.host_match.is_empty(), // Custom owns the unmatched state
                            };
                            let base = p.base_url;
                            let list = p.host_match == "11434";
                            row = row.child(
                                div()
                                    .id(("ai-provider", p.label.as_ptr() as usize))
                                    .px(px(9.))
                                    .py(px(3.))
                                    .rounded(px(12.))
                                    .border_1()
                                    .border_color(rgb(RULE_COLOR))
                                    .cursor(CursorStyle::PointingHand)
                                    .text_size(px(11.5))
                                    .when(active, |d| {
                                        d.bg(rgba(0x1A1A1812u32)).text_color(rgb(TEXT_COLOR))
                                    })
                                    .when(!active, |d| d.text_color(rgb(MUTED_COLOR)))
                                    .hover(|d| d.bg(rgba(0x1A1A180Au32)))
                                    .tooltip(tip(p.note, None))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |editor, _: &MouseDownEvent, _, cx| {
                                            cx.stop_propagation();
                                            editor.ai_settings_pick_provider(base, list, cx);
                                        }),
                                    )
                                    .child(p.label),
                            );
                        }
                        row
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(3.))
                            .on_mouse_down(MouseButton::Left, focus_field(0))
                            .child(label("Base URL"))
                            .child(panel.base_url.clone())
                            .child({
                                let url = panel.base_url.read(cx).content.clone();
                                match provider_for(&url) {
                                    Some(p) => helper(p.note.into()),
                                    None => helper(
                                        "any OpenAI-compatible /chat/completions endpoint".into(),
                                    ),
                                }
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(3.))
                            .on_mouse_down(MouseButton::Left, focus_field(1))
                            .child({
                                // "Get a key →" jumps to the current provider's
                                // key page — the missing-account step, in one
                                // click, without leaving for a search engine.
                                let key_url = (!key_from_env)
                                    .then(|| provider_for(&panel.base_url.read(cx).content))
                                    .flatten()
                                    .and_then(|p| p.key_url);
                                div()
                                    .flex()
                                    .justify_between()
                                    .items_center()
                                    .child(label("API key"))
                                    .when_some(key_url, |d, url| {
                                        d.child(
                                            div()
                                                .id("ai-get-key")
                                                .text_size(px(10.5))
                                                .text_color(rgb(0x4F6F8A))
                                                .cursor(CursorStyle::PointingHand)
                                                .hover(|d| d.text_color(rgb(TEXT_COLOR)))
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    move |_: &MouseDownEvent, _, _| {
                                                        crate::files::open_external(url);
                                                    },
                                                )
                                                .child("Get a key →"),
                                        )
                                    })
                            })
                            .child(panel.api_key.clone())
                            .child(helper(if key_from_env {
                                "key comes from STROP_API_KEY; this field is ignored \
                                 and never written"
                                    .into()
                            } else {
                                "stored as plain text in config.toml — or export \
                                 STROP_API_KEY and leave this empty"
                                    .into()
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(3.))
                            .on_mouse_down(MouseButton::Left, focus_field(2))
                            .child(label("Model"))
                            .child(panel.model.clone())
                            .child(helper(
                                "free text — typing filters the provider's list below".into(),
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(button("ai-settings-test", "Test").on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                                    cx.stop_propagation();
                                    editor.ai_settings_test(cx);
                                }),
                            ))
                            .child(button("ai-settings-models", "List models").on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                                    cx.stop_propagation();
                                    editor.ai_settings_list_models(cx);
                                }),
                            ))
                            .child(
                                button(
                                    "ai-settings-save",
                                    if self.pending_pass.is_some() { "Save & run" } else { "Save" },
                                )
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|editor, _: &MouseDownEvent, window, cx| {
                                        cx.stop_propagation();
                                        editor.save_ai_settings(&SaveAiSettings, window, cx);
                                    }),
                                ),
                            ),
                    )
                    .when_some(status_line, |d, status| d.child(status))
                    .when_some(models_note, |d, note| {
                        d.child(
                            div()
                                .text_size(px(11.))
                                .text_color(rgb(MUTED_COLOR))
                                .child(note),
                        )
                    })
                    .when(has_models, |d| {
                        if filtered.is_empty() {
                            d.child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(rgb(MUTED_COLOR))
                                    .child("no models match the filter"),
                            )
                        } else {
                            d.child(model_list)
                        }
                    })
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .pt(px(2.))
                            .child(
                                div()
                                    .id("ai-settings-edit-file")
                                    .text_size(px(11.))
                                    .text_color(rgb(MUTED_COLOR))
                                    .cursor(CursorStyle::PointingHand)
                                    .hover(|d| d.text_color(rgb(TEXT_COLOR)))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|editor, _: &MouseDownEvent, window, cx| {
                                            cx.stop_propagation();
                                            editor.close_ai_settings(window, cx);
                                            editor.open_ai_config(&OpenAiConfig, window, cx);
                                        }),
                                    )
                                    .child("Edit config file…"),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(rgb(MUTED_COLOR))
                                    .child("enter tests · ctrl-enter saves"),
                            ),
                    ),
            )
    }

    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.previous_word_boundary(self.cursor_offset()), cx);
    }

    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.next_word_boundary(self.cursor_offset()), cx);
    }

    fn paragraph_up(&mut self, _: &ParagraphUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.previous_paragraph_boundary(self.cursor_offset()), cx);
    }

    fn paragraph_down(&mut self, _: &ParagraphDown, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.next_paragraph_boundary(self.cursor_offset()), cx);
    }

    fn select_paragraph_up(&mut self, _: &SelectParagraphUp, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_paragraph_boundary(self.cursor_offset()), cx);
    }

    fn select_paragraph_down(
        &mut self,
        _: &SelectParagraphDown,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(self.next_paragraph_boundary(self.cursor_offset()), cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical_by(-1, true, cx);
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical_by(1, true, cx);
    }

    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_word_boundary(self.cursor_offset()), cx);
    }

    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_word_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        // Scope to the caret's REGION (spec §1): the writer selecting-all in the
        // manuscript must not sweep up the private compost, and vice versa.
        let (start, end) = self.caret_region_bytes();
        self.move_to(start, cx);
        self.select_to(end, cx);
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        let ((start, affinity), _) = self.visual_line_bounds(self.cursor_offset());
        self.goal_x = None;
        self.set_cursor(start, affinity, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let (_, (end, affinity)) = self.visual_line_bounds(self.cursor_offset());
        self.goal_x = None;
        self.set_cursor(end, affinity, cx);
    }

    fn select_to_home(&mut self, _: &SelectToHome, _: &mut Window, cx: &mut Context<Self>) {
        let ((start, affinity), _) = self.visual_line_bounds(self.cursor_offset());
        self.goal_x = None;
        self.extend_cursor(start, affinity, cx);
    }

    fn select_to_end(&mut self, _: &SelectToEnd, _: &mut Window, cx: &mut Context<Self>) {
        let (_, (end, affinity)) = self.visual_line_bounds(self.cursor_offset());
        self.goal_x = None;
        self.extend_cursor(end, affinity, cx);
    }

    fn doc_start(&mut self, _: &DocStart, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn doc_end(&mut self, _: &DocEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.doc.len_bytes(), cx);
    }

    fn select_to_doc_start(
        &mut self,
        _: &SelectToDocStart,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_to(0, cx);
    }

    fn select_to_doc_end(&mut self, _: &SelectToDocEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.doc.len_bytes(), cx);
    }

    fn page_up(&mut self, _: &PageUp, _: &mut Window, cx: &mut Context<Self>) {
        self.page_by(-1, false, cx);
    }

    fn page_down(&mut self, _: &PageDown, _: &mut Window, cx: &mut Context<Self>) {
        self.page_by(1, false, cx);
    }

    fn select_page_up(&mut self, _: &SelectPageUp, _: &mut Window, cx: &mut Context<Self>) {
        self.page_by(-1, true, cx);
    }

    fn select_page_down(&mut self, _: &SelectPageDown, _: &mut Window, cx: &mut Context<Self>) {
        self.page_by(1, true, cx);
    }

    fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        if self.history_view.is_some() {
            self.restore_selected(cx);
            return;
        }
        // Enter at the end of a heading/divider starts a paragraph, not
        // another heading (the split otherwise inherits the block kind).
        let cursor = self.cursor_offset();
        let (_, par_end) = self.paragraph_bounds(cursor);
        let block = self.doc.block_of_byte(cursor.min(self.doc.len_bytes()));
        let demote = self.selected_range.is_empty()
            && cursor == par_end
            && matches!(
                self.doc.blocks().kind(block),
                BlockKind::Heading(_) | BlockKind::Divider
            );
        self.replace_text_in_range(None, "\n", window, cx);
        if demote {
            let new_block = self.doc.block_of_byte(self.cursor_offset());
            self.doc
                .set_block_kind_in_current_tx(new_block, BlockKind::Paragraph);
            cx.notify();
        }
    }

    /// Toggle a block kind over the selected block range, one transaction.
    fn toggle_block(&mut self, kind: BlockKind, cx: &mut Context<Self>) {
        if self.strip.is_parked() {
            self.pulse_strip(cx);
            return;
        }
        if self.history_view.is_some() {
            return;
        }
        let start_block = self.doc.block_of_byte(self.selected_range.start);
        let end_block = self.doc.block_of_byte(self.selected_range.end);
        let target = if *self.doc.blocks().kind(start_block) == kind {
            BlockKind::Paragraph
        } else {
            kind
        };
        self.doc.set_block_kind(start_block, target.clone());
        for block in start_block + 1..=end_block {
            self.doc.set_block_kind_in_current_tx(block, target.clone());
        }
        self.mark_dirty();
        self.bump_activity();
        cx.notify();
    }

    fn heading1(&mut self, _: &Heading1, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_block(BlockKind::Heading(1), cx);
    }

    fn heading2(&mut self, _: &Heading2, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_block(BlockKind::Heading(2), cx);
    }

    fn heading3(&mut self, _: &Heading3, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_block(BlockKind::Heading(3), cx);
    }

    fn toggle_quote_block(&mut self, _: &ToggleQuoteBlock, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_block(BlockKind::Blockquote, cx);
    }

    fn toggle_code_block(&mut self, _: &ToggleCodeBlock, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_block(BlockKind::CodeBlock { info: String::new() }, cx);
    }

    fn toggle_bullet_list(&mut self, _: &ToggleBulletList, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_block(BlockKind::ListItem { ordered: false, depth: 0 }, cx);
    }

    fn toggle_ordered_list(
        &mut self,
        _: &ToggleOrderedList,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_block(BlockKind::ListItem { ordered: true, depth: 0 }, cx);
    }

    fn toggle_strong(&mut self, _: &ToggleStrong, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_span(InlineAttr::Strong, cx);
    }

    fn toggle_emphasis(&mut self, _: &ToggleEmphasis, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_span(InlineAttr::Emphasis, cx);
    }

    fn toggle_underline(&mut self, _: &ToggleUnderline, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_span(InlineAttr::Underline, cx);
    }

    fn toggle_strikethrough(
        &mut self,
        _: &ToggleStrikethrough,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_span(InlineAttr::Strikethrough, cx);
    }

    fn toggle_highlight(&mut self, _: &ToggleHighlight, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_span(InlineAttr::Highlight, cx);
    }

    fn toggle_code(&mut self, _: &ToggleCode, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_span(InlineAttr::Code, cx);
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            return;
        }
        // Copy MUST work while parked (Bug B): the writer wants to lift text out
        // of a past revision. The selection's byte offsets index the laid-out
        // text — which IS the preview while previewing — so the slice comes from
        // the preview string, not the live doc (`index_for_mouse` clamps to the
        // same source). Selection highlight is suppressed in preview, but the
        // range is honest, so ctrl-c carries the past's words.
        let text = match &self.history_preview {
            Some(p) => {
                let r = self.selected_range.start.min(p.text.len())
                    ..self.selected_range.end.min(p.text.len());
                p.text.get(r).unwrap_or("").to_owned()
            }
            None => self.doc.slice_bytes(self.selected_range.clone()),
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        // A cut is a mutation — refused (pulsed) while parked, not a silent
        // copy-then-nothing (Bug B). Use Copy to lift text out of the past.
        if self.strip.is_parked() {
            self.pulse_strip(cx);
            return;
        }
        if !self.selected_range.is_empty() {
            let text = self.doc.slice_bytes(self.selected_range.clone());
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            self.replace_text_in_range(None, "", window, cx);
        }
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            // Headless-rig transport shim (see smoke::clipboard_override):
            // present so a leak test CAN catch a field paste falling
            // through to the document — never set outside smoke runs.
            if let Some(text) = crate::smoke::clipboard_override() {
                self.apply_replace(None, &text.replace("\r\n", "\n"), false, cx);
            }
            return;
        };
        for entry in item.into_entries() {
            match entry {
                ClipboardEntry::Image(image) => {
                    self.import_image_bytes(image.bytes, cx);
                    return;
                }
                ClipboardEntry::String(text) => {
                    // Pasted text is never typographed — already authored.
                    let text = text.text().replace("\r\n", "\n");
                    self.apply_replace(None, &text, false, cx);
                    return;
                }
                // New variant post-0.2.2; pasting file paths did nothing
                // before, keep it that way (file *drops* import images).
                ClipboardEntry::ExternalPaths(_) => {}
            }
        }
    }

    /// Run the §5b import pipeline off the UI thread, then insert a block.
    fn import_image_bytes(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { strop_core::images::import_image(bytes) })
                .await;
            this.update(cx, |editor: &mut Editor, cx| match result {
                Ok(imported) => editor.insert_image_block(imported, cx),
                Err(e) => eprintln!("strop: {e}"),
            })
            .ok();
        })
        .detach();
    }

    fn insert_image_block(
        &mut self,
        imported: strop_core::images::Imported,
        cx: &mut Context<Self>,
    ) {
        let Some(store) = &self.store else {
            eprintln!("strop: image pasted, but no document store to keep it in");
            return;
        };
        let src = store.put_asset(imported.bytes, imported.ext);
        self.mark_dirty();
        let cursor = self.cursor_offset().min(self.doc.len_bytes());
        let (_, par_end) = self.paragraph_bounds(cursor);
        self.doc.edit_bytes(par_end..par_end, "\n");
        let block = self.doc.block_of_byte((par_end + 1).min(self.doc.len_bytes()));
        self.doc.set_block_kind_in_current_tx(
            block,
            BlockKind::Image {
                src,
                alt: String::new(),
                caption: String::new(),
            },
        );
        let cursor = par_end + 1;
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.sync_mutations();
        self.bump_activity();
        cx.notify();
    }

    fn on_file_drop(&mut self, paths: &ExternalPaths, _: &mut Window, cx: &mut Context<Self>) {
        for path in paths.paths() {
            match std::fs::read(path) {
                Ok(bytes) => self.import_image_bytes(bytes, cx),
                Err(e) => eprintln!("strop: cannot read {}: {e}", path.display()),
            }
        }
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if self.strip.is_parked() {
            self.pulse_strip(cx);
            return;
        }
        if self.history_view.is_some() {
            return;
        }
        if let Some(cursor_char) = self.doc.undo() {
            if let Some(cursor_char) = cursor_char {
                let cursor = self.doc.char_to_byte(cursor_char);
                self.selected_range = cursor..cursor;
            }
            self.selection_reversed = false;
            self.cursor_affinity_down = false;
            self.goal_x = None;
            self.marked_range = None;
            self.caret_attrs.clear();
            self.sync_mutations();
            self.bump_activity();
            cx.notify();
        }
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if self.strip.is_parked() {
            self.pulse_strip(cx);
            return;
        }
        if self.history_view.is_some() {
            return;
        }
        if let Some(cursor_char) = self.doc.redo() {
            if let Some(cursor_char) = cursor_char {
                let cursor = self.doc.char_to_byte(cursor_char);
                self.selected_range = cursor..cursor;
            }
            self.selection_reversed = false;
            self.cursor_affinity_down = false;
            self.goal_x = None;
            self.marked_range = None;
            self.caret_attrs.clear();
            self.sync_mutations();
            self.bump_activity();
            cx.notify();
        }
    }

    // -- Scrolling ------------------------------------------------------------

    fn on_scroll_wheel(&mut self, ev: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        // §0.6 law 1, second line of defense: while a blocking overlay is
        // up, the document never scrolls — even if a wheel event slips
        // past the overlay's own stop_propagation.
        if self.palette_input.is_some() || self.ai_settings.is_some() || self.shortcuts_open {
            return;
        }
        // Scrolling is the writer looking around — a parked pass lands now
        // (attention already moved; nothing can pop "under" it mid-thought).
        self.flush_deferred_pass(cx);
        // Exit-fade ghosts are viewport-frozen snapshots: once the text
        // moves under them they'd hang mid-air, so a scroll just drops them.
        self.departing.clear();
        let Some(frame) = self.last_frame.as_ref() else {
            return;
        };
        let delta = ev.delta.pixel_delta(frame.line_height);
        let target = (self.scroll_top - delta.y).clamp(px(0.), frame.max_scroll());
        if target != self.scroll_top {
            self.scroll_top = target;
            // Anchored to stale geometry once the text moves — dismiss.
            self.selection_popover = false;
            cx.notify();
        }
    }

    /// Clicking an off-screen pill ("N above" / "N below") reveals the NEAREST
    /// hidden card in that direction. It reads the SAME `MarginLayout` the pill
    /// counted — one source of truth, so the count and what the click can reach
    /// can never diverge (the bug class that left pills dead, or scrolling to a
    /// door-suppressed non-card). Two reveals, per how the card hid: an anchor
    /// that scrolled off-screen → scroll JUST enough to bring it to the near edge
    /// (one more card into view, never a full page); a card packing pushed out
    /// while its anchor stays on-screen → SELECT it, so the packer's Pass 3
    /// forces it fully into the lane (scrolling can't help — its anchor already
    /// shows). Either way, the pill always does something.
    fn reveal_offscreen(&mut self, below: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(frame) = self.last_frame.as_ref() else {
            return;
        };
        let vp_h = f32::from(frame.bounds.size.height);
        let max_scroll = f32::from(frame.max_scroll());
        let layout = self.margin_cards(true);
        let refs = if below { layout.below } else { layout.above };
        // Nearest in the direction: smallest anchor_y for "below" (closest to the
        // bottom edge), largest for "above" (closest to the top edge).
        let target = if below {
            refs.into_iter().min_by(|a, b| a.anchor_y.total_cmp(&b.anchor_y))
        } else {
            refs.into_iter().max_by(|a, b| a.anchor_y.total_cmp(&b.anchor_y))
        };
        let Some(target) = target else { return };
        if target.anchor_culled {
            let t = px(reveal_scroll(target.anchor_y, vp_h, max_scroll, below));
            if t != self.scroll_top {
                self.scroll_top = t;
                self.selection_popover = false;
                cx.notify();
            }
        } else {
            // Anchor on-screen but packed off: select it so Pass 3 brings the
            // (now active) card fully into the lane next frame.
            self.select_card(target.id, window, cx);
            cx.notify();
        }
    }

    /// One tick of drag-edge autoscroll: while the pointer is held beyond
    /// the viewport edge, keep scrolling (speed ∝ overshoot) and extending.
    fn autoscroll_tick(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.is_selecting {
            self.autoscroll_active = false;
            return false;
        }
        let (Some(frame), Some(pos)) = (self.last_frame.as_ref(), self.drag_point) else {
            return true;
        };
        let bounds = frame.bounds;
        let overshoot = if pos.y < bounds.top() {
            pos.y - bounds.top()
        } else if pos.y > bounds.bottom() {
            pos.y - bounds.bottom()
        } else {
            return true;
        };
        let step = f32::from(overshoot).clamp(-48., 48.) * 0.4;
        self.scroll_top = (self.scroll_top + px(step)).clamp(px(0.), frame.max_scroll());
        self.drag_extend_to(pos, cx);
        cx.notify();
        true
    }

    // -- Mouse ----------------------------------------------------------------

    fn index_for_mouse(&self, position: Point<Pixels>) -> (usize, bool) {
        let Some(frame) = self.last_frame.as_ref() else {
            return (0, false);
        };
        let (ix, aff) = frame.index_for_point(frame.doc_point(position));
        // Stale-frame guard: never hand out offsets beyond the text the frame
        // laid out. While parked that text is the PREVIEW (a past revision that
        // may be longer or shorter than the live doc), so the selection clamps
        // to the preview's length — that's what Copy slices from (Bug B).
        let max = self
            .history_preview
            .as_ref()
            .map_or_else(|| self.doc.len_bytes(), |p| p.text.len());
        (ix.min(max), aff)
    }

    /// Footnote click routing (DESIGN §2-footnotes): a point on an in-text
    /// ref resolves to the start of its def's text; a point in a footnote
    /// def's marker gutter (the painted "N.") resolves to just after the
    /// in-text ref. None = not a footnote target, place the caret normally.
    fn footnote_jump_target(&self, position: Point<Pixels>) -> Option<usize> {
        let frame = self.last_frame.as_ref()?;
        let p = frame.doc_point(position);
        let par_ix = frame
            .paragraphs
            .iter()
            .position(|par| p.y >= par.top && p.y < par.top + par.height)?;
        let par = &frame.paragraphs[par_ix];
        // Stale-frame guard (compositor throttling): offsets beyond the
        // live rope never leave this function.
        if par.range.end > self.doc.len_bytes() {
            return None;
        }
        let rope = self.doc.rope();
        if p.x < par.indent {
            // The def's marker gutter: jump back to the in-text ref.
            if let Some(BlockKind::FootnoteDef { id }) = self.doc.blocks().kinds().get(par_ix) {
                let span = self
                    .doc
                    .spans()
                    .spans()
                    .iter()
                    .filter(|s| matches!(&s.attr, InlineAttr::FootnoteRef(d) if d == id))
                    .min_by_key(|s| s.range.start)?;
                return Some(rope.char_to_byte(span.range.end));
            }
            return None;
        }
        // A ref's carrier: jump forward to the def. Hit-test the span's
        // glyph band — a caret landing *next to* the ref must not teleport.
        let line_ix = (((p.y - par.top) / par.line_height) as usize).min(par.line_count() - 1);
        for s in self.doc.spans().spans() {
            let InlineAttr::FootnoteRef(id) = &s.attr else {
                continue;
            };
            let (bs, be) = (
                rope.char_to_byte(s.range.start),
                rope.char_to_byte(s.range.end),
            );
            if bs < par.range.start || be > par.range.end {
                continue;
            }
            let line = par.line_of(bs - par.range.start, true);
            if line != line_ix {
                continue;
            }
            let x0 = par.x_for(bs - par.range.start, line);
            let x1 = par.x_for(be - par.range.start, line);
            if p.x < x0 || p.x >= x1 {
                continue;
            }
            let def = self
                .doc
                .blocks()
                .kinds()
                .iter()
                .position(|k| matches!(k, BlockKind::FootnoteDef { id: d } if d == id))?;
            // The def line's start — the painted "N." lives in the gutter,
            // so this IS "after the marker".
            return Some(rope.line_to_byte(def));
        }
        None
    }

    /// Extend the drag selection toward a window point, clamped to the
    /// viewport so dragging past an edge selects to the edge (the autoscroll
    /// tick brings the rest into reach).
    fn drag_extend_to(&mut self, pos: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(frame) = self.last_frame.as_ref() else {
            return;
        };
        let bounds = frame.bounds;
        let clamped = point(
            pos.x,
            pos.y.clamp(bounds.top(), bounds.bottom() - px(1.)),
        );
        // Use the *current* scroll offset, which the autoscroll tick may have
        // moved past the painted frame's.
        let doc =
            clamped - bounds.origin + point(px(0.), self.scroll_top);
        let (ix, affinity) = frame.index_for_point(doc);
        match self.select_granularity {
            SelectGranularity::Char => self.extend_cursor(ix, affinity, cx),
            _ => self.extend_by_unit(ix, cx),
        }
    }

    /// The selection unit at `offset` for the current drag granularity.
    fn unit_at(&self, offset: usize) -> Range<usize> {
        match self.select_granularity {
            SelectGranularity::Char => offset..offset,
            SelectGranularity::Word => self.word_range_at(offset),
            SelectGranularity::Paragraph => {
                let (start, end) = self.paragraph_bounds(offset);
                start..end
            }
        }
    }

    /// Union the unit under the pointer with the origin unit (GTK/Word
    /// drag-by-word/paragraph behavior).
    fn extend_by_unit(&mut self, offset: usize, cx: &mut Context<Self>) {
        let Some(origin) = self.selection_origin.clone() else {
            return;
        };
        let unit = self.unit_at(offset);
        self.selection_reversed = unit.start < origin.start;
        self.selected_range = origin.start.min(unit.start)..origin.end.max(unit.end);
        self.cursor_affinity_down = false;
        self.bump_activity();
        self.publish_primary(cx);
        cx.notify();
    }

    /// DESIGN §0.6 law 3: any mouse-down outside a light-dismiss layer
    /// (palette, shortcuts, selection popover) closes it. Registered on
    /// the window root — the layers themselves stop propagation, so only
    /// genuinely-outside clicks arrive here.
    fn light_dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette_input.is_some() {
            self.close_palette(window, cx);
        }
        if self.shortcuts_open {
            self.shortcuts_open = false;
            window.focus(&self.focus_handle, cx);
            cx.notify();
        }
        // A click outside the flank commits its open link argument-field before
        // the popover itself is dismissed, so the URL lands (spec §0.1).
        if let Some(url) = self.link_input.as_ref().map(|(_, f)| f.read(cx).content.clone()) {
            self.commit_link(url, window, cx);
        }
        if self.selection_popover {
            self.selection_popover = false;
            cx.notify();
        }
        if self.narrow_notes_open {
            self.narrow_notes_open = false;
            cx.notify();
        }
        if self.editor_menu_open {
            self.editor_menu_open = false;
            cx.notify();
        }
    }

    fn on_mouse_down(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // A live link argument-field commits like every other single-line field
        // when the writer clicks away into the document (spec §0.1). Done up
        // front so even the early-return paths below (lane click, footnote jump)
        // don't strand it; the flank's own chrome stops propagation, so a click
        // that reaches here is genuinely outside it.
        if let Some(url) = self.link_input.as_ref().map(|(_, f)| f.read(cx).content.clone()) {
            self.commit_link(url, window, cx);
        }
        // A click in the right note lane is NOT a document click — the lane's
        // own cards and pills handle it. The lane is a sibling element painted
        // over this (full-width) column, so a card/pill's stop_propagation
        // doesn't reach this handler; without this guard, clicking any margin
        // card or off-screen pill also jumped the text caret.
        if f32::from(ev.position.x) > self.column_right(window) + MARGIN_GAP {
            return;
        }
        // A click in the graveyard tail section (Bug B). The section is painted
        // inside this element, so its verbs' hit rects (doc-space) are tested
        // here. It is a read-only RECORD: a click anywhere in it dispatches a
        // verb if one is hit and is otherwise swallowed — never a manuscript
        // caret (the normal logic would snap it to the last paragraph).
        if ev.click_count == 1
            && self.history_view.is_none()
            && self
                .last_frame
                .as_ref()
                .and_then(|f| f.grave_section_top.map(|t| f.doc_point(ev.position).y >= t - px(22.)))
                .unwrap_or(false)
        {
            cx.stop_propagation();
            if let Some(action) = self.grave_action_at(ev.position) {
                match action {
                    GraveAction::ShowOrigin(id) => self.show_grave_origin(id, cx),
                    GraveAction::PutBack(id) => self.put_back_entry(id, cx),
                    GraveAction::Delete(id) => self.delete_grave_entry(id, cx),
                    GraveAction::Expand(id) => self.toggle_grave_entry(id, cx),
                }
            }
            return;
        }
        // Footnote jumps (DESIGN §2-footnotes): a plain click on a ref's
        // mark goes to its def; a click on a def's "N." gutter goes back
        // to the ref. Never starts a drag selection.
        if ev.click_count == 1
            && !ev.modifiers.shift
            && self.history_view.is_none()
            && let Some(target) = self.footnote_jump_target(ev.position)
        {
            self.goal_x = None;
            self.selection_popover = false;
            self.set_cursor(target, false, cx);
            return;
        }
        self.goal_x = None;
        self.is_selecting = true;
        self.selection_popover = false;
        self.drag_point = Some(ev.position);
        if !self.autoscroll_active {
            self.autoscroll_active = true;
            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;
                    let go_on = this
                        .update(cx, |editor: &mut Editor, cx| editor.autoscroll_tick(cx))
                        .unwrap_or(false);
                    if !go_on {
                        break;
                    }
                }
            })
            .detach();
        }
        let (ix, affinity) = self.index_for_mouse(ev.position);
        match ev.click_count {
            1 => {
                self.select_granularity = SelectGranularity::Char;
                if ev.modifiers.shift {
                    self.selection_origin = None;
                    self.extend_cursor(ix, affinity, cx);
                } else {
                    self.selection_origin = Some(ix..ix);
                    self.set_cursor(ix, affinity, cx);
                }
                // Bidirectional activation: clicking inside an anchor activates
                // its margin card. The click snaps to the nearest caret boundary,
                // so a click on the trailing half of an anchor's last glyph lands
                // at `c == end` — still on the visible mark; `note_at_char`
                // accepts that trailing boundary (without double-claiming when
                // another anchor starts there), so the whole highlight is live.
                let c = self.doc.rope().byte_to_char(ix.min(self.doc.len_bytes()));
                let ranges: Vec<(u64, usize, usize)> = self
                    .doc
                    .notes()
                    .open()
                    .map(|n| (n.id, n.range.start, n.range.end))
                    .collect();
                let hit_id = note_at_char(&ranges, c);
                let hit = hit_id.and_then(|id| self.doc.notes().get(id));
                // Reaching for a resting diagnosis opens the door (DESIGN
                // §4.4), so the card it activates is actually on screen —
                // and an attention shift this explicit lands a parked pass.
                if self.drafting && hit.is_some_and(|n| n.kind == NoteKind::Diagnosis) {
                    self.flush_deferred_pass(cx);
                    self.drafting = false;
                }
                // Clicking the text selects the hit card (or clears selection);
                // either way it commits and closes any composer first, so a
                // click into the document never strands an open note editor.
                match hit_id {
                    Some(id) => self.select_card(id, window, cx),
                    None => self.deselect_card(window, cx),
                }
            }
            2 => {
                // Double-click on an image block edits its alt text.
                let block = self.doc.block_of_byte(ix.min(self.doc.len_bytes()));
                if matches!(self.doc.blocks().kind(block), BlockKind::Image { .. }) {
                    self.edit_image_alt(block, window, cx);
                    return;
                }
                self.select_granularity = SelectGranularity::Word;
                self.selection_origin = Some(self.word_range_at(ix));
                self.extend_by_unit(ix, cx);
            }
            _ => {
                self.select_granularity = SelectGranularity::Paragraph;
                let (start, end) = self.paragraph_bounds(ix);
                self.selection_origin = Some(start..end);
                self.extend_by_unit(ix, cx);
            }
        }
    }

    fn on_middle_click(&mut self, ev: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        // freedesktop PRIMARY contract: middle button pastes the primary
        // selection (never the clipboard) at the click position. PRIMARY is
        // X11/Wayland-only (gpui exposes read_from_primary on Linux/BSD); on
        // macOS/Windows middle-click carries no paste, so this is a no-op.
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        {
            let Some(text) = cx.read_from_primary().and_then(|item| item.text()) else {
                return;
            };
            let (ix, _) = self.index_for_mouse(ev.position);
            self.selected_range = ix..ix;
            self.selection_reversed = false;
            self.apply_replace(None, &text.replace("\r\n", "\n"), false, cx);
        }
        #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
        let _ = (ev, cx);
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        // Medium lineage: the popover appears when the button releases over
        // a live selection — never mid-drag.
        if self.is_selecting && !self.selected_range.is_empty() {
            self.selection_popover = true;
            cx.notify();
        }
        self.is_selecting = false;
        self.drag_point = None;
    }

    fn on_mouse_move(&mut self, ev: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.is_selecting {
            return;
        }
        self.drag_point = Some(ev.position);
        self.drag_extend_to(ev.position, cx);
    }

    /// Footnote click targets for the smoke harness, in window coordinates:
    /// every in-text ref mark, every def-marker gutter, every zone row's
    /// text origin (marker sits 24px left of it).
    pub fn debug_footnotes(&self) -> String {
        let Some(frame) = self.last_frame.as_ref() else {
            return "no-frame".into();
        };
        let rope = self.doc.rope();
        let origin = frame.bounds.origin;
        let mut out = String::new();
        for s in self.doc.spans().spans() {
            let InlineAttr::FootnoteRef(id) = &s.attr else {
                continue;
            };
            let (bs, be) = (
                rope.char_to_byte(s.range.start),
                rope.char_to_byte(s.range.end),
            );
            let Some(par) = frame
                .paragraphs
                .iter()
                .find(|p| p.range.start <= bs && be <= p.range.end)
            else {
                continue;
            };
            let line = par.line_of(bs - par.range.start, true);
            let x0 = par.x_for(bs - par.range.start, line);
            let x1 = par.x_for(be - par.range.start, line);
            let y = par.top + par.line_height * (line as f32) + par.line_height / 2.
                - frame.scroll_top;
            out += &format!(
                "ref {id} @{:.0},{:.0}\n",
                f32::from(origin.x + (x0 + x1) / 2.),
                f32::from(origin.y + y)
            );
        }
        for (ix, kind) in self.doc.blocks().kinds().iter().enumerate() {
            let BlockKind::FootnoteDef { id } = kind else {
                continue;
            };
            let Some(par) = frame.paragraphs.get(ix) else {
                continue;
            };
            let y = par.top + par.line_height / 2. - frame.scroll_top;
            out += &format!(
                "def {id} @{:.0},{:.0}\n",
                f32::from(origin.x + par.indent / 2.),
                f32::from(origin.y + y)
            );
        }
        let bounds_map = self.zone_row_bounds.borrow();
        let mut rows: Vec<_> = bounds_map.iter().collect();
        rows.sort_by_key(|(ix, _)| **ix);
        for (ix, b) in rows {
            out += &format!(
                "zone {ix} @{:.0},{:.0} w={:.0}\n",
                f32::from(b.origin.x),
                f32::from(b.center().y),
                f32::from(b.size.width)
            );
        }
        out
    }

    /// One-line JSON snapshot of the layer stack for the smoke harness
    /// (H1, `dump:ui`). Overlays list topmost first — the §0.6 Esc order.
    pub fn debug_ui_dump(&self, window: &Window, cx: &App) -> String {
        let mut overlays: Vec<&str> = Vec::new();
        if self.ai_settings.is_some() {
            overlays.push("ai_settings");
        }
        if self.palette_input.is_some() {
            overlays.push("palette");
        }
        if self.shortcuts_open {
            overlays.push("shortcuts");
        }
        if self.selection_popover {
            overlays.push("popover");
        }
        if self.replace_input.is_some() {
            overlays.push("replace");
        }
        if self.history_view.is_some() {
            overlays.push("history");
        }
        if self.strip.open {
            overlays.push("strip");
        }
        // Every live single-line field, so "focused" can name its context.
        let mut fields: Vec<Entity<TextField>> = Vec::new();
        if let Some(panel) = &self.ai_settings {
            fields.extend([
                panel.base_url.clone(),
                panel.api_key.clone(),
                panel.model.clone(),
            ]);
        }
        fields.extend(self.palette_input.clone());
        fields.extend(self.replace_input.clone());
        fields.extend(self.doc_rename_input.clone());
        fields.extend(self.focus.input().cloned());
        fields.extend(self.rename_input.as_ref().map(|(_, i)| i.clone()));
        fields.extend(self.alt_input.as_ref().map(|(_, i)| i.clone()));
        fields.extend(self.link_input.as_ref().map(|(_, i)| i.clone()));
        fields.extend(self.goal_input.clone());
        let focused_field = fields
            .iter()
            .find(|f| f.read(cx).focus_handle.is_focused(window));
        let focused = focused_field.map_or("Editor", |f| f.read(cx).debug_caret().0);
        let focused_input_text = focused_field.map(|f| f.read(cx).content.clone());
        // The focused field's caret + selection as CHAR indices, so the smoke
        // rig can assert mouse/keyboard selection behavior (not just eyeball it).
        let (field_cursor, field_sel) = focused_field
            .map(|f| {
                let (_, cursor, sel) = f.read(cx).debug_caret();
                (Some(cursor), Some(sel))
            })
            .unwrap_or((None, None));
        let doc_hash = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            self.doc.text().hash(&mut hasher);
            format!("{:016x}", hasher.finish())
        };
        // Margin layout, so the rig can assert the invariants the proptests cover
        // only in the pure layer — at the integration level, against a real frame:
        // no two VISIBLE cards overlap (the never-overlap rule); a selected card
        // is actually in the visible set (the displacement fix); and the pill
        // counts are what the lane reports. `null` when there is no frame yet.
        let margin = self.last_frame.as_ref().map(|_| {
            let layout = self.margin_cards(true);
            let mut spans: Vec<(f32, f32)> =
                layout.cards.iter().map(|c| (c.top, c.height)).collect();
            spans.sort_by(|a, b| a.0.total_cmp(&b.0));
            let overlap = spans.windows(2).any(|w| w[0].0 + w[0].1 > w[1].0 + 0.5);
            serde_json::json!({
                "visible": layout.cards.len(),
                "above": layout.above.len(),
                "below": layout.below.len(),
                "overlap": overlap,
                "has_active": self.focus.active_id().is_some(),
                "active_visible": layout.cards.iter().any(|c| c.active),
                // Receded one-line cards (over the full-size budget) — visible
                // counts them too: they are IN the lane, just smaller.
                "collapsed": layout.cards.iter().filter(|c| c.collapsed).count(),
            })
        });
        serde_json::json!({
            "overlays": overlays,
            "focused": focused,
            "scroll_y": f32::from(self.scroll_top),
            "doc_chars": self.doc.rope().len_chars(),
            "doc_hash": doc_hash,
            "focused_input_text": focused_input_text,
            "field_cursor": field_cursor,
            "field_sel": field_sel,
            "margin": margin,
            // The omnibox results list (palette-scroll): `selected` is the
            // highlighted row, `rows` the list length, `scroll_y`/`max_scroll_y`
            // the list's own scroll (not the document's) — the rig asserts
            // keyboard nav past the fold actually moves this, and that mouse
            // wheel still does too. `null` when the omnibox is closed.
            "omni": self.palette_input.as_ref().map(|_| serde_json::json!({
                "selected": self.palette_selected,
                "rows": self.omni_rows(&self.palette_query).len(),
                "scroll_y": f32::from(self.omni_scroll.offset().y),
                "max_scroll_y": f32::from(self.omni_scroll.max_offset().y),
            })),
            // A completed pass parked behind the reveal clock (mid-burst
            // arrival) — the rig asserts park-then-land timing against this.
            "ai_deferred": self.deferred_pass.is_some(),
            // Cards still inside their entrance fade — the rig asserts the
            // fade lifecycle (marked at landing, cleared right after).
            "appearing": self.appearing.len(),
            // Exit-fade ghosts of just-resolved cards (same lifecycle checks).
            "departing": self.departing.len(),
            // Re-pack slides in flight — the rig asserts a resolve in a
            // crowded lane MOVES the survivors (then settles), and that a
            // scroll clears all motion instantly. `moves_started` is the
            // session-monotonic total, immune to the dump racing the 200ms
            // flight on a cold launch.
            "moving": self.moving.len(),
            "moves_started": self.moves_started,
            "reduce_motion": self.config.reduce_motion,
            // The editor button's face + the state it's a priority over, so the
            // rig can assert the transitions (seed:deliver → cooking → ready →
            // reading) and the door law (open menu while drafting rests cards).
            "editor_btn": {
                "face": self.editor_face().token(),
                "open": !self.drafting,
                "cooking": matches!(self.ai_status, Some(AiStatus::Running { .. })),
                "ready": self.deferred_pass.is_some(),
                "open_count": self.open_query_count(),
            },
            // The history strip (P1). `bakes` is session-monotonic — the
            // stability-law assertion: scrubbing must NEVER bump it (only open
            // and Restore do). Everything else is scrub state the rig checks.
            "strip": self.strip.bake.as_ref().map(|b| serde_json::json!({
                "open": self.strip.open,
                "pos_ms": self.strip.pos_ms,
                "parked": self.strip.parked,
                "runs": self.doc.journal().runs.len(),
                "events": self.doc.journal().events.len(),
                "stations": b.stations.len(),
                "words_at": self.strip.words_at,
                "bakes": self.strip.bakes,
                // Axis width in working px — non-zero even for a LEGACY era
                // (checkpoints only), the merged-axis assertion (Bug A).
                "work": b.timeline.total_work,
                // The parked banner is up (mode indicator) and, when a refused
                // edit is mid-flash, the pulse is live — both MODEL bits (Bug B).
                "banner": self.strip.parked,
                "pulse": self.strip_pulse.is_some(),
            })),
            // Presentation gate: the margin lane + rail render only when no
            // history surface is previewing (review H36) — the model above is
            // ungated, so the rig asserts THIS bit for hide/show.
            "margin_hidden": self.history_view.is_some() || self.strip.is_parked(),
            // Asides (docs/impl/02-asides.md §6): the rail's compost block
            // count (0 = no rail), the graveyard entry count, and the
            // MANUSCRIPT-only word count (compost excluded — asides.md §1).
            "compost_blocks": self.doc.aside_boundary().unwrap_or(0),
            "grave_entries": self.doc.graveyard().len(),
            // Arrival/exile blinks (one-shot), and the open-card census so the
            // rig can watch a dangling note migrate (writer notes drop, the
            // compost gains an item) or a dead diagnosis close (Bug C).
            "compost_flash": self.rail_flash.is_some(),
            "grave_flash": self.grave_flash.is_some(),
            "open_notes": self.doc.notes().open().filter(|n| n.kind == NoteKind::Note).count(),
            "open_diagnoses": self.doc.notes().open().filter(|n| n.kind == NoteKind::Diagnosis).count(),
            // Presentation gate: the sticky footer bar "unsticks into the section
            // header" once the tail section is on screen (asides.md §3) — the rig
            // asserts THIS bit for the bar's hide/show, like `margin_hidden`.
            "grave_bar_hidden": self.grave_tail_on_screen(),
            "manuscript_words": self.manuscript_word_count(),
            "rail": self.rail_open,
            "sel": [self.selected_range.start, self.selected_range.end],
            // The selection flanks (docs/impl/03-flanks.md §3): presence of each
            // (`left` false only under a history surface; `right` false for a
            // compost-rail selection or a narrow lane) and the lane `y` the right
            // menu pins to. `null` when there's no live selection to flank.
            "flanks": self.flank_layout(window).map(|l| serde_json::json!({
                "left": !matches!(l.gate.left, FlankLeft::None),
                "right": l.gate.right,
                "link": self.link_input.is_some(),
                "y": l.right_top,
            })),
        })
        .to_string()
    }

    /// Cursor geometry for the smoke harness: byte offset, paragraph index,
    /// wrapped-line index within the paragraph, and x position.
    /// Rig-only: inject a dense cluster of demo diagnosis cards so the visual
    /// rig can exercise the margin lane (overlap packing, z-order, active-pin)
    /// without a live AI pass. Quotes are written to match the seed fixture's
    /// text; the middle card is activated. Driven by the `seed:diag` smoke
    /// token — never reached outside a STROP_SMOKE run.
    pub fn debug_seed_notes(&mut self, cx: &mut Context<Self>) {
        use strop_core::diagnose::to_annotations;
        let anns =
            to_annotations(&self.doc.text(), Self::demo_diagnoses(), self.doc.notes(), 0, 0);
        self.doc.add_diagnoses(anns);
        self.drafting = false; // reviewing: the editor's cards are shown
        if let Some(n) = self.doc.notes().open().nth(2) {
            self.focus = CardFocus::Selected(n.id);
        }
        self.mark_dirty();
        cx.notify();
    }

    /// The four demo diagnoses the rig seeds deliver (anchored to the
    /// tutorial/fixture quotes) — shared so every seed path flags the same
    /// passages.
    fn demo_diagnoses() -> Vec<strop_core::diagnose::Diagnosis> {
        use strop_core::diagnose::Diagnosis;
        [
            ("sold his shadow", "buried lede",
             "The strongest image of the piece opens it — do you want it spent in the first clause, or held?"),
            ("quiet thing", "ambiguous shorthand",
             "'quiet' reads as calm here, not silent — are you sure the reader lands where you mean?"),
            ("dogs had begun to growl", "vague mechanism",
             "the four 'There was' sentences are doing the work of the 'but also' — do you want the reader to feel that strain, or is it leaking?"),
            ("children, who notice everything", "telling not showing",
             "'not only' promises a 'but also' the list never grammatically completes — is the incompletion intentional?"),
        ]
        .into_iter()
        .map(|(q, p, query)| Diagnosis {
            quote: q.into(),
            problem: p.into(),
            query: query.into(),
            level: "line".into(),
        })
        .collect()
    }

    /// Rig hook (`resolve:first`): resolve the first open note through the
    /// real `set_note_status` path — instant model commit, exit-fade ghost —
    /// without depending on the done-button's pixel position (the button's
    /// hit-test is ordinary gpui listener machinery the click checks already
    /// cover; the class under test here is the ghost lifecycle).
    pub fn debug_resolve_first(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let first = self.doc.notes().open().next().map(|n| n.id);
        if let Some(id) = first {
            self.set_note_status(id, NoteStatus::Done, window, cx);
        }
    }

    /// Rig hook (`resolve:last`): resolve the NEWEST open note. In a crowded
    /// seeded lane that is the bottom full-size card — resolving it frees a
    /// budget slot, a receded card expands, and the run below shifts: the
    /// deterministic re-pack the motion checks need. (resolve:first hits the
    /// oldest card, already receded to a one-liner at its own anchor — its
    /// departure legitimately moves nothing.)
    pub fn debug_resolve_last(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let last = self.doc.notes().open().last().map(|n| n.id);
        if let Some(id) = last {
            self.set_note_status(id, NoteStatus::Done, window, cx);
        }
    }

    /// Rig hook (`seed:aside`): a fixture with BOTH piles — a compost rail
    /// (one asided item) and a graveyard (one filed cut). Replaces the doc so
    /// the geometry is deterministic, then drives the real verbs.
    pub fn debug_seed_aside(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let len = self.doc.len_bytes();
        self.doc.edit_bytes(
            0..len,
            "A compost note to keep for later.\nThe manuscript opens here with its real first line.\nAnd a second manuscript sentence long enough — comfortably over the eighty-character graveyard cut threshold — to file automatically.",
        );
        self.sync_mutations();
        self.word_count = self.manuscript_word_count();
        // Aside the first paragraph → births the rail.
        let end0 = self.doc.rope().line_to_byte(1).saturating_sub(1);
        self.selected_range = 0..end0;
        self.set_aside(&SetAside, window, cx);
        // Send the last paragraph (well over the threshold) → a graveyard entry.
        let lines = self.doc.rope().len_lines();
        let start = self.doc.rope().line_to_byte(lines.saturating_sub(1));
        let end = self.doc.len_bytes();
        self.selected_range = start..end;
        self.send_to_graveyard(&SendToGraveyard, window, cx);
        cx.notify();
    }

    /// Rig hook (`seed:demo`): a rich asides fixture for the visual rig — three
    /// compost items (so the separators + tail mark show), the sidebar open (its
    /// Compost section), and two graveyard entries: an older short one (receded)
    /// and a newest multi-paragraph one (rendered full). Exercises Bugs A & B in
    /// one frame.
    pub fn debug_seed_demo(&mut self, cx: &mut Context<Self>) {
        let len = self.doc.len_bytes();
        // Blank lines separate the three compost items; the boundary is the
        // separator before the manuscript (block 5). Building it directly keeps
        // the fixture deterministic (repeated asides of the first paragraph would
        // strand blank lines).
        self.doc.edit_bytes(
            0..len,
            "Premise B (dead): generation ship, mutiny in cold storage.\n\nENDING \u{2014} she stays; the ferry leaves without her, and the light on the water is the last line.\n\n\u{201C}Salt on the railing like the river had been chewing it.\u{201D}\n\nMara took the night crossing because the day boats were full of people who still believed the far shore existed.\nThe stowaway appeared on the third night, casting two shadows in the single running light.\nThe dockmaster's daughter kept a ledger of everything the river had taken.\nThe customs officer had a theory about the fog, and he told it to anyone who would listen: that it was the river forgetting its banks, one memory at a time.\n\nHe had charts. He had dates. Nobody would ever listen to a word of it.",
        );
        self.doc.set_aside_boundary(Some(5)); // blocks 0..5 = the compost rail
        self.sync_mutations();
        // File two cuts: an older short one (recedes once a newer arrives), then
        // the newest MULTI-paragraph one (renders full, no character cap).
        self.debug_cut_substring("The dockmaster's daughter kept a ledger of everything the river had taken.", cx);
        self.debug_cut_substring(
            "The customs officer had a theory about the fog, and he told it to anyone who would listen: that it was the river forgetting its banks, one memory at a time.\n\nHe had charts. He had dates. Nobody would ever listen to a word of it.",
            cx,
        );
        self.rail_open = true; // reveal the sidebar's Compost section
        self.word_count = self.manuscript_word_count();
        cx.notify();
    }

    /// Cut the first occurrence of `needle` into the graveyard (rig helper).
    fn debug_cut_substring(&mut self, needle: &str, cx: &mut Context<Self>) {
        let text = self.doc.text();
        if let Some(byte) = text.find(needle) {
            self.file_cut(byte..byte + needle.len(), cx);
        }
    }

    /// Rig hook (`seed:annotated`): a manuscript whose SECOND paragraph carries
    /// BOTH a writer note and a machine diagnosis, anchored inside it, and is
    /// selected — ready for `exile:selection`. After the cut the writer note
    /// must migrate to the compost (open_notes drops, compost_blocks rises) and
    /// the diagnosis must close (open_diagnoses drops) — Bug C, the dangling
    /// note the user hit on a real document.
    pub fn debug_seed_annotated(&mut self, cx: &mut Context<Self>) {
        let len = self.doc.len_bytes();
        self.doc.edit_bytes(
            0..len,
            "Keep this opening paragraph as the manuscript's real first line.\nThis whole annotated paragraph runs comfortably past the eighty-character graveyard threshold and carries the writer's own margin note plus a machine diagnosis, each anchored inside it.",
        );
        self.sync_mutations();
        let para_start = self.doc.rope().line_to_byte(1);
        let s = self.doc.rope().byte_to_char(para_start);
        // The writer's own note (migrates to the compost when its anchor dies).
        self.doc
            .add_note(s..s + 4, "does this paragraph earn its keep?".into(), now_unix());
        // A machine diagnosis on a different slice of the same paragraph (closes
        // — a machine card never lingers pointing at nothing).
        self.doc.add_diagnoses(vec![Annotation {
            id: 0,
            range: (s + 6)..(s + 11),
            body: "flabby line".into(),
            status: NoteStatus::Open,
            created_unix: now_unix(),
            kind: NoteKind::Diagnosis,
            title: "flabby".into(),
            level: "line".into(),
            orphaned: false,
            pass_id: 1,
            unverified: false,
        }]);
        // Select the annotated paragraph, ready for exile.
        self.selected_range = para_start..self.doc.len_bytes();
        self.selection_reversed = false;
        self.word_count = self.manuscript_word_count();
        self.mark_dirty();
        cx.notify();
    }

    /// Rig hook (`select:para`, docs/impl/03-flanks.md §3): select the caret's
    /// paragraph deterministically AND raise the popover — programmatic selection
    /// alone never sets `selection_popover` (only mouse-up does), so without this
    /// the flanks would never render for the dump to observe (finding 117).
    pub fn debug_select_para(&mut self, cx: &mut Context<Self>) {
        let (start, end) = self.paragraph_bounds(self.cursor_offset());
        if start >= end {
            return;
        }
        self.selected_range = start..end;
        self.selection_reversed = false;
        self.is_selecting = false;
        self.selection_popover = true;
        cx.notify();
    }

    /// Rig hook (`aside:selection`): run `Set aside` on the current selection.
    pub fn debug_aside_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_aside(&SetAside, window, cx);
        cx.notify();
    }

    /// Rig hook (`exile:selection`): run `Send to the graveyard` on the
    /// current selection (any size — the explicit verb).
    pub fn debug_exile_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.send_to_graveyard(&SendToGraveyard, window, cx);
        cx.notify();
    }

    /// Rig hook (`putback:last`): put the newest graveyard entry back.
    pub fn debug_putback_last(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.doc.graveyard().entries().last().map(|e| e.id) {
            self.put_back_entry(id, cx);
        }
        cx.notify();
    }

    /// Rig hook (`seed:deliver`): push the demo pass through the REAL arrival
    /// gate (`deliver_pass`) — mid-typing-burst it parks behind the reveal
    /// clock, in a lull it lands at once. rig-check.sh types a key first to
    /// open a burst, asserts nothing surfaced, waits out TYPING_LULL, and
    /// asserts the cards landed — the whole clock against a real window.
    pub fn debug_deliver_pass(&mut self, cx: &mut Context<Self>) {
        self.drafting = false; // a real pass opens the door at request time
        let generation = self.ai_generation;
        self.deliver_pass(Self::demo_diagnoses(), generation, cx);
        cx.notify();
    }

    /// Rig hook (`ebtn:open`): open the editor button's dropdown. The door law
    /// then holds even with the menu up — cards rest while drafting.
    pub fn debug_open_editor_menu(&mut self, cx: &mut Context<Self>) {
        self.editor_menu_open = true;
        cx.notify();
    }

    /// Rig hook (`ebtn:door`): the menu footer's presence verb — flush a parked
    /// pass and flip the door, exactly the path a click on it takes.
    pub fn debug_toggle_door(&mut self, cx: &mut Context<Self>) {
        self.toggle_door(cx);
    }

    /// Rig hook (`seed:journal`): install a deterministic synthetic fortnight —
    /// six sittings of ~100 minutes' WORKING time across two weeks (drafting
    /// early, reworking late, one big mid-arc cut), each closed by a pass —
    /// so the strip has a screen-filling fabric to bake and scrub without a
    /// live session. The first fixture's sittings were 20 SECONDS long and
    /// the fixed quant honestly rendered the whole fortnight one sliver wide.
    pub fn debug_seed_journal(&mut self, cx: &mut Context<Self>) {
        use strop_core::journal::{EditRun, Journal, JournalEvent};
        let now = strop_core::journal::now_ms();
        let day = 86_400_000i64;
        let mut runs: Vec<EditRun> = Vec::new();
        let mut events: Vec<JournalEvent> = Vec::new();
        let mut len = 0usize;
        for (s, days_ago) in [13i64, 11, 8, 5, 2, 0].into_iter().enumerate() {
            let start = now - days_ago * day - 4 * 3_600_000;
            let drafting = s < 3; // early sittings grow; late ones rework
            for k in 0..150i64 {
                let t0 = start + k * 40_000; // a run every ~40s of work
                if !drafting && k % 5 == 2 {
                    // Rework: cut a stretch from the middle (burnt flecks, the
                    // envelope dips). One big cut mid-arc.
                    let big = s == 4 && k == 60;
                    let del = if big { 1400 } else { 90 + (k as usize % 7) * 22 };
                    runs.push(EditRun {
                        t0,
                        t1: t0 + 9_000,
                        pos: (len / 3).min(len),
                        del_chars: del.min(len),
                        ins: String::new(),
                    });
                    len = len.saturating_sub(del).max(200);
                } else {
                    // Prose arrives in ~5-14 word gusts near the leading edge.
                    let words = 5 + (k as usize % 10);
                    let ins = "the ferry held its line against the dark water "
                        .split_whitespace()
                        .cycle()
                        .take(words)
                        .collect::<Vec<_>>()
                        .join(" ")
                        + " ";
                    runs.push(EditRun {
                        t0,
                        t1: t0 + 12_000,
                        pos: len.saturating_sub(k as usize % 40),
                        del_chars: 0,
                        ins: ins.clone(),
                    });
                    len += ins.chars().count();
                }
            }
            let sit_end = start + 150 * 40_000;
            events.push(JournalEvent::Pass {
                t: sit_end + 60_000,
                mode: if s == 3 { "believing".into() } else { "developmental".into() },
                cards: 2 + (s as u32 % 3),
            });
            if s.is_multiple_of(2) {
                events.push(JournalEvent::CardClosed {
                    t: sit_end + 300_000,
                    id: s as u64 + 1,
                    resolved: true,
                });
            }
        }
        self.doc.set_journal(Journal::from_parts(runs, events));
        self.mark_dirty();
        cx.notify();
    }

    /// Rig hook (`seed:legacy`): the legacy litmus shape (Bug A) — a store with
    /// six materialized checkpoints spread across two weeks (growing word
    /// counts, one mid-arc cut) and an EMPTY journal. Before Bug A this
    /// rendered as an empty strip: `Timeline::build` read only the journal, so
    /// every checkpoint tick landed at `work_at = 0` and overprinted the left
    /// edge. Now the checkpoint states ARE the axis — stations spread, the
    /// envelope steps, the whole pre-journal history is scannable.
    pub fn debug_seed_legacy(&mut self, cx: &mut Context<Self>) {
        use strop_core::journal::Journal;
        use strop_core::store::CheckpointState;
        let Some(store) = self.store.as_ref() else {
            eprintln!("strop: seed:legacy needs a document file");
            return;
        };
        let now_secs = strop_core::journal::now_ms() / 1000;
        let day = 86_400i64;
        // (days_ago, name, words, manual). Growing, with a dip at the rework —
        // the envelope steps up at a cut, as a real draft's does.
        let plan: [(i64, &str, usize, bool); 6] = [
            (13, "Started", 90, false),
            (11, "First scenes", 340, true),
            (8, "Middle drafted", 720, true),
            (5, "Rework the arc", 610, true),
            (2, "Full draft", 1500, true),
            (0, "Draft complete", 2100, false),
        ];
        for (days_ago, name, words, manual) in plan {
            let text = seed_prose(words);
            let lines = text.lines().count().max(1);
            let state = CheckpointState {
                text,
                spans: SpanSet::default(),
                blocks: BlockMap::new(lines),
            };
            store.debug_push_checkpoint(name, now_secs - days_ago * day - 4 * 3600, manual, state);
        }
        // The litmus is checkpoints WITHOUT a journal (no keystroke record).
        self.doc.set_journal(Journal::default());
        self.mark_dirty();
        cx.notify();
    }

    /// Rig hook (`strip:open`): open the strip surface.
    pub fn debug_strip_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_strip(window, cx);
    }

    /// Rig hook (`strip:scrub:<0..1>`): park at a fraction of the whole history
    /// (via the timeline, not the rail geometry — deterministic headless).
    pub fn debug_strip_scrub(&mut self, frac: f32, cx: &mut Context<Self>) {
        // Mirror the click path's no-history guard (a zero-width axis never
        // parks) so the rig can assert it.
        if !self.strip_has_past() {
            return;
        }
        let pos = self
            .strip
            .bake
            .as_ref()
            .map(|b| b.timeline.wall_at(frac.clamp(0., 1.) * b.timeline.total_work));
        if let Some(pos) = pos {
            self.strip.parked = true;
            self.strip.scrubbing = false;
            self.strip_scrub_to(pos, cx);
        }
    }

    /// Rig hook (`strip:pin:<0..1>`): set/clear the Compare pin at a fraction.
    pub fn debug_strip_pin(&mut self, frac: f32, cx: &mut Context<Self>) {
        let pos = self
            .strip
            .bake
            .as_ref()
            .map(|b| b.timeline.wall_at(frac.clamp(0., 1.) * b.timeline.total_work));
        if let Some(pos) = pos {
            self.strip_toggle_pin(pos, cx);
        }
    }

    /// Rig hook (`strip:restore`): the strip's Restore (re-bakes — the one
    /// lawful in-session re-bake, so `bakes` legitimately bumps here).
    pub fn debug_strip_restore(&mut self, cx: &mut Context<Self>) {
        self.strip_restore(cx);
    }

    /// Rig hook (`strip:now`): Esc/Now — drop the preview, back to the present.
    pub fn debug_strip_now(&mut self, cx: &mut Context<Self>) {
        self.strip_return_to_now(cx);
    }

    /// Rig hook (`seed:many`): a CROWDED lane — eight diagnoses across two
    /// passes, none selected, so the full-size budget (FULL_DIAGNOSIS_CAP)
    /// visibly recedes the oldest pass to one-line cards while every flagged
    /// passage keeps a card in the lane (the honesty invariant, asserted by
    /// rig-check.sh against a real frame). Quotes must appear, in order, in
    /// the open document — rig-check.sh writes a fixture that contains them.
    pub fn debug_seed_many(&mut self, cx: &mut Context<Self>) {
        use strop_core::diagnose::{Diagnosis, to_annotations};
        let mk = |i: usize| Diagnosis {
            quote: format!("crowded margin phrase number {i}"),
            problem: format!("finding {i}"),
            query: "Does this line carry its weight?".into(),
            level: "line".into(),
        };
        for (pass, range) in [(1u64, 1..=4usize), (2, 5..=8)] {
            let demos: Vec<Diagnosis> = range.map(mk).collect();
            let anns = to_annotations(&self.doc.text(), demos, self.doc.notes(), 0, pass);
            self.doc.add_diagnoses(anns);
        }
        self.drafting = false; // reviewing: the editor's cards are shown
        self.mark_dirty();
        cx.notify();
    }

    pub fn debug_cursor(&self) -> String {
        let cursor = self.cursor_offset();
        let tail_start = self.doc.rope().byte_to_char(cursor).saturating_sub(12);
        let tail: String = self
            .doc
            .rope()
            .chars_at(tail_start)
            .take(self.doc.rope().byte_to_char(cursor) - tail_start)
            .collect();
        let hist = match &self.history_view {
            Some(hv) => {
                let (bytes, pspans, styled_kinds) =
                    self.history_preview.as_ref().map_or((0, 0, 0), |p| {
                        (
                            p.text.len(),
                            p.spans_bytes.len(),
                            p.kinds
                                .iter()
                                .filter(|k| !matches!(k, BlockKind::Paragraph))
                                .count(),
                        )
                    });
                format!(
                    " hist={}/{} preview={bytes}b pspans={pspans} pkinds={styled_kinds}",
                    hv.selected + 1,
                    hv.entries.len(),
                )
            }
            None => String::new(),
        };
        let ai = match &self.ai_status {
            None => "".to_owned(),
            Some(AiStatus::NeedsSetup { local_model: Some(m) }) => {
                format!(" ai=needs-setup local={m}")
            }
            Some(AiStatus::NeedsSetup { local_model: None }) => " ai=needs-setup".to_owned(),
            Some(AiStatus::Running { .. }) => " ai=running".to_owned(),
            Some(AiStatus::Note { .. }) => " ai=note".to_owned(),
            Some(AiStatus::Error { .. }) => " ai=error".to_owned(),
        };
        let pending = if self.pending_pass.is_some() { " ai_pending=1" } else { "" };
        let panel = match &self.ai_settings {
            None => String::new(),
            Some(p) => format!(
                " ai_panel={} ai_models={}",
                match &p.test {
                    AiSettingsTest::Idle => "open",
                    AiSettingsTest::Running => "testing",
                    AiSettingsTest::Ok { .. } => "ok",
                    AiSettingsTest::Failed { .. } => "err",
                },
                p.models.len()
            ),
        };
        // F5 session tags: outline rail, word goal, open-time intent.
        let mut session = String::new();
        if self.rail_open {
            session += " rail=open";
        }
        // The door (DESIGN §4.4): drafting hides the editor's cards, reviewing
        // shows them; the held-back counts let smoke prove the rail's content.
        if self.drafting {
            session += &format!(" door=draft resting={}", self.resting_diagnoses());
        } else {
            session += " door=review";
            let held = self.suppressed_copy();
            if held > 0 {
                session += &format!(" copy_held={held}");
            }
        }
        if let Some((goal, start)) = self.session_goal {
            session += &format!(" goal={:+}/{goal}", self.word_count as i64 - start as i64);
        }
        if self.session_had_edits {
            session += " edits=1";
        }
        // F6 explorability tags: the palette's first row for the live
        // query (Frequent/ prefix marks the frequency section) and the
        // chord whisper's visibility window.
        if self.palette_input.is_some() {
            let top = match self.omni_rows(&self.palette_query).into_iter().next() {
                Some(OmniRow::Frequent(cmd)) => Some(format!("Frequent/{}", cmd.label)),
                Some(OmniRow::Cmd(cmd)) => Some(cmd.label.to_owned()),
                Some(OmniRow::Recent(p)) => Some(format!("Recent/{}", p.display())),
                Some(OmniRow::Match { line, .. }) => Some(format!("Match/L{}", line + 1)),
                Some(OmniRow::Heading { text, .. }) => Some(format!("Heading/{text}")),
                None => None,
            };
            session += &format!(" palette_top={:?}", top.unwrap_or_default());
        }
        if self.chord_whisper.is_some() {
            // The generation stays at 1 for the whole session if the
            // once-per-session rule holds.
            session += &format!(" whisper=chord/{}", self.chord_whisper_generation);
        }
        let doc_state = format!(
            "off={cursor} sel={:?} tail={tail:?} kind={:?} spans={:?}{hist}{ai}{pending}{panel}{session} mode={}",
            self.selected_range,
            self.doc.blocks().kind(self.doc.block_of_byte(cursor)),
            self.doc.spans().spans(),
            self.effective_mode(),
        );
        // Geometry may lag when the compositor throttles an occluded
        // window; doc state above stays authoritative.
        let geometry = self
            .last_frame
            .as_ref()
            .and_then(|f| f.cursor_position(cursor, self.cursor_affinity_down))
            .map(|(par, line, x)| {
                format!(
                    "par={par} line={line} x={x:?} aff={} scroll={:?}",
                    self.cursor_affinity_down as u8, self.scroll_top
                )
            })
            .unwrap_or_else(|| "geom=stale".into());
        format!("{doc_state} {geometry}")
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.doc.byte_to_utf16(range.start)..self.doc.byte_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.doc.utf16_to_byte(range.start)..self.doc.utf16_to_byte(range.end)
    }

    /// Core text replacement; optionally runs the typograph on the result.
    fn apply_replace(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        typograph: bool,
        cx: &mut Context<Self>,
    ) {
        // The past is READ-ONLY (Bug B): every edit gesture while parked —
        // insert, delete, paste alike — leaves the text untouched and pulses
        // the banner instead. The old restore-then-type path is gone: a
        // keystroke must never silently commit a restore (P2, the writer's own
        // terror — "even I was scared we'd broken everything"). Restore is the
        // explicit chip; Esc returns to now.
        if self.strip.is_parked() {
            self.pulse_strip(cx);
            return;
        }
        if self.history_view.is_some() {
            return; // history preview is read-only
        }
        self.selection_popover = false;
        let range = range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        // The auto-cut trigger (docs/impl/02-asides.md §4): a SINGLE
        // selection-deletion op of a manuscript passage ≥ AUTO_CUT_MIN_CHARS
        // files itself in the graveyard. Only a real deletion of a real
        // selection — a replace-by-typing keeps its text and never files; a
        // backspace run is many one-char ops, none reaching the threshold
        // (H24 + H43). The cut never straddles the boundary (selections are
        // clamped, review B4). See `auto_cut_qualifies` for the pure predicate.
        if !range.is_empty() {
            let start_char = self.doc.rope().byte_to_char(range.start);
            let end_char = self.doc.rope().byte_to_char(range.end);
            if auto_cut_qualifies(
                new_text,
                end_char - start_char,
                start_char,
                self.doc.manuscript_base_char(),
            ) {
                self.file_cut(range, cx);
                return;
            }
        }

        self.doc.edit_bytes_coalescing(range.clone(), new_text);
        let mut cursor = range.start + new_text.len();
        let mut block_shortcut_fired = false;

        // `# `..`### ` at block start converts to a heading; the hash
        // prefix is removed in the same transaction, so one undo restores
        // the literal hashes and the paragraph kind together.
        if typograph && new_text == " " {
            let (par_start, _) = self.paragraph_bounds(cursor);
            let head = self.doc.slice_bytes(par_start..cursor);
            let hashes = head.strip_suffix(' ').unwrap_or("");
            if !hashes.is_empty() && hashes.len() <= 3 && hashes.bytes().all(|b| b == b'#') {
                let level = hashes.len() as u8;
                let block = self.doc.block_of_byte(par_start);
                self.doc.edit_bytes(par_start..cursor, "");
                self.doc
                    .set_block_kind_in_current_tx(block, BlockKind::Heading(level));
                cursor = par_start;
                block_shortcut_fired = true;
            }
        }

        if typograph && !block_shortcut_fired {
            let (par_start, _) = self.paragraph_bounds(cursor);
            let prefix = self.doc.slice_bytes(par_start..cursor);
            let lang = match self.config.language {
                Language::Ru => typograph::Lang::Ru,
                Language::En => typograph::Lang::En,
                Language::Auto => typograph::detect_lang(self.doc.rope().chars()),
            };
            if let Some(sub) = typograph::process(&prefix, lang) {
                // The substitution is its own transaction: one undo reverts
                // it alone, restoring the literally-typed characters — and
                // since rules fire only on the typed char, the restored text
                // never re-fires (the Birman override contract).
                let start = cursor - sub.span;
                self.doc.edit_bytes(start..cursor, &sub.text);
                cursor = start + sub.text.len();
            }
        }

        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.cursor_affinity_down = false;
        self.goal_x = None;
        self.marked_range = None;
        self.sync_mutations();

        // Sticky caret formatting applies to what was just inserted (after
        // sync, so it layers over the spans' own expansion behavior).
        if !new_text.is_empty() && !self.caret_attrs.is_empty() {
            let char_range = {
                let rope = self.doc.rope();
                rope.byte_to_char(range.start)..rope.byte_to_char(cursor)
            };
            if char_range.start < char_range.end {
                for (attr, on) in self.caret_attrs.clone() {
                    self.doc.format_in_current_tx(char_range.clone(), attr, on);
                }
            }
        }

        self.bump_activity();
        cx.notify();
    }
}

impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.doc.slice_bytes(range))
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked_range.as_ref().map(|r| self.range_to_utf16(r))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Typograph only single typed characters — multi-char inserts are
        // IME commits or programmatic and arrive already authored.
        let typograph = new_text.chars().count() == 1;
        self.apply_replace(range_utf16, new_text, typograph, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.doc.edit_bytes(range.clone(), new_text);
        self.marked_range = if new_text.is_empty() {
            None
        } else {
            Some(range.start..range.start + new_text.len())
        };
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .map(|r| range.start + r.start..range.start + r.end)
            .unwrap_or_else(|| {
                let cursor = range.start + new_text.len();
                cursor..cursor
            });
        self.selection_reversed = false;
        self.cursor_affinity_down = false;
        self.goal_x = None;
        self.sync_mutations();
        self.bump_activity();
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let frame = self.last_frame.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        let pos = frame.position_of(range.start, self.cursor_affinity_down)?;
        Some(Bounds::new(
            frame.bounds.origin + pos - point(px(0.), frame.scroll_top),
            size(px(2.), frame.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let (byte_ix, _) = self.index_for_mouse(point);
        Some(self.doc.byte_to_utf16(byte_ix))
    }
}

// -- Element ------------------------------------------------------------------

struct EditorElement {
    editor: Entity<Editor>,
}

/// The inputs that determine the laid-out paragraphs. Two frames with equal
/// keys produce a byte-identical layout, so the later one reuses the earlier
/// frame's paragraphs instead of re-shaping the whole document (the prepaint
/// fast-path). Scroll offset and caret position are deliberately absent — they
/// don't affect paragraph geometry, only the carried-forward scroll/caret quad,
/// which is recomputed every frame regardless.
#[derive(Clone, PartialEq)]
struct LayoutKey {
    /// Document content (text + spans + blocks + note ranges) — see
    /// `Document::revision`.
    revision: u64,
    width_bits: u32,
    font_scale_bits: u32,
    /// Non-empty selection only: an empty selection paints no highlight, so a
    /// collapsed-caret move leaves the layout identical.
    selection: Option<(usize, usize)>,
    marked: Option<(usize, usize)>,
    find_query: Option<String>,
    active_note: Option<u64>,
    /// The compost arrival blink is a paint decoration baked into the
    /// paragraphs, but it is NOT captured by `revision` (clearing the flash
    /// bumps nothing). Keying on it forces a rebuild when it toggles, so the
    /// blink actually clears instead of sticking on the reuse fast-path.
    compost_flash: bool,
    /// The graveyard tail section is painted after the last block; its content
    /// and the counter live outside `revision` too (a bar flash, an entry
    /// expand). Keying on a coarse fingerprint rebuilds when they change.
    grave_fingerprint: u64,
}

struct PrepaintState {
    paragraphs: Vec<ParagraphLayout>,
    grave_lines: Vec<GraveLine>,
    grave_section_top: Option<Pixels>,
    cursor: Option<PaintQuad>,
    line_height: Pixels,
    scroll_top: Pixels,
    content_height: Pixels,
    layout_key: LayoutKey,
}

impl IntoElement for EditorElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Per-kind block metrics and decorations, per the PT-pairings research:
/// PT Sans Bold headings over the PT Serif body (the superfamily is
/// metrically harmonized for exactly this), all boxes on the 28px rhythm.
/// PT ships no SemiBold, so the sans face alone carries the H3 contrast.
struct BlockStyle {
    size: Pixels,
    line_height: Pixels,
    indent: Pixels,
    extra_top: Pixels,
    family: Option<&'static str>,
    weight: Option<FontWeight>,
    muted: bool,
    bg: Option<gpui::Rgba>,
    quote_rule: bool,
}

impl Default for BlockStyle {
    fn default() -> Self {
        Self {
            size: px(20.),
            line_height: px(28.),
            indent: px(0.),
            extra_top: px(0.),
            family: None,
            weight: None,
            muted: false,
            bg: None,
            quote_rule: false,
        }
    }
}

fn block_style_scaled(kind: &BlockKind, scale: f32) -> BlockStyle {
    let mut style = block_style(kind);
    if (scale - 1.).abs() > f32::EPSILON {
        // Keep the rhythm: line heights round to 2px so boxes stay tidy.
        style.size = px((f32::from(style.size) * scale).round());
        style.line_height = px((f32::from(style.line_height) * scale / 2.).round() * 2.);
        style.extra_top = px((f32::from(style.extra_top) * scale / 2.).round() * 2.);
    }
    style
}

fn block_style(kind: &BlockKind) -> BlockStyle {
    let heading = Some("PT Sans");
    let bold = Some(FontWeight::BOLD);
    match kind {
        BlockKind::Heading(1) => BlockStyle {
            size: px(32.),
            line_height: px(42.),
            extra_top: px(14.),
            family: heading,
            weight: bold,
            ..Default::default()
        },
        BlockKind::Heading(2) => BlockStyle {
            size: px(24.),
            family: heading,
            weight: bold,
            ..Default::default()
        },
        BlockKind::Heading(_) => BlockStyle {
            family: heading,
            weight: bold,
            ..Default::default()
        },
        BlockKind::Blockquote => BlockStyle {
            indent: px(28.),
            quote_rule: true,
            ..Default::default()
        },
        BlockKind::ListItem { depth, .. } => BlockStyle {
            indent: px(28.) * (*depth as f32 + 1.),
            ..Default::default()
        },
        BlockKind::Divider => BlockStyle {
            muted: true,
            ..Default::default()
        },
        BlockKind::FootnoteDef { .. } => BlockStyle {
            muted: true,
            // The page-bottom register: ~0.9× body (H4 — a Footnotes
            // section, set apart in size as well as place).
            size: px(18.),
            line_height: px(25.),
            // Room for the painted "N." marker, list-style.
            indent: px(28.),
            ..Default::default()
        },
        BlockKind::CodeBlock { .. } => BlockStyle {
            size: px(16.),
            indent: px(12.),
            family: Some(CODE_FONT),
            bg: Some(rgba(CODE_BG_COLOR)),
            ..Default::default()
        },
        _ => BlockStyle::default(),
    }
}

/// Painted footnote numbers (DESIGN §2-footnotes): the Nth distinct ref in
/// text order paints as N — numbering derives from ref order while stored
/// ids stay stable internal labels, the universal Word/Pandoc architecture.
/// Defs whose ref is gone get the following numbers in block order, so an
/// orphaned def keeps a painted identity instead of going blank.
fn footnote_numbers(refs: &[(usize, &str)], kinds: &[BlockKind]) -> HashMap<String, usize> {
    let mut ordered: Vec<(usize, &str)> = refs.to_vec();
    ordered.sort_unstable();
    let mut map = HashMap::new();
    for (_, id) in ordered {
        let next = map.len() + 1;
        map.entry(id.to_owned()).or_insert(next);
    }
    for kind in kinds {
        if let BlockKind::FootnoteDef { id } = kind {
            let next = map.len() + 1;
            map.entry(id.clone()).or_insert(next);
        }
    }
    map
}

/// Alpha source-over compositing, so selection never hides content
/// backgrounds (highlight, code — and later annotation markings): partial
/// overlaps already split into their own runs by the cut logic.
fn blend_over(top: gpui::Rgba, bottom: gpui::Rgba) -> gpui::Rgba {
    let a = top.a + bottom.a * (1. - top.a);
    if a <= f32::EPSILON {
        return gpui::Rgba {
            r: 0.,
            g: 0.,
            b: 0.,
            a: 0.,
        };
    }
    let ch = |t: f32, b: f32| (t * top.a + b * bottom.a * (1. - top.a)) / a;
    gpui::Rgba {
        r: ch(top.r, bottom.r),
        g: ch(top.g, bottom.g),
        b: ch(top.b, bottom.b),
        a,
    }
}

/// Split a paragraph into runs cut at selection/marked/formatting
/// boundaries. Selection and highlight paint via
/// `WrappedLine::paint_background`; IME composition gets an underline;
/// formatting maps to font weight/style and decorations.
// One parameter per cut source; bundling them into a struct would only
// rename the arity, not reduce it.
#[allow(clippy::too_many_arguments)]
fn runs_for_paragraph(
    par_range: &Range<usize>,
    selection: &Range<usize>,
    marked: Option<&Range<usize>>,
    spans: &[(Range<usize>, InlineAttr)],
    notes: &[(Range<usize>, bool, bool)],
    matches: &[Range<usize>],
    dels: &[Range<usize>],
    base: &TextRun,
) -> Vec<TextRun> {
    let mut cuts = vec![par_range.start, par_range.end];
    for r in [Some(selection), marked].into_iter().flatten() {
        cuts.push(r.start.clamp(par_range.start, par_range.end));
        cuts.push(r.end.clamp(par_range.start, par_range.end));
    }
    for (r, _) in spans {
        cuts.push(r.start.clamp(par_range.start, par_range.end));
        cuts.push(r.end.clamp(par_range.start, par_range.end));
    }
    for (r, _, _) in notes {
        cuts.push(r.start.clamp(par_range.start, par_range.end));
        cuts.push(r.end.clamp(par_range.start, par_range.end));
    }
    for r in matches.iter().chain(dels.iter()) {
        cuts.push(r.start.clamp(par_range.start, par_range.end));
        cuts.push(r.end.clamp(par_range.start, par_range.end));
    }
    cuts.sort_unstable();
    cuts.dedup();

    cuts.windows(2)
        .filter(|w| w[1] > w[0])
        .map(|w| {
            let in_selection = w[0] >= selection.start && w[1] <= selection.end;
            let in_marked = marked.is_some_and(|m| w[0] >= m.start && w[1] <= m.end);

            let mut font = base.font.clone();
            let mut color = base.color;
            // Content background (highlight/code); selection composites
            // over it at the end instead of replacing it.
            let mut content_bg: Option<gpui::Rgba> = None;
            let mut underline = in_marked.then(|| UnderlineStyle {
                color: Some(base.color),
                thickness: px(1.),
                wavy: false,
            });
            let mut strikethrough = None;

            for (range, attr) in spans {
                if !(range.start <= w[0] && w[1] <= range.end) {
                    continue;
                }
                match attr {
                    InlineAttr::Strong => font.weight = FontWeight::BOLD,
                    InlineAttr::Emphasis => font.style = FontStyle::Italic,
                    InlineAttr::Underline => {
                        underline.get_or_insert(UnderlineStyle {
                            color: Some(color),
                            thickness: px(1.),
                            wavy: false,
                        });
                    }
                    InlineAttr::Strikethrough => {
                        strikethrough = Some(StrikethroughStyle {
                            color: Some(color),
                            thickness: px(1.),
                        });
                    }
                    InlineAttr::Highlight => {
                        content_bg.get_or_insert(rgba(HIGHLIGHT_COLOR));
                    }
                    InlineAttr::Code => {
                        font.family = CODE_FONT.into();
                        content_bg.get_or_insert(rgba(CODE_BG_COLOR));
                    }
                    InlineAttr::Link(_) => {
                        color = rgb(LINK_COLOR).into();
                        underline.get_or_insert(UnderlineStyle {
                            color: Some(rgb(LINK_COLOR).into()),
                            thickness: px(1.),
                            wavy: false,
                        });
                    }
                    InlineAttr::FootnoteRef(_) => {
                        // The carrier digits keep their advance (caret,
                        // hit-testing) but paint transparent; paint() draws
                        // a superior figure over them (DESIGN §2-footnotes
                        // — PT ships no superscripts, so we paint our own,
                        // same machinery as list markers).
                        color = gpui::transparent_black();
                    }
                }
            }

            // Note anchors tint (wheat); diagnosis anchors get a quiet WAVY
            // squiggle in muted ink — the spellcheck idiom, so a tool mark
            // never reads as the writer's own straight underline (ctrl-u; the
            // one mark §2 keeps) — promoting to a tint when active. If a span
            // carries the writer's underline already, that straight line wins
            // (get_or_insert): their formatting outranks the tool's mark.
            // Selection composites over everything.
            for (r, active, is_diagnosis) in notes {
                if r.start <= w[0] && w[1] <= r.end {
                    if *is_diagnosis && !active {
                        // The AI's anchor mark wears the cool machine-voice ink
                        // (color language): a wavy blue squiggle, distinct from
                        // the writer's warm amber note tint.
                        underline.get_or_insert(UnderlineStyle {
                            color: Some(rgb(AI_ACCENT).into()),
                            thickness: px(1.),
                            wavy: true,
                        });
                        continue;
                    }
                    // The active band wears its layer's voice: a diagnosis lights
                    // up cool blue (matching its card + squiggle — the machine is
                    // pointing here), a writer note warm amber. A resting note
                    // keeps its faint amber tint; a resting diagnosis took the
                    // squiggle branch above and never reaches here.
                    let tint = if *is_diagnosis {
                        rgba(DIAGNOSIS_TINT_ACTIVE)
                    } else if *active {
                        rgba(NOTE_TINT_ACTIVE)
                    } else {
                        rgba(NOTE_TINT)
                    };
                    content_bg = Some(match content_bg {
                        Some(bg) => blend_over(tint, bg),
                        None => tint,
                    });
                }
            }

            for r in matches {
                if r.start <= w[0] && w[1] <= r.end {
                    content_bg = Some(match content_bg {
                        Some(bg) => blend_over(rgba(FIND_MATCH_BG), bg),
                        None => rgba(FIND_MATCH_BG), // sage — distinct from wheat
                    });
                }
            }
            // Diff deletions: strikethrough, dimmed — Track Changes idiom.
            for r in dels {
                if r.start <= w[0] && w[1] <= r.end {
                    color = rgb(MUTED_COLOR).into();
                    strikethrough = Some(StrikethroughStyle {
                        color: Some(rgb(MUTED_COLOR).into()),
                        thickness: px(1.),
                    });
                }
            }

            let background = match (in_selection, content_bg) {
                (true, Some(bg)) => Some(blend_over(rgba(SELECTION_COLOR), bg).into()),
                (true, None) => Some(rgba(SELECTION_COLOR).into()),
                (false, Some(bg)) => Some(bg.into()),
                (false, None) => None,
            };

            TextRun {
                len: w[1] - w[0],
                font,
                color,
                background_color: background,
                underline,
                strikethrough,
            }
        })
        .collect()
}

/// The graveyard data a frame needs, captured while the `editor` borrow is live
/// (before it ends at the per-block reuse take), then shaped after the loop.
struct GraveEntryView {
    id: u64,
    text: String,
    origin_quote: String,
    cut_unix: i64,
    words: u32,
    /// Rendered full (whisper + full text): the newest, or one the writer
    /// expanded.
    full: bool,
}

const GRAVE_SECTION_GAP: f32 = 44.; // breathing room after the last prose block
const GRAVE_HEADER_H: f32 = 26.;
const GRAVE_WHISPER_H: f32 = 20.;
const GRAVE_BODY_SIZE: f32 = 15.;
const GRAVE_BODY_LH: f32 = 22.;
const GRAVE_BODY_INDENT: f32 = 14.; // room for the 3px stale left-rule
const GRAVE_RECEDED_H: f32 = 24.;
const GRAVE_ENTRY_GAP: f32 = 16.;

/// Shape one graveyard segment (a piece of a whisper/header/receded row).
fn grave_seg(
    window: &mut Window,
    text: impl Into<String>,
    size: f32,
    color: u32,
    family: &'static str,
    italic: bool,
    underline: bool,
) -> gpui::ShapedLine {
    let text: String = text.into();
    let mut font = gpui::font(family);
    if italic {
        font.style = FontStyle::Italic;
    }
    let hsla: gpui::Hsla = rgb(color).into();
    let run = TextRun {
        len: text.len(),
        font,
        color: hsla,
        background_color: None,
        underline: underline.then_some(UnderlineStyle {
            color: Some(hsla),
            thickness: px(1.),
            wavy: false,
        }),
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line(SharedString::from(text), px(size), &[run], None)
}

/// Build the graveyard tail section's pre-shaped rows (Bug B, asides.md §3):
/// a "Graveyard · N" header, then entries newest-first — the newest (or an
/// expanded one) in full (whisper + verbs + the full cut text, dimmed, ruled),
/// older ones receded to a single clickable line. Returns the rows and the
/// section's bottom (the new content height). `entries` is already newest-first.
fn shape_grave_section(
    window: &mut Window,
    entries: &[GraveEntryView],
    wrap_width: Pixels,
    start_top: Pixels,
) -> (Vec<GraveLine>, Pixels) {
    let mut lines = Vec::new();
    if entries.is_empty() {
        return (lines, start_top);
    }
    let mut top = start_top;

    // Section header: the tombstone slab is drawn in paint; the label sits to
    // its right (same idiom as the footer bar).
    let header = grave_seg(window, format!("Graveyard · {}", entries.len()), 13., MUTED_COLOR, "PT Sans", false, false);
    lines.push(GraveLine {
        segments: vec![(px(18.), header)],
        body: None,
        top,
        height: px(GRAVE_HEADER_H),
        line_height: px(GRAVE_HEADER_H),
        left_rule: false,
        header: true,
        hits: Vec::new(),
    });
    top += px(GRAVE_HEADER_H) + px(10.);

    for e in entries {
        // The whisper speaks the strip's own date grammar ("Today",
        // "Fri 19 Jun") — one calendar voice per app (P8), never the machine's
        // ISO stamp. STROP_TEST_STILL keeps the frozen form for byte-compares.
        let date = if std::env::var("STROP_TEST_STILL").is_ok() {
            format_unix(e.cut_unix)
        } else {
            strip::date_label(e.cut_unix, strop_core::journal::now_ms() / 1000)
        };
        if e.full {
            // The whisper row: prefix + optional italic origin quote + verbs.
            let mut segments: Vec<(Pixels, gpui::ShapedLine)> = Vec::new();
            let mut hits: Vec<(Bounds<Pixels>, GraveAction)> = Vec::new();
            let mut x = px(0.);
            let mut push =
                |window: &mut Window, text: String, italic: bool, action: Option<GraveAction>| {
                    let color = if action.is_some() { AI_ACCENT } else { MUTED_COLOR };
                    let shaped = grave_seg(window, text, 11., color, "PT Sans", italic, action.is_some());
                    let w = shaped.width();
                    if let Some(a) = action {
                        hits.push((
                            Bounds::new(point(x, top), size(w, px(GRAVE_WHISPER_H))),
                            a,
                        ));
                    }
                    segments.push((x, shaped));
                    x += w;
                };
            if e.origin_quote.is_empty() {
                push(window, format!("cut · {date} · "), false, None);
            } else {
                push(window, "cut from after ".to_string(), false, None);
                push(window, format!("\u{201C}\u{2026}{}\u{201D}", e.origin_quote), true, None);
                push(window, format!(" · {date} · "), false, None);
            }
            push(window, "show origin".to_string(), false, Some(GraveAction::ShowOrigin(e.id)));
            push(window, " · ".to_string(), false, None);
            push(window, "put back".to_string(), false, Some(GraveAction::PutBack(e.id)));
            push(window, " · ".to_string(), false, None);
            push(window, "delete".to_string(), false, Some(GraveAction::Delete(e.id)));
            lines.push(GraveLine {
                segments,
                body: None,
                top,
                height: px(GRAVE_WHISPER_H),
                line_height: px(GRAVE_WHISPER_H),
                left_rule: false,
                header: false,
                hits,
            });
            top += px(GRAVE_WHISPER_H);

            // The full cut text, dimmed, ruled — one wrapped row per source
            // paragraph, no character cap (P1: the record is verbatim).
            let body_run = TextRun {
                len: 0,
                font: gpui::font("PT Serif"),
                color: rgb(MUTED_COLOR).into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            for para in e.text.split('\n') {
                let run = TextRun { len: para.len(), ..body_run.clone() };
                let wrapped = window
                    .text_system()
                    .shape_text(
                        SharedString::from(para.to_owned()),
                        px(GRAVE_BODY_SIZE),
                        &[run],
                        Some(wrap_width - px(GRAVE_BODY_INDENT)),
                        None,
                    )
                    .expect("shape_text failed")
                    .into_iter()
                    .next()
                    .expect("shape_text returned no lines");
                let h = wrapped.size(px(GRAVE_BODY_LH)).height;
                lines.push(GraveLine {
                    segments: Vec::new(),
                    body: Some((px(GRAVE_BODY_INDENT), wrapped)),
                    top,
                    height: h,
                    line_height: px(GRAVE_BODY_LH),
                    left_rule: true,
                    header: false,
                    hits: Vec::new(),
                });
                top += h;
            }
            top += px(GRAVE_ENTRY_GAP);
        } else {
            // Receded: one clickable line, click expands in place.
            let first: String = e.text.split_whitespace().take(7).collect::<Vec<_>>().join(" ");
            let quote = if e.origin_quote.is_empty() {
                String::new()
            } else {
                format!(" · cut from after \u{201C}\u{2026}{}\u{201D}", e.origin_quote)
            };
            let line = format!("{first}{quote} · {date} · {} words", e.words);
            let shaped = grave_seg(window, line, 12., MUTED_COLOR, "PT Sans", false, false);
            let w = shaped.width();
            lines.push(GraveLine {
                segments: vec![(px(GRAVE_BODY_INDENT), shaped)],
                body: None,
                top,
                height: px(GRAVE_RECEDED_H),
                line_height: px(GRAVE_RECEDED_H),
                left_rule: true,
                header: false,
                hits: vec![(
                    Bounds::new(point(px(0.), top), size(w + px(GRAVE_BODY_INDENT), px(GRAVE_RECEDED_H))),
                    GraveAction::Expand(e.id),
                )],
            });
            top += px(GRAVE_RECEDED_H);
        }
    }
    (lines, top)
}

impl Element for EditorElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let _guard = DrawGuard::enter();
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let _guard = DrawGuard::enter();
        let perf_start = std::env::var_os("STROP_PERF").map(|_| Instant::now());
        // Ensure encoded-image handles exist for every Image block (the
        // actual decode is async inside GPUI's asset cache).
        let needed: Vec<String> = {
            let editor = self.editor.read(cx);
            editor
                .doc
                .blocks()
                .kinds()
                .iter()
                .filter_map(|k| match k {
                    BlockKind::Image { src, .. }
                        if !editor.image_assets.contains_key(src) =>
                    {
                        Some(src.clone())
                    }
                    _ => None,
                })
                .collect()
        };
        if !needed.is_empty() {
            self.editor.update_in_draw(cx, |editor| {
                for id in needed {
                    let Some(store) = &editor.store else { continue };
                    let Some(bytes) = store.get_asset(&id) else { continue };
                    let format = if id.ends_with(".png") {
                        gpui::ImageFormat::Png
                    } else if id.ends_with(".webp") {
                        gpui::ImageFormat::Webp
                    } else {
                        gpui::ImageFormat::Jpeg
                    };
                    editor
                        .image_assets
                        .insert(id, Arc::new(gpui::Image::from_bytes(format, bytes)));
                }
            });
        }

        let editor = self.editor.read(cx);
        let style = window.text_style();
        let line_height = window.line_height();
        let paragraph_gap = line_height; // vertical rhythm: one full line
        let wrap_width = bounds.size.width;
        let viewport = bounds.size.height;

        let preview = editor.history_preview.clone();
        let in_history = preview.is_some();

        // --- Layout reuse fast-path ------------------------------------------
        // When the document, width, font and selection-highlight are unchanged
        // since the last paint, the laid-out paragraphs are byte-identical, so
        // there is no reason to re-materialize the text, re-project spans and
        // re-shape every block (the whole-document O(N) prepaint). Scroll,
        // cursor blink and collapsed-caret moves all land here — they touch
        // only the scroll offset and the caret quad, both recomputed below from
        // the carried-forward paragraphs. History preview and image blocks opt
        // out: the preview text and async image decode aren't captured by the
        // document revision.
        let layout_key = LayoutKey {
            revision: editor.doc.revision(),
            width_bits: f32::from(wrap_width).to_bits(),
            font_scale_bits: editor
                .config
                .font_size
                .map_or(1f32, |s| (s / 20.).clamp(0.6, 2.))
                .to_bits(),
            selection: {
                let s = &editor.selected_range;
                (!in_history && s.start != s.end).then_some((s.start, s.end))
            },
            marked: editor
                .marked_range
                .as_ref()
                .filter(|_| !in_history)
                .map(|m| (m.start, m.end)),
            find_query: if in_history {
                None
            } else if editor.palette_input.is_some() {
                match omni_mode(&editor.palette_query) {
                    (OmniMode::Find, rest) => Some(rest.to_string()),
                    _ => None,
                }
            } else {
                None
            },
            active_note: editor.focus.active_id(),
            compost_flash: editor.rail_flash.is_some(),
            grave_fingerprint: editor.grave_layout_fingerprint(),
        };
        let can_reuse = !in_history
            && !editor
                .doc
                .blocks()
                .kinds()
                .iter()
                .any(|k| matches!(k, BlockKind::Image { .. }))
            && editor.last_frame.as_ref().is_some_and(|f| {
                f.bounds.size.width == wrap_width && f.layout_key == layout_key
            });
        if can_reuse {
            let cursor_offset = editor.cursor_offset();
            let cursor_affinity = editor.cursor_affinity_down;
            let cursor_blink_visible = editor.cursor_visible; // !in_history holds here
            let autoscroll = editor.autoscroll_request;
            let mut scroll_top = editor.scroll_top;
            // `editor`'s immutable borrow ends here (last use above); the
            // mutable update_in_draw below is then free to run.
            let (paragraphs, grave_lines, grave_section_top, content_height) =
                self.editor.update_in_draw(cx, |ed| {
                    let f = ed
                        .last_frame
                        .as_mut()
                        .expect("can_reuse implies a stored frame");
                    (
                        std::mem::take(&mut f.paragraphs),
                        std::mem::take(&mut f.grave_lines),
                        f.grave_section_top,
                        f.content_height,
                    )
                });
            let max_scroll = (content_height + line_height - viewport).max(px(0.));
            scroll_top = scroll_top.clamp(px(0.), max_scroll);
            let cursor_pos = paragraphs
                .iter()
                .find(|p| cursor_offset <= p.range.end)
                .map(|par| {
                    let (line, x) =
                        par.position(cursor_offset - par.range.start, cursor_affinity);
                    (
                        point(x, par.top + par.line_height * (line as f32)),
                        par.line_height,
                    )
                });
            if autoscroll && let Some((pos, cursor_lh)) = cursor_pos {
                if pos.y < scroll_top {
                    scroll_top = pos.y;
                } else if pos.y + cursor_lh > scroll_top + viewport {
                    scroll_top = pos.y + cursor_lh - viewport;
                }
            }
            self.editor.update_in_draw(cx, |editor| {
                editor.scroll_top = scroll_top;
                editor.autoscroll_request = false;
            });
            let cursor = cursor_pos.and_then(|(pos, cursor_lh)| {
                let y = pos.y - scroll_top;
                if !cursor_blink_visible || y + cursor_lh <= px(0.) || y >= viewport {
                    return None;
                }
                Some(fill(
                    Bounds::new(bounds.origin + point(pos.x, y), size(px(2.), cursor_lh)),
                    rgb(TEXT_COLOR),
                ))
            });
            if let Some(start) = perf_start {
                eprintln!(
                    "strop-perf: prepaint {:?} (reuse, {} blocks)",
                    start.elapsed(),
                    paragraphs.len()
                );
            }
            return PrepaintState {
                paragraphs,
                grave_lines,
                grave_section_top,
                cursor,
                line_height,
                scroll_top,
                content_height,
                layout_key,
            };
        }
        // --- End fast-path; full rebuild below -------------------------------

        let (text, diff_inserts, diff_deletes, preview_spans, preview_kinds) = match preview {
            Some(p) => (p.text, p.inserts, p.deletes, Some(p.spans_bytes), Some(p.kinds)),
            None => (editor.doc.text(), Vec::new(), Vec::new(), None, None),
        };
        let selection = if in_history {
            0..0
        } else {
            editor.selected_range.clone()
        };
        let marked = if in_history {
            None
        } else {
            editor.marked_range.clone()
        };
        let cursor_offset = editor.cursor_offset();
        let cursor_affinity = editor.cursor_affinity_down;
        let cursor_blink_visible = editor.cursor_visible && !in_history;
        let mut scroll_top = editor.scroll_top;
        let autoscroll = editor.autoscroll_request;
        // Formatting spans, converted to byte ranges for this frame. In
        // history mode the preview carries its own, already projected
        // through the diff onto the merged text.
        let spans_bytes: Vec<(Range<usize>, InlineAttr)> = if let Some(spans) = preview_spans {
            spans
        } else {
            let rope = editor.doc.rope();
            editor
                .doc
                .spans()
                .spans()
                .iter()
                .map(|s| {
                    (
                        rope.char_to_byte(s.range.start)..rope.char_to_byte(s.range.end),
                        s.attr.clone(),
                    )
                })
                .collect()
        };

        let base_run = TextRun {
            len: 0,
            font: style.font(),
            color: style.color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };

        let font_scale = editor.config.font_size.map_or(1., |s| (s / 20.).clamp(0.6, 2.));
        let kinds: Vec<BlockKind> = if let Some(kinds) = preview_kinds {
            kinds
        } else {
            editor.doc.blocks().kinds().to_vec()
        };
        // Painted footnote numbers, derived from ref order in this frame's
        // text (preview or live) — stored ids stay internal labels.
        let fn_numbers = {
            let refs: Vec<(usize, &str)> = spans_bytes
                .iter()
                .filter_map(|(r, attr)| match attr {
                    InlineAttr::FootnoteRef(id) => Some((r.start, id.as_str())),
                    _ => None,
                })
                .collect();
            footnote_numbers(&refs, &kinds)
        };
        let find_matches: Vec<Range<usize>> = if in_history {
            diff_inserts.clone() // inserts reuse the sage tint
        } else if editor.palette_input.is_some() {
            // Tint every match while the omnibox is in find mode — the live
            // preview the old bottom strip never gave.
            match omni_mode(&editor.palette_query) {
                (OmniMode::Find, rest) => editor.find_matches(rest),
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        };
        // Open-annotation anchors (byte ranges, active, is_diagnosis):
        // notes tint; diagnoses underline quietly until activated.
        let note_ranges: Vec<(Range<usize>, bool, bool)> = if in_history {
            Vec::new()
        } else {
            let rope = editor.doc.rope();
            editor
                .doc
                .notes()
                .open()
                .map(|n| {
                    (
                        rope.char_to_byte(n.range.start)..rope.char_to_byte(n.range.end),
                        editor.focus.active_id() == Some(n.id),
                        n.kind == NoteKind::Diagnosis,
                    )
                })
                .collect()
        };
        let image_handles: Vec<Option<Arc<gpui::Image>>> = kinds
            .iter()
            .map(|k| match k {
                BlockKind::Image { src, .. } => editor.image_assets.get(src).cloned(),
                _ => None,
            })
            .collect();
        let scale = window.scale_factor();

        // Asides state captured while the `editor` borrow is still live (it ends
        // at the `prev` take below): the compost boundary + arrival-blink flag
        // for the compost face (Bug A), and the graveyard entries for the tail
        // section (Bug B). History mode shows neither.
        let aside_boundary = if in_history { None } else { editor.doc.aside_boundary() };
        let rail_flashing = editor.rail_flash.is_some();
        let grave_render: Vec<GraveEntryView> = if in_history {
            Vec::new()
        } else {
            let entries = editor.doc.graveyard().entries();
            let newest_id = entries.last().map(|e| e.id);
            entries
                .iter()
                .rev() // newest-first for the view
                .map(|e| GraveEntryView {
                    id: e.id,
                    text: e.text.clone(),
                    origin_quote: e.origin_quote.clone(),
                    cut_unix: e.cut_unix,
                    words: e.words,
                    full: Some(e.id) == newest_id || editor.grave_expanded.contains(&e.id),
                })
                .collect()
        };

        // Previous frame's paragraphs, for per-block shaped-line reuse on this
        // rebuild: a block whose text, runs and metrics are unchanged keeps its
        // already-shaped line instead of re-shaping. This makes a full rebuild
        // after a run of reuse frames cheap WITHOUT depending on GPUI's own
        // two-frame line cache (which the reuse frames let go cold). Taken only
        // when the wrap width matches — a resize reflows every line. Safe here:
        // the immutable `editor` borrow's last use was the setup above; the
        // loop below reads only locals + `window`.
        let mut prev: Option<Vec<Option<ParagraphLayout>>> =
            self.editor.update_in_draw(cx, |editor| match &mut editor.last_frame {
                Some(f) if f.bounds.size.width == wrap_width => {
                    Some(std::mem::take(&mut f.paragraphs).into_iter().map(Some).collect())
                }
                _ => None,
            });
        let mut paragraphs = Vec::new();
        let mut top = px(0.);
        let mut offset = 0;
        let mut ordered_no = 0usize;
        for (block_ix, par_text) in text.split('\n').enumerate() {
            let kind = kinds.get(block_ix).cloned().unwrap_or_default();
            let mut bstyle = block_style_scaled(&kind, font_scale);
            // The compost face (Bug A, asides.md §1): blocks `0..=boundary` are
            // the compost region — a mini-column, smaller and muted. Empty
            // compost/boundary blocks draw a hairline at their midline (the item
            // separator grammar); the boundary block also gets the tail anchor
            // bar (P11). The first compost block carries a small "COMPOST"
            // whisper above it. An absent boundary = no compost = none of this.
            let in_compost = aside_boundary.is_some_and(|b| block_ix <= b);
            let is_boundary = aside_boundary == Some(block_ix);
            let compost_rule = in_compost && par_text.is_empty();
            if in_compost {
                bstyle.size = px((f32::from(bstyle.size) * 0.8).round());
                bstyle.line_height = px((f32::from(bstyle.line_height) * 0.8 / 2.).round() * 2.);
                bstyle.muted = true;
            }
            let compost_header = (aside_boundary.is_some() && block_ix == 0).then(|| {
                grave_seg(window, "COMPOST", 10.5, MUTED_COLOR, "PT Sans", false, false)
            });
            if compost_header.is_some() {
                bstyle.extra_top += px(20.); // room for the whisper above block 0
            }
            let compost_flash = rail_flashing
                && aside_boundary.is_some_and(|b| block_ix + 1 == b && !par_text.is_empty());
            // The footnote-definition run at the document end reads as one
            // "Footnotes" section: a hairline rule sits above its first
            // block (H4). Detected by neighbour, not by kind alone.
            let section_rule = matches!(kind, BlockKind::FootnoteDef { .. })
                && block_ix > 0
                && !matches!(
                    kinds.get(block_ix - 1),
                    Some(BlockKind::FootnoteDef { .. })
                );
            let marker = match &kind {
                BlockKind::ListItem { ordered: false, .. } => {
                    ordered_no = 0;
                    Some(SharedString::from("•"))
                }
                BlockKind::ListItem { ordered: true, .. } => {
                    ordered_no += 1;
                    Some(SharedString::from(format!("{ordered_no}.")))
                }
                // The footnote's own number, visible while editing the
                // definition (the bottom zone only shows when the REF is
                // on-screen — the def line must carry its identity). The
                // painted number follows ref order, not the stored id.
                BlockKind::FootnoteDef { id } => {
                    ordered_no = 0;
                    let label = fn_numbers
                        .get(id)
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| id.clone());
                    Some(SharedString::from(format!("{label}.")))
                }
                _ => {
                    ordered_no = 0;
                    None
                }
            };

            let mut block_font = base_run.font.clone();
            if let Some(family) = bstyle.family {
                block_font.family = family.into();
            }
            if let Some(weight) = bstyle.weight {
                block_font.weight = weight;
            }
            let block_base = TextRun {
                font: block_font,
                color: if bstyle.muted {
                    rgb(MUTED_COLOR).into()
                } else {
                    base_run.color
                },
                ..base_run.clone()
            };

            let range = offset..offset + par_text.len();
            let par_spans: Vec<(Range<usize>, InlineAttr)> = spans_bytes
                .iter()
                .filter(|(r, _)| r.start < range.end && range.start < r.end)
                .cloned()
                .collect();
            let par_notes: Vec<(Range<usize>, bool, bool)> = note_ranges
                .iter()
                .filter(|(r, _, _)| r.start < range.end && range.start < r.end)
                .cloned()
                .collect();
            let par_matches: Vec<Range<usize>> = find_matches
                .iter()
                .filter(|r| r.start < range.end && range.start < r.end)
                .cloned()
                .collect();
            let par_dels: Vec<Range<usize>> = diff_deletes
                .iter()
                .filter(|r| r.start < range.end && range.start < r.end)
                .cloned()
                .collect();
            // Superior figures to paint over the invisible carriers —
            // skipped for refs inside history-diff deletions, where the
            // carrier re-inks (muted, struck) and a painted number on top
            // would double up.
            let fn_marks: Vec<(usize, gpui::ShapedLine)> = par_spans
                .iter()
                .filter_map(|(r, attr)| {
                    let InlineAttr::FootnoteRef(id) = attr else {
                        return None;
                    };
                    if r.start < range.start || r.end > range.end {
                        return None;
                    }
                    if par_dels.iter().any(|d| d.start < r.end && r.start < d.end) {
                        return None;
                    }
                    let label = fn_numbers
                        .get(id)
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| id.clone());
                    let label = SharedString::from(label);
                    let run = TextRun {
                        len: label.len(),
                        font: gpui::font("PT Serif"),
                        color: rgb(LINK_COLOR).into(),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    // Shaped HERE, in prepaint: never shape during paint.
                    let shaped = window.text_system().shape_line(
                        label,
                        bstyle.size * 0.65,
                        &[run],
                        None,
                    );
                    Some((r.start - range.start, shaped))
                })
                .collect();
            let marker: Option<gpui::ShapedLine> = marker.map(|m| {
                let run = TextRun {
                    len: m.len(),
                    font: gpui::font("PT Serif"),
                    color: rgb(MUTED_COLOR).into(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                window.text_system().shape_line(m, px(16.), &[run], None)
            });
            let runs = runs_for_paragraph(
                &range,
                &selection,
                marked.as_ref(),
                &par_spans,
                &par_notes,
                &par_matches,
                &par_dels,
                &block_base,
            );
            // Shaping feeds runs as byte windows: a run boundary off a char
            // boundary or a length mismatch silently corrupts glyphs.
            {
                let mut at = 0usize;
                let mut valid = true;
                for run in &runs {
                    if !par_text.is_char_boundary(at) {
                        valid = false;
                    }
                    at += run.len;
                }
                if at != par_text.len() || !valid {
                    eprintln!(
                        "strop-bug: run misalignment in block {block_ix}: runs sum {at} vs len {}, boundary-ok {valid}",
                        par_text.len()
                    );
                }
            }
            // Reuse the previous frame's shaped line when this block's text,
            // runs and metrics are unchanged (per-block layout reuse, matched by
            // index — a split/merge shifts indices and re-shapes from the edit
            // down). (text, runs, size, indent) is exactly the shape key, so a
            // match is byte-identical. `WrappedLine` isn't `Clone`, so the
            // matching slot is moved out of `prev`.
            let reused = match prev.as_mut().and_then(|v| v.get_mut(block_ix)) {
                Some(slot)
                    if slot.as_ref().is_some_and(|p| {
                        p.font_size == bstyle.size
                            && p.indent == bstyle.indent
                            && p.line.text.as_ref() == par_text
                            && p.runs == runs
                    }) =>
                {
                    slot.take()
                }
                _ => None,
            };
            let (line, boundaries) = if let Some(p) = reused {
                (p.line, p.boundaries)
            } else {
                let line = window
                    .text_system()
                    .shape_text(
                        SharedString::from(par_text.to_owned()),
                        bstyle.size,
                        &runs,
                        Some(wrap_width - bstyle.indent),
                        None,
                    )
                    .expect("shape_text failed")
                    .into_iter()
                    .next()
                    .expect("shape_text returned no lines");
                let boundaries: Vec<usize> = line
                    .wrap_boundaries()
                    .iter()
                    .map(|b| {
                        let run = &line.unwrapped_layout.runs[b.run_ix];
                        run.glyphs[b.glyph_ix].index
                    })
                    .collect();
                (line, boundaries)
            };
            top += bstyle.extra_top;
            // Breathing room above the Footnotes section rule (H4).
            if section_rule {
                top += px(24.);
            }
            let mut height = line.size(bstyle.line_height).height;
            let image = image_handles[block_ix].clone().and_then(|handle| {
                let render = handle.use_render_image(window, cx)?;
                let dev = render.size(0);
                // Crisp at this DPI, capped to the column width.
                let natural_w = dev.width.0 as f32 / scale;
                let natural_h = dev.height.0 as f32 / scale;
                let w = natural_w.min(f32::from(wrap_width));
                let h = natural_h * (w / natural_w);
                Some((render, gpui::size(px(w), px(h))))
            });
            if matches!(kind, BlockKind::Image { .. }) {
                height = image
                    .as_ref()
                    .map(|(_, sz)| sz.height)
                    .unwrap_or(px(56.)); // decoding placeholder
            }
            paragraphs.push(ParagraphLayout {
                line,
                range: range.clone(),
                boundaries,
                top,
                height,
                line_height: bstyle.line_height,
                indent: bstyle.indent,
                bg: bstyle.bg,
                quote_rule: bstyle.quote_rule,
                section_rule,
                compost_rule,
                compost_tail: is_boundary,
                compost_header,
                compost_flash,
                marker,
                fn_marks,
                font_size: bstyle.size,
                image,
                runs,
            });
            top += height + paragraph_gap;
            offset = range.end + 1; // step over '\n'
        }

        // `top` has accumulated one trailing gap past the last paragraph.
        let manuscript_bottom = top - paragraph_gap;
        // The graveyard tail section (Bug B) lives in the scroll flow, after the
        // last block — so `content_height` (and thus max_scroll) grows to reach
        // it. `grave_section_top` records the header's doc-space y for the
        // footer bar's hide-when-visible gate.
        let (grave_lines, grave_section_top, content_height) = if grave_render.is_empty() {
            (Vec::new(), None, manuscript_bottom)
        } else {
            let section_top = manuscript_bottom + px(GRAVE_SECTION_GAP);
            let (lines, bottom) =
                shape_grave_section(window, &grave_render, wrap_width, section_top);
            (lines, Some(section_top), bottom)
        };
        let max_scroll = (content_height + line_height - viewport).max(px(0.));
        scroll_top = scroll_top.clamp(px(0.), max_scroll);

        // Cursor position in document space (needed for autoscroll + quad).
        let cursor_pos = paragraphs
            .iter()
            .find(|p| cursor_offset <= p.range.end)
            .map(|par| {
                let (line, x) = par.position(cursor_offset - par.range.start, cursor_affinity);
                (
                    point(x, par.top + par.line_height * (line as f32)),
                    par.line_height,
                )
            });

        if autoscroll && let Some((pos, cursor_lh)) = cursor_pos {
            if pos.y < scroll_top {
                scroll_top = pos.y;
            } else if pos.y + cursor_lh > scroll_top + viewport {
                scroll_top = pos.y + cursor_lh - viewport;
            }
        }

        // Write the clamped/adjusted scroll back; no notify needed, we're
        // mid-frame and painting with this exact value.
        self.editor.update_in_draw(cx, |editor| {
            editor.scroll_top = scroll_top;
            editor.autoscroll_request = false;
        });

        let cursor = cursor_pos.and_then(|(pos, cursor_lh)| {
            let y = pos.y - scroll_top;
            if !cursor_blink_visible || y + cursor_lh <= px(0.) || y >= viewport {
                return None;
            }
            Some(fill(
                Bounds::new(bounds.origin + point(pos.x, y), size(px(2.), cursor_lh)),
                rgb(TEXT_COLOR),
            ))
        });

        if let Some(start) = perf_start {
            eprintln!(
                "strop-perf: prepaint {:?} ({} blocks)",
                start.elapsed(),
                paragraphs.len()
            );
        }

        PrepaintState {
            paragraphs,
            grave_lines,
            grave_section_top,
            cursor,
            line_height,
            scroll_top,
            content_height,
            layout_key,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let _guard = DrawGuard::enter();
        let (focus_handle, para_flash, grave_flashing) = {
            let ed = self.editor.read(cx);
            (
                ed.focus_handle.clone(),
                ed.para_flash.map(|(c, _)| ed.doc.char_to_byte(c.min(ed.doc.rope().len_chars()))),
                ed.grave_flash.is_some(),
            )
        };
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );

        let line_height = prepaint.line_height;
        let scroll_top = prepaint.scroll_top;
        let viewport = bounds.size.height;
        // The compost mini-column measure (asides.md §1: ~35ch, capped so it
        // never runs the full prose width).
        let compost_w = px(f32::from(bounds.size.width).min(340.));
        for par in &prepaint.paragraphs {
            let y = par.top - scroll_top;
            if y + par.height <= px(0.) || y >= viewport {
                continue; // outside the viewport
            }
            // Kind decorations: code-block panel, quote rule, list markers.
            if let Some(bg) = par.bg {
                window.paint_quad(fill(
                    Bounds::new(
                        bounds.origin + point(px(-8.), y - px(6.)),
                        size(bounds.size.width + px(16.), par.height + px(12.)),
                    ),
                    bg,
                ));
            }
            if par.quote_rule {
                window.paint_quad(fill(
                    Bounds::new(
                        bounds.origin + point(px(10.), y),
                        size(px(3.), par.height),
                    ),
                    rgb(RULE_COLOR),
                ));
            }
            // The Footnotes section rule (H4): a hairline across the column,
            // floated in the gap above the first definition block.
            if par.section_rule {
                window.paint_quad(fill(
                    Bounds::new(
                        bounds.origin + point(px(0.), y - px(13.)),
                        size(bounds.size.width, px(1.)),
                    ),
                    rgb(RULE_COLOR),
                ));
            }
            // Compost decorations (Bug A). The arrival blink first (behind the
            // text): the tail item's background pulses on an aside/migration.
            if par.compost_flash {
                window.paint_quad(fill(
                    Bounds::new(
                        bounds.origin + point(px(-6.), y - px(3.)),
                        size(compost_w + px(12.), par.height + px(6.)),
                    ),
                    rgba(COMPOST_FLASH),
                ));
            }
            // The "COMPOST" whisper above the first compost block.
            if let Some(shaped) = &par.compost_header {
                shaped
                    .paint(
                        bounds.origin + point(px(0.), y - px(16.)),
                        px(14.),
                        TextAlign::Left,
                        None,
                        window,
                        cx,
                    )
                    .ok();
            }
            // An empty compost/boundary block is a separator: a hairline at its
            // midline (asides.md §1). The boundary also gets the tail anchor bar.
            if par.compost_rule {
                let mid = y + par.height / 2.;
                window.paint_quad(fill(
                    Bounds::new(bounds.origin + point(px(0.), mid), size(compost_w, px(1.))),
                    rgb(RULE_COLOR),
                ));
                if par.compost_tail {
                    window.paint_quad(fill(
                        Bounds::new(
                            bounds.origin + point(px(0.), mid - px(1.)),
                            size(px(26.), px(2.)),
                        ),
                        rgb(COMPOST_TAIL),
                    ));
                }
            }
            // The originflash / put-back paragraph blink (Bug B): tint the block
            // the caret was sent to. Painted live from `para_flash`, so it clears
            // next frame without a layout rebuild.
            if para_flash.is_some_and(|b| par.range.start <= b && b <= par.range.end) {
                window.paint_quad(fill(
                    Bounds::new(
                        bounds.origin + point(px(-6.), y - px(3.)),
                        size(bounds.size.width + px(12.), par.height + px(6.)),
                    ),
                    rgba(SELECTION_COLOR),
                ));
            }
            let origin = bounds.origin + point(par.indent, y);
            if let Some((render, sz)) = &par.image
                && let Err(e) = window.paint_image(
                    Bounds::new(origin, *sz),
                    Corners::default(),
                    render.clone(),
                    0,
                    false,
                )
            {
                eprintln!("strop: paint image: {e}");
            }
            if let Some(shaped) = &par.marker {
                shaped
                    .paint(
                        bounds.origin + point(par.indent - px(24.), y + px(2.)),
                        par.line_height,
                        TextAlign::Left,
                        None,
                        window,
                        cx,
                    )
                    .ok();
            }
            par.line
                .paint_background(origin, par.line_height, TextAlign::Left, None, window, cx)
                .expect("paint_background failed");
            par.line
                .paint(origin, par.line_height, TextAlign::Left, None, window, cx)
                .expect("paint failed");
            // Footnote refs as painted superior figures (DESIGN
            // §2-footnotes): ~65% of the block size, baseline raised ~35%
            // of the font size so the top lands near cap height, accent
            // ink — size signals "footnote", color "interactive". The
            // transparent carrier under it keeps the advance.
            for (local, shaped) in &par.fn_marks {
                let line_ix = par.line_of(*local, true);
                let x = par.x_for(*local, line_ix);
                // paint() centers ascent+descent inside line_height;
                // cancel that and put the small baseline 35% of the font
                // size above the body baseline.
                let body = &par.line.unwrapped_layout;
                let body_baseline =
                    (par.line_height - body.ascent - body.descent) / 2. + body.ascent;
                let small_baseline =
                    (par.line_height - shaped.ascent - shaped.descent) / 2. + shaped.ascent;
                let dy = body_baseline - small_baseline - par.font_size * 0.35;
                let line_y = y + par.line_height * (line_ix as f32);
                shaped
                    .paint(
                        bounds.origin + point(x, line_y + dy),
                        par.line_height,
                        TextAlign::Left,
                        None,
                        window,
                        cx,
                    )
                    .ok();
            }
        }

        // The graveyard tail section (Bug B): drawn after the last block, in the
        // scroll flow. Read-only — dimmed cut text, a section header, the verbs.
        for gl in &prepaint.grave_lines {
            let y = gl.top - scroll_top;
            if y + gl.height <= px(0.) || y >= viewport {
                continue; // culled
            }
            if gl.header {
                // The tombstone slab + a flash tint on a fresh cut (live).
                if grave_flashing {
                    window.paint_quad(fill(
                        Bounds::new(
                            bounds.origin + point(px(-6.), y - px(3.)),
                            size(bounds.size.width + px(12.), gl.height + px(6.)),
                        ),
                        rgba(COMPOST_FLASH),
                    ));
                }
                // The tombstone slab (a drawn glyph — ⚰ is outside PT). A narrow
                // "neck" quad over a wider "base" reads as a headstone.
                window.paint_quad(fill(
                    Bounds::new(
                        bounds.origin + point(px(3.), y + px(5.)),
                        size(px(7.), px(3.)),
                    ),
                    rgb(MUTED_COLOR),
                ));
                window.paint_quad(fill(
                    Bounds::new(
                        bounds.origin + point(px(2.), y + px(7.)),
                        size(px(9.), px(8.)),
                    ),
                    rgb(MUTED_COLOR),
                ));
                // A hairline above the whole section (the "record" boundary).
                window.paint_quad(fill(
                    Bounds::new(
                        bounds.origin + point(px(0.), y - px(20.)),
                        size(bounds.size.width, px(1.)),
                    ),
                    rgb(RULE_COLOR),
                ));
            }
            if gl.left_rule {
                window.paint_quad(fill(
                    Bounds::new(bounds.origin + point(px(0.), y), size(px(3.), gl.height)),
                    rgb(STALE_BG),
                ));
            }
            for (x, shaped) in &gl.segments {
                shaped
                    .paint(bounds.origin + point(*x, y), gl.line_height, TextAlign::Left, None, window, cx)
                    .ok();
            }
            if let Some((x, wrapped)) = &gl.body {
                wrapped
                    .paint(bounds.origin + point(*x, y), gl.line_height, TextAlign::Left, None, window, cx)
                    .ok();
            }
        }

        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }

        let paragraphs = std::mem::take(&mut prepaint.paragraphs);
        let grave_lines = std::mem::take(&mut prepaint.grave_lines);
        let grave_section_top = prepaint.grave_section_top;
        let content_height = prepaint.content_height;
        let layout_key = prepaint.layout_key.clone();
        // Overlays (margin lane, AI card/idle hint, selection popover)
        // position themselves from `last_frame` — the PREVIOUS paint's
        // geometry. When this paint's geometry differs (window resize,
        // output scale change, panel reflow), they just rendered against
        // stale numbers and nothing else would schedule a repaint — the
        // wflip.sh plain-fixture failure: the idle hint stuck at the old
        // column edge after a scale flip. Request one follow-up frame;
        // the notify is deferred to after this draw (never mid-draw — the
        // 2026-06-12 corruption rule), and it converges: the next paint
        // sees identical geometry and schedules nothing.
        let geometry_changed = self
            .editor
            .read(cx)
            .last_frame
            .as_ref()
            .is_none_or(|f| f.bounds != bounds || f.scroll_top != scroll_top);
        if geometry_changed {
            window.request_animation_frame();
        }
        self.editor.update_in_draw(cx, |editor| {
            editor.last_frame = Some(TextFrame {
                bounds,
                line_height,
                scroll_top,
                content_height,
                paragraphs,
                grave_lines,
                grave_section_top,
                layout_key,
            });
        });
    }
}

impl Editor {
    /// A mark button for the selection popover. The label demonstrates its
    /// own mark — B is bold, I italic, S struck, {} mono, == a highlit chip
    /// (H3) — so the toolbar teaches what it does without words.
    fn format_button(
        &self,
        label: &'static str,
        attr: InlineAttr,
        tip_label: &'static str,
        chord: Option<&'static str>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.attr_active(&attr);
        let label_el = match &attr {
            InlineAttr::Strong => div()
                .font_weight(FontWeight::BOLD)
                .child(label)
                .into_any_element(),
            InlineAttr::Emphasis => div().italic().child(label).into_any_element(),
            InlineAttr::Strikethrough => div().line_through().child(label).into_any_element(),
            InlineAttr::Code => div()
                .font_family(CODE_FONT)
                .text_size(px(12.))
                .child(label)
                .into_any_element(),
            // HIGHLIGHT_COLOR carries an alpha byte (it's an rgba constant);
            // rgb() would drop it and render pink. Match how the document
            // paints a highlight: translucent amber over the surface.
            InlineAttr::Highlight => div()
                .bg(rgba(HIGHLIGHT_COLOR))
                .px(px(3.))
                .rounded(px(3.))
                .text_color(rgb(TEXT_COLOR))
                .child(label)
                .into_any_element(),
            _ => div().child(label).into_any_element(),
        };
        div()
            .id(label)
            .px(px(7.))
            .py(px(2.))
            .rounded(px(5.))
            .cursor(CursorStyle::PointingHand)
            .text_color(if active {
                rgb(TEXT_COLOR)
            } else {
                rgb(MUTED_COLOR)
            })
            .when(active, |d| d.bg(rgba(0x1A1A1812u32)))
            .hover(|d| d.bg(rgba(0x1A1A180Au32)))
            .tooltip(tip(tip_label, chord))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |editor, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    editor.toggle_span(attr.clone(), cx);
                }),
            )
            .child(label_el)
    }

    fn heading_button(
        &self,
        label: &'static str,
        level: u8,
        tip_label: &'static str,
        chord: Option<&'static str>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let kind = self.doc.blocks().kind(self.doc.block_of_byte(self.selected_range.start));
        let active = matches!(kind, BlockKind::Heading(l) if *l == level);
        div()
            .id(label)
            .px(px(7.))
            .py(px(2.))
            .rounded(px(5.))
            .cursor(CursorStyle::PointingHand)
            .text_color(if active {
                rgb(TEXT_COLOR)
            } else {
                rgb(MUTED_COLOR)
            })
            .when(active, |d| d.bg(rgba(0x1A1A1812u32)))
            .hover(|d| d.bg(rgba(0x1A1A180Au32)))
            .tooltip(tip(tip_label, chord))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |editor, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    editor.toggle_block(BlockKind::Heading(level), cx);
                }),
            )
            .child(label)
    }

    /// The footnote button (H3): a hand-drawn superior "1" (the literal ¹
    /// U+00B9 isn't guaranteed in PT, and a fallback-font glyph is the
    /// corruption class we fixed). Inserts a footnote at the caret.
    fn footnote_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("fn-mark")
            .px(px(7.))
            .py(px(2.))
            .rounded(px(5.))
            .cursor(CursorStyle::PointingHand)
            .text_color(rgb(MUTED_COLOR))
            .hover(|d| d.bg(rgba(0x1A1A180Au32)))
            .tooltip(tip("Footnote", Some("ctrl-alt-f")))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    editor.insert_footnote(&InsertFootnote, window, cx);
                }),
            )
            .child(
                div()
                    .flex()
                    .items_start()
                    .h(px(14.))
                    .child(div().text_size(px(9.)).child("1")),
            )
    }

    /// The link cell (docs/impl/03-flanks.md §0.1): the second argument-taker.
    /// A link has no PT-covered glyph (⤴ would force a font fallback — the
    /// garbled-glyph bug class), so it draws its own mark: a short LINK_COLOR
    /// underline bar (blue = link, per the color language). Clicking opens the
    /// URL argument-field (`open_link_input`) rather than toggling — a link needs
    /// a target — and the cell lights when the selection already carries one.
    fn link_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let active = if self.selected_range.is_empty() {
            false
        } else {
            let rope = self.doc.rope();
            let range = rope.byte_to_char(self.selected_range.start)
                ..rope.byte_to_char(self.selected_range.end);
            self.doc.link_over(range).is_some()
        };
        div()
            .id("link-mark")
            .px(px(7.))
            .py(px(2.))
            .rounded(px(5.))
            .cursor(CursorStyle::PointingHand)
            .when(active, |d| d.bg(rgba(0x1A1A1812u32)))
            .hover(|d| d.bg(rgba(0x1A1A180Au32)))
            .tooltip(tip("Link", None))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    editor.open_link_input(window, cx);
                }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(px(14.))
                    .child(div().w(px(13.)).h(px(2.)).rounded(px(1.)).bg(rgb(LINK_COLOR))),
            )
    }

    /// The FLATTENED formatting set for the narrow horizontal fallback (review
    /// H21): the whole closed grid on one row — inline marks, the two
    /// argument-takers (link, footnote), the heading ladder — grouped by
    /// dividers. The VERBS never join it; they stay palette-reachable. This is
    /// formatting only, so a window too narrow to float a grid still formats.
    fn format_tools(&self, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(1.))
            .child(self.format_button("B", InlineAttr::Strong, "Bold", Some("ctrl-b"), cx))
            .child(self.format_button("I", InlineAttr::Emphasis, "Italic", Some("ctrl-i"), cx))
            .child(self.format_button("U", InlineAttr::Underline, "Underline", Some("ctrl-u"), cx))
            .child(self.format_button(
                "S",
                InlineAttr::Strikethrough,
                "Strikethrough",
                Some("ctrl-shift-x"),
                cx,
            ))
            .child(self.format_button(
                "==",
                InlineAttr::Highlight,
                "Highlight",
                Some("ctrl-shift-h"),
                cx,
            ))
            .child(self.format_button("{}", InlineAttr::Code, "Code", Some("ctrl-e"), cx))
            .child(popover_divider())
            // The two argument-takers.
            .child(self.link_button(cx))
            .child(self.footnote_button(cx))
            .child(popover_divider())
            // The heading ladder.
            .child(self.heading_button("H1", 1, "Heading 1", Some("ctrl-1"), cx))
            .child(self.heading_button("H2", 2, "Heading 2", Some("ctrl-2"), cx))
            .child(self.heading_button("H3", 3, "Heading 3", Some("ctrl-3"), cx))
    }

    /// The closed-set GRID for the gutter-float (docs/impl/03-flanks.md §0.1,
    /// asides.md §4: "closed sets pack into grids"). The eight inline attrs in
    /// two columns — six instant toggles, a hairline SEAM, then the two
    /// argument-takers (link, footnote) below it: parallel form reserved for
    /// parallel behaviour (P8). A SECOND seam separates the heading block row.
    /// Two columns halve pointer travel versus the old 1×N stack (Fitts).
    fn format_grid(&self, cx: &mut Context<Self>) -> gpui::Div {
        let seam = || div().h(px(1.)).w_full().my(px(3.)).bg(rgb(RULE_COLOR));
        // Fixed cells so the two columns align regardless of glyph width.
        let cell = |el: gpui::AnyElement| {
            div()
                .w(px(30.))
                .flex()
                .items_center()
                .justify_center()
                .child(el)
        };
        let row = |a: gpui::AnyElement, b: gpui::AnyElement| {
            div()
                .flex()
                .items_center()
                .gap(px(2.))
                .child(cell(a))
                .child(cell(b))
        };
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(2.))
            .child(row(
                self.format_button("B", InlineAttr::Strong, "Bold", Some("ctrl-b"), cx)
                    .into_any_element(),
                self.format_button("I", InlineAttr::Emphasis, "Italic", Some("ctrl-i"), cx)
                    .into_any_element(),
            ))
            .child(row(
                self.format_button("U", InlineAttr::Underline, "Underline", Some("ctrl-u"), cx)
                    .into_any_element(),
                self.format_button(
                    "S",
                    InlineAttr::Strikethrough,
                    "Strikethrough",
                    Some("ctrl-shift-x"),
                    cx,
                )
                .into_any_element(),
            ))
            .child(row(
                self.format_button("==", InlineAttr::Highlight, "Highlight", Some("ctrl-shift-h"), cx)
                    .into_any_element(),
                self.format_button("{}", InlineAttr::Code, "Code", Some("ctrl-e"), cx)
                    .into_any_element(),
            ))
            .child(seam())
            // The argument-takers, below the first seam.
            .child(row(
                self.link_button(cx).into_any_element(),
                self.footnote_button(cx).into_any_element(),
            ))
            .child(seam())
            // The heading block row: the full ladder on one line under the seam.
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.))
                    .child(cell(
                        self.heading_button("H1", 1, "Heading 1", Some("ctrl-1"), cx)
                            .into_any_element(),
                    ))
                    .child(cell(
                        self.heading_button("H2", 2, "Heading 2", Some("ctrl-2"), cx)
                            .into_any_element(),
                    ))
                    .child(cell(
                        self.heading_button("H3", 3, "Heading 3", Some("ctrl-3"), cx)
                            .into_any_element(),
                    )),
            )
    }

    /// The per-frame flank geometry + gating (docs/impl/03-flanks.md §1-2).
    /// `None` when there's no live selection to flank. Computed ONCE so the left
    /// popover, the right menu, and the rig dump all agree on presence and y.
    fn flank_layout(&self, window: &Window) -> Option<FlankLayout> {
        if !self.selection_popover || self.selected_range.is_empty() || self.is_selecting {
            return None;
        }
        let frame = self.last_frame.as_ref()?;
        let (par_ix, line, x) =
            frame.cursor_position(self.selected_range.start.min(frame.doc_len()), false)?;
        let par = &frame.paragraphs[par_ix];
        // CSD insets (the old popover math): the `content` surface the flanks are
        // children of is inset by the shadow gutter on each untiled edge
        // (Wayland floating windows; server-decorated platforms and tiled edges
        // inset 0). Content space = window space minus that inset.
        let (l_inset, t_inset, b_inset) = match window.window_decorations() {
            Decorations::Client { tiling } => (
                if tiling.left { 0. } else { CSD_GUTTER },
                if tiling.top { 0. } else { CSD_GUTTER },
                if tiling.bottom { 0. } else { CSD_GUTTER },
            ),
            Decorations::Server => (0., 0., 0.),
        };
        // RAW (lane) y of the selection's first visual line — co-registers with
        // the margin CARDS (which also skip the inset: `margin_cards` uses the
        // bare `frame.bounds.origin.y + pos.y - scroll_top`). The LEFT flank
        // subtracts the top inset to co-register with the prose column instead;
        // sharing one basis would drift the menu a gutter off the cards it
        // occludes on floating windows (review finding 89).
        let raw_top = f32::from(frame.bounds.origin.y) + f32::from(par.top)
            + f32::from(par.line_height) * line as f32
            - f32::from(frame.scroll_top);
        let col_left = f32::from(frame.bounds.origin.x) - l_inset;
        let rail_w = self.rail_width(window);
        let left_gutter = col_left - rail_w;
        // A history surface (strip open, the panel, or a parked preview) claims
        // the right side AND freezes a read-only past — suppress both flanks so
        // neither offers live mutation over it (review H22).
        let history_up =
            self.history_view.is_some() || self.strip.open || self.history_preview.is_some();
        // A compost-rail selection: the manuscript-only verbs don't apply, so the
        // right menu stands down (spec §1, finding 108); the left flank still
        // rises (the rail is the writer's text, and it renders as leading
        // paragraphs in the SAME frame, so its coordinates are in hand).
        let in_compost = self.doc.aside_boundary().is_some()
            && self
                .doc
                .rope()
                .byte_to_char(self.selected_range.start.min(self.doc.len_bytes()))
                < self.doc.manuscript_base_char();
        let lane_available = self.margin_fits(window);
        let gutter_ok = left_gutter >= FLANK_GRID_W + 12.;
        Some(FlankLayout {
            gate: flank_gate(history_up, in_compost, lane_available, gutter_ok),
            left_top: raw_top - t_inset,
            right_top: raw_top,
            line_h: f32::from(par.line_height),
            x: f32::from(x),
            col_left,
            vw: self.content_width(window),
            vh: f32::from(window.viewport_size().height) - t_inset - b_inset,
            rail_w,
            lane_left: self.column_right(window) + MARGIN_GAP,
        })
    }

    /// The LEFT flank (docs/impl/03-flanks.md §0.1). Formatting rides the
    /// selection (DESIGN §2-toolbar) — but "the text I'm writing is sacred", so
    /// the closed-set grid floats VERTICALLY in the empty left gutter, never over
    /// the words. The link argument-field, when open, OWNS the flank; where there
    /// is no gutter to hold a grid it falls back to the formatting-only
    /// horizontal popover above the line.
    fn render_selection_popover(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Div> {
        let layout = self.flank_layout(window)?;
        // The link argument-field OWNS the flank while open (spec §0.1): a wide
        // URL field at the selection line — the grid is far too narrow to type a
        // URL into. The field holds keyboard focus, so "typing dismisses" (an
        // EDITOR-focus rule) never fires over the URL; Enter commits, Esc cancels.
        if let Some((_, input)) = &self.link_input {
            return Some(self.render_link_field(&layout, input.clone()));
        }
        match layout.gate.left {
            FlankLeft::None => None,
            FlankLeft::Grid => Some(self.render_flank_grid(&layout, cx)),
            FlankLeft::Horizontal => Some(self.render_flank_horizontal(&layout, cx)),
        }
    }

    /// The gutter-float grid (the resting left flank).
    fn render_flank_grid(&self, layout: &FlankLayout, cx: &mut Context<Self>) -> gpui::Div {
        let top = layout
            .left_top
            .clamp(BAR_HEIGHT + 8., (layout.vh - FLANK_GRID_H - 8.).max(BAR_HEIGHT + 8.));
        let left = (layout.col_left - 12. - FLANK_GRID_W).max(layout.rail_w + 6.);
        div()
            .absolute()
            .left(px(left))
            .top(px(top))
            .w(px(FLANK_GRID_W))
            .bg(rgb(0xFCFAF4))
            .border_1()
            .border_color(rgb(RULE_COLOR))
            .rounded(px(8.))
            .shadow_md()
            .py(px(5.))
            .px(px(3.))
            .flex()
            .flex_col()
            .items_center()
            .font_family("PT Serif")
            .text_size(px(13.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(self.format_grid(cx))
    }

    /// The narrow fallback: the FORMATTING-ONLY horizontal popover above the
    /// selection line (review H21 — the verbs stay palette-reachable, they do
    /// NOT crowd this row).
    fn render_flank_horizontal(&self, layout: &FlankLayout, cx: &mut Context<Self>) -> gpui::Div {
        const POPOVER_W: f32 = 340.;
        const POPOVER_H: f32 = 30.;
        let left = (layout.col_left + layout.x - POPOVER_W / 2.)
            .clamp(8., (layout.vw - POPOVER_W - 8.).max(8.));
        let above = layout.left_top - POPOVER_H - 8.;
        let top = if above >= BAR_HEIGHT + 4. {
            above
        } else {
            layout.left_top + layout.line_h + 8.
        }
        .clamp(BAR_HEIGHT + 4., layout.vh - POPOVER_H - 8.);
        div()
            .absolute()
            .left(px(left))
            .top(px(top))
            .w(px(POPOVER_W))
            .bg(rgb(0xFCFAF4))
            .border_1()
            .border_color(rgb(RULE_COLOR))
            .rounded(px(6.))
            .shadow_md()
            .px(px(4.))
            .py(px(3.))
            .flex()
            .items_center()
            .justify_center()
            .font_family("PT Serif")
            .text_size(px(13.))
            // Clicks on the popover chrome must not reach the canvas — they would
            // collapse the very selection being formatted.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(self.format_tools(cx))
    }

    /// The link argument-field popover (spec §0.1): a single-line URL editor at
    /// the selection line, the rename-input idiom. It stops propagation on its
    /// own chrome so a click inside never collapses the selection the link needs.
    fn render_link_field(&self, layout: &FlankLayout, input: Entity<TextField>) -> gpui::Div {
        const LINK_W: f32 = 320.;
        const LINK_H: f32 = 34.;
        let left = (layout.col_left + layout.x - LINK_W / 2.)
            .clamp(8., (layout.vw - LINK_W - 8.).max(8.));
        let above = layout.left_top - LINK_H - 8.;
        let top = if above >= BAR_HEIGHT + 4. {
            above
        } else {
            layout.left_top + layout.line_h + 8.
        }
        .clamp(BAR_HEIGHT + 4., layout.vh - LINK_H - 8.);
        div()
            .absolute()
            .left(px(left))
            .top(px(top))
            .w(px(LINK_W))
            .bg(rgb(0xFCFAF4))
            .border_1()
            .border_color(rgb(RULE_COLOR))
            .rounded(px(6.))
            .shadow_md()
            .px(px(8.))
            .py(px(4.))
            .flex()
            .items_center()
            .gap(px(8.))
            .font_family("PT Sans")
            .text_size(px(12.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(div().text_color(rgb(MUTED_COLOR)).child("Link"))
            .child(div().flex_1().child(input))
            .child(
                div()
                    .text_color(rgb(MUTED_COLOR))
                    .text_size(px(10.))
            )
    }

    /// The RIGHT flank — the selection's verb menu (docs/impl/03-flanks.md §0.2).
    /// An INDEPENDENT overlay that OCCLUDES cards at its y (review B8: it is
    /// transient, so it never touches the packer — no re-pack, so the lane never
    /// slides on a select/deselect, finding 61). Rows are carrier sentences,
    /// MOUSE-ONLY (reviews B2/B9: no letter caps — bare keys would type over the
    /// live selection); each acts and dismisses BOTH flanks first (H20). Idle at
    /// α≈0.72, waking on hover (the lab selmenu idiom). Presence is decided in
    /// `flank_layout` (compost-rail and history both stand it down).
    fn render_selection_menu(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let layout = self.flank_layout(window)?;
        if !layout.gate.right {
            return None;
        }
        // Never cover the lane-top furniture the writer needs (the door rail /
        // ai_status band): clamp below it when present (finding 60), else just
        // below the titlebar — the same floor the cards clear.
        let has_top_furniture = self.margin_rail_count().is_some() || self.ai_status.is_some();
        let floor = BAR_HEIGHT + 8. + if has_top_furniture { 30. } else { 0. };
        const MENU_H_EST: f32 = 132.; // four carrier rows
        let top = layout
            .right_top
            .clamp(floor, (layout.vh - MENU_H_EST - 8.).max(floor));
        Some(
            div()
                .absolute()
                .left(px(layout.lane_left))
                .top_0()
                .bottom_0()
                .w(px(MARGIN_WIDTH))
                // No wheel handler — the one document scroll owns the lane; a
                // handler here would double-fire (mirrors render_margin).
                .child(
                    div()
                        .absolute()
                        .top(px(top))
                        // Borrow half the margin gap: the chord chips need the
                        // room, and the extra 8px stays off the prose.
                        .left(px(0.))
                        .w(px(MARGIN_WIDTH))
                        // Rows stack top-down (GPUI defaults to flex-row): the
                        // carrier sentences are a column, one verb per line.
                        .flex()
                        .flex_col()
                        .bg(rgb(CARD_BG))
                        .border_1()
                        .border_color(rgb(RULE_COLOR))
                        .rounded(px(9.))
                        .shadow_md()
                        // Fully opaque: the menu shares the lane with margin
                        // cards, and the old resting translucency (0.72) let a
                        // card bleed through the rows — an unreadable double
                        // exposure whenever the two overlapped.
                        .font_family("PT Sans")
                        .text_size(px(12.5))
                        .py(px(2.))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(self.selection_menu_row("sel-note", SelVerb::Note, "Add a note", cx))
                        .child(self.selection_menu_row(
                            "sel-aside",
                            SelVerb::Aside,
                            "Set aside, out of the story",
                            cx,
                        ))
                        .child(self.selection_menu_row(
                            "sel-grave",
                            SelVerb::Graveyard,
                            "Send to the graveyard",
                            cx,
                        ))
                        .child(self.selection_menu_row(
                            "sel-ask",
                            SelVerb::Ask,
                            "Ask the editor about this…",
                            cx,
                        )),
                ),
        )
    }

    /// One carrier-sentence row of the right verb menu. The icon is a small
    /// color-language dot drawn as a div (no non-PT glyph — the ☰/✂/❋ of the
    /// mockup would force a font fallback, and ☰ collides with outline/palette,
    /// finding 109): warm amber for the writer's note, drained for the graveyard,
    /// cool blue for the machine ask. Acting dismisses BOTH flanks first (H20),
    /// THEN dispatches — the verb still sees the live `selected_range` (dismissal
    /// only hides the flanks, it never clears the selection).
    fn selection_menu_row(
        &self,
        id: &'static str,
        verb: SelVerb,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Each verb's dress derives from the verb itself: the colour-language
        // dot (warm = the writer's note, drained = the graveyard, cool = the
        // machine ask), the chord it teaches, and whether the hover wash warns.
        let (dot, chord, destructive) = match verb {
            SelVerb::Note => (rgb(ACTIVE_BORDER), "ctrl-m", false),
            SelVerb::Aside => (rgb(MUTED_COLOR), "ctrl-shift-a", false),
            SelVerb::Graveyard => (rgb(0xC9C5BAu32), "ctrl-shift-g", true),
            SelVerb::Ask => (rgb(AI_ACCENT), "ctrl-shift-d", false),
        };
        div()
            .id(id)
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(11.))
            .py(px(7.))
            .cursor(CursorStyle::PointingHand)
            .hover(|d| d.bg(rgb(if destructive { STALE_BG } else { NOTE_CARD_BG })))
            .child(div().w(px(7.)).h(px(7.)).rounded_full().bg(dot))
            .child(div().text_color(rgb(TEXT_COLOR)).child(label))
            // The chord chip (the lab's `.k` keycap): the menu teaches its own
            // shortcuts in place, instead of a manual teaching them in prose.
            .child(
                div()
                    .ml_auto()
                    .flex_shrink_0()
                    .px(px(3.))
                    .font_family(CODE_FONT)
                    .text_size(px(9.))
                    .text_color(rgb(MUTED_COLOR))
                    .border_1()
                    .border_color(rgb(RULE_COLOR))
                    .rounded(px(4.))
                    .bg(rgb(BG_COLOR))
                    .child(chord),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |editor, _: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    // H20: both flanks down before the result (a composer card, a
                    // pass, an aside) takes the lane — one pinned object at that y.
                    editor.dismiss_flanks(cx);
                    match verb {
                        SelVerb::Note => editor.add_note(&AddNote, window, cx),
                        SelVerb::Aside => editor.set_aside(&SetAside, window, cx),
                        SelVerb::Graveyard => {
                            editor.send_to_graveyard(&SendToGraveyard, window, cx)
                        }
                        // The scoped ask reuses ctrl-shift-d's path: `run_pass`
                        // already scopes over a non-empty selection, opens the
                        // door, and runs `effective_mode` (finding 118).
                        SelVerb::Ask => editor.run_diagnosis(&RunDiagnosis, window, cx),
                    }
                }),
            )
    }

    // UI chrome avoids glyphs outside the bundled PT fonts (arrows, circles,
    // checks): every such character forces a mid-session system-font fallback
    // load, the exact path behind the garbled-glyph bugs. Indicators that
    // have no PT-covered character are drawn as divs instead.
    fn window_button(
        &self,
        label: &'static str,
        tip_label: &'static str,
        chord: Option<&'static str>,
        action: fn(&mut Window, &mut App),
    ) -> impl IntoElement {
        div()
            .id(label)
            // Clickable, not a drag handle: occlude so the Windows titlebar
            // hit-test resolves to this control rather than HTCAPTION.
            .occlude()
            .w(px(34.))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .text_color(rgb(MUTED_COLOR))
            .hover(|d| d.bg(rgba(0x1A1A180Au32)).text_color(rgb(TEXT_COLOR)))
            .tooltip(tip(tip_label, chord))
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                cx.stop_propagation();
                action(window, cx);
            })
            .child(label)
    }

    /// The one piece of chrome: title, word count, history, menu, window
    /// controls. Formatting lives in the selection popover (DESIGN
    /// §2-toolbar: zero category precedent for persistent format buttons).
    fn render_titlebar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Dragging the bar moves the window — by two mechanisms, because the
        // platforms split. `start_window_move()` drives Wayland/X11/macOS but
        // is a no-op on Windows, where the OS only moves a window whose
        // hit-test claims HTCAPTION — which is exactly what
        // `window_control_area(Drag)` makes gpui report. Both ride the whole
        // bar; interactive children opt out below (stop_propagation for the
        // move handler, and `occlude()` so the Windows hit-test resolves to the
        // child, not the caption — otherwise their clicks would start a drag).
        let drag =
            |_: &MouseDownEvent, window: &mut Window, _: &mut App| window.start_window_move();
        div()
            .h(px(BAR_HEIGHT))
            .w_full()
            .flex()
            .items_center()
            .window_control_area(WindowControlArea::Drag)
            .on_mouse_down(MouseButton::Left, drag)
            .border_b_1()
            .border_color(rgb(RULE_COLOR))
            .font_family("PT Serif")
            .text_size(px(13.))
            // Left third. This and the controls third below are symmetric
            // flex_1 spans, so the Search button between them stays truly
            // centred no matter how the editor button's label breathes
            // (the jumping-Search bug).
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .flex()
                    .items_center()
                // macOS draws its native traffic-light buttons over the top-left of
                // our (full-size-content) titlebar — recentred by `traffic_light_
                // position` in main.rs. Reserve their width so the outline toggle and
                // document name start to their right instead of underneath them.
                .when(cfg!(target_os = "macos"), |bar| {
                    bar.child(div().w(px(76.)).h_full())
                })
                // The compost rail opens on the LEFT, so its control lives
                // at the far left — spatial honesty (the H2 papercut: it was
                // on the right). Three stacked bars, drawn like every glyph.
                .child(
                    div()
                        .id("outline-toggle")
                        .occlude()
                        .px(px(8.))
                        .py(px(2.))
                        .ml(px(8.))
                        .rounded(px(5.))
                        .cursor(CursorStyle::PointingHand)
                        .when(self.rail_width(window) > 0., |d| d.bg(rgba(0x1A1A1812u32)))
                        // Arrival blink (S2, P12 — the control is the
                        // indicator): when a passage lands in a CLOSED rail,
                        // the always-visible toggle carries the compliance
                        // signal; an off-screen region flash can't.
                        .map(|d| {
                            let a = self.rail_flash.map_or(0., |t| {
                                let e = t.elapsed().as_millis() as f32 / 1400.;
                                (0.33 * (1. - e)).clamp(0., 0.33)
                            });
                            d.when(a > 0. && !self.rail_open, |d| d.bg(tint(0xC8A951, a)))
                        })
                        .hover(|d| d.bg(rgba(0x1A1A180Au32)))
                        .tooltip(tip("Compost", Some("ctrl-shift-o")))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|editor, _: &MouseDownEvent, window, cx| {
                                cx.stop_propagation();
                                editor.toggle_rail(&ToggleOutline, window, cx);
                            }),
                        )
                        .child({
                            // Honest light (extraction audit #7): the toggle
                            // reads active only when the rail actually shows —
                            // history or a narrow window can suppress it while
                            // rail_open stays latently true.
                            let bar_color = if self.rail_width(window) > 0. {
                                rgb(TEXT_COLOR)
                            } else {
                                rgb(MUTED_COLOR)
                            };
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .items_start()
                                        .gap(px(2.))
                                        .children(
                                            [11., 8., 5.]
                                                .into_iter()
                                                .map(|w| div().w(px(w)).h(px(1.5)).bg(bar_color)),
                                        ),
                                )
                                // Presence, not a count (H5; asides.md §5
                                // forbids gamified size): a non-empty compost
                                // leaves a lasting dot on its control — the
                                // residual trace a transient blink can't be.
                                .when(self.doc.aside_boundary().is_some(), |d| {
                                    d.child(
                                        div()
                                            .size(px(4.))
                                            .rounded_full()
                                            .bg(rgb(0xC9C5BAu32)),
                                    )
                                })
                        }),
                )
                // Document name — click or F2 to rename in place, file and all.
                .child(match (&self.doc_rename_input, &self.store) {
                    (Some(input), _) => div()
                        .occlude()
                        .ml(px(8.))
                        .w(px(220.))
                        .child(input.clone())
                        .into_any_element(),
                    (None, Some(store)) => {
                        let stem = store
                            .path()
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Untitled")
                            .to_owned();
                        div()
                            .id("doc-title")
                            .occlude()
                            .ml(px(8.))
                            .px(px(4.))
                            .rounded(px(4.))
                            .text_color(rgb(MUTED_COLOR))
                            .cursor(CursorStyle::PointingHand)
                            .hover(|d| d.bg(rgba(0x1A1A180Au32)))
                            .tooltip(tip("Rename", Some("f2")))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|editor, _: &MouseDownEvent, window, cx| {
                                    cx.stop_propagation();
                                    editor.rename_document(&RenameDocument, window, cx);
                                }),
                            )
                            .child(stem)
                            .into_any_element()
                    }
                    (None, None) => div()
                        .ml(px(8.))
                        .text_color(rgb(MUTED_COLOR))
                        .child("Strop")
                        .into_any_element(),
                })
                // Word count + the session goal's live delta (DESIGN §4.2).
                // Clickable: sets or changes the goal for this sitting.
                .child(
                    div()
                        .id("word-count")
                        .occlude()
                        .ml(px(12.))
                        .px(px(4.))
                        .rounded(px(4.))
                        .cursor(CursorStyle::PointingHand)
                        .hover(|d| d.bg(rgba(0x1A1A180Au32)))
                        .tooltip(tip("Set session goal", None))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|editor, _: &MouseDownEvent, window, cx| {
                                cx.stop_propagation();
                                editor.set_session_goal(&SetSessionGoal, window, cx);
                            }),
                        )
                        .child({
                            let count = format_thousands(self.word_count);
                            match self.session_goal {
                                None => div()
                                    .text_color(rgb(MUTED_COLOR))
                                    .child(format!("{count} words"))
                                    .into_any_element(),
                                Some((goal, start)) => {
                                    let delta = self.word_count as i64 - start as i64;
                                    let reached = delta >= goal as i64;
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(6.))
                                        .text_color(rgb(MUTED_COLOR))
                                        .child(format!("{count} words"))
                                        .child(if reached {
                                            // Goal met: the separator quietly fills
                                            // in sage. No banner (§4b tension 3).
                                            div()
                                                .size(px(5.))
                                                .rounded_full()
                                                .bg(rgb(SAGE_COLOR))
                                                .into_any_element()
                                        } else {
                                            div().child("·").into_any_element()
                                        })
                                        .child(format!("{delta:+}/{goal}"))
                                        .into_any_element()
                                }
                            }
                        }),
                )
            )
            // The omnibar (06 §1): the centre control IS the palette's input —
            // a real type-able field whose dropdown is the results card, never
            // a second input. At rest the same box holds the placeholder; the
            // first click swaps the live TextField in at identical geometry
            // and `find` focuses it, so the box types the moment it's touched.
            // The runway (fixed width, S4) is the affordance; the I-beam
            // confirms it. The space around it stays the window-drag handle.
            .child(match &self.palette_input {
                Some(input) => {
                    // The find-mode match counter rides right of the query —
                    // where the eye reads it (S7); the ranges are already
                    // computed for the live preview.
                    let counter = match omni_mode(&self.palette_query) {
                        (OmniMode::Find, rest) if !rest.trim().is_empty() => {
                            let n = self.omni_match_ranges().len();
                            Some(if n == 0 {
                                "0".to_owned()
                            } else {
                                format!("{}/{n}", self.palette_selected.min(n - 1) + 1)
                            })
                        }
                        _ => None,
                    };
                    div()
                        .id("omni-pill")
                        .occlude()
                        .flex_shrink_0()
                        .w(px(omni_field_width(window)))
                        .px(px(10.))
                        .py(px(2.))
                        .rounded(px(6.))
                        .border_1()
                        .border_color(rgb(ACTIVE_BORDER))
                        .bg(rgb(0xFFFFFF))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(div().flex_1().min_w(px(0.)).child(input.clone()))
                        .when_some(counter, |d, c| {
                            d.child(
                                div()
                                    .flex_shrink_0()
                                    .text_size(px(11.))
                                    .text_color(rgb(MUTED_COLOR))
                                    .child(c),
                            )
                        })
                        .into_any_element()
                }
                None => div()
                    .id("omni-pill")
                    .occlude()
                    .flex_shrink_0()
                    .w(px(omni_field_width(window)))
                    .px(px(10.))
                    .py(px(2.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(rgb(RULE_COLOR))
                    .bg(rgb(0xFFFFFF))
                    .cursor(CursorStyle::IBeam)
                    .hover(|d| d.border_color(rgb(0xD8D2C2)))
                    .tooltip(tip("Search · > commands · @ headings", Some("ctrl-f")))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|editor, _: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            editor.find(&Find, window, cx);
                        }),
                    )
                    // The web-placeholder contract: "Search" sits exactly
                    // where the first typed letter will land (same inset,
                    // same size), and the chord hint keeps its old home.
                    .flex()
                    .items_center()
                    .child(div().text_color(rgb(MUTED_COLOR)).child("Search"))
                    .child(
                        div()
                            .ml_auto()
                            .flex_shrink_0()
                            .text_size(px(11.))
                            .text_color(rgb(MUTED_COLOR))
                            // One chord notation everywhere (extraction audit
                            // #12): the tooltips and palette rows all speak
                            // lowercase "ctrl-f"; the hint matches them.
                            .child("ctrl-f"),
                    )
                    .into_any_element(),
            })
            // Right third — mirrors the left (equal claims keep the centre
            // still): the editor button, palette, history, window controls.
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_end()
                // The editor button (impl 04 §0): the AI subsystem's single home,
                // where its results live — just left of the margin side. The old
                // drawn mini-card is gone; the control now WEARS its state (a
                // priority face) and opens the dropdown that names the reads.
                .child({
                    let face = self.editor_face();
                    let open = self.editor_menu_open;
                    let n = self.open_query_count();
                    // The face's label. "Reading" is the glossary presence word,
                    // never the internal "Reviewing" (review H31).
                    let label: String = match face {
                        EditorFace::NeedsSetup => "Ask the editor · needs setup".to_owned(),
                        EditorFace::Cooking | EditorFace::Error | EditorFace::Idle => {
                            "Ask the editor".to_owned()
                        }
                        EditorFace::Ready => "Ask the editor · a read is ready".to_owned(),
                        EditorFace::Reading if n > 0 => {
                            format!("Reading · {n} open · Ask the editor")
                        }
                        EditorFace::Reading => "Reading · Ask the editor".to_owned(),
                    };
                    // A colour cue only where the words don't carry it: cool = the
                    // machine is at work, red = it failed (color-language.md axis).
                    let dot = match face {
                        EditorFace::Cooking => Some(AI_ACCENT),
                        EditorFace::Error => Some(ERROR),
                        _ => None,
                    };
                    // Hover names the read while cooking, carries the message on a
                    // failure (the full card still lives on render_ai_status), and
                    // teaches the chord in every state.
                    let tip_label: SharedString = match (face, self.ai_status.as_ref()) {
                        (EditorFace::Cooking, Some(AiStatus::Running { label })) => {
                            format!("Running: {label}").into()
                        }
                        (EditorFace::Error, Some(AiStatus::Error { title, .. })) => title.clone().into(),
                        (EditorFace::NeedsSetup, _) => "Ask the editor — set up a provider".into(),
                        _ => "Ask the editor".into(),
                    };
                    div()
                        .id("editor-btn")
                        .occlude()
                        .flex_shrink_0()
                        .ml(px(4.))
                        // Dressed as a control (the lab's .ebtn): border, shape,
                        // an arrow — the one dropdown in the bar must read as
                        // one at rest, not only on hover.
                        .px(px(10.))
                        .py(px(3.))
                        .rounded(px(7.))
                        .border_1()
                        .border_color(rgb(RULE_COLOR))
                        .cursor(CursorStyle::PointingHand)
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .when(open, |d| d.bg(rgba(0x1A1A1812u32)))
                        .hover(|d| d.bg(rgb(CARD_BG)))
                        .text_color(rgb(if face == EditorFace::Reading {
                            TEXT_COLOR
                        } else {
                            MUTED_COLOR
                        }))
                        .tooltip(tip(tip_label, Some("ctrl-shift-d")))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                editor.toggle_editor_menu(cx);
                            }),
                        )
                        // The glue probe: the dropdown must sit right-edge-to-
                        // right-edge with THIS control, so record where the
                        // control actually painted (see editor_btn_right).
                        // Plain shared-cell write, never an entity update
                        // mid-draw. The menu renders BEFORE this runs, so when
                        // the edge moved this frame (resize, a label change)
                        // and the menu is up, schedule ONE follow-up frame —
                        // the EditorElement's geometry-changed idiom; it
                        // converges as soon as the edge holds still.
                        .child({
                            let cell = self.editor_btn_right.clone();
                            capture_canvas(
                                move |bounds, window, _| {
                                    // inset_0 spans the button's PADDING box;
                                    // add its border back, so the cell holds
                                    // the control's true painted edge.
                                    let edge = bounds.right() + px(1.);
                                    let stale = cell.get() != Some(edge);
                                    cell.set(Some(edge));
                                    if stale && open {
                                        window.request_animation_frame();
                                    }
                                },
                                |_, _, _, _| {},
                            )
                            .absolute()
                            .inset_0()
                        })
                        .when_some(dot, |d, color| {
                            d.child(div().size(px(6.)).rounded_full().bg(rgb(color)))
                        })
                        .child(div().child(label))
                        // The dropdown wedge — contiguous decreasing bars fuse
                        // into a solid ▾ (the app's own drawn-glyph idiom; the
                        // real "▾" isn't in the PT fonts). It wears the label's
                        // ink, not a second colour.
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .children([7., 5., 3., 1.].into_iter().map(|w| {
                                    div().w(px(w)).h(px(1.)).bg(rgb(
                                        if face == EditorFace::Reading {
                                            TEXT_COLOR
                                        } else {
                                            MUTED_COLOR
                                        },
                                    ))
                                })),
                        )
                })
                // The day-zero affordance: a user who knows nothing clicks the
                // one unexplained button and lands in a searchable list of every
                // capability (GNOME primary-menu position).
                .child(
                    div()
                        .id("palette-toggle")
                        .occlude()
                        .px(px(8.))
                        .py(px(2.))
                        .rounded(px(5.))
                        .cursor(CursorStyle::PointingHand)
                        .when(self.palette_input.is_some(), |d| d.bg(rgba(0x1A1A1812u32)))
                        .hover(|d| d.bg(rgba(0x1A1A180Au32)))
                        .tooltip(tip("Command Palette", Some("ctrl-shift-p")))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|editor, _: &MouseDownEvent, window, cx| {
                                cx.stop_propagation();
                                editor.toggle_palette(&TogglePalette, window, cx);
                            }),
                        )
                        .child(
                            // Drawn hamburger (no PT-covered menu glyph).
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .children((0..3).map(|_| {
                                    div().w(px(11.)).h(px(1.5)).bg(rgb(MUTED_COLOR))
                                })),
                        ),
                )
                .child(
                    div()
                        .id("history-toggle")
                        .occlude()
                        .px(px(8.))
                        .py(px(2.))
                        .ml(px(4.))
                        .rounded(px(5.))
                        .cursor(CursorStyle::PointingHand)
                        .text_color(if self.strip.open {
                            rgb(TEXT_COLOR)
                        } else {
                            rgb(MUTED_COLOR)
                        })
                        .when(self.strip.open, |d| d.bg(rgba(0x1A1A1812u32)))
                        .hover(|d| d.bg(rgba(0x1A1A180Au32)))
                        .tooltip(tip("History", Some("ctrl-alt-h")))
                        .on_mouse_down(
                            MouseButton::Left,
                            // The clock toggles the STRIP (the new first history
                            // surface); the right-side panel lives on in the palette
                            // ("History panel"). The two never open together.
                            cx.listener(|editor, _: &MouseDownEvent, window, cx| {
                                cx.stop_propagation();
                                editor.toggle_strip(&ToggleStrip, window, cx);
                            }),
                        )
                        .child(
                            // History: drawn clock-face stand-in (↺ isn't in PT).
                            div()
                                .size(px(11.))
                                .rounded_full()
                                .border_1()
                                .border_color(if self.strip.open {
                                    rgb(TEXT_COLOR)
                                } else {
                                    rgb(MUTED_COLOR)
                                })
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(div().size(px(3.)).rounded_full().bg(if self.strip.open {
                                    rgb(TEXT_COLOR)
                                } else {
                                    rgb(MUTED_COLOR)
                                })),
                        ),
                )
                // Windows/Linux get our own drawn window controls. macOS keeps its
                // native traffic lights (top-left) instead, so we omit these here —
                // which also retires the macOS papercut that our "×" quit the whole
                // app, and sidesteps the fact that the "–"/"×" glyph labels are the
                // very thing the macOS glyph bug hides (issue #10).
                .when(!cfg!(target_os = "macos"), |bar| {
                    bar.child(self.window_button("–", "Minimize", None, |window, _| {
                        window.minimize_window()
                    }))
                    .child(
                        // Zoom: drawn square (U+25A1 isn't in PT).
                        div()
                            .id("win-zoom")
                            .occlude()
                            .w(px(34.))
                            .h_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .hover(|d| d.bg(rgba(0x1A1A180Au32)))
                            .tooltip(tip("Maximize", None))
                            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                cx.stop_propagation();
                                window.zoom_window();
                            })
                            .child(
                                div()
                                    .size(px(9.))
                                    .border_1()
                                    .border_color(rgb(MUTED_COLOR))
                                    .rounded(px(1.)),
                            ),
                    )
                    .child(self.window_button("×", "Close", Some("ctrl-q"), |_, cx| cx.quit()))
                })
            )
    }
}


/// Days-from-epoch to civil date (Howard Hinnant's algorithm); good enough
/// for checkpoint labels (UTC — rough UI, backlogged with the rest).
fn format_unix(secs: i64) -> String {
    // STROP_TEST_STILL (scripts/wflip.sh): captures are byte-compared, so
    // every rendered timestamp freezes to a fixed string.
    if std::env::var("STROP_TEST_STILL").is_ok() {
        return "0000-00-00 00:00".into();
    }
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", rem / 3600, (rem % 3600) / 60)
}

impl Editor {
    /// Width of the history side panel for this window, 0 when closed.
    /// Push, not overlay (DESIGN §2-history): the document column reflows.
    /// In narrow windows the panel shrinks first; prose keeps ~DOC_MIN_WIDTH.
    fn history_panel_width(&self, window: &Window) -> f32 {
        if self.history_view.is_none() {
            return 0.;
        }
        let vw = f32::from(window.viewport_size().width);
        (vw - DOC_MIN_WIDTH).clamp(180., HISTORY_PANEL_WIDTH)
    }

    /// Width of the outline rail, 0 when closed. Push, not overlay, like
    /// the history panel; it stands down while history is open (the canvas
    /// shows a merged diff there — live-doc offsets wouldn't match) and in
    /// windows too narrow to keep the prose at DOC_MIN_WIDTH.
    fn rail_width(&self, window: &Window) -> f32 {
        if !self.rail_open || self.history_view.is_some() {
            return 0.;
        }
        let vw = f32::from(window.viewport_size().width);
        let free = vw - DOC_MIN_WIDTH;
        if free < 120. { 0. } else { free.min(RAIL_PANEL_WIDTH) }
    }

    /// The document's headings: (block index, level, text, byte offset of
    /// the heading's start).
    fn outline_items(&self) -> Vec<(usize, u8, String, usize)> {
        let kinds = self.doc.blocks().kinds();
        let mut items = Vec::new();
        let mut byte = 0usize;
        for (ix, line) in self.doc.rope().lines().enumerate() {
            if let Some(BlockKind::Heading(level)) = kinds.get(ix) {
                let text: String = line.chars().take(120).collect();
                let text = text.trim().to_owned();
                // A blank line wearing a heading kind (e.g. the seam a
                // set-aside heading leaves behind) is not an outline entry.
                if !text.is_empty() {
                    items.push((ix, *level, text, byte));
                }
            }
            byte += line.len_bytes();
        }
        items
    }

    /// The compost items for the sidebar (Bug A): one `(preview, byte_start)`
    /// per item — a run of non-empty compost blocks (blank lines separate
    /// items). The preview is the item's first line, ~60 chars. Empty when there
    /// is no rail (the sidebar is then outline-only, unchanged).
    fn compost_items(&self) -> Vec<(String, usize)> {
        let Some(boundary) = self.doc.aside_boundary() else {
            return Vec::new();
        };
        let mut items = Vec::new();
        let mut byte = 0usize;
        let mut prev_empty = true; // the first non-empty block opens an item
        for (ix, line) in self.doc.rope().lines().enumerate() {
            if ix >= boundary {
                break;
            }
            let s: String = line.chars().filter(|c| *c != '\n').collect();
            let empty = s.trim().is_empty();
            if !empty && prev_empty {
                items.push((s.trim().chars().take(60).collect(), byte));
            }
            prev_empty = empty;
            byte += line.len_bytes();
        }
        items
    }

    /// The compost rail (06 §2): the left panel IS the compost's navigator —
    /// a header and a row per item; click scrolls to the block and flashes
    /// it. The outline that used to live here is gone (third-time product
    /// decision: nobody cares about a header tree in a three-page blogpost);
    /// heading navigation survives as the palette's `@` mode. An empty
    /// compost opens to the header and air — no hint, no lecture (P4).
    fn render_rail(&self, panel_w: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let items = self.compost_items();
        // Arrival blink (06 §2): the newest item is the compost tail — the
        // same decaying seltint wash the parked banner's refusal pulse uses.
        let flash_a = self.rail_flash.map_or(0., |t| {
            let e = t.elapsed().as_millis() as f32 / 1400.;
            (0.33 * (1. - e)).clamp(0., 0.33)
        });
        let last = items.len().saturating_sub(1);
        let mut list = div()
            .id("rail-list")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .px(px(8.))
            .py(px(6.));
        for (i, (preview, byte_start)) in items.iter().enumerate() {
            let jump = *byte_start;
            list = list.child(
                div()
                    .id(("compost-row", i))
                    .px(px(8.))
                    .py(px(3.))
                    .rounded(px(4.))
                    .cursor(CursorStyle::PointingHand)
                    .when(i == last && flash_a > 0., |d| {
                        d.bg(tint(0xC8A951, flash_a))
                    })
                    .hover(|d| d.bg(rgba(0x1A1A180Au32)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |editor, _: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            editor.move_to(jump, cx); // caret + scroll-to
                            let c = editor
                                .doc
                                .rope()
                                .byte_to_char(jump.min(editor.doc.len_bytes()));
                            editor.para_flash = Some((c, Instant::now()));
                            editor.schedule_flash_clear(cx);
                            window.focus(&editor.focus_handle, cx);
                        }),
                    )
                    .child(
                        div()
                            .min_w(px(0.))
                            .truncate()
                            .font_family("PT Serif")
                            .text_color(rgb(MUTED_COLOR))
                            .child(preview.clone()),
                    ),
            );
        }
        div()
            .id("rail-panel")
            .absolute()
            .top(px(BAR_HEIGHT))
            .left_0()
            .bottom_0()
            .w(px(panel_w))
            .bg(rgb(0xF4F1EA))
            .border_r_1()
            .border_color(rgb(RULE_COLOR))
            .flex()
            .flex_col()
            .font_family("PT Sans")
            .text_size(px(12.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            // The rail owns its wheel (extraction audit #15): its list
            // scrolls itself; the prose behind must not move with it.
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .px(px(14.))
                    .py(px(6.))
                    .border_b_1()
                    .border_color(rgb(RULE_COLOR))
                    .text_color(rgb(MUTED_COLOR))
                    .child("Compost"),
            )
            .child(list)
    }

    /// The sticky graveyard footer bar (docs/impl/02-asides.md §4; asides.md
    /// §3): "Graveyard · N", the omnipresent navigator. It blinks on an exile
    /// and, clicked, scrolls to the tail section (the record lives in the scroll
    /// flow now — Bug B). It HIDES when the section header is on screen ("unsticks
    /// into the section header"). Right after a cut it carries a transient "put
    /// back" quick-verb for the just-filed entry (the mockup's gravebar). The ⚰
    /// glyph is outside the PT fonts, so the coffin is a drawn div.
    fn render_graveyard_bar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let n = self.doc.graveyard().len();
        if n == 0 || self.history_view.is_some() || self.grave_tail_on_screen() {
            return None;
        }
        let flashing = self.grave_flash.is_some();
        // The quick "put back" appears only in the flash window right after a cut
        // — the newest entry (last filed). One verb everywhere (P8, P13).
        let newest = flashing
            .then(|| self.doc.graveyard().entries().last().map(|e| e.id))
            .flatten();
        Some(
            div()
                .id("graveyard-bar")
                .absolute()
                .bottom_0()
                .left_0()
                .right_0()
                .h(px(28.))
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(16.))
                .bg(rgb(if flashing { 0xEFE6D0 } else { 0xF4F1EA }))
                .border_t_1()
                .border_color(rgb(RULE_COLOR))
                .font_family("PT Sans")
                .text_size(px(12.))
                .text_color(rgb(if flashing { TEXT_COLOR } else { MUTED_COLOR }))
                .cursor(CursorStyle::PointingHand)
                .hover(|d| d.text_color(rgb(TEXT_COLOR)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        editor.scroll_to_graveyard(cx);
                    }),
                )
                .child(tombstone_icon())
                .child(format!("Graveyard · {n}"))
                .when_some(newest, |d, id| {
                    d.child(
                        div()
                            .id("grave-quick-putback")
                            .ml(px(4.))
                            .text_color(rgb(AI_ACCENT))
                            .cursor(CursorStyle::PointingHand)
                            .hover(|d| d.text_color(rgb(TEXT_COLOR)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |editor, _: &MouseDownEvent, _, cx| {
                                    cx.stop_propagation();
                                    editor.put_back_entry(id, cx);
                                }),
                            )
                            .child("put back"),
                    )
                }),
        )
    }

    /// The mode banner (DESIGN §2-history, principle 5 — no hidden modes):
    /// a slim strip across the top of the document area naming what you're
    /// viewing, with the one verb (Restore) and the exit. It lives in the
    /// column's top padding — never over prose.
    fn render_history_banner(&self, panel_w: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let (name, stamp) = self
            .history_view
            .as_ref()
            .map(|hv| {
                let e = &hv.entries[hv.selected];
                (e.name.clone(), format_unix(e.created_unix))
            })
            .unwrap_or_default();
        div()
            .absolute()
            .top(px(BAR_HEIGHT))
            .left_0()
            .right(px(panel_w))
            .h(px(30.))
            .bg(rgb(0xF4F1EA))
            .border_b_1()
            .border_color(rgb(RULE_COLOR))
            .px(px(16.))
            .flex()
            .items_center()
            .gap(px(8.))
            .font_family("PT Serif")
            .text_size(px(12.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(div().flex_shrink_0().text_color(rgb(MUTED_COLOR)).child("Viewing:"))
            .child(
                div()
                    .min_w(px(0.))
                    .truncate()
                    .text_color(rgb(TEXT_COLOR))
                    .child(format!("{name} · {stamp}")),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("restore-btn")
                    .flex_shrink_0()
                    .px(px(8.))
                    .py(px(1.))
                    .rounded(px(4.))
                    .cursor(CursorStyle::PointingHand)
                    .bg(rgb(0xE8DFC8))
                    .text_color(rgb(TEXT_COLOR))
                    .hover(|d| d.bg(rgb(0xDFD3B0)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            editor.restore_selected(cx);
                        }),
                    )
                    .child("Restore this version"),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .text_color(rgb(MUTED_COLOR))
                    .child("Esc to exit"),
            )
    }

    /// The parked-mode banner (Bug B): a stale strip directly under the
    /// titlebar that IS the mode indicator for the text area while previewing
    /// the past — the mockup's history-preview variant (scene 1). **bold
    /// moment label** · "N words" · a dark [Restore] chip · "·" · "Esc
    /// returns". No lecture (P4). The moment label is the checkpoint's name
    /// (curly-quoted) when the playhead sits on a tick, else the bare date/
    /// time; it flashes seltint on a REFUSED edit (`pulse_strip`), fading over
    /// ~900ms — the read-only mode made visible, its refusal legible.
    fn render_strip_banner(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if !self.strip.is_parked() {
            return None;
        }
        let now_ms = self
            .strip
            .bake
            .as_ref()
            .map_or_else(strop_core::journal::now_ms, |b| b.now_ms);
        // On a station tick → its name, curly-quoted; else the date/time.
        let at_station = self.strip.bake.as_ref().and_then(|b| {
            let play = b.timeline.work_at(self.strip.pos_ms);
            b.stations
                .iter()
                .filter(|s| !s.label.is_empty())
                .find(|s| (s.x - play).abs() < 5.)
                .map(|s| format!("\u{201c}{}\u{201d}", s.label))
        });
        let moment = at_station.unwrap_or_else(|| strip::format_moment(self.strip.pos_ms, now_ms));
        let words = format!("{} words", format_thousands(self.strip.words_at));
        // The refusal pulse: seltint (0xC8A951 @ .33) decaying to 0 across the
        // pulse window — a pure function of the stored Instant.
        let pulse_a = self.strip_pulse.map_or(0., |t| {
            let e = t.elapsed().as_millis() as f32 / STRIP_PULSE_MS as f32;
            (0.33 * (1. - e)).clamp(0., 0.33)
        });

        let restore_chip = div()
            .id("strip-banner-restore")
            .occlude()
            .px(px(12.))
            .py(px(2.))
            .rounded(px(11.))
            .bg(rgb(TEXT_COLOR))
            .text_color(rgb(BG_COLOR))
            .font_family("PT Sans")
            .text_size(px(11.5))
            .cursor(CursorStyle::PointingHand)
            .hover(|d| d.bg(rgb(0x33322D)))
            .child("Restore")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    editor.strip_restore(cx);
                }),
            );

        Some(
            div()
                .absolute()
                .top(px(BAR_HEIGHT))
                .left_0()
                .right_0()
                .h(px(30.))
                .bg(rgb(0xEFEEEA))
                .border_b_1()
                .border_color(rgb(RULE_COLOR))
                .flex()
                .items_center()
                .justify_center()
                .gap(px(9.))
                .font_family("PT Sans")
                .text_size(px(12.))
                .text_color(rgb(MUTED_COLOR))
                // A click on the banner is not a document click.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .px(px(5.))
                        .py(px(1.))
                        .rounded(px(4.))
                        .bg(tint(0xC8A951, pulse_a))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(TEXT_COLOR))
                        .child(moment),
                )
                .child(div().child(format!("\u{b7} {words}")))
                .child(restore_chip)
                .child(div().text_color(rgb(0xB8B4A8)).child("\u{b7}"))
                .child(div().child("Esc returns")),
        )
    }

    /// The history side panel (DESIGN §2-history): full-height, right,
    /// displaces the margin while open. Two-tier list — named checkpoints
    /// are first-class rows; runs of automatic ones collapse into
    /// expandable "N auto-checkpoints" rows (Figma's answer to autosave
    /// noise). vs-prev/vs-draft rides the bottom as a segmented control.
    fn render_history_panel(&self, panel_w: f32, cx: &mut Context<Self>) -> impl IntoElement {
        let empty_expanded = HashSet::new();
        let hv = self.history_view.as_ref();
        let materializing = hv.is_some_and(|h| h.materializing);
        let (entries, selected, named_only, compare_current, expanded) = match hv {
            Some(hv) => (
                hv.entries.as_slice(),
                hv.selected,
                hv.named_only,
                hv.compare_current,
                &hv.expanded,
            ),
            None => (&[][..], 0, false, false, &empty_expanded),
        };
        // One checkpoint row: dot marker (drawn: ●/○ aren't in PT), name,
        // time, word delta, drift scalar when flagged. Double-click renames
        // in place. Expanded auto rows indent under their group row.
        let entry_row = |ix: usize, e: &HistoryEntry, indent: bool| {
            let stamp = format_unix(e.created_unix);
            let (_, time) = stamp.split_once(' ').unwrap_or((stamp.as_str(), ""));
            let time = time.to_owned();
            // Word delta against the previous version, spelled with its
            // unit (the bare "+412 −0" read as a riddle).
            let (ins, del) = e.delta;
            let delta = match (ins, del) {
                (0, 0) => String::new(),
                (i, 0) => format!("+{i} words"),
                (0, d) => format!("−{d} words"),
                (i, d) => format!("+{i} −{d} words"),
            };
            div()
                .id(("hist-row", ix))
                .px(px(8.))
                .py(px(4.))
                .when(indent, |d| d.pl(px(20.)))
                .rounded(px(5.))
                .cursor(CursorStyle::PointingHand)
                .when(ix == selected, |d| d.bg(rgba(0x1A1A1812u32)))
                .hover(|d| d.bg(rgba(0x1A1A180Au32)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |editor, ev: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        if ev.click_count >= 2 {
                            editor.start_rename(ix, window, cx);
                        } else {
                            editor.history_select(ix, cx);
                        }
                    }),
                )
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .child(
                            match self
                                .rename_input
                                .as_ref()
                                .filter(|(rix, _)| *rix == ix)
                            {
                                Some((_, input)) => {
                                    div().flex_1().child(input.clone())
                                }
                                None => div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(
                                        div()
                                            .flex_shrink_0()
                                            .size(px(7.))
                                            .rounded_full()
                                            .when(e.manual, |d| {
                                                d.bg(rgb(TEXT_COLOR))
                                            })
                                            .when(!e.manual, |d| {
                                                d.border_1().border_color(
                                                    rgb(MUTED_COLOR),
                                                )
                                            }),
                                    )
                                    .child(
                                        div()
                                            .min_w(px(0.))
                                            .truncate()
                                            .text_color(if e.name.is_empty() {
                                                rgb(MUTED_COLOR)
                                            } else {
                                                rgb(TEXT_COLOR)
                                            })
                                            .when(e.manual, |d| {
                                                d.font_weight(FontWeight::BOLD)
                                            })
                                            // Autos have no name: label them so
                                            // the row never reads as a blank.
                                            .child(if e.name.is_empty() {
                                                "Auto-save".to_owned()
                                            } else {
                                                e.name.clone()
                                            }),
                                    ),
                            },
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .ml(px(6.))
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .text_size(px(11.))
                                .child(
                                    div().text_color(rgb(MUTED_COLOR)).child(if delta.is_empty() {
                                        time
                                    } else {
                                        format!("{time} · {delta}")
                                    }),
                                )
                                .when_some(e.drift_sigma, |d, s| {
                                    // Scalar caps at >10σ: beyond that the
                                    // number is noise, not information.
                                    d.child(div().text_color(rgb(0x8A6A3A)).child(
                                        if s >= 10. {
                                            ">10σ".to_owned()
                                        } else {
                                            format!("{s:+.1}σ")
                                        },
                                    ))
                                }),
                        ),
                )
                .into_any_element()
        };
        let day_header = |day: &str| {
            div()
                .px(px(8.))
                .pt(px(8.))
                .text_size(px(11.))
                .text_color(rgb(MUTED_COLOR))
                .child(day.to_owned())
                .into_any_element()
        };
        let mut last_day = String::new();
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        let mut ix = 0usize;
        while ix < entries.len() {
            let stamp = format_unix(entries[ix].created_unix);
            let (day, _) = stamp.split_once(' ').unwrap_or((stamp.as_str(), ""));
            if entries[ix].manual {
                if day != last_day {
                    last_day = day.to_owned();
                    rows.push(day_header(day));
                }
                rows.push(entry_row(ix, &entries[ix], false));
                ix += 1;
                continue;
            }
            // A run of automatic checkpoints: one collapsed row between
            // named neighbours; click (or arrow-stepping into it) unfolds.
            let end = entries[ix..]
                .iter()
                .position(|e| e.manual)
                .map_or(entries.len(), |p| ix + p);
            if named_only {
                ix = end;
                continue;
            }
            if day != last_day {
                last_day = day.to_owned();
                rows.push(day_header(day));
            }
            let n = end - ix;
            let is_open = expanded.contains(&ix);
            let holds_selection = (ix..end).contains(&selected);
            let gix = ix;
            rows.push(
                div()
                    .id(("hist-group", gix))
                    .px(px(8.))
                    .py(px(4.))
                    .rounded(px(5.))
                    .cursor(CursorStyle::PointingHand)
                    .when(!is_open && holds_selection, |d| d.bg(rgba(0x1A1A1812u32)))
                    .hover(|d| d.bg(rgba(0x1A1A180Au32)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |editor, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            if let Some(hv) = &mut editor.history_view {
                                if !hv.expanded.remove(&gix) {
                                    hv.expanded.insert(gix);
                                }
                                cx.notify();
                            }
                        }),
                    )
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex_shrink_0()
                            .size(px(7.))
                            .rounded_full()
                            .border_1()
                            .border_color(rgb(MUTED_COLOR)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_color(rgb(MUTED_COLOR))
                            .child(format!(
                                "{n} auto-checkpoint{}",
                                if n == 1 { "" } else { "s" }
                            )),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(11.))
                            .text_color(rgb(MUTED_COLOR))
                            .child(if is_open { "hide" } else { "show" }),
                    )
                    .into_any_element(),
            );
            if is_open {
                for (k, entry) in entries.iter().enumerate().take(end).skip(ix) {
                    rows.push(entry_row(k, entry, true));
                }
            }
            ix = end;
        }
        div()
            .id("history-panel")
            .absolute()
            .top(px(BAR_HEIGHT))
            .right_0()
            .bottom_0()
            .w(px(panel_w))
            .bg(rgb(0xF4F1EA))
            .border_l_1()
            .border_color(rgb(RULE_COLOR))
            .flex()
            .flex_col()
            .font_family("PT Serif")
            .text_size(px(13.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .px(px(14.))
                    .py(px(6.))
                    .border_b_1()
                    .border_color(rgb(RULE_COLOR))
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(TEXT_COLOR))
                            .child("History"),
                    )
                    .child(
                        // Named-only filter as a real checkbox-chip: the bare
                        // word "named" gave no hint it was a control.
                        div()
                            .id("named-only")
                            .flex()
                            .items_center()
                            .gap(px(5.))
                            .px(px(6.))
                            .py(px(2.))
                            .rounded(px(4.))
                            .cursor(CursorStyle::PointingHand)
                            .text_size(px(11.))
                            .text_color(if named_only {
                                rgb(TEXT_COLOR)
                            } else {
                                rgb(MUTED_COLOR)
                            })
                            .hover(|d| d.bg(rgba(0x1A1A180Au32)))
                            .tooltip(tip("Show only named checkpoints", None))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                                    cx.stop_propagation();
                                    if let Some(hv) = &mut editor.history_view {
                                        hv.named_only = !hv.named_only;
                                        cx.notify();
                                    }
                                }),
                            )
                            .child(
                                div()
                                    .size(px(11.))
                                    .rounded(px(2.))
                                    .border_1()
                                    .border_color(rgb(MUTED_COLOR))
                                    .when(named_only, |d| d.bg(rgb(TEXT_COLOR))),
                            )
                            .child("Named only"),
                    ),
            )
            .child(
                // The interaction model, stated where it is seen first
                // (it used to be a muted line buried at the very bottom).
                // Google-Docs rewind: preview by clicking, restore is safe.
                div()
                    .px(px(14.))
                    .py(px(7.))
                    .border_b_1()
                    .border_color(rgb(RULE_COLOR))
                    .text_size(px(11.))
                    .line_height(px(16.))
                    .text_color(rgb(MUTED_COLOR))
                    .child(
                        "Click a version to preview it in the document; Up/Down steps through \
                         them. Double-click a name to rename. Restore brings a version back — \
                         undoable, like everything here. Nothing is ever lost.",
                    ),
            )
            // A legacy file's first history open: versions are being read out
            // of the old format in the background (once, ever) — the rows are
            // already here, previews land in a moment.
            .when(materializing, |d| {
                d.child(
                    div()
                        .px(px(14.))
                        .py(px(7.))
                        .border_b_1()
                        .border_color(rgb(RULE_COLOR))
                        .text_size(px(11.))
                        .text_color(rgb(MUTED_COLOR))
                        .child("Assembling version previews — a moment…"),
                )
            })
            .child(
                div()
                    .id("history-list")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .p(px(6.))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .children(rows),
            )
            .when(compare_current, |d| {
                // vs-draft: how the draft sits against the self-baseline,
                // plus descriptive stylometry between the selected version
                // and the draft (rhythm first — research: flattening
                // variance is the LLM-characteristic signal).
                let lang = match self.config.language {
                    Language::Ru => typograph::Lang::Ru,
                    Language::En => typograph::Lang::En,
                    Language::Auto => typograph::detect_lang(self.doc.rope().chars()),
                };
                let d = match self.voice_baseline.as_ref() {
                    Some(baseline) => {
                        // assess() requires the baseline's own language —
                        // the function-word vectors are per-language and
                        // differently sized.
                        let report = baseline.assess(&strop_core::voice::signature(
                            &self.doc.text(),
                            baseline.lang(),
                        ));
                        // Chrome is English-only (DESIGN §0.7), regardless of
                        // the document's language.
                        let headline = if report.overall_sigma > 2. {
                            format!(
                                "Voice: {:.1}σ outside your normal range ({} texts)",
                                report.overall_sigma, baseline.docs
                            )
                        } else {
                            format!("Voice: within your normal range ({} texts)", baseline.docs)
                        };
                        d.child(
                            div()
                                .px(px(14.))
                                .pt(px(6.))
                                .flex()
                                .flex_col()
                                .gap(px(1.))
                                .text_size(px(11.))
                                // A voice anomaly is a descriptive flag, not an
                                // error — red is reserved (color language). The
                                // "Nσ outside your normal range" headline carries
                                // the signal in text, so it stays muted.
                                .text_color(rgb(MUTED_COLOR))
                                .children(std::iter::once(headline).chain(report.flags)),
                        )
                    }
                    None => d,
                };
                let drift = self
                    .history_view
                    .as_ref()
                    .map(|hv| {
                        let from =
                            strop_core::voice::signature(&hv.entries[hv.selected].text, lang);
                        let to = strop_core::voice::signature(&self.doc.text(), lang);
                        strop_core::voice::describe_drift(&from, &to, lang)
                    })
                    .unwrap_or_default();
                d.when(!drift.is_empty(), |d| {
                    d.child(
                        div()
                            .px(px(14.))
                            .pt(px(6.))
                            .flex()
                            .flex_col()
                            .gap(px(1.))
                            .text_size(px(11.))
                            .text_color(rgb(0x8A6A3A))
                            .children(
                                std::iter::once("Voice drift (v0, descriptive):".to_owned())
                                    .chain(drift),
                            ),
                    )
                })
            })
            .child(
                // Pinned at the panel bottom (the Docs "Show changes"
                // position): what the canvas diff compares against.
                div()
                    .border_t_1()
                    .border_color(rgb(RULE_COLOR))
                    .p(px(8.))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        div()
                            .px(px(2.))
                            .text_size(px(11.))
                            .text_color(rgb(MUTED_COLOR))
                            .child("Show changes in the document, compared against:"),
                    )
                    .child(
                        div().flex().text_size(px(12.)).children(
                            [("Previous version", false), ("Current draft", true)].map(
                                |(label, value)| {
                                    let on = compare_current == value;
                                    div()
                                        .id(label)
                                        .flex_1()
                                        .py(px(3.))
                                        .flex()
                                        .justify_center()
                                        .cursor(CursorStyle::PointingHand)
                                        .border_1()
                                        .border_color(rgb(RULE_COLOR))
                                        .when(!value, |d| d.rounded_l(px(5.)))
                                        .when(value, |d| d.rounded_r(px(5.)).border_l_0())
                                        .when(on, |d| {
                                            d.bg(rgb(0xE8DFC8)).text_color(rgb(TEXT_COLOR))
                                        })
                                        .when(!on, |d| {
                                            d.text_color(rgb(MUTED_COLOR))
                                                .hover(|d| d.bg(rgba(0x1A1A180Au32)))
                                        })
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(
                                                move |editor, _: &MouseDownEvent, _, cx| {
                                                    cx.stop_propagation();
                                                    if let Some(hv) =
                                                        &mut editor.history_view
                                                        && hv.compare_current != value
                                                    {
                                                        hv.compare_current = value;
                                                        editor.rebuild_preview();
                                                        cx.notify();
                                                    }
                                                },
                                            ),
                                        )
                                        .child(label)
                                },
                            ),
                        ),
                    ),
            )
    }

    /// Map a window-space x on a zone row's text to a byte offset within
    /// the def mirror, by re-shaping the row's exact text at its rendered
    /// font (PT Serif 14px) against the bounds captured at paint time.
    fn zone_def_offset(
        &self,
        row: usize,
        text: &str,
        def_len: usize,
        x: Pixels,
        window: &mut Window,
    ) -> Option<usize> {
        let bounds = *self.zone_row_bounds.borrow().get(&row)?;
        let run = TextRun {
            len: text.len(),
            font: gpui::font("PT Serif"),
            color: rgb(MUTED_COLOR).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let shaped = window.text_system().shape_line(
            SharedString::from(text.to_owned()),
            px(14.),
            &[run],
            None,
        );
        // Clamp into the real def: the trailing "…" is not document text.
        Some(shaped.closest_index_for_x(x - bounds.origin.x).min(def_len))
    }

    /// The bottom zone (DESIGN §2-footnotes): a read-only mirror of the
    /// defs whose refs are on-screen. Click the row's text = caret lands
    /// at that offset in the def line (the Word notes-pane niche); click
    /// the row's "N." = jump back to the in-text ref.
    fn render_footnote_zone(
        &self,
        footnotes: Vec<ZoneNote>,
        hidden: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let row_bounds = self.zone_row_bounds.clone();
        div()
            .id("footnote-zone")
            .absolute()
            .bottom_0()
            .left_0()
            .right_0()
            .max_h(px(168.)) // 6 rows; ~1/3 of a short window
            .overflow_y_scroll()
            .bg(rgb(BG_COLOR))
            .border_t_1()
            .border_color(rgb(RULE_COLOR))
            .flex()
            .flex_col()
            .items_center()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .w_full()
                    .max_w(px(660.))
                    .px(px(28.))
                    .py(px(6.))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .font_family("PT Serif")
                    .text_size(px(14.))
                    .text_color(rgb(MUTED_COLOR))
                    .children(footnotes.into_iter().enumerate().map(|(ix, note)| {
                        let row_bounds = row_bounds.clone();
                        let ZoneNote {
                            no,
                            def,
                            def_start,
                            def_len,
                            ref_byte,
                        } = note;
                        let def_text = def.clone();
                        div()
                            .id(ix)
                            .px(px(4.))
                            .rounded(px(4.))
                            .flex()
                            .items_start()
                            .hover(|d| d.bg(rgba(0x1A1A180Au32)))
                            .child(
                                // The marker mirrors the def line's painted
                                // "N." — clicking it jumps back to the ref.
                                div()
                                    .w(px(24.))
                                    .flex_shrink_0()
                                    .cursor(CursorStyle::PointingHand)
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(
                                            move |editor, _: &MouseDownEvent, _, cx| {
                                                cx.stop_propagation();
                                                editor.goal_x = None;
                                                editor.set_cursor(ref_byte, false, cx);
                                            },
                                        ),
                                    )
                                    .child(format!("{no}.")),
                            )
                            .child(
                                // The text is the edit surface: the click
                                // lands the caret at the matching offset in
                                // the def line, which this row mirrors.
                                div()
                                    .flex_1()
                                    .relative()
                                    .cursor(CursorStyle::IBeam)
                                    .child(
                                        capture_canvas(
                                            // Plain shared-cell write: never
                                            // entity.update() during a draw
                                            // pass (see zone_row_bounds).
                                            move |bounds, _, _| {
                                                row_bounds
                                                    .borrow_mut()
                                                    .insert(ix, bounds);
                                            },
                                            |_, _, _, _| {},
                                        )
                                        .absolute()
                                        .size_full(),
                                    )
                                    .child(def)
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(
                                            move |editor, ev: &MouseDownEvent, window, cx| {
                                                cx.stop_propagation();
                                                let off = editor
                                                    .zone_def_offset(
                                                        ix,
                                                        &def_text,
                                                        def_len,
                                                        ev.position.x,
                                                        window,
                                                    )
                                                    .unwrap_or(0);
                                                editor.goal_x = None;
                                                editor.set_cursor(def_start + off, false, cx);
                                            },
                                        ),
                                    ),
                            )
                    }))
                    .when(hidden > 0, |d| {
                        // Stacking policy: the rest collapse to a count.
                        d.child(
                            div()
                                .px(px(4.))
                                .opacity(0.7)
                                .child(format!("+{hidden} more")),
                        )
                    }),
            )
    }
}

/// One bottom-zone row (DESIGN §2-footnotes): painted number, the def's
/// text mirror, and the offsets its two click targets resolve to.
struct ZoneNote {
    /// Painted number (ref order), not the stored id.
    no: usize,
    /// Mirrored def text, truncated for the row.
    def: String,
    /// Byte offset of the def text's start — the edit-surface target.
    def_start: usize,
    /// Byte length of the def prefix the mirror shows (truncation-aware).
    def_len: usize,
    /// Just after the in-text ref — the marker's jump-back target.
    ref_byte: usize,
}

/// One rewind-list entry, materialized on entering history mode.
struct HistoryEntry {
    name: String,
    created_unix: i64,
    manual: bool,
    text: String,
    /// Formatting at this checkpoint — projected into the preview so the
    /// document doesn't strip to plain text while time-travelling.
    spans: SpanSet,
    blocks: BlockMap,
    /// (+words, -words) vs the previous checkpoint.
    delta: (usize, usize),
    /// Voice drift vs the writer's self-baseline, in LOO sigmas — set only
    /// when the baseline exists AND the checkpoint sits outside the normal
    /// range (>2σ). Scalars in the list; prose diff on the canvas.
    drift_sigma: Option<f32>,
}

/// History mode (DESIGN §2-history, the Docs/Figma hybrid): right side
/// panel with a two-tier list + the document as read-only diff canvas.
struct HistoryView {
    entries: Vec<HistoryEntry>,
    selected: usize,
    named_only: bool,
    /// false: diff vs previous checkpoint ("work of that session");
    /// true: diff vs the current draft ("what restoring would change").
    compare_current: bool,
    /// Expanded auto-checkpoint runs, keyed by the run's first entry index.
    /// Runs are collapsed by default (Figma's answer to autosave noise);
    /// arrow-stepping into one unfolds it.
    expanded: HashSet<usize>,
    /// A legacy file's states are still being materialized in the background
    /// (backfill_checkpoint_states): rows show names/dates only, the preview
    /// and restore wait. Once per file, ever — the states persist.
    materializing: bool,
}

/// First index of the run of consecutive auto checkpoints containing `ix`
/// (caller guarantees `entries[ix]` is automatic).
fn auto_group_start(entries: &[HistoryEntry], ix: usize) -> usize {
    let mut start = ix;
    while start > 0 && !entries[start - 1].manual {
        start -= 1;
    }
    start
}

/// The omnibox's three modes, chosen by the query's first character
/// (DESIGN §2-omnibox): `>` runs a command, `@` jumps to a heading, anything
/// else finds text. The prefix is stripped from the returned query slice
/// (and a leading space after it trimmed for command/heading — find keeps
/// every character, since spaces are searchable).
#[derive(Clone, Copy, PartialEq)]
enum OmniMode {
    Find,
    Command,
    Heading,
}

fn omni_mode(query: &str) -> (OmniMode, &str) {
    if let Some(rest) = query.strip_prefix('>') {
        (OmniMode::Command, rest.trim_start())
    } else if let Some(rest) = query.strip_prefix('@') {
        (OmniMode::Heading, rest.trim_start())
    } else {
        (OmniMode::Find, query)
    }
}

/// One omnibox row. Commands (and their "Frequent" badge, DESIGN §3.3) and
/// recent documents are the command mode; a find match carries its range +
/// a line snippet; a heading carries its byte offset + level.
enum OmniRow {
    Cmd(&'static crate::commands::Command),
    Frequent(&'static crate::commands::Command),
    Recent(std::path::PathBuf),
    Match {
        line: usize,
        snippet: String,
    },
    Heading {
        byte: usize,
        level: u8,
        text: String,
    },
}

/// The section a grouped-mode (bare `>`) row falls under — "" for rows that
/// never group (find/heading/command-with-query). Shared by `render_omni`
/// (to know when to insert a header) and `omni_child_index` (to know it did)
/// so the two can't drift apart.
fn omni_row_section(row: &OmniRow) -> &'static str {
    match row {
        OmniRow::Cmd(cmd) => cmd.section,
        OmniRow::Frequent(_) => "Frequent",
        OmniRow::Recent(_) => "Recent Documents",
        _ => "",
    }
}

/// Where row `ix` lands among the omni-list's direct children. Outside
/// grouped mode this is just `ix`; grouped mode (the bare `>` browsing view)
/// interleaves a section-header div ahead of each new section, so every row
/// after the first section boundary sits one or more children further along
/// than its index in `rows` — `render_omni` and `omni_scroll_into_view` both
/// need this mapping to agree, since one lays the headers out and the other
/// scrolls to a row assuming they're there.
fn omni_child_index(rows: &[OmniRow], grouped: bool, ix: usize) -> usize {
    let mut child = 0;
    let mut last_section = "";
    for (row_ix, row) in rows.iter().enumerate() {
        if grouped {
            let section = omni_row_section(row);
            if section != last_section {
                last_section = section;
                child += 1;
            }
        }
        if row_ix == ix {
            return child;
        }
        child += 1;
    }
    child
}

/// The AI surface's state machine (PLAN.md E3). Status lives where the
/// results will land — the margin — not in a toast or a titlebar whisper.
enum AiStatus {
    /// No provider configured: the empty state teaches setup. `local_model`
    /// is filled by the background Ollama probe — when present, the card
    /// offers a one-click, key-free, fully-local first pass (the cliff is
    /// gone for anyone running a local model).
    NeedsSetup { local_model: Option<String> },
    Running {
        label: String,
    },
    /// Success/info; fades after a few seconds.
    Note {
        title: String,
        detail: String,
    },
    /// Persistent until dismissed, retried, or superseded.
    Error {
        title: String,
        detail: String,
    },
}

/// App-side failure classes; the card copy names what failed and the
/// next action, never a bare status code.
enum AiFailure {
    Llm(strop_core::llm::LlmError),
    Parse(String),
}

/// The AI settings panel (DESIGN §2-ai, Kirill's mandate: provider setup
/// is the core onboarding task). The form holds live values; the config
/// file stays authoritative — Save writes through toml_edit so comments
/// and hand edits survive, and hand editing keeps working forever.
struct AiSettings {
    base_url: Entity<TextField>,
    api_key: Entity<TextField>,
    model: Entity<TextField>,
    /// Inline test/save feedback ON the panel — never a margin card while
    /// the panel is open (the margin is covered by the backdrop anyway).
    test: AiSettingsTest,
    /// GET /models result; the model field filters it, click/enter picks.
    models: Vec<String>,
    /// Selection inside the *filtered* list (up/down/enter).
    selected: usize,
    /// List-area message: "fetching…", "no models", or the fetch error.
    models_note: Option<String>,
}

enum AiSettingsTest {
    Idle,
    Running,
    Ok { ms: u64 },
    Failed { message: String },
}

/// Ollama's default OpenAI-compatible endpoint — the zero-key local path.
const LOCAL_OLLAMA_URL: &str = "http://localhost:11434/v1";

/// One opinionated provider per row (DESIGN principle 9: defaults are the
/// product). `key_url` is the page where a writer mints a key; None means
/// no key is needed (local) or the field is free (custom).
struct ProviderInfo {
    label: &'static str,
    base_url: &'static str,
    /// Substring that identifies this provider in an arbitrary base URL,
    /// so a hand-typed config still lights up the right chip and key link.
    host_match: &'static str,
    key_url: Option<&'static str>,
    /// Local-first first: no account, no key, private. Then the cloud
    /// paths, cheapest-to-start first.
    note: &'static str,
}

const PROVIDERS: &[ProviderInfo] = &[
    ProviderInfo {
        label: "Local (Ollama)",
        base_url: LOCAL_OLLAMA_URL,
        host_match: "11434",
        key_url: None,
        note: "no key, no account — your text never leaves this machine",
    },
    ProviderInfo {
        label: "OpenRouter",
        base_url: "https://openrouter.ai/api/v1",
        host_match: "openrouter.ai",
        key_url: Some("https://openrouter.ai/keys"),
        note: "one key, hundreds of models (several free)",
    },
    ProviderInfo {
        label: "Poe",
        base_url: "https://api.poe.com/v1",
        host_match: "api.poe.com",
        key_url: Some("https://poe.com/api_key"),
        note: "one subscription across Claude, GPT, Gemini",
    },
    ProviderInfo {
        label: "OpenAI",
        base_url: "https://api.openai.com/v1",
        host_match: "api.openai.com",
        key_url: Some("https://platform.openai.com/api-keys"),
        note: "GPT models direct from OpenAI",
    },
    ProviderInfo {
        label: "Custom",
        base_url: "",
        host_match: "",
        key_url: None,
        note: "any other OpenAI-compatible endpoint",
    },
];

/// The provider whose `host_match` is in `base_url` (so a hand-typed URL
/// still resolves). None for an empty/unrecognized URL.
fn provider_for(base_url: &str) -> Option<&'static ProviderInfo> {
    let url = base_url.trim();
    if url.is_empty() {
        return None;
    }
    PROVIDERS
        .iter()
        .find(|p| !p.host_match.is_empty() && url.contains(p.host_match))
}

/// Choose a model from a provider's /models list for the one-click local
/// path: skip obvious embedding-only models, else take the first.
fn pick_local_model(models: Vec<String>) -> Option<String> {
    models
        .iter()
        .find(|m| {
            let m = m.to_lowercase();
            !m.contains("embed") && !m.contains("bge") && !m.contains("rerank")
        })
        .or_else(|| models.first())
        .cloned()
}

fn host_of(base_url: &str) -> String {
    base_url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or(base_url)
        .to_owned()
}

fn env_key_set() -> bool {
    std::env::var("STROP_API_KEY").is_ok_and(|k| !k.is_empty())
}

/// STROP_API_KEY wins over whatever the form holds — the same precedence
/// `AiConfig::resolved_api_key` applies to the file.
fn resolved_key(field: &str) -> String {
    std::env::var("STROP_API_KEY")
        .ok()
        .filter(|k| !k.is_empty())
        .unwrap_or_else(|| field.to_owned())
}

impl AiFailure {
    fn into_status(self, base_url: &str, model: &str) -> AiStatus {
        use strop_core::llm::LlmError as E;
        let host = host_of(base_url);
        let (title, detail) = match self {
            Self::Llm(E::Auth(m)) => (format!("{host} rejected the API key"), m),
            Self::Llm(E::RateLimited(m)) => {
                (format!("Rate limited by {host} — try again in a moment"), m)
            }
            Self::Llm(E::Provider(m)) => (format!("{host} returned an error"), m),
            Self::Llm(E::Network(m)) => (format!("Couldn't reach {host}"), m),
            Self::Llm(E::Shape(m)) => (format!("Unusable reply from {model}"), m),
            Self::Parse(m) => (
                format!("{model} replied, but not in diagnosis format — usually a too-small model"),
                m,
            ),
        };
        AiStatus::Error { title, detail }
    }
}

/// The materialized preview: merged diff text + styled ranges (bytes).
/// Formatting is projected from both diff sides — kept/inserted content
/// carries the newer version's spans and block kinds, deleted content the
/// older version's — so history reads as the document, not as plain text.
#[derive(Clone)]
struct PreviewDoc {
    text: String,
    inserts: Vec<Range<usize>>,
    deletes: Vec<Range<usize>>,
    spans_bytes: Vec<(Range<usize>, InlineAttr)>,
    kinds: Vec<BlockKind>,
}

struct MarginLayout {
    /// Cards to render — packed and at least partly in view.
    cards: Vec<MarginCard>,
    /// Open cards hidden above / below the viewport — the SINGLE source of truth
    /// for both the edge-count pills AND `reveal_offscreen` (so a pill can never
    /// read "1 below" yet do nothing: the count is `below.len()` and clicking it
    /// navigates to `below`'s nearest entry). Each carries enough to navigate
    /// there. A card never vanishes without a trace (DESIGN principle 2).
    /// Door-held cards are NOT here; the rail (render_margin_rail) owns those.
    above: Vec<OffscreenRef>,
    below: Vec<OffscreenRef>,
}

/// An open card hidden past a viewport edge — enough to NAVIGATE to it, not just
/// tally it. `anchor_y` is the anchor's content-space y. `anchor_culled`
/// distinguishes the two ways a card hides, which need different reveals:
/// `true` = the anchor itself scrolled off-screen → reveal by SCROLLING to it;
/// `false` = the anchor is on-screen but packing pushed the card out → reveal by
/// SELECTING it (the packer's Pass 3 then forces the active card into view).
/// Deriving both pills and reveal from these kills the "count vs. reach computed
/// from different filters" bug class (the dead-pill + wrong-target findings).
#[derive(Clone, Copy)]
struct OffscreenRef {
    id: u64,
    anchor_y: f32,
    anchor_culled: bool,
}

#[derive(Clone)]
struct MarginCard {
    id: u64,
    top: f32,
    /// The anchor's content-space y (scroll-independent) — kept so a card the
    /// packer later pushes off-screen can still be navigated to by reveal.
    anchor_y: f32,
    height: f32,
    body: String,
    active: bool,
    kind: NoteKind,
    title: String,
    level: String,
    /// Anchor lost in a checkpoint restore (see `Annotation::orphaned`): the
    /// label gains a quiet "· detached" so a card sitting at a best-effort
    /// offset never reads as confidently anchored.
    orphaned: bool,
    /// A diagnosis whose flagged text was edited since it was raised — greyed
    /// as "unverified" (Annotation::unverified). Always false for writer notes.
    unverified: bool,
    /// Which AI pass raised it (0 for writer notes) — the recency the full-size
    /// budget sorts by when a crowded lane recedes older passes.
    pass_id: u64,
    /// Over the lane's full-size budget: render as a one-line card at the
    /// anchor (title only, muted) instead of the full card. Never true for
    /// writer notes or the selected card; `height` is COLLAPSED_CARD_H.
    collapsed: bool,
}

/// A note card's header label: "Note" / a diagnosis level (or "Diagnosis"),
/// with a quiet "· detached" when the anchor was lost in a restore.
fn note_card_label(is_diagnosis: bool, level: &str, orphaned: bool) -> String {
    let base = if is_diagnosis {
        if level.is_empty() { "Diagnosis" } else { level }
    } else {
        "Note"
    };
    if orphaned {
        format!("{base} · detached")
    } else {
        base.to_owned()
    }
}

/// Hash a card's identity-for-height: kind + title + body. Immutable for a
/// diagnosis; for a note it changes only at a composer commit — so a cache hit
/// means the stored measured height is still exact.
fn card_height_key(kind: NoteKind, title: &str, body: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    matches!(kind, NoteKind::Diagnosis).hash(&mut h);
    title.hash(&mut h);
    body.hash(&mut h);
    h.finish()
}

/// A margin card's placement inputs: its anchor target (viewport-space top it
/// wants), measured height, whether it's a hard PIN (a writer note or the
/// active card — holds its anchor) and whether it's the ACTIVE/selected card.
#[derive(Clone, Copy, Debug)]
struct PlaceItem {
    anchor: f32,
    height: f32,
    pin: bool,
    active: bool,
}

/// Place margin cards in ONE pass and return each card's top (viewport-space),
/// in input order. Items come in document/anchor order. Three guarantees,
/// pinned down by the packer proptests:
///
/// 1. no two cards EVER overlap (the writer's never-overlap rule, no excuses);
/// 2. every card sits at/below `floor` (never under the titlebar) — EXCEPT a
///    card the active card displaced upward off the top edge, which the caller
///    culls into the `above` count (so it is never painted under the titlebar);
/// 3. the ACTIVE card lies fully within `[floor, viewport_bottom]` whenever
///    its height fits there — UNCONDITIONALLY, even when a pinned writer note
///    competes for the slack above it: the focused card wins the lane.
///
/// Mechanics: writer notes (Layer A) and the active card hold their anchors;
/// inactive diagnoses (Layer B) yield around them. The active card's anchor is
/// first clamped UP so the whole card fits the viewport (the "selected card ran
/// off the bottom edge" bug). A floor+downward sweep gives the non-overlap
/// guarantee; a pull-up then slides the rigid run of movable cards above each
/// pin up into existing slack only. Pass 3 then re-clamps the active card into
/// view if a competing pin pushed it below, and shoves the run above it UP to
/// stay clear — displacing (and honestly counting) rather than overlapping.
fn place_margin_cards(items: &[PlaceItem], floor: f32, viewport_bottom: f32, gap: f32) -> Vec<f32> {
    let n = items.len();
    // Anchor targets (floored); the active card is clamped up to fit the lane.
    let anchor: Vec<f32> = items
        .iter()
        .map(|it| {
            let a = it.anchor.max(floor);
            if it.active {
                (viewport_bottom - it.height - CARD_BOTTOM_MARGIN).min(a).max(floor)
            } else {
                a
            }
        })
        .collect();
    // Pass 1 — floor + downward no-overlap sweep.
    let mut top = vec![floor; n];
    let mut bottom = floor;
    for i in 0..n {
        top[i] = anchor[i].max(bottom);
        bottom = top[i] + items[i].height + gap;
    }
    // Pass 2 — raise each pin toward its anchor, COMPRESSING the movable run
    // directly above it into its internal slack (not a rigid slide — that left
    // loose gaps between spread-out cards unused and stranded the selected card
    // off the bottom edge). A pin never rises past the floor or a pinned note
    // above it. Bottom-up so a lower pin makes room before a higher one runs.
    for i in (0..n).rev() {
        if !items[i].pin || top[i] <= anchor[i] {
            continue;
        }
        // The movable run directly above pin i is [k, i); `base` is the floor or
        // the bottom of the nearest pin above (which holds its own anchor).
        let mut k = i;
        while k > 0 && !items[k - 1].pin {
            k -= 1;
        }
        let base = if k > 0 {
            top[k - 1] + items[k - 1].height + gap
        } else {
            floor
        };
        let need: f32 = items[k..i].iter().map(|it| it.height + gap).sum();
        // Highest pin i may sit: its anchor, unless the run above needs the room
        // (then it sits just low enough that the run still clears the floor).
        top[i] = anchor[i].max(base + need).min(top[i]);
        // Pack the run beneath it: keep each card where it is when there's slack,
        // push it up only as far as avoiding overlap demands.
        let mut limit = top[i];
        for j in (k..i).rev() {
            let cap = limit - items[j].height - gap;
            if top[j] > cap {
                top[j] = cap;
            }
            limit = top[j];
        }
    }
    // Pass 3 — the active card MUST be fully in view: it is what the writer is
    // looking at right now. Passes 1-2 can still strand it below the fold when a
    // pinned writer note above eats the slack (the old guarantee-3 carve-out).
    // The focused card wins WITHOUT overlapping anything (the never-overlap rule
    // holds): re-clamp its top up into [floor, viewport_bottom - height], then
    // shove the run directly above it UP to stay clear. A card shoved past the
    // floor is off the top edge — the caller culls it into the honest `above`
    // count (it becomes "N above", never overlapped, never over the titlebar).
    if let Some(a) = items.iter().position(|it| it.active) {
        let ceil = (viewport_bottom - items[a].height - CARD_BOTTOM_MARGIN).max(floor);
        if top[a] > ceil {
            top[a] = ceil;
        }
        // Cascade upward: each card above keeps clear of the one below it, the
        // active card being the fixed lower bound. Cards run off the top as
        // needed (they get culled + counted, not clipped or overlapped).
        let mut limit = top[a];
        for i in (0..a).rev() {
            let cap = limit - items[i].height - gap;
            if top[i] > cap {
                top[i] = cap;
            }
            limit = top[i];
        }
    }
    top
}

/// Where a packed card sits relative to the viewport: rendered (`Shown`), or
/// rolled into the above/below edge count. PURE GEOMETRY — a card shows iff at
/// least one line of it overlaps the viewport. No `active` special case: the
/// packer (Pass 3) guarantees the active card is in view, so it shows by
/// geometry like everything else; and if it somehow can't fit (taller than the
/// lane) this counts it honestly instead of lying "Shown" while it's off-screen.
/// That honesty is what keeps the "N above / N below" counts trustworthy
/// (nothing vanishes) and what reveal_offscreen relies on to find a card.
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
enum CardSlot {
    Shown,
    Above,
    Below,
}

fn card_slot(top: f32, height: f32, vp_top: f32, vp_bottom: f32) -> CardSlot {
    if top + height > vp_top + CARD_LINE_H && top < vp_bottom - CARD_LINE_H {
        CardSlot::Shown
    } else if top + height <= vp_top + CARD_LINE_H {
        CardSlot::Above
    } else {
        CardSlot::Below
    }
}

/// The scroll offset that brings a card anchored at content-y `anchor_y` to the
/// NEAR edge of the viewport — just into view, not a page. `below` reveals it at
/// the bottom edge (anchor lands `REVEAL_INSET` above the bottom, leaving room
/// for the card), `above` at the top edge. Clamped to the scrollable range.
/// Pure, so the "pill reveals one more card, never paginates" property is unit-
/// testable: after a `below` reveal the anchor sits near the BOTTOM, not the top.
fn reveal_scroll(anchor_y: f32, vp_h: f32, max_scroll: f32, below: bool) -> f32 {
    let target = if below {
        anchor_y - vp_h + REVEAL_INSET
    } else {
        anchor_y - REVEAL_INSET
    };
    target.clamp(0., max_scroll)
}

/// Of the lane's diagnoses `(id, pass_id)`, the ones past the full-size budget:
/// keep the newest `cap` (highest pass_id, then id) full, the rest recede to
/// one-line cards. Pure, so the budget policy is unit-testable without a frame.
fn oldest_beyond_cap(surfaced: &[(u64, u64)], cap: usize) -> std::collections::HashSet<u64> {
    if surfaced.len() <= cap {
        return std::collections::HashSet::new();
    }
    let mut v = surfaced.to_vec();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.0.cmp(&a.0)));
    v.into_iter().skip(cap).map(|(id, _)| id).collect()
}

/// A freshly-landed card's entrance: one decelerating opacity fade over
/// CARD_APPEAR (cubic ease-out ≈ the "enter" token, attention-motion.md §3).
/// `is_new` holds only while the landing pass is inside its fade window
/// (Editor::appearing), so a card scrolled out and back re-mounts WITHOUT
/// replaying the entrance — old content never re-announces itself.
fn appear_fade<E: Styled + IntoElement + 'static>(el: E, id: u64, is_new: bool) -> gpui::AnyElement {
    if !is_new {
        return el.into_any_element();
    }
    el.with_animation(
        ("card-appear", id as usize),
        Animation::new(CARD_APPEAR).with_easing(|t| 1. - (1. - t).powi(3)),
        |el, t| el.opacity(t),
    )
    .into_any_element()
}

/// One card's share of a staggered re-pack round: flat 0 through its delay
/// (`delay_frac` of the whole run), then a cubic ease-in-out over the rest —
/// the "standard" in-place token. Pure, so the timing contract (still during
/// the delay, settled at 1, monotone between) is a unit test.
fn staggered_ease(t: f32, delay_frac: f32) -> f32 {
    if t <= delay_frac {
        return 0.;
    }
    if t >= 1. {
        return 1.;
    }
    let t = (t - delay_frac) / (1. - delay_frac);
    if t < 0.5 {
        4. * t * t * t
    } else {
        1. - (-2. * t + 2.).powi(3) / 2.
    }
}

/// Diff the lane between two frames: which cards MOVED — present in both with
/// a changed content-space top. `snap` (a scroll frame, a live composer, a
/// typing burst — continuous causes, not discrete re-packs) reports nothing:
/// those frames track 1:1 and must never animate. Newly-appeared ids are not
/// moves (they get the entrance fade); departed ids just drop. Pure, so the
/// tween-vs-snap policy is a unit test, not a feel regression.
fn plan_lane_moves(
    prev: &std::collections::HashMap<u64, f32>,
    now: &[(u64, f32)],
    snap: bool,
) -> Vec<(u64, f32, f32)> {
    if snap {
        return Vec::new();
    }
    now.iter()
        .filter_map(|&(id, top)| {
            let from = *prev.get(&id)?;
            ((from - top).abs() > 0.5).then_some((id, from, top))
        })
        .collect()
}

/// Wrap a mid-move card: the sliding mode animates its `top` from the old
/// slot to the CURRENT target (so scroll under a live tween stays 1:1); the
/// reduce_motion mode keeps the card AT the target and fades it in over the
/// same duration instead (its old-slot ghost fades out in render_margin) —
/// travel becomes a cross-fade, never a teleport. The animation is keyed by
/// (id, generation) so a mid-flight re-target restarts the run; note ids stay
/// far below 2^48, so the packed key can't collide.
fn move_slide<E: Styled + IntoElement + 'static>(
    el: E,
    id: u64,
    mv: &CardMove,
    target_top: f32,
    reduce_motion: bool,
) -> gpui::AnyElement {
    let key = ("card-move", (id as usize) | ((mv.generation as usize) << 48));
    let total = mv.total();
    let delay_frac = mv.delay.as_secs_f32() / total.as_secs_f32();
    let delta = mv.delta;
    let anim = Animation::new(total).with_easing(move |t| staggered_ease(t, delay_frac));
    if reduce_motion {
        el.with_animation(key, anim, |el, t| el.opacity(t)).into_any_element()
    } else {
        el.with_animation(key, anim, move |el, t| {
            el.top(px((target_top + delta * (1. - t)).max(4.)))
        })
        .into_any_element()
    }
}

/// A card's non-interactive ghost (no id, no listeners — a click lands on
/// whatever is really there): the shared body for the exit fade (departing)
/// and the reduce_motion cross-fade. Mirrors the live card's look — the
/// receded one-liner included — so the fading light reads as the same object,
/// and a collapsed card's ghost never sprawls over its neighbours.
fn ghost_card(card: &MarginCard) -> gpui::Div {
    let is_diagnosis = card.kind == NoteKind::Diagnosis;
    let base = div()
        .absolute()
        .top(px(card.top.max(4.)))
        .left(px(8.))
        .w(px(MARGIN_WIDTH - 8.))
        .overflow_hidden()
        .rounded(px(if is_diagnosis { 3. } else { 9. }))
        .bg(rgb(if card.unverified {
            STALE_BG
        } else if is_diagnosis {
            DIAGNOSIS_CARD_BG
        } else {
            NOTE_CARD_BG
        }))
        .border_1()
        .border_color(rgb(RULE_COLOR))
        .font_family("PT Serif")
        .text_color(rgb(MUTED_COLOR));
    if card.collapsed {
        return base
            .h(px(COLLAPSED_CARD_H))
            .px(px(8.))
            .py(px(3.))
            .text_size(px(11.))
            .line_height(px(COLLAPSED_CARD_H - 8.))
            .child(div().truncate().child(if card.title.is_empty() {
                card.level.clone()
            } else {
                card.title.clone()
            }));
    }
    base.p(px(8.))
        .text_size(px(13.))
        .line_height(px(CARD_LINE_H))
        .when(is_diagnosis && !card.title.is_empty(), |d| {
            d.child(div().font_weight(FontWeight::BOLD).child(card.title.clone()))
        })
        .child(div().child(card.body.clone()))
}

/// Which open annotation a click at char index `c` activates, given each open
/// note's `(id, start, end)` (anchor covers `[start, end)`). A click snaps to
/// the nearest caret boundary, so a click on the trailing half of the last glyph
/// lands at `c == end` — still on the painted mark. So: prefer the anchor that
/// strictly CONTAINS `c`; failing that, accept one that ENDS exactly at `c`. The
/// containment check runs first, so a back-to-back `[..,c)[c,..)` pair resolves
/// to the second (it contains `c`) and the trailing fallback never double-claims.
/// Pure, so the half-glyph trailing-edge dead-zone is a unit test, not a surprise.
fn note_at_char(ranges: &[(u64, usize, usize)], c: usize) -> Option<u64> {
    ranges
        .iter()
        .find(|(_, s, e)| *s <= c && c < *e)
        .or_else(|| ranges.iter().find(|(_, _, e)| *e == c))
        .map(|(id, _, _)| *id)
}

/// The door (DESIGN §4.4): does this open note surface as a margin card right
/// now? Writer notes always do; a diagnosis is hidden while drafting, and a
/// copy-level one is held back while a developmental one is still open (the
/// mandatory altitude order). The held-back ones surface as the rail's count.
fn note_surfaces(kind: NoteKind, level: &str, drafting: bool, has_dev: bool) -> bool {
    kind != NoteKind::Diagnosis || (!drafting && (!has_dev || level != "copy"))
}

/// The editor menu's copy-row gate (impl 04 §0), pure so its RELEASE is unit-
/// tested: gated iff any OPEN diagnosis sits at the developmental level. Keyed
/// off the same signal as `note_surfaces`'s copy-hold, so the request gate and
/// the result hold can never disagree. Releases the instant the last
/// developmental query closes — no pass re-run.
fn copy_gate_active<'a>(open_notes: impl Iterator<Item = (NoteKind, &'a str)>) -> bool {
    open_notes.filter(|(k, _)| *k == NoteKind::Diagnosis).any(|(_, level)| level == "developmental")
}

impl Editor {
    /// Shape `text` at the card's inner width and return its REAL wrapped
    /// height (the measurement that replaced the `chars/30` estimate). Embedded
    /// newlines and multi-row wraps are summed; empty text is zero.
    fn shape_text_height(window: &Window, text: &str) -> f32 {
        Self::shape_text_height_w(window, text, CARD_INNER_W, false)
    }

    /// Wrapped height of `text` at a given inner width, in PT Serif 13 / one
    /// shaped row per painted row. Committed body text wraps at `CARD_INNER_W`;
    /// the live composer wraps at the narrower `COMPOSER_INNER_W` (its box has
    /// padding), so its reservation matches what it paints. `bold` MUST match the
    /// paint weight: a diagnosis title is painted bold, and bold advances are
    /// wider, so measuring it normal-weight under-reserves a row at the wrap
    /// boundary and the next card overlaps it — measure exactly what we paint.
    fn shape_text_height_w(window: &Window, text: &str, width: f32, bold: bool) -> f32 {
        if text.is_empty() {
            return 0.;
        }
        let s = SharedString::from(text.to_owned());
        let mut font = gpui::font("PT Serif");
        if bold {
            font.weight = FontWeight::BOLD;
        }
        let run = TextRun {
            len: s.len(),
            font,
            color: rgb(TEXT_COLOR).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        match window
            .text_system()
            .shape_text(s, px(13.), &[run], Some(px(width)), None)
        {
            Ok(lines) => lines
                .iter()
                .map(|l| f32::from(l.size(px(CARD_LINE_H)).height))
                .sum(),
            Err(_) => CARD_LINE_H,
        }
    }

    /// A card's painted height: chrome + a wrapped title (diagnoses only) + a
    /// wrapped body, with at least `min_body_rows` of body/composer room.
    fn measure_card_height(window: &Window, title: &str, body: &str, min_body_rows: f32) -> f32 {
        let body_h = Self::shape_text_height(window, body).max(min_body_rows * CARD_LINE_H);
        // The title is painted BOLD (diagnoses only) — measure it bold to match.
        let title_h = Self::shape_text_height_w(window, title, CARD_INNER_W, true);
        CARD_CHROME_H + title_h + body_h
    }

    /// Measure-and-cache every open card's height while the window's text system
    /// is in hand (called from `render`), so `margin_cards`'s placement runs on
    /// real extents, not estimates. Committed/immutable content is cached by
    /// hash; the one actively-composed note is measured live each frame from its
    /// composer (its text changes every keystroke, so it can't be cached).
    fn refresh_card_heights(&mut self, window: &Window, cx: &mut Context<Self>) {
        // The live card is the one being COMPOSED (not merely selected): its id
        // and its draft text come from the same `Composing` variant, so the
        // live height can't be measured against the wrong card.
        let composing_id = self.focus.composing_id();
        let live = self.focus.input().map(|i| i.read(cx).content.clone());
        // Collect specs first: can't hold a `doc` borrow while mutating the cache.
        let specs: Vec<(u64, NoteKind, String, String)> = self
            .doc
            .notes()
            .open()
            .map(|n| (n.id, n.kind, n.title.clone(), n.body.clone()))
            .collect();
        self.active_card_height = None;
        for (id, kind, title, body) in specs {
            let is_diag = kind == NoteKind::Diagnosis;
            if Some(id) == composing_id && !is_diag {
                // The live composer wraps at its (narrower) box width and adds
                // the box's own vertical chrome; reserve exactly that so the
                // growing field never clips or overlaps the card below.
                let body = live.as_deref().unwrap_or("");
                let body_h = Self::shape_text_height_w(window, body, COMPOSER_INNER_W, false)
                    .max(2. * CARD_LINE_H);
                self.active_card_height = Some(CARD_CHROME_H + body_h + COMPOSER_BOX_CHROME);
                continue;
            }
            let key = card_height_key(kind, &title, &body);
            self.card_heights.entry(key).or_insert_with(|| {
                let (t, min_rows) = if is_diag { (title.as_str(), 0.) } else { ("", 1.) };
                Self::measure_card_height(window, t, &body, min_rows)
            });
        }
    }

    /// Build the margin cards: door-filter the open notes, measure + place them
    /// (`place_margin_cards`), and — when `cull` is set (the wide lane) — drop
    /// those whose anchor is off-screen or that packing pushed out of view,
    /// reporting their counts as honest `above`/`below` edges so nothing
    /// vanishes silently. `cull = false` (the narrow drawer) returns every open
    /// card with no edge counts; the drawer ignores positions.
    fn margin_cards(&self, cull: bool) -> MarginLayout {
        let Some(frame) = self.last_frame.as_ref() else {
            return MarginLayout {
                cards: Vec::new(),
                above: Vec::new(),
                below: Vec::new(),
            };
        };
        let rope = self.doc.rope();
        let len = self.doc.len_bytes();
        let mut cards: Vec<MarginCard> = Vec::new();
        // Open cards hidden above / below the viewport — surfaced as edge pills
        // AND navigated by reveal_offscreen (one source of truth).
        let mut above: Vec<OffscreenRef> = Vec::new();
        let mut below: Vec<OffscreenRef> = Vec::new();
        // The door (DESIGN §4.4): drafting hides the editor's cards (the
        // writer's own notes stay); reviewing shows them, but suppresses
        // copy-level cards while a developmental one is still open (the
        // mandatory altitude order). Either way the held-back count surfaces
        // in the rail (render_margin_rail) — nothing vanishes silently.
        let drafting = self.drafting;
        let has_dev = !drafting
            && self
                .doc
                .notes()
                .open()
                .any(|n| n.kind == NoteKind::Diagnosis && n.level == "developmental");
        for n in self.doc.notes().open() {
            // The SELECTED card is exempt from the door: clicking a diagnosis is
            // an explicit attention act that overrides the altitude-order hold
            // (and mirrors the anchor-cull exemption below) — otherwise the click
            // would light the anchor while the card stayed suppressed, a
            // highlight with no card (the reported "click shows no card" bug).
            let active = self.focus.active_id() == Some(n.id);
            if !active && !note_surfaces(n.kind, &n.level, drafting, has_dev) {
                continue;
            }
            let byte = rope.char_to_byte(n.range.start.min(rope.len_chars())).min(len);
            let Some(pos) = frame.position_of(byte, false) else {
                continue;
            };
            let desired =
                f32::from(frame.bounds.origin.y) + f32::from(pos.y) - f32::from(frame.scroll_top);
            let is_diag = n.kind == NoteKind::Diagnosis;
            // Cull to the viewport: a card whose ANCHOR is off-screen doesn't
            // belong in the lane — it would pile at the floor (the scroll-pileup
            // bug) and attribute to nothing visible. The active card is exempt:
            // you're working it, so it stays even if its anchor scrolled away.
            let vp_top = f32::from(frame.bounds.origin.y);
            let vp_bot = vp_top + f32::from(frame.bounds.size.height);
            if cull
                && !active
                && (desired < vp_top - CARD_OVERSCAN || desired > vp_bot + CARD_OVERSCAN)
            {
                // Anchor itself off-screen → reveal by scrolling to anchor_y.
                let r = OffscreenRef {
                    id: n.id,
                    anchor_y: f32::from(pos.y),
                    anchor_culled: true,
                };
                if desired < vp_top {
                    above.push(r);
                } else {
                    below.push(r);
                }
                continue;
            }
            // Real MEASURED height (refresh_card_heights), never an estimate.
            // The active composer rides a live field; every other card reads the
            // content-hash cache, with a one-frame char-count fallback for a
            // brand-new card the refresh hasn't measured yet.
            let height = if self.focus.composing_id() == Some(n.id) {
                self.active_card_height
                    .unwrap_or(CARD_CHROME_H + 2. * CARD_LINE_H)
            } else {
                let key = card_height_key(n.kind, &n.title, &n.body);
                self.card_heights.get(&key).copied().unwrap_or_else(|| {
                    let body_rows = (n.body.chars().count() as f32 / 30.)
                        .ceil()
                        .max(if is_diag { 0. } else { 1. });
                    let title_rows = if is_diag && !n.title.is_empty() {
                        (n.title.chars().count() as f32 / 30.).ceil()
                    } else {
                        0.
                    };
                    CARD_CHROME_H + (title_rows + body_rows) * CARD_LINE_H
                })
            };
            cards.push(MarginCard {
                id: n.id,
                top: desired,
                anchor_y: f32::from(pos.y),
                height,
                body: n.body.clone(),
                active,
                kind: n.kind,
                title: n.title.clone(),
                level: n.level.clone(),
                orphaned: n.orphaned,
                unverified: n.unverified,
                pass_id: n.pass_id,
                collapsed: false,
            });
        }
        // The full-size budget (FULL_DIAGNOSIS_CAP): among the diagnoses that
        // made it into THIS lane, the newest few render full and older passes
        // RECEDE to a one-line card at their anchor — present, clickable,
        // smaller. Counted lane-local (after the anchor cull) so a crowded page
        // elsewhere never shrinks this one, and never hidden: every flagged
        // passage in view keeps a visible card (the honesty invariant; a
        // squiggle with no card reads as a bug, and was reported as one). The
        // selected card is exempt, so clicking a receded card expands it. Lane
        // presentation only — the narrow drawer (cull=false) lists all cards.
        if cull {
            let surfaced: Vec<(u64, u64)> = cards
                .iter()
                .filter(|c| c.kind == NoteKind::Diagnosis && !c.active)
                .map(|c| (c.id, c.pass_id))
                .collect();
            let receded = oldest_beyond_cap(&surfaced, FULL_DIAGNOSIS_CAP);
            for card in &mut cards {
                if receded.contains(&card.id) {
                    card.collapsed = true;
                    card.height = COLLAPSED_CARD_H;
                }
            }
        }
        // Place them in one pass (see `place_margin_cards`): writer notes and
        // the active card hold their anchors, inactive diagnoses yield around
        // them, the selected card is kept fully in view, and no two overlap.
        // `top` currently holds each card's anchor target.
        let floor = BAR_HEIGHT + 8.;
        let viewport_bottom = f32::from(frame.bounds.origin.y + frame.bounds.size.height);
        let items: Vec<PlaceItem> = cards
            .iter()
            .map(|c| PlaceItem {
                anchor: c.top,
                height: c.height,
                pin: c.kind == NoteKind::Note || c.active,
                active: c.active,
            })
            .collect();
        for (card, top) in cards
            .iter_mut()
            .zip(place_margin_cards(&items, floor, viewport_bottom, MARGIN_GAP))
        {
            card.top = top;
        }
        if !cull {
            return MarginLayout { cards, above, below };
        }
        // Packing can shove an on-screen-anchored card off an edge (the active
        // card wins the bottom slot and displaces the run above it up; or a run
        // below an active card overflows the bottom). Record those, never clip
        // them silently: keep only cards with a real slice in view, capture the
        // rest as off-screen refs (anchor_culled = false → reveal by selecting,
        // since their anchor is on-screen and scrolling won't help).
        let vp_top = f32::from(frame.bounds.origin.y);
        let vp_bottom = f32::from(frame.bounds.origin.y + frame.bounds.size.height);
        let mut visible = Vec::with_capacity(cards.len());
        for card in cards {
            let packed_off = OffscreenRef {
                id: card.id,
                anchor_y: card.anchor_y,
                anchor_culled: false,
            };
            // A card Pass 3 pushed above the floor (to clear the active card) is
            // off the TOP — count it, don't paint it over the titlebar. (Pass 3
            // only moves cards up, so `top < floor` uniquely marks the displaced.)
            if card.top < floor - 0.5 {
                above.push(packed_off);
                continue;
            }
            match card_slot(card.top, card.height, vp_top, vp_bottom) {
                CardSlot::Shown => visible.push(card),
                CardSlot::Above => above.push(packed_off),
                CardSlot::Below => below.push(packed_off),
            }
        }
        MarginLayout {
            cards: visible,
            above,
            below,
        }
    }

    /// Render pre-pass (once per frame, before `render_margin`): diff the
    /// packed lane against the previous frame and start, re-target, or expire
    /// the re-pack slides (`moving`). One rule decides tween vs. snap: a
    /// DISCRETE re-pack in a still lane slides (a card resolved, a pass
    /// landed, a selection expanded — object constancy pays); any CONTINUOUS
    /// cause — scroll, a live composer growing, a typing burst reflowing
    /// anchors — tracks 1:1 and clears all motion (never animate against the
    /// writer's own movement, never mid-burst; attention-motion.md §2-3).
    fn update_lane_motion(&mut self, cx: &mut Context<Self>) {
        let Some(frame) = self.last_frame.as_ref() else {
            return;
        };
        let scroll = f32::from(frame.scroll_top);
        let scrolled = (scroll - self.lane_scroll).abs() > 0.5;
        self.lane_scroll = scroll;
        let viewport = (
            f32::from(frame.bounds.size.width),
            f32::from(frame.bounds.size.height),
        );
        let resized = (viewport.0 - self.lane_viewport.0).abs() > 0.5
            || (viewport.1 - self.lane_viewport.1).abs() > 0.5;
        self.lane_viewport = viewport;
        let layout = self.margin_cards(true);
        // Content-space tops (scroll added back), so a pure scroll diffs to
        // zero and only real re-packs register.
        let now_tops: Vec<(u64, f32)> =
            layout.cards.iter().map(|c| (c.id, c.top + scroll)).collect();
        // Finished slides settle; a card that left the lane drops its motion
        // state (so a later return can never replay a stale move).
        let present: std::collections::HashSet<u64> =
            now_tops.iter().map(|&(id, _)| id).collect();
        self.moving
            .retain(|id, mv| present.contains(id) && mv.start.elapsed() < mv.total());
        self.lane_tops.retain(|id, _| present.contains(id));
        let snap =
            scrolled || resized || self.focus.composing_id().is_some() || self.typing_burst_live();
        if snap {
            self.moving.clear();
            self.lane_tops = now_tops.into_iter().collect();
            return;
        }
        let mut moves = plan_lane_moves(&self.lane_tops, &now_tops, false);
        // Stagger top-down by destination, capped so a long train stays one
        // brisk gesture, not a slow wave.
        moves.sort_by(|a, b| a.2.total_cmp(&b.2));
        let reduce = self.config.reduce_motion;
        let mut started = false;
        for (i, &(id, from, to)) in moves.iter().enumerate() {
            started = true;
            if let Some(mv) = self.moving.get_mut(&id) {
                // Mid-flight and the pack moved again: head for the new
                // target from the currently-DISPLAYED spot (no snap-back),
                // on a fresh animation run. The old-slot ghost (reduce
                // mode) is dropped — a churning lane earns less light.
                let displayed = mv.to + mv.delta * (1. - mv.eased(mv.start.elapsed()));
                mv.delta = displayed - to;
                mv.to = to;
                mv.start = Instant::now();
                mv.delay = Duration::ZERO;
                mv.generation += 1;
                mv.ghost = None;
                continue;
            }
            // reduce_motion: travel becomes a cross-fade — snapshot the card
            // at its old slot (content-space top; render re-anchors it to the
            // live scroll) to fade out under the live card fading in.
            let ghost = reduce
                .then(|| {
                    layout.cards.iter().find(|c| c.id == id).map(|c| {
                        let mut g = c.clone();
                        g.top = from;
                        g
                    })
                })
                .flatten();
            self.moves_started += 1;
            self.moving.insert(
                id,
                CardMove {
                    delta: from - to,
                    to,
                    start: Instant::now(),
                    delay: MOVE_STAGGER * (i as u32).min(MOVE_STAGGER_CAP),
                    generation: 0,
                    ghost,
                },
            );
        }
        for (id, top) in now_tops {
            self.lane_tops.insert(id, top);
        }
        // One nudge after the longest slide lands: with_animation stops
        // requesting frames at t=1, so without this a finished entry would
        // linger until the next natural frame (and could replay if its card
        // re-mounts after a door toggle).
        if started {
            let wait = self.moving.values().map(CardMove::total).max().unwrap_or(CARD_MOVE)
                + Duration::from_millis(60);
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(wait).await;
                this.update(cx, |editor: &mut Editor, cx| {
                    editor
                        .moving
                        .retain(|_, mv| mv.start.elapsed() < mv.total());
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
    }

    /// Width the column and note lane actually have to live in: the viewport
    /// MINUS the CSD shadow gutter on each untiled edge. Client decorations
    /// inset the content by `CSD_GUTTER` a side, so the raw viewport overcounts
    /// by ~44px on a floating window — the bug that let the lane overrun the
    /// content's right edge and clip the cards. Tiled/server windows have no
    /// gutter, so this is just the viewport there (why the tiled rig missed it).
    fn content_width(&self, window: &Window) -> f32 {
        let vw = f32::from(window.viewport_size().width);
        match window.window_decorations() {
            Decorations::Client { tiling } => {
                let l = if tiling.left { 0. } else { CSD_GUTTER };
                let r = if tiling.right { 0. } else { CSD_GUTTER };
                vw - l - r
            }
            Decorations::Server => vw,
        }
    }

    fn margin_fits(&self, window: &Window) -> bool {
        if self.history_view.is_some() {
            return false; // history displaces the lane wholesale
        }
        let cw = self.content_width(window);
        let (left, w) = self.column_frame(window);
        cw - (left + w) >= NOTE_LANE_TOTAL
    }

    /// The door rail's count, when it has something to hold: drafting hides the
    /// editor's diagnoses ("N resting"); reviewing holds copy-level cards under
    /// an open developmental one ("N copy-level"). The DOOR is the only thing
    /// the rail counts — the full-size budget never hides a card (over-budget
    /// ones recede to one-line cards, still in the lane), so it owes the rail
    /// nothing. `None` means the rail stands down — a quiet margin.
    fn margin_rail_count(&self) -> Option<usize> {
        let n = if self.drafting {
            self.resting_diagnoses()
        } else {
            self.suppressed_copy()
        };
        (n > 0).then_some(n)
    }

    /// Cheap emptiness predicate for `lane_has_content`: would ANY open note
    /// surface as a margin card right now? Mirrors `margin_cards`'s door filter
    /// (drafting hides diagnoses; an open developmental card suppresses copy
    /// ones) but skips positioning and height-estimating every card — work
    /// `column_frame` (several calls per render) never needed just to test
    /// whether the lane is occupied.
    fn has_margin_cards(&self) -> bool {
        if self.last_frame.is_none() {
            return false;
        }
        let drafting = self.drafting;
        let has_dev = !drafting
            && self
                .doc
                .notes()
                .open()
                .any(|n| n.kind == NoteKind::Diagnosis && n.level == "developmental");
        self.doc
            .notes()
            .open()
            .any(|n| note_surfaces(n.kind, &n.level, drafting, has_dev))
    }

    /// Does anything want the right-hand note lane right now? An empty lane
    /// never pulls the column off-centre — the column only shifts in the
    /// service of cards that would otherwise have nowhere to go.
    fn lane_has_content(&self) -> bool {
        self.has_margin_cards()
            || self.margin_rail_count().is_some()
            || self.ai_status.is_some()
    }

    /// The prose column's geometry — (left inset, width) — as a function of
    /// viewport width. This is the no-jump invariant in code: at rest the
    /// column is CENTRED (rhyming with the centred omnibox), and it stays
    /// centred while the right margin can host the note lane. The outline never
    /// enters this — it overlays, so toggling it can't move the column — and
    /// resizing slides everything continuously with no breakpoint snap.
    ///
    /// - No notes (or history): centred at every width.
    /// - Notes, WIDE: still centred — the lane lives in the right margin.
    /// - Notes, NARROWING: once centring would push the lane off the right
    ///   edge, the column shifts left exactly enough to keep the lane (the two
    ///   formulas meet at the crossover, so no jump), down to COL_LEFT_MIN.
    /// - Notes, NARROW: below that the lane can't fit; notes go to the pill and
    ///   the column stays stuck left (continuous with the shift above).
    fn column_frame(&self, window: &Window) -> (f32, f32) {
        // Measured in CONTENT space (the column lives inside the inset content),
        // so centring here lands the column centred in the *visible* window.
        let cw = self.content_width(window);
        let w = COL_MAX_WIDTH.min((cw - 2. * COL_LEFT_MIN).max(DOC_MIN_WIDTH.min(cw)));
        let centred = ((cw - w) / 2.).max(COL_LEFT_MIN);
        if self.history_view.is_some() || !self.lane_has_content() {
            return (centred, w);
        }
        // Keep the lane in the right margin: cap the left inset so the right
        // margin is never smaller than the lane. `shifted` ≤ `centred` exactly
        // when centring's right margin < lane, so `min` gives a seamless
        // centred→shifted handoff; the floor parks it left for the pill.
        let shifted = cw - w - NOTE_LANE_TOTAL;
        (shifted.min(centred).max(COL_LEFT_MIN), w)
    }

    /// The column's right edge in CONTENT space — where the note lane begins.
    /// Derived from `column_frame` (the SAME basis the column is laid out with),
    /// NOT from last frame's `frame.bounds`. That's the fix for two things: the
    /// lane no longer lags the column by a frame during a resize (the jitter —
    /// column + lane now slide as one slab, since the column's width is constant
    /// so nothing reflows), and there's no gutter-offset drift between the fit
    /// check and the actual placement.
    fn column_right(&self, window: &Window) -> f32 {
        let (left, w) = self.column_frame(window);
        left + w
    }

    /// Narrow-window composer: the margin (and its in-card composer) is
    /// hidden, so the note body is edited in a bottom strip instead.
    fn render_composer_strip(&self) -> Option<impl IntoElement> {
        let input = self.focus.input().cloned()?;
        Some(
            div()
                .absolute()
                .bottom_0()
                .left_0()
                .right_0()
                .bg(rgb(0xF4F1EA))
                .border_t_1()
                .border_color(rgb(RULE_COLOR))
                .px(px(28.))
                .py(px(8.))
                .flex()
                .items_center()
                .gap(px(8.))
                .font_family("PT Serif")
                .text_size(px(13.))
                .child(div().text_color(rgb(MUTED_COLOR)).child("Note:"))
                .child(div().flex_1().child(input)),
        )
    }

    /// The chord whisper (DESIGN §3.5): a muted one-liner in the bottom
    /// right corner — its own tiny surface, deliberately NOT ai_status
    /// (no card, no title) and never over prose; it fades on its timer
    /// or dies with the next repaint after it.
    fn render_chord_whisper(&self) -> Option<impl IntoElement> {
        let text = self.chord_whisper.clone()?;
        Some(
            div()
                .absolute()
                .bottom(px(8.))
                .right(px(12.))
                .px(px(8.))
                .py(px(3.))
                .rounded(px(4.))
                // Translucent paper: in wide windows the corner is margin
                // lane (prose-free); in narrow ones it can graze the
                // viewport's last clipped line, and the translucency
                // keeps even that readable for the 6s the whisper lives.
                .bg(rgba(0xFBFAF8D9u32))
                .font_family("PT Sans")
                .text_size(px(11.))
                .text_color(rgb(MUTED_COLOR))
                .child(text),
        )
    }

    /// The AI surface (PLAN.md E3), pinned where results land: top of the
    /// margin lane when it fits, a floating top-right card otherwise.
    /// With no status and an empty margin, a one-line hint teaches the
    /// chord — the AI must be visible before the chord is known.
    fn render_ai_status(&self, window: &Window, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let fits = self.margin_fits(window);
        let (left, width) = if fits {
            let col_right = self.column_right(window);
            (col_right + MARGIN_GAP + 8., MARGIN_WIDTH - 8.)
        } else {
            (0., 0.) // narrow: bottom strip, never floating over prose
        };
        let card = move |bg: u32| {
            // Wide window: a card at the top of the margin lane. Narrow:
            // a full-width strip at the bottom — status must never sit on
            // top of the user's text (Kirill's "insult to injury").
            if fits {
                div()
                    .absolute()
                    .top(px(BAR_HEIGHT + 8.))
                    .left(px(left))
                    .w(px(width))
                    .rounded(px(6.))
                    .flex_col()
            } else {
                // Narrow window: a full-width strip pinned to the bottom,
                // never over prose. Stack the content (title · detail ·
                // actions) so the buttons can't be pushed off the right edge
                // by a long privacy line — the default-sized window lands
                // here, so the setup actions must always be reachable.
                div().absolute().bottom_0().left_0().right_0().border_t_1().flex_col()
            }
            .p(px(10.))
            .bg(rgb(bg))
            .border_1()
            .border_color(rgb(RULE_COLOR))
            .font_family("PT Serif")
            .text_size(px(12.))
            .text_color(rgb(TEXT_COLOR))
            .flex()
            .gap(px(6.))
        };
        let action_button = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .px(px(8.))
                .py(px(2.))
                .rounded(px(4.))
                .border_1()
                .border_color(rgb(RULE_COLOR))
                .cursor(CursorStyle::PointingHand)
                .hover(|d| d.bg(rgba(0x1A1A180Au32)))
                .text_size(px(11.))
                .child(label)
        };
        let mode = self.effective_mode();
        Some(match self.ai_status.as_ref() {
            None => {
                // Idle hint, only in the wide margin and only when the margin
                // is otherwise empty. The door rail (render_margin_rail) claims
                // the same top-of-lane slot whenever it's holding something
                // back; in drafting that happens with zero visible cards, so
                // margin_cards being empty isn't enough — check the rail's own
                // condition too, or the hint paints straight over it (Image-4).
                let rail_showing = if self.drafting {
                    self.resting_diagnoses() > 0
                } else {
                    self.suppressed_copy() > 0
                };
                if !fits
                    || self.has_margin_cards()
                    || rail_showing
                    || self.doc.len_bytes() == 0
                {
                    return None;
                }
                div()
                    .absolute()
                    .top(px(BAR_HEIGHT + 12.))
                    .left(px(left))
                    .w(px(width))
                    .font_family("PT Serif")
                    .text_size(px(11.))
                    .text_color(rgb(MUTED_COLOR))
                    .child(format!("Margin: ctrl-shift-d — {mode} read"))
                    .into_any_element()
            }
            Some(AiStatus::NeedsSetup { local_model }) => {
                let base = card(CARD_BG);
                match local_model.clone() {
                    // The cliff is gone: a local model answered the probe.
                    // Lead with the one-click, key-free, private path.
                    Some(model) => base
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .child("A local model is ready"),
                        )
                        .child(div().text_color(rgb(MUTED_COLOR)).child(format!(
                            "{model} is running on this machine. Diagnose with it now — no \
                             key, no account, and your text never leaves your computer.",
                        )))
                        .child(
                            div()
                                .flex()
                                .gap(px(6.))
                                .child(
                                    action_button("ai-use-local", "Run with this model")
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |editor, _: &MouseDownEvent, _, cx| {
                                                cx.stop_propagation();
                                                editor.use_local_model(model.clone(), cx);
                                            }),
                                        ),
                                )
                                .child(action_button("ai-setup", "Other provider…").on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|editor, _: &MouseDownEvent, window, cx| {
                                        cx.stop_propagation();
                                        editor.open_ai_settings(&OpenAiSettings, window, cx);
                                    }),
                                ))
                                .child(action_button("ai-setup-dismiss", "Dismiss").on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|editor, _: &MouseDownEvent, window, cx| {
                                        cx.stop_propagation();
                                        editor.cancel_ai_run(&CancelAiRun, window, cx);
                                    }),
                                )),
                        )
                        .into_any_element(),
                    None => base
                        .child(div().font_weight(FontWeight::BOLD).child("Diagnosis needs a model"))
                        .child(div().text_color(rgb(MUTED_COLOR)).child(
                            "Strop sends your text directly to the OpenAI-compatible endpoint you \
                             configure — only when you run a pass, never while you type.",
                        ))
                        .child(
                            div()
                                .flex()
                                .gap(px(6.))
                                .child(action_button("ai-setup", "Set up…").on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|editor, _: &MouseDownEvent, window, cx| {
                                        cx.stop_propagation();
                                        editor.open_ai_settings(&OpenAiSettings, window, cx);
                                    }),
                                ))
                                .child(action_button("ai-test", "Test connection").on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|editor, _: &MouseDownEvent, window, cx| {
                                        cx.stop_propagation();
                                        editor.test_ai_connection(&TestAiConnection, window, cx);
                                    }),
                                ))
                                .child(action_button("ai-setup-dismiss", "Dismiss").on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|editor, _: &MouseDownEvent, window, cx| {
                                        cx.stop_propagation();
                                        editor.cancel_ai_run(&CancelAiRun, window, cx);
                                    }),
                                )),
                        )
                        .into_any_element(),
                }
            }
            Some(AiStatus::Running { label }) => card(CARD_BG)
                .child(div().text_color(rgb(MUTED_COLOR)).child(format!("Running: {label}…")))
                .child(
                    div().flex().child(action_button("ai-cancel", "Cancel").on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|editor, _: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            editor.cancel_ai_run(&CancelAiRun, window, cx);
                        }),
                    )),
                )
                .into_any_element(),
            Some(AiStatus::Note { title, detail }) => card(0xF2F4EC)
                .child(div().child(title.clone()))
                .when(!detail.is_empty(), |d| {
                    d.child(div().text_color(rgb(MUTED_COLOR)).child(detail.clone()))
                })
                .into_any_element(),
            Some(AiStatus::Error { title, detail }) => card(0xFAF0EC)
                .child(div().font_weight(FontWeight::BOLD).child(title.clone()))
                .when(!detail.is_empty(), |d| {
                    d.child(
                        div()
                            .text_color(rgb(MUTED_COLOR))
                            .text_size(px(11.))
                            .child(detail.clone()),
                    )
                })
                .child(
                    div()
                        .flex()
                        .gap(px(6.))
                        .child(action_button("ai-err-config", "Set up…").on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|editor, _: &MouseDownEvent, window, cx| {
                                cx.stop_propagation();
                                editor.open_ai_settings(&OpenAiSettings, window, cx);
                            }),
                        ))
                        .child(action_button("ai-err-retry", "Retry").on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                editor.ai_status = None;
                                let kind = editor.last_pass.clone();
                                editor.run_pass(kind, cx);
                            }),
                        ))
                        .child(action_button("ai-err-dismiss", "Dismiss").on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|editor, _: &MouseDownEvent, window, cx| {
                                cx.stop_propagation();
                                editor.cancel_ai_run(&CancelAiRun, window, cx);
                            }),
                        )),
                )
                .into_any_element(),
        })
    }

    fn render_margin(&self, window: &Window, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if !self.margin_fits(window) {
            return None;
        }
        let col_right = self.column_right(window);
        let MarginLayout {
            mut cards,
            above,
            below,
        } = self.margin_cards(true);
        if cards.is_empty() && above.is_empty() && below.is_empty() && self.departing.is_empty() {
            return None;
        }
        // Ghosts of just-resolved cards (departing), painted FIRST so the live
        // lane sits over them: a brief exit fade where the card was, non-
        // interactive (no id, no listeners — a click lands on whatever is
        // really there). In the common uncrowded lane nothing else moves, so
        // the whole gesture is just the card taking its leave.
        let ghosts: Vec<gpui::AnyElement> = self
            .departing
            .iter()
            .map(|(card, _)| {
                ghost_card(card)
                    .with_animation(
                        ("card-depart", card.id as usize),
                        Animation::new(CARD_RESOLVE).with_easing(|t| t * t * t),
                        |el, t| el.opacity(1. - t),
                    )
                    .into_any_element()
            })
            .collect();
        // reduce_motion cross-fades: a moving card's old-slot snapshot fades
        // out under the live card fading in at its new slot — the same "the
        // card went from here to there" information as the slide, with no
        // travel. Snapshots carry CONTENT-SPACE tops; re-anchor to this
        // frame's scroll so even the fading light tracks the text 1:1.
        let scroll_now = self
            .last_frame
            .as_ref()
            .map(|f| f32::from(f.scroll_top))
            .unwrap_or(0.);
        let crossfades: Vec<gpui::AnyElement> = self
            .moving
            .iter()
            .filter_map(|(&id, mv)| {
                let snapshot = mv.ghost.as_ref()?;
                let mut old = snapshot.clone();
                old.top = snapshot.top - scroll_now;
                let key =
                    ("card-move-out", (id as usize) | ((mv.generation as usize) << 48));
                let delay_frac = mv.delay.as_secs_f32() / mv.total().as_secs_f32();
                Some(
                    ghost_card(&old)
                        .with_animation(
                            key,
                            Animation::new(mv.total())
                                .with_easing(move |t| staggered_ease(t, delay_frac)),
                            |el, t| el.opacity(1. - t),
                        )
                        .into_any_element(),
                )
            })
            .collect();
        let (above_n, below_n) = (above.len(), below.len());
        let floor = BAR_HEIGHT + 8.;
        // A quiet pill at a lane edge when cards are hidden past it — the honest
        // "there's more here, it didn't vanish" cue (DESIGN principle 2).
        let edge_chip = move |label: String, at_bottom: bool| {
            let chip = div()
                .absolute()
                .left(px((MARGIN_WIDTH - 88.) / 2.))
                .w(px(88.))
                .flex()
                .justify_center()
                .px(px(8.))
                .py(px(2.))
                .rounded(px(10.))
                .bg(rgb(CARD_BG))
                .border_1()
                .border_color(rgb(RULE_COLOR))
                .text_size(px(10.))
                .text_color(rgb(MUTED_COLOR))
                .font_family("PT Serif")
                .child(label);
            if at_bottom {
                chip.bottom(px(6.))
            } else {
                chip.top(px(floor))
            }
        };
        // Paint the active card LAST so it sits ON TOP of any neighbour it
        // overlaps (GPUI paints siblings in tree order). Tops are unchanged —
        // this is purely z-order: "the selected annotation is always on top."
        if let Some(i) = cards.iter().position(|c| c.active) {
            let active = cards.remove(i);
            cards.push(active);
        }
        // The off-screen-count pills are clickable (issue 2): a click pages the
        // document toward the nearest hidden card that way. Built before the
        // container so each `cx.listener` borrow is its own statement.
        let above_chip = (above_n > 0).then(|| {
            edge_chip(format!("{above_n} above"), false)
                .cursor(CursorStyle::PointingHand)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|editor, _: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        editor.reveal_offscreen(false, window, cx);
                    }),
                )
        });
        let below_chip = (below_n > 0).then(|| {
            edge_chip(format!("{below_n} below"), true)
                .cursor(CursorStyle::PointingHand)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|editor, _: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        editor.reveal_offscreen(true, window, cx);
                    }),
                )
        });
        let reduce_motion = self.config.reduce_motion;
        let lane_left = col_right + MARGIN_GAP;
        Some(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(px(lane_left))
                .w(px(MARGIN_WIDTH))
                // Scroll is handled once at the document root (see `render`): a
                // wheel anywhere over the document surface — prose, gutters, this
                // lane, the whitespace beyond it — scrolls the one document. No
                // per-element wheel handler here (it would double-fire via bubble).
                .children(ghosts)
                .children(crossfades)
                .children(cards.into_iter().map(|card| {
                    let MarginCard {
                        id,
                        top,
                        body,
                        active,
                        kind,
                        title,
                        level,
                        orphaned,
                        unverified,
                        collapsed,
                        ..
                    } = card;
                    // Inside its entrance fade? (One fade per landed pass;
                    // never replayed — see appear_fade.)
                    let is_new = self.appearing.contains(&id);
                    // Receded (over the full-size budget): one muted title line
                    // at the anchor — present and clickable, just smaller, the
                    // way dense marginalia shrink on paper. Clicking selects it,
                    // and the selected card is budget-exempt, so it expands in
                    // place. Height is FORCED to COLLAPSED_CARD_H so the packer
                    // and the paint can never disagree (the overlap bug class).
                    if collapsed {
                        let compact = div()
                            .id(("note-card", id as usize))
                            .absolute()
                            .top(px(top.max(4.)))
                            .left(px(8.))
                            .w(px(MARGIN_WIDTH - 8.))
                            .h(px(COLLAPSED_CARD_H))
                            .px(px(8.))
                            .py(px(3.))
                            .overflow_hidden()
                            .rounded(px(3.))
                            .bg(rgb(if unverified { STALE_BG } else { DIAGNOSIS_CARD_BG }))
                            .border_1()
                            .border_color(rgb(RULE_COLOR))
                            .cursor(CursorStyle::PointingHand)
                            .font_family("PT Serif")
                            .text_size(px(11.))
                            .line_height(px(COLLAPSED_CARD_H - 8.))
                            .text_color(rgb(MUTED_COLOR))
                            .hover(|d| d.text_color(rgb(TEXT_COLOR)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |editor, _: &MouseDownEvent, window, cx| {
                                    cx.stop_propagation();
                                    if editor.focus.active_id() != Some(id) {
                                        editor.select_card(id, window, cx);
                                        cx.notify();
                                    }
                                }),
                            )
                            .child(div().truncate().child({
                                let line = if title.is_empty() {
                                    level.clone()
                                } else {
                                    title.clone()
                                };
                                // The detached marker survives receding — an
                                // orphaned card at a best-effort anchor must
                                // never read as confidently anchored, however
                                // small it renders (the count-grammar rule:
                                // detached outranks every other treatment).
                                if orphaned {
                                    format!("{line} · detached")
                                } else {
                                    line
                                }
                            }));
                        if let Some(mv) = self.moving.get(&id) {
                            return move_slide(compact, id, mv, top, reduce_motion);
                        }
                        return appear_fade(compact, id, is_new);
                    }
                    // The composer renders only on the note it is actually
                    // editing — never on a clicked AI card (the id comes from
                    // the same `Composing` variant as the input).
                    let composing_here = self.focus.composing_id() == Some(id);
                    let composer = composing_here
                        .then(|| self.focus.input().cloned())
                        .flatten();
                    let is_diagnosis = kind == NoteKind::Diagnosis;
                    let label = note_card_label(is_diagnosis, &level, orphaned);
                    let card = div()
                        .id(("note-card", id as usize))
                        .absolute()
                        .top(px(top.max(4.)))
                        .left(px(if active { 0. } else { 8. }))
                        .w(px(MARGIN_WIDTH - 8.))
                        .p(px(8.))
                        .overflow_hidden()
                        // Two kinds of object, two shapes (no text tag, no
                        // colour wash): the writer's own notes are softly
                        // rounded (personal marginalia); AI diagnoses are
                        // crisper-cornered (formal editorial), reinforcing the
                        // bold-title cue. AI provenance, felt not labelled.
                        .rounded(px(if is_diagnosis { 3. } else { 9. }))
                        // Paper-tint differentiation (theme color language): a
                        // warm cream wash for the writer's own note (ink on the
                        // page), a cool blue wash for a live AI diagnosis (the
                        // machine voice, over the page). An unverified diagnosis
                        // DRAINS to neutral — doubt = desaturation, fading back
                        // into the page (never red; that's reserved for errors).
                        .bg(rgb(if unverified {
                            STALE_BG
                        } else if is_diagnosis {
                            DIAGNOSIS_CARD_BG
                        } else {
                            NOTE_CARD_BG
                        }))
                        .border_1()
                        .border_color(if active {
                            rgb(ACTIVE_BORDER)
                        } else {
                            rgb(RULE_COLOR)
                        })
                        .cursor(CursorStyle::PointingHand)
                        .font_family("PT Serif")
                        .text_size(px(13.))
                        .line_height(px(CARD_LINE_H))
                        // Unverified (flagged text edited since): greyed — the
                        // claim may no longer hold, so it recedes until the
                        // writer judges it. Never auto-dismissed.
                        .text_color(rgb(if unverified { MUTED_COLOR } else { TEXT_COLOR }))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |editor, _: &MouseDownEvent, window, cx| {
                                cx.stop_propagation();
                                let note = editor.doc.notes().get(id);
                                let is_note = note.is_some_and(|n| n.kind == NoteKind::Note);
                                let body = note.map(|n| n.body.clone()).unwrap_or_default();
                                if is_note {
                                    // Clicking a note opens (or re-opens) its
                                    // composer; clicking the one already being
                                    // composed is a no-op so the caret doesn't jump.
                                    if editor.focus.composing_id() != Some(id) {
                                        editor.open_composer(id, body, window, cx);
                                        cx.notify();
                                    }
                                } else if editor.focus.active_id() != Some(id) {
                                    // A diagnosis only ever gets selected.
                                    editor.select_card(id, window, cx);
                                    cx.notify();
                                }
                            }),
                        )
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .text_size(px(11.))
                                .text_color(rgb(MUTED_COLOR))
                                .child(label)
                                .child(
                                    div()
                                        .flex()
                                        .gap(px(6.))
                                        .child(
                                            div()
                                                .id(("note-done", id as usize))
                                                .cursor(CursorStyle::PointingHand)
                                                .hover(|d| d.text_color(rgb(TEXT_COLOR)))
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(
                                                        move |editor,
                                                              _: &MouseDownEvent,
                                                              window,
                                                              cx| {
                                                            cx.stop_propagation();
                                                            editor.set_note_status(
                                                                id,
                                                                NoteStatus::Done,
                                                                window,
                                                                cx,
                                                            );
                                                        },
                                                    ),
                                                )
                                                .child("done"),
                                        )
                                        .child(
                                            div()
                                                .id(("note-dismiss", id as usize))
                                                .cursor(CursorStyle::PointingHand)
                                                .hover(|d| d.text_color(rgb(TEXT_COLOR)))
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(
                                                        move |editor,
                                                              _: &MouseDownEvent,
                                                              window,
                                                              cx| {
                                                            cx.stop_propagation();
                                                            editor.set_note_status(
                                                                id,
                                                                NoteStatus::Dismissed,
                                                                window,
                                                                cx,
                                                            );
                                                        },
                                                    ),
                                                )
                                                .child("×"),
                                        ),
                                ),
                        )
                        .when(is_diagnosis && !title.is_empty(), |d| {
                            d.child(div().font_weight(FontWeight::BOLD).child(title.clone()))
                        })
                        // The body region is EXACTLY ONE of composer-or-text —
                        // a single match, never both (the "double" bug) and
                        // never neither (the "blank committed card" bug). The
                        // old two-`when` form let those conditions disagree.
                        .child(match card_body(composing_here) {
                            CardBody::Composer => composer
                                .map(|input| div().child(input))
                                .unwrap_or_else(div),
                            CardBody::Text if body.is_empty() && !is_diagnosis => {
                                div().text_color(rgb(MUTED_COLOR)).child("(empty note)")
                            }
                            CardBody::Text => div().child(body.clone()),
                        });
                    // A mid-move card slides (or cross-fades) to its new slot;
                    // entrance fades are for genuinely new cards only — the
                    // two never overlap (a new card has no old slot to leave).
                    if let Some(mv) = self.moving.get(&id) {
                        return move_slide(card, id, mv, top, reduce_motion);
                    }
                    appear_fade(card, id, is_new)
                }))
                .children(above_chip)
                .children(below_chip),
        )
    }

    /// The door's visible state (DESIGN §4.4, principle 5 — no hidden modes):
    /// a thin rail at the lane top that names what the closed door is holding
    /// ("3 resting · open") and opens it on a click; in reviewing it instead
    /// notes copy-level cards held back until the structural ones clear.
    /// Returns None when nothing is held — a quiet margin with nothing to
    /// gate looks exactly like an empty one, so the mode only shows when it
    /// is actually doing something.
    fn render_margin_rail(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if !self.margin_fits(window) {
            return None;
        }
        let col_right = self.column_right(window);
        let lane_left = col_right + MARGIN_GAP + 8.;
        let top = BAR_HEIGHT + 8.;
        let drafting = self.drafting;
        let n = self.margin_rail_count()?;
        let label = if drafting {
            format!("{n} resting · open")
        } else {
            format!("{n} copy-level · after structure")
        };
        let styled = |d: gpui::Div| {
            d.absolute()
                .top(px(top))
                .left(px(lane_left))
                .w(px(MARGIN_WIDTH - 8.))
                .px(px(8.))
                .py(px(4.))
                .rounded(px(6.))
                .border_1()
                .border_color(rgb(RULE_COLOR))
                .bg(rgb(0xF7F5EF))
                .font_family("PT Sans")
                .text_size(px(11.))
                .text_color(rgb(MUTED_COLOR))
                .child(label.clone())
        };
        Some(if drafting {
            styled(div())
                .id("margin-door-open")
                .cursor(CursorStyle::PointingHand)
                .hover(|d| d.text_color(rgb(TEXT_COLOR)))
                .tooltip(tip("Open the door — show the editor's notes", Some("ctrl-shift-r")))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        editor.flush_deferred_pass(cx);
                        editor.drafting = false;
                        cx.notify();
                    }),
                )
                .into_any_element()
        } else {
            styled(div()).into_any_element()
        })
    }

    /// How many lane items the narrow drawer should advertise: the visible
    /// cards in reviewing, the door's held-back count in drafting.
    fn narrow_notes_count(&self, cards: &[MarginCard]) -> usize {
        if cards.is_empty() {
            self.margin_rail_count().unwrap_or(0)
        } else {
            cards.len()
        }
    }

    /// One menu row (impl 04 §0): a carrier sentence ("A believing read — …")
    /// that teaches the craft word in place. Its `kind` is pinned into the run
    /// directly, so choosing a depth never mutates the persistent
    /// `diagnosis_mode` (the sticky-mode trap). Inert rows stay VISIBLE but
    /// unclickable (review H33: cooking or a history preview must not dispatch);
    /// a `gate` reason livens the copy row's `when` line — the only row that
    /// explains, because the gate is data (no "usually after…" advice anywhere).
    ///
    /// The dress is the lab's `.erow`/`.vq`: the read's VERB in a bold face,
    /// its qualifier in small muted ink on the SAME line (never the wrapped
    /// serif paragraph this replaced), a full-bleed cool hover wash (this is
    /// the MACHINE's menu; the selection menu's writer verbs wash warm). P12
    /// accents — the control is the indicator: the row whose read the machine
    /// holds right now (cooking, or parked behind the reveal clock) wears the
    /// same cool dot the button pulses, and the row ctrl-shift-d currently
    /// aims at teaches its chord as a keycap chip (the chip FOLLOWS the
    /// session depth, so the default read is always marked).
    fn editor_menu_row(
        &self,
        id: &'static str,
        (verb, qualifier): (&'static str, &'static str),
        kind: PassKind,
        inert: bool,
        gate: Option<&'static str>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let disabled = inert || gate.is_some();
        let held = self.last_pass == kind
            && (matches!(self.ai_status, Some(AiStatus::Running { .. }))
                || self.deferred_pass.is_some());
        let chord = match &kind {
            PassKind::Believing => Some("ctrl-shift-b"),
            PassKind::Diagnostic(mode) if *mode == self.effective_mode() => Some("ctrl-shift-d"),
            _ => None,
        };
        let base = div()
            .id(id)
            .relative()
            .px(px(12.))
            .py(px(7.))
            .flex()
            .flex_col()
            .gap(px(2.))
            // The held dot rides the padding band as an overlay, so the verb
            // column never shifts when a read starts or parks.
            .when(held, |d| {
                d.child(
                    div()
                        .absolute()
                        .left(px(3.))
                        .top_0()
                        .bottom_0()
                        .flex()
                        .items_center()
                        .child(div().size(px(6.)).rounded_full().bg(rgb(AI_ACCENT))),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .child(
                        div()
                            .flex_shrink_0()
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(13.))
                            .text_color(rgb(if disabled { MUTED_COLOR } else { TEXT_COLOR }))
                            .child(verb),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_size(px(11.))
                            .text_color(rgb(MUTED_COLOR))
                            .child(qualifier),
                    )
                    // The chord chip (the lab's `.k` keycap, the selection
                    // menu's exact dress): the menu teaches its own shortcuts
                    // in place, instead of a manual teaching them in prose.
                    .when_some(chord, |d, chord| {
                        d.child(
                            div()
                                .flex_shrink_0()
                                .px(px(3.))
                                .font_family(CODE_FONT)
                                .text_size(px(9.))
                                .text_color(rgb(MUTED_COLOR))
                                .border_1()
                                .border_color(rgb(RULE_COLOR))
                                .rounded(px(4.))
                                .bg(rgb(BG_COLOR))
                                .child(chord),
                        )
                    }),
            );
        let base = match gate {
            Some(reason) => base.child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(MUTED_COLOR))
                    .child(reason),
            ),
            None => base,
        };
        if disabled {
            base.into_any_element()
        } else {
            base.cursor(CursorStyle::PointingHand)
                .hover(|d| d.bg(rgb(DIAGNOSIS_CARD_BG)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |editor, _: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        // Selecting a row closes the menu, then runs the pinned
                        // read (single-flight guard in run_pass handles the rest).
                        editor.close_editor_menu(cx);
                        editor.run_pass(kind.clone(), cx);
                    }),
                )
                .into_any_element()
        }
    }

    /// The editor button's dropdown (impl 04 §0): the AI subsystem's single
    /// home, glued flush under the titlebar control (right edges aligned, top
    /// corners squared into the bar — the lab's fix for the detached-dropdown
    /// sin), borrowing the narrow-notes panel's anchoring idiom (a bool-toggled
    /// overlay, light-dismissed + Esc). Surface dress is the selection menu's
    /// shipped values (CARD_BG, RULE hairline, 9px rounding), so the two verb
    /// menus read as one family.
    fn render_editor_menu(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if !self.editor_menu_open {
            return None;
        }
        // Rows go inert while a pass cooks (single-flight — no queueing in v1)
        // or the past is on screen (review H33: a pass must never diagnose a
        // document the screen isn't showing). An UNCONFIGURED provider does NOT
        // disable them — clicking routes through run_pass's pending-pass path,
        // so the request that triggers setup is the one that gets answered.
        let inert = self.history_preview.is_some()
            || matches!(self.ai_status, Some(AiStatus::Running { .. }));
        // The copy gate is DATA: while a developmental query is open, the copy
        // row wears its reason as a `when` line and stands down (the altitude
        // order). Releases with no pass re-run when the last one closes.
        let copy_gate = self
            .copy_gated()
            .then_some("after the structural queries settle");
        // Anchored flush under the button, right edges aligned — against the
        // button's PAINTED edge (the capture canvas inside the control), not
        // a fixed-width estimate of the chrome right of it: those controls
        // flex-shrink in a narrow bar, which left the estimate ~75px off its
        // control. The estimate survives only as the first-frame fallback
        // (palette-toggle 27 + history-toggle 4+31 [+ 3×34 drawn window
        // controls off macOS — its traffic lights sit top-LEFT]).
        let vw = f32::from(window.viewport_size().width);
        let est = vw - if cfg!(target_os = "macos") { 62. } else { 164. };
        let btn_right = self.editor_btn_right.get().map_or(est, f32::from);
        let menu_right = (vw - btn_right).max(8.);
        // The lab's one-line law (392 there, chipless): verb + qualifier +
        // the keycap chip hold ONE line — 430 clears the longest row (the
        // believing read's qualifier beside its ctrl-shift-b chip). A window
        // too narrow for that cedes width, never the button's edge.
        let menu_w = 430f32.min(vw - menu_right - 8.);
        let door_open = !self.drafting;
        let footer = div()
            .pt(px(7.))
            .px(px(12.))
            .pb(px(8.))
            .border_t_1()
            .border_color(rgb(RULE_COLOR))
            .flex()
            .items_center()
            .justify_between()
            .text_size(px(11.))
            .child(div().text_color(rgb(MUTED_COLOR)).child(format!(
                "{} queries open · {} resolved",
                self.open_query_count(),
                self.resolved_query_count(),
            )))
            .child(
                // The presence pair (glossary): Reading (door open) / Away
                // (drafting). A drawn toggle — the current pole wears the ink
                // AND the weight (never color alone, WCAG 1.4.1); a click
                // flips it through toggle_door's flush semantics. No "⇄"
                // glyph (not in the bundled PT fonts); a dot divides the poles.
                div()
                    .id("editor-menu-door")
                    .cursor(CursorStyle::PointingHand)
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .px(px(4.))
                    .py(px(1.))
                    .rounded(px(4.))
                    .hover(|d| d.bg(rgba(0x1A1A180Au32)))
                    .tooltip(tip("The door", Some("ctrl-shift-r")))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            editor.toggle_door(cx);
                        }),
                    )
                    .child(
                        div()
                            .text_color(rgb(if door_open { TEXT_COLOR } else { MUTED_COLOR }))
                            .when(door_open, |d| d.font_weight(FontWeight::BOLD))
                            .child("Reading"),
                    )
                    .child(div().size(px(3.)).rounded_full().bg(rgb(MUTED_COLOR)))
                    .child(
                        div()
                            .text_color(rgb(if door_open { MUTED_COLOR } else { TEXT_COLOR }))
                            .when(!door_open, |d| d.font_weight(FontWeight::BOLD))
                            .child("Away"),
                    ),
            );
        Some(
            div()
                .id("editor-menu")
                .absolute()
                // Overlap the bar's own hairline by 1px: the menu's top border
                // paints over it in the same RULE ink, so control and menu
                // share ONE hairline — glued, with the top corners squared.
                .top(px(BAR_HEIGHT - 1.))
                .right(px(menu_right))
                .w(px(menu_w))
                .bg(rgb(CARD_BG))
                .border_1()
                .border_color(rgb(RULE_COLOR))
                .rounded_b(px(9.))
                .shadow_lg()
                .flex()
                .flex_col()
                .font_family("PT Sans")
                .text_color(rgb(TEXT_COLOR))
                // Contained like the palette/narrow panel: clicks/scroll stay in.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .px(px(12.))
                        .pt(px(8.))
                        .pb(px(3.))
                        .text_size(px(11.))
                        .text_color(rgb(MUTED_COLOR))
                        .child("Ask the editor for…"),
                )
                .child(self.editor_menu_row(
                    "er-believing",
                    ("A believing read", "— what's alive here, what it's secretly about"),
                    PassKind::Believing,
                    inert,
                    None,
                    cx,
                ))
                .child(self.editor_menu_row(
                    "er-developmental",
                    ("A developmental read", "— the structure: stakes, turns, the ending"),
                    PassKind::Diagnostic("developmental".to_owned()),
                    inert,
                    None,
                    cx,
                ))
                .child(self.editor_menu_row(
                    "er-line",
                    ("A line read", "— rhythm, imagery, dialogue"),
                    PassKind::Diagnostic("line".to_owned()),
                    inert,
                    None,
                    cx,
                ))
                .child(self.editor_menu_row(
                    "er-copy",
                    ("A copy read", "— slips, typos, repetitions"),
                    PassKind::Diagnostic("copy".to_owned()),
                    inert,
                    copy_gate,
                    cx,
                ))
                .child(self.editor_menu_row(
                    "er-doubting",
                    ("A doubting read", "— the strongest case against it"),
                    PassKind::Doubting,
                    inert,
                    None,
                    cx,
                ))
                .child(footer)
                .into_any_element(),
        )
    }

    /// The always-visible feedback that notes EXIST when the window is too
    /// narrow for the lane (DESIGN §narrow-margin): a count pill in the
    /// column's empty top-padding band — never over prose. Clicking it
    /// toggles the panel. Below ~932px this is the only thing standing
    /// between the writer and "where did my diagnoses go?".
    fn render_narrow_notes_pill(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        // Parked = previewing the past: the pill counts cards anchored to the
        // LIVE document, and its panel would float them over past text — the
        // same law that hides the wide margin while parked (H36).
        if self.margin_fits(window) || self.history_view.is_some() || self.strip.is_parked() {
            return None;
        }
        let count = self.narrow_notes_count(&self.margin_cards(false).cards);
        if count == 0 {
            return None;
        }
        let open = self.narrow_notes_open;
        let noun = if count == 1 { "note" } else { "notes" };
        // The diagnose feature's mini-card motif, so the pill rhymes with what
        // it holds (a drawn outline — no PT-absent glyph, the atlas rule).
        let mark = rgb(MUTED_COLOR);
        Some(
            div()
                .id("narrow-notes-pill")
                .absolute()
                .top(px(BAR_HEIGHT + 6.))
                .right(px(12.))
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(8.))
                .py(px(3.))
                .rounded(px(6.))
                .border_1()
                .border_color(if open { rgb(ACTIVE_BORDER) } else { rgb(RULE_COLOR) })
                .bg(rgb(0xF7F5EF))
                .cursor(CursorStyle::PointingHand)
                .hover(|d| d.bg(rgb(0xEFEBE0)))
                .font_family("PT Sans")
                .text_size(px(11.))
                .text_color(rgb(MUTED_COLOR))
                .tooltip(tip("Window too narrow for the margin — show notes", None))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        editor.narrow_notes_open = !editor.narrow_notes_open;
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .w(px(12.))
                        .h(px(10.))
                        .rounded(px(2.))
                        .border_1()
                        .border_color(mark)
                        .flex()
                        .flex_col()
                        .justify_center()
                        .gap(px(1.5))
                        .px(px(2.))
                        .child(div().w(px(6.)).h(px(1.)).bg(mark))
                        .child(div().w(px(4.)).h(px(1.)).bg(mark)),
                )
                .child(format!("{count} {noun}"))
                .into_any_element(),
        )
    }

    /// The narrow drawer's panel: the cards the lane can't show, stacked and
    /// scrollable, dropped under the pill. Viewing only — editing a note still
    /// lands in the bottom composer strip (render_composer_strip); clicking a
    /// writer's note here opens that composer. In drafting the door is closed,
    /// so the panel offers to open it instead of listing hidden diagnoses.
    fn render_narrow_notes_panel(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if !self.narrow_notes_open
            || self.margin_fits(window)
            || self.history_view.is_some()
            || self.strip.is_parked()
        {
            return None;
        }
        let cards = self.margin_cards(false).cards;
        if self.narrow_notes_count(&cards) == 0 {
            return None;
        }
        let vw = f32::from(window.viewport_size().width);
        let vh = f32::from(window.viewport_size().height);
        let panel_w = (vw - 24.).min(340.);
        let mut list = div()
            .id("narrow-notes-list")
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(10.))
            .max_h(px((vh * 0.6).max(120.)))
            .overflow_y_scroll();
        if cards.is_empty() {
            // Drafting: the door holds the diagnoses back. Offer to open it,
            // mirroring the wide-window rail (render_margin_rail).
            let n = self.margin_rail_count().unwrap_or(0);
            list = list.child(
                div()
                    .id("narrow-open-door")
                    .cursor(CursorStyle::PointingHand)
                    .px(px(6.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(rgb(RULE_COLOR))
                    .bg(rgb(0xF7F5EF))
                    .font_family("PT Sans")
                    .text_size(px(12.))
                    .text_color(rgb(MUTED_COLOR))
                    .hover(|d| d.text_color(rgb(TEXT_COLOR)))
                    .tooltip(tip("Open the door — show the editor's notes", Some("ctrl-shift-r")))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            editor.flush_deferred_pass(cx);
                            editor.drafting = false;
                            cx.notify();
                        }),
                    )
                    .child(format!("{n} resting — open the door")),
            );
        } else {
            for card in &cards {
                list = list.child(self.narrow_note_card(card, cx));
            }
        }
        Some(
            div()
                .absolute()
                .top(px(BAR_HEIGHT + 34.))
                .right(px(12.))
                .w(px(panel_w))
                .bg(rgb(0xFCFAF4))
                .border_1()
                .border_color(rgb(RULE_COLOR))
                .rounded(px(8.))
                .shadow_lg()
                .text_color(rgb(TEXT_COLOR))
                // Contained like the palette: clicks/scroll stay inside.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .child(list)
                .into_any_element(),
        )
    }

    /// One card in the narrow drawer: the same content the margin shows, in a
    /// stacked (non-absolute) box. No inline composer — a writer's note opens
    /// the bottom strip on click; diagnoses are read-only here as everywhere.
    fn narrow_note_card(&self, card: &MarginCard, cx: &mut Context<Self>) -> gpui::AnyElement {
        let MarginCard { id, body, kind, title, level, orphaned, .. } = card;
        let (id, kind) = (*id, *kind);
        let is_diagnosis = kind == NoteKind::Diagnosis;
        let label = note_card_label(is_diagnosis, level, *orphaned);
        let body = body.clone();
        let title = title.clone();
        div()
            .id(("narrow-note", id as usize))
            .p(px(8.))
            .rounded(px(6.))
            .bg(rgb(CARD_BG))
            .border_1()
            .border_color(rgb(RULE_COLOR))
            .cursor(CursorStyle::PointingHand)
            .font_family("PT Serif")
            .text_size(px(13.))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |editor, _: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    let note = editor.doc.notes().get(id);
                    if note.is_some_and(|n| n.kind == NoteKind::Note) {
                        let body = note.map(|n| n.body.clone()).unwrap_or_default();
                        editor.open_composer(id, body, window, cx);
                    }
                }),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .text_size(px(11.))
                    .text_color(rgb(MUTED_COLOR))
                    .child(label)
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(
                                div()
                                    .id(("narrow-done", id as usize))
                                    .cursor(CursorStyle::PointingHand)
                                    .hover(|d| d.text_color(rgb(TEXT_COLOR)))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |editor, _: &MouseDownEvent, window, cx| {
                                            cx.stop_propagation();
                                            editor.set_note_status(id, NoteStatus::Done, window, cx);
                                        }),
                                    )
                                    .child("done"),
                            )
                            .child(
                                div()
                                    .id(("narrow-dismiss", id as usize))
                                    .cursor(CursorStyle::PointingHand)
                                    .hover(|d| d.text_color(rgb(TEXT_COLOR)))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |editor, _: &MouseDownEvent, window, cx| {
                                            cx.stop_propagation();
                                            editor.set_note_status(id, NoteStatus::Dismissed, window, cx);
                                        }),
                                    )
                                    .child("×"),
                            ),
                    ),
            )
            .when(is_diagnosis && !title.is_empty(), |d| {
                d.child(div().font_weight(FontWeight::BOLD).child(title.clone()))
            })
            .child(if body.is_empty() && !is_diagnosis {
                div().text_color(rgb(MUTED_COLOR)).child("(empty note)")
            } else {
                div().child(body.clone())
            })
            .into_any_element()
    }
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Measure-and-cache margin-card heights up front (the window's text
        // system is in hand here) so margin_cards places on real extents.
        self.refresh_card_heights(window, cx);
        // Then diff the packed lane against last frame: a discrete re-pack
        // starts the card slides; scroll/composer/burst frames snap instead.
        self.update_lane_motion(cx);
        // History mode pushes the document aside (DESIGN §2-history: push,
        // not overlay — single-document app, reflow is cheap). The column
        // re-centers and re-wraps in the remaining width.
        let hist_panel_w = self.history_panel_width(window);
        // The outline rail OVERLAYS the reserved left margin now (no push) —
        // rail_w is still its width, for the rail's own render.
        let rail_w = self.rail_width(window);
        let in_history = self.history_view.is_some();
        // The column's left inset and width — a pure function of viewport width
        // (column_frame): the no-jump invariant. Used only outside history.
        let (col_x, col_w) = self.column_frame(window);
        // Client-side decorations (H2 / window-decorations-csd.md): when the
        // compositor leaves us our own chrome (GNOME/sway Wayland always do),
        // we draw both the resize border AND the shadow gutter (below).
        // set_client_inset tells the platform how far the content is inset so
        // hit-testing and overlay geometry stay correct.
        let decorations = window.window_decorations();
        let tiling = match decorations {
            Decorations::Client { tiling } => tiling,
            Decorations::Server => Tiling::default(),
        };
        let client = matches!(decorations, Decorations::Client { .. });
        window.set_client_inset(px(if client { CSD_GUTTER } else { 0. }));
        let content = div()
            .size_full()
            .relative()
            .bg(rgb(BG_COLOR))
            .flex()
            .flex_col()
            // The whole window sits under one "App" key context so the
            // app-global commands (every menu verb that isn't a text
            // mutation — palette, find, file ops, AI, history, session) fire
            // from ANY focus, not only when the document holds it. bind_keys
            // binds those to "App" and the document-mutating ones to the inner
            // "Editor" context. Their handlers live here on the root so they
            // stay reachable when a field overlay (palette, note, settings)
            // has focus and the "Editor" subtree is off the dispatch path; the
            // editor column carries the same handlers, and the deeper one wins
            // when the document is focused, so these duplicates never
            // double-fire.
            .key_context("App")
            // Field overlays mount on this root, outside the column's listener
            // stack, so their actions bubble here: tab between fields, replace
            // (ctrl-h), and the palette row motion PaletteInput's up/down emit.
            .on_action(cx.listener(Self::note_tab))
            .on_action(cx.listener(Self::replace))
            .on_action(cx.listener(Self::palette_up))
            .on_action(cx.listener(Self::palette_down))
            // App-global command handlers, mirrored from the editor column so
            // they fire while a field overlay holds focus (see "App" above).
            .on_action(cx.listener(Self::new_document))
            .on_action(cx.listener(Self::open_file))
            .on_action(cx.listener(Self::rename_document))
            .on_action(cx.listener(Self::reveal_in_files))
            .on_action(cx.listener(Self::copy_document_path))
            .on_action(cx.listener(Self::save_copy_as))
            .on_action(cx.listener(Self::export_markdown))
            .on_action(cx.listener(Self::find))
            .on_action(cx.listener(Self::toggle_rail))
            .on_action(cx.listener(Self::toggle_graveyard))
            .on_action(cx.listener(Self::run_diagnosis))
            .on_action(cx.listener(Self::run_believing))
            .on_action(cx.listener(Self::toggle_review))
            .on_action(cx.listener(Self::mode_developmental))
            .on_action(cx.listener(Self::mode_line))
            .on_action(cx.listener(Self::mode_copy))
            .on_action(cx.listener(Self::open_ai_config))
            .on_action(cx.listener(Self::open_ai_settings))
            .on_action(cx.listener(Self::test_ai_connection))
            .on_action(cx.listener(Self::cancel_ai_run))
            .on_action(cx.listener(Self::toggle_history))
            .on_action(cx.listener(Self::toggle_strip))
            .on_action(cx.listener(Self::add_checkpoint))
            .on_action(cx.listener(Self::set_session_goal))
            .on_action(cx.listener(Self::toggle_palette))
            .on_action(cx.listener(Self::show_shortcuts))
            .on_action(cx.listener(Self::open_welcome))
            // §0.6 law 3 (click-outside) lives on the root so the whole
            // window counts as "outside", gutters and titlebar included.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _: &MouseDownEvent, window, cx| {
                    editor.light_dismiss(window, cx);
                }),
            )
            // One scroll handler for the WHOLE document surface (issue: scroll
            // only worked over the prose). "Everything that isn't a panel is one
            // scrollable document": prose, both gutters, the margin lane and the
            // whitespace beyond it all live under this root, so a wheel anywhere
            // bubbles here and scrolls. Panels (omnibox/settings/shortcuts/narrow
            // notes) stop_propagation on their own wheel — and on_scroll_wheel
            // early-returns while one is open — so they stay unaffected.
            .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
            .child(self.render_titlebar(window, cx))
            .child(
                div()
                    .w_full()
                    .flex_1()
                    .min_h(px(0.))
                    .flex()
                    .overflow_hidden()
                    .map(|d| {
                        if in_history {
                            // History keeps the legacy push-and-recentre layout.
                            d.justify_center()
                                .when(hist_panel_w > 0., |d| d.pr(px(hist_panel_w)))
                        } else {
                            // Left-anchored column at a width-only x; the
                            // outline overlays the reserved left margin (it no
                            // longer pushes, so toggling it can't move prose).
                            d.justify_start().pl(px(col_x))
                        }
                    })
                    .child(
                        div()
                            .key_context("Editor")
                    .track_focus(&self.focus_handle)
                    .cursor(CursorStyle::IBeam)
                    .on_action(cx.listener(Self::backspace))
                    .on_action(cx.listener(Self::delete))
                    .on_action(cx.listener(Self::delete_word_left))
                    .on_action(cx.listener(Self::delete_word_right))
                    .on_action(cx.listener(Self::left))
                    .on_action(cx.listener(Self::right))
                    .on_action(cx.listener(Self::up))
                    .on_action(cx.listener(Self::down))
                    .on_action(cx.listener(Self::word_left))
                    .on_action(cx.listener(Self::word_right))
                    .on_action(cx.listener(Self::paragraph_up))
                    .on_action(cx.listener(Self::paragraph_down))
                    .on_action(cx.listener(Self::select_paragraph_up))
                    .on_action(cx.listener(Self::select_paragraph_down))
                    .on_action(cx.listener(Self::select_left))
                    .on_action(cx.listener(Self::select_right))
                    .on_action(cx.listener(Self::select_up))
                    .on_action(cx.listener(Self::select_down))
                    .on_action(cx.listener(Self::select_word_left))
                    .on_action(cx.listener(Self::select_word_right))
                    .on_action(cx.listener(Self::select_all))
                    .on_action(cx.listener(Self::home))
                    .on_action(cx.listener(Self::end))
                    .on_action(cx.listener(Self::select_to_home))
                    .on_action(cx.listener(Self::select_to_end))
                    .on_action(cx.listener(Self::doc_start))
                    .on_action(cx.listener(Self::doc_end))
                    .on_action(cx.listener(Self::select_to_doc_start))
                    .on_action(cx.listener(Self::select_to_doc_end))
                    .on_action(cx.listener(Self::page_up))
                    .on_action(cx.listener(Self::page_down))
                    .on_action(cx.listener(Self::select_page_up))
                    .on_action(cx.listener(Self::select_page_down))
                    .on_action(cx.listener(Self::newline))
                    .on_action(cx.listener(Self::copy))
                    .on_action(cx.listener(Self::cut))
                    .on_action(cx.listener(Self::paste))
                    .on_action(cx.listener(Self::undo))
                    .on_action(cx.listener(Self::redo))
                    .on_action(cx.listener(Self::toggle_strong))
                    .on_action(cx.listener(Self::toggle_emphasis))
                    .on_action(cx.listener(Self::toggle_underline))
                    .on_action(cx.listener(Self::toggle_strikethrough))
                    .on_action(cx.listener(Self::toggle_highlight))
                    .on_action(cx.listener(Self::toggle_code))
                    .on_action(cx.listener(Self::heading1))
                    .on_action(cx.listener(Self::heading2))
                    .on_action(cx.listener(Self::heading3))
                    .on_action(cx.listener(Self::toggle_quote_block))
                    .on_action(cx.listener(Self::toggle_code_block))
                    .on_action(cx.listener(Self::toggle_bullet_list))
                    .on_action(cx.listener(Self::toggle_ordered_list))
                    .on_action(cx.listener(Self::add_checkpoint))
                    .on_action(cx.listener(Self::export_markdown))
                    .on_action(cx.listener(Self::insert_footnote))
                    .on_action(cx.listener(Self::add_note))
                    .on_action(cx.listener(Self::run_diagnosis))
                    .on_action(cx.listener(Self::run_believing))
                    .on_action(cx.listener(Self::toggle_review))
                    .on_action(cx.listener(Self::find))
                    .on_action(cx.listener(Self::replace))
                    .on_action(cx.listener(Self::note_tab))
                    .on_action(cx.listener(Self::escape_mode))
                    .on_action(cx.listener(Self::toggle_history))
                    .on_action(cx.listener(Self::toggle_strip))
                    .on_action(cx.listener(Self::open_file))
                    .on_action(cx.listener(Self::save_copy_as))
                    .on_action(cx.listener(Self::toggle_palette))
                    .on_action(cx.listener(Self::toggle_popover))
                    .on_action(cx.listener(Self::palette_up))
                    .on_action(cx.listener(Self::palette_down))
                    .on_action(cx.listener(Self::new_document))
                    .on_action(cx.listener(Self::rename_document))
                    .on_action(cx.listener(Self::reveal_in_files))
                    .on_action(cx.listener(Self::copy_document_path))
                    .on_action(cx.listener(Self::open_ai_config))
                    .on_action(cx.listener(Self::open_ai_settings))
                    .on_action(cx.listener(Self::test_ai_connection))
                    .on_action(cx.listener(Self::cancel_ai_run))
                    .on_action(cx.listener(Self::mode_developmental))
                    .on_action(cx.listener(Self::mode_line))
                    .on_action(cx.listener(Self::mode_copy))
                    .on_action(cx.listener(Self::show_shortcuts))
                    .on_action(cx.listener(Self::open_welcome))
                    .on_action(cx.listener(Self::toggle_rail))
                    .on_action(cx.listener(Self::set_session_goal))
                    .on_action(cx.listener(Self::set_aside))
                    .on_action(cx.listener(Self::send_to_graveyard))
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                    .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_middle_click))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .on_mouse_move(cx.listener(Self::on_mouse_move))
                    // Scroll lives on the root `content` (one handler for the
                    // whole document surface — see below); not here, or it would
                    // double-fire as the event bubbles up.
                    .on_drop(cx.listener(Self::on_file_drop))
                            // History recentres/rewraps in the remaining width;
                            // otherwise the column takes its width-only measure.
                            .map(|d| {
                                if in_history {
                                    d.w_full().max_w(px(COL_MAX_WIDTH))
                                } else {
                                    d.w(px(col_w))
                                }
                            })
                            .h_full()
                            .pt(px(56.))
                            .pb(px(28.))
                            .px(px(28.))
                            .font_family("PT Serif")
                            .text_size(px(
                                self.config.font_size.unwrap_or(20.).clamp(12., 40.)
                            ))
                            .line_height(px({
                                let fs = self.config.font_size.unwrap_or(20.).clamp(12., 40.);
                                ((fs * 1.4) / 2.).round() * 2.
                            }))
                            .text_color(rgb(TEXT_COLOR))
                            .child(EditorElement { editor: cx.entity() }),
                    ),
            )
            // Overlays AFTER the canvas: GPUI paints siblings in tree
            // order, so anything mounted before it ends up UNDER the text
            // (the bug Kirill caught from the first screenshot).
            .when(self.history_view.is_some(), |d| {
                d.child(self.render_history_banner(hist_panel_w, cx))
                    .child(self.render_history_panel(hist_panel_w, cx))
            })
            .when(rail_w > 0., |d| d.child(self.render_rail(rail_w, cx)))
            .map(|d| {
                // The footnote zone keys off live-doc offsets; in history
                // the canvas shows the merged diff, so it stands down.
                let (footnotes, hidden) = if self.history_view.is_some() {
                    (Vec::new(), 0)
                } else {
                    self.visible_footnotes()
                };
                d.when(!footnotes.is_empty(), |d| {
                    d.child(self.render_footnote_zone(footnotes, hidden, cx))
                })
            })
            .map(|d| {
                // The panel displaces the margin lane while open (DESIGN
                // §2-history) — and the AI status card that rides it. The strip
                // hides the lane + rail only while PREVIEWING the past (parked):
                // cards anchored to the live document must not float over past
                // text (review H36). At now, with the strip open, the live doc
                // and its cards stay.
                if self.history_view.is_some() || self.strip.is_parked() {
                    return d;
                }
                let d = match self.render_margin(window, cx) {
                    Some(margin) => d.child(margin),
                    None => d,
                };
                let d = match self.render_margin_rail(window, cx) {
                    Some(rail) => d.child(rail),
                    None => d,
                };
                match self.render_ai_status(window, cx) {
                    Some(status) => d.child(status),
                    None => d,
                }
            })
            .map(|d| {
                if !self.margin_fits(window) {
                    match self.render_composer_strip() {
                        Some(strip) => d.child(strip),
                        None => d,
                    }
                } else {
                    d
                }
            })
            .map(|d| match self.render_alt_strip() {
                Some(strip) => d.child(strip),
                None => d,
            })
            .map(|d| match self.render_goal_strip() {
                Some(strip) => d.child(strip),
                None => d,
            })
            // The history strip (P1): a bottom-strip overlay in the machine-room
            // dark, above the other bottom bands but below palette/shortcuts.
            .map(|d| match self.render_strip(window, cx) {
                Some(strip) => d.child(strip),
                None => d,
            })
            // The parked-mode banner (Bug B): a top overlay under the titlebar,
            // over the past preview — the text area's mode indicator.
            .map(|d| match self.render_strip_banner(cx) {
                Some(banner) => d.child(banner),
                None => d,
            })
            .map(|d| match self.render_chord_whisper() {
                Some(whisper) => d.child(whisper),
                None => d,
            })
            // The graveyard's sticky footer navigator (asides.md §3): a bottom
            // overlay that scrolls to the tail section and unsticks (hides) once
            // that section's header is on screen. The record itself is painted in
            // the scroll flow by the EditorElement (Bug B) — no overlay panel.
            .map(|d| match self.render_graveyard_bar(cx) {
                Some(bar) => d.child(bar),
                None => d,
            })
            // Narrow-window notes: the count pill (low — just feedback) and,
            // when toggled, the drop-down panel (high — an explicit overlay).
            .map(|d| match self.render_narrow_notes_pill(window, cx) {
                Some(pill) => d.child(pill),
                None => d,
            })
            // The two selection flanks rise together (docs/impl/03-flanks.md).
            // The RIGHT verb menu paints AFTER render_margin (above), so it
            // OCCLUDES the cards at its y (review B8); the LEFT formatting flank
            // lives in the opposite gutter.
            .map(|d| match self.render_selection_menu(window, cx) {
                Some(menu) => d.child(menu),
                None => d,
            })
            .map(|d| match self.render_selection_popover(window, cx) {
                Some(popover) => d.child(popover),
                None => d,
            })
            .map(|d| match self.render_narrow_notes_panel(window, cx) {
                Some(panel) => d.child(panel),
                None => d,
            })
            // The editor button's dropdown, glued under its titlebar control.
            .map(|d| match self.render_editor_menu(window, cx) {
                Some(menu) => d.child(menu),
                None => d,
            })
            // Last children = topmost: the omnibox, the keyboard map and
            // the AI settings panel cover everything below.
            .when(self.palette_input.is_some(), |d| {
                d.child(self.render_omni(window, cx))
            })
            .when(self.shortcuts_open, |d| d.child(self.render_shortcuts(cx)))
            .when(self.ai_settings.is_some(), |d| {
                d.child(self.render_ai_settings(cx))
            });

        // CSD chrome (window-decorations-csd.md): the content sits in an inner
        // surface inset by the shadow gutter on each untiled edge, with a soft
        // drop shadow, rounded corners and a hairline border — the visible
        // window boundary GNOME/sway Wayland never draw. Server decorations
        // (macOS/Windows/X11) get none of it; the OS draws the shadow. Resize
        // handles ride the OUTER backdrop so the border stays grabbable
        // through the gutter (an OS gesture, topmost even under a modal).
        let inset = |t: bool| px(if client && !t { CSD_GUTTER } else { 0. });
        let floating = client && !tiling.top && !tiling.bottom && !tiling.left && !tiling.right;
        // Round only the corners whose BOTH edges are free (a snapped edge is
        // square — GTK/libadwaita behaviour). Applied to the shadow node AND the
        // content node so the content's own background clips to the same radius
        // and no square corner pokes through the rounded border.
        let round = move |d: gpui::Div| {
            d.when(!tiling.top && !tiling.left, |d| d.rounded_tl(px(CSD_ROUNDING)))
                .when(!tiling.top && !tiling.right, |d| d.rounded_tr(px(CSD_ROUNDING)))
                .when(!tiling.bottom && !tiling.left, |d| d.rounded_bl(px(CSD_ROUNDING)))
                .when(!tiling.bottom && !tiling.right, |d| {
                    d.rounded_br(px(CSD_ROUNDING))
                })
        };
        div()
            .size_full()
            .relative()
            .bg(rgba(0x00000000))
            .child(
                div()
                    .absolute()
                    .top(inset(tiling.top))
                    .bottom(inset(tiling.bottom))
                    .left(inset(tiling.left))
                    .right(inset(tiling.right))
                    .overflow_hidden()
                    .when(client, |d| {
                        let d = d
                            .border_color(rgb(RULE_COLOR))
                            .when(!tiling.top, |d| d.border_t_1())
                            .when(!tiling.bottom, |d| d.border_b_1())
                            .when(!tiling.left, |d| d.border_l_1())
                            .when(!tiling.right, |d| d.border_r_1());
                        // Layered, downward-biased shadow — contact + cast +
                        // ambient. Low alphas that sum softer than the old single
                        // 0.35 slab; the soft layer reaches 6+14=20px down, inside
                        // the 22px gutter, so nothing clips. Only on a fully
                        // floating window; a snapped edge gets no shadow.
                        round(d).when(floating, |d| {
                            let s = |y: f32, blur: f32, a: f32| {
                                BoxShadow::new(px(0.), px(y), Hsla { h: 0., s: 0., l: 0., a })
                                    .blur_radius(px(blur))
                            };
                            d.shadow(vec![s(1., 2., 0.14), s(3., 8., 0.10), s(6., 14., 0.07)])
                        })
                    })
                    .child(round(content)),
            )
            .when(client, |d| d.children(resize_handles(tiling)))
    }
}

impl Focusable for Editor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// Word-motion boundaries as free functions over `&Document` (no GPUI context),
// so they are unit-testable and — crucially — *iterative*. The previous
// recursive form added one stack frame per consecutive blank line, so a paste
// of tens of thousands of empty lines could overflow the stack (an uncatchable
// abort). The loop is byte-for-byte equivalent for every input.

fn previous_word_boundary(doc: &Document, mut offset: usize) -> usize {
    let rope = doc.rope();
    loop {
        if offset == 0 {
            return 0;
        }
        let start = rope.line_to_byte(rope.byte_to_line(offset));
        if offset == start {
            // Continue from the end of the previous paragraph (iterate, never
            // recurse: a long blank run must not grow the stack).
            offset -= 1;
            continue;
        }
        let line = doc.slice_bytes(start..offset);
        return line
            .split_word_bound_indices()
            .rev()
            .find(|(_, seg)| seg.chars().next().is_some_and(char::is_alphanumeric))
            .map_or(start, |(ix, _)| start + ix);
    }
}

fn next_word_boundary(doc: &Document, mut offset: usize) -> usize {
    let rope = doc.rope();
    let len = doc.len_bytes();
    loop {
        if offset >= len {
            return len;
        }
        let line_ix = rope.byte_to_line(offset);
        let end = if line_ix + 1 < rope.len_lines() {
            rope.line_to_byte(line_ix + 1).saturating_sub(1)
        } else {
            len
        };
        if offset == end {
            offset += 1;
            continue;
        }
        let line = doc.slice_bytes(offset..end);
        return line
            .split_word_bound_indices()
            .find(|(_, seg)| seg.chars().next().is_some_and(char::is_alphanumeric))
            .map_or(end, |(ix, seg)| offset + ix + seg.len())
            .min(len);
    }
}

// ---- The history strip: painting & the floor ------------------------------

/// The strip's painted rail geometry (WINDOW coords), captured each paint so
/// the scrub mouse handlers map a pointer against the SAME bounds the fabric
/// drew (see `Editor::strip_rail`).
#[derive(Clone, Copy)]
pub struct StripGeom {
    rail_x0: f32,
    rail_x1: f32,
}

/// The thumb's travel: `min(total_work, rail_width)` (design §1). When the
/// history fits, the rail and the fixed-scale fabric coincide 1:1; when it
/// overflows, the thumb compresses the whole duration into the rail.
fn strip_travel(rail_w: f32, total_work: f32) -> f32 {
    total_work.min(rail_w).max(1.)
}

/// An rgb constant with an explicit alpha — the strip's translucent fabric
/// marks (flecks fuse by alpha; veils are faint).
fn tint(c: u32, a: f32) -> gpui::Rgba {
    let mut x = rgb(c);
    x.a = a;
    x
}

impl Editor {
    /// The bottom strip overlay (spec §0): machine-room dark, the seek-bar
    /// floor. The fabric + rail + thumb + ticks + labels are custom-painted by
    /// `StripElement`; the readout / Restore / Now chips are real divs on top
    /// (each `stop_propagation`, so a click on a control never scrubs — the
    /// hit-region rule, review mid). The container owns the scrub gesture
    /// (mousedown = park, drag = continuous, up/up_out = end — the selection-
    /// drag pattern) and pans the fabric on wheel.
    fn render_strip(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        if !self.strip.open {
            return None;
        }
        let parked = self.strip.parked;
        let now_ms = self
            .strip
            .bake
            .as_ref()
            .map_or_else(strop_core::journal::now_ms, |b| b.now_ms);
        // The readout: at now → the live count; parked → the reconstructed
        // moment, plus any Compare delta folded into the SAME single line.
        let readout = if parked {
            let mut s = strip::format_readout(self.strip.pos_ms, self.strip.words_at, now_ms);
            if let Some(pin) = self.strip.pin_ms {
                let delta = self.strip.words_at as i64 - self.strip.pin_words as i64;
                let when = strip::date_label(pin / 1000, now_ms / 1000);
                s = format!("{s} · {delta:+} since {when}");
            }
            s
        } else {
            strip::format_readout(now_ms, self.word_count, now_ms)
        };

        // A chip button in the machine-room family.
        let chip = |label: SharedString, bright: bool| {
            div()
                .px(px(8.))
                .py(px(3.))
                .rounded(px(4.))
                .bg(rgb(strip::READOUT_CHIP))
                .font_family("PT Sans")
                .text_size(px(11.))
                .text_color(if bright { rgb(0xE7E1D0) } else { rgb(0x8F8A7C) })
                .child(label)
        };

        let readout_chip = chip(readout.into(), true)
            .id("strip-readout")
            .occlude()
            // The readout is fixed at the left end and NEVER parks (design §3).
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());

        let restore_btn = div()
            .id("strip-restore")
            .occlude()
            .px(px(8.))
            .py(px(3.))
            .rounded(px(4.))
            .bg(rgb(0x2E2C22))
            .cursor(CursorStyle::PointingHand)
            .font_family("PT Sans")
            .text_size(px(11.))
            .text_color(rgb(0xE7E1D0))
            .hover(|d| d.bg(rgb(0x3A3728)))
            .child("Restore")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    editor.strip_restore(cx);
                }),
            );

        // Now: dim at the present, bright when parked (it and Restore announce
        // themselves as the pair they are — design §3, review H4: the two
        // exits from the past light up in the same beat). Click / Esc return.
        let now_chip = chip("Now".into(), parked)
            .when(parked, |d| d.bg(rgb(0xEAE6D8)).text_color(rgb(0x23221F)))
            .id("strip-now")
            .occlude()
            .cursor(CursorStyle::PointingHand)
            .hover(|d| d.bg(rgb(0x2E2C22)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    editor.strip_return_to_now(cx);
                }),
            );

        // Close affordance (Bug C): a small '×' in the strip's top-right, the
        // note-dismiss idiom (muted → bright on hover, pointing hand). Closes
        // the strip from ANY state; click-away deliberately still does not.
        let close_x = div()
            .id("strip-close")
            .occlude()
            .px(px(4.))
            .cursor(CursorStyle::PointingHand)
            .font_family("PT Sans")
            .text_size(px(14.))
            .text_color(rgb(0x8F8A7C))
            .hover(|d| d.text_color(rgb(0xE7E1D0)))
            .child("\u{d7}")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    editor.close_strip(cx);
                }),
            );

        Some(
            div()
                .absolute()
                .bottom_0()
                .left_0()
                .right_0()
                .h(px(strip::STRIP_H))
                .bg(rgb(strip::GROUND))
                .border_t_1()
                .border_color(rgb(0x3A382E))
                .occlude()
                // Scrub: mousedown parks (shift = Compare pin), drag scrubs
                // continuously, up/up_out end — the selection-drag pattern.
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|editor, ev: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        editor.strip_park_at_x(f32::from(ev.position.x), ev.modifiers.shift, cx);
                    }),
                )
                .on_mouse_move(cx.listener(|editor, ev: &MouseMoveEvent, _, cx| {
                    if editor.strip.scrubbing
                        && ev.pressed_button == Some(MouseButton::Left)
                        && let Some(pos) = editor.strip_pos_at_x(f32::from(ev.position.x))
                    {
                        editor.strip_scrub_to(pos, cx);
                    }
                }))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|editor, _: &MouseUpEvent, _, cx| {
                        editor.strip.scrubbing = false;
                        cx.notify();
                    }),
                )
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(|editor, _: &MouseUpEvent, _, cx| {
                        editor.strip.scrubbing = false;
                        cx.notify();
                    }),
                )
                .on_scroll_wheel(cx.listener(|editor, ev: &ScrollWheelEvent, _, cx| {
                    cx.stop_propagation();
                    let d = f32::from(ev.delta.pixel_delta(px(16.)).y)
                        + f32::from(ev.delta.pixel_delta(px(16.)).x);
                    editor.strip_pan(d, cx);
                }))
                .child(StripElement { editor: cx.entity() })
                // Left group: the readout, with Restore beside it when parked.
                .child(
                    div()
                        .absolute()
                        .left(px(strip::SIDE_PAD - 8.))
                        .top(px(4.))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(readout_chip)
                        .when(parked, |d| d.child(restore_btn)),
                )
                // Now + close, fixed at the far right.
                .child(
                    div()
                        .absolute()
                        .right(px(8.))
                        .top(px(4.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(now_chip)
                        .child(close_x),
                ),
        )
    }
}

struct StripElement {
    editor: Entity<Editor>,
}

impl IntoElement for StripElement {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

/// A shaped strip label, positioned at bake-time geometry (never re-shaped
/// mid-scrub — the stability law; shaping only ever happens in prepaint, per
/// the 2026-06-12 sprite-corruption rule).
struct StripText {
    origin: Point<Pixels>,
    line: gpui::ShapedLine,
}

#[derive(Default)]
struct StripPrepaint {
    rail_x0: f32,
    rail_x1: f32,
    band_top: f32,
    view: f32,
    labels: Vec<StripText>,
    dates: Vec<StripText>,
}

impl Element for StripElement {
    type RequestLayoutState = ();
    type PrepaintState = StripPrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let _guard = DrawGuard::enter();
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let _guard = DrawGuard::enter();
        let editor = self.editor.read(cx);
        let rail_x0 = f32::from(bounds.origin.x) + strip::SIDE_PAD;
        let rail_x1 = f32::from(bounds.origin.x + bounds.size.width) - strip::SIDE_PAD;
        let band_top = f32::from(bounds.origin.y);
        let view = editor.strip.view_offset;
        let Some(bake) = editor.strip.bake.as_ref() else {
            return StripPrepaint {
                rail_x0,
                rail_x1,
                band_top,
                view,
                ..Default::default()
            };
        };
        let fab_x = |work: f32| rail_x0 + work - view;
        let shape = |text: &str, size: f32, color: u32, w: &mut Window| {
            let run = TextRun {
                len: text.len(),
                font: gpui::font("PT Sans"),
                color: rgb(color).into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            w.text_system()
                .shape_line(SharedString::from(text.to_owned()), px(size), &[run], None)
        };
        // Station labels — only the ones the bake kept, and only within view.
        let mut labels = Vec::new();
        for st in &bake.stations {
            if !st.show || st.label.is_empty() {
                continue;
            }
            let wx = fab_x(st.x);
            if wx < rail_x0 - 60. || wx > rail_x1 + 60. {
                continue;
            }
            let line = shape(&st.label, 11., 0xB8B2A2, window);
            let lx = if st.flip_left {
                wx - f32::from(line.width) - 3.
            } else {
                wx + 3.
            };
            let ly = band_top + strip::TOP_ROW_H + 1. + (st.row as f32) * 11.;
            labels.push(StripText {
                origin: point(px(lx), px(ly)),
                line,
            });
        }
        // Date lane — thinned by real shaped width: a checkpoint-dense era
        // packs several day-firsts into a few px, and overprinted labels read
        // as one smear ("Tue 23 JulToday"). The LAST date always survives a
        // collision (it is Today far more often than not): it sheds the
        // neighbours it would overprint instead of being shed.
        let mut dates: Vec<StripText> = Vec::new();
        let mut last_right = f32::NEG_INFINITY;
        for (i, dt) in bake.dates.iter().enumerate() {
            let wx = fab_x(dt.x);
            if wx < rail_x0 - 40. || wx > rail_x1 + 40. {
                continue;
            }
            let line = shape(&dt.label, 10., 0x87826F, window);
            if wx < last_right + 14. {
                if i + 1 != bake.dates.len() {
                    continue;
                }
                while dates.last().is_some_and(|d| {
                    f32::from(d.origin.x) + f32::from(d.line.width) + 14. > wx
                }) {
                    dates.pop();
                }
            }
            last_right = wx + f32::from(line.width);
            dates.push(StripText {
                origin: point(px(wx + 2.), px(band_top + strip::STRIP_H - strip::DATE_LANE_H)),
                line,
            });
        }
        StripPrepaint {
            rail_x0,
            rail_x1,
            band_top,
            view,
            labels,
            dates,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let _guard = DrawGuard::enter();
        let editor = self.editor.read(cx);
        let rail_x0 = prepaint.rail_x0;
        let rail_x1 = prepaint.rail_x1;
        let band_top = prepaint.band_top;
        // Hand the rail geometry to the scrub mouse handlers (interior
        // mutability through the shared Rc — never an entity write).
        *editor.strip_rail.borrow_mut() = Some(StripGeom { rail_x0, rail_x1 });

        let rail_y = band_top + strip::RAIL_Y;
        let rail_w = (rail_x1 - rail_x0).max(1.);
        // A thin quad, clipped to the rail's x-window. `window` is a parameter
        // (not captured), so the direct paint_quad calls below — thumb, flecks,
        // playhead — never fight this helper for the mutable window borrow.
        let rect = |window: &mut Window, x: f32, y: f32, w: f32, h: f32, color: gpui::Rgba| {
            let xa = x.max(rail_x0);
            let xb = (x + w).min(rail_x1);
            if xb <= xa || h <= 0. {
                return;
            }
            window.paint_quad(fill(
                Bounds::new(point(px(xa), px(y)), size(px(xb - xa), px(h))),
                color,
            ));
        };

        let Some(bake) = editor.strip.bake.as_ref() else {
            // Degraded (no bake yet): just the rail line, so the floor exists.
            rect(window, rail_x0, rail_y, rail_w, 2., tint(strip::GREY, 0.6));
            return;
        };
        let view = prepaint.view;
        let total = bake.timeline.total_work;
        let pos = editor.strip.pos_ms;
        let fab_x = |work: f32| rail_x0 + work - view;
        let fab_top = band_top + strip::FAB_Y0;
        let fab_bot = fab_top + strip::FABRIC_H;

        // --- Cream page-fill + stepwise envelope stroke ----------------------
        if !bake.envelope.is_empty() {
            let mut prev = strip::EnvPoint {
                x: 0.,
                y: bake.envelope[0].y,
            };
            for p in bake.envelope.iter().chain(std::iter::once(&strip::EnvPoint {
                x: total,
                y: bake.envelope.last().map_or(fab_top, |e| e.y),
            })) {
                let xa = fab_x(prev.x);
                let xb = fab_x(p.x);
                let level = band_top + prev.y;
                // The faint cream page under the line.
                rect(window, xa, fab_top, xb - xa, level - fab_top, tint(strip::CREAM, strip::CREAM_FILL_ALPHA));
                // The envelope stroke: a horizontal cap + the vertical step.
                rect(window, xa, level - 0.65, xb - xa, 1.3, tint(strip::CREAM, strip::ENVELOPE_ALPHA));
                let (y0, y1) = ((prev.y).min(p.y), (prev.y).max(p.y));
                rect(window, xb - 0.65, band_top + y0, 1.3, y1 - y0, tint(strip::CREAM, strip::ENVELOPE_ALPHA));
                prev = *p;
            }
        }

        // --- Veils: the machine read here (a full-height cool column) --------
        for v in &bake.veils {
            let x = fab_x(v.x);
            rect(window, x - 2., fab_top, 4., strip::FABRIC_H, tint(strip::VEIL, strip::VEIL_ALPHA));
        }

        // --- Threads: a card's open life (cool), sage/grey terminal dot ------
        for t in &bake.threads {
            let x0 = fab_x(t.x0);
            let x1 = fab_x(t.x1);
            let y = band_top + t.y;
            rect(window, x0, y, x1 - x0, 1., tint(strip::THREAD, 0.5));
            if !t.open {
                let dot = if t.resolved { strip::SAGE } else { strip::GREY };
                rect(window, x1 - 1.5, y - 1.5, 3., 3., tint(dot, 0.9));
            }
        }

        // --- Seams: a folded >15 min gap ------------------------------------
        for s in &bake.seams {
            let x = fab_x(s.x);
            rect(window, x, fab_top, 1., strip::FABRIC_H, tint(strip::GREY, 0.35));
        }

        // --- Flecks: the fabric (2×2 amber grains; thousands = one batch).
        // Windowed: flecks are baked in time order (x non-decreasing), so the
        // visible range is one partition_point + an early break — paint cost
        // is O(visible), not O(journal) (wave-1 review, perf/mid; a year of
        // history would otherwise walk every grain per frame).
        let lo = view;
        let hi = view + (rail_x1 - rail_x0);
        let start = bake.flecks.partition_point(|f| f.x < lo);
        for f in &bake.flecks[start..] {
            if f.x > hi {
                break;
            }
            let x = fab_x(f.x);
            let color = if f.del {
                tint(strip::FLECK_DEL, strip::FLECK_DEL_ALPHA)
            } else {
                tint(strip::FLECK_INS, strip::FLECK_INS_ALPHA)
            };
            window.paint_quad(fill(
                Bounds::new(point(px(x), px(band_top + f.y)), size(px(strip::FLECK), px(strip::FLECK))),
                color,
            ));
        }

        // --- Station ticks (+ restore arcs), brightened near the playhead ----
        let play_x = fab_x(bake.timeline.work_at(pos));
        for st in &bake.stations {
            let x = fab_x(st.x);
            // Brighten by TIME distance to the playhead (association by light,
            // design §2) — stable under the fabric's zoom, unlike pixel distance.
            let near = (st.at_ms - pos).abs() < 45_000;
            let base = if st.restore { strip::SAGE } else { strip::GREY };
            let a = if near { 0.95 } else { 0.5 };
            rect(window, x, band_top + strip::TOP_ROW_H, 1., strip::FABRIC_H + strip::LABEL_LANE_H, tint(base, a));
            // Restore arc: a dashed connector back to the source station, ridden
            // in the label lane (design §1).
            if let Some(src) = st.arc_to {
                let sx = fab_x(src);
                let (a0, a1) = (sx.min(x), sx.max(x));
                let ay = band_top + strip::TOP_ROW_H + strip::LABEL_LANE_H - 3.;
                let mut dash = a0;
                while dash < a1 {
                    rect(window, dash, ay, 3., 1., tint(strip::SAGE, 0.55));
                    dash += 6.;
                }
            }
        }

        // --- The not-yet: everything right of a parked playhead dims one
        // alpha step — a STATIC encoding of position, so any paused frame
        // reads correctly (design §0's screenshot test; the corridor tester's
        // safety signal is seeing today still ON the strip, dimmed not gone).
        if editor.strip.parked && play_x < rail_x1 {
            rect(
                window,
                play_x,
                band_top,
                rail_x1 - play_x,
                strip::TOP_ROW_H + strip::LABEL_LANE_H + strip::FABRIC_H,
                tint(strip::GROUND, 0.55),
            );
        }

        // --- The rail, the thumb, the playhead, and the Compare pin ----------
        rect(window, rail_x0, rail_y, rail_w, 2., tint(strip::GREY, 0.55));
        let travel = strip_travel(rail_w, total);
        let frac = if total > 0. {
            bake.timeline.work_at(pos) / total
        } else {
            1.
        };
        let thumb_x = rail_x0 + frac * travel;
        // The thumb carries the highest contrast in the strip (design §0).
        let mut thumb = fill(
            Bounds::new(point(px(thumb_x - 5.), px(rail_y - 5.)), size(px(10.), px(10.))),
            rgb(0xEDE7D6),
        );
        thumb.corner_radii = Corners::all(px(5.));
        window.paint_quad(thumb);
        // The playhead line binds the rail to the fabric.
        if play_x >= rail_x0 && play_x <= rail_x1 {
            window.paint_quad(fill(
                Bounds::new(point(px(play_x - 0.5), px(rail_y)), size(px(1.), px(fab_bot - rail_y))),
                tint(0xEDE7D6, if editor.strip.parked { 0.85 } else { 0.4 }),
            ));
        }
        if let Some(pin) = editor.strip.pin_ms {
            let px_pin = fab_x(bake.timeline.work_at(pin));
            if px_pin >= rail_x0 && px_pin <= rail_x1 {
                window.paint_quad(fill(
                    Bounds::new(point(px(px_pin - 0.5), px(rail_y)), size(px(1.), px(fab_bot - rail_y))),
                    tint(0xEDE7D6, 0.3),
                ));
            }
        }

        // --- Shaped text: station labels + the date lane ---------------------
        for t in prepaint.labels.drain(..) {
            t.line.paint(t.origin, px(12.), TextAlign::Left, None, window, cx).ok();
        }
        for t in prepaint.dates.drain(..) {
            t.line.paint(t.origin, px(12.), TextAlign::Left, None, window, cx).ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Margin packer invariants, against random card stacks. Floor and viewport
    // are fixed; cards are sorted by anchor (the order margin_cards feeds).
    const PACK_FLOOR: f32 = 44.;
    const PACK_VP_BOTTOM: f32 = 800.;
    const PACK_GAP: f32 = 16.;

    proptest! {
        // INV1 (no overlap, EVER — the never-overlap rule, no active-card excuse)
        // + INV2 (every card at/below the floor) hold for ANY stack. The one
        // relaxation: cards the active card displaced UP off the top edge may sit
        // above the floor — they're culled into the `above` count by the caller,
        // never painted under the titlebar. Those are exactly the cards ABOVE the
        // active card's (sorted) index.
        #[test]
        fn packed_cards_never_overlap_and_clear_the_floor(
            raw in proptest::collection::vec((0f32..3000., 20f32..400., any::<bool>()), 0..12usize),
            active_sel in 0usize..64,
        ) {
            let n = raw.len();
            let active = if n == 0 { usize::MAX } else { active_sel % n };
            let mut items: Vec<PlaceItem> = raw
                .iter()
                .enumerate()
                .map(|(i, &(anchor, height, is_note))| PlaceItem {
                    anchor,
                    height,
                    pin: is_note || i == active,
                    active: i == active,
                })
                .collect();
            items.sort_by(|a, b| a.anchor.total_cmp(&b.anchor));
            // The active card's index AFTER the sort (the pre-sort `active` is stale).
            let active_idx = items.iter().position(|it| it.active);
            let tops = place_margin_cards(&items, PACK_FLOOR, PACK_VP_BOTTOM, PACK_GAP);
            for i in 0..tops.len() {
                // INV2: clears the floor — unless displaced UP above the active
                // card (i < active_idx), which is culled, not painted.
                if active_idx.is_none_or(|a| i >= a) {
                    prop_assert!(tops[i] >= PACK_FLOOR - 0.01, "card {i} above floor");
                }
                // INV1: no two cards overlap. No exceptions — displacement, not
                // overlap, is how the active card wins the lane.
                if i + 1 < tops.len() {
                    prop_assert!(
                        tops[i + 1] + 0.01 >= tops[i] + items[i].height + PACK_GAP,
                        "cards {i}/{} overlap", i + 1
                    );
                }
            }
        }

        // INV3 (strengthened): the SELECTED card is ALWAYS fully within the
        // viewport when its own height fits the lane — even when writer notes are
        // pinned in the slack above it (the bug class: a tall low-anchored note
        // above the chosen diagnosis used to shove it off the bottom, and the
        // count lied "Shown"). `is_note` pins now compete; Pass 3 must still win.
        #[test]
        fn selected_card_stays_fully_in_view(
            cards in proptest::collection::vec((0f32..3000., 20f32..100., any::<bool>()), 1..6usize),
            active_sel in 0usize..64,
        ) {
            let n = cards.len();
            let active = active_sel % n;
            let mut items: Vec<PlaceItem> = cards
                .iter()
                .enumerate()
                .map(|(i, &(anchor, height, is_note))| PlaceItem {
                    anchor,
                    height,
                    pin: is_note || i == active, // writer notes compete for the slack
                    active: i == active,
                })
                .collect();
            items.sort_by(|a, b| a.anchor.total_cmp(&b.anchor));
            let active = items.iter().position(|it| it.active).unwrap();
            let h = items[active].height;
            // The guarantee holds whenever the active card itself fits the lane —
            // no "whole stack fits" / "no competing pin" caveat anymore.
            prop_assume!(h <= PACK_VP_BOTTOM - PACK_FLOOR - CARD_BOTTOM_MARGIN);
            let tops = place_margin_cards(&items, PACK_FLOOR, PACK_VP_BOTTOM, PACK_GAP);
            prop_assert!(tops[active] >= PACK_FLOOR - 0.01, "active card under the floor");
            prop_assert!(
                tops[active] + h <= PACK_VP_BOTTOM + 0.01,
                "active card {} + {h} overruns viewport {PACK_VP_BOTTOM}", tops[active]
            );
        }

        // Card visibility is PURE GEOMETRY and honest: a card counted Above/Below
        // is genuinely off that edge; a Shown card genuinely overlaps the
        // viewport — so "N above / N below" never hides an on-screen card nor
        // claims an off-screen one. No active special case: the packer forces the
        // active card into view, so geometry alone decides (see card_slot).
        #[test]
        fn card_visibility_is_honest(
            top in -1500f32..1500.,
            height in 10f32..300.,
        ) {
            let (vp_top, vp_bottom) = (44f32, 800f32);
            match card_slot(top, height, vp_top, vp_bottom) {
                CardSlot::Shown => prop_assert!(
                    top + height > vp_top && top < vp_bottom,
                    "a Shown card overlaps the viewport"
                ),
                CardSlot::Above => prop_assert!(top + height <= vp_top + CARD_LINE_H),
                CardSlot::Below => prop_assert!(top >= vp_bottom - CARD_LINE_H),
            }
        }

        // reveal_scroll lands the revealed anchor at the NEAR edge, not a page
        // away (the "pill paginates instead of bringing one more into view" bug).
        // A below-reveal puts the anchor near the BOTTOM; an above-reveal near the
        // TOP — REVEAL_INSET from the edge, never ~a viewport away.
        #[test]
        fn reveal_scroll_lands_at_the_near_edge(
            anchor_y in 0f32..5000.,
            vp_h in 200f32..1200.,
            max_scroll in 0f32..5000.,
            below in any::<bool>(),
        ) {
            let s = reveal_scroll(anchor_y, vp_h, max_scroll, below);
            prop_assert!(s >= -0.01 && s <= max_scroll + 0.01, "scroll within range");
            let unclamped = if below {
                anchor_y - vp_h + REVEAL_INSET
            } else {
                anchor_y - REVEAL_INSET
            };
            // When the ideal target isn't clamped, the anchor lands exactly
            // REVEAL_INSET from the near edge (bottom for below, top for above).
            if unclamped >= 0. && unclamped <= max_scroll {
                let anchor_vp = anchor_y - s; // anchor's y within the viewport
                let want = if below { vp_h - REVEAL_INSET } else { REVEAL_INSET };
                prop_assert!(
                    (anchor_vp - want).abs() < 0.01,
                    "revealed anchor at {anchor_vp}, wanted near-edge {want}"
                );
            }
        }
    }

    #[test]
    fn note_at_char_accepts_the_trailing_boundary() {
        // Anchor [2,6): inside hits, and so does the trailing boundary c==6 (a
        // click on the right half of the last glyph snaps there) — the bug was
        // the strict `< end` test missing it. Outside still misses.
        let one = [(7u64, 2usize, 6usize)];
        assert_eq!(note_at_char(&one, 2), Some(7)); // leading boundary
        assert_eq!(note_at_char(&one, 4), Some(7)); // interior
        assert_eq!(note_at_char(&one, 6), Some(7)); // trailing boundary (was a dead zone)
        assert_eq!(note_at_char(&one, 7), None); // past the end
        assert_eq!(note_at_char(&one, 1), None); // before the start
        // Back-to-back anchors [2,6)[6,9): the shared boundary belongs to the
        // SECOND (it contains 6), so the trailing fallback never double-claims.
        let pair = [(7u64, 2, 6), (8u64, 6, 9)];
        assert_eq!(note_at_char(&pair, 6), Some(8));
        assert_eq!(note_at_char(&pair, 9), Some(8)); // the second's own trailing boundary
        assert_eq!(note_at_char(&[], 0), None);
    }

    #[test]
    fn auto_cut_fires_only_on_a_big_manuscript_deletion() {
        // At/over the threshold, a pure deletion inside the manuscript fires.
        assert!(auto_cut_qualifies("", AUTO_CUT_MIN_CHARS, 100, 0));
        assert!(auto_cut_qualifies("", 500, 100, 40));
        // Just under the threshold does NOT (both sides of the boundary case).
        assert!(!auto_cut_qualifies("", AUTO_CUT_MIN_CHARS - 1, 100, 0));
        // A replace-by-typing never files, however large the deleted run.
        assert!(!auto_cut_qualifies("x", 500, 100, 0));
        // A compost deletion (before the manuscript base) is a move, not a cut.
        assert!(!auto_cut_qualifies("", 200, 5, 40));
        // An empty selection never fires.
        assert!(!auto_cut_qualifies("", 0, 100, 0));
    }

    #[test]
    fn oldest_beyond_cap_recedes_the_oldest_passes() {
        use std::collections::HashSet;
        // At or under the budget: everything renders full.
        assert!(oldest_beyond_cap(&[(1, 1), (2, 1), (3, 2)], 5).is_empty());
        assert!(oldest_beyond_cap(&[(1, 1), (2, 2), (3, 3), (4, 4), (5, 5)], 5).is_empty());
        // Over it: the newest passes stay full (highest pass_id), oldest recede.
        // pass 3 = ids 5,6,7; pass 2 = 3,4; pass 1 = 1,2. cap 4 keeps {7,6,5,4}.
        let receded =
            oldest_beyond_cap(&[(1, 1), (2, 1), (3, 2), (4, 2), (5, 3), (6, 3), (7, 3)], 4);
        assert_eq!(receded, HashSet::from([1u64, 2, 3]));
    }

    #[test]
    fn staggered_ease_honours_delay_and_settles() {
        // Still through the whole delay window; settled at the end.
        assert_eq!(staggered_ease(0., 0.3), 0.);
        assert_eq!(staggered_ease(0.3, 0.3), 0.);
        assert_eq!(staggered_ease(1., 0.3), 1.);
        assert_eq!(staggered_ease(1.2, 0.3), 1.);
        // No delay = the plain in-out curve, symmetric around the midpoint.
        assert!((staggered_ease(0.5, 0.) - 0.5).abs() < 1e-6);
        // Monotone non-decreasing across the run (no overshoot, no rebound —
        // an overshooting margin card would "bounce", pure distraction).
        let mut prev = 0.;
        for i in 0..=100 {
            let e = staggered_ease(i as f32 / 100., 0.25);
            assert!(e >= prev - 1e-6, "dip at t={i}");
            assert!((0. ..=1.).contains(&e));
            prev = e;
        }
    }

    #[test]
    fn plan_lane_moves_tweens_only_discrete_repacks() {
        use std::collections::HashMap;
        let prev: HashMap<u64, f32> = [(1, 100.), (2, 300.)].into();
        // A moved card reports (id, from, to); a settled one stays quiet.
        let moves = plan_lane_moves(&prev, &[(1, 100.), (2, 240.)], false);
        assert_eq!(moves, vec![(2, 300., 240.)]);
        // Snap frames (scroll / composer / typing burst) never animate.
        assert!(plan_lane_moves(&prev, &[(1, 100.), (2, 240.)], true).is_empty());
        // A NEW id is an entrance (fade), never a move; a departed id drops.
        assert!(plan_lane_moves(&prev, &[(3, 50.)], false).is_empty());
        // Sub-pixel jitter is not a move.
        assert!(plan_lane_moves(&prev, &[(1, 100.4), (2, 300.)], false).is_empty());
    }

    #[test]
    fn door_filter_note_surfaces() {
        // Writer notes always surface, in either mode.
        assert!(note_surfaces(NoteKind::Note, "", true, true));
        assert!(note_surfaces(NoteKind::Note, "", false, false));
        // Diagnoses are hidden while drafting.
        assert!(!note_surfaces(NoteKind::Diagnosis, "developmental", true, false));
        // Reviewing: developmental shows; copy is held back iff a developmental
        // one is still open (the altitude order), else it shows.
        assert!(note_surfaces(NoteKind::Diagnosis, "developmental", false, true));
        assert!(!note_surfaces(NoteKind::Diagnosis, "copy", false, true));
        assert!(note_surfaces(NoteKind::Diagnosis, "copy", false, false));
    }

    #[test]
    fn flank_gate_decides_which_flanks_rise() {
        use FlankLeft::*;
        // (history_up, in_compost, lane_available, gutter_ok)
        let g = |h, c, l, gk| flank_gate(h, c, l, gk);

        // A history surface suppresses BOTH flanks — the past is read-only and
        // the right side is claimed (review H22). Every other input is ignored.
        for &(c, l, gk) in &[(false, true, true), (true, false, false)] {
            let r = g(true, c, l, gk);
            assert_eq!(r.left, None, "history suppresses the left flank");
            assert!(!r.right, "history suppresses the right flank");
        }

        // No history, wide gutter, live lane, manuscript selection: the grid and
        // the verb menu both rise (the resting case).
        let r = g(false, false, true, true);
        assert_eq!(r.left, Grid);
        assert!(r.right);

        // The narrow gutter narrows only the FORM of the left flank — it still
        // rises, as the formatting-only horizontal fallback (review H21). The
        // right menu is unaffected: its fallback keys on the LANE, not the gutter
        // (finding 57), so it stays up while the lane is available.
        let r = g(false, false, true, false);
        assert_eq!(r.left, Horizontal);
        assert!(r.right, "the gutter width never gates the right menu");

        // A compost-rail selection: the left (formatting) flank rises, the right
        // (verb) menu does not — the rail is the writer's scrap box (finding 108).
        let r = g(false, true, true, true);
        assert_eq!(r.left, Grid);
        assert!(!r.right, "no verb menu for a rail selection");

        // No lane (narrow-notes mode collapsed it): the right menu has nowhere to
        // occlude into, so it stands down even for a manuscript selection.
        let r = g(false, false, false, true);
        assert_eq!(r.left, Grid);
        assert!(!r.right, "no lane, no right menu");
    }

    #[test]
    fn editor_face_is_a_priority_not_exclusive_states() {
        use EditorFace::*;
        let mk = |needs_setup, error, cooking, ready, door_open| {
            face_for(&FaceInputs { needs_setup, error, cooking, ready, door_open })
        };
        // The spec's order, top-down: NeedsSetup > Error > cooking > ready >
        // Reading·N > idle. Each row lights SEVERAL inputs and asserts the
        // winner, so the priority is what's tested, not a lucky single input.
        assert_eq!(mk(true, true, true, true, true), NeedsSetup, "setup outranks all");
        assert_eq!(mk(false, true, true, true, true), Error, "error outranks cooking");
        assert_eq!(mk(false, false, true, true, true), Cooking, "cooking outranks a parked read");
        assert_eq!(mk(false, false, false, true, true), Ready, "a parked read outranks the door");
        assert_eq!(mk(false, false, false, false, true), Reading, "door open, nothing pending");
        assert_eq!(mk(false, false, false, false, false), Idle, "drafting, quiet");
        // The two combinations the review named (H32): door open × cooking, and
        // door open × a parked read, each resolve to exactly ONE face.
        assert_eq!(mk(false, false, true, false, true), Cooking);
        assert_eq!(mk(false, false, false, true, true), Ready);
        // Every face round-trips to its rig token.
        for f in [NeedsSetup, Error, Cooking, Ready, Reading, Idle] {
            assert_eq!(
                face_for(&FaceInputs {
                    needs_setup: f == NeedsSetup,
                    error: f == Error,
                    cooking: f == Cooking,
                    ready: f == Ready,
                    door_open: f == Reading,
                }),
                f,
                "{}",
                f.token()
            );
        }
    }

    #[test]
    fn pass_kind_threads_its_identity_end_to_end() {
        // The journal mode string and the Running label both derive from the
        // kind, so doubting can never collapse onto believing (review H27) and
        // a "doubting" string can never fall through to the line prompt.
        assert_eq!(PassKind::Believing.mode_str(), "believing");
        assert_eq!(PassKind::Doubting.mode_str(), "doubting");
        assert_eq!(PassKind::Diagnostic("copy".into()).mode_str(), "copy");
        assert_eq!(PassKind::Believing.run_label(), "believing read");
        assert_eq!(PassKind::Doubting.run_label(), "doubting read");
        assert_eq!(
            PassKind::Diagnostic("developmental".into()).run_label(),
            "developmental read"
        );
        // The three kinds are distinct, and a pinned depth is carried by value
        // (the fix for the sticky-mode trap — no diagnosis_mode mutation).
        assert_ne!(PassKind::Doubting, PassKind::Believing);
        assert_ne!(PassKind::Diagnostic("line".into()), PassKind::Diagnostic("copy".into()));
    }

    #[test]
    fn copy_gate_releases_when_the_last_developmental_closes() {
        use NoteKind::{Diagnosis, Note};
        // Gated while an open developmental query stands (line/copy alongside
        // don't matter — the developmental one is the gate).
        assert!(copy_gate_active(
            [(Diagnosis, "developmental"), (Diagnosis, "line")].into_iter()
        ));
        // Released the moment none is open: copy/line/believing-level cards
        // never gate, so closing the last developmental livens the row with no
        // pass re-run.
        assert!(!copy_gate_active([(Diagnosis, "copy"), (Diagnosis, "line")].into_iter()));
        // A writer note is not a diagnosis and never gates; empty never gates.
        assert!(!copy_gate_active([(Note, "developmental")].into_iter()));
        assert!(!copy_gate_active(std::iter::empty()));
    }

    #[test]
    fn thousands_separator() {
        assert_eq!(format_thousands(0), "0");
        assert_eq!(format_thousands(56), "56");
        assert_eq!(format_thousands(999), "999");
        assert_eq!(format_thousands(1234), "1,234");
        assert_eq!(format_thousands(1234567), "1,234,567");
    }

    #[test]
    fn provider_resolves_from_hand_typed_urls() {
        // A bare host substring is enough: a config edited by hand still
        // lights up the right chip and "Get a key" link.
        assert_eq!(provider_for("http://localhost:11434/v1").unwrap().label, "Local (Ollama)");
        assert_eq!(provider_for("https://openrouter.ai/api/v1/").unwrap().label, "OpenRouter");
        assert_eq!(provider_for("https://api.poe.com/v1").unwrap().label, "Poe");
        assert_eq!(provider_for("https://api.openai.com/v1").unwrap().label, "OpenAI");
        // Local needs no key; the cloud paths point somewhere to get one.
        assert!(provider_for("http://localhost:11434/v1").unwrap().key_url.is_none());
        assert!(provider_for("https://openrouter.ai/api/v1").unwrap().key_url.is_some());
        // Unknown / empty falls through to the free-text (Custom) state.
        assert!(provider_for("https://my.proxy.internal/v1").is_none());
        assert!(provider_for("").is_none());
        // Custom is never matched by host (its host_match is empty), so it
        // can't shadow a real provider.
        assert!(PROVIDERS.iter().any(|p| p.label == "Custom" && p.host_match.is_empty()));
    }

    #[test]
    fn local_model_pick_skips_embedders() {
        // Ollama commonly serves embedding models alongside chat ones; the
        // one-click path must not hand the diagnosis prompt to an embedder.
        assert_eq!(
            pick_local_model(vec!["nomic-embed-text".into(), "llama3.3:latest".into()]),
            Some("llama3.3:latest".into())
        );
        // No chat model? Still offer something rather than nothing.
        assert_eq!(
            pick_local_model(vec!["bge-large".into()]),
            Some("bge-large".into())
        );
        assert_eq!(pick_local_model(vec![]), None);
    }

    fn base() -> TextRun {
        TextRun {
            len: 0,
            font: gpui::font("PT Serif"),
            color: rgb(TEXT_COLOR).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }
    }

    #[test]
    fn bold_and_highlight_runs() {
        let par = 0..10;
        let spans = vec![
            (2..5, InlineAttr::Strong),
            (5..8, InlineAttr::Highlight),
        ];
        let runs = runs_for_paragraph(&par, &(0..0), None, &spans, &[], &[], &[], &base());
        // Segments: [0,2) plain, [2,5) bold, [5,8) highlight, [8,10) plain.
        assert_eq!(runs.len(), 4);
        assert_eq!(runs[1].font.weight, FontWeight::BOLD);
        assert!(runs[1].background_color.is_none());
        assert_eq!(runs[2].font.weight, FontWeight::default());
        assert!(runs[2].background_color.is_some());
        assert!(runs[0].background_color.is_none());
    }

    #[test]
    fn selection_composites_over_highlight() {
        // Markings stay visible through selection: the selected-and-
        // highlighted segment gets a blend distinct from both pure colors;
        // partial overlap splits into its own runs.
        let par = 0..6;
        let spans = vec![(0..6, InlineAttr::Highlight)];
        let runs = runs_for_paragraph(&par, &(2..4), None, &spans, &[], &[], &[], &base());
        let blended = blend_over(rgba(SELECTION_COLOR), rgba(HIGHLIGHT_COLOR));
        assert_eq!(runs[1].background_color, Some(blended.into()));
        assert_ne!(
            runs[1].background_color,
            Some(rgba(SELECTION_COLOR).into())
        );
        assert_ne!(
            runs[1].background_color,
            Some(rgba(HIGHLIGHT_COLOR).into())
        );
        // Outside the selection the pure highlight shows.
        assert_eq!(runs[0].background_color, Some(rgba(HIGHLIGHT_COLOR).into()));
        // Selection over plain text stays the plain selection color.
        let plain = runs_for_paragraph(&par, &(2..4), None, &[], &[], &[], &[], &base());
        assert_eq!(
            plain[1].background_color,
            Some(rgba(SELECTION_COLOR).into())
        );
    }

    #[test]
    fn auto_runs_group_from_their_first_entry() {
        let e = |manual: bool| HistoryEntry {
            name: String::new(),
            created_unix: 0,
            manual,
            text: String::new(),
            spans: SpanSet::default(),
            blocks: BlockMap::default(),
            delta: (0, 0),
            drift_sigma: None,
        };
        // [named, auto, auto, named, auto]
        let entries = vec![e(true), e(false), e(false), e(true), e(false)];
        assert_eq!(auto_group_start(&entries, 1), 1);
        assert_eq!(auto_group_start(&entries, 2), 1);
        assert_eq!(auto_group_start(&entries, 4), 4);
        // A leading auto run starts at index 0.
        let entries = vec![e(false), e(false), e(true)];
        assert_eq!(auto_group_start(&entries, 1), 0);
    }

    #[test]
    fn footnote_numbers_follow_ref_order_not_stored_ids() {
        // Stored ids "2" and "1" appear in reverse order in the text; the
        // painted numbers follow text order (DESIGN §2-footnotes). The
        // orphan def "9" (its ref deleted) gets the next number.
        let refs = vec![(40usize, "1"), (10usize, "2")];
        let kinds = vec![
            BlockKind::Paragraph,
            BlockKind::FootnoteDef { id: "1".into() },
            BlockKind::FootnoteDef { id: "2".into() },
            BlockKind::FootnoteDef { id: "9".into() },
        ];
        let map = footnote_numbers(&refs, &kinds);
        assert_eq!(map.get("2"), Some(&1));
        assert_eq!(map.get("1"), Some(&2));
        assert_eq!(map.get("9"), Some(&3));
    }

    #[test]
    fn footnote_ref_carrier_paints_transparent_without_pill() {
        // The carrier digit keeps its advance but must not ink: the
        // superior figure is painted over it in the paint phase.
        let par = 0..5;
        let spans = vec![(2..3, InlineAttr::FootnoteRef("1".into()))];
        let runs = runs_for_paragraph(&par, &(0..0), None, &spans, &[], &[], &[], &base());
        assert_eq!(runs.len(), 3);
        assert_eq!(runs[1].color.a, 0.);
        assert!(runs[1].background_color.is_none());
        // Neighbors stay inked.
        assert_ne!(runs[0].color.a, 0.);
    }

    #[test]
    fn code_run_switches_family_and_marked_text_underlines() {
        let par = 0..8;
        let spans = vec![(0..4, InlineAttr::Code)];
        let runs = runs_for_paragraph(&par, &(0..0), Some(&(4..8)), &spans, &[], &[], &[], &base());
        assert_eq!(runs[0].font.family.as_ref(), CODE_FONT);
        assert!(runs[1].underline.is_some());
    }

    #[test]
    fn word_motion_survives_long_blank_run() {
        // A pathological run of empty paragraphs must not blow the stack: word
        // motion has to iterate, not recurse one frame per line. (Recursive,
        // this overflows the 2 MB test-thread stack and aborts the binary.)
        let n = 200_000;

        let mut text = String::with_capacity(n + 4);
        for _ in 0..n {
            text.push('\n');
        }
        text.push_str("word");
        let doc = Document::new(&text, SpanSet::default(), BlockMap::default());
        assert_eq!(previous_word_boundary(&doc, n), 0);

        let mut text2 = String::with_capacity(n + 4);
        text2.push_str("word");
        for _ in 0..n {
            text2.push('\n');
        }
        let doc2 = Document::new(&text2, SpanSet::default(), BlockMap::default());
        let len = doc2.len_bytes();
        assert_eq!(next_word_boundary(&doc2, 0), 4); // selects "word"
        assert_eq!(next_word_boundary(&doc2, 4), len); // blank run -> doc end
    }

    // --- Margin-card interaction FSM (CardFocus) -------------------------
    // The class of bugs these guard: a committed note rendering blank, a
    // composer lingering on a deselected card, a draft leaking onto the wrong
    // card. They were all "two booleans disagreed." The enum + these decisions
    // make "disagree" unrepresentable; the tests pin the decisions.

    #[test]
    fn card_body_renders_exactly_one_thing() {
        // The render decision is total and exclusive: composing → composer,
        // otherwise → text. There is no input that yields both or neither (that
        // was the "input AND label, same contents" bug, and the blank-card bug).
        assert!(matches!(card_body(true), CardBody::Composer));
        assert!(matches!(card_body(false), CardBody::Text));
    }

    #[test]
    fn idle_focus_addresses_no_card() {
        let f = CardFocus::Idle;
        assert_eq!(f.active_id(), None);
        assert_eq!(f.composing_id(), None);
        assert!(f.input().is_none());
    }

    #[test]
    fn selected_focus_is_active_but_never_composing() {
        // A selected card (an AI diagnosis, or a note whose composer resolved)
        // is highlighted but has no open composer — so the body label shows
        // (card_body(false) == Text), which is exactly the fix for the
        // committed-note-renders-blank bug.
        let f = CardFocus::Selected(7);
        assert_eq!(f.active_id(), Some(7));
        assert_eq!(f.composing_id(), None);
        assert!(f.input().is_none());
        assert!(matches!(card_body(f.composing_id() == Some(7)), CardBody::Text));
    }

    #[test]
    fn composing_implies_active_on_the_same_card() {
        // The corruption-class invariant, made structural: whatever is being
        // composed is also what's active, and the id is single-valued. The
        // composer and the note id share one variant, so the draft mirror and
        // the composer render (both read `composing_id`) can never address a
        // card other than `active_id`. Asserted here over the id projection;
        // the `input` field is non-None by the variant's shape (it cannot be
        // constructed without an `Entity<TextField>`).
        for f in [CardFocus::Idle, CardFocus::Selected(3), CardFocus::Selected(9)] {
            if let Some(cid) = f.composing_id() {
                assert_eq!(f.active_id(), Some(cid), "composing must be active");
            }
        }
        // The only state carrying a composer is `Composing`, and its `id` and
        // `input` are one variant's two fields — there is no way to hold an
        // input without the id it edits, nor to point them at different notes.
        // That is the structural guarantee that retired the draft-leak bug; it
        // needs no test because it cannot be constructed wrong (the entity-
        // bearing variant would take gpui `test-support`, deliberately not on).
    }
}
