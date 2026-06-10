mod editor;
mod smoke;

use gpui::{
    App, Application, Bounds, Focusable, KeyBinding, TitlebarOptions, WindowBounds, WindowOptions,
    actions, prelude::*, px, size,
};

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

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.text_system()
            .add_fonts(vec![LITERATA.into(), LITERATA_ITALIC.into()])
            .expect("failed to load bundled Literata");

        editor::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("ctrl-q", Quit, None)]);
        cx.on_action(|_: &Quit, cx| cx.quit());

        let bounds = Bounds::centered(None, size(px(960.), px(720.)), cx);
        let window = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Strop".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                let editor = cx.new(|cx| {
                    let editor = Editor::new(cx, SAMPLE);
                    editor.start_blink(cx);
                    editor
                });
                window.focus(&editor.focus_handle(cx));
                editor
            },
        )
        .expect("failed to open window");
        smoke::maybe_run(window, cx);
        cx.activate(true);
    });
}
