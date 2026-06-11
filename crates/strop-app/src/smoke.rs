//! Headless-ish interaction smoke harness. Set `STROP_SMOKE` to a
//! space-separated keystroke script ("down down up ctrl-a") and the app will
//! replay it after the first frames, printing cursor geometry per key, then
//! quit. Drive with: `STROP_SMOKE="down up" cargo run -p strop-app`.

use std::time::Duration;

use gpui::{
    AnyWindowHandle, App, AppContext as _, Keystroke, Modifiers, MouseButton, MouseDownEvent,
    MouseUpEvent, PlatformInput, WindowHandle, point, px,
};

use crate::editor::Editor;

pub fn maybe_run(window: WindowHandle<Editor>, cx: &mut App) {
    let Ok(script) = std::env::var("STROP_SMOKE") else {
        return;
    };
    let any: AnyWindowHandle = window.into();
    cx.spawn(async move |cx| {
        // Let the first frames paint so the editor has layout geometry.
        cx.background_executor()
            .timer(Duration::from_millis(800))
            .await;

        let state = window
            .update(cx, |editor, _, _| editor.debug_cursor())
            .unwrap_or_default();
        eprintln!("SMOKE start: {state}");

        for key in script.split_whitespace() {
            // `fn-geo` prints footnote click targets; `click:X,Y` synthesizes
            // a full left click (window coords) through GPUI dispatch — the
            // same path real mouse input takes, div listeners included.
            if key == "fn-geo" {
                let geo = window
                    .update(cx, |editor, _, _| editor.debug_footnotes())
                    .unwrap_or_default();
                eprintln!("SMOKE fn-geo:\n{geo}");
                continue;
            }
            if let Some(pos) = key.strip_prefix("click:") {
                let (x, y) = pos.split_once(',').expect("bad click in STROP_SMOKE");
                let position = point(
                    px(x.parse::<f32>().expect("bad click x")),
                    px(y.parse::<f32>().expect("bad click y")),
                );
                cx.update_window(any, |_, window, cx| {
                    window.dispatch_event(
                        PlatformInput::MouseDown(MouseDownEvent {
                            button: MouseButton::Left,
                            position,
                            modifiers: Modifiers::default(),
                            click_count: 1,
                            first_mouse: false,
                        }),
                        cx,
                    );
                    window.dispatch_event(
                        PlatformInput::MouseUp(MouseUpEvent {
                            button: MouseButton::Left,
                            position,
                            modifiers: Modifiers::default(),
                            click_count: 1,
                        }),
                        cx,
                    );
                })
                .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                let state = window
                    .update(cx, |editor, _, _| editor.debug_cursor())
                    .unwrap_or_default();
                eprintln!("SMOKE {key}: {state}");
                continue;
            }
            let keystroke = Keystroke::parse(key).expect("bad keystroke in STROP_SMOKE");
            cx.update_window(any, |_, window, cx| {
                window.dispatch_keystroke(keystroke, cx);
            })
            .ok();
            // Give the frame a chance to repaint (layout state refreshes).
            cx.background_executor()
                .timer(Duration::from_millis(80))
                .await;
            let state = window
                .update(cx, |editor, _, _| editor.debug_cursor())
                .unwrap_or_default();
            eprintln!("SMOKE {key}: {state}");
        }
        // STROP_SMOKE_HOLD keeps the window alive after the script — the
        // visual rig screenshots it from outside, then kills the process.
        if std::env::var("STROP_SMOKE_HOLD").is_err() {
            // AsyncApp::update is now infallible (returns R, not Result).
            cx.update(|cx| cx.quit());
        } else {
            eprintln!("SMOKE HOLD");
        }
    })
    .detach();
}
