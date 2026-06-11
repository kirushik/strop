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
    Entity, EntityInputHandler, FocusHandle, Focusable, FontStyle, FontWeight, GlobalElementId,
    KeyBinding, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad,
    Pixels, Point, ScrollWheelEvent, SharedString, StrikethroughStyle, Style, TextAlign, TextRun,
    UTF16Selection, UnderlineStyle, Window, WrappedLine, actions, div, fill, point, prelude::*,
    px, relative, rgb, rgba, size,
};
use strop_core::document::{BlockKind, BlockMap, Document, InlineAttr, SpanSet};
use strop_core::{Store, typograph};
use unicode_segmentation::UnicodeSegmentation;

pub const BG_COLOR: u32 = 0xFBFAF8;
pub const TEXT_COLOR: u32 = 0x1A1A18;
const SELECTION_COLOR: u32 = 0xB4D5FE88;
const HIGHLIGHT_COLOR: u32 = 0xF9E29CAA;
const CODE_BG_COLOR: u32 = 0x1A1A1814;
const LINK_COLOR: u32 = 0x1A56A0;
const CODE_FONT: &str = "PT Mono";
const BAR_HEIGHT: f32 = 36.;
const MUTED_COLOR: u32 = 0x8A8678;
const RULE_COLOR: u32 = 0xE8E4DC;

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
        KeyBinding::new("ctrl-b", ToggleStrong, ctx),
        KeyBinding::new("ctrl-i", ToggleEmphasis, ctx),
        KeyBinding::new("ctrl-u", ToggleUnderline, ctx),
        KeyBinding::new("ctrl-shift-x", ToggleStrikethrough, ctx),
        KeyBinding::new("ctrl-shift-h", ToggleHighlight, ctx),
        KeyBinding::new("ctrl-e", ToggleCode, ctx),
        KeyBinding::new("ctrl-alt-1", Heading1, ctx),
        KeyBinding::new("ctrl-alt-2", Heading2, ctx),
        KeyBinding::new("ctrl-alt-3", Heading3, ctx),
        KeyBinding::new("ctrl-alt-q", ToggleQuoteBlock, ctx),
        KeyBinding::new("ctrl-alt-c", ToggleCodeBlock, ctx),
        // Google Docs list conventions.
        KeyBinding::new("ctrl-shift-8", ToggleBulletList, ctx),
        KeyBinding::new("ctrl-shift-7", ToggleOrderedList, ctx),
        KeyBinding::new("ctrl-alt-s", AddCheckpoint, ctx),
        KeyBinding::new("ctrl-shift-e", ExportMarkdown, ctx),
        KeyBinding::new("ctrl-alt-f", InsertFootnote, ctx),
    ]);
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
    /// Rough rewind panel (B5); proper history visualization is backlogged.
    show_history: bool,
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
    /// Per-block metrics (headings, code, quotes differ from body).
    line_height: Pixels,
    indent: Pixels,
    /// Kind-derived decorations, resolved at prepaint.
    bg: Option<gpui::Rgba>,
    quote_rule: bool,
    marker: Option<SharedString>,
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

impl Editor {
    pub fn new(cx: &mut Context<Self>, text: &str, spans: SpanSet, blocks: BlockMap) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
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
            show_history: false,
            last_frame: None,
        }
    }

    /// Attach the durable store and start the idle-save heartbeat: edits
    /// mirror into Loro immediately, the snapshot hits disk once typing
    /// pauses for a second (and on quit, via `save_now`).
    pub fn attach_store(&mut self, store: Store, cx: &mut Context<Self>) {
        self.store = Some(store);
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(1000))
                    .await;
                let alive = this.update(cx, |editor: &mut Editor, _| {
                    if editor.store_dirty && editor.last_input.elapsed() >= Duration::from_secs(1)
                    {
                        editor.save_now();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// Fan buffer changes out to every offset-tracking consumer (formatting
    /// spans, durable store). Must run after every mutation.
    fn sync_mutations(&mut self) {
        let ops = self.doc.take_ops();
        if ops.is_empty() {
            return;
        }
        if let Some(store) = &self.store {
            store.apply(&ops);
            self.store_dirty = true;
        }
    }

    /// Restore persisted cross-session undo/redo.
    pub fn restore_history(&mut self, history: strop_core::document::History) {
        self.doc.import_history(history);
    }

    /// Record a named version snapshot in the document file.
    fn add_checkpoint(&mut self, _: &AddCheckpoint, _: &mut Window, cx: &mut Context<Self>) {
        self.sync_mutations();
        if let Some(store) = &self.store {
            let name = format!("Checkpoint {}", store.checkpoints().len() + 1);
            store.add_checkpoint(&name);
            self.store_dirty = true;
            eprintln!("strop: recorded \"{name}\"");
        }
        cx.notify();
    }

    fn restore_checkpoint(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(store) = &self.store else { return };
        let checkpoints = store.checkpoints();
        let Some(cp) = checkpoints.get(ix) else { return };
        let Some((text, spans, blocks)) = store.state_at(&cp.frontiers) else {
            eprintln!("strop: cannot read checkpoint state");
            return;
        };
        self.doc.restore_state(&text, spans, blocks);
        self.selected_range = 0..0;
        self.selection_reversed = false;
        self.cursor_affinity_down = false;
        self.goal_x = None;
        self.marked_range = None;
        self.caret_attrs.clear();
        self.sync_mutations();
        self.bump_activity();
        cx.notify();
    }

    /// Export next to the .strop file (doc.strop -> doc.md).
    fn export_markdown(&mut self, _: &ExportMarkdown, _: &mut Window, cx: &mut Context<Self>) {
        let Some(store) = &self.store else {
            eprintln!("strop: no document file to export next to");
            return;
        };
        let md = strop_core::markdown::to_markdown(
            &self.doc.text(),
            self.doc.spans(),
            self.doc.blocks(),
        );
        let path = store.path().with_extension("md");
        match std::fs::write(&path, md) {
            Ok(()) => eprintln!("strop: exported {}", path.display()),
            Err(e) => eprintln!("strop: export failed: {e}"),
        }
        cx.notify();
    }

    /// Insert a footnote: a ref atom at the cursor, a def block at the end,
    /// cursor lands in the def. (Two transactions; two undos remove it.)
    fn insert_footnote(&mut self, _: &InsertFootnote, _: &mut Window, cx: &mut Context<Self>) {
        let n = self
            .doc
            .blocks()
            .kinds()
            .iter()
            .filter(|k| matches!(k, BlockKind::FootnoteDef { .. }))
            .count()
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
        self.store_dirty = true;
        self.bump_activity();
        cx.notify();
    }

    /// Footnotes whose refs are visible in the viewport: (id, def text,
    /// def byte offset). Derived from the last painted frame.
    fn visible_footnotes(&self) -> Vec<(String, String, usize)> {
        let Some(frame) = self.last_frame.as_ref() else {
            return Vec::new();
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
            return Vec::new();
        }
        let rope = self.doc.rope();
        let (clo, chi) = (rope.byte_to_char(lo), rope.byte_to_char(hi));
        let mut out: Vec<(String, String, usize)> = Vec::new();
        for span in self.doc.spans().spans() {
            let InlineAttr::FootnoteRef(id) = &span.attr else {
                continue;
            };
            if span.range.start >= chi || span.range.end <= clo {
                continue;
            }
            if out.iter().any(|(seen, _, _)| seen == id) {
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
            let mut def = self.doc.slice_bytes(start..end);
            if def.chars().count() > 110 {
                def = def.chars().take(110).collect::<String>() + "…";
            }
            out.push((id.clone(), def, start));
        }
        out
    }

    pub fn save_now(&mut self) {
        self.sync_mutations();
        if let Some(store) = &self.store {
            match store.save_with_state(
                self.doc.spans(),
                self.doc.blocks(),
                &self.doc.export_history(200),
            ) {
                Ok(()) => self.store_dirty = false,
                Err(e) => eprintln!("strop: failed to save {}: {e}", store.path().display()),
            }
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
        self.store_dirty = true;
        cx.notify();
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
        self.caret_attrs.clear();
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
        self.caret_attrs.clear();
        self.bump_activity();
        self.publish_primary(cx);
        cx.notify();
    }

    /// Linux PRIMARY-selection contract: any selection (mouse or keyboard)
    /// is published; middle-click pastes it. No-op on other platforms.
    fn publish_primary(&self, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            let text = self.doc.slice_bytes(self.selected_range.clone());
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
        if offset == 0 {
            return 0;
        }
        let (start, _) = self.paragraph_bounds(offset);
        if offset == start {
            // Continue the search from the end of the previous paragraph.
            return self.previous_word_boundary(offset - 1).max(0);
        }
        let line = self.doc.slice_bytes(start..offset);
        line.split_word_bound_indices()
            .rev()
            .find(|(_, seg)| seg.chars().next().is_some_and(char::is_alphanumeric))
            .map_or(start, |(ix, _)| start + ix)
    }

    fn next_word_boundary(&self, offset: usize) -> usize {
        let len = self.doc.len_bytes();
        if offset >= len {
            return len;
        }
        let (_, end) = self.paragraph_bounds(offset);
        if offset == end {
            return self.next_word_boundary(offset + 1).min(len);
        }
        let line = self.doc.slice_bytes(offset..end);
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
            // At the start of a styled block, the first backspace strips
            // the block kind instead of merging (Notion/Docs convention).
            let cursor = self.cursor_offset();
            let (par_start, _) = self.paragraph_bounds(cursor);
            if cursor == par_start {
                let block = self.doc.block_of_byte(cursor);
                if *self.doc.blocks().kind(block) != BlockKind::Paragraph {
                    self.doc.set_block_kind(block, BlockKind::Paragraph);
                    self.store_dirty = true;
                    self.bump_activity();
                    cx.notify();
                    return;
                }
            }
            let prev = self.previous_boundary(cursor);
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
        self.select_to(self.doc.len_bytes(), cx);
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
        self.store_dirty = true;
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
        if !self.selected_range.is_empty() {
            let text = self.doc.slice_bytes(self.selected_range.clone());
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            let text = self.doc.slice_bytes(self.selected_range.clone());
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            self.replace_text_in_range(None, "", window, cx);
        }
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            // Pasted text is never typographed — it is already authored.
            self.apply_replace(None, &text.replace("\r\n", "\n"), false, cx);
        }
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
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
        let (ix, aff) = frame.index_for_point(frame.doc_point(position));
        // Stale-frame guard: never hand out offsets beyond the live rope.
        (ix.min(self.doc.len_bytes()), aff)
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

    fn on_middle_click(&mut self, ev: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        // freedesktop PRIMARY contract: middle button pastes the primary
        // selection (never the clipboard) at the click position.
        let Some(text) = cx.read_from_primary().and_then(|item| item.text()) else {
            return;
        };
        let (ix, _) = self.index_for_mouse(ev.position);
        self.selected_range = ix..ix;
        self.selection_reversed = false;
        self.apply_replace(None, &text.replace("\r\n", "\n"), false, cx);
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
        let tail_start = self.doc.rope().byte_to_char(cursor).saturating_sub(12);
        let tail: String = self
            .doc
            .rope()
            .chars_at(tail_start)
            .take(self.doc.rope().byte_to_char(cursor) - tail_start)
            .collect();
        let doc_state = format!(
            "off={cursor} sel={:?} tail={tail:?} kind={:?} spans={:?}",
            self.selected_range,
            self.doc.blocks().kind(self.doc.block_of_byte(cursor)),
            self.doc.spans().spans()
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
        let range = range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

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
            let lang = typograph::detect_lang(self.doc.rope().chars());
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

/// Per-kind block metrics and decorations, per the PT-pairings research:
/// same-family Literata SemiBold headings (display optical for H1), all
/// boxes on the 28px rhythm.
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

fn block_style(kind: &BlockKind) -> BlockStyle {
    let semibold = Some(FontWeight::SEMIBOLD);
    match kind {
        BlockKind::Heading(1) => BlockStyle {
            size: px(32.),
            line_height: px(42.),
            extra_top: px(14.),
            family: Some("Literata 36pt"),
            weight: semibold,
            ..Default::default()
        },
        BlockKind::Heading(2) => BlockStyle {
            size: px(24.),
            weight: semibold,
            ..Default::default()
        },
        BlockKind::Heading(_) => BlockStyle {
            weight: semibold,
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
        BlockKind::Divider | BlockKind::FootnoteDef { .. } => BlockStyle {
            muted: true,
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
fn runs_for_paragraph(
    par_range: &Range<usize>,
    selection: &Range<usize>,
    marked: Option<&Range<usize>>,
    spans: &[(Range<usize>, InlineAttr)],
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
                    InlineAttr::FootnoteRef(_) => {}
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
        let line_height = window.line_height();
        let paragraph_gap = line_height; // vertical rhythm: one full line
        let wrap_width = bounds.size.width;
        let viewport = bounds.size.height;

        let text = editor.doc.text();
        let selection = editor.selected_range.clone();
        let marked = editor.marked_range.clone();
        let cursor_offset = editor.cursor_offset();
        let cursor_affinity = editor.cursor_affinity_down;
        let cursor_blink_visible = editor.cursor_visible;
        let mut scroll_top = editor.scroll_top;
        let autoscroll = editor.autoscroll_request;
        // Formatting spans, converted to byte ranges for this frame.
        let spans_bytes: Vec<(Range<usize>, InlineAttr)> = {
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

        let kinds: Vec<BlockKind> = editor.doc.blocks().kinds().to_vec();

        let mut paragraphs = Vec::new();
        let mut top = px(0.);
        let mut offset = 0;
        let mut ordered_no = 0usize;
        for (block_ix, par_text) in text.split('\n').enumerate() {
            let kind = kinds.get(block_ix).cloned().unwrap_or_default();
            let bstyle = block_style(&kind);
            let marker = match &kind {
                BlockKind::ListItem { ordered: false, .. } => {
                    ordered_no = 0;
                    Some(SharedString::from("•"))
                }
                BlockKind::ListItem { ordered: true, .. } => {
                    ordered_no += 1;
                    Some(SharedString::from(format!("{ordered_no}.")))
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
            let runs =
                runs_for_paragraph(&range, &selection, marked.as_ref(), &par_spans, &block_base);
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
            top += bstyle.extra_top;
            let height = line.size(bstyle.line_height).height;
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
                marker,
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
        self.editor.update(cx, |editor, _| {
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
            let origin = bounds.origin + point(par.indent, y);
            if let Some(marker) = &par.marker {
                let run = TextRun {
                    len: marker.len(),
                    font: gpui::font("Literata"),
                    color: rgb(MUTED_COLOR).into(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let shaped =
                    window
                        .text_system()
                        .shape_line(marker.clone(), px(16.), &[run], None);
                shaped
                    .paint(
                        bounds.origin + point(par.indent - px(24.), y + px(2.)),
                        par.line_height,
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

impl Editor {
    fn format_button(
        &self,
        label: &'static str,
        attr: InlineAttr,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.attr_active(&attr);
        div()
            .id(label)
            .px(px(8.))
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
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |editor, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    editor.toggle_span(attr.clone(), cx);
                }),
            )
            .child(label)
    }

    fn window_button(
        &self,
        label: &'static str,
        action: fn(&mut Window, &mut App),
    ) -> impl IntoElement {
        div()
            .id(label)
            .w(px(34.))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .text_color(rgb(MUTED_COLOR))
            .hover(|d| d.bg(rgba(0x1A1A180Au32)).text_color(rgb(TEXT_COLOR)))
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                cx.stop_propagation();
                action(window, cx);
            })
            .child(label)
    }

    /// The one piece of chrome: a unified bar — drag region, formatting
    /// toggles with live state, window controls. Deliberately quiet.
    fn render_titlebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let drag =
            |_: &MouseDownEvent, window: &mut Window, _: &mut App| window.start_window_move();
        div()
            .h(px(BAR_HEIGHT))
            .w_full()
            .flex()
            .items_center()
            .border_b_1()
            .border_color(rgb(RULE_COLOR))
            .font_family("Literata")
            .text_size(px(13.))
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .pl(px(14.))
                    .text_color(rgb(MUTED_COLOR))
                    .on_mouse_down(MouseButton::Left, drag)
                    .child("Strop"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.))
                    .child(self.format_button("B", InlineAttr::Strong, cx))
                    .child(self.format_button("I", InlineAttr::Emphasis, cx))
                    .child(self.format_button("U", InlineAttr::Underline, cx))
                    .child(self.format_button("S", InlineAttr::Strikethrough, cx))
                    .child(self.format_button("H", InlineAttr::Highlight, cx))
                    .child(self.format_button("{}", InlineAttr::Code, cx)),
            )
            .child(
                div()
                    .id("history-toggle")
                    .px(px(8.))
                    .py(px(2.))
                    .ml(px(8.))
                    .rounded(px(5.))
                    .cursor(CursorStyle::PointingHand)
                    .text_color(if self.show_history {
                        rgb(TEXT_COLOR)
                    } else {
                        rgb(MUTED_COLOR)
                    })
                    .when(self.show_history, |d| d.bg(rgba(0x1A1A1812u32)))
                    .hover(|d| d.bg(rgba(0x1A1A180Au32)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            editor.show_history = !editor.show_history;
                            cx.notify();
                        }),
                    )
                    .child("↺"),
            )
            .child(
                div()
                    .w(px(28.))
                    .h_full()
                    .on_mouse_down(MouseButton::Left, drag),
            )
            .child(self.window_button("–", |window, _| window.minimize_window()))
            .child(self.window_button("□", |window, _| window.zoom_window()))
            .child(self.window_button("✕", |_, cx| cx.quit()))
    }
}


/// Days-from-epoch to civil date (Howard Hinnant's algorithm); good enough
/// for checkpoint labels (UTC — rough UI, backlogged with the rest).
fn format_unix(secs: i64) -> String {
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
    fn render_history_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let checkpoints = self
            .store
            .as_ref()
            .map(|s| s.checkpoints())
            .unwrap_or_default();
        div()
            .id("history-panel")
            .absolute()
            .top(px(BAR_HEIGHT + 8.))
            .right(px(8.))
            .w(px(280.))
            .max_h(px(440.))
            .overflow_y_scroll()
            .bg(rgb(0xF4F1EA))
            .border_1()
            .border_color(rgb(RULE_COLOR))
            .rounded(px(8.))
            .p(px(6.))
            .flex()
            .flex_col()
            .gap(px(2.))
            .font_family("Literata")
            .text_size(px(13.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .px(px(8.))
                    .py(px(4.))
                    .text_color(rgb(MUTED_COLOR))
                    .child(if checkpoints.is_empty() {
                        "No checkpoints yet — ctrl-alt-s records one."
                    } else {
                        "Click a version to restore it (undoable)."
                    }),
            )
            .children(checkpoints.into_iter().enumerate().rev().map(|(ix, cp)| {
                div()
                    .id(ix)
                    .px(px(8.))
                    .py(px(5.))
                    .rounded(px(5.))
                    .cursor(CursorStyle::PointingHand)
                    .hover(|d| d.bg(rgba(0x1A1A180Au32)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |editor, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            editor.restore_checkpoint(ix, cx);
                        }),
                    )
                    .child(div().text_color(rgb(TEXT_COLOR)).child(cp.name.clone()))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(MUTED_COLOR))
                            .child(format_unix(cp.created_unix)),
                    )
            }))
    }
}

impl Editor {
    /// The viewport footnote zone (document-model §4c): definitions whose
    /// refs are on screen, pinned to the window bottom as an overlay inset.
    /// Read-only projection; click jumps to the def block.
    fn render_footnote_zone(
        &self,
        footnotes: Vec<(String, String, usize)>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
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
                    .font_family("Literata")
                    .text_size(px(14.))
                    .text_color(rgb(MUTED_COLOR))
                    .children(footnotes.into_iter().enumerate().map(
                        |(ix, (id, def, target))| {
                            div()
                                .id(ix)
                                .px(px(4.))
                                .rounded(px(4.))
                                .cursor(CursorStyle::PointingHand)
                                .hover(|d| d.bg(rgba(0x1A1A180Au32)))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |editor, _: &MouseDownEvent, _, cx| {
                                        cx.stop_propagation();
                                        editor.goal_x = None;
                                        editor.set_cursor(target, false, cx);
                                    }),
                                )
                                .child(format!("{id}. {def}"))
                        },
                    )),
            )
    }
}

impl Render for Editor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .relative()
            .bg(rgb(BG_COLOR))
            .flex()
            .flex_col()
            .child(self.render_titlebar(cx))
            .when(self.show_history, |d| {
                d.child(self.render_history_panel(cx))
            })
            .map(|d| {
                let footnotes = self.visible_footnotes();
                d.when(!footnotes.is_empty(), |d| {
                    d.child(self.render_footnote_zone(footnotes, cx))
                })
            })
            .child(
                div()
                    .w_full()
                    .flex_1()
                    .min_h(px(0.))
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
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                    .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_middle_click))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .on_mouse_move(cx.listener(Self::on_mouse_move))
                    .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
                            .w_full()
                            .max_w(px(660.))
                            .h_full()
                            .pt(px(56.))
                            .pb(px(28.))
                            .px(px(28.))
                            .font_family("Literata")
                            .text_size(px(20.))
                            .line_height(px(28.))
                            .text_color(rgb(TEXT_COLOR))
                            .child(EditorElement { editor: cx.entity() }),
                    ),
            )
    }
}

impl Focusable for Editor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> TextRun {
        TextRun {
            len: 0,
            font: gpui::font("Literata"),
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
        let runs = runs_for_paragraph(&par, &(0..0), None, &spans, &base());
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
        let runs = runs_for_paragraph(&par, &(2..4), None, &spans, &base());
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
        let plain = runs_for_paragraph(&par, &(2..4), None, &[], &base());
        assert_eq!(
            plain[1].background_color,
            Some(rgba(SELECTION_COLOR).into())
        );
    }

    #[test]
    fn code_run_switches_family_and_marked_text_underlines() {
        let par = 0..8;
        let spans = vec![(0..4, InlineAttr::Code)];
        let runs = runs_for_paragraph(&par, &(0..0), Some(&(4..8)), &spans, &base());
        assert_eq!(runs[0].font.family.as_ref(), CODE_FONT);
        assert!(runs[1].underline.is_some());
    }
}
