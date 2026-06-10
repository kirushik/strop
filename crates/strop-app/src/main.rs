use gpui::{
    div, prelude::*, px, rgb, size, App, Application, Bounds, Context, SharedString,
    TitlebarOptions, Window, WindowBounds, WindowOptions,
};

const LITERATA: &[u8] = include_bytes!("../../../assets/fonts/Literata[opsz,wght].ttf");
const LITERATA_ITALIC: &[u8] =
    include_bytes!("../../../assets/fonts/Literata-Italic[opsz,wght].ttf");

/// v0: a static proof of the typographic canvas — Literata 20/28 on a ~64ch
/// measure, vertical gaps in line-height multiples. Editing lands next.
struct Canvas {
    paragraphs: Vec<SharedString>,
}

impl Render for Canvas {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0xFBFAF8))
            .flex()
            .justify_center()
            .overflow_hidden()
            .child(
                div()
                    .max_w(px(660.))
                    .pt(px(84.))
                    .px(px(28.))
                    .font_family("Literata")
                    .text_size(px(20.))
                    .line_height(px(28.))
                    .text_color(rgb(0x1A1A18))
                    .children(
                        self.paragraphs
                            .iter()
                            .map(|p| div().mb(px(28.)).child(p.clone())),
                    ),
            )
    }
}

const SAMPLE: &[&str] = &[
    "Strop is a writer’s editor with an editor inside — one that diagnoses, \
     and never rewrites you into the average.",
    "Хороший редактор называет проблему — «здесь зарыта мысль», «начало \
     хоронит главное» — и оставляет решение автору. Тире, кавычки-ёлочки и \
     неразрывные пробелы должны просто работать: 1941—1945, «так», “so”.",
    "The reader is right that something is wrong, and almost always wrong \
     about how to fix it.",
];

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.text_system()
            .add_fonts(vec![LITERATA.into(), LITERATA_ITALIC.into()])
            .expect("failed to load bundled Literata");

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
            |_, cx| {
                cx.new(|_| Canvas {
                    paragraphs: SAMPLE.iter().map(|s| SharedString::from(*s)).collect(),
                })
            },
        )
        .expect("failed to open window");
        cx.activate(true);
    });
}
