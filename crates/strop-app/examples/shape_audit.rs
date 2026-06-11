//! Headless audit of GPUI's shaping pipeline — no pixels needed.
//!
//! For every shaped glyph whose run resolves to one of our bundled faces,
//! compare the glyph id GPUI produced against the face's own cmap (via
//! ttf-parser over the same embedded bytes). Cyrillic and plain Latin have
//! no ligatures in PT, so shaped id == cmap id for every checked char; a
//! mismatch means the shaper used a different face than the run claims.
//!
//! The order of shaping calls is the experiment variable: GPUI reuses one
//! cosmic-text ShapeBuffer across calls and caches layouts, so pollution
//! shows up as the SAME line shaping differently in different company.
//! Run with ORDER=targets-first and ORDER=decoys-first and diff:
//!   ORDER=targets-first cargo run -p strop-app --example shape_audit
//!
//! `SIG <target> = ...` lines are stable glyph-id signatures for diffing.

use gpui::{Font, FontWeight, SharedString, TextRun, prelude::*, px};

const PT_SERIF: &[u8] = include_bytes!("../../../assets/fonts/PTSerif-Regular.ttf");
const PT_SERIF_ITALIC: &[u8] = include_bytes!("../../../assets/fonts/PTSerif-Italic.ttf");
const PT_SERIF_BOLD: &[u8] = include_bytes!("../../../assets/fonts/PTSerif-Bold.ttf");
const PT_SERIF_BOLD_ITALIC: &[u8] = include_bytes!("../../../assets/fonts/PTSerif-BoldItalic.ttf");
const PT_SANS: &[u8] = include_bytes!("../../../assets/fonts/PTSans-Regular.ttf");
const PT_SANS_ITALIC: &[u8] = include_bytes!("../../../assets/fonts/PTSans-Italic.ttf");
const PT_SANS_BOLD: &[u8] = include_bytes!("../../../assets/fonts/PTSans-Bold.ttf");
const PT_SANS_BOLD_ITALIC: &[u8] = include_bytes!("../../../assets/fonts/PTSans-BoldItalic.ttf");
const PT_MONO: &[u8] = include_bytes!("../../../assets/fonts/PTMono-Regular.ttf");

fn glyph_num(id: gpui::GlyphId) -> u32 {
    // GlyphId's inner field is pub(crate); Debug prints "GlyphId(n)".
    let s = format!("{id:?}");
    s.trim_start_matches("GlyphId(")
        .trim_end_matches(')')
        .parse()
        .expect("unexpected GlyphId debug format")
}

fn face(family: &'static str, weight: FontWeight, italic: bool) -> Font {
    let mut font = gpui::font(family);
    font.weight = weight;
    if italic {
        font.style = gpui::FontStyle::Italic;
    }
    font
}

/// Minimal root view; the audit runs in the window-build closure because
/// shape_text lives on WindowTextSystem (whose constructor is pub(crate)).
struct AuditView;

impl gpui::Render for AuditView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        gpui::div()
    }
}

struct Target {
    label: &'static str,
    text: &'static str,
    /// (byte_len, face); must sum to text.len()
    runs: Vec<(usize, Font)>,
    size: f32,
}

fn whole(text: &'static str, font: Font) -> Vec<(usize, Font)> {
    vec![(text.len(), font)]
}

fn main() {
    let order = std::env::var("ORDER").unwrap_or_else(|_| "targets-first".into());
    gpui_platform::application().run(move |cx: &mut gpui::App| {
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

        // Window only to reach WindowTextSystem; never focused or activated,
        // same etiquette as the smoke harness.
        let window = cx
            .open_window(
                gpui::WindowOptions {
                    focus: false,
                    ..Default::default()
                },
                |_, cx| cx.new(|_| AuditView),
            )
            .expect("open audit window");
        window
            .update(cx, |_, window, _| {
                run_audit(window, &order);
            })
            .expect("audit window update");
        // quit() before the event loop starts is a no-op in current gpui
        // (the Linux platform's stop signal only lands once calloop runs),
        // so defer it to a foreground task.
        cx.spawn(async move |cx| {
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
}

fn run_audit(window: &mut gpui::Window, order: &str) {
    {
        let ts = window.text_system().clone();

        // FontId -> (label, cmap face), via the same bytes GPUI loaded.
        let known: Vec<(&str, &'static [u8], Font)> = vec![
            ("serif", PT_SERIF, face("PT Serif", FontWeight::NORMAL, false)),
            ("serif-it", PT_SERIF_ITALIC, face("PT Serif", FontWeight::NORMAL, true)),
            ("serif-bold", PT_SERIF_BOLD, face("PT Serif", FontWeight::BOLD, false)),
            ("serif-bold-it", PT_SERIF_BOLD_ITALIC, face("PT Serif", FontWeight::BOLD, true)),
            ("sans", PT_SANS, face("PT Sans", FontWeight::NORMAL, false)),
            ("sans-bold", PT_SANS_BOLD, face("PT Sans", FontWeight::BOLD, false)),
            ("mono", PT_MONO, face("PT Mono", FontWeight::NORMAL, false)),
        ];
        let mut by_id: Vec<(usize, &str, ttf_parser::Face)> = Vec::new();
        for (label, bytes, font) in &known {
            let id = ts.resolve_font(font);
            let parsed = ttf_parser::Face::parse(bytes, 0).expect("ttf parse");
            println!("font {label} -> FontId({})", id.0);
            by_id.push((id.0, label, parsed));
        }

        let serif = || face("PT Serif", FontWeight::NORMAL, false);
        let serif_bold = || face("PT Serif", FontWeight::BOLD, false);
        let sans_bold = || face("PT Sans", FontWeight::BOLD, false);
        let mono = || face("PT Mono", FontWeight::NORMAL, false);

        // The lines that corrupted in Kirill's screenshots, plus UI-chrome
        // strings full of fallback characters (○ ↑ ↓ ↺ −) that force
        // mid-session system-font loads.
        let lish = "Gordon Lish cut Carver’s stories by half and more; Maxwell Perkins";
        let targets: Vec<Target> = vec![
            Target {
                label: "h3-cyrillic",
                text: "И с чем её едят",
                runs: whole("И с чем её едят", sans_bold()),
                size: 20.,
            },
            Target {
                label: "h1",
                text: "Глава 2.",
                runs: whole("Глава 2.", sans_bold()),
                size: 32.,
            },
            Target {
                label: "h2",
                text: "Связность",
                runs: whole("Связность", sans_bold()),
                size: 24.,
            },
            Target {
                label: "williams",
                text: "Уильямс учит связности — старое перед новым, переходы как мосты.",
                runs: whole(
                    "Уильямс учит связности — старое перед новым, переходы как мосты.",
                    serif(),
                ),
                size: 20.,
            },
            Target {
                label: "lish-bold-run",
                text: lish,
                runs: vec![
                    ("Gordon ".len(), serif()),
                    ("Lish".len(), serif_bold()),
                    (lish.len() - "Gordon Lish".len(), serif()),
                ],
                size: 20.,
            },
            Target {
                label: "history-row",
                text: "○ Session start",
                runs: whole("○ Session start", serif()),
                size: 13.,
            },
            Target {
                label: "history-time",
                text: "09:22  +9 −0",
                runs: whole("09:22  +9 −0", serif()),
                size: 11.,
            },
            Target {
                label: "history-hints",
                text: "↑/↓ step versions · Esc exits · restoring is undoable",
                runs: whole("↑/↓ step versions · Esc exits · restoring is undoable", serif()),
                size: 11.,
            },
            Target {
                label: "typograph",
                text: "Тире, кавычки-ёлочки: 1941—1945, «так», “so”.",
                runs: whole("Тире, кавычки-ёлочки: 1941—1945, «так», “so”.", serif()),
                size: 20.,
            },
        ];

        // Decoys mimic everything else the app shapes in a frame: body
        // paragraphs, bold runs at odd offsets, code, tiny UI text, and
        // fallback-heavy chrome. Distinct from targets so the layout cache
        // can't serve them.
        let decoy_texts: Vec<(String, Font, f32)> = {
            let mut v: Vec<(String, Font, f32)> = Vec::new();
            v.push(("Strop is a writer’s editor with an editor inside — one that diagnoses.".into(), serif(), 20.));
            v.push(("Хороший редактор называет проблему — «здесь зарыта мысль» — и оставляет решение автору.".into(), serif(), 20.));
            v.push(("Гомогенизация голоса — не свойство модели, а свойство интерфейса.".into(), serif(), 20.));
            v.push(("The reader is right that something is wrong. It it it?".into(), serif(), 20.));
            v.push(("B  I  U  S  H  {}  ↺".into(), serif(), 14.));
            v.push(("fn main() { println!(\"привет\"); }".into(), mono(), 16.));
            v.push(("History  named  vs draft  Restore".into(), serif(), 13.));
            v.push(("Перо знает о бумаге больше, чем писатель о читателе.".into(), serif(), 20.));
            v.push(("Voice: within your normal range (5 texts) ✓".into(), serif(), 12.));
            v.push(("частота тире: +3.1σ от вашей нормы".into(), serif(), 12.));
            for i in 0..20 {
                v.push((format!("Без№{i} смешанный текст mixed line {i} с дефисами-и-тире — и цифрами 12{i}."), serif(), 20.));
                v.push((format!("Заголовок-приманка номер {i}"), sans_bold(), 24.));
            }
            v
        };

        let audit_target = |t: &Target| {
            let runs: Vec<TextRun> = t
                .runs
                .iter()
                .map(|(len, font)| TextRun {
                    len: *len,
                    font: font.clone(),
                    color: gpui::black(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                })
                .collect();
            let lines = ts
                .shape_text(SharedString::from(t.text), px(t.size), &runs, None, None)
                .expect("shape_text failed");
            let mut sig = String::new();
            for line in &lines {
                for run in &line.unwrapped_layout.runs {
                    let label = by_id
                        .iter()
                        .find(|(id, _, _)| *id == run.font_id.0)
                        .map(|(_, l, _)| *l);
                    sig.push_str(&format!("[{}#{}:", run.font_id.0, label.unwrap_or("?")));
                    for glyph in &run.glyphs {
                        let ch = t.text[glyph.index..].chars().next().unwrap_or('?');
                        let num = glyph_num(glyph.id);
                        sig.push_str(&format!(" {num}"));
                        // cmap check: only meaningful for our known faces and
                        // ligature-free chars (all Cyrillic + plain Latin).
                        if let Some((_, label, parsed)) =
                            by_id.iter().find(|(id, _, _)| *id == run.font_id.0)
                        {
                            if ch.is_alphanumeric() {
                                match parsed.glyph_index(ch) {
                                    Some(expected) if u32::from(expected.0) != num => {
                                        println!(
                                            "MISMATCH {} byte={} ch='{}' shaped={} cmap={} font={}#{}",
                                            t.label, glyph.index, ch, num, expected.0, run.font_id.0, label
                                        );
                                    }
                                    None => println!(
                                        "NO-CMAP {} ch='{}' font={}#{} (claimed face lacks the char!)",
                                        t.label, ch, run.font_id.0, label
                                    ),
                                    _ => {}
                                }
                            }
                        }
                    }
                    sig.push(']');
                }
            }
            println!("SIG {} = {}", t.label, sig);
        };

        let shape_decoys = |ts: &gpui::WindowTextSystem, decoys: &[(String, Font, f32)]| {
            for (text, font, size) in decoys {
                let run = TextRun {
                    len: text.len(),
                    font: font.clone(),
                    color: gpui::black(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let _ = ts.shape_text(
                    SharedString::from(text.clone()),
                    px(*size),
                    &[run],
                    Some(px(660.)),
                    None,
                );
            }
        };

        println!("=== order: {order} ===");
        match order {
            "decoys-first" => {
                shape_decoys(&ts, &decoy_texts);
                targets.iter().for_each(&audit_target);
            }
            _ => {
                targets.iter().for_each(&audit_target);
                shape_decoys(&ts, &decoy_texts);
                // Shape targets again post-decoys: the layout cache should
                // serve identical results — printed SIGs prove or refute it.
                println!("--- second pass (post-decoys, cache-served?) ---");
                targets.iter().for_each(&audit_target);
            }
        }
        println!("=== audit complete ===");
    }
}
