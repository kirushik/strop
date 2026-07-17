//! The modeless keyboard reference (impl/18).  The command registry remains
//! the authority; this module owns only its presentation model and window.

use gpui::{
    AnyWindowHandle, App, Bounds, BoxShadow, Context, CursorStyle, Decorations, Entity,
    FocusHandle, Focusable, Hsla, IntoElement, MouseButton, MouseDownEvent, Pixels, Render,
    ResizeEdge, Tiling, Window, WindowBackgroundAppearance, WindowBounds, WindowControlArea,
    WindowDecorations, WindowHandle, WindowOptions, div, px, rgb, rgba, size,
};
use gpui::prelude::*;

use crate::editor::{Editor, EscapeMode, ShowShortcuts};
use crate::draw_guard::EntityUpdateExt as _;
use crate::icons::{self, icon};

pub const DEFAULT_WIDTH: f32 = 900.;
pub const DEFAULT_HEIGHT: f32 = 560.;
const HEADER_HEIGHT: f32 = 68.;
const SECTION_HEADING_HEIGHT: f32 = 24.;
const ROW_PITCH: f32 = 16.;
const GRID_BOTTOM: f32 = 20.;
const CSD_GUTTER: f32 = 22.;
const CSD_ROUNDING: f32 = 10.;
const RESIZE_INSET: f32 = 8.;

fn default_window_size() -> (f32, f32) {
    (DEFAULT_WIDTH + 2. * CSD_GUTTER, DEFAULT_HEIGHT + 2. * CSD_GUTTER)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToggleDecision {
    Open,
    Raise,
    CloseAndRestore,
}

pub fn toggle_decision(present: bool, reference_focused: bool) -> ToggleDecision {
    match (present, reference_focused) {
        (false, _) => ToggleDecision::Open,
        (true, false) => ToggleDecision::Raise,
        (true, true) => ToggleDecision::CloseAndRestore,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeymapRow {
    pub action: String,
    pub keys: String,
}

pub type KeymapSection = (String, Vec<KeymapRow>);

/// The non-command editing vocabulary has one owner, shared by the model and
/// renderer instead of living as an inline appendix in a paint method.
pub fn editing_baseline() -> Vec<KeymapRow> {
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
    .map(|(action, keys)| KeymapRow {
        action: action.to_owned(),
        keys: keys.to_owned(),
    })
    .collect()
}

pub fn keymap_sections() -> Vec<KeymapSection> {
    let mut sections: Vec<KeymapSection> = Vec::new();
    for command in crate::commands::all() {
        let row = KeymapRow {
            action: command.label.to_owned(),
            keys: command.keys.unwrap_or("palette").to_owned(),
        };
        match sections.iter_mut().find(|(name, _)| name == command.section) {
            Some((_, rows)) => rows.push(row),
            None => sections.push((command.section.to_owned(), vec![row])),
        }
    }
    sections.push(("Text editing".to_owned(), editing_baseline()));
    sections
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SheetLayout {
    pub columns: Vec<Vec<usize>>,
    pub scrolls: bool,
}

fn section_height(section: &KeymapSection) -> f32 {
    SECTION_HEADING_HEIGHT + section.1.len() as f32 * ROW_PITCH
}

/// Deterministic intact-section packing: registry order, always into the
/// currently shortest column (leftmost wins ties).
pub fn allocate_layout(width: f32, height: f32, sections: &[KeymapSection]) -> SheetLayout {
    let count = if width >= 780. { 3 } else if width >= 560. { 2 } else { 1 };
    let mut columns = vec![Vec::new(); count];
    let mut heights = vec![0.; count];
    for (ix, section) in sections.iter().enumerate() {
        let column = heights
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap().then(a.0.cmp(&b.0)))
            .map(|(ix, _)| ix)
            .unwrap();
        columns[column].push(ix);
        heights[column] += section_height(section);
    }
    let available = (height - HEADER_HEIGHT - GRID_BOTTOM).max(0.);
    SheetLayout {
        columns,
        scrolls: heights.into_iter().any(|height| height > available),
    }
}

fn state_file() -> std::path::PathBuf {
    crate::paths::state_dir().join("keymap-window.json")
}

fn load_bounds() -> Option<(f32, f32, f32, f32)> {
    serde_json::from_str(&std::fs::read_to_string(state_file()).ok()?).ok()
}

fn save_bounds(bounds: Bounds<Pixels>) {
    let path = state_file();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let record = (
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        f32::from(bounds.size.height),
    );
    if let Ok(json) = serde_json::to_string(&record) {
        let _ = std::fs::write(path, json);
    }
}

pub fn clamp_bounds(
    record: (f32, f32, f32, f32),
    work: (f32, f32, f32, f32),
) -> (f32, f32, f32, f32) {
    let (wx, wy, ww, wh) = work;
    let w = record.2.max(400.).min(ww);
    let h = record.3.max(300.).min(wh);
    let x = record.0.max(wx).min(wx + ww - w);
    let y = record.1.max(wy).min(wy + wh - h);
    (x, y, w, h)
}

/// The editor chooses the monitor: remembered keymap coordinates must not pull
/// the reference back to a different display. A display contains the editor
/// only when it contains the editor's whole window rectangle.
pub fn containing_display(
    editor: (f32, f32, f32, f32),
    displays: &[(f32, f32, f32, f32)],
) -> Option<usize> {
    let (ex, ey, ew, eh) = editor;
    displays.iter().position(|&(x, y, w, h)| {
        ex >= x && ey >= y && ex + ew <= x + w && ey + eh <= y + h
    })
}

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

fn resize_handles(tiling: Tiling) -> Vec<gpui::AnyElement> {
    let out = px(CSD_GUTTER - RESIZE_INSET);
    let thick = px(2. * RESIZE_INSET);
    let thin = px(RESIZE_INSET);
    let mut handles = Vec::new();
    if !tiling.top {
        handles.push(resize_strip("keymap-rz-top", ResizeEdge::Top, CursorStyle::ResizeUpDown)
            .top(out).left(out).right(out).h(thin).into_any_element());
    }
    if !tiling.bottom {
        handles.push(resize_strip("keymap-rz-bottom", ResizeEdge::Bottom, CursorStyle::ResizeUpDown)
            .bottom(out).left(out).right(out).h(thick).into_any_element());
    }
    if !tiling.left {
        handles.push(resize_strip("keymap-rz-left", ResizeEdge::Left, CursorStyle::ResizeLeftRight)
            .left(out).top(out).bottom(out).w(thick).into_any_element());
    }
    if !tiling.right {
        handles.push(resize_strip("keymap-rz-right", ResizeEdge::Right, CursorStyle::ResizeLeftRight)
            .right(out).top(out).bottom(out).w(thick).into_any_element());
    }
    if !tiling.top && !tiling.left {
        handles.push(resize_strip("keymap-rz-tl", ResizeEdge::TopLeft, CursorStyle::ResizeUpLeftDownRight)
            .top(out).left(out).size(thick).into_any_element());
    }
    if !tiling.top && !tiling.right {
        handles.push(resize_strip("keymap-rz-tr", ResizeEdge::TopRight, CursorStyle::ResizeUpRightDownLeft)
            .top(out).right(out).size(thick).into_any_element());
    }
    if !tiling.bottom && !tiling.left {
        handles.push(resize_strip("keymap-rz-bl", ResizeEdge::BottomLeft, CursorStyle::ResizeUpRightDownLeft)
            .bottom(out).left(out).size(thick).into_any_element());
    }
    if !tiling.bottom && !tiling.right {
        handles.push(resize_strip("keymap-rz-br", ResizeEdge::BottomRight, CursorStyle::ResizeUpLeftDownRight)
            .bottom(out).right(out).size(thick).into_any_element());
    }
    handles
}

pub struct KeymapWindow {
    focus_handle: FocusHandle,
    editor: Entity<Editor>,
    editor_window: AnyWindowHandle,
    sections: Vec<KeymapSection>,
}

impl Focusable for KeymapWindow {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl KeymapWindow {
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let bounds = window.bounds();
        let editor = self.editor.clone();
        let editor_window = self.editor_window;
        window.remove_window();
        save_bounds(bounds);
        editor.update_checked(cx, |editor, _| editor.keymap_closed());
        let _ = editor_window.update(cx, |_, window, cx| {
            window.activate_window();
            window.focus(&editor.focus_handle(cx), cx);
        });
    }


    fn request_quit(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let editor = self.editor.clone();
        let _ = self.editor_window.update(cx, move |_, editor_window, cx| {
            editor_window.activate_window();
            editor.update_checked(cx, |editor, cx| {
                editor.request_quit(&crate::Quit, editor_window, cx);
            });
        });
    }
}

impl Render for KeymapWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bounds = window.bounds();
        let decorations = window.window_decorations();
        let tiling = match decorations {
            Decorations::Client { tiling } => tiling,
            Decorations::Server => Tiling::default(),
        };
        let client = matches!(decorations, Decorations::Client { .. });
        window.set_client_inset(px(if client { CSD_GUTTER } else { 0. }));
        let horizontal_inset = if client {
            (if tiling.left { 0. } else { CSD_GUTTER })
                + if tiling.right { 0. } else { CSD_GUTTER }
        } else { 0. };
        let vertical_inset = if client {
            (if tiling.top { 0. } else { CSD_GUTTER })
                + if tiling.bottom { 0. } else { CSD_GUTTER }
        } else { 0. };
        let layout = allocate_layout(
            f32::from(bounds.size.width) - horizontal_inset,
            f32::from(bounds.size.height) - vertical_inset,
            &self.sections,
        );
        let drag = |_: &MouseDownEvent, window: &mut Window, _: &mut App| {
            window.start_window_move();
        };
        let content = div()
            .key_context("Keymap")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &EscapeMode, window, cx| this.close(window, cx)))
            .on_action(cx.listener(|this, _: &ShowShortcuts, window, cx| this.close(window, cx)))
            .on_action(cx.listener(|this, _: &crate::Quit, window, cx| this.request_quit(window, cx)))
            .size_full()
            .bg(rgb(0xF6F4EF))
            .font_family("PT Sans")
            .text_color(rgb(0x242321))
            .child(
                div()
                    .h(px(HEADER_HEIGHT))
                    .window_control_area(WindowControlArea::Drag)
                    .on_mouse_down(MouseButton::Left, drag)
                    .px(px(22.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(rgb(0xC9C4BA))
                    .child(div().text_size(px(18.)).child("Keyboard map"))
                    .child(
                        div().flex().items_center().h_full().child(
                            div()
                                .text_size(px(10.))
                                .text_color(rgb(0x716D66))
                                .child("PHYSICAL KEYS"),
                        ).when(client, |d| d.child(
                            div().id("keymap-close").occlude().ml(px(14.)).w(px(28.)).h_full()
                                .flex().items_center().justify_center().cursor(CursorStyle::PointingHand)
                                .group("keymap-close").hover(|d| d.bg(rgba(0x1A1A180A)))
                                .on_mouse_down(MouseButton::Left,
                                    cx.listener(|this, _: &MouseDownEvent, window, cx| {
                                        cx.stop_propagation();
                                        this.close(window, cx);
                                    }))
                                .child(icon(icons::WIN_CLOSE, 13., 0x716D66)
                                    .group_hover("keymap-close", |s| s.text_color(rgb(0x242321))))
                        )),
                    ),
            )
            .child(
                div()
                    .id("keymap-grid")
                    .h(px((f32::from(bounds.size.height) - vertical_inset - HEADER_HEIGHT).max(0.)))
                    .when(layout.scrolls, |grid| grid.overflow_y_scroll())
                    .px(px(22.))
                    .pt(px(14.))
                    .pb(px(GRID_BOTTOM))
                    .flex()
                    .gap(px(22.))
                    .children(layout.columns.into_iter().map(|column| {
                        div().flex_1().min_w_0().children(column.into_iter().map(|ix| {
                            let (name, rows) = &self.sections[ix];
                            div()
                                .pb(px(8.))
                                .child(
                                    div()
                                        .h(px(SECTION_HEADING_HEIGHT))
                                        .text_size(px(10.))
                                        .text_color(rgb(0x716D66))
                                        .child(name.to_uppercase()),
                                )
                                .children(rows.iter().map(|row| {
                                    div()
                                        .h(px(ROW_PITCH))
                                        .flex()
                                        .justify_between()
                                        .gap(px(10.))
                                        .child(div().min_w_0().text_size(px(12.)).child(row.action.clone()))
                                        .child(
                                            div()
                                                .flex_none()
                                                .text_size(px(11.))
                                                .text_color(rgb(0x625F59))
                                                .child(row.keys.clone()),
                                        )
                                }))
                        }))
                    })),
            );

        let inset = |tiled: bool| px(if client && !tiled { CSD_GUTTER } else { 0. });
        let floating = client && !tiling.top && !tiling.bottom && !tiling.left && !tiling.right;
        let round = move |d: gpui::Div| {
            d.when(!tiling.top && !tiling.left, |d| d.rounded_tl(px(CSD_ROUNDING)))
                .when(!tiling.top && !tiling.right, |d| d.rounded_tr(px(CSD_ROUNDING)))
                .when(!tiling.bottom && !tiling.left, |d| d.rounded_bl(px(CSD_ROUNDING)))
                .when(!tiling.bottom && !tiling.right, |d| d.rounded_br(px(CSD_ROUNDING)))
        };
        div().size_full().relative().bg(rgba(0x00000000)).child(
            div().absolute()
                .top(inset(tiling.top)).bottom(inset(tiling.bottom))
                .left(inset(tiling.left)).right(inset(tiling.right))
                .overflow_hidden()
                .when(client, |d| {
                    let d = d.border_color(rgb(0xC9C4BA))
                        .when(!tiling.top, |d| d.border_t_1())
                        .when(!tiling.bottom, |d| d.border_b_1())
                        .when(!tiling.left, |d| d.border_l_1())
                        .when(!tiling.right, |d| d.border_r_1());
                    round(d).when(floating, |d| {
                        let shadow = |y: f32, blur: f32, a: f32| {
                            BoxShadow::new(px(0.), px(y), Hsla { h: 0., s: 0., l: 0., a })
                                .blur_radius(px(blur))
                        };
                        d.shadow(vec![shadow(1., 2., 0.14), shadow(3., 8., 0.10), shadow(6., 14., 0.07)])
                    })
                })
                .child(round(content)),
        ).when(client, |d| d.children(resize_handles(tiling)))
    }
}

pub fn open(
    editor: Entity<Editor>,
    editor_window: AnyWindowHandle,
    editor_bounds: Bounds<Pixels>,
    cx: &mut App,
) -> Option<WindowHandle<KeymapWindow>> {
    let displays = cx.displays();
    let editor_tuple = (
        f32::from(editor_bounds.origin.x), f32::from(editor_bounds.origin.y),
        f32::from(editor_bounds.size.width), f32::from(editor_bounds.size.height),
    );
    let display = containing_display(editor_tuple, &displays.iter().map(|display| {
        let bounds = display.bounds();
        (f32::from(bounds.origin.x), f32::from(bounds.origin.y),
         f32::from(bounds.size.width), f32::from(bounds.size.height))
    }).collect::<Vec<_>>())
        .and_then(|ix| displays.get(ix).cloned())
        .or_else(|| cx.primary_display());
    let work = display
        .as_ref()
        .map(|display| display.bounds())
        .unwrap_or_else(|| Bounds::centered(None, size(px(DEFAULT_WIDTH), px(DEFAULT_HEIGHT)), cx));
    let work_tuple = (
        f32::from(work.origin.x),
        f32::from(work.origin.y),
        f32::from(work.size.width),
        f32::from(work.size.height),
    );
    let (default_width, default_height) = default_window_size();
    let editor_right = f32::from(editor_bounds.origin.x + editor_bounds.size.width);
    let editor_left = f32::from(editor_bounds.origin.x);
    let beside_x = if editor_right + default_width <= work_tuple.0 + work_tuple.2 {
        editor_right
    } else if editor_left - default_width >= work_tuple.0 {
        editor_left - default_width
    } else {
        work_tuple.0 + (work_tuple.2 - default_width) / 2.
    };
    let remembered = load_bounds().unwrap_or((
        beside_x,
        f32::from(editor_bounds.origin.y),
        default_width,
        default_height,
    ));
    let (x, y, w, h) = clamp_bounds(remembered, work_tuple);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: gpui::point(px(x), px(y)),
                size: size(px(w), px(h)),
            })),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("Keyboard map".into()),
                ..Default::default()
            }),
            window_decorations: Some(WindowDecorations::Client),
            window_background: if cfg!(any(target_os = "linux", target_os = "freebsd")) {
                WindowBackgroundAppearance::Transparent
            } else {
                WindowBackgroundAppearance::Opaque
            },
            display_id: display.as_ref().map(|display| display.id()),
            focus: true,
            ..Default::default()
        },
        move |window, cx| {
            window.set_window_title("Keyboard map");
            let view = cx.new(|cx| KeymapWindow {
                focus_handle: cx.focus_handle(),
                editor: editor.clone(),
                editor_window,
                sections: keymap_sections(),
            });
            window.focus(&view.focus_handle(cx), cx);
            let close_editor = editor.clone();
            let restore_window = editor_window;
            window.on_window_should_close(cx, move |window, cx| {
                save_bounds(window.bounds());
                close_editor.update_checked(cx, |editor, _| editor.keymap_closed());
                let _ = restore_window.update(cx, |_, window, cx| {
                    window.activate_window();
                    window.focus(&close_editor.focus_handle(cx), cx);
                });
                true
            });
            view
        },
    )
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_contains_registry_once_in_deterministic_order() {
        let first = keymap_sections();
        assert_eq!(first, keymap_sections());
        let mut rows: Vec<_> = first[..first.len() - 1]
            .iter()
            .flat_map(|(_, rows)| rows.iter().map(|row| row.action.as_str()))
            .collect();
        assert_eq!(rows.len(), crate::commands::all().len());
        rows.sort_unstable();
        let mut registry = crate::commands::all()
            .iter()
            .map(|command| command.label)
            .collect::<Vec<_>>();
        registry.sort_unstable();
        assert_eq!(rows, registry);
        let section_order = crate::commands::all().iter().fold(
            Vec::new(),
            |mut names, command| {
                if !names.contains(&command.section) {
                    names.push(command.section);
                }
                names
            },
        );
        assert_eq!(
            first[..first.len() - 1].iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>(),
            section_order
        );
        for command in crate::commands::all().iter().filter(|command| command.keys.is_none()) {
            assert!(first.iter().flat_map(|(_, rows)| rows).any(|row| {
                row.action == command.label && row.keys == "palette"
            }));
        }
    }

    #[test]
    fn layout_breakpoints_keep_sections_intact_and_default_fits() {
        let sections = keymap_sections();
        for (width, columns) in [(900., 3), (779., 2), (560., 2), (559., 1)] {
            let layout = allocate_layout(width, 1000., &sections);
            assert_eq!(layout.columns.len(), columns);
            let mut assigned: Vec<_> = layout.columns.into_iter().flatten().collect();
            assigned.sort_unstable();
            assert_eq!(assigned, (0..sections.len()).collect::<Vec<_>>());
        }
        assert!(
            !allocate_layout(900., 560., &sections).scrolls,
            "registry outgrew the default no-scroll density budget"
        );
        assert!(
            !allocate_layout(
                default_window_size().0 - 2. * CSD_GUTTER,
                default_window_size().1 - 2. * CSD_GUTTER,
                &sections,
            ).scrolls,
            "client decoration inset broke the default no-scroll budget"
        );
        assert!(allocate_layout(900., 300., &sections).scrolls);
    }

    #[test]
    fn bounds_are_fully_clamped_to_the_work_area() {
        assert_eq!(
            clamp_bounds((-200., 900., 900., 560.), (100., 50., 800., 500.)),
            (100., 50., 800., 500.)
        );
    }

    #[test]
    fn editor_display_selection_handles_a_two_display_matrix() {
        let displays = [(0., 0., 1920., 1080.), (1920., -200., 2560., 1440.)];
        assert_eq!(containing_display((100., 80., 900., 700.), &displays), Some(0));
        assert_eq!(containing_display((2100., 0., 1200., 900.), &displays), Some(1));
        assert_eq!(containing_display((1800., 0., 300., 700.), &displays), None);
        assert_eq!(containing_display((5000., 0., 800., 600.), &displays), None);

        assert_eq!(
            clamp_bounds((100., 100., 900., 560.), displays[1]),
            (1920., 100., 900., 560.),
            "remembered primary-display bounds clamp onto the editor's display"
        );
    }

    #[test]
    fn controller_toggle_has_one_window_and_restores_on_the_focused_toggle() {
        assert_eq!(toggle_decision(false, false), ToggleDecision::Open);
        assert_eq!(toggle_decision(true, false), ToggleDecision::Raise);
        assert_eq!(
            toggle_decision(true, true),
            ToggleDecision::CloseAndRestore
        );
    }
}
