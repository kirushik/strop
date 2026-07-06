//! Headless-ish interaction smoke harness. Set `STROP_SMOKE` to a
//! space-separated keystroke script ("down down up ctrl-a") and the app will
//! replay it after the first frames, printing cursor geometry per key, then
//! quit. Drive with: `STROP_SMOKE="down up" cargo run -p strop-app`.

use std::sync::Mutex;
use std::time::Duration;

use gpui::{
    AnyWindowHandle, App, AppContext as _, ClipboardItem, Keystroke, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput, ScrollDelta, ScrollWheelEvent,
    TouchPhase, WindowHandle, point, px,
};

/// Clipboard TRANSPORT shim for the headless rig only: gpui's wayland
/// `write_to_clipboard` is silently dropped unless the window holds real
/// seat focus, which a headless sway with WLR_LIBINPUT_NO_DEVICES never
/// grants. The `clipb64:` token stores the text here too; the two paste
/// read-sites fall back to it when the platform clipboard reads empty.
/// Only the transport is shimmed — binding routing and insertion paths
/// stay fully real. Never set outside a STROP_SMOKE run.
static CLIP_OVERRIDE: Mutex<Option<String>> = Mutex::new(None);

pub fn clipboard_override() -> Option<String> {
    CLIP_OVERRIDE.lock().ok()?.clone()
}

/// Mirror an app-side clipboard WRITE into the shim so the rig can read it
/// back (the wayland write is dropped without real seat focus — see above).
/// A no-op outside STROP_SMOKE runs.
pub fn mirror_clipboard(text: &str) {
    if std::env::var("STROP_SMOKE").is_ok()
        && let Ok(mut slot) = CLIP_OVERRIDE.lock()
    {
        *slot = Some(text.to_owned());
    }
}

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
            // `clipb64:<base64>` seeds the clipboard (text); `wheel:X,Y,DY`
            // synthesizes a scroll-wheel event at window coords; `dump:ui`
            // prints the layer-stack snapshot (H1 — DESIGN §0.6 checks).
            if let Some(b64) = key.strip_prefix("clipb64:") {
                let text = decode_base64(b64).expect("bad clipb64 in STROP_SMOKE");
                *CLIP_OVERRIDE.lock().expect("clipboard override poisoned") = Some(text.clone());
                cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_string(text)));
                eprintln!("SMOKE {key}: clipboard set");
                continue;
            }
            if let Some(spec) = key.strip_prefix("wheel:") {
                let mut it = spec.split(',');
                let mut next = || {
                    it.next()
                        .and_then(|v| v.parse::<f32>().ok())
                        .expect("bad wheel in STROP_SMOKE")
                };
                let (x, y, dy) = (next(), next(), next());
                cx.update_window(any, |_, window, cx| {
                    window.dispatch_event(
                        PlatformInput::ScrollWheel(ScrollWheelEvent {
                            position: point(px(x), px(y)),
                            delta: ScrollDelta::Pixels(point(px(0.), px(dy))),
                            modifiers: Modifiers::default(),
                            touch_phase: TouchPhase::Moved,
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
            // `scraps:travel` — the travel verb (the chip / ctrl-shift-o):
            // arms the excursion latch and lands at the seam or `pile_end`.
            if key == "scraps:travel" {
                window
                    .update(cx, |editor, window, cx| {
                        editor.scraps_travel(&crate::editor::ScrapsTravel, window, cx)
                    })
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE scraps:travel");
                continue;
            }
            if key == "seed:diag" {
                window
                    .update(cx, |editor, _, cx| editor.debug_seed_notes(cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(120))
                    .await;
                eprintln!("SMOKE seed:diag: demo diagnosis cards seeded");
                continue;
            }
            // Asides (docs/impl/02-asides.md §6). `seed:aside` builds a doc
            // with a compost rail and a graveyard entry; `aside:selection` /
            // `exile:selection` run the verbs on the current selection;
            // `putback:last` restores the newest cut.
            if key == "seed:aside" {
                window
                    .update(cx, |editor, window, cx| editor.debug_seed_aside(window, cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(120))
                    .await;
                eprintln!("SMOKE seed:aside: compost rail + graveyard entry seeded");
                continue;
            }
            if key == "aside:selection" {
                window
                    .update(cx, |editor, window, cx| editor.debug_aside_selection(window, cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE aside:selection: selection moved to compost");
                continue;
            }
            // `seed:topera` rebuilds the live doc in the SHIPPED
            // compost-at-top shape and saves it — the NEXT launch of the
            // same file then exercises the one-time Scraps migration against
            // a real store (rig-check's migration section).
            if key == "seed:topera" {
                window
                    .update(cx, |editor, _, cx| editor.debug_seed_top_era(cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(120))
                    .await;
                eprintln!("SMOKE seed:topera: top-era compost file written");
                continue;
            }
            // `seed:demo` seeds the rich asides fixture for the VISUAL rig:
            // three compost items + sidebar + a full multi-paragraph grave entry
            // and a receded one (Bugs A & B in one frame).
            if key == "seed:demo" {
                window
                    .update(cx, |editor, _, cx| editor.debug_seed_demo(cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
                eprintln!("SMOKE seed:demo: compost items + graveyard section seeded");
                continue;
            }
            // `seed:annotated` seeds a paragraph carrying a writer note + a
            // diagnosis, selected — so a following `exile:selection` exercises
            // the dead-anchor reconcile (note migrates, diagnosis closes — Bug C).
            if key == "seed:annotated" {
                window
                    .update(cx, |editor, _, cx| editor.debug_seed_annotated(cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(120))
                    .await;
                eprintln!("SMOKE seed:annotated: annotated paragraph seeded + selected");
                continue;
            }
            // Flanks (docs/impl/03-flanks.md §3): select the caret paragraph and
            // raise the popover so `dump:ui`'s `flanks` object is observable.
            if key == "select:para" {
                window
                    .update(cx, |editor, _, cx| editor.debug_select_para(cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE select:para: caret paragraph selected + flanks raised");
                continue;
            }
            if key == "exile:selection" {
                window
                    .update(cx, |editor, window, cx| editor.debug_exile_selection(window, cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE exile:selection: selection filed in the graveyard");
                continue;
            }
            // `seed:mockup1|2|3` — the Gate-2 fidelity scenes, built
            // through the real verbs (park / typed scraps / exile).
            if let Some(n) = key.strip_prefix("seed:mockup") {
                let scene: u8 = n.parse().expect("bad seed:mockup scene");
                window
                    .update(cx, |editor, window, cx| {
                        editor.debug_seed_mockup(scene, window, cx)
                    })
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
                eprintln!("SMOKE {key}: sourdough scene seeded");
                continue;
            }
            // `move:manuscript` selects the caret's pile paragraph and runs
            // the retrieval verb; `putback:scrap` runs the provenance line's
            // Put back at the caret's record.
            if key == "move:manuscript" {
                window
                    .update(cx, |editor, window, cx| {
                        editor.debug_move_to_manuscript(window, cx)
                    })
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE move:manuscript: scrap moved home");
                continue;
            }
            if key == "putback:scrap" {
                window
                    .update(cx, |editor, window, cx| {
                        editor.debug_put_back_scrap(window, cx)
                    })
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE putback:scrap: scrap returned to origin");
                continue;
            }
            if key == "putback:last" {
                window
                    .update(cx, |editor, _, cx| editor.debug_putback_last(cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE putback:last: newest cut put back");
                continue;
            }
            if key == "seed:many" {
                window
                    .update(cx, |editor, _, cx| editor.debug_seed_many(cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(120))
                    .await;
                eprintln!("SMOKE seed:many: crowded lane seeded (8 diagnoses, 2 passes)");
                continue;
            }
            // `seed:deliver` pushes the demo pass through the REAL arrival
            // gate (reveal clock): mid-typing-burst it parks, in a lull it
            // lands — unlike seed:diag, which bypasses the gate on purpose.
            if key == "seed:deliver" {
                window
                    .update(cx, |editor, _, cx| editor.debug_deliver_pass(cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(120))
                    .await;
                eprintln!("SMOKE seed:deliver: demo pass sent through the arrival gate");
                continue;
            }
            // `ebtn:open` opens the editor button's dropdown; `ebtn:door`
            // flips the door through the menu footer's presence verb. Together
            // they let the rig assert the door law (cards rest while drafting
            // even with the menu open) and the face's transitions.
            if key == "ebtn:open" {
                window
                    .update(cx, |editor, _, cx| editor.debug_open_editor_menu(cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE ebtn:open: editor menu opened");
                continue;
            }
            if key == "ebtn:door" {
                window
                    .update(cx, |editor, _, cx| editor.debug_toggle_door(cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE ebtn:door: door toggled");
                continue;
            }
            // History strip (P1): `seed:journal` installs a synthetic fortnight;
            // `strip:open` opens the surface; `strip:scrub:<0..1>` /
            // `strip:pin:<0..1>` park/pin at a fraction of the whole history;
            // `strip:restore` / `strip:now` are the two exits from the past.
            if key == "seed:journal" {
                window
                    .update(cx, |editor, _, cx| editor.debug_seed_journal(cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE seed:journal: synthetic fortnight installed");
                continue;
            }
            // `seed:legacy` — the legacy litmus (Bug A): six materialized
            // checkpoints across two weeks, EMPTY journal. The strip's axis must
            // come from the checkpoint states, not the (absent) journal.
            if key == "seed:legacy" {
                window
                    .update(cx, |editor, _, cx| editor.debug_seed_legacy(cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE seed:legacy: legacy checkpoint history installed");
                continue;
            }
            if key == "strip:open" {
                window
                    .update(cx, |editor, window, cx| editor.debug_strip_open(window, cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(120))
                    .await;
                eprintln!("SMOKE strip:open");
                continue;
            }
            if let Some(frac) = key.strip_prefix("strip:scrub:") {
                let f: f32 = frac.parse().expect("bad strip:scrub fraction");
                window
                    .update(cx, |editor, _, cx| editor.debug_strip_scrub(f, cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE strip:scrub:{f}");
                continue;
            }
            if let Some(frac) = key.strip_prefix("strip:pin:") {
                let f: f32 = frac.parse().expect("bad strip:pin fraction");
                window
                    .update(cx, |editor, _, cx| editor.debug_strip_pin(f, cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE strip:pin:{f}");
                continue;
            }
            if key == "strip:restore" {
                window
                    .update(cx, |editor, _, cx| editor.debug_strip_restore(cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(120))
                    .await;
                eprintln!("SMOKE strip:restore");
                continue;
            }
            if key == "strip:now" {
                window
                    .update(cx, |editor, _, cx| editor.debug_strip_now(cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE strip:now");
                continue;
            }
            // The cold read (impl 05 Wave B). `coldread:open` toggles the
            // room through the real verb; `coldread:flip:N` flips to page N;
            // `coldread:select:F,T` sets a word-snapped page selection;
            // `coldread:react:<glyph>` files a chip reaction;
            // `coldread:past[:N]` enters the history variant through the
            // parked banner's own gate; `coldread:copycheck` runs the F5
            // copy golden through the clipboard shim.
            if key == "coldread:open" {
                window
                    .update(cx, |editor, window, cx| editor.debug_coldread_open(window, cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
                eprintln!("SMOKE coldread:open");
                continue;
            }
            if let Some(n) = key.strip_prefix("coldread:flip:") {
                let page: usize = n.parse().expect("bad coldread:flip page");
                window
                    .update(cx, |editor, _, cx| editor.debug_coldread_flip(page, cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE {key}");
                continue;
            }
            if let Some(spec) = key.strip_prefix("coldread:select:") {
                let mut it = spec.split(',');
                let mut next = || {
                    it.next()
                        .and_then(|v| v.parse::<usize>().ok())
                        .expect("bad coldread:select range")
                };
                let (from, to) = (next(), next());
                window
                    .update(cx, |editor, _, cx| editor.debug_coldread_select(from, to, cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE {key}");
                continue;
            }
            if let Some(glyph) = key.strip_prefix("coldread:react:") {
                let glyph = glyph.to_owned();
                window
                    .update(cx, |editor, _, cx| editor.debug_coldread_react(&glyph, cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE {key}");
                continue;
            }
            if key == "coldread:past" || key.starts_with("coldread:past:") {
                let pick = key
                    .strip_prefix("coldread:past:")
                    .and_then(|n| n.parse::<usize>().ok());
                window
                    .update(cx, |editor, window, cx| {
                        editor.debug_coldread_past(pick, window, cx)
                    })
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
                eprintln!("SMOKE {key}");
                continue;
            }
            if key == "coldread:copycheck" {
                window
                    .update(cx, |editor, window, cx| {
                        editor.debug_coldread_copycheck(window, cx)
                    })
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                continue;
            }
            // `reduce:motion` flips the config's motion-sensitivity switch
            // for this run, so the rig can drive the cross-fade code path.
            if key == "reduce:motion" {
                window
                    .update(cx, |editor, _, cx| {
                        editor.config.reduce_motion = true;
                        cx.notify();
                    })
                    .ok();
                eprintln!("SMOKE reduce:motion: cross-fade mode on");
                continue;
            }
            // `resolve:first` / `resolve:last` mark the oldest / newest open
            // note Done through the real set_note_status path (instant commit
            // + exit-fade ghost). `last` hits a full-size card in the seeded
            // crowded lane — the deterministic re-pack for the motion checks.
            if key == "resolve:first" || key == "resolve:last" {
                let first = key == "resolve:first";
                window
                    .update(cx, |editor, window, cx| {
                        if first {
                            editor.debug_resolve_first(window, cx);
                        } else {
                            editor.debug_resolve_last(window, cx);
                        }
                    })
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(30))
                    .await;
                eprintln!("SMOKE {key}: open note resolved");
                continue;
            }
            // `wait:MS` — idle the script (the reveal clock's lull, status
            // fades, animations) without faking any input.
            if let Some(ms) = key.strip_prefix("wait:") {
                let ms: u64 = ms.parse().expect("bad wait ms in STROP_SMOKE");
                cx.background_executor()
                    .timer(Duration::from_millis(ms))
                    .await;
                eprintln!("SMOKE wait:{ms}");
                continue;
            }
            if key == "dump:ui" {
                let dump = window
                    .update(cx, |editor, window, cx| editor.debug_ui_dump(window, cx))
                    .unwrap_or_default();
                println!("UI-DUMP: {dump}");
                continue;
            }
            // `click:X,Y` or `click:X,Y,N` — a left click with click_count N
            // (N=2 double, N=3 triple), through the real GPUI dispatch path.
            if let Some(spec) = key.strip_prefix("click:") {
                let mut it = spec.split(',');
                let x = it.next().and_then(|v| v.parse::<f32>().ok()).expect("bad click x");
                let y = it.next().and_then(|v| v.parse::<f32>().ok()).expect("bad click y");
                let count = it.next().and_then(|v| v.parse::<usize>().ok()).unwrap_or(1);
                let position = point(px(x), px(y));
                cx.update_window(any, |_, window, cx| {
                    window.dispatch_event(
                        PlatformInput::MouseDown(MouseDownEvent {
                            button: MouseButton::Left,
                            position,
                            modifiers: Modifiers::default(),
                            click_count: count,
                            first_mouse: false,
                        }),
                        cx,
                    );
                    window.dispatch_event(
                        PlatformInput::MouseUp(MouseUpEvent {
                            button: MouseButton::Left,
                            position,
                            modifiers: Modifiers::default(),
                            click_count: count,
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
            // `drag:X1,Y1,X2,Y2` — press at the start, move in steps to the end,
            // release: the canonical click-drag selection gesture. Each move
            // carries the pressed button so drag-tracking sees a real drag.
            if let Some(spec) = key.strip_prefix("drag:") {
                let mut it = spec.split(',');
                let mut next = || {
                    it.next()
                        .and_then(|v| v.parse::<f32>().ok())
                        .expect("bad drag in STROP_SMOKE")
                };
                let (x1, y1, x2, y2) = (next(), next(), next(), next());
                let start = point(px(x1), px(y1));
                let end = point(px(x2), px(y2));
                cx.update_window(any, |_, window, cx| {
                    window.dispatch_event(
                        PlatformInput::MouseDown(MouseDownEvent {
                            button: MouseButton::Left,
                            position: start,
                            modifiers: Modifiers::default(),
                            click_count: 1,
                            first_mouse: false,
                        }),
                        cx,
                    );
                    // A handful of intermediate moves so drag-extend tracks it.
                    for i in 1..=4 {
                        let t = i as f32 / 4.0;
                        let pos = point(px(x1 + (x2 - x1) * t), px(y1 + (y2 - y1) * t));
                        window.dispatch_event(
                            PlatformInput::MouseMove(MouseMoveEvent {
                                position: pos,
                                pressed_button: Some(MouseButton::Left),
                                modifiers: Modifiers::default(),
                            }),
                            cx,
                        );
                    }
                    window.dispatch_event(
                        PlatformInput::MouseUp(MouseUpEvent {
                            button: MouseButton::Left,
                            position: end,
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

/// Standard-alphabet base64 (RFC 4648), padding optional. Tiny on purpose:
/// the harness shouldn't pull a dependency for one smoke token.
fn decode_base64(s: &str) -> Option<String> {
    let val = |c: u8| -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some(u32::from(c - b'A')),
            b'a'..=b'z' => Some(u32::from(c - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(c - b'0') + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let mut acc: u32 = 0;
    let mut nbits = 0u32;
    let mut out = Vec::new();
    for c in s.bytes() {
        if c == b'=' {
            break;
        }
        acc = (acc << 6) | val(c)?;
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            out.push((acc >> nbits) as u8);
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::decode_base64;

    #[test]
    fn base64_decodes_standard_strings() {
        assert_eq!(decode_base64("c2stdGVzdC0xMjM0").as_deref(), Some("sk-test-1234"));
        assert_eq!(decode_base64("YQ==").as_deref(), Some("a"));
        assert_eq!(decode_base64("YWI=").as_deref(), Some("ab"));
        assert_eq!(decode_base64("YWJj").as_deref(), Some("abc"));
        assert_eq!(decode_base64("").as_deref(), Some(""));
        assert_eq!(decode_base64("!!"), None);
    }
}
