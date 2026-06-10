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

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, FocusHandle, Focusable, GlobalElementId, KeyBinding, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point,
    ScrollWheelEvent, SharedString, Style, TextAlign, TextRun, UTF16Selection, UnderlineStyle,
    Window, WrappedLine, actions, div, fill, point, prelude::*, px, relative, rgb, rgba, size,
};
use strop_core::Buffer;
use unicode_segmentation::UnicodeSegmentation;

pub const BG_COLOR: u32 = 0xFBFAF8;
pub const TEXT_COLOR: u32 = 0x1A1A18;
const SELECTION_COLOR: u32 = 0xB4D5FE88;

actions!(
    editor,
    [
        Backspace, Delete, DeleteWordLeft, DeleteWordRight, Left, Right, Up, Down, WordLeft,
        WordRight, ParagraphUp, ParagraphDown, SelectLeft, SelectRight, SelectUp, SelectDown,
        SelectWordLeft, SelectWordRight, SelectParagraphUp, SelectParagraphDown, SelectAll, Home,
        End, SelectToHome, SelectToEnd, DocStart, DocEnd, SelectToDocStart, SelectToDocEnd,
        PageUp, PageDown, SelectPageUp, SelectPageDown, Newline, Copy, Cut, Paste, Undo, Redo,
    ]
);

pub fn bind_keys(cx: &mut App) {
    let ctx = Some("Editor");
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
        KeyBinding::new("ctrl-z", Undo, ctx),
        KeyBinding::new("ctrl-shift-z", Redo, ctx),
        KeyBinding::new("ctrl-y", Redo, ctx),
    ]);
}

pub struct Editor {
    focus_handle: FocusHandle,
    buffer: Buffer,
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
    last_frame: Option<TextFrame>,
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

    /// X position of a local index within its visual line.
    fn x_for(&self, local: usize, line: usize) -> Pixels {
        let layout = &self.line.unwrapped_layout;
        layout.x_for_index(local) - layout.x_for_index(self.line_start(line))
    }

    fn position(&self, local: usize, affinity_down: bool) -> (usize, Pixels) {
        let line = self.line_of(local, affinity_down);
        (line, self.x_for(local, line))
    }

    /// Closest local index to `x` on visual line `line`, with the affinity
    /// that renders the cursor on that same line.
    fn index_at(&self, line: usize, x: Pixels, line_height: Pixels) -> (usize, bool) {
        let line = line.min(self.line_count() - 1);
        let y = line_height * (line as f32) + line_height / 2.;
        let ix = self
            .line
            .closest_index_for_position(point(x, y), line_height)
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
        Some(point(x, par.top + self.line_height * (line as f32)))
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
                let (ix, aff) = target.index_at(line, x, self.line_height);
                return (target.range.start + ix, aff);
            }
            if p.y < par.top + par.height {
                let line = ((p.y - par.top) / self.line_height) as usize;
                let (ix, aff) = par.index_at(line, x, self.line_height);
                return (par.range.start + ix, aff);
            }
        }
        (self.doc_len(), false)
    }
}

impl Editor {
    pub fn new(cx: &mut Context<Self>, text: &str) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            buffer: Buffer::new(text),
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
            last_frame: None,
        }
    }

    /// Start the cursor-blink heartbeat. GNOME-style: solid while typing,
    /// blinking when idle, solid again (and quiet — no repaints) after 10s.
    pub fn start_blink(&self, cx: &mut Context<Self>) {
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
        self.bump_activity();
        cx.notify();
    }

    /// Extend the selection's moving end to `offset`. Keeps `goal_x`.
    fn extend_cursor(&mut self, offset: usize, affinity_down: bool, cx: &mut Context<Self>) {
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
        self.bump_activity();
        self.publish_primary(cx);
        cx.notify();
    }

    /// Linux PRIMARY-selection contract: any selection (mouse or keyboard)
    /// is published; middle-click pastes it. No-op on other platforms.
    fn publish_primary(&self, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            let text = self.buffer.slice_bytes(self.selected_range.clone());
            cx.write_to_primary(ClipboardItem::new_string(text));
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
        let rope = self.buffer.rope();
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
        let line = self.buffer.slice_bytes(start..offset);
        line.grapheme_indices(true)
            .next_back()
            .map_or(start, |(ix, _)| start + ix)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        let len = self.buffer.len_bytes();
        if offset >= len {
            return len;
        }
        let (_, end) = self.paragraph_bounds(offset);
        if offset == end {
            return offset + 1; // step over the newline
        }
        let line = self.buffer.slice_bytes(offset..end);
        line.grapheme_indices(true)
            .nth(1)
            .map_or(end, |(ix, _)| offset + ix)
    }

    fn previous_word_boundary(&self, offset: usize) -> usize {
        if offset == 0 {
            return 0;
        }
        let (start, _) = self.paragraph_bounds(offset);
        if offset == start {
            // Continue the search from the end of the previous paragraph.
            return self.previous_word_boundary(offset - 1).max(0);
        }
        let line = self.buffer.slice_bytes(start..offset);
        line.split_word_bound_indices()
            .rev()
            .find(|(_, seg)| seg.chars().next().is_some_and(char::is_alphanumeric))
            .map_or(start, |(ix, _)| start + ix)
    }

    fn next_word_boundary(&self, offset: usize) -> usize {
        let len = self.buffer.len_bytes();
        if offset >= len {
            return len;
        }
        let (_, end) = self.paragraph_bounds(offset);
        if offset == end {
            return self.next_word_boundary(offset + 1).min(len);
        }
        let line = self.buffer.slice_bytes(offset..end);
        line.split_word_bound_indices()
            .find(|(_, seg)| seg.chars().next().is_some_and(char::is_alphanumeric))
            .map_or(end, |(ix, seg)| offset + ix + seg.len())
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
        let len = self.buffer.len_bytes();
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
        let line = self.buffer.slice_bytes(start..end);
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
                let (ix, aff) = par.index_at(l, x, frame.line_height);
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
        let y = frame.paragraphs[par_ix].top
            + frame.line_height * (line_ix as f32)
            + frame.line_height / 2.;
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
            let prev = self.previous_boundary(self.cursor_offset());
            self.select_to(prev, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let next = self.next_boundary(self.cursor_offset());
            self.select_to(next, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete_word_left(&mut self, _: &DeleteWordLeft, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let prev = self.previous_word_boundary(self.cursor_offset());
            self.select_to(prev, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete_word_right(
        &mut self,
        _: &DeleteWordRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_range.is_empty() {
            let next = self.next_word_boundary(self.cursor_offset());
            self.select_to(next, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
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
        self.vertical_by(-1, false, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical_by(1, false, cx);
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
        self.move_to(0, cx);
        self.select_to(self.buffer.len_bytes(), cx);
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
        self.move_to(self.buffer.len_bytes(), cx);
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
        self.select_to(self.buffer.len_bytes(), cx);
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
        self.replace_text_in_range(None, "\n", window, cx);
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            let text = self.buffer.slice_bytes(self.selected_range.clone());
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            let text = self.buffer.slice_bytes(self.selected_range.clone());
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            self.replace_text_in_range(None, "", window, cx);
        }
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text.replace("\r\n", "\n"), window, cx);
        }
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(cursor_char) = self.buffer.undo() {
            let cursor = self.buffer.char_to_byte(cursor_char);
            self.selected_range = cursor..cursor;
            self.selection_reversed = false;
            self.cursor_affinity_down = false;
            self.goal_x = None;
            self.marked_range = None;
            self.bump_activity();
            cx.notify();
        }
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(cursor_char) = self.buffer.redo() {
            let cursor = self.buffer.char_to_byte(cursor_char);
            self.selected_range = cursor..cursor;
            self.selection_reversed = false;
            self.cursor_affinity_down = false;
            self.goal_x = None;
            self.marked_range = None;
            self.bump_activity();
            cx.notify();
        }
    }

    // -- Scrolling ------------------------------------------------------------

    fn on_scroll_wheel(&mut self, ev: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(frame) = self.last_frame.as_ref() else {
            return;
        };
        let delta = ev.delta.pixel_delta(frame.line_height);
        let target = (self.scroll_top - delta.y).clamp(px(0.), frame.max_scroll());
        if target != self.scroll_top {
            self.scroll_top = target;
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
        frame.index_for_point(frame.doc_point(position))
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

    fn on_mouse_down(&mut self, ev: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.goal_x = None;
        self.is_selecting = true;
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
            }
            2 => {
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

    fn on_middle_click(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // freedesktop PRIMARY contract: middle button pastes the primary
        // selection (never the clipboard) at the click position.
        let Some(text) = cx.read_from_primary().and_then(|item| item.text()) else {
            return;
        };
        let (ix, _) = self.index_for_mouse(ev.position);
        self.selected_range = ix..ix;
        self.selection_reversed = false;
        self.replace_text_in_range(None, &text.replace("\r\n", "\n"), window, cx);
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
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

    /// Cursor geometry for the smoke harness: byte offset, paragraph index,
    /// wrapped-line index within the paragraph, and x position.
    pub fn debug_cursor(&self) -> String {
        let cursor = self.cursor_offset();
        let Some(frame) = self.last_frame.as_ref() else {
            return format!("off={cursor} (no frame)");
        };
        match frame.cursor_position(cursor, self.cursor_affinity_down) {
            Some((par, line, x)) => format!(
                "off={cursor} par={par} line={line} x={x:?} aff={} sel={:?} scroll={:?}",
                self.cursor_affinity_down as u8, self.selected_range, self.scroll_top
            ),
            None => format!("off={cursor} (no paragraph)"),
        }
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.buffer.byte_to_utf16(range.start)..self.buffer.byte_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.buffer.utf16_to_byte(range.start)..self.buffer.utf16_to_byte(range.end)
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
        Some(self.buffer.slice_bytes(range))
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
        let range = range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.buffer.edit_bytes_coalescing(range.clone(), new_text);
        let cursor = range.start + new_text.len();
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.cursor_affinity_down = false;
        self.goal_x = None;
        self.marked_range = None;
        self.bump_activity();
        cx.notify();
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

        self.buffer.edit_bytes(range.clone(), new_text);
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
        Some(self.buffer.byte_to_utf16(byte_ix))
    }
}

// -- Element ------------------------------------------------------------------

struct EditorElement {
    editor: Entity<Editor>,
}

struct PrepaintState {
    paragraphs: Vec<ParagraphLayout>,
    cursor: Option<PaintQuad>,
    line_height: Pixels,
    scroll_top: Pixels,
    content_height: Pixels,
}

impl IntoElement for EditorElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Split a paragraph into runs cut at selection/marked boundaries, so
/// selection paints via `WrappedLine::paint_background` and IME composition
/// gets an underline.
fn runs_for_paragraph(
    par_range: &Range<usize>,
    selection: &Range<usize>,
    marked: Option<&Range<usize>>,
    base: &TextRun,
) -> Vec<TextRun> {
    let mut cuts = vec![par_range.start, par_range.end];
    for r in [Some(selection), marked].into_iter().flatten() {
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
            TextRun {
                len: w[1] - w[0],
                font: base.font.clone(),
                color: base.color,
                background_color: in_selection.then(|| rgba(SELECTION_COLOR).into()),
                underline: in_marked.then(|| UnderlineStyle {
                    color: Some(base.color),
                    thickness: px(1.),
                    wavy: false,
                }),
                strikethrough: None,
            }
        })
        .collect()
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
        let editor = self.editor.read(cx);
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let paragraph_gap = line_height; // vertical rhythm: one full line
        let wrap_width = bounds.size.width;
        let viewport = bounds.size.height;

        let text = editor.buffer.text();
        let selection = editor.selected_range.clone();
        let marked = editor.marked_range.clone();
        let cursor_offset = editor.cursor_offset();
        let cursor_affinity = editor.cursor_affinity_down;
        let cursor_blink_visible = editor.cursor_visible;
        let mut scroll_top = editor.scroll_top;
        let autoscroll = editor.autoscroll_request;

        let base_run = TextRun {
            len: 0,
            font: style.font(),
            color: style.color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };

        let mut paragraphs = Vec::new();
        let mut top = px(0.);
        let mut offset = 0;
        for par_text in text.split('\n') {
            let range = offset..offset + par_text.len();
            let runs = runs_for_paragraph(&range, &selection, marked.as_ref(), &base_run);
            let line = window
                .text_system()
                .shape_text(
                    SharedString::from(par_text.to_owned()),
                    font_size,
                    &runs,
                    Some(wrap_width),
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
            let height = line.size(line_height).height;
            paragraphs.push(ParagraphLayout {
                line,
                range: range.clone(),
                boundaries,
                top,
                height,
            });
            top += height + paragraph_gap;
            offset = range.end + 1; // step over '\n'
        }

        // `top` has accumulated one trailing gap past the last paragraph.
        let content_height = top - paragraph_gap;
        let max_scroll = (content_height + line_height - viewport).max(px(0.));
        scroll_top = scroll_top.clamp(px(0.), max_scroll);

        // Cursor position in document space (needed for autoscroll + quad).
        let cursor_pos = paragraphs
            .iter()
            .find(|p| cursor_offset <= p.range.end)
            .map(|par| {
                let (line, x) = par.position(cursor_offset - par.range.start, cursor_affinity);
                point(x, par.top + line_height * (line as f32))
            });

        if autoscroll && let Some(pos) = cursor_pos {
            if pos.y < scroll_top {
                scroll_top = pos.y;
            } else if pos.y + line_height > scroll_top + viewport {
                scroll_top = pos.y + line_height - viewport;
            }
        }

        // Write the clamped/adjusted scroll back; no notify needed, we're
        // mid-frame and painting with this exact value.
        self.editor.update(cx, |editor, _| {
            editor.scroll_top = scroll_top;
            editor.autoscroll_request = false;
        });

        let cursor = cursor_pos.and_then(|pos| {
            let y = pos.y - scroll_top;
            if !cursor_blink_visible || y + line_height <= px(0.) || y >= viewport {
                return None;
            }
            Some(fill(
                Bounds::new(
                    bounds.origin + point(pos.x, y),
                    size(px(2.), line_height),
                ),
                rgb(TEXT_COLOR),
            ))
        });

        PrepaintState {
            paragraphs,
            cursor,
            line_height,
            scroll_top,
            content_height,
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
        let focus_handle = self.editor.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );

        let line_height = prepaint.line_height;
        let scroll_top = prepaint.scroll_top;
        let viewport = bounds.size.height;
        for par in &prepaint.paragraphs {
            let y = par.top - scroll_top;
            if y + par.height <= px(0.) || y >= viewport {
                continue; // outside the viewport
            }
            let origin = bounds.origin + point(px(0.), y);
            par.line
                .paint_background(origin, line_height, TextAlign::Left, None, window, cx)
                .expect("paint_background failed");
            par.line
                .paint(origin, line_height, TextAlign::Left, None, window, cx)
                .expect("paint failed");
        }

        if focus_handle.is_focused(window) {
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        }

        let paragraphs = std::mem::take(&mut prepaint.paragraphs);
        let content_height = prepaint.content_height;
        self.editor.update(cx, |editor, _| {
            editor.last_frame = Some(TextFrame {
                bounds,
                line_height,
                scroll_top,
                content_height,
                paragraphs,
            });
        });
    }
}

impl Render for Editor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(BG_COLOR))
            .flex()
            .justify_center()
            .overflow_hidden()
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
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                    .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_middle_click))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .on_mouse_move(cx.listener(Self::on_mouse_move))
                    .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
                    .w_full()
                    .max_w(px(660.))
                    .h_full()
                    .pt(px(84.))
                    .pb(px(28.))
                    .px(px(28.))
                    .font_family("Literata")
                    .text_size(px(20.))
                    .line_height(px(28.))
                    .text_color(rgb(TEXT_COLOR))
                    .child(EditorElement { editor: cx.entity() }),
            )
    }
}

impl Focusable for Editor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
