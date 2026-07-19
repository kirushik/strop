//! Shared exterior for modeless auxiliary windows.  P1 requires these
//! sheets to remain visibly distinct from the writer's paper; keeping the
//! decoration response here prevents each auxiliary surface from inventing
//! a subtly different border, shadow, or close affordance.

use gpui::{
    App, BoxShadow, CursorStyle, Decorations, Div, Hsla, IntoElement, MouseButton,
    MouseDownEvent, Tiling, Window, WindowBackgroundAppearance, WindowControlArea,
    WindowDecorations, WindowOptions, div, px, rgb, rgba,
};
use gpui::prelude::*;

use crate::icons::{self, icon};

pub const HEADER_HEIGHT: f32 = 68.;
pub const CSD_GUTTER: f32 = 22.;
const CSD_ROUNDING: f32 = 10.;

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
                    div().id(close_id).occlude().ml(px(14.)).w(px(28.)).h_full()
                        .flex().items_center().justify_center().cursor(CursorStyle::PointingHand)
                        .group(close_group).hover(|d| d.bg(rgba(0x1A1A180A)))
                        .on_mouse_down(MouseButton::Left, close)
                        .child(icon(icons::WIN_CLOSE, 13., 0x716D66)
                            .group_hover(close_group, |s| s.text_color(rgb(0x242321))))
                )),
        )
}

pub fn shell(content: Div, metrics: Metrics) -> impl IntoElement {
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
    let drag = |_: &MouseDownEvent, window: &mut Window, _: &mut App| {
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
