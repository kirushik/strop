// Windows attaches a console to a console-subsystem binary launched from
// Explorer — for a GUI app that is a spare black window next to ours. The
// "windows" subsystem suppresses it. Gated to release builds so `cargo run`
// and debug builds keep their console (eprintln!/panic output go nowhere
// under the windows subsystem). The `target_os = "windows"` arm is belt-and-
// braces — the attribute is already ignored on non-Windows targets — and just
// keeps it scoped to where it means something.
#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]
// Draw-pass discipline (docs/VISUAL-RIG.md): raw Entity::update and canvas
// are banned crate-wide; clippy.toml points each ban at its draw_guard
// wrapper. Deny, not warn — a mid-draw notify is a corruption bug, not style.
#![deny(clippy::disallowed_methods)]
// The rig's UI dump is one large serde_json::json! literal (editor.rs,
// debug_ui_dump); json_internal! recurses per key and the Scraps keys pushed
// it past the default 128. A wider budget beats splitting the dump.
#![recursion_limit = "256"]

mod commands;
mod config;
mod draw_guard;
mod editor;
mod files;
mod paths;
mod single_instance;
mod smoke;
mod strip;
mod text_field;
mod theme;
mod tutorial;

use std::path::PathBuf;
use std::time::Duration;

use gpui::{
    App, Bounds, Focusable, KeyBinding, TitlebarOptions, WindowBackgroundAppearance, WindowBounds,
    WindowDecorations, WindowOptions, actions, prelude::*, px, size,
};
use strop_core::Store;
use strop_core::document::{BlockMap, SpanSet};

use draw_guard::EntityUpdateExt as _;
use editor::Editor;

actions!(strop, [Quit]);

// The PT superfamily (ParaType, OFL): serif body, sans headings, mono code —
// drawn as independent fonts with the four canonical styles per family, and
// metrically harmonized with full Cyrillic. Replaced Literata after its
// variable-font-derived statics ("Literata" / "Literata SemiBold" /
// "Literata 36pt") showed migrating glyph corruption in GPUI's shaping/atlas
// path; the document bytes were proven clean, so the fonts were the variable
// under test.
const PT_SERIF: &[u8] = include_bytes!("../../../assets/fonts/PTSerif-Regular.ttf");
const PT_SERIF_ITALIC: &[u8] = include_bytes!("../../../assets/fonts/PTSerif-Italic.ttf");
const PT_SERIF_BOLD: &[u8] = include_bytes!("../../../assets/fonts/PTSerif-Bold.ttf");
const PT_SERIF_BOLD_ITALIC: &[u8] = include_bytes!("../../../assets/fonts/PTSerif-BoldItalic.ttf");
const PT_SANS: &[u8] = include_bytes!("../../../assets/fonts/PTSans-Regular.ttf");
const PT_SANS_ITALIC: &[u8] = include_bytes!("../../../assets/fonts/PTSans-Italic.ttf");
const PT_SANS_BOLD: &[u8] = include_bytes!("../../../assets/fonts/PTSans-Bold.ttf");
const PT_SANS_BOLD_ITALIC: &[u8] = include_bytes!("../../../assets/fonts/PTSans-BoldItalic.ttf");
const PT_MONO: &[u8] = include_bytes!("../../../assets/fonts/PTMono-Regular.ttf");

const SAMPLE: &str = "Strop is a writer’s editor with an editor inside — one that diagnoses, and never rewrites you into the average.\n\
Хороший редактор называет проблему — «здесь зарыта мысль», «начало хоронит главное» — и оставляет решение автору. Тире, кавычки-ёлочки и неразрывные пробелы должны просто работать: 1941—1945, «так», “so”.\n\
The reader is right that something is wrong, and almost always wrong about how to fix it.\n\
Gordon Lish cut Carver’s stories by half and more; Maxwell Perkins cut in service of the author’s own intent. Between those poles lies every editorial decision this tool will ever surface — and the dial belongs to the writer, not to the model.\n\
Гомогенизация голоса — не свойство модели, а свойство интерфейса: автодополнение в строке съедает авторство по одному слову за раз. Поэтому здесь его нет и не будет.\n\
A diagnostic margin note names the problem and stops. “The lede is buried in the third paragraph.” “This hedge weakens the claim you just spent four sentences earning.” The fix is yours.\n\
Уильямс учит связности — старое перед новым, переходы как мосты. Клинкенборг отвечает: предложение стоит само по себе, а мосты чаще всего — строительный мусор. Инструмент не должен выбирать за автора эту веру.\n\
The voice-distance metric is the regression test for the whole thesis: an edit that drags your surprisal signature toward the mean is a bug, not a suggestion.\n\
Перо знает о бумаге больше, чем писатель о читателе; редактор — тот, кто читал за обоих.\n\
And somewhere past the tenth paragraph, the window must scroll — which is, frankly, the only reason this sentence exists.";

/// Remembered window bounds, in the per-user state dir (see `paths`).
fn state_file() -> PathBuf {
    paths::state_dir().join("window.json")
}

fn load_bounds() -> Option<(f32, f32, f32, f32)> {
    let json = std::fs::read_to_string(state_file()).ok()?;
    serde_json::from_str(&json).ok()
}

fn save_bounds(b: (f32, f32, f32, f32)) {
    let path = state_file();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string(&b) {
        let _ = std::fs::write(path, json);
    }
}

/// `strop [file.strop|file.md|--new|--welcome]`. With no argument:
/// migrate the legacy hidden scratch if present, else reopen the most
/// recent document, else the first run ever gets the tutorial (PLAN.md
/// E2/E4 — documents are never created in hidden locations). The bool
/// marks "seed this as the welcome tutorial".
fn data_file() -> (PathBuf, bool) {
    match std::env::args().nth(1).as_deref() {
        Some("--new") => return (files::untitled_path(), false),
        Some("--welcome") => return (files::welcome_path(), true),
        Some(arg) => return (arg.into(), false),
        None => {}
    }
    if let Some(migrated) = files::migrate_scratch() {
        return (migrated, false);
    }
    if let Some(recent) = files::recents().into_iter().next() {
        return (recent, false);
    }
    (files::welcome_path(), true)
}

fn main() {
    // gpui_platform::application() replaced gpui::Application::new() after
    // the facade/platform crate split.
    gpui_platform::application().run(|cx: &mut App| {
        cx.text_system()
            .add_fonts(vec![
                PT_SERIF.into(),
                PT_SERIF_ITALIC.into(),
                PT_SERIF_BOLD.into(),
                PT_SERIF_BOLD_ITALIC.into(),
                PT_SANS.into(),
                PT_SANS_ITALIC.into(),
                PT_SANS_BOLD.into(),
                PT_SANS_BOLD_ITALIC.into(),
                PT_MONO.into(),
            ])
            .expect("failed to load bundled fonts");

        editor::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("ctrl-q", Quit, None)]);
        cx.on_action(|_: &Quit, cx| cx.quit());

        // Smoke runs must not steal the user's OS focus — keystroke dispatch
        // uses GPUI's internal focus, which we set explicitly below. They
        // also never touch the user's real document: no store unless a file
        // was passed explicitly (which lets smoke scripts test persistence).
        let smoke = std::env::var("STROP_SMOKE").is_ok();
        // Resolve the document path exactly once: data_file() has side
        // effects (scratch migration) that smoke runs must never trigger.
        let (doc_path, welcome): (Option<PathBuf>, bool) =
            if smoke && std::env::args().nth(1).is_none() {
                (None, false)
            } else {
                let (p, welcome) = data_file();
                (Some(p), welcome)
            };
        let mut instance_guard: Option<single_instance::InstanceGuard> = None;
        let store = match &doc_path {
            None => None,
            Some(p) => {
                let store_path = if p.extension().is_some_and(|e| e == "md") {
                    p.with_extension("strop")
                } else {
                    p.clone()
                };
                // One writer per document: if a live instance already holds
                // this file, ask it to surface and exit BEFORE we open (and
                // mutate) the Loro store — two writers on one store is the
                // multiplayer-persistence hole this closes. Liveness is the
                // rendezvous socket, so a crashed instance never locks the
                // file (single_instance.rs). Smoke runs never claim.
                if !smoke {
                    match single_instance::claim(&store_path) {
                        Ok(single_instance::Claim::AlreadyOpen) => {
                            eprintln!(
                                "strop: “{}” is already open — raising that window",
                                store_path.display()
                            );
                            // Nothing opened, nothing to flush: leave the
                            // primary untouched and end this process.
                            std::process::exit(0);
                        }
                        Ok(single_instance::Claim::Primary(guard)) => {
                            instance_guard = Some(guard);
                        }
                        Err(e) => eprintln!("strop: single-instance check failed: {e}"),
                    }
                }
                match Store::open(store_path) {
                    Ok(opened) => {
                        if !smoke {
                            files::push_recent(opened.0.path());
                        }
                        Some(opened)
                    }
                    Err(e) => {
                        eprintln!("strop: cannot open {}: {e}", p.display());
                        None
                    }
                }
            }
        };
        // Opening a .md imports it into a sibling .strop (existing .strop
        // wins — the durable file is the source of truth once it exists).
        let md_import: Option<(String, SpanSet, BlockMap)> = doc_path.as_ref().and_then(|arg| {
            if arg.extension().is_some_and(|e| e == "md") && !arg.with_extension("strop").exists()
            {
                std::fs::read_to_string(arg).ok().map(|md| {
                    let (text, spans, blocks) = strop_core::markdown::from_markdown(&md);
                    (text, spans, blocks)
                })
            } else {
                None
            }
        });

        let mut tutorial_notes = None;
        let (initial_text, initial_spans, initial_blocks, initial_history) = match &store {
            Some((store, existing)) => match existing {
                Some(loaded) => {
                    // "Undo everything since I sat down" is always one
                    // visible restore away. Seal with the state `open` just
                    // produced — never re-derive it from the doc.
                    store.seal_session_with(
                        "Session start",
                        false,
                        (loaded.text.clone(), loaded.spans.clone(), loaded.blocks.clone()),
                    );
                    (
                        loaded.text.clone(),
                        loaded.spans.clone(),
                        loaded.blocks.clone(),
                        loaded.history.clone(),
                    )
                }
                None => match &md_import {
                    Some((text, spans, blocks)) => {
                        store.seed(text);
                        (text.clone(), spans.clone(), blocks.clone(), None)
                    }
                    None if welcome => {
                        // First run ever (or "Open Welcome Guide"): the
                        // tutorial document, margin demo cards included.
                        let (text, spans, blocks, notes) = tutorial::document();
                        store.seed(&text);
                        store.add_checkpoint("Fresh tutorial", true);
                        tutorial_notes = Some(notes);
                        (text, spans, blocks, None)
                    }
                    None => {
                        store.seed("");
                        (String::new(), SpanSet::default(), BlockMap::default(), None)
                    }
                },
            },
            None => (SAMPLE.to_owned(), SpanSet::default(), BlockMap::default(), None),
        };

        let bounds = match load_bounds() {
            Some((x, y, w, h)) => Bounds {
                origin: gpui::point(px(x), px(y)),
                size: size(px(w.max(400.)), px(h.max(300.))),
            },
            None => Bounds::centered(None, size(px(960.), px(720.)), cx),
        };
        let title = {
            let stem = doc_path
                .as_ref()
                .and_then(|p| p.file_stem())
                .and_then(|s| s.to_str())
                .unwrap_or("Smoke")
                .to_owned();
            format!("{stem} — Strop")
        };
        let window = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(title.clone().into()),
                    // Strop draws its own titlebar, so the OS must not stack a
                    // native one on top of it. On Windows this flag is what
                    // flips gpui's `hide_title_bar` — without it the native
                    // caption is kept (the reported "two title bars" bug, and
                    // why the window was draggable only by the native one). On
                    // macOS it makes the system titlebar transparent so our
                    // chrome shows through. Linux/Wayland CSD ignores it.
                    appears_transparent: true,
                    // macOS KEEPS its native traffic-light buttons (we hide our
                    // own redundant controls there — see render_titlebar). With
                    // `appears_transparent` + full-size content view the lights
                    // otherwise sit at the very top-left and overlap our chrome.
                    // Recentre them in the 36px bar: y=11 vertically centres the
                    // ~14px buttons; render_titlebar insets the bar's left
                    // content past them. (No-op on other platforms.)
                    #[cfg(target_os = "macos")]
                    traffic_light_position: Some(gpui::point(px(20.), px(11.))),
                    ..Default::default()
                }),
                focus: !smoke,
                // Strop draws its own titlebar and (since H2) its own resize
                // borders. Request client-side decorations so the compositor
                // hands us the resize affordance instead of leaving the
                // window with neither (GNOME Wayland does no server-side
                // decorations — the H2 "can't resize by dragging" bug).
                window_decorations: Some(WindowDecorations::Client),
                // Transparent surface ONLY where the CSD shadow gutter
                // (editor.rs: render) must show through — the Linux client-side-
                // decoration path (GNOME/sway Wayland draw no server shadow).
                // macOS and Windows draw their own (server-side) shadow and want
                // an OPAQUE layer. This matters on macOS specifically: a
                // transparent CAMetalLayer disables gpui's direct-to-display fast
                // path and routes every frame through the window server's alpha
                // compositor for no benefit (we cover the whole window with an
                // opaque background quad anyway). The author's own note below
                // already assumed these platforms were opaque — the code just
                // never made it conditional.
                window_background: if cfg!(any(target_os = "linux", target_os = "freebsd")) {
                    WindowBackgroundAppearance::Transparent
                } else {
                    WindowBackgroundAppearance::Opaque
                },
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title(&title);
                let editor = cx.new(|cx| {
                    let mut editor = Editor::new(cx, &initial_text, initial_spans, initial_blocks);
                    editor.config = config::load();
                    editor.load_voice_corpus();
                    if let Some(history) = initial_history {
                        editor.restore_history(history);
                    }
                    if let Some((_, Some(loaded))) = &store {
                        editor.restore_annotations(loaded.annotations.clone());
                        editor.restore_journal(loaded.journal.clone());
                        editor.restore_graveyard(loaded.graveyard.clone());
                        editor.restore_provenance(loaded.provenance.clone());
                    }
                    if let Some(notes) = tutorial_notes {
                        editor.restore_annotations(notes);
                        // The tutorial's whole point is to show the margin —
                        // open the door (DESIGN §4.4) so the demo diagnoses
                        // are visible on first run, not collapsed to the rail.
                        editor.enter_reviewing();
                    }
                    // Re-entry (DESIGN §4): the caret is restored where the last
                    // session left it — nothing is ever asked at open (the §4
                    // invariant). The intent question was retired (impl 04 §1).
                    if let Some((store, _)) = &store
                        && let Some(entry) = files::load_session(store.path())
                    {
                        editor.restore_session(entry);
                    }
                    editor.start_blink(cx);
                    if let Some((store, _)) = store {
                        editor.attach_store(store, cx);
                    }
                    // A shipped compost-at-top document migrates ONCE, here —
                    // after every channel is restored, before the first edit
                    // (docs/impl/08-compost-fresh.md §2 "Adoption &
                    // migration"; adjudications time-persistence 4).
                    editor.migrate_scraps_geometry();
                    editor
                });
                window.focus(&editor.focus_handle(cx), cx);
                // Single-window app: route an OS-driven close request (the macOS
                // traffic-light close — now our only window control there since we
                // hide our own; or a compositor/WM close on Linux/Windows) to
                // quit, so `on_app_quit` (below) flushes the document + exit state.
                // macOS does NOT terminate on last-window-close by default, so
                // without this the native close would just hide the window. Our own
                // "×" (shown off-macOS) calls cx.quit() directly.
                //
                // Return `false` to VETO the platform's synchronous window close
                // and let cx.quit() be the sole teardown driver — mirroring Zed's
                // own handler (crates/zed/src/zed.rs). Returning `true` would let
                // the platform close and DROP the Editor entity *before* quit's
                // shutdown() runs `on_app_quit`, turning its save_now()/
                // record_exit_state() into a silent no-op (the handler below uses
                // update_checked). shutdown() runs on_app_quit *before* clearing
                // windows, so vetoing keeps the Editor alive until it's saved.
                //
                // Calling cx.quit() synchronously here is fine — a clean close of a
                // normal document exits in ~0.1s. A *slow* close is a separate
                // problem: the `on_app_quit` save below is synchronous, so a large
                // document (a multi-MB Loro doc with mark churn) can block teardown
                // for several seconds and trip the compositor's not-responding
                // watchdog. That's an engine save-perf issue, not a quit-path one.
                window.on_window_should_close(cx, |_, cx| {
                    cx.quit();
                    false
                });
                editor
            },
        )
        .expect("failed to open window");

        // Flush the document on quit; the idle-save loop covers the rest.
        let editor = window
            .update(cx, |_, _, cx| cx.entity())
            .expect("window just opened");
        let window_for_quit = window;
        cx.on_app_quit(move |cx| {
            editor.update_checked(cx, |editor, _| {
                editor.save_now();
                // Caret remembered for next open (resume mid-sentence);
                // never a question, never a dialog (DESIGN §4b tension 6).
                editor.record_exit_state();
            });
            let _ = window_for_quit.update(cx, |_, window, _| {
                let b = window.bounds();
                save_bounds((
                    f32::from(b.origin.x),
                    f32::from(b.origin.y),
                    f32::from(b.size.width),
                    f32::from(b.size.height),
                ));
            });
            async {}
        })
        .detach();

        // Surface this window when a later `strop <same file>` hands off
        // (best-effort: app activation, which the Wayland backend maps to
        // xdg-activation where the compositor honours it). The task owns the
        // guard, so the rendezvous socket is released when the app quits.
        if let Some(guard) = instance_guard {
            cx.spawn(async move |cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(400))
                        .await;
                    if guard.pull_raise() {
                        cx.update(|cx| cx.activate(true));
                    }
                }
            })
            .detach();
        }

        smoke::maybe_run(window, cx);
        if !smoke {
            cx.activate(true);
        }
    });
}
