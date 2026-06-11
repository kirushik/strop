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

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{
    App, Bounds, ClipboardEntry, ClipboardItem, Context, Corners, CursorStyle, Element, ElementId,
    ElementInputHandler, ExternalPaths, RenderImage,
    Entity, EntityInputHandler, FocusHandle, Focusable, FontStyle, FontWeight, GlobalElementId,
    KeyBinding, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad,
    Pixels, Point, ScrollWheelEvent, SharedString, StrikethroughStyle, Style, TextAlign, TextRun,
    UTF16Selection, UnderlineStyle, Window, WrappedLine, actions, div, fill, point, prelude::*,
    px, relative, rgb, rgba, size,
};
use strop_core::document::{
    Annotations, BlockKind, BlockMap, Document, InlineAttr, NoteKind, NoteStatus, SpanSet,
};
use strop_core::{Store, typograph};

use crate::config::{Config, Language};
use unicode_segmentation::UnicodeSegmentation;

pub const BG_COLOR: u32 = 0xFBFAF8;
pub const TEXT_COLOR: u32 = 0x1A1A18;
const SELECTION_COLOR: u32 = 0xB4D5FE88;
const HIGHLIGHT_COLOR: u32 = 0xF9E29CAA;
const CODE_BG_COLOR: u32 = 0x1A1A1814;
const LINK_COLOR: u32 = 0x1A56A0;
const NOTE_TINT: u32 = 0xE3B84926; // wheat/amber ~15% — Docs-trained intuition
const NOTE_TINT_ACTIVE: u32 = 0xE3B8494D; // ~30% when active
const MARGIN_WIDTH: f32 = 248.;
const MARGIN_GAP: f32 = 16.;
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
        OpenFile, SaveCopyAs, AddNote, RunDiagnosis, RunBelieving, Find, Replace, EscapeMode,
        ToggleHistory, TogglePalette, PaletteUp, PaletteDown, NewDocument, RenameDocument,
        RevealInFiles, CopyDocumentPath,
    ]
);

pub fn bind_keys(cx: &mut App) {
    let ctx = Some("Editor");
    // Commands (anything a menu would list) bind from the registry, so the
    // palette and the keymap can never disagree about a chord.
    let editor_ctx: std::rc::Rc<gpui::KeyBindingContextPredicate> =
        gpui::KeyBindingContextPredicate::parse("Editor").unwrap().into();
    cx.bind_keys(crate::commands::all().iter().filter_map(|cmd| {
        let keys = cmd.keys?;
        Some(
            KeyBinding::load(
                keys,
                (cmd.make)(),
                Some(editor_ctx.clone()),
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
        KeyBinding::new("escape", EscapeMode, ctx),
        // GNOME's menu key opens the palette — it IS the menu.
        KeyBinding::new("f10", TogglePalette, ctx),
        KeyBinding::new("enter", NoteCommit, Some("NoteInput")),
        KeyBinding::new("escape", NoteCancel, Some("NoteInput")),
        KeyBinding::new("backspace", NoteBackspace, Some("NoteInput")),
        KeyBinding::new("tab", NoteTab, Some("NoteInput")),
        // The palette's query field: same editing actions, plus row motion.
        KeyBinding::new("enter", NoteCommit, Some("PaletteInput")),
        KeyBinding::new("escape", NoteCancel, Some("PaletteInput")),
        KeyBinding::new("backspace", NoteBackspace, Some("PaletteInput")),
        KeyBinding::new("up", PaletteUp, Some("PaletteInput")),
        KeyBinding::new("down", PaletteDown, Some("PaletteInput")),
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
    /// History mode (rewind v2): list + read-only diff preview.
    history_view: Option<HistoryView>,
    history_preview: Option<PreviewDoc>,
    /// Edits since the last checkpoint — idle-gap session sealing.
    dirty_since_checkpoint: bool,
    /// Encoded image assets by id; Arc<gpui::Image> handles feed GPUI's
    /// decode-once cache via use_render_image.
    image_assets: HashMap<String, Arc<gpui::Image>>,
    /// User settings (config.toml), loaded at startup.
    pub config: Config,
    /// Active (snapped/highlighted) margin note, if any.
    active_note: Option<u64>,
    diagnosis_running: bool,
    last_ai_error: Option<String>,
    /// Find bar (ctrl-f): live-highlighting query input; Enter advances.
    find_input: Option<Entity<NoteInput>>,
    /// The command palette (PLAN.md E1): the menu, summoned not mounted.
    palette_input: Option<Entity<NoteInput>>,
    palette_selected: usize,
    /// In-titlebar document rename (PLAN.md E2).
    doc_rename_input: Option<Entity<NoteInput>>,
    find_current: usize,
    /// Replace field (ctrl-h adds it beside find): Enter on it replaces
    /// the current match; the All button replaces every match (one undo).
    replace_input: Option<Entity<NoteInput>>,
    /// Rename-in-place for a history row: (entry index, composer).
    rename_input: Option<(usize, Entity<NoteInput>)>,
    /// Alt-text composer for an image block: (block index, composer).
    alt_input: Option<(usize, Entity<NoteInput>)>,
    /// Self-baseline from the [voice] corpus (None until >=3 docs load).
    pub voice_baseline: Option<strop_core::voice::Baseline>,
    /// In-card composer for the active note's body.
    note_input: Option<Entity<NoteInput>>,
    last_frame: Option<TextFrame>,
}

#[derive(Clone, Copy, PartialEq)]
enum SelectGranularity {
    Char,
    Word,
    Paragraph,
}

/// Minimal single-line composer for note bodies: typing + backspace + IME;
/// Enter commits, Escape cancels. Deliberately tiny — the main editor's
/// machinery stays the only real text surface.
pub struct NoteInput {
    focus_handle: FocusHandle,
    content: String,
    marked: Option<Range<usize>>,
    /// Keymap context: "NoteInput" everywhere except the palette, whose
    /// query field needs its own bindings (up/down move the selection).
    key_context: &'static str,
}

pub enum NoteInputEvent {
    Commit(String),
    Cancel,
}

impl gpui::EventEmitter<NoteInputEvent> for NoteInput {}

impl NoteInput {
    fn new(cx: &mut Context<Self>, content: String) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content,
            marked: None,
            key_context: "NoteInput",
        }
    }

    fn for_palette(cx: &mut Context<Self>) -> Self {
        Self {
            key_context: "PaletteInput",
            ..Self::new(cx, String::new())
        }
    }

    fn commit(&mut self, _: &NoteCommit, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(NoteInputEvent::Commit(self.content.clone()));
    }

    fn cancel(&mut self, _: &NoteCancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(NoteInputEvent::Cancel);
    }

    fn backspace(&mut self, _: &NoteBackspace, _: &mut Window, cx: &mut Context<Self>) {
        self.content.pop();
        cx.notify();
    }
}

actions!(note_input, [NoteCommit, NoteCancel, NoteBackspace, NoteTab]);

impl EntityInputHandler for NoteInput {
    fn text_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        Some(self.content.clone())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let end = self.content.chars().map(|c| c.len_utf16()).sum();
        Some(UTF16Selection {
            range: end..end,
            reversed: false,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.clone()
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked = None;
    }

    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(m) = self.marked.take() {
            let byte_start = self.content.len().saturating_sub(m.end - m.start);
            self.content.truncate(byte_start);
        }
        self.content.push_str(text);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(m) = self.marked.take() {
            let byte_start = self.content.len().saturating_sub(m.end - m.start);
            self.content.truncate(byte_start);
        }
        self.marked = Some(0..text.len());
        self.content.push_str(text);
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
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

impl Render for NoteInput {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context(self.key_context)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::commit))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::backspace))
            .w_full()
            .min_h(px(22.))
            .px(px(6.))
            .py(px(2.))
            .rounded(px(4.))
            .bg(rgb(0xFFFFFF))
            .border_1()
            .border_color(rgb(RULE_COLOR))
            .text_size(px(13.))
            .text_color(rgb(TEXT_COLOR))
            .child(NoteInputElement { input: cx.entity() })
    }
}

impl Focusable for NoteInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// Paints the composer text and registers the IME handler.
struct NoteInputElement {
    input: Entity<NoteInput>,
}

impl IntoElement for NoteInputElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for NoteInputElement {
    type RequestLayoutState = ();
    type PrepaintState = Option<gpui::ShapedLine>;

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
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let content = self.input.read(cx).content.clone();
        let style = window.text_style();
        let run = TextRun {
            len: content.len(),
            font: style.font(),
            color: style.color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        Some(window.text_system().shape_line(
            SharedString::from(content),
            style.font_size.to_pixels(window.rem_size()),
            &[run],
            None,
        ))
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        line: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        if let Some(line) = line.take() {
            let cursor_x = line.width;
            line.paint(bounds.origin, window.line_height(), window, cx)
                .ok();
            if focus_handle.is_focused(window) {
                window.paint_quad(fill(
                    Bounds::new(
                        bounds.origin + point(cursor_x, px(2.)),
                        size(px(1.5), window.line_height() - px(4.)),
                    ),
                    rgb(TEXT_COLOR),
                ));
            }
        }
    }
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
    /// Decoded image for Image blocks, with its display size.
    image: Option<(Arc<RenderImage>, gpui::Size<Pixels>)>,
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
            history_view: None,
            history_preview: None,
            dirty_since_checkpoint: false,
            image_assets: HashMap::new(),
            config: Config::default(),
            active_note: None,
            diagnosis_running: false,
            last_ai_error: None,
            find_input: None,
            palette_input: None,
            palette_selected: 0,
            doc_rename_input: None,
            find_current: 0,
            replace_input: None,
            rename_input: None,
            alt_input: None,
            voice_baseline: None,
            note_input: None,
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
                    // Idle gap seals a writing session — checkpoints are
                    // navigation markers for "a sitting", not safety.
                    if editor.dirty_since_checkpoint
                        && editor.last_input.elapsed() >= Duration::from_secs(900)
                    {
                        if let Some(store) = &editor.store {
                            store.add_checkpoint_if_changed("Session", false);
                            editor.dirty_since_checkpoint = false;
                        }
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
            self.dirty_since_checkpoint = true;
        }
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
                format!(
                    "{}/{rest}",
                    std::env::var("HOME").unwrap_or_default()
                )
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
                if let Some(text) = text {
                    if text.split_whitespace().count() >= 200 {
                        texts.push(text);
                    }
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
        self.doc.set_notes(annotations);
    }

    /// Record a named version snapshot in the document file.
    fn add_checkpoint(&mut self, _: &AddCheckpoint, _: &mut Window, cx: &mut Context<Self>) {
        self.sync_mutations();
        if let Some(store) = &self.store {
            let name = format!("Checkpoint {}", store.checkpoints().len() + 1);
            store.add_checkpoint(&name, true);
            self.dirty_since_checkpoint = false;
            self.store_dirty = true;
            eprintln!("strop: recorded \"{name}\"");
        }
        cx.notify();
    }

    /// Build the rewind list: materialize every checkpoint once, compute
    /// word deltas between consecutive states.
    fn enter_history(&mut self, cx: &mut Context<Self>) {
        let Some(store) = &self.store else {
            return;
        };
        let mut entries: Vec<HistoryEntry> = Vec::new();
        let mut prev_text = String::new();
        for cp in store.checkpoints() {
            let Some((text, spans, blocks)) = store.state_at(&cp.frontiers) else {
                continue;
            };
            let delta = strop_core::diff::word_delta(&strop_core::diff::prose_diff(
                &prev_text, &text,
            ));
            prev_text = text.clone();
            entries.push(HistoryEntry {
                name: cp.name.clone(),
                created_unix: cp.created_unix,
                manual: cp.manual,
                frontiers: cp.frontiers.clone(),
                text,
                spans,
                blocks,
                delta,
            });
        }
        if entries.is_empty() {
            return;
        }
        let selected = entries.len() - 1;
        self.history_view = Some(HistoryView {
            entries,
            selected,
            named_only: false,
            compare_current: false,
        });
        self.rebuild_preview();
        cx.notify();
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

    fn edit_image_alt(&mut self, block: usize, window: &mut Window, cx: &mut Context<Self>) {
        let BlockKind::Image { src, alt, caption } = self.doc.blocks().kind(block).clone()
        else {
            return;
        };
        let input = cx.new(|cx| NoteInput::new(cx, alt));
        cx.subscribe_in(
            &input,
            window,
            move |editor, _, event: &NoteInputEvent, window, cx| {
                if let NoteInputEvent::Commit(new_alt) = event {
                    editor.doc.set_block_kind(
                        block,
                        BlockKind::Image {
                            src: src.clone(),
                            alt: new_alt.clone(),
                            caption: caption.clone(),
                        },
                    );
                    editor.store_dirty = true;
                }
                editor.alt_input = None;
                window.focus(&editor.focus_handle);
                cx.notify();
            },
        )
        .detach();
        window.focus(&input.read(cx).focus_handle);
        self.alt_input = Some((block, input));
        cx.notify();
    }

    fn start_rename(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(hv) = &self.history_view else { return };
        let seed = hv.entries[ix].name.clone();
        let input = cx.new(|cx| NoteInput::new(cx, seed));
        cx.subscribe_in(
            &input,
            window,
            move |editor, _, event: &NoteInputEvent, window, cx| {
                if let NoteInputEvent::Commit(name) = event {
                    if !name.trim().is_empty() {
                        if let Some(store) = &editor.store {
                            store.rename_checkpoint(ix, name.trim());
                            editor.store_dirty = true;
                        }
                        if let Some(hv) = &mut editor.history_view {
                            if let Some(e) = hv.entries.get_mut(ix) {
                                e.name = name.trim().to_owned();
                                e.manual = true;
                            }
                        }
                    }
                }
                editor.rename_input = None;
                window.focus(&editor.focus_handle);
                cx.notify();
            },
        )
        .detach();
        window.focus(&input.read(cx).focus_handle);
        self.rename_input = Some((ix, input));
        cx.notify();
    }

    fn history_select(&mut self, ix: usize, cx: &mut Context<Self>) {
        if let Some(hv) = &mut self.history_view {
            hv.selected = ix.min(hv.entries.len() - 1);
            self.rebuild_preview();
            self.scroll_top = px(0.);
            cx.notify();
        }
    }

    /// Restore the selected checkpoint: auto-checkpoint the present first
    /// (the rail narrates what happened), restore as an undoable forward
    /// edit, exit history.
    fn restore_selected(&mut self, cx: &mut Context<Self>) {
        let Some(hv) = &self.history_view else { return };
        let entry = &hv.entries[hv.selected];
        let (name, frontiers) = (entry.name.clone(), entry.frontiers.clone());
        let Some(store) = &self.store else { return };
        store.add_checkpoint(&format!("Before restoring “{name}”"), false);
        let Some((text, spans, blocks)) = store.state_at(&frontiers) else {
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
        self.exit_history(cx);
        self.sync_mutations();
        self.store_dirty = true;
        self.bump_activity();
        cx.notify();
    }

    /// Export next to the .strop file (doc.strop -> doc.md).
    fn export_markdown(&mut self, _: &ExportMarkdown, _: &mut Window, cx: &mut Context<Self>) {
        let Some(store) = &self.store else {
            eprintln!("strop: no document file to export next to");
            return;
        };
        let mut md = strop_core::markdown::to_markdown(
            &self.doc.text(),
            self.doc.spans(),
            self.doc.blocks(),
        );
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
                        if let Some(store) = &editor.store {
                            if let Err(e) = store.save_copy_to(&path) {
                                eprintln!("strop: save copy: {e}");
                            }
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
        self.store_dirty = true;
        self.open_composer(id, String::new(), window, cx);
        self.bump_activity();
        cx.notify();
    }

    fn open_composer(
        &mut self,
        id: u64,
        body: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_note = Some(id);
        let input = cx.new(|cx| NoteInput::new(cx, body));
        cx.subscribe_in(
            &input,
            window,
            move |editor, _, event: &NoteInputEvent, window, cx| {
                match event {
                    NoteInputEvent::Commit(body) => {
                        editor.doc.set_note_body(id, body.clone());
                        editor.store_dirty = true;
                    }
                    NoteInputEvent::Cancel => {}
                }
                editor.note_input = None;
                // Focus returns to the text — the composer's handle is gone.
                window.focus(&editor.focus_handle);
                cx.notify();
            },
        )
        .detach();
        window.focus(&input.read(cx).focus_handle);
        self.note_input = Some(input);
    }

    fn set_note_status(&mut self, id: u64, status: NoteStatus, cx: &mut Context<Self>) {
        self.doc.set_note_status(id, status);
        if self.active_note == Some(id) {
            self.active_note = None;
            self.note_input = None;
        }
        self.store_dirty = true;
        self.bump_activity();
        cx.notify();
    }

    /// The thesis, running: an editorial pass that names problems as
    /// queries in the margin and never rewrites a word.
    fn run_diagnosis(&mut self, _: &RunDiagnosis, _: &mut Window, cx: &mut Context<Self>) {
        self.run_pass(false, cx);
    }

    fn run_believing(&mut self, _: &RunBelieving, _: &mut Window, cx: &mut Context<Self>) {
        self.run_pass(true, cx);
    }

    fn run_pass(&mut self, believing: bool, cx: &mut Context<Self>) {
        if self.diagnosis_running {
            return;
        }
        let ai = self.config.ai.clone();
        if ai.base_url.is_empty() || ai.model.is_empty() {
            self.last_ai_error =
                Some("configure [ai] base_url/api_key/model in config.toml".into());
            cx.notify();
            return;
        }
        // Scope: the selection if there is one, else the whole document
        // (capped — a 24k-char window is plenty for an editorial pass).
        let scope = if self.selected_range.is_empty() {
            self.doc.text()
        } else {
            self.doc.slice_bytes(self.selected_range.clone())
        };
        let scope: String = scope.chars().take(24_000).collect();
        let mode = "line".to_owned(); // levels-of-edit switch: config later
        self.diagnosis_running = true;
        self.last_ai_error = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let client =
                        strop_core::llm::LlmClient::new(&ai.base_url, &ai.api_key, &ai.model);
                    let system = if believing {
                        strop_core::diagnose::believing_system_prompt()
                    } else {
                        strop_core::diagnose::system_prompt(&mode)
                    };
                    let user = strop_core::diagnose::user_prompt(&scope);
                    client
                        .chat(&system, &user, 2048)
                        .map_err(|e| e.to_string())
                        .and_then(|response| strop_core::diagnose::parse(&response))
                })
                .await;
            this.update(cx, |editor: &mut Editor, cx| {
                editor.diagnosis_running = false;
                match result {
                    Ok(diagnoses) => {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0);
                        let count = diagnoses.len();
                        // Anchor against the text as it is NOW — quotes
                        // that no longer match are dropped.
                        let annotations = strop_core::diagnose::to_annotations(
                            &editor.doc.text(),
                            diagnoses,
                            editor.doc.notes(),
                            now,
                        );
                        let kept = annotations.len();
                        editor.doc.add_diagnoses(annotations);
                        editor.store_dirty = true;
                        eprintln!("strop: diagnosis pass: {kept} of {count} anchored");
                    }
                    Err(e) => {
                        editor.last_ai_error = Some(e);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn find(&mut self, _: &Find, window: &mut Window, cx: &mut Context<Self>) {
        let seed = if self.selected_range.is_empty() {
            String::new()
        } else {
            self.doc.slice_bytes(self.selected_range.clone())
        };
        let input = cx.new(|cx| NoteInput::new(cx, seed));
        cx.observe(&input, |_, _, cx| cx.notify()).detach();
        cx.subscribe_in(
            &input,
            window,
            |editor, input, event: &NoteInputEvent, window, cx| match event {
                NoteInputEvent::Commit(_) => {
                    // Enter: jump to the next match, keep the bar open.
                    let query = input.read(cx).content.clone();
                    let matches = editor.find_matches(&query);
                    if matches.is_empty() {
                        return;
                    }
                    editor.find_current = (editor.find_current + 1) % matches.len();
                    let m = matches[editor.find_current].clone();
                    editor.selected_range = m;
                    editor.selection_reversed = false;
                    editor.bump_activity();
                    cx.notify();
                    let _ = window; // focus stays on the bar
                }
                NoteInputEvent::Cancel => {
                    editor.find_input = None;
                    editor.replace_input = None;
                    window.focus(&editor.focus_handle);
                    cx.notify();
                }
            },
        )
        .detach();
        window.focus(&input.read(cx).focus_handle);
        self.find_input = Some(input);
        self.find_current = 0;
        cx.notify();
    }

    /// Case-insensitive (first-lowercase-char folding — exact for RU/EN,
    /// approximate for ß-class expansions) match positions in byte ranges.
    /// Tab in the find/replace bar hops between the two fields (the
    /// action bubbles up from the NoteInput context to the editor).
    fn note_tab(&mut self, _: &NoteTab, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(find), Some(rep)) = (self.find_input.clone(), self.replace_input.clone())
        else {
            return;
        };
        if find.read(cx).focus_handle.is_focused(window) {
            window.focus(&rep.read(cx).focus_handle);
        } else {
            window.focus(&find.read(cx).focus_handle);
        }
        cx.notify();
    }

    /// ctrl-shift-p / F10 / the titlebar menu button: the command palette.
    /// Every command the app has, searchable, with its chord on the row —
    /// this is the menu bar of a chrome-minimal editor (PLAN.md E1).
    fn toggle_palette(&mut self, _: &TogglePalette, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette_input.is_some() {
            self.close_palette(window, cx);
            return;
        }
        let input = cx.new(NoteInput::for_palette);
        cx.observe(&input, |editor, _, cx| {
            editor.palette_selected = 0; // query changed: selection restarts
            cx.notify();
        })
        .detach();
        cx.subscribe_in(
            &input,
            window,
            |editor, input, event: &NoteInputEvent, window, cx| match event {
                NoteInputEvent::Commit(_) => {
                    let query = input.read(cx).content.clone();
                    editor.execute_palette_entry(&query, editor.palette_selected, window, cx);
                }
                NoteInputEvent::Cancel => editor.close_palette(window, cx),
            },
        )
        .detach();
        window.focus(&input.read(cx).focus_handle);
        self.palette_input = Some(input);
        self.palette_selected = 0;
        cx.notify();
    }

    fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette_input = None;
        window.focus(&self.focus_handle);
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
        let input = cx.new(|cx| NoteInput::new(cx, stem));
        cx.observe(&input, |_, _, cx| cx.notify()).detach();
        cx.subscribe_in(
            &input,
            window,
            |editor, _, event: &NoteInputEvent, window, cx| match event {
                NoteInputEvent::Commit(title) => editor.finish_rename(title.clone(), window, cx),
                NoteInputEvent::Cancel => {
                    editor.doc_rename_input = None;
                    window.focus(&editor.focus_handle);
                    cx.notify();
                }
            },
        )
        .detach();
        window.focus(&input.read(cx).focus_handle);
        self.doc_rename_input = Some(input);
        cx.notify();
    }

    fn finish_rename(&mut self, title: String, window: &mut Window, cx: &mut Context<Self>) {
        self.doc_rename_input = None;
        window.focus(&self.focus_handle);
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
    /// palette is both the menu and the door to the other essays.
    fn palette_rows(&self, query: &str) -> Vec<PaletteRow> {
        let mut rows: Vec<PaletteRow> = crate::commands::ranked(query)
            .into_iter()
            .map(PaletteRow::Cmd)
            .collect();
        let current = self.store.as_ref().map(|s| s.path().to_owned());
        for p in crate::files::recents() {
            if Some(&p) == current.as_ref() {
                continue;
            }
            let name = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if query.trim().is_empty()
                || crate::commands::score_text(query.trim(), name).is_some()
            {
                rows.push(PaletteRow::Recent(p));
            }
        }
        rows
    }

    fn execute_palette_entry(
        &mut self,
        query: &str,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let rows = self.palette_rows(query);
        let Some(row) = rows.get(ix) else {
            return;
        };
        match row {
            PaletteRow::Cmd(cmd) => {
                let action = (cmd.make)();
                // Close first: focus returns to the document, so the action
                // lands exactly as if its chord had been pressed there.
                self.close_palette(window, cx);
                window.dispatch_action(action, cx);
            }
            PaletteRow::Recent(path) => {
                let path = path.clone();
                self.close_palette(window, cx);
                crate::files::open_in_new_window(&path);
            }
        }
    }

    fn palette_up(&mut self, _: &PaletteUp, _: &mut Window, cx: &mut Context<Self>) {
        self.palette_selected = self.palette_selected.saturating_sub(1);
        cx.notify();
    }

    fn palette_down(&mut self, _: &PaletteDown, _: &mut Window, cx: &mut Context<Self>) {
        let len = self
            .palette_input
            .as_ref()
            .map_or(0, |i| self.palette_rows(&i.read(cx).content).len());
        if len > 0 {
            self.palette_selected = (self.palette_selected + 1).min(len - 1);
        }
        cx.notify();
    }

    fn render_palette(&self, cx: &Context<Self>) -> impl IntoElement {
        let input = self.palette_input.clone().expect("palette open");
        let query = input.read(cx).content.clone();
        let rows = self.palette_rows(&query);
        let selected = self.palette_selected.min(rows.len().saturating_sub(1));
        let grouped = query.trim().is_empty();
        let home = std::env::var("HOME").unwrap_or_default();
        let mut list = div()
            .id("palette-list")
            .max_h(px(420.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .pb(px(6.));
        let mut last_section = "";
        for (ix, row) in rows.iter().enumerate() {
            let section = match row {
                PaletteRow::Cmd(cmd) => cmd.section,
                PaletteRow::Recent(_) => "Recent Documents",
            };
            if grouped && section != last_section {
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
            let (label, right): (String, Option<String>) = match row {
                PaletteRow::Cmd(cmd) => {
                    (cmd.label.to_owned(), cmd.keys.map(|k| k.to_owned()))
                }
                PaletteRow::Recent(p) => {
                    let stem = p
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("?")
                        .to_owned();
                    let dir = p
                        .parent()
                        .map(|d| d.display().to_string().replace(&home, "~"))
                        .unwrap_or_default();
                    (stem, Some(dir))
                }
            };
            list = list.child(
                div()
                    .id(("palette-row", ix))
                    .px(px(12.))
                    .py(px(4.))
                    .flex()
                    .justify_between()
                    .items_center()
                    .gap(px(12.))
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
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(rgb(TEXT_COLOR))
                            .child(label),
                    )
                    .when_some(right, |d, right| {
                        d.child(
                            div()
                                .text_size(px(11.))
                                .text_color(rgb(MUTED_COLOR))
                                .min_w(px(0.))
                                .truncate()
                                .child(right),
                        )
                    }),
            );
        }
        if rows.is_empty() {
            list = list.child(
                div()
                    .px(px(12.))
                    .py(px(8.))
                    .text_size(px(13.))
                    .text_color(rgb(MUTED_COLOR))
                    .child("No matching command"),
            );
        }
        div()
            .absolute()
            .top(px(BAR_HEIGHT + 6.))
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .child(
                div()
                    .w(px(460.))
                    .bg(rgb(0xFCFAF4))
                    .border_1()
                    .border_color(rgb(RULE_COLOR))
                    .rounded(px(8.))
                    .shadow_lg()
                    .font_family("PT Serif")
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .p(px(8.))
                            .border_b_1()
                            .border_color(rgb(RULE_COLOR))
                            .child(input.clone()),
                    )
                    .child(list),
            )
    }

    /// ctrl-h: ensure the find bar exists, add the replace field.
    fn replace(&mut self, _: &Replace, window: &mut Window, cx: &mut Context<Self>) {
        if self.find_input.is_none() {
            self.find(&Find, window, cx);
        }
        if self.replace_input.is_some() {
            return;
        }
        let input = cx.new(|cx| NoteInput::new(cx, String::new()));
        cx.observe(&input, |_, _, cx| cx.notify()).detach();
        cx.subscribe_in(
            &input,
            window,
            |editor, _, event: &NoteInputEvent, window, cx| match event {
                NoteInputEvent::Commit(replacement) => {
                    editor.replace_current(replacement.clone(), cx);
                }
                NoteInputEvent::Cancel => {
                    editor.find_input = None;
                    editor.replace_input = None;
                    window.focus(&editor.focus_handle);
                    cx.notify();
                }
            },
        )
        .detach();
        self.replace_input = Some(input);
        cx.notify();
    }

    /// Replace the current match and advance to the next.
    fn replace_current(&mut self, replacement: String, cx: &mut Context<Self>) {
        let Some(find) = self.find_input.clone() else {
            return;
        };
        let query = find.read(cx).content.clone();
        let matches = self.find_matches(&query);
        if matches.is_empty() {
            return;
        }
        let ix = self.find_current % matches.len();
        let target = matches[ix].clone();
        self.doc.edit_bytes(target.clone(), &replacement);
        let cursor = target.start + replacement.len();
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.sync_mutations();
        self.store_dirty = true;
        // Land on what is now the match at the same index.
        let matches = self.find_matches(&query);
        if !matches.is_empty() {
            self.find_current = ix % matches.len();
            self.selected_range = matches[self.find_current].clone();
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
        let (Some(find), Some(rep)) = (self.find_input.clone(), self.replace_input.clone())
        else {
            return;
        };
        let query = find.read(cx).content.clone();
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
        self.store_dirty = true;
        self.find_current = 0;
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

    fn render_find_strip(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let input = self.find_input.clone()?;
        let query = input.read(cx).content.clone();
        let count = self.find_matches(&query).len();
        let label = if query.is_empty() {
            String::new()
        } else if count == 0 {
            "нет совпадений".into()
        } else {
            format!("{}/{count}", (self.find_current % count.max(1)) + 1)
        };
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
                .child(div().text_color(rgb(MUTED_COLOR)).child("Find:"))
                .child(div().flex_1().child(input))
                .child(div().text_color(rgb(MUTED_COLOR)).child(label))
                .when_some(self.replace_input.clone(), |d, rep| {
                    d.child(div().text_color(rgb(MUTED_COLOR)).child("›"))
                        .child(div().flex_1().child(rep))
                        .child(
                            div()
                                .id("replace-all")
                                .px(px(8.))
                                .py(px(1.))
                                .rounded(px(4.))
                                .cursor(CursorStyle::PointingHand)
                                .bg(rgb(0xE8DFC8))
                                .hover(|d| d.bg(rgb(0xDFD3B0)))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                                        cx.stop_propagation();
                                        editor.replace_all(cx);
                                    }),
                                )
                                .child("All"),
                        )
                }),
        )
    }

    pub fn save_now(&mut self) {
        self.sync_mutations();
        if let Some(store) = &self.store {
            match store.save_with_state(
                self.doc.spans(),
                self.doc.blocks(),
                &self.doc.export_history(200),
                self.doc.notes(),
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
    /// is published; middle-click pastes it. With auto_copy_selection
    /// (config), the regular clipboard gets it too — Kirill's habit.
    fn publish_primary(&self, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            let text = self.doc.slice_bytes(self.selected_range.clone());
            if self.config.auto_copy_selection {
                cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
            }
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

    fn escape_mode(&mut self, _: &EscapeMode, _: &mut Window, cx: &mut Context<Self>) {
        if self.history_view.is_some() {
            self.exit_history(cx);
        }
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
        let Some(item) = cx.read_from_clipboard() else {
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
        self.store_dirty = true;
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

    fn on_mouse_down(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
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
                // Bidirectional activation: clicking inside an anchor
                // activates its margin card.
                let c = self.doc.rope().byte_to_char(ix.min(self.doc.len_bytes()));
                self.active_note = self
                    .doc
                    .notes()
                    .open()
                    .find(|n| n.range.start <= c && c < n.range.end)
                    .map(|n| n.id);
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
        let doc_state = format!(
            "off={cursor} sel={:?} tail={tail:?} kind={:?} spans={:?}{hist}",
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
        if self.history_view.is_some() {
            return; // history preview is read-only
        }
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
                    InlineAttr::FootnoteRef(_) => {}
                }
            }

            // Note anchors tint (wheat); diagnosis anchors underline
            // quietly in muted ink — never red, never wavy — promoting to
            // a tint when active. Selection composites over everything.
            for (r, active, is_diagnosis) in notes {
                if r.start <= w[0] && w[1] <= r.end {
                    if *is_diagnosis && !active {
                        underline.get_or_insert(UnderlineStyle {
                            color: Some(rgb(MUTED_COLOR).into()),
                            thickness: px(1.),
                            wavy: false,
                        });
                        continue;
                    }
                    let tint = if *active {
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
                        Some(bg) => blend_over(rgba(0x7FB8A455), bg),
                        None => rgba(0x7FB8A455), // sage — distinct from wheat
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
            self.editor.update(cx, |editor, _| {
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
        let find_matches: Vec<Range<usize>> = if in_history {
            diff_inserts.clone() // inserts reuse the sage tint
        } else {
            editor
                .find_input
                .as_ref()
                .map(|i| editor.find_matches(&i.read(cx).content))
                .unwrap_or_default()
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
                        editor.active_note == Some(n.id),
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

        let mut paragraphs = Vec::new();
        let mut top = px(0.);
        let mut offset = 0;
        let mut ordered_no = 0usize;
        for (block_ix, par_text) in text.split('\n').enumerate() {
            let kind = kinds.get(block_ix).cloned().unwrap_or_default();
            let bstyle = block_style_scaled(&kind, font_scale);
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
                marker,
                image,
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

        if let Some(start) = perf_start {
            eprintln!(
                "strop-perf: prepaint {:?} ({} blocks)",
                start.elapsed(),
                paragraphs.len()
            );
        }

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
            if let Some((render, sz)) = &par.image {
                if let Err(e) = window.paint_image(
                    Bounds::new(origin, *sz),
                    Corners::default(),
                    render.clone(),
                    0,
                    false,
                ) {
                    eprintln!("strop: paint image: {e}");
                }
            }
            if let Some(marker) = &par.marker {
                let run = TextRun {
                    len: marker.len(),
                    font: gpui::font("PT Serif"),
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

    // UI chrome avoids glyphs outside the bundled PT fonts (arrows, circles,
    // checks): every such character forces a mid-session system-font fallback
    // load, the exact path behind the garbled-glyph bugs. Indicators that
    // have no PT-covered character are drawn as divs instead.
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
            .font_family("PT Serif")
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
                    .child(match (&self.doc_rename_input, &self.store) {
                        // F2 / click: rename the document right here.
                        (Some(input), _) => div()
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
                                .px(px(4.))
                                .rounded(px(4.))
                                .cursor(CursorStyle::PointingHand)
                                .hover(|d| d.bg(rgba(0x1A1A180Au32)))
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
                        (None, None) => div().child("Strop").into_any_element(),
                    })
                    .when(self.diagnosis_running, |d| {
                        d.child(div().ml(px(10.)).child("· диагноз…"))
                    })
                    .when_some(self.last_ai_error.clone(), |d, err| {
                        d.child(
                            div()
                                .ml(px(10.))
                                .text_color(rgb(0xA04A3A))
                                .child(format!("AI: {err}")),
                        )
                    }),
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
                    .text_color(if self.history_view.is_some() {
                        rgb(TEXT_COLOR)
                    } else {
                        rgb(MUTED_COLOR)
                    })
                    .when(self.history_view.is_some(), |d| d.bg(rgba(0x1A1A1812u32)))
                    .hover(|d| d.bg(rgba(0x1A1A180Au32)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            if editor.history_view.is_some() {
                                editor.exit_history(cx);
                            } else {
                                editor.enter_history(cx);
                            }
                        }),
                    )
                    .child(
                        // History: drawn clock-face stand-in (↺ isn't in PT).
                        div()
                            .size(px(11.))
                            .rounded_full()
                            .border_1()
                            .border_color(if self.history_view.is_some() {
                                rgb(TEXT_COLOR)
                            } else {
                                rgb(MUTED_COLOR)
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(div().size(px(3.)).rounded_full().bg(
                                if self.history_view.is_some() {
                                    rgb(TEXT_COLOR)
                                } else {
                                    rgb(MUTED_COLOR)
                                },
                            )),
                    ),
            )
            .child(
                // The day-zero affordance: a user who knows nothing clicks
                // the only unexplained button and lands in a searchable
                // list of every capability (GNOME primary-menu position).
                div()
                    .id("palette-toggle")
                    .px(px(8.))
                    .py(px(2.))
                    .ml(px(4.))
                    .rounded(px(5.))
                    .cursor(CursorStyle::PointingHand)
                    .when(self.palette_input.is_some(), |d| d.bg(rgba(0x1A1A1812u32)))
                    .hover(|d| d.bg(rgba(0x1A1A180Au32)))
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
                    .w(px(28.))
                    .h_full()
                    .on_mouse_down(MouseButton::Left, drag),
            )
            .child(self.window_button("–", |window, _| window.minimize_window()))
            .child(
                // Zoom: drawn square (U+25A1 isn't in PT).
                div()
                    .id("win-zoom")
                    .w(px(34.))
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .hover(|d| d.bg(rgba(0x1A1A180Au32)))
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
            .child(self.window_button("×", |_, cx| cx.quit()))
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
        let hv = self.history_view.as_ref();
        let (entries, selected, named_only, compare_current) = match hv {
            Some(hv) => (
                hv.entries.as_slice(),
                hv.selected,
                hv.named_only,
                hv.compare_current,
            ),
            None => (&[][..], 0, false, false),
        };
        let toggle = |label: &'static str, on: bool| {
            div()
                .id(label)
                .px(px(6.))
                .py(px(1.))
                .rounded(px(4.))
                .cursor(CursorStyle::PointingHand)
                .text_color(if on { rgb(TEXT_COLOR) } else { rgb(MUTED_COLOR) })
                .when(on, |d| d.bg(rgba(0x1A1A1812u32)))
                .hover(|d| d.bg(rgba(0x1A1A180Au32)))
                .child(label)
        };
        let mut last_day = String::new();
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        for (ix, e) in entries.iter().enumerate() {
            if named_only && !e.manual {
                continue;
            }
            let stamp = format_unix(e.created_unix);
            let (day, time) = stamp.split_once(' ').unwrap_or((stamp.as_str(), ""));
            if day != last_day {
                last_day = day.to_owned();
                rows.push(
                    div()
                        .px(px(8.))
                        .pt(px(8.))
                        .text_size(px(11.))
                        .text_color(rgb(MUTED_COLOR))
                        .child(day.to_owned())
                        .into_any_element(),
                );
            }
            let (ins, del) = e.delta;
            let delta = if ins == 0 && del == 0 {
                String::new()
            } else {
                format!("+{ins} −{del}")
            };
            let active = ix == selected;
            rows.push(
                div()
                    .id(("hist-row", ix))
                    .px(px(8.))
                    .py(px(4.))
                    .rounded(px(5.))
                    .cursor(CursorStyle::PointingHand)
                    .when(active, |d| d.bg(rgba(0x1A1A1812u32)))
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
                                            // Drawn marker: ●/○ aren't in PT.
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
                                                .text_color(rgb(TEXT_COLOR))
                                                .when(e.manual, |d| {
                                                    d.font_weight(FontWeight::BOLD)
                                                })
                                                .child(e.name.clone()),
                                        ),
                                },
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .ml(px(6.))
                                    .text_size(px(11.))
                                    .text_color(rgb(MUTED_COLOR))
                                    .child(format!("{time}  {delta}")),
                            ),
                    )
                    .into_any_element(),
            );
        }
        div()
            .id("history-panel")
            .absolute()
            .top(px(BAR_HEIGHT + 8.))
            .right(px(8.))
            .w(px(300.))
            .max_h(px(520.))
            .overflow_y_scroll()
            .bg(rgb(0xF4F1EA))
            .border_1()
            .border_color(rgb(RULE_COLOR))
            .rounded(px(8.))
            .p(px(6.))
            .flex()
            .flex_col()
            .gap(px(2.))
            .font_family("PT Serif")
            .text_size(px(13.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .px(px(8.))
                    .py(px(4.))
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(div().text_color(rgb(MUTED_COLOR)).child("History"))
                    .child(
                        div()
                            .flex()
                            .gap(px(4.))
                            .child(
                                toggle("named", named_only).on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                                        cx.stop_propagation();
                                        if let Some(hv) = &mut editor.history_view {
                                            hv.named_only = !hv.named_only;
                                            cx.notify();
                                        }
                                    }),
                                ),
                            )
                            .child(
                                toggle("vs draft", compare_current).on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|editor, _: &MouseDownEvent, _, cx| {
                                        cx.stop_propagation();
                                        if let Some(hv) = &mut editor.history_view {
                                            hv.compare_current = !hv.compare_current;
                                            editor.rebuild_preview();
                                            cx.notify();
                                        }
                                    }),
                                ),
                            )
                            .child(
                                div()
                                    .id("restore-btn")
                                    .px(px(8.))
                                    .py(px(1.))
                                    .rounded(px(4.))
                                    .cursor(CursorStyle::PointingHand)
                                    .bg(rgb(0xE8DFC8))
                                    .text_color(rgb(TEXT_COLOR))
                                    .hover(|d| d.bg(rgb(0xDFD3B0)))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(
                                            |editor, _: &MouseDownEvent, _, cx| {
                                                cx.stop_propagation();
                                                editor.restore_selected(cx);
                                            },
                                        ),
                                    )
                                    .child("Restore"),
                            ),
                    ),
            )
            .child(
                div()
                    .px(px(8.))
                    .text_size(px(11.))
                    .text_color(rgb(MUTED_COLOR))
                    .child("Up/Down step versions · Esc exits · restoring is undoable"),
            )
            .when_some(self.voice_baseline.as_ref(), |d, baseline| {
                let lang = match self.config.language {
                    Language::Ru => typograph::Lang::Ru,
                    Language::En => typograph::Lang::En,
                    Language::Auto => typograph::detect_lang(self.doc.rope().chars()),
                };
                let report =
                    baseline.assess(&strop_core::voice::signature(&self.doc.text(), lang));
                let ru = lang == typograph::Lang::Ru;
                let headline = if report.overall_sigma > 2. {
                    if ru {
                        format!(
                            "Голос: {:.1}σ за пределами вашей нормы ({} текстов)",
                            report.overall_sigma, baseline.docs
                        )
                    } else {
                        format!(
                            "Voice: {:.1}σ outside your normal range ({} texts)",
                            report.overall_sigma, baseline.docs
                        )
                    }
                } else if ru {
                    format!("Голос: в пределах вашей нормы ({} текстов)", baseline.docs)
                } else {
                    format!("Voice: within your normal range ({} texts)", baseline.docs)
                };
                d.child(
                    div()
                        .px(px(8.))
                        .pt(px(4.))
                        .flex()
                        .flex_col()
                        .gap(px(1.))
                        .text_size(px(11.))
                        .text_color(if report.overall_sigma > 2. {
                            rgb(0xA04A3A)
                        } else {
                            rgb(MUTED_COLOR)
                        })
                        .children(std::iter::once(headline).chain(report.flags)),
                )
            })
            .when(compare_current, |d| {
                // Voice drift v0: descriptive stylometry between the
                // selected version and the draft (rhythm first — research:
                // flattening variance is the LLM-characteristic signal).
                let lang = match self.config.language {
                    Language::Ru => typograph::Lang::Ru,
                    Language::En => typograph::Lang::En,
                    Language::Auto => typograph::detect_lang(self.doc.rope().chars()),
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
                            .px(px(8.))
                            .pt(px(4.))
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
            .children(rows)
    }

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
                    .font_family("PT Serif")
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

/// One rewind-list entry, materialized on entering history mode.
struct HistoryEntry {
    name: String,
    created_unix: i64,
    manual: bool,
    frontiers: Vec<u8>,
    text: String,
    /// Formatting at this checkpoint — projected into the preview so the
    /// document doesn't strip to plain text while time-travelling.
    spans: SpanSet,
    blocks: BlockMap,
    /// (+words, -words) vs the previous checkpoint.
    delta: (usize, usize),
}

/// History mode: Docs-style list + read-only inline-diff preview.
struct HistoryView {
    entries: Vec<HistoryEntry>,
    selected: usize,
    named_only: bool,
    /// false: diff vs previous checkpoint ("work of that session");
    /// true: diff vs the current draft ("what restoring would change").
    compare_current: bool,
}

/// One palette row: a command from the registry, or a recent document.
enum PaletteRow {
    Cmd(&'static crate::commands::Command),
    Recent(std::path::PathBuf),
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

struct MarginCard {
    id: u64,
    top: f32,
    height: f32,
    body: String,
    active: bool,
    kind: NoteKind,
    title: String,
    level: String,
}

impl Editor {
    /// The Docs-style margin solver (via Liveblocks' AnchoredThreads):
    /// downward sweep normally; with an active card, it snaps to its anchor,
    /// later cards push down, earlier cards push up from it in reverse.
    fn margin_cards(&self) -> Vec<MarginCard> {
        let Some(frame) = self.last_frame.as_ref() else {
            return Vec::new();
        };
        let rope = self.doc.rope();
        let len = self.doc.len_bytes();
        let mut cards: Vec<MarginCard> = Vec::new();
        for n in self.doc.notes().open() {
            let byte = rope.char_to_byte(n.range.start.min(rope.len_chars())).min(len);
            let Some(pos) = frame.position_of(byte, false) else {
                continue;
            };
            let desired =
                f32::from(frame.bounds.origin.y) + f32::from(pos.y) - f32::from(frame.scroll_top);
            let text_len = n.title.chars().count() + n.body.chars().count();
            let lines = (text_len / 30 + 1).clamp(1, 4) as f32;
            let height = 30. + 18. * lines + 22.;
            cards.push(MarginCard {
                id: n.id,
                top: desired,
                height,
                body: n.body.clone(),
                active: self.active_note == Some(n.id),
                kind: n.kind,
                title: n.title.clone(),
                level: n.level.clone(),
            });
        }
        // cards are in document order (notes are kept sorted by anchor).
        let active_ix = cards.iter().position(|c| c.active);
        match active_ix {
            None => {
                let mut bottom = f32::MIN;
                for card in cards.iter_mut() {
                    card.top = card.top.max(bottom);
                    bottom = card.top + card.height + MARGIN_GAP;
                }
            }
            Some(a) => {
                // Ascending from the active card (which gets its anchor y).
                let mut bottom = f32::MIN;
                for card in cards[a..].iter_mut() {
                    card.top = card.top.max(bottom);
                    bottom = card.top + card.height + MARGIN_GAP;
                }
                // Descending above it, nearest first: push up out of the way.
                let mut top_limit = cards[a].top;
                for card in cards[..a].iter_mut().rev() {
                    let max_top = top_limit - card.height - MARGIN_GAP;
                    card.top = card.top.min(max_top);
                    top_limit = card.top;
                }
            }
        }
        cards
    }

    fn margin_fits(&self, window: &Window) -> bool {
        let vw = f32::from(window.viewport_size().width);
        let Some(frame) = self.last_frame.as_ref() else {
            return false;
        };
        let col_right = f32::from(frame.bounds.origin.x) + f32::from(frame.bounds.size.width);
        vw >= col_right + MARGIN_GAP + MARGIN_WIDTH + 8.
    }

    /// Narrow-window composer: the margin (and its in-card composer) is
    /// hidden, so the note body is edited in a bottom strip instead.
    fn render_composer_strip(&self) -> Option<impl IntoElement> {
        let input = self.note_input.clone()?;
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

    fn render_margin(&self, window: &Window, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if !self.margin_fits(window) {
            return None;
        }
        let frame = self.last_frame.as_ref()?;
        let col_right = f32::from(frame.bounds.origin.x) + f32::from(frame.bounds.size.width);
        let cards = self.margin_cards();
        if cards.is_empty() {
            return None;
        }
        let lane_left = col_right + MARGIN_GAP;
        Some(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(px(lane_left))
                .w(px(MARGIN_WIDTH))
                .children(cards.into_iter().map(|card| {
                    let MarginCard {
                        id,
                        top,
                        body,
                        active,
                        kind,
                        title,
                        level,
                        ..
                    } = card;
                    let composer = if active { self.note_input.clone() } else { None };
                    let is_diagnosis = kind == NoteKind::Diagnosis;
                    div()
                        .id(("note-card", id as usize))
                        .absolute()
                        .top(px(top.max(4.)))
                        .left(px(if active { 0. } else { 8. }))
                        .w(px(MARGIN_WIDTH - 8.))
                        .p(px(8.))
                        .rounded(px(6.))
                        .bg(rgb(0xFFFDF6))
                        .border_1()
                        .border_color(if active {
                            rgb(0xC8A951)
                        } else {
                            rgb(RULE_COLOR)
                        })
                        .cursor(CursorStyle::PointingHand)
                        .font_family("PT Serif")
                        .text_size(px(13.))
                        .text_color(rgb(TEXT_COLOR))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |editor, _: &MouseDownEvent, window, cx| {
                                cx.stop_propagation();
                                if editor.active_note != Some(id) {
                                    let note = editor.doc.notes().get(id);
                                    let is_note =
                                        note.is_some_and(|n| n.kind == NoteKind::Note);
                                    let body =
                                        note.map(|n| n.body.clone()).unwrap_or_default();
                                    if is_note {
                                        editor.open_composer(id, body, window, cx);
                                    } else {
                                        editor.active_note = Some(id);
                                    }
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
                                .child(if is_diagnosis {
                                    if level.is_empty() {
                                        "Diagnosis".to_owned()
                                    } else {
                                        level.clone()
                                    }
                                } else {
                                    "Note".to_owned()
                                })
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
                                                              _,
                                                              cx| {
                                                            cx.stop_propagation();
                                                            editor.set_note_status(
                                                                id,
                                                                NoteStatus::Done,
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
                                                              _,
                                                              cx| {
                                                            cx.stop_propagation();
                                                            editor.set_note_status(
                                                                id,
                                                                NoteStatus::Dismissed,
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
                        .when_some(composer, |d, input| d.child(input))
                        .when(!active || is_diagnosis, |d| {
                            d.child(if body.is_empty() && !is_diagnosis {
                                div().text_color(rgb(MUTED_COLOR)).child("(empty note)")
                            } else {
                                div().child(body.clone())
                            })
                        })
                })),
        )
    }
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .relative()
            .bg(rgb(BG_COLOR))
            .flex()
            .flex_col()
            // Bottom strips (find/replace, composer) mount on this root,
            // outside the column's listener stack — actions from their
            // inputs bubble here.
            .on_action(cx.listener(Self::note_tab))
            .child(self.render_titlebar(cx))
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
                    .on_action(cx.listener(Self::add_note))
                    .on_action(cx.listener(Self::run_diagnosis))
                    .on_action(cx.listener(Self::run_believing))
                    .on_action(cx.listener(Self::find))
                    .on_action(cx.listener(Self::replace))
                    .on_action(cx.listener(Self::note_tab))
                    .on_action(cx.listener(Self::escape_mode))
                    .on_action(cx.listener(Self::toggle_history))
                    .on_action(cx.listener(Self::open_file))
                    .on_action(cx.listener(Self::save_copy_as))
                    .on_action(cx.listener(Self::toggle_palette))
                    .on_action(cx.listener(Self::palette_up))
                    .on_action(cx.listener(Self::palette_down))
                    .on_action(cx.listener(Self::new_document))
                    .on_action(cx.listener(Self::rename_document))
                    .on_action(cx.listener(Self::reveal_in_files))
                    .on_action(cx.listener(Self::copy_document_path))
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                    .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_middle_click))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .on_mouse_move(cx.listener(Self::on_mouse_move))
                    .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
                    .on_drop(cx.listener(Self::on_file_drop))
                            .w_full()
                            .max_w(px(660.))
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
                d.child(self.render_history_panel(cx))
            })
            .map(|d| {
                let footnotes = self.visible_footnotes();
                d.when(!footnotes.is_empty(), |d| {
                    d.child(self.render_footnote_zone(footnotes, cx))
                })
            })
            .map(|d| match self.render_margin(window, cx) {
                Some(margin) => d.child(margin),
                None => d,
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
            .map(|d| match self.render_find_strip(cx) {
                Some(strip) => d.child(strip),
                None => d,
            })
            .map(|d| match self.render_alt_strip() {
                Some(strip) => d.child(strip),
                None => d,
            })
            // Last child = topmost: the palette covers everything below.
            .when(self.palette_input.is_some(), |d| {
                d.child(self.render_palette(cx))
            })
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
    fn code_run_switches_family_and_marked_text_underlines() {
        let par = 0..8;
        let spans = vec![(0..4, InlineAttr::Code)];
        let runs = runs_for_paragraph(&par, &(0..0), Some(&(4..8)), &spans, &[], &[], &[], &base());
        assert_eq!(runs[0].font.family.as_ref(), CODE_FONT);
        assert!(runs[1].underline.is_some());
    }
}
