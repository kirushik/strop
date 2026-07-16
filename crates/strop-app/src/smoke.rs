//! Headless-ish interaction smoke harness. Set `STROP_SMOKE` to a
//! space-separated keystroke script ("down down up ctrl-a") and the app will
//! replay it after the first frames, printing cursor geometry per key, then
//! quit. Drive with: `STROP_SMOKE="down up" cargo run -p strop-app`.

use std::sync::Mutex;
use std::time::Duration;

use gpui::{
    AnyWindowHandle, App, AppContext as _, ClipboardItem, ExternalPaths, FileDropEvent, Keystroke,
    Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput,
    ScrollDelta, ScrollWheelEvent, TouchPhase, WindowHandle, point, px,
};

/// Clipboard TRANSPORT shim for the headless rig only: gpui's wayland
/// `write_to_clipboard` is silently dropped unless the window holds real
/// seat focus, which a headless sway with WLR_LIBINPUT_NO_DEVICES never
/// grants. The `clipb64:` token stores here too; the paste read-sites
/// fall back to it when the platform clipboard reads empty. The slot
/// holds a whole `ClipboardItem` so §9's two-entry image form (Markdown
/// line + bitmap) round-trips under the rig, not just text. Only the
/// transport is shimmed — binding routing and insertion paths stay fully
/// real. Never set outside a STROP_SMOKE run.
static CLIP_OVERRIDE: Mutex<Option<ClipboardItem>> = Mutex::new(None);

/// The text view of the shim (coldread copycheck compares text goldens).
pub fn clipboard_override() -> Option<String> {
    CLIP_OVERRIDE.lock().ok()?.clone()?.text()
}

/// The full-item view (the document paste path reads entries).
pub fn clipboard_item_override() -> Option<ClipboardItem> {
    CLIP_OVERRIDE.lock().ok()?.clone()
}

/// Mirror an app-side clipboard WRITE into the shim so the rig can read it
/// back (the wayland write is dropped without real seat focus — see above).
/// A no-op outside STROP_SMOKE runs.
pub fn mirror_clipboard(text: &str) {
    mirror_clipboard_item(&ClipboardItem::new_string(text.to_owned()));
}

/// `mirror_clipboard` for a multi-entry item (§9's image copy/cut).
pub fn mirror_clipboard_item(item: &ClipboardItem) {
    if std::env::var("STROP_SMOKE").is_ok()
        && let Ok(mut slot) = CLIP_OVERRIDE.lock()
    {
        *slot = Some(item.clone());
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
                *CLIP_OVERRIDE.lock().expect("clipboard override poisoned") =
                    Some(ClipboardItem::new_string(text.clone()));
                cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_string(text)));
                eprintln!("SMOKE {key}: clipboard set");
                continue;
            }
            // `clipimg:PATH` seeds the clipboard with a BARE bitmap entry —
            // a foreign clipboard in §9's terms (no Strop image line) — for
            // driving the §5b fallback and §4's bitmap replace-in-place.
            // `clipimg:PATH;B64MD` adds a base64 Markdown line as the text
            // sibling — the §9 two-entry form with a non-resolving asset:
            // src, for driving the ladder's import-sibling rung.
            if let Some(spec) = key.strip_prefix("clipimg:") {
                let (path, md) = match spec.split_once(';') {
                    Some((p, b64)) => {
                        (p, Some(decode_base64(b64).expect("bad clipimg md in STROP_SMOKE")))
                    }
                    None => (spec, None),
                };
                let bytes = std::fs::read(path).expect("bad clipimg path in STROP_SMOKE");
                let mut entries = Vec::new();
                if let Some(md) = md {
                    entries.push(gpui::ClipboardEntry::String(gpui::ClipboardString::new(md)));
                }
                entries.push(gpui::ClipboardEntry::Image(gpui::Image::from_bytes(
                    gpui::ImageFormat::Png,
                    bytes,
                )));
                let item = ClipboardItem { entries };
                *CLIP_OVERRIDE.lock().expect("clipboard override poisoned") = Some(item.clone());
                cx.update(|cx| cx.write_to_clipboard(item));
                eprintln!("SMOKE {key}: bitmap clipboard set");
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
            // Flanks (docs/impl/03-flanks.md §3, papercuts-2026-07 §1 LAW 1):
            // `select:para` raises the flanks via a settled selection so
            // `dump:ui`'s `flanks` object is observable; `select:kbd` proves a
            // KEYBOARD selection (shift+arrows, through `extend_cursor`) raises
            // them identically. Follow either with `ctrl-m dump:ui` to prove the
            // composer takes them DOWN (transient field ⇒ hidden, C2).
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
            if key == "select:kbd" {
                window
                    .update(cx, |editor, _, cx| editor.debug_select_kbd(cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE select:kbd: keyboard selection made + flanks raised");
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
            // Inline images (docs/inline-images.md §11): `seed:image`
            // builds a doc with a captioned picture, prose, and a tall
            // uncaptioned portrait — through the real verbs (put_asset,
            // insert_image_block, typing into the caption line) — so the
            // rig can finally SEE pictures: caption under (never on),
            // empty slot chrome-free, the two-thirds-viewport cap.
            if key == "seed:image" {
                window
                    .update(cx, |editor, _, cx| editor.debug_seed_image(cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
                eprintln!("SMOKE seed:image: captioned + tall pictures seeded");
                continue;
            }
            // `seed:imgrepro` builds the round's FIELD REPRO exactly (empty
            // paragraph, captioned picture, prose below — inline-images
            // §11's acceptance shape); `img-geo` prints every picture's
            // pixel-rect + caption targets (window coords) for `click:`.
            if key == "seed:imgrepro" {
                window
                    .update(cx, |editor, _, cx| editor.debug_seed_image_repro(cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
                eprintln!("SMOKE seed:imgrepro: empty para + picture + prose seeded");
                continue;
            }
            if key == "img-geo" {
                let geo = window
                    .update(cx, |editor, _, _| editor.debug_images())
                    .unwrap_or_default();
                eprintln!("SMOKE img-geo:\n{geo}");
                continue;
            }
            // `keyup:<key>` dispatches a real KeyUp through GPUI (the same
            // path a physical release takes). dispatch_keystroke sends only
            // KeyDown, so without this token the staged-exile freshness law
            // (inline-images §5, R5: completion waits for the staging key's
            // release) would refuse every scripted second press — exactly
            // as it must for a real held key.
            if let Some(k) = key.strip_prefix("keyup:") {
                let keystroke = Keystroke::parse(k).expect("bad keyup in STROP_SMOKE");
                cx.update_window(any, |_, window, cx| {
                    window.dispatch_event(
                        PlatformInput::KeyUp(gpui::KeyUpEvent { keystroke }),
                        cx,
                    );
                })
                .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE {key}");
                continue;
            }
            // External file drag-and-drop (inline-images §7), synthesized
            // as the PLATFORM events the compositor would send — the same
            // FileDropEvent path gpui's window.rs:4602 translates, so the
            // active_drag plumbing, the drag-over rule and the drop
            // listeners all run for real. `dragenter:X,Y,PATH[;PATH…]`
            // starts the drag over the window; `dragmove:X,Y` is a
            // Pending hover (paints the §7 insertion rule); `dragdrop:X,Y`
            // submits the drop at that position; `dragleave` exits.
            if let Some(spec) = key.strip_prefix("dragenter:") {
                let mut it = spec.splitn(3, ',');
                let x: f32 = it.next().and_then(|v| v.parse().ok()).expect("bad dragenter x");
                let y: f32 = it.next().and_then(|v| v.parse().ok()).expect("bad dragenter y");
                let paths: Vec<std::path::PathBuf> = it
                    .next()
                    .expect("dragenter needs a path")
                    .split(';')
                    .map(std::path::PathBuf::from)
                    .collect();
                cx.update_window(any, |_, window, cx| {
                    window.dispatch_event(
                        PlatformInput::FileDrop(FileDropEvent::Entered {
                            position: point(px(x), px(y)),
                            paths: ExternalPaths(paths.into_iter().collect()),
                        }),
                        cx,
                    );
                })
                .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE {key}");
                continue;
            }
            if let Some(spec) = key.strip_prefix("dragmove:") {
                let mut it = spec.split(',');
                let mut next = || {
                    it.next()
                        .and_then(|v| v.parse::<f32>().ok())
                        .expect("bad dragmove in STROP_SMOKE")
                };
                let (x, y) = (next(), next());
                cx.update_window(any, |_, window, cx| {
                    window.dispatch_event(
                        PlatformInput::FileDrop(FileDropEvent::Pending {
                            position: point(px(x), px(y)),
                        }),
                        cx,
                    );
                })
                .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE {key}");
                continue;
            }
            if let Some(spec) = key.strip_prefix("dragdrop:") {
                let mut it = spec.split(',');
                let mut next = || {
                    it.next()
                        .and_then(|v| v.parse::<f32>().ok())
                        .expect("bad dragdrop in STROP_SMOKE")
                };
                let (x, y) = (next(), next());
                cx.update_window(any, |_, window, cx| {
                    window.dispatch_event(
                        PlatformInput::FileDrop(FileDropEvent::Submit {
                            position: point(px(x), px(y)),
                        }),
                        cx,
                    );
                })
                .ok();
                // The drop's import runs on the background executor.
                cx.background_executor()
                    .timer(Duration::from_millis(300))
                    .await;
                let state = window
                    .update(cx, |editor, _, _| editor.debug_cursor())
                    .unwrap_or_default();
                eprintln!("SMOKE {key}: {state}");
                continue;
            }
            if key == "dragleave" {
                cx.update_window(any, |_, window, cx| {
                    window.dispatch_event(PlatformInput::FileDrop(FileDropEvent::Exited), cx);
                })
                .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE {key}");
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
            // exercises the same transition as the fixed margin chip.
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
            if key == "ebtn:close" {
                window
                    .update(cx, |editor, _, cx| editor.debug_close_editor_menu(cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE ebtn:close: editor menu closed");
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
            if key == "notes:glance" {
                window
                    .update(cx, |editor, window, cx| editor.debug_glance_first(window, cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE notes:glance: first diagnosis lifted");
                continue;
            }
            if key == "notes:drain" {
                window
                    .update(cx, |editor, _, cx| editor.debug_drain_diagnoses(cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE notes:drain: diagnoses marked unverified");
                continue;
            }
            if key == "ai:empty" {
                window.update(cx, |editor, _, cx| editor.debug_ai_empty(cx)).ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE ai:empty: valid zero-query result seeded");
                continue;
            }
            if key == "ai:running" {
                window
                    .update(cx, |editor, _, cx| editor.debug_ai_running(cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE ai:running: active read seeded");
                continue;
            }
            if key == "ai:error" {
                window.update(cx, |editor, _, cx| editor.debug_ai_error(cx)).ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE ai:error: retryable failure seeded");
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
            if key == "seed:cards" {
                window
                    .update(cx, |editor, _, cx| editor.debug_seed_cards(cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE seed:cards: card-history fortnight installed");
                continue;
            }
            if key == "seed:novel" {
                window
                    .update(cx, |editor, _, cx| editor.debug_seed_novel(cx))
                    .ok();
                cx.background_executor().timer(Duration::from_millis(120)).await;
                eprintln!("SMOKE seed:novel: round-two long fixture installed");
                continue;
            }
            if let Some(mode) = key.strip_prefix("seed:scrollbar-") {
                window
                    .update(cx, |editor, _, cx| editor.debug_seed_scrollbar(mode, cx))
                    .ok();
                cx.background_executor().timer(Duration::from_millis(120)).await;
                eprintln!("SMOKE seed:scrollbar-{mode}: rail fixture installed");
                continue;
            }
            if let Some(fraction) = key.strip_prefix("rail:drag:") {
                let fraction: f32 = fraction.parse().expect("rail:drag fraction");
                window
                    .update(cx, |editor, _, cx| editor.debug_space_drag(fraction, cx))
                    .ok();
                cx.background_executor().timer(Duration::from_millis(80)).await;
                eprintln!("SMOKE rail:drag:{fraction}: readout held live");
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
            if let Some(label) = key.strip_prefix("strip:station:") {
                let label = label.replace('_', " ");
                window.update(cx, |editor, _, cx| editor.debug_strip_station(&label, cx)).ok();
                cx.background_executor().timer(Duration::from_millis(80)).await;
                eprintln!("SMOKE strip:station:{label}");
                continue;
            }
            // `compare:begin` pins the PARKED moment as A through the real
            // verb path (strip_begin_compare) — the interactive-parity route
            // for the rig: fraction-mapped pins proved treacherous at the
            // edges of a mixed checkpoint+journal axis, while a parked
            // station's preview is always a materialized truth.
            if key == "compare:begin" {
                window.update(cx, |editor, _, cx| editor.debug_compare_begin(cx)).ok();
                cx.background_executor().timer(Duration::from_millis(80)).await;
                eprintln!("SMOKE compare:begin");
                continue;
            }
            if key == "compare:page:a" || key == "compare:page:b" {
                let side_b = key.ends_with(":b");
                window.update(cx, |editor, _, cx| editor.debug_compare_page(side_b, cx)).ok();
                cx.background_executor().timer(Duration::from_millis(80)).await;
                eprintln!("SMOKE {key}");
                continue;
            }
            if let Some(frac) = key.strip_prefix("scroll:") {
                let f: f32 = frac.parse().expect("bad scroll fraction");
                window.update(cx, |editor, _, cx| editor.debug_scroll_fraction(f, cx)).ok();
                cx.background_executor().timer(Duration::from_millis(80)).await;
                eprintln!("SMOKE scroll:{f}");
                continue;
            }
            if key == "strip:thread:first" {
                window.update(cx, |editor, _, cx| editor.debug_strip_thread(cx)).ok();
                cx.background_executor().timer(Duration::from_millis(80)).await;
                eprintln!("SMOKE strip:thread:first");
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
            // `coldread:raise` opens the reaction input on the current
            // selection (drive the D1 multi-word regression by following it
            // with real keystroke tokens, e.g. `h i space t h e r e enter`).
            if key == "coldread:raise" {
                window
                    .update(cx, |editor, window, cx| editor.debug_coldread_raise(window, cx))
                    .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE {key}");
                continue;
            }
            // `coldread:pageclick:Z` — a real page click in flip zone Z (-1/0/1)
            // through the actual mouse handlers (D1's commit-only carve-out).
            if let Some(z) = key.strip_prefix("coldread:pageclick:") {
                let zone: i8 = z.parse().expect("bad coldread:pageclick zone");
                window
                    .update(cx, |editor, window, cx| {
                        editor.debug_coldread_pageclick(zone, window, cx)
                    })
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
            // `move:X,Y` — a plain pointer move (no button): hover states
            // (the cold read's flip-zone shading, tooltips-by-position).
            if let Some(spec) = key.strip_prefix("move:") {
                let mut it = spec.split(',');
                let mut next = || {
                    it.next()
                        .and_then(|v| v.parse::<f32>().ok())
                        .expect("bad move in STROP_SMOKE")
                };
                let (x, y) = (next(), next());
                cx.update_window(any, |_, window, cx| {
                    window.dispatch_event(
                        PlatformInput::MouseMove(MouseMoveEvent {
                            position: point(px(x), px(y)),
                            pressed_button: None,
                            modifiers: Modifiers::default(),
                        }),
                        cx,
                    );
                })
                .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                eprintln!("SMOKE {key}");
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
