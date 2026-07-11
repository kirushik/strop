//! The one reusable plain-text input field. Every small box of letters in the
//! app — the margin-note composer, the command palette, the AI-settings fields,
//! the rename box — is a `TextField`. (The main prose canvas is NOT: it is
//! Loro-backed, multi-block, attributed, and lives in `editor.rs`.)
//!
//! A `TextField` is the full contract a "box you can type in" implies: a real
//! caret, a selection, grapheme-correct motion (UAX#29 for words), the whole
//! keyboard editing set, IME preedit, clipboard, and the full mouse model —
//! click-to-place, drag-select, double-click-word, triple-click-line, and
//! word/line-snapped drag-extend. The single-line variant scrolls to keep the
//! caret in view; the multi-line variant soft-wraps and grows downward.
//!
//! The pixel-geometry helpers (`*_grapheme`, `*_word`, `word_range_at`,
//! `line_range_at`, the utf16/char conversions) are pure and unit-tested; the
//! widget wires them to GPUI's IME plumbing and the painted geometry.

use std::ops::Range;
use std::sync::Arc;

use gpui::{
    App, AvailableSpace, Bounds, ClipboardItem, Context, Element, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, FocusHandle, Focusable, GlobalElementId, KeyBinding, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, SharedString, Style,
    TextAlign, TextRun, UTF16Selection, Window, WrappedLine, actions, div, fill, point,
    prelude::*, px, relative, rgb, rgba, size,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::draw_guard::{DrawGuard, EntityUpdateExt as _};
use crate::editor::{CARD_LINE_H, COMPOSER_INNER_W};
use crate::theme::{FIELD_SELECTION_BG, RULE_COLOR, TEXT_COLOR};

// === Pure text/caret helpers (unit-tested; no GPUI) ======================

/// The grapheme-cluster boundary one cluster left of byte `i` (`0` at start).
/// Grapheme-aware so a caret never lands inside an emoji ZWJ sequence or a
/// combining mark — the bug a byte/char step would let through.
fn prev_grapheme(s: &str, i: usize) -> usize {
    if i == 0 {
        return 0;
    }
    let i = i.min(s.len());
    s[..i].grapheme_indices(true).next_back().map_or(0, |(ix, _)| ix)
}

/// The grapheme-cluster boundary one cluster right of byte `i` (`len` at end).
fn next_grapheme(s: &str, i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    s[i..]
        .grapheme_indices(true)
        .nth(1)
        .map_or(s.len(), |(ix, _)| i + ix)
}

/// Word-left: the start of the previous word (UAX#29 word bounds, skipping
/// punctuation/space-only segments to alphanumeric-leading ones). Identical
/// semantics to the prose canvas's `previous_word_boundary`, so the gesture
/// feels the same in every field. Crosses logical lines at their edges.
fn prev_word(s: &str, mut i: usize) -> usize {
    i = i.min(s.len());
    loop {
        if i == 0 {
            return 0;
        }
        let ls = line_start(s, i);
        if i == ls {
            // At a line start: step over the preceding newline and retry on the
            // line above (iterate, never recurse — a long blank run mustn't grow
            // the stack).
            i -= 1;
            continue;
        }
        let line = &s[ls..i];
        return line
            .split_word_bound_indices()
            .rev()
            .find(|(_, seg)| seg.chars().next().is_some_and(char::is_alphanumeric))
            .map_or(ls, |(ix, _)| ls + ix);
    }
}

/// Word-right: the end of the next word. Mirror of `prev_word`.
fn next_word(s: &str, mut i: usize) -> usize {
    let len = s.len();
    i = i.min(len);
    loop {
        if i >= len {
            return len;
        }
        let le = line_end(s, i);
        if i == le {
            // Sitting on a newline: step over it onto the next line.
            i += 1;
            continue;
        }
        let line = &s[i..le];
        return line
            .split_word_bound_indices()
            .find(|(_, seg)| seg.chars().next().is_some_and(char::is_alphanumeric))
            .map_or(le, |(ix, seg)| i + ix + seg.len())
            .min(len);
    }
}

/// The word segment containing byte `i`, for double-click selection. Scoped to
/// the logical line; returns the UAX#29 segment under the caret (which may be a
/// punctuation or space run, matching the prose canvas).
fn word_range_at(s: &str, i: usize) -> Range<usize> {
    let ls = line_start(s, i);
    let le = line_end(s, i);
    if ls == le {
        return ls..le;
    }
    let local = (i.min(le) - ls).min(le - ls - 1);
    let line = &s[ls..le];
    for (ix, seg) in line.split_word_bound_indices() {
        if ix <= local && local < ix + seg.len() {
            return ls + ix..ls + ix + seg.len();
        }
    }
    ls..le
}

/// The logical line containing byte `i`, for triple-click selection.
fn line_range_at(s: &str, i: usize) -> Range<usize> {
    line_start(s, i)..line_end(s, i)
}

/// Start of the logical line (after the previous `\n`, or 0).
fn line_start(s: &str, i: usize) -> usize {
    s[..i.min(s.len())].rfind('\n').map(|b| b + 1).unwrap_or(0)
}

/// End of the logical line (the next `\n`, or `len`).
fn line_end(s: &str, i: usize) -> usize {
    let i = i.min(s.len());
    s[i..].find('\n').map(|b| i + b).unwrap_or(s.len())
}

fn byte_to_utf16(s: &str, byte: usize) -> usize {
    s[..byte.min(s.len())].chars().map(|c| c.len_utf16()).sum()
}

fn utf16_to_byte(s: &str, utf16: usize) -> usize {
    let mut u = 0;
    for (b, c) in s.char_indices() {
        if u >= utf16 {
            return b;
        }
        u += c.len_utf16();
    }
    s.len()
}

fn byte_to_char_idx(s: &str, byte: usize) -> usize {
    s[..byte.min(s.len())].chars().count()
}

fn char_idx_to_byte(s: &str, ch: usize) -> usize {
    s.char_indices().nth(ch).map(|(b, _)| b).unwrap_or(s.len())
}

/// The single-line vs multiline newline policy, applied at the splice point so
/// every entry path (typing, paste, OS insertText/dictation, IME commit) obeys
/// it. A single-line field can hold NO line break (a filename, a palette query,
/// an API key): \n / \r flatten to a space, a CRLF collapsing to one. The
/// multiline composer keeps breaks, normalizing CRLF/CR to a lone \n.
fn sanitize_for_field(text: &str, multiline: bool) -> String {
    if multiline {
        text.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        text.replace("\r\n", " ").replace(['\n', '\r'], " ")
    }
}

// === The widget ==========================================================

/// Drag-selection granularity, set by the initiating click count.
#[derive(Clone, Copy, PartialEq)]
enum DragUnit {
    Char,
    Word,
    Line,
}

pub struct TextField {
    pub(crate) focus_handle: FocusHandle,
    /// The real text. `pub(crate)` because the parent editor reads it back
    /// (palette query, settings values, rename title, …).
    pub(crate) content: String,
    /// Caret byte offset into `content`, always on a grapheme boundary.
    cursor: usize,
    /// Selection anchor byte offset; `Some` ⇒ the selection spans
    /// `anchor`↔`cursor`. Cleared by any non-extending caret move.
    anchor: Option<usize>,
    /// IME preedit byte range within `content`, while composing.
    marked: Option<Range<usize>>,
    /// Keymap context: which binding set routes to this field. "NoteInput"
    /// (plain single-line), "NoteComposer" (multi-line), "PaletteInput"
    /// (up/down move the list), "SettingsInput" (tab/up/down/ctrl-enter).
    key_context: &'static str,
    /// Secret-field display: dots except the last 4 chars (the API key). The
    /// real content is untouched — typing and paste work normally; copy/cut
    /// of a masked field is refused so the secret never leaves.
    masked: bool,
    /// Soft-wrap across rows and grow downward, instead of single-line scroll.
    /// Only the in-card note composer sets this (its lane is ~5 words wide).
    multiline: bool,
    /// No own frame/fill — the host draws the one frame (the omnibar).
    bare: bool,
    /// Geometry of the last paint, for click hit-testing and vertical motion.
    /// Maps over the DISPLAY string; caret↔geometry conversions go through char
    /// index so a masked field (dots ≠ real bytes) still lands the caret right.
    geometry: Option<FieldGeometry>,
    /// True between a mouse-down and the matching mouse-up: drag extends.
    is_selecting: bool,
    /// The unit a drag extends by (set by click count): char / word / line.
    drag_unit: DragUnit,
    /// The unit (or point) the initiating click fixed; a drag unions the unit
    /// under the pointer with this, so word/line drags grow whole units.
    selection_origin: Option<Range<usize>>,
    /// Where glyph x=0 painted last frame, WINDOW-relative (single-line folds
    /// in the caret-scroll shift; wrapped is the bounds origin). The OS
    /// hit-test path (`character_index_for_point`) is handed a window point, so
    /// it subtracts this to reach field-local space — like the mouse handlers.
    text_origin: Point<Pixels>,
}

/// What the field painted last frame, enough to map a point ↔ a caret index.
/// The multiline composer is a STACK of hard lines (split on `\n`); each hard
/// line soft-wraps within the box on its own. `Wrapped` carries one row per
/// hard line so the caret, clicks, and vertical motion map across them — the
/// single-line-only mapping was the "Shift+Enter jumps to line 0" bug.
// The large variant (`Single`, a whole `ShapedLine`) is the COMMON single-line
// case; boxing it to appease `large_enum_variant` would push that hot path onto
// the heap for a per-field layout-size win that never matters (one value, held
// across frames in `Option<FieldGeometry>`).
#[allow(clippy::large_enum_variant)]
enum FieldGeometry {
    Single(gpui::ShapedLine),
    Wrapped(Vec<WrappedRow>),
}

/// One hard line ("\n"-delimited) of the multiline composer, as painted.
struct WrappedRow {
    /// The shaped layout for this hard line (it may itself soft-wrap).
    layout: Arc<gpui::WrappedLineLayout>,
    /// Display-byte offset where this hard line begins in the content.
    start: usize,
    /// Y offset of this row's top within the field. (A point lands in the last
    /// row whose `top` is at or above it — heights need not be stored.)
    top: Pixels,
}

/// Byte offsets where each hard line ("\n"-delimited) begins. `len() ==
/// number of `\n` + 1`, matching how `shape_text` splits — so the shaped
/// lines align index-for-index with these starts.
fn hard_line_starts(s: &str) -> Vec<usize> {
    let mut v = vec![0];
    for (i, b) in s.bytes().enumerate() {
        if b == b'\n' {
            v.push(i + 1);
        }
    }
    v
}

/// Map a display-byte offset to its (x, y) within the wrapped composer.
fn wrapped_position(rows: &[WrappedRow], disp_byte: usize, line_height: Pixels) -> Point<Pixels> {
    if rows.is_empty() {
        return point(px(0.), px(0.));
    }
    let i = rows.iter().rposition(|r| r.start <= disp_byte).unwrap_or(0);
    let row = &rows[i];
    let p = row
        .layout
        .position_for_index(disp_byte - row.start, line_height)
        .unwrap_or_default();
    point(p.x, row.top + p.y)
}

/// Map a field-local point to the nearest display-byte across the wrapped rows.
fn wrapped_byte_at(rows: &[WrappedRow], pt: Point<Pixels>, line_height: Pixels) -> usize {
    if rows.is_empty() {
        return 0;
    }
    let i = rows
        .iter()
        .rposition(|r| pt.y >= r.top)
        .unwrap_or(0)
        .min(rows.len() - 1);
    let row = &rows[i];
    let local = row
        .layout
        .closest_index_for_position(point(pt.x, pt.y - row.top), line_height)
        .unwrap_or_else(|e| e);
    row.start + local
}

pub enum TextFieldEvent {
    Commit(String),
    Cancel,
}

impl gpui::EventEmitter<TextFieldEvent> for TextField {}

impl TextField {
    fn base(cx: &mut Context<Self>, content: String, key_context: &'static str) -> Self {
        let cursor = content.len();
        Self {
            focus_handle: cx.focus_handle(),
            content,
            cursor,
            anchor: None,
            marked: None,
            key_context,
            masked: false,
            multiline: false,
            bare: false,
            geometry: None,
            is_selecting: false,
            drag_unit: DragUnit::Char,
            selection_origin: None,
            text_origin: Point::default(),
        }
    }

    /// A plain single-line field (rename, replace, alt-text, goal, …).
    pub(crate) fn single(cx: &mut Context<Self>, content: String) -> Self {
        Self::base(cx, content, "NoteInput")
    }

    /// A single-line field that opens with its content selected — the
    /// prefilled-datum idiom (the goal chip): the current value is shown,
    /// typing replaces it, and erasing it erases the datum.
    pub(crate) fn single_selected(cx: &mut Context<Self>, content: String) -> Self {
        let mut field = Self::base(cx, content, "NoteInput");
        if !field.content.is_empty() {
            field.anchor = Some(0);
        }
        field
    }

    /// The in-card note composer: a multi-line, soft-wrapping field. Its own key
    /// context carries the extras single-line fields don't want: up/down caret
    /// rows and shift/ctrl-enter line breaks.
    pub(crate) fn multiline(cx: &mut Context<Self>, content: String) -> Self {
        Self {
            multiline: true,
            key_context: "NoteComposer",
            ..Self::base(cx, content, "NoteComposer")
        }
    }

    /// The command palette's query field: editing chords plus up/down row
    /// motion. `bare` — the omnibar pill draws the one frame (06 §1: never a
    /// frame inside a frame); the field brings only its text and caret.
    pub(crate) fn palette(cx: &mut Context<Self>, content: String) -> Self {
        Self {
            bare: true,
            ..Self::base(cx, content, "PaletteInput")
        }
    }

    /// A field of the AI settings panel (F4): its own context so tab/up/down/
    /// ctrl-enter mean panel things; `masked` for the key.
    pub(crate) fn settings(cx: &mut Context<Self>, content: String, masked: bool) -> Self {
        Self {
            masked,
            ..Self::base(cx, content, "SettingsInput")
        }
    }

    /// Introspection for the smoke rig: the field's key context, and its caret +
    /// selection as CHAR indices (so a test can assert selection rather than
    /// eyeball it). Char indices, not bytes, so multibyte content reads cleanly.
    pub(crate) fn debug_caret(&self) -> (&'static str, usize, [usize; 2]) {
        let r = self.sel_range();
        (
            self.key_context,
            byte_to_char_idx(&self.content, self.cursor),
            [
                byte_to_char_idx(&self.content, r.start),
                byte_to_char_idx(&self.content, r.end),
            ],
        )
    }

    /// The display string: real content, except a masked field shows dots for
    /// all but the last 4 chars. Shared by measurement and paint so they agree.
    fn display_content(&self) -> String {
        if self.masked {
            let chars: Vec<char> = self.content.chars().collect();
            let visible_from = chars.len().saturating_sub(4);
            chars
                .iter()
                .enumerate()
                .map(|(i, c)| if i < visible_from { '•' } else { *c })
                .collect()
        } else {
            self.content.clone()
        }
    }

    /// The selected byte range (`start <= end`), or an empty range at the caret.
    fn sel_range(&self) -> Range<usize> {
        match self.anchor {
            Some(a) if a <= self.cursor => a..self.cursor,
            Some(a) => self.cursor..a,
            None => self.cursor..self.cursor,
        }
    }

    fn has_selection(&self) -> bool {
        self.anchor.is_some_and(|a| a != self.cursor)
    }

    /// Replace `range` with `text`, leaving the caret after it and clearing the
    /// selection + any IME mark. The single splice point — typing, paste,
    /// delete, and IME commit all route here, so every edit happens AT THE
    /// CARET, never at the end.
    fn replace(&mut self, range: Range<usize>, text: &str) {
        // Newline policy lives HERE, the single splice point, so EVERY path —
        // typing, paste, OS insertText/dictation, IME commit — obeys it, not
        // just the old paste guard. Flatten FIRST, then splice and measure, so
        // the caret offset uses the FLATTENED length and never desyncs.
        let text = sanitize_for_field(text, self.multiline);
        self.content.replace_range(range.clone(), &text);
        self.cursor = range.start + text.len();
        self.anchor = None;
        self.marked = None;
    }

    /// Move the caret to byte `target`; `extend` keeps/starts a selection,
    /// otherwise the selection collapses.
    fn move_to(&mut self, target: usize, extend: bool) {
        if extend {
            if self.anchor.is_none() {
                self.anchor = Some(self.cursor);
            }
        } else {
            self.anchor = None;
        }
        self.cursor = target.min(self.content.len());
        if self.anchor == Some(self.cursor) {
            self.anchor = None;
        }
    }

    /// Horizontal / logical caret target for a motion (everything but up/down,
    /// which need paint geometry and live in `move_vertical`).
    fn horizontal_target(&self, motion: Motion) -> usize {
        let s = &self.content;
        let c = self.cursor;
        match motion {
            Motion::Left => prev_grapheme(s, c),
            Motion::Right => next_grapheme(s, c),
            Motion::WordLeft => prev_word(s, c),
            Motion::WordRight => next_word(s, c),
            Motion::LineStart => line_start(s, c),
            Motion::LineEnd => line_end(s, c),
            Motion::DocStart => 0,
            Motion::DocEnd => s.len(),
            Motion::Up | Motion::Down => c,
        }
    }

    /// Dispatch a `FieldMove`: vertical motions need geometry, the rest are pure.
    fn do_move(&mut self, motion: Motion, extend: bool, cx: &mut Context<Self>) {
        match motion {
            Motion::Up => self.move_vertical(false, extend, cx),
            Motion::Down => self.move_vertical(true, extend, cx),
            _ => self.apply_motion(motion, extend, cx),
        }
    }

    /// Apply a horizontal/logical motion (keyboard arrows, word, home/end).
    fn apply_motion(&mut self, motion: Motion, extend: bool, cx: &mut Context<Self>) {
        // A non-extending Left/Right with an active selection collapses to the
        // selection edge WITHOUT moving past it.
        let collapsing =
            !extend && self.has_selection() && matches!(motion, Motion::Left | Motion::Right);
        let target = if collapsing {
            match motion {
                Motion::Left => self.sel_range().start,
                _ => self.sel_range().end,
            }
        } else {
            self.horizontal_target(motion)
        };
        self.move_to(target, extend);
        cx.notify();
    }

    /// Vertical caret motion in the multi-line composer: walk one row up/down at
    /// the current x, via the last paint's wrapped geometry.
    fn move_vertical(&mut self, down: bool, extend: bool, cx: &mut Context<Self>) {
        let line_height = px(CARD_LINE_H);
        let target = match &self.geometry {
            Some(FieldGeometry::Wrapped(rows)) if !rows.is_empty() => {
                let here = wrapped_position(rows, self.cursor_display_byte(), line_height);
                let dy = if down { line_height } else { -line_height };
                let probe = point(here.x, (here.y + dy).max(px(0.)));
                let disp = wrapped_byte_at(rows, probe, line_height);
                self.display_byte_to_cursor(disp)
            }
            // Single-line (or no geometry): up→start, down→end.
            _ => {
                if down {
                    self.content.len()
                } else {
                    0
                }
            }
        };
        self.move_to(target, extend);
        cx.notify();
    }

    /// A real-content byte offset translated into the DISPLAY string (masked
    /// fields: dots ≠ real bytes; map through char index, which both share).
    fn real_to_display_byte(&self, byte: usize) -> usize {
        if self.masked {
            let ch = byte_to_char_idx(&self.content, byte);
            char_idx_to_byte(&self.display_content(), ch)
        } else {
            byte
        }
    }

    /// The caret, as a byte offset into the DISPLAY string.
    fn cursor_display_byte(&self) -> usize {
        self.real_to_display_byte(self.cursor)
    }

    /// A DISPLAY-string byte offset back to a real-content caret byte.
    fn display_byte_to_cursor(&self, disp_byte: usize) -> usize {
        if self.masked {
            let ch = byte_to_char_idx(&self.display_content(), disp_byte);
            char_idx_to_byte(&self.content, ch)
        } else {
            disp_byte.min(self.content.len())
        }
    }

    /// The real-content byte under a field-local point (the element passes the
    /// point already relative to the text origin). `None` if nothing painted yet.
    fn byte_at_local(&self, local: Point<Pixels>) -> Option<usize> {
        let disp = match &self.geometry {
            Some(FieldGeometry::Single(line)) => line.closest_index_for_x(local.x),
            Some(FieldGeometry::Wrapped(rows)) if !rows.is_empty() => {
                wrapped_byte_at(rows, local, px(CARD_LINE_H))
            }
            _ => return None,
        };
        Some(self.display_byte_to_cursor(disp))
    }

    /// Mouse-down: begin a selection at the click. `click_count` picks the
    /// granularity (1 char, 2 word, 3 line); `shift` extends the existing
    /// selection instead of starting fresh.
    fn begin_select(
        &mut self,
        local: Point<Pixels>,
        click_count: usize,
        shift: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(ix) = self.byte_at_local(local) else {
            return;
        };
        self.is_selecting = true;
        match click_count {
            1 => {
                self.drag_unit = DragUnit::Char;
                // Shift-click keeps the current anchor (or current caret) as the
                // fixed end; a plain click drops both ends at the click.
                let anchor_pt = if shift {
                    self.anchor.unwrap_or(self.cursor)
                } else {
                    ix
                };
                self.selection_origin = Some(anchor_pt..anchor_pt);
                self.anchor = Some(anchor_pt);
                self.cursor = ix;
            }
            2 => {
                self.drag_unit = DragUnit::Word;
                let w = word_range_at(&self.content, ix);
                self.selection_origin = Some(w.clone());
                self.anchor = Some(w.start);
                self.cursor = w.end;
            }
            _ => {
                self.drag_unit = DragUnit::Line;
                let l = line_range_at(&self.content, ix);
                self.selection_origin = Some(l.clone());
                self.anchor = Some(l.start);
                self.cursor = l.end;
            }
        }
        if self.anchor == Some(self.cursor) {
            self.anchor = None;
        }
        cx.notify();
    }

    /// Mouse-move while a drag is active: extend the selection to the pointer,
    /// snapping to whole words/lines when the drag began as a double/triple click.
    fn drag_to(&mut self, local: Point<Pixels>, cx: &mut Context<Self>) {
        if !self.is_selecting {
            return;
        }
        let Some(ix) = self.byte_at_local(local) else {
            return;
        };
        let origin = self
            .selection_origin
            .clone()
            .unwrap_or(self.cursor..self.cursor);
        match self.drag_unit {
            DragUnit::Char => {
                self.anchor = Some(origin.start);
                self.cursor = ix;
            }
            DragUnit::Word | DragUnit::Line => {
                let unit = if self.drag_unit == DragUnit::Word {
                    word_range_at(&self.content, ix)
                } else {
                    line_range_at(&self.content, ix)
                };
                // The fixed end is the origin edge farthest from the pointer; the
                // caret rides the near edge so the whole unit is always covered.
                if unit.start < origin.start {
                    self.anchor = Some(origin.end);
                    self.cursor = unit.start;
                } else {
                    self.anchor = Some(origin.start);
                    self.cursor = unit.end;
                }
            }
        }
        if self.anchor == Some(self.cursor) {
            self.anchor = None;
        }
        cx.notify();
    }

    fn end_select(&mut self, cx: &mut Context<Self>) {
        self.is_selecting = false;
        cx.notify();
    }

    fn select_all(&mut self, _: &FieldSelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.anchor = Some(0);
        self.cursor = self.content.len();
        cx.notify();
    }

    fn commit(&mut self, _: &FieldCommit, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextFieldEvent::Commit(self.content.clone()));
    }

    fn cancel(&mut self, _: &FieldCancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextFieldEvent::Cancel);
    }

    /// A hard line break (multiline only): Shift+Enter / Ctrl+Enter. Single-line
    /// fields don't bind it, so Enter there is always commit.
    fn newline(&mut self, _: &FieldNewline, _: &mut Window, cx: &mut Context<Self>) {
        self.replace(self.sel_range(), "\n");
        cx.notify();
    }

    fn backspace(&mut self, _: &FieldBackspace, _: &mut Window, cx: &mut Context<Self>) {
        if self.has_selection() {
            self.replace(self.sel_range(), "");
        } else {
            let from = prev_grapheme(&self.content, self.cursor);
            self.replace(from..self.cursor, "");
        }
        cx.notify();
    }

    /// ctrl-backspace: delete the word (or selection) left of the caret.
    fn backspace_word(&mut self, _: &FieldBackspaceWord, _: &mut Window, cx: &mut Context<Self>) {
        if self.has_selection() {
            self.replace(self.sel_range(), "");
        } else {
            let from = prev_word(&self.content, self.cursor);
            self.replace(from..self.cursor, "");
        }
        cx.notify();
    }

    /// Delete: the grapheme (or selection) to the RIGHT of the caret.
    fn delete(&mut self, _: &FieldDelete, _: &mut Window, cx: &mut Context<Self>) {
        if self.has_selection() {
            self.replace(self.sel_range(), "");
        } else {
            let to = next_grapheme(&self.content, self.cursor);
            self.replace(self.cursor..to, "");
        }
        cx.notify();
    }

    /// ctrl-delete: the word (or selection) to the right of the caret.
    fn delete_word(&mut self, _: &FieldDeleteWord, _: &mut Window, cx: &mut Context<Self>) {
        if self.has_selection() {
            self.replace(self.sel_range(), "");
        } else {
            let to = next_word(&self.content, self.cursor);
            self.replace(self.cursor..to, "");
        }
        cx.notify();
    }

    fn copy(&mut self, _: &FieldCopy, _: &mut Window, cx: &mut Context<Self>) {
        // Never copy a masked secret out; everything else copies the selection.
        if self.masked || !self.has_selection() {
            return;
        }
        let text = self.content[self.sel_range()].to_owned();
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    fn cut(&mut self, _: &FieldCut, _: &mut Window, cx: &mut Context<Self>) {
        if self.masked || !self.has_selection() {
            return;
        }
        let text = self.content[self.sel_range()].to_owned();
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.replace(self.sel_range(), "");
        cx.notify();
    }

    /// ctrl-v in any field (DESIGN §0.6 law 1): paste lands in the focused
    /// field, never in the document behind it. Single-line fields flatten
    /// newlines to spaces; the multi-line composer keeps them. Masked fields
    /// paste normally.
    fn paste(&mut self, _: &FieldPaste, window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .or_else(crate::smoke::clipboard_override)
        else {
            return;
        };
        // The newline policy now lives in `replace` (the single splice point),
        // so paste, OS insertText/dictation, and IME commit all obey it; no
        // per-path flattening here (it would only double-normalize). The shared
        // splice path inserts at the caret, over the selection.
        self.replace_text_in_range(None, &text, window, cx);
    }
}

actions!(
    text_field,
    [
        FieldCommit,
        FieldCancel,
        FieldNewline,
        FieldBackspace,
        FieldBackspaceWord,
        FieldDelete,
        FieldDeleteWord,
        FieldTab,
        FieldPaste,
        FieldCopy,
        FieldCut,
        FieldSelectAll,
    ]
);

/// A caret motion. Up/Down go through `move_vertical` (they need the wrapped
/// paint geometry); the rest are pure (`horizontal_target`).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Motion {
    Left,
    Right,
    WordLeft,
    WordRight,
    LineStart,
    LineEnd,
    DocStart,
    DocEnd,
    Up,
    Down,
}

/// One action for every caret move: `select` extends the selection (Shift+).
/// Collapsing 20 keystrokes into one keeps the binding table readable and the
/// handler a single dispatch.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = text_field, no_json)]
pub(crate) struct FieldMove {
    pub(crate) motion: Motion,
    pub(crate) select: bool,
}

/// The shared caret-movement / selection / delete / clipboard bindings every
/// text field honors. `FieldMove { motion, select }` collapses the 16 movement
/// chords into one action.
pub(crate) fn field_editing_bindings(ctx: &'static str) -> Vec<KeyBinding> {
    use Motion::*;
    let mv = |keys: &str, motion: Motion, select: bool| {
        KeyBinding::new(keys, FieldMove { motion, select }, Some(ctx))
    };
    vec![
        mv("left", Left, false),
        mv("shift-left", Left, true),
        mv("right", Right, false),
        mv("shift-right", Right, true),
        mv("ctrl-left", WordLeft, false),
        mv("ctrl-shift-left", WordLeft, true),
        mv("ctrl-right", WordRight, false),
        mv("ctrl-shift-right", WordRight, true),
        mv("home", LineStart, false),
        mv("shift-home", LineStart, true),
        mv("end", LineEnd, false),
        mv("shift-end", LineEnd, true),
        mv("ctrl-home", DocStart, false),
        mv("ctrl-shift-home", DocStart, true),
        mv("ctrl-end", DocEnd, false),
        mv("ctrl-shift-end", DocEnd, true),
        KeyBinding::new("delete", FieldDelete, Some(ctx)),
        KeyBinding::new("ctrl-delete", FieldDeleteWord, Some(ctx)),
        KeyBinding::new("ctrl-a", FieldSelectAll, Some(ctx)),
        KeyBinding::new("ctrl-c", FieldCopy, Some(ctx)),
        KeyBinding::new("ctrl-x", FieldCut, Some(ctx)),
    ]
}

/// Extras only the multi-line composer wants: vertical caret rows and hard line
/// breaks. (Single-line fields use up/down for list-nav or nothing.)
pub(crate) fn composer_only_bindings(ctx: &'static str) -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("up", FieldMove { motion: Motion::Up, select: false }, Some(ctx)),
        KeyBinding::new("shift-up", FieldMove { motion: Motion::Up, select: true }, Some(ctx)),
        KeyBinding::new("down", FieldMove { motion: Motion::Down, select: false }, Some(ctx)),
        KeyBinding::new("shift-down", FieldMove { motion: Motion::Down, select: true }, Some(ctx)),
        KeyBinding::new("shift-enter", FieldNewline, Some(ctx)),
        KeyBinding::new("ctrl-enter", FieldNewline, Some(ctx)),
    ]
}

impl EntityInputHandler for TextField {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        // The OS read path (IME surrounding-text, accessibility/screen readers).
        // A masked field must hand back DOTS, never the secret: slice the dotted
        // DISPLAY string by its OWN utf16/byte offsets (`•` is 3 bytes / 1 utf16
        // unit, so real-content offsets don't carry over). Edits still target the
        // real content — only this read-out is masked.
        let source = if self.masked {
            self.display_content()
        } else {
            self.content.clone()
        };
        let start = utf16_to_byte(&source, range_utf16.start);
        let end = utf16_to_byte(&source, range_utf16.end);
        *adjusted = Some(byte_to_utf16(&source, start)..byte_to_utf16(&source, end));
        Some(source[start..end].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let r = self.sel_range();
        let reversed = self.anchor.is_some_and(|a| a > self.cursor);
        Some(UTF16Selection {
            range: byte_to_utf16(&self.content, r.start)..byte_to_utf16(&self.content, r.end),
            reversed,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked
            .as_ref()
            .map(|m| byte_to_utf16(&self.content, m.start)..byte_to_utf16(&self.content, m.end))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Range to replace: the OS-supplied one, else the IME preedit, else the
        // current selection (the empty caret when nothing is selected).
        let range = range_utf16
            .map(|r| utf16_to_byte(&self.content, r.start)..utf16_to_byte(&self.content, r.end))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.sel_range());
        self.replace(range, text);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        new_selected_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .map(|r| utf16_to_byte(&self.content, r.start)..utf16_to_byte(&self.content, r.end))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.sel_range());
        // Enforce the newline policy on the preedit too: `replace()`'s guard is
        // bypassed on this IME/dictation path, so without this a single-line
        // field could take a \n/\r via composition. Offsets are measured on the
        // SANITIZED text so the marked range and cursor never desync from what
        // was actually spliced in.
        let text = sanitize_for_field(text, self.multiline);
        // Splice the preedit in and keep it marked so the next compose step
        // replaces it cleanly (the byte range is exact).
        self.content.replace_range(range.clone(), &text);
        let marked = range.start..range.start + text.len();
        self.cursor = new_selected_utf16
            .map(|s| marked.start + utf16_to_byte(&text, s.end))
            .unwrap_or(marked.end);
        self.anchor = None;
        self.marked = Some(marked);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(bounds)
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        // gpui hands this a WINDOW-relative point (it does NOT subtract element
        // bounds, unlike the mouse path). Localize it the same way the mouse
        // handlers do — minus the painted text origin — or a field off the
        // window origin / a scrolled single-line field returns the wrong index.
        let cursor = self.byte_at_local(point - self.text_origin)?;
        Some(byte_to_utf16(&self.content, cursor))
    }
}

impl Render for TextField {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context(self.key_context)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::commit))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::backspace_word))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::delete_word))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::select_all))
            // Every caret move is one action; one handler dispatches them all.
            .on_action(cx.listener(|this, a: &FieldMove, _, cx| this.do_move(a.motion, a.select, cx)))
            .w_full()
            .min_h(px(22.))
            // A bare field brings no chrome of its own — its host draws the
            // one frame (the omnibar pill). Everything else keeps the classic
            // inset-box dress.
            .when(!self.bare, |d| {
                d.px(px(6.))
                    .py(px(2.))
                    .rounded(px(4.))
                    .bg(rgb(0xFFFFFF))
                    .border_1()
                    .border_color(rgb(RULE_COLOR))
            })
            .text_size(px(13.))
            .text_color(rgb(TEXT_COLOR))
            // Single-line fields clip and the element scrolls itself to keep the
            // caret in view; the multi-line composer wraps and grows instead, so
            // it must NOT clip.
            .when(!self.multiline, |d| d.overflow_hidden())
            .child(TextFieldElement { input: cx.entity() })
    }
}

impl Focusable for TextField {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// Paints the field text + caret + selection, registers the IME handler and the
/// full mouse model.
struct TextFieldElement {
    input: Entity<TextField>,
}

impl IntoElement for TextFieldElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl TextFieldElement {
    fn store_geometry(&self, geom: FieldGeometry, text_origin: Point<Pixels>, cx: &mut App) {
        // Stash the window-relative text origin alongside the geometry, so the OS
        // hit-test path can localize its window point exactly as the mouse does.
        // This runs INSIDE paint (under DrawGuard), so it must use the draw-safe
        // write — no Context, so no mid-frame notify (draw_guard module docs).
        self.input.update_in_draw(cx, |input| {
            input.geometry = Some(geom);
            input.text_origin = text_origin;
        });
    }

    /// The complete mouse model: click-to-place + drag-select, double-click
    /// word, triple-click line, word/line-snapped drag-extend, shift-click.
    /// `text_origin` is where glyph x=0 painted (the single-line scroll already
    /// folded in), so points map into field-local space.
    fn register_mouse(
        &self,
        bounds: Bounds<Pixels>,
        text_origin: Point<Pixels>,
        window: &mut Window,
    ) {
        let input = self.input.clone();
        window.on_mouse_event(move |ev: &MouseDownEvent, phase, window, cx| {
            if phase != gpui::DispatchPhase::Bubble
                || ev.button != MouseButton::Left
                || !bounds.contains(&ev.position)
            {
                return;
            }
            let local = ev.position - text_origin;
            input.update_checked(cx, |input, cx| {
                window.focus(&input.focus_handle, cx);
                input.begin_select(local, ev.click_count, ev.modifiers.shift, cx);
            });
        });
        // Move/up are NOT bounds-gated: a drag that leaves the field still
        // tracks. They act only while `is_selecting`, set between down and up.
        let input = self.input.clone();
        window.on_mouse_event(move |ev: &MouseMoveEvent, phase, _window, cx| {
            if phase != gpui::DispatchPhase::Bubble || !input.read(cx).is_selecting {
                return;
            }
            let local = ev.position - text_origin;
            input.update_checked(cx, |input, cx| input.drag_to(local, cx));
        });
        let input = self.input.clone();
        window.on_mouse_event(move |ev: &MouseUpEvent, phase, _window, cx| {
            if phase != gpui::DispatchPhase::Bubble || ev.button != MouseButton::Left {
                return;
            }
            if input.read(cx).is_selecting {
                input.update_checked(cx, |input, cx| input.end_select(cx));
            }
        });
    }
}

/// What `prepaint` shaped for `paint`: a single horizontally-scrolled line, or a
/// soft-wrapped block that paints across rows (the note composer).
// Same call as `FieldGeometry`: the large `Single` arm is the common case and
// this value is transient per-paint, so boxing buys nothing worth an alloc.
#[allow(clippy::large_enum_variant)]
enum ComposerLayout {
    Single(Option<gpui::ShapedLine>),
    /// One shaped `WrappedLine` per hard line ("\n"-delimited), top to bottom.
    Wrapped(Vec<WrappedLine>),
}

/// Paint the selection highlight across the wrapped rows it spans: one rect per
/// visual row, from the row's selection start-x to its end-x.
fn paint_wrapped_selection(
    window: &mut Window,
    line: &WrappedLine,
    origin: Point<Pixels>,
    line_height: Pixels,
    sel: &Range<usize>,
    color: gpui::Rgba,
) {
    let Some(start) = line.position_for_index(sel.start, line_height) else {
        return;
    };
    let Some(end) = line.position_for_index(sel.end, line_height) else {
        return;
    };
    let mut y = start.y;
    while y <= end.y {
        let x0 = if y == start.y { start.x } else { px(0.) };
        // Each row but the last extends to the field's right edge; the last
        // stops at the caret end.
        let x1 = if y == end.y { end.x } else { px(COMPOSER_INNER_W) };
        if x1 > x0 {
            window.paint_quad(fill(
                Bounds::new(origin + point(x0, y + px(1.)), size(x1 - x0, line_height - px(2.))),
                color,
            ));
        }
        y += line_height;
    }
}

/// Total wrapped height of the composer's text at a given width (one row per
/// painted row), at least one line tall so an empty field still has a caret.
fn composer_wrapped_height(
    window: &Window,
    content: &str,
    width: Pixels,
    line_height: Pixels,
) -> Pixels {
    if content.is_empty() {
        return line_height;
    }
    let style = window.text_style();
    let run = TextRun {
        len: content.len(),
        font: style.font(),
        color: style.color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    match window.text_system().shape_text(
        SharedString::from(content.to_owned()),
        style.font_size.to_pixels(window.rem_size()),
        &[run],
        Some(width),
        None,
    ) {
        Ok(lines) => lines
            .iter()
            .map(|l| l.size(line_height).height)
            .fold(px(0.), |a, h| a + h)
            .max(line_height),
        Err(_) => line_height,
    }
}

impl Element for TextFieldElement {
    type RequestLayoutState = ();
    type PrepaintState = ComposerLayout;

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
        if self.input.read(cx).multiline {
            // Grow with the wrapped text: measure its height at the field's
            // available width each layout pass.
            let input = self.input.clone();
            let line_height = window.line_height();
            let id = window.request_measured_layout(style, move |known, available, window, cx| {
                let content = input.read(cx).display_content();
                let width = known.width.unwrap_or(match available.width {
                    AvailableSpace::Definite(w) => w,
                    _ => px(COMPOSER_INNER_W),
                });
                let height = composer_wrapped_height(window, &content, width, line_height);
                size(width, height)
            });
            (id, ())
        } else {
            style.size.height = window.line_height().into();
            (window.request_layout(style, [], cx), ())
        }
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
        let input = self.input.read(cx);
        let multiline = input.multiline;
        let content = input.display_content();
        let style = window.text_style();
        let run = TextRun {
            len: content.len(),
            font: style.font(),
            color: style.color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let font_size = style.font_size.to_pixels(window.rem_size());
        if multiline {
            // Soft-wrap at the box width. `shape_text` returns ONE WrappedLine
            // per hard line ("\n"-delimited) — keep them ALL (the old code took
            // only the first, dropping every line after a Shift+Enter).
            let lines = window
                .text_system()
                .shape_text(
                    SharedString::from(content),
                    font_size,
                    &[run],
                    Some(bounds.size.width),
                    None,
                )
                .map(|lines| lines.into_iter().collect())
                .unwrap_or_default();
            ComposerLayout::Wrapped(lines)
        } else {
            ComposerLayout::Single(Some(window.text_system().shape_line(
                SharedString::from(content),
                font_size,
                &[run],
                None,
            )))
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        layout: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let _guard = DrawGuard::enter();
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        let line_height = window.line_height();
        let focused = focus_handle.is_focused(window);
        // Caret + selection are read from the field, mapped into the display
        // string (masked: dots ≠ real bytes).
        let (cursor_db, sel_db, has_sel, display) = {
            let input = self.input.read(cx);
            let r = input.sel_range();
            (
                input.cursor_display_byte(),
                input.real_to_display_byte(r.start)..input.real_to_display_byte(r.end),
                input.has_selection(),
                input.display_content(),
            )
        };
        let sel_color = rgba(FIELD_SELECTION_BG); // the active-card gold, translucent
        match layout {
            ComposerLayout::Single(line) => {
                let Some(line) = line.take() else { return };
                let caret_x = line.x_for_index(cursor_db);
                // Scroll so the caret stays in view: shift left when it runs past
                // the right edge, right when it backs past the left edge.
                let shift = (caret_x - bounds.size.width + px(2.)).max(px(0.));
                let origin = point(bounds.origin.x - shift, bounds.origin.y);
                if has_sel {
                    let x0 = line.x_for_index(sel_db.start);
                    let x1 = line.x_for_index(sel_db.end);
                    window.paint_quad(fill(
                        Bounds::new(origin + point(x0, px(1.)), size(x1 - x0, line_height - px(2.))),
                        sel_color,
                    ));
                }
                line.paint(origin, line_height, TextAlign::Left, None, window, cx)
                    .ok();
                if focused {
                    window.paint_quad(fill(
                        Bounds::new(
                            origin + point(caret_x, px(2.)),
                            size(px(1.5), line_height - px(4.)),
                        ),
                        rgb(TEXT_COLOR),
                    ));
                }
                self.store_geometry(FieldGeometry::Single(line), origin, cx);
                self.register_mouse(bounds, origin, window);
            }
            ComposerLayout::Wrapped(lines) => {
                let origin = bounds.origin;
                // Stack the hard lines top to bottom, each soft-wrapping on its
                // own. Build the row map as we paint so the caret, selection,
                // clicks, and vertical motion all address the full stack.
                let starts = hard_line_starts(&display);
                let mut rows: Vec<WrappedRow> = Vec::with_capacity(lines.len());
                let mut y = px(0.);
                for (i, line) in lines.iter().enumerate() {
                    let start = starts.get(i).copied().unwrap_or(0);
                    let row_len = line.len();
                    let h = line.size(line_height).height.max(line_height);
                    let row_origin = origin + point(px(0.), y);
                    if has_sel {
                        // The slice of the selection that falls in this hard line.
                        let s = sel_db.start.clamp(start, start + row_len);
                        let e = sel_db.end.clamp(start, start + row_len);
                        if e > s {
                            paint_wrapped_selection(
                                window,
                                line,
                                row_origin,
                                line_height,
                                &((s - start)..(e - start)),
                                sel_color,
                            );
                        }
                    }
                    line.paint(row_origin, line_height, TextAlign::Left, None, window, cx)
                        .ok();
                    rows.push(WrappedRow { layout: Arc::clone(line), start, top: y });
                    y += h;
                }
                if focused {
                    let caret = wrapped_position(&rows, cursor_db, line_height);
                    window.paint_quad(fill(
                        Bounds::new(
                            origin + caret + point(px(0.), px(2.)),
                            size(px(1.5), line_height - px(4.)),
                        ),
                        rgb(TEXT_COLOR),
                    ));
                }
                self.store_geometry(FieldGeometry::Wrapped(rows), origin, cx);
                self.register_mouse(bounds, origin, window);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grapheme_motion_never_splits_a_cluster() {
        // "a" + family emoji (ZWJ sequence) + "b": stepping right from 0 must
        // jump the whole cluster, not land inside it.
        let s = "a👨‍👩‍👧b";
        let after_a = next_grapheme(s, 0);
        assert_eq!(after_a, 1);
        let after_emoji = next_grapheme(s, after_a);
        // The whole ZWJ family is one cluster; the next boundary is right before
        // 'b', and `s.is_char_boundary` must hold there.
        assert!(s.is_char_boundary(after_emoji));
        assert_eq!(&s[after_emoji..], "b");
        // And back again.
        assert_eq!(prev_grapheme(s, after_emoji), after_a);
        assert_eq!(prev_grapheme(s, after_a), 0);
    }

    #[test]
    fn word_motion_matches_word_range() {
        let s = "hello world  foo";
        // From 0, word-right lands at the end of "hello".
        assert_eq!(next_word(s, 0), 5);
        // From end, word-left lands at the start of "foo".
        assert_eq!(prev_word(s, s.len()), 13);
        // Double-click inside "world" selects exactly "world".
        let w = word_range_at(s, 7);
        assert_eq!(&s[w], "world");
    }

    #[test]
    fn word_motion_skips_punctuation_runs() {
        // UAX#29 breaks "foo!bar" into foo / ! / bar (unlike "foo.bar", where the
        // full stop is a MidNumLet that joins the letters). Word-right from 0
        // stops after "foo"; word-left from past the "!" returns to its start.
        let s = "foo!bar baz";
        assert_eq!(next_word(s, 0), 3);
        assert_eq!(prev_word(s, 4), 0);
    }

    #[test]
    fn line_range_is_the_logical_line() {
        let s = "one\ntwo three\nfour";
        let r = line_range_at(s, 6); // inside "two three"
        assert_eq!(&s[r], "two three");
        // Word motion crosses the newline rather than stopping dead at it.
        assert_eq!(next_word(s, 3), 7); // from the \n after "one" → end of "two"
    }

    #[test]
    fn utf16_and_char_conversions_roundtrip() {
        let s = "a🜂b"; // 🜂 is 4 UTF-8 bytes, 2 UTF-16 units, 1 char
        assert_eq!(byte_to_utf16(s, 5), 3); // a(1) + 🜂(2) = 3 units before 'b'
        assert_eq!(utf16_to_byte(s, 3), 5);
        assert_eq!(byte_to_char_idx(s, 5), 2); // 'a','🜂' = 2 chars before byte 5
        assert_eq!(char_idx_to_byte(s, 2), 5);
    }

    #[test]
    fn hard_line_starts_align_with_split() {
        // One start per "\n"-segment (matching how shape_text splits), each at
        // the byte just after the preceding newline.
        assert_eq!(hard_line_starts(""), vec![0]);
        assert_eq!(hard_line_starts("abc"), vec![0]);
        assert_eq!(hard_line_starts("ab\ncd"), vec![0, 3]);
        // Trailing newline and an empty middle line each still get a start.
        assert_eq!(hard_line_starts("a\n\nb\n"), vec![0, 2, 3, 5]);
        // Count matches split('\n') exactly, so shaped lines align index-wise.
        for s in ["", "x", "a\nb", "a\n\nb\n", "\n\n"] {
            assert_eq!(hard_line_starts(s).len(), s.split('\n').count());
        }
    }

    #[test]
    fn word_motion_survives_long_blank_run() {
        // A run of blank lines must not blow the stack (iterative paragraph walk).
        let s = format!("word{}word", "\n".repeat(500));
        let from = s.len();
        let landed = prev_word(&s, from);
        assert_eq!(&s[landed..], "word");
    }

    #[test]
    fn single_line_field_flattens_newlines_multiline_keeps_them() {
        // The newline guard lives in `replace`, so whatever path a break arrives
        // by — paste, OS insertText/dictation, multi-line IME commit — it gets
        // sanitized. Single-line: \n / \r → space, a CRLF collapsing to one; the
        // caret then lands after the FLATTENED text (offset = range.start + len).
        for raw in ["a\nb", "a\r\nb", "a\rb"] {
            let flat = sanitize_for_field(raw, false);
            assert_eq!(flat, "a b");
            assert_eq!(flat.len(), 3); // == caret offset after inserting at 0
        }
        // The multiline composer preserves the break (CRLF/CR normalized to \n).
        assert_eq!(sanitize_for_field("a\nb", true), "a\nb");
        assert_eq!(sanitize_for_field("a\r\nb", true), "a\nb");
    }
}
