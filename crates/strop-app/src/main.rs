// Draw-pass discipline (docs/VISUAL-RIG.md): raw Entity::update and canvas
// are banned crate-wide; clippy.toml points each ban at its draw_guard
// wrapper. Deny, not warn — a mid-draw notify is a corruption bug, not style.
#![deny(clippy::disallowed_methods)]

mod commands;
mod config;
mod draw_guard;
mod editor;
mod files;
mod smoke;
mod tutorial;

use std::path::PathBuf;

use gpui::{
    App, Bounds, Focusable, KeyBinding, TitlebarOptions, WindowBounds, WindowDecorations,
    WindowOptions, actions, prelude::*, px, size,
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

/// Remembered window bounds, in ~/.local/state (or XDG_STATE_HOME).
fn state_file() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").expect("HOME not set")).join(".local/state")
        })
        .join("strop/window.json")
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
        let store = match &doc_path {
            None => None,
            Some(p) => {
                let store_path = if p.extension().is_some_and(|e| e == "md") {
                    p.with_extension("strop")
                } else {
                    p.clone()
                };
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
                    // visible restore away.
                    store.add_checkpoint_if_changed("Session start", false);
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
                    ..Default::default()
                }),
                focus: !smoke,
                // Strop draws its own titlebar and (since H2) its own resize
                // borders. Request client-side decorations so the compositor
                // hands us the resize affordance instead of leaving the
                // window with neither (GNOME Wayland does no server-side
                // decorations — the H2 "can't resize by dragging" bug).
                window_decorations: Some(WindowDecorations::Client),
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
                    }
                    if let Some(notes) = tutorial_notes {
                        editor.restore_annotations(notes);
                    }
                    // The if-then ritual's open half (DESIGN §4.1): caret
                    // restored, last close's intent surfaced — and nothing
                    // is ever asked at open (the §4 invariant).
                    if let Some((store, _)) = &store
                        && let Some(entry) = files::load_intent(store.path())
                    {
                        editor.restore_session(entry);
                    }
                    editor.start_blink(cx);
                    if let Some((store, _)) = store {
                        editor.attach_store(store, cx);
                    }
                    editor
                });
                window.focus(&editor.focus_handle(cx), cx);
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

        smoke::maybe_run(window, cx);
        if !smoke {
            cx.activate(true);
        }
    });
}
