//! Shared exterior for modeless auxiliary windows.  P1 requires these
//! sheets to remain visibly distinct from the writer's paper; keeping the
//! decoration response here prevents each auxiliary surface from inventing
//! a subtly different border, shadow, or close affordance.

use gpui::{
    AnyWindowHandle, App, BoxShadow, CursorStyle, Decorations, Div, Entity, Focusable, Hsla,
    IntoElement, KeyBinding, MouseButton,
    MouseDownEvent, Tiling, Window, WindowBackgroundAppearance, WindowControlArea,
    WindowDecorations, WindowOptions, div, px, rgb, rgba,
};
use gpui::prelude::*;

use crate::icons::{self, icon};

pub const KEY_CONTEXT: &str = "AuxWindow";
pub const HEADER_HEIGHT: f32 = 68.;
pub const CSD_GUTTER: f32 = 22.;
const CSD_ROUNDING: f32 = 10.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToggleDecision {
    Open,
    CloseAndRestore,
}

pub fn toggle_decision(present: bool) -> ToggleDecision {
    if present { ToggleDecision::CloseAndRestore } else { ToggleDecision::Open }
}

/// Shared keyboard grammar for every auxiliary window.  Keeping the
/// context and bindings together makes it impossible for a new sheet to
/// listen for a verb which its focused tree can never resolve.
pub fn bindings() -> Vec<KeyBinding> {
    vec![KeyBinding::new(
        "escape", crate::editor::EscapeMode, Some(KEY_CONTEXT))]
}

pub fn restore_editor_focus(
    editor: &Entity<crate::editor::Editor>,
    editor_window: AnyWindowHandle,
    cx: &mut App,
) {
    let _ = editor_window.update(cx, |_, window, cx| {
        window.activate_window();
        window.focus(&editor.focus_handle(cx), cx);
    });
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

pub fn containing_display(
    editor: (f32, f32, f32, f32),
    displays: &[(f32, f32, f32, f32)],
) -> Option<usize> {
    let (ex, ey, ew, eh) = editor;
    displays.iter().position(|&(x, y, w, h)| {
        ex >= x && ey >= y && ex + ew <= x + w && ey + eh <= y + h
    })
}

pub struct Metrics {
    pub client: bool,
    pub tiling: Tiling,
    pub horizontal_inset: f32,
    pub vertical_inset: f32,
}

pub fn window_options(title: &str) -> WindowOptions {
    WindowOptions {
        app_id: Some("cc.pimenov.strop".to_owned()),
        titlebar: Some(gpui::TitlebarOptions {
            title: Some(title.into()),
            ..Default::default()
        }),
        window_decorations: Some(WindowDecorations::Client),
        is_resizable: false,
        window_background: if cfg!(any(target_os = "linux", target_os = "freebsd")) {
            WindowBackgroundAppearance::Transparent
        } else {
            WindowBackgroundAppearance::Opaque
        },
        focus: true,
        ..Default::default()
    }
}

pub fn metrics(window: &mut Window) -> Metrics {
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
    Metrics { client, tiling, horizontal_inset, vertical_inset }
}

pub fn titlebar(
    title: &'static str,
    trailing: Option<Div>,
    client: bool,
    close_id: &'static str,
    close_group: &'static str,
    close: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Div {
    div()
        .h(px(HEADER_HEIGHT))
        .px(px(22.))
        .flex()
        .items_center()
        .justify_between()
        .border_b_1()
        .border_color(rgb(0xC9C4BA))
        .child(div().text_size(px(18.)).child(title))
        .child(
            div().flex().items_center().h_full()
                .when_some(trailing, |d, trailing| d.child(trailing))
                .when(client, |d| d.child(
                    // Full-height hitbox, but the hover wash is the same 26px
                    // rounded square the editor's window controls wear — in a
                    // 68px header the old edge-to-edge wash was a floor-to-
                    // ceiling sliver that read as a glitch.
                    div().id(close_id).occlude().ml(px(14.)).w(px(28.)).h_full()
                        .flex().items_center().justify_center().cursor(CursorStyle::PointingHand)
                        .group(close_group)
                        .on_mouse_down(MouseButton::Left, close)
                        .child(div().w(px(26.)).h(px(26.)).rounded(px(5.))
                            .flex().items_center().justify_center()
                            .group_hover(close_group, |d| d.bg(rgba(0x1A1A180A)))
                            .child(icon(icons::WIN_CLOSE, 13., 0x716D66)
                                .group_hover(close_group, |s| s.text_color(rgb(0x242321)))))
                )),
        )
}

pub fn shell(content: Div, metrics: Metrics) -> impl IntoElement {
    shell_with_move(content, metrics, None)
}

type MoveHandler = dyn Fn(&MouseDownEvent, &mut Window, &mut App);

pub fn shell_with_move(
    content: Div,
    metrics: Metrics,
    on_move: Option<Box<MoveHandler>>,
) -> impl IntoElement {
    let client = metrics.client;
    let tiling = metrics.tiling;
    let inset = |tiled: bool| px(if client && !tiled { CSD_GUTTER } else { 0. });
    let floating = client && !tiling.top && !tiling.bottom && !tiling.left && !tiling.right;
    let round = move |d: Div| {
        d.when(!tiling.top && !tiling.left, |d| d.rounded_tl(px(CSD_ROUNDING)))
            .when(!tiling.top && !tiling.right, |d| d.rounded_tr(px(CSD_ROUNDING)))
            .when(!tiling.bottom && !tiling.left, |d| d.rounded_bl(px(CSD_ROUNDING)))
            .when(!tiling.bottom && !tiling.right, |d| d.rounded_br(px(CSD_ROUNDING)))
    };
    let drag = move |ev: &MouseDownEvent, window: &mut Window, cx: &mut App| {
        if let Some(on_move) = &on_move { on_move(ev, window, cx); }
        window.start_window_move();
    };
    div().size_full().relative().bg(rgba(0x00000000))
        .window_control_area(WindowControlArea::Drag)
        .on_mouse_down(MouseButton::Left, drag)
        .child(
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
        )
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui::{Context, FocusHandle, Focusable, Render, TestAppContext, VisualTestContext};

    use super::*;

    struct AuxSurface {
        escapes: Rc<Cell<usize>>,
        focus: FocusHandle,
    }

    impl Focusable for AuxSurface {
        fn focus_handle(&self, _: &App) -> FocusHandle { self.focus.clone() }
    }

    impl Render for AuxSurface {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let escapes = self.escapes.clone();
            div().key_context(KEY_CONTEXT).track_focus(&self.focus)
                .on_action(move |_: &crate::editor::EscapeMode, _, _| {
                    escapes.set(escapes.get() + 1);
                })
        }
    }

    #[gpui::test]
    fn escape_resolves_through_the_shared_context_for_both_aux_surfaces(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| cx.bind_keys(bindings()));
        for _window_kind in ["keymap", "about"] {
            let escapes = Rc::new(Cell::new(0));
            let window = cx.update({
                let escapes = escapes.clone();
                move |cx| cx.open_window(Default::default(), |window, cx| {
                    let surface = cx.new(|cx| AuxSurface {
                        escapes,
                        focus: cx.focus_handle(),
                    });
                    window.focus(&surface.focus_handle(cx), cx);
                    surface
                }).unwrap()
            });
            let mut visual = VisualTestContext::from_window(window.into(), cx);
            visual.update(|window, cx| {
                window.dispatch_keystroke(gpui::Keystroke::parse("escape").unwrap(), cx);
            });
            assert_eq!(escapes.get(), 1);
        }
    }

    #[test]
    fn toggle_grammar_is_one_shared_two_state_switch() {
        assert_eq!(toggle_decision(false), ToggleDecision::Open);
        assert_eq!(toggle_decision(true), ToggleDecision::CloseAndRestore);
    }

    struct MoveHookSurface {
        kicks: Rc<Cell<usize>>,
    }

    impl Render for MoveHookSurface {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let kicks = self.kicks.clone();
            // Mirrors the About window's drag-kick hook: mutate state, then
            // refresh(). The draw-phase sibling (request_animation_frame)
            // asserts outside layout/prepaint/paint and panicked debug
            // builds from exactly this dispatch path. The real shell can't
            // mount here — the test platform leaves start_window_move
            // unimplemented — so this pins the handler-context contract on
            // the same on_mouse_down dispatch the shell uses.
            div().size_full()
                .window_control_area(WindowControlArea::Drag)
                .on_mouse_down(MouseButton::Left, move |_, window, _| {
                    kicks.set(kicks.get() + 1);
                    window.refresh();
                })
        }
    }

    #[gpui::test]
    fn move_hook_shape_survives_real_mouse_dispatch(
        cx: &mut TestAppContext,
    ) {
        let kicks = Rc::new(Cell::new(0));
        let window = cx.update({
            let kicks = kicks.clone();
            move |cx| cx.open_window(Default::default(), |_, cx| {
                cx.new(|_| MoveHookSurface { kicks })
            }).unwrap()
        });
        let mut visual = VisualTestContext::from_window(window.into(), cx);
        visual.simulate_mouse_down(gpui::point(gpui::px(200.), gpui::px(200.)),
            gpui::MouseButton::Left, gpui::Modifiers::default());
        assert_eq!(kicks.get(), 1);
    }
}
