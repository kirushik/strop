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

mod ai_log;
mod about;
mod bookpage;
mod commands;
mod config;
mod draw_guard;
mod editor;
mod hyphen;
mod icons;
mod keymap_window;
mod files;
mod paths;
mod single_instance;
mod smoke;
mod strip;
mod startup_error;
mod text_field;
mod theme;
mod tutorial;
mod update;

use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui::{
    App, Bounds, Focusable, KeyBinding, TitlebarOptions, WindowBackgroundAppearance, WindowBounds,
    WindowDecorations, WindowOptions, actions, prelude::*, px, size,
};
use strop_core::Store;
use strop_core::document::{BlockMap, SpanSet};

use draw_guard::EntityUpdateExt as _;
use editor::Editor;

actions!(strop, [Quit, AboutStrop]);

fn register_unhandled_quit(
    cx: &mut App,
    listener: impl Fn(&mut App) + 'static,
) {
    // Action handlers in a rendered window stop propagation by default. The
    // editor therefore keeps ownership of its durable-save preflight, while
    // this app-level fallback makes Ctrl-Q work in startup/recovery and any
    // future auxiliary window that has no document lifecycle to protect.
    cx.on_action(move |_: &Quit, cx| listener(cx));
}

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
        Some(arg) => return (files::resolve_portal_path(arg), false),
        None => {}
    }
    if let Some(migrated) = files::migrate_scratch() {
        return (migrated, false);
    }
    if let Some(recent) = files::recents()
        .into_iter()
        .find(|path| path.exists())
    {
        return (recent, false);
    }
    (files::welcome_path(), true)
}

/// Where a document path durably lives, and whether opening it means a
/// Markdown import: a .md maps to its sibling .strop, importing only when
/// that sibling does not exist yet — once born, the .strop is the source
/// of truth and wins every later open. The import itself is LAZY (see
/// `Editor::stage_import_birth`): deciding `true` here creates nothing.
/// An unborn sidecar is minted through the sibling-minting law (Copilot,
/// PR #28): a .md that reached us as an UNRESOLVABLE portal path must not
/// grow its .strop inside the doc mount, where a foreign name strands as
/// an invisible `.xdp-*` temp — `files::host_parent_or_documents` sends
/// it to the documents folder instead. For any ordinary path that law
/// answers with the .md's own parent, so nothing moves.
fn open_target(doc_path: &Path) -> (PathBuf, bool) {
    if doc_path.extension().is_some_and(|e| e == "md") {
        let sibling = doc_path.with_extension("strop");
        if sibling.exists() {
            return (sibling, false);
        }
        let name = sibling
            .file_name()
            .unwrap_or(std::ffi::OsStr::new("Untitled.strop"));
        let minted = files::host_parent_or_documents(doc_path).join(name);
        let import = !minted.exists();
        (minted, import)
    } else {
        (doc_path.to_owned(), false)
    }
}

fn main() {
    // Before anything — before arguments are read and before any
    // single-instance socket can exist: if a verified update is staged,
    // this swaps binaries and re-execs (docs/releasing.md §4). The
    // rendezvous below must only ever be performed by the binary that
    // will actually run.
    update::startup_apply_if_staged();

    // gpui_platform::application() replaced gpui::Application::new() after
    // the facade/platform crate split. The asset source feeds gpui's svg()
    // pipeline the embedded icon plate (docs/iconography.md).
    gpui_platform::application().with_assets(icons::StropAssets).run(|cx: &mut App| {
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

        // The cold read's book face: URW Bookman Light / Light Italic /
        // Demi (urw-base35; full Cyrillic on every style). Loaded as
        // RUNTIME DATA FILES — read from assets at startup, never
        // include_bytes! — because the face is AGPL-3.0-with-exception in
        // a GPL-3.0-or-later app and the mere-aggregation posture wants it
        // an independent work on disk (impl 05 §3.1; license text lives
        // beside the files). A missing file degrades honestly: the page
        // falls through font_fallbacks to the bundled PT Serif backstop.
        let book_fonts: Vec<std::borrow::Cow<'static, [u8]>> = [
            "fonts/coldread/URWBookman-Light.otf",
            "fonts/coldread/URWBookman-LightItalic.otf",
            "fonts/coldread/URWBookman-Demi.otf",
        ]
        .iter()
        .filter_map(|rel| match paths::asset_file(rel) {
            Some(path) => match std::fs::read(&path) {
                Ok(bytes) => Some(bytes.into()),
                Err(e) => {
                    eprintln!("strop: cannot read {}: {e} — the cold read falls back to PT Serif", path.display());
                    None
                }
            },
            None => {
                eprintln!("strop: {rel} not found — the cold read falls back to PT Serif");
                None
            }
        })
        .collect();
        if !book_fonts.is_empty()
            && let Err(e) = cx.text_system().add_fonts(book_fonts)
        {
            eprintln!("strop: failed to register URW Bookman: {e} — the cold read falls back to PT Serif");
        }

        editor::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("ctrl-q", Quit, None)]);
        register_unhandled_quit(cx, |cx| cx.quit());

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
        // Where the durable file lives and whether this open is a Markdown
        // import — decided once, up front (`open_target`): the answer gates
        // both the store below and the lazy-birth staging at the end.
        let open_plan = doc_path.as_ref().map(|p| open_target(p));
        let store = match &doc_path {
            None => None,
            Some(p) => {
                let (store_path, planned_import) = open_plan.clone().expect("doc_path is Some");
                let require_existing = std::env::var_os("STROP_REQUIRE_EXISTING").is_some();
                // Intentional birth at an explicit CLI path may create its
                // parent; a LAZY .md import must not — its sidecar's parent
                // is the .md's own directory, and a glance creates nothing
                // (fleet finding: failed lazy opens littered empty dirs).
                if !require_existing
                    && !planned_import
                    && !store_path.exists()
                    && let Some(parent) = store_path.parent()
                    && let Err(e) = std::fs::create_dir_all(parent)
                {
                    startup_error::show(
                        startup_error::OpenFailure::from_io(
                            startup_error::OpenOperation::Open,
                            store_path,
                            &e,
                        ),
                        cx,
                    );
                    return;
                }
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
                match Store::open_with_backup_destination(
                    &store_path,
                    Some(&paths::migration_backups_dir()),
                ) {
                    Ok((_, None)) if require_existing => {
                        let e = std::io::Error::from(std::io::ErrorKind::NotFound);
                        drop(instance_guard.take());
                        startup_error::show(
                            startup_error::OpenFailure::from_io(
                                startup_error::OpenOperation::Open,
                                store_path,
                                &e,
                            ),
                            cx,
                        );
                        return;
                    }
                    Ok(opened) => {
                        if !smoke {
                            files::push_recent(opened.0.path());
                        }
                        Some(opened)
                    }
                    Err(e) => {
                        eprintln!("strop: cannot open {}: {e}", p.display());
                        // Release the rendezvous before Try Again launches a
                        // fresh process for the same path.
                        drop(instance_guard.take());
                        startup_error::show(
                            startup_error::OpenFailure::from_io(
                                startup_error::OpenOperation::Open,
                                store_path,
                                &e,
                            ),
                            cx,
                        );
                        return;
                    }
                }
            }
        };
        // Opening a .md imports it into a sibling .strop (existing .strop
        // wins — the durable file is the source of truth once it exists).
        // The import is READ here but BORN lazily: the state is staged on
        // the editor (`stage_import_birth`) and the .strop file — plus its
        // "Started" birth seal — appears only at the first mutation, so a
        // user whose default .md opener is Strop can glance at any folder
        // without littering it (quit-without-edits is byte-identical).
        let md_import: Option<(String, SpanSet, BlockMap)> = if let Some(arg) = &doc_path
            && open_plan.as_ref().is_some_and(|(_, import)| *import)
        {
            match std::fs::read_to_string(arg) {
                Ok(md) => {
                    let (text, spans, blocks) = strop_core::markdown::from_markdown(&md);
                    Some((text, spans, blocks))
                }
                Err(e) => {
                    eprintln!("strop: cannot import {}: {e}", arg.display());
                    drop(instance_guard.take());
                    startup_error::show(
                        startup_error::OpenFailure::from_io(
                            startup_error::OpenOperation::Import,
                            arg.clone(),
                            &e,
                        ),
                        cx,
                    );
                    return;
                }
            }
        } else {
            None
        };

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
                        // Lazy birth: NOTHING is seeded or sealed here. The
                        // store stays an empty doc over a missing file; the
                        // staged state below seeds it — and seals "Started",
                        // the birth checkpoint the journal-era strip anchors
                        // on — at the first mutation (Editor::materialize_
                        // store). Until then the editor is fully functional
                        // over the imported text, and a quit leaves the
                        // filesystem exactly as the open found it.
                        (text.clone(), spans.clone(), blocks.clone(), None)
                    }
                    None if welcome => {
                        // First run ever (or "Open Welcome Guide"): the
                        // tutorial document, margin demo cards included.
                        let (text, spans, blocks, notes) = tutorial::document();
                        store.seed(&text);
                        store.add_checkpoint("Fresh tutorial", false);
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
                app_id: Some("cc.pimenov.strop".to_owned()),
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
                    // The first window is up: start the update checks (§4/§5
                    // — launch + every 8 h; channel- and config-gated inside).
                    update::spawn_checks(&editor.config);
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
                    // Lazy birth staging must ride with the store: both are
                    // in hand before the writer's first act can possibly
                    // reach `materialize_store`.
                    if let Some((text, spans, blocks)) = md_import {
                        editor.stage_import_birth(text, spans, blocks);
                    }
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
                // the same save preflight as ctrl-q and the drawn close control.
                // macOS does NOT terminate on last-window-close by default, so
                // without this the native close would just hide the window.
                //
                // Return `false` to VETO the platform's synchronous window close
                // until the ordered worker reports the newest generation durable.
                // Returning `true` would drop the Editor — and its recovery UI —
                // before a failed final write could be handled.
                let close_editor = editor.clone();
                window.on_window_should_close(cx, move |window, cx| {
                    close_editor.update_checked(cx, |editor, cx| {
                        editor.request_quit(&Quit, window, cx);
                    });
                    false
                });
                editor
            },
        )
        .expect("failed to open window");

        // Normal quit paths preflight the newest save generation while the
        // window is still alive. Quit observers cannot veto shutdown.
        let editor = window
            .update(cx, |_, _, cx| cx.entity())
            .expect("window just opened");
        let about_window = std::rc::Rc::new(std::cell::RefCell::new(
            None::<gpui::WindowHandle<about::AboutWindow>>,
        ));
        let about_slot = about_window.clone();
        let editor_window = window;
        cx.on_action(move |_: &AboutStrop, cx| {
            if let Some(reference) = *about_slot.borrow()
                && reference.update(cx, |_, window, _| window.activate_window()).is_ok()
            {
                return;
            }
            *about_slot.borrow_mut() = None;
            if let Ok(bounds) = editor_window.update(cx, |_, window, _| window.bounds()) {
                *about_slot.borrow_mut() = about::open(editor_window.into(), bounds, cx);
            }
        });
        let window_for_quit = window;
        cx.on_app_quit(move |cx| {
            // Finding 7 / LAW 2: an open transient field must not lose its
            // content to the quit. Flush the composer (synchronous doc mutation)
            // and every single-line field (including link and cold-read
            // reaction) in a SEPARATE update, so their resolution effects
            // deliver — writing edits into the doc — BEFORE the save update
            // below serializes it.
            editor.update_checked(cx, |editor, cx| {
                editor.commit_composer_no_focus(cx);
                editor.commit_transient_fields_on_quit(cx);
            });
            editor.update_checked(cx, |editor, _| {
                // The last-ditch write, for the quit paths that never reach
                // `request_quit` (the smoke script, an unhandled Quit): those
                // reach the process's end with no window left to hold open, so
                // the guarantee has to be met HERE. It must WAIT, not enqueue —
                // the worker owns the write now, and `save_now` alone would let
                // the process exit with the bytes still in flight. A graceful
                // quit has already saved by this point, and the fingerprint
                // guards make this second call a no-op that writes nothing.
                if let Err(e) = editor.flush_saves() {
                    // LAW 2's last line: the process may not exit with
                    // dirty bytes and no witness. Salvage a full snapshot
                    // into the state dir and say where it went.
                    eprintln!("strop: final save failed at quit: {e}");
                    editor.salvage_recovery_copy();
                }
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

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui::{
        FocusHandle, Focusable, IntoElement, Render, TestAppContext, VisualTestContext, Window, div,
    };

    use super::*;

    struct QuitSurface {
        local_quits: Option<Rc<Cell<usize>>>,
        focus_handle: FocusHandle,
    }

    impl Focusable for QuitSurface {
        fn focus_handle(&self, _: &App) -> FocusHandle {
            self.focus_handle.clone()
        }
    }

    impl Render for QuitSurface {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let mut root = div();
            if let Some(local_quits) = self.local_quits.clone() {
                root = root.on_action(move |_: &Quit, _, _| {
                    local_quits.set(local_quits.get() + 1);
                });
            }
            root.track_focus(&self.focus_handle)
        }
    }

    fn quit_window(
        cx: &mut TestAppContext,
        local_quits: Option<Rc<Cell<usize>>>,
    ) -> VisualTestContext {
        let window = cx.update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                let surface = cx.new(|cx| QuitSurface {
                    local_quits,
                    focus_handle: cx.focus_handle(),
                });
                window.focus(&surface.focus_handle(cx), cx);
                surface
            })
            .unwrap()
        });
        VisualTestContext::from_window(window.into(), cx)
    }

    #[test]
    fn open_target_routes_md_imports_and_the_sibling_strop_wins() {
        let dir = std::env::temp_dir()
            .join(format!("strop-open-target-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let md = dir.join("essay.md");
        std::fs::write(&md, "# hi").unwrap();
        assert_eq!(
            open_target(&md),
            (dir.join("essay.strop"), true),
            "a lone .md maps to its sibling and imports (lazily)"
        );
        std::fs::write(dir.join("essay.strop"), b"x").unwrap();
        assert_eq!(
            open_target(&md),
            (dir.join("essay.strop"), false),
            "an existing sibling .strop wins — never a re-import"
        );
        let strop = dir.join("direct.strop");
        assert_eq!(
            open_target(&strop),
            (strop.clone(), false),
            "a .strop opens as itself"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[gpui::test]
    fn unhandled_quit_reaches_the_app_fallback(cx: &mut TestAppContext) {
        let fallback_quits = Rc::new(Cell::new(0));
        cx.update({
            let fallback_quits = fallback_quits.clone();
            move |cx| {
                register_unhandled_quit(cx, move |_| {
                    fallback_quits.set(fallback_quits.get() + 1);
                });
            }
        });
        let mut window = quit_window(cx, None);

        window.dispatch_action(Quit);

        assert_eq!(fallback_quits.get(), 1);
    }

    #[gpui::test]
    fn window_quit_handler_keeps_ownership_of_preflight(cx: &mut TestAppContext) {
        let fallback_quits = Rc::new(Cell::new(0));
        let local_quits = Rc::new(Cell::new(0));
        cx.update({
            let fallback_quits = fallback_quits.clone();
            move |cx| {
                register_unhandled_quit(cx, move |_| {
                    fallback_quits.set(fallback_quits.get() + 1);
                });
            }
        });
        let mut window = quit_window(cx, Some(local_quits.clone()));

        window.dispatch_action(Quit);

        assert_eq!(local_quits.get(), 1);
        assert_eq!(fallback_quits.get(), 0);
    }
}
