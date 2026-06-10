mod editor;
mod smoke;

use std::path::PathBuf;

use gpui::{
    App, Application, Bounds, Focusable, KeyBinding, TitlebarOptions, WindowBounds, WindowOptions,
    actions, prelude::*, px, size,
};
use strop_core::Store;

use editor::Editor;

actions!(strop, [Quit]);

const LITERATA: &[u8] = include_bytes!("../../../assets/fonts/Literata[opsz,wght].ttf");
const LITERATA_ITALIC: &[u8] =
    include_bytes!("../../../assets/fonts/Literata-Italic[opsz,wght].ttf");

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

/// `strop [file.strop]`; default document lives in the XDG data dir.
fn data_file() -> PathBuf {
    if let Some(arg) = std::env::args().nth(1) {
        return arg.into();
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").expect("HOME not set")).join(".local/share")
        });
    base.join("strop/scratch.strop")
}

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.text_system()
            .add_fonts(vec![LITERATA.into(), LITERATA_ITALIC.into()])
            .expect("failed to load bundled Literata");

        editor::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("ctrl-q", Quit, None)]);
        cx.on_action(|_: &Quit, cx| cx.quit());

        // Smoke runs must not steal the user's OS focus — keystroke dispatch
        // uses GPUI's internal focus, which we set explicitly below. They
        // also never touch the user's real document: no store unless a file
        // was passed explicitly (which lets smoke scripts test persistence).
        let smoke = std::env::var("STROP_SMOKE").is_ok();
        let store = if smoke && std::env::args().nth(1).is_none() {
            None
        } else {
            match Store::open(data_file()) {
                Ok(opened) => Some(opened),
                Err(e) => {
                    eprintln!("strop: cannot open {}: {e}", data_file().display());
                    None
                }
            }
        };
        let initial_text = match &store {
            Some((store, existing)) => match existing {
                Some(text) => text.clone(),
                None => {
                    store.seed(SAMPLE);
                    SAMPLE.to_owned()
                }
            },
            None => SAMPLE.to_owned(),
        };

        let bounds = Bounds::centered(None, size(px(960.), px(720.)), cx);
        let window = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Strop".into()),
                    ..Default::default()
                }),
                focus: !smoke,
                ..Default::default()
            },
            |window, cx| {
                let editor = cx.new(|cx| {
                    let mut editor = Editor::new(cx, &initial_text);
                    editor.start_blink(cx);
                    if let Some((store, _)) = store {
                        editor.attach_store(store, cx);
                    }
                    editor
                });
                window.focus(&editor.focus_handle(cx));
                editor
            },
        )
        .expect("failed to open window");

        // Flush the document on quit; the idle-save loop covers the rest.
        let editor = window
            .update(cx, |_, _, cx| cx.entity())
            .expect("window just opened");
        cx.on_app_quit(move |cx| {
            editor.update(cx, |editor, _| editor.save_now());
            async {}
        })
        .detach();

        smoke::maybe_run(window, cx);
        if !smoke {
            cx.activate(true);
        }
    });
}
