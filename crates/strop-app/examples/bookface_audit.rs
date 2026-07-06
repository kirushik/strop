//! CFF/OTF smoke check for the cold read's book face (Wave A gate,
//! impl 05 §3.1): strop has only ever bundled TTFs; URW Bookman ships CFF
//! outlines. Register the bundled OTFs the same way main.rs does (runtime
//! file reads — never include_bytes!, the AGPL mere-aggregation posture),
//! shape an English and a Russian sentence in "URW Bookman", and prove the
//! face RESOLVED rather than silently fell back:
//!
//!   - every shaped run's FontId must be URW Bookman's, not PT Serif's;
//!   - per-word widths must be non-zero and differ from PT Serif's widths
//!     for the same strings (identical widths = fallback = fail loudly);
//!   - Demi must measure wider than Light for the same string.
//!
//! Also prints a real shape_line timing (500 distinct words) — the
//! entry-pagination budget check the fake-measurer microbench can't give.
//!
//!   cargo run -p strop-app --example bookface_audit
//!
//! Caveat recorded per recon §3: on machines with fonts-urw-base35
//! installed, "URW Bookman" may also resolve from the system set (same
//! upstream file); the CFF-shaping proof holds either way.

use gpui::{Font, FontWeight, SharedString, TextRun, prelude::*, px};

const PT_SERIF: &[u8] = include_bytes!("../../../assets/fonts/PTSerif-Regular.ttf");

/// The example's stand-in for paths::asset_file (examples are their own
/// crates): the repo's assets/ relative to this crate's manifest.
fn asset(rel: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .join(rel);
    std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

fn face(family: &'static str, weight: FontWeight, italic: bool) -> Font {
    let mut font = gpui::font(family);
    font.weight = weight;
    if italic {
        font.style = gpui::FontStyle::Italic;
    }
    font
}

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

fn main() {
    gpui_platform::application().run(move |cx: &mut gpui::App| {
        cx.text_system()
            .add_fonts(vec![
                PT_SERIF.into(),
                asset("fonts/coldread/URWBookman-Light.otf").into(),
                asset("fonts/coldread/URWBookman-LightItalic.otf").into(),
                asset("fonts/coldread/URWBookman-Demi.otf").into(),
            ])
            .expect("failed to register fonts");

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
            .update(cx, |_, window, _| run_audit(window))
            .expect("audit window update");
        cx.spawn(async move |cx| {
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
}

fn shape_words(
    ts: &gpui::WindowTextSystem,
    font: &Font,
    sentence: &str,
) -> Vec<(String, f32, Vec<usize>)> {
    sentence
        .split_whitespace()
        .map(|w| {
            let run = TextRun {
                len: w.len(),
                font: font.clone(),
                color: gpui::black(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let line = ts.shape_line(SharedString::from(w.to_owned()), px(16.5), &[run], None);
            let fonts = line.runs.iter().map(|r| r.font_id.0).collect();
            (w.to_owned(), f32::from(line.width), fonts)
        })
        .collect()
}

fn run_audit(window: &mut gpui::Window) {
    let ts = window.text_system().clone();
    let bookman = face("URW Bookman", FontWeight::LIGHT, false);
    let bookman_demi = face("URW Bookman", FontWeight::SEMIBOLD, false);
    let serif = face("PT Serif", FontWeight::NORMAL, false);
    let id_bookman = ts.resolve_font(&bookman).0;
    let id_serif = ts.resolve_font(&serif).0;
    println!("resolve: URW Bookman Light -> FontId({id_bookman}), PT Serif -> FontId({id_serif})");
    assert_ne!(id_bookman, id_serif, "URW Bookman resolved to PT Serif — the face did not register");

    let en = "The quick brown fox jumps over the lazy typographer";
    let ru = "Хороший редактор называет проблему и оставляет решение автору";
    let mut failures = 0;
    for (label, sentence) in [("EN", en), ("RU", ru)] {
        let book = shape_words(&ts, &bookman, sentence);
        let pt = shape_words(&ts, &serif, sentence);
        let mut differing = 0;
        for ((w, bw, bfonts), (_, pw, _)) in book.iter().zip(&pt) {
            println!("  {label} {w:<14} bookman {bw:8.3}px  ptserif {pw:8.3}px  runs {bfonts:?}");
            if *bw <= 0.0 {
                println!("FAIL: zero width for {w:?} in URW Bookman");
                failures += 1;
            }
            if (bw - pw).abs() > 0.01 {
                differing += 1;
            }
            for f in bfonts {
                if *f != id_bookman {
                    println!(
                        "FAIL: {label} word {w:?} shaped with FontId({f}) — fallback, not URW Bookman"
                    );
                    failures += 1;
                }
            }
        }
        if differing == 0 {
            println!("FAIL: {label} widths identical to PT Serif — the face fell back");
            failures += 1;
        }
    }

    // Demi is a real second style, not a synthetic bold of Light.
    let light_w = f32::from(
        ts.shape_line(
            SharedString::from("справедливость"),
            px(16.5),
            &[TextRun {
                len: "справедливость".len(),
                font: bookman.clone(),
                color: gpui::black(),
                background_color: None,
                underline: None,
                strikethrough: None,
            }],
            None,
        )
        .width,
    );
    let demi_w = f32::from(
        ts.shape_line(
            SharedString::from("справедливость"),
            px(16.5),
            &[TextRun {
                len: "справедливость".len(),
                font: bookman_demi.clone(),
                color: gpui::black(),
                background_color: None,
                underline: None,
                strikethrough: None,
            }],
            None,
        )
        .width,
    );
    println!("  demi check: Light {light_w:.3}px vs Demi {demi_w:.3}px");
    if demi_w <= light_w {
        println!("FAIL: Demi no wider than Light — the Demi style did not resolve");
        failures += 1;
    }

    // Real shape_line cost for the entry budget (research §7 cost model):
    // 500 distinct words, cold cache.
    let t0 = std::time::Instant::now();
    for i in 0..500 {
        let w = format!("слово{i}word");
        let run = TextRun {
            len: w.len(),
            font: bookman.clone(),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let _ = ts.shape_line(SharedString::from(w), px(16.5), &[run], None);
    }
    let per = t0.elapsed() / 500;
    println!("  shape_line timing: {per:?}/word over 500 distinct bilingual words");

    if failures > 0 {
        println!("=== bookface audit FAILED ({failures} failures) ===");
        std::process::exit(1);
    }
    println!("=== bookface audit PASSED: CFF/OTF URW Bookman shapes and resolves ===");
}
