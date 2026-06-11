//! Headless-ish interaction smoke harness. Set `STROP_SMOKE` to a
//! space-separated keystroke script ("down down up ctrl-a") and the app will
//! replay it after the first frames, printing cursor geometry per key, then
//! quit. Drive with: `STROP_SMOKE="down up" cargo run -p strop-app`.

use std::time::Duration;

use gpui::{AnyWindowHandle, App, AppContext as _, Keystroke, WindowHandle};

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
        // AsyncApp::update is now infallible (returns R, not Result).
        cx.update(|cx| cx.quit());
    })
    .detach();
}
