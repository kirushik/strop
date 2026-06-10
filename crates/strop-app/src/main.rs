mod editor;

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
The reader is right that something is wrong, and almost always wrong about how to fix it.";

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.text_system()
            .add_fonts(vec![LITERATA.into(), LITERATA_ITALIC.into()])
            .expect("failed to load bundled Literata");

        editor::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("ctrl-q", Quit, None)]);
        cx.on_action(|_: &Quit, cx| cx.quit());

        let bounds = Bounds::centered(None, size(px(960.), px(720.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Strop".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                let editor = cx.new(|cx| Editor::new(cx, SAMPLE));
                window.focus(&editor.focus_handle(cx));
                editor
            },
        )
        .expect("failed to open window");
        cx.activate(true);
    });
}
