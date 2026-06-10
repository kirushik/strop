//! The prose canvas: a multi-paragraph editable text element built directly
//! on GPUI's IME-capable input plumbing (`EntityInputHandler`).
//!
//! v0 scope: plain text, cursor/selection/mouse, word movement, clipboard.
//! Not yet: scrolling, undo (waits for strop-core transactions), inline
//! styles, the typograph input engine, cursor blink.

use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, FocusHandle, Focusable, GlobalElementId, KeyBinding, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point,
    SharedString, Style, TextAlign, TextRun, UTF16Selection, UnderlineStyle, Window, WrappedLine,
    actions, div, fill, point, prelude::*, px, relative, rgb, rgba, size,
};
use strop_core::Buffer;
use unicode_segmentation::UnicodeSegmentation;

pub const BG_COLOR: u32 = 0xFBFAF8;
pub const TEXT_COLOR: u32 = 0x1A1A18;
const SELECTION_COLOR: u32 = 0xB4D5FE88;

actions!(
    editor,
    [
        Backspace, Delete, Left, Right, Up, Down, WordLeft, WordRight, SelectLeft, SelectRight,
        SelectUp, SelectDown, SelectAll, Home, End, Newline, Copy, Cut, Paste, Undo, Redo,
    ]
);

pub fn bind_keys(cx: &mut App) {
    let ctx = Some("Editor");
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("up", Up, ctx),
        KeyBinding::new("down", Down, ctx),
        KeyBinding::new("ctrl-left", WordLeft, ctx),
        KeyBinding::new("ctrl-right", WordRight, ctx),
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new("shift-up", SelectUp, ctx),
        KeyBinding::new("shift-down", SelectDown, ctx),
        KeyBinding::new("ctrl-a", SelectAll, ctx),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("enter", Newline, ctx),
        KeyBinding::new("shift-enter", Newline, ctx),
        KeyBinding::new("ctrl-c", Copy, ctx),
        KeyBinding::new("ctrl-x", Cut, ctx),
        KeyBinding::new("ctrl-v", Paste, ctx),
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
    marked_range: Option<Range<usize>>,
    is_selecting: bool,
    last_frame: Option<TextFrame>,
}

/// Geometry of the last painted frame, for mouse, IME, and vertical-motion
/// mapping. Rebuilt on every paint.
struct TextFrame {
    bounds: Bounds<Pixels>,
    line_height: Pixels,
    paragraphs: Vec<ParagraphLayout>,
}

struct ParagraphLayout {
    line: WrappedLine,
    /// Byte range in the document, excluding the trailing newline.
    range: Range<usize>,
    /// Y offset of the paragraph top, relative to `TextFrame::bounds` origin.
    top: Pixels,
    height: Pixels,
}

impl TextFrame {
    fn doc_len(&self) -> usize {
        self.paragraphs.last().map_or(0, |p| p.range.end)
    }

    fn paragraph_containing(&self, offset: usize) -> Option<&ParagraphLayout> {
        self.paragraphs.iter().find(|p| offset <= p.range.end)
    }

    /// Position of a byte offset, relative to `bounds` origin.
    fn position_of(&self, offset: usize) -> Option<Point<Pixels>> {
        let par = self.paragraph_containing(offset)?;
        let local = par
            .line
            .position_for_index(offset - par.range.start, self.line_height)?;
        Some(point(local.x, par.top + local.y))
    }

    /// Byte offset closest to a point given relative to `bounds` origin.
    /// Points in inter-paragraph gaps snap to the following paragraph.
    fn index_for_point(&self, p: Point<Pixels>) -> usize {
        if p.y < px(0.) {
            return 0;
        }
        for par in &self.paragraphs {
            if p.y < par.top + par.height {
                let local = point(
                    p.x.max(px(0.)),
                    (p.y - par.top).clamp(px(0.), par.height - px(1.)),
                );
                let ix = par
                    .line
                    .closest_index_for_position(local, self.line_height)
                    .unwrap_or_else(|ix| ix);
                return par.range.start + ix;
            }
        }
        self.doc_len()
    }
}

impl Editor {
    pub fn new(cx: &mut Context<Self>, text: &str) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            buffer: Buffer::new(text),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            is_selecting: false,
            last_frame: None,
        }
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify();
    }

    // -- Boundary helpers (byte offsets) ------------------------------------

    fn line_bounds(&self, offset: usize) -> (usize, usize) {
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

    fn previous_boundary(&self, offset: usize) -> usize {
        if offset == 0 {
            return 0;
        }
        let (start, _) = self.line_bounds(offset);
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
        let (_, end) = self.line_bounds(offset);
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
        let (start, _) = self.line_bounds(offset);
        if offset == start {
            return offset - 1;
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
        let (_, end) = self.line_bounds(offset);
        if offset == end {
            return offset + 1;
        }
        let line = self.buffer.slice_bytes(offset..end);
        line.split_word_bound_indices()
            .find(|(ix, seg)| {
                ix + seg.len() > 0 && seg.chars().next().is_some_and(char::is_alphanumeric)
            })
            .map_or(end, |(ix, seg)| offset + ix + seg.len())
    }

    fn vertical_index(&self, direction: f32) -> Option<usize> {
        let frame = self.last_frame.as_ref()?;
        let pos = frame.position_of(self.cursor_offset())?;
        // Land mid-line above/below; index_for_point handles doc edges.
        // TODO: persist goal-x across consecutive vertical moves.
        let target = point(pos.x, pos.y + direction * frame.line_height + frame.line_height / 2.);
        Some(frame.index_for_point(target))
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
        if let Some(ix) = self.vertical_index(-1.) {
            self.move_to(ix, cx);
        }
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self.vertical_index(1.) {
            self.move_to(ix, cx);
        }
    }

    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.previous_word_boundary(self.cursor_offset()), cx);
    }

    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.next_word_boundary(self.cursor_offset()), cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self.vertical_index(-1.) {
            self.select_to(ix, cx);
        }
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self.vertical_index(1.) {
            self.select_to(ix, cx);
        }
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.buffer.len_bytes(), cx);
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        let (start, _) = self.line_bounds(self.cursor_offset());
        self.move_to(start, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let (_, end) = self.line_bounds(self.cursor_offset());
        self.move_to(end, cx);
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
            self.marked_range = None;
            cx.notify();
        }
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(cursor_char) = self.buffer.redo() {
            let cursor = self.buffer.char_to_byte(cursor_char);
            self.selected_range = cursor..cursor;
            self.selection_reversed = false;
            self.marked_range = None;
            cx.notify();
        }
    }

    // -- Mouse ----------------------------------------------------------------

    fn index_for_mouse(&self, position: Point<Pixels>) -> usize {
        let Some(frame) = self.last_frame.as_ref() else {
            return 0;
        };
        frame.index_for_point(position - frame.bounds.origin)
    }

    fn on_mouse_down(&mut self, ev: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.is_selecting = true;
        let ix = self.index_for_mouse(ev.position);
        if ev.modifiers.shift {
            self.select_to(ix, cx);
        } else {
            self.move_to(ix, cx);
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, ev: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            let ix = self.index_for_mouse(ev.position);
            self.select_to(ix, cx);
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
        self.marked_range = None;
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
        let pos = frame.position_of(range.start)?;
        Some(Bounds::new(
            frame.bounds.origin + pos,
            size(px(2.), frame.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let byte_ix = self.index_for_mouse(point);
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
}

impl IntoElement for EditorElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Split `(start, end)` into runs cut at selection/marked boundaries, so
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

        let text = editor.buffer.text();
        let selection = editor.selected_range.clone();
        let marked = editor.marked_range.clone();
        let cursor_offset = editor.cursor_offset();

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
            let height = line.size(line_height).height;
            paragraphs.push(ParagraphLayout {
                line,
                range: range.clone(),
                top,
                height,
            });
            top += height + paragraph_gap;
            offset = range.end + 1; // step over '\n'
        }

        let cursor = paragraphs
            .iter()
            .find(|p| cursor_offset <= p.range.end)
            .and_then(|par| {
                let local = par
                    .line
                    .position_for_index(cursor_offset - par.range.start, line_height)?;
                Some(fill(
                    Bounds::new(
                        bounds.origin + point(local.x, par.top + local.y),
                        size(px(2.), line_height),
                    ),
                    rgb(TEXT_COLOR),
                ))
            });

        PrepaintState {
            paragraphs,
            cursor,
            line_height,
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
        for par in &prepaint.paragraphs {
            let origin = bounds.origin + point(px(0.), par.top);
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
        self.editor.update(cx, |editor, _| {
            editor.last_frame = Some(TextFrame {
                bounds,
                line_height,
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
                    .on_action(cx.listener(Self::left))
                    .on_action(cx.listener(Self::right))
                    .on_action(cx.listener(Self::up))
                    .on_action(cx.listener(Self::down))
                    .on_action(cx.listener(Self::word_left))
                    .on_action(cx.listener(Self::word_right))
                    .on_action(cx.listener(Self::select_left))
                    .on_action(cx.listener(Self::select_right))
                    .on_action(cx.listener(Self::select_up))
                    .on_action(cx.listener(Self::select_down))
                    .on_action(cx.listener(Self::select_all))
                    .on_action(cx.listener(Self::home))
                    .on_action(cx.listener(Self::end))
                    .on_action(cx.listener(Self::newline))
                    .on_action(cx.listener(Self::copy))
                    .on_action(cx.listener(Self::cut))
                    .on_action(cx.listener(Self::paste))
                    .on_action(cx.listener(Self::undo))
                    .on_action(cx.listener(Self::redo))
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .on_mouse_move(cx.listener(Self::on_mouse_move))
                    .w_full()
                    .max_w(px(660.))
                    .h_full()
                    .pt(px(84.))
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
