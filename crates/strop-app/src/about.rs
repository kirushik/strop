//! The app's colophon (docs/releasing.md §8), and the updater's only UI.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

use gpui::{
    AnyWindowHandle, App, Bounds, Context, Entity, FocusHandle, Focusable, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Render,
    Transformation, Window, WindowBounds, WindowHandle, WindowOptions, div, point, px,
    radians, rgb, size,
};
use gpui::prelude::*;

use crate::aux_window;
use crate::draw_guard::EntityUpdateExt as _;
use crate::editor::{Editor, EscapeMode};
use crate::icons::{self, icon};
use crate::theme::{AUX_BG, LINK_COLOR, MUTED_COLOR, RULE_COLOR, TEXT_COLOR};
use crate::update::{self, Channel, UpdateState};

const WIDTH: f32 = 660.;
const HEIGHT: f32 = 720.;
const LICENSE_HEIGHT: f32 = 210.;
const LICENSE_CHUNK_LINES: usize = 80;

mod pendulum {
    pub const LIMIT: f32 = 1.31;
    const W0: f32 = 7.;
    const ZETA: f32 = 0.10;

    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct State {
        pub theta: f32,
        pub omega: f32,
    }

    fn clamp(mut state: State) -> State {
        if state.theta >= LIMIT {
            state.theta = LIMIT;
            if state.omega > 0. {
                state.omega = 0.;
            }
        } else if state.theta <= -LIMIT {
            state.theta = -LIMIT;
            if state.omega < 0. {
                state.omega = 0.;
            }
        }
        state
    }

    pub fn step(mut state: State, dt: f32) -> State {
        let dt = dt.clamp(0., 1. / 30.);
        state.omega += (-W0 * W0 * state.theta.sin()
            - 2. * ZETA * W0 * state.omega) * dt;
        state.theta += state.omega * dt;
        state = clamp(state);
        if state.theta.abs() < 0.002 && state.omega.abs() < 0.004 {
            State::default()
        } else {
            state
        }
    }

    pub fn impulse(mut state: State, omega_add: f32) -> State {
        state.omega = (state.omega + omega_add).clamp(-6., 6.);
        clamp(state)
    }

    pub fn follow_grab(
        state: State,
        pointer: (f32, f32),
        pivot: (f32, f32),
        dt: f32,
    ) -> State {
        let theta = (pointer.0 - pivot.0)
            .atan2(pointer.1 - pivot.1)
            .clamp(-LIMIT, LIMIT);
        let omega = if dt > 0. { (theta - state.theta) / dt } else { 0. };
        clamp(State { theta, omega })
    }

    // gpui's rotate matrix is [[cos,-sin],[sin,cos]] about the element
    // origin, which carries the ring point (0,-26) to x = +26·sin θ —
    // so the compensating translation is tx = -26·sin θ (the original
    // spec's sign, derived under the opposite convention, doubled the
    // drift; the fixed-ring invariant below is the normative law).
    pub fn pivot_translation(theta: f32) -> (f32, f32) {
        let py = -26.;
        (py * theta.sin(), py - py * theta.cos())
    }
}

#[derive(Default)]
struct MarkMotion {
    state: pendulum::State,
    pivot: Option<(f32, f32)>,
    grabbed: bool,
    last_sample: Option<(f32, Instant)>,
    last_frame: Option<Instant>,
    last_window_x: Option<f32>,
}

pub use aux_window::{ToggleDecision, toggle_decision};

pub fn channel_text(channel: Channel) -> &'static str {
    match channel {
        Channel::GithubWin => "github-win",
        Channel::GithubMac => "github-mac",
        Channel::GithubWinPortable => "github-win-portable",
        Channel::GithubLinux => "github-linux",
        Channel::Flathub => "flathub",
        Channel::Deb => "deb",
        Channel::Rpm => "rpm",
        Channel::Dev => "development build",
    }
}

fn age_text(last_check: SystemTime, now: SystemTime) -> String {
    let secs = now.duration_since(last_check).unwrap_or_default().as_secs();
    if secs < 3600 {
        format!("checked {} min ago", secs / 60)
    } else {
        format!("checked {} h ago", secs / 3600)
    }
}

pub fn update_text(state: &UpdateState, channel: Channel, now: SystemTime) -> Option<String> {
    match state {
        UpdateState::Inert => None,
        UpdateState::Disabled => Some("update checks are off ([update] in config.toml)".into()),
        UpdateState::Idle { last_check: None } => Some("not checked yet".into()),
        UpdateState::Idle { last_check: Some(at) } => Some(age_text(*at, now)),
        UpdateState::Checking => Some("checking…".into()),
        UpdateState::Available { version } => match channel {
            Channel::Deb | Channel::Rpm | Channel::Flathub => Some(format!(
                "Strop {version} is available — it arrives through your package manager.")),
            _ => Some(format!("{version} is out")),
        },
        UpdateState::Staged { version } => {
            Some(format!("{version} downloaded — next launch gets it"))
        }
        UpdateState::AppliedThisLaunch { from, to, .. } => {
            Some(format!("updated {from} → {to} · what changed"))
        }
        UpdateState::Failed { attempted: Some(version), kept, .. } => {
            Some(format!("couldn't apply {version} — kept {kept}"))
        }
        UpdateState::Failed { attempted: None, kept, .. } => {
            Some(format!("couldn't check for updates — running {kept}"))
        }
    }
}

pub fn backups_text(count: usize) -> String {
    match count {
        0 => "No migration backups".into(),
        1 => "1 migration backup".into(),
        n => format!("{n} migration backups"),
    }
}

fn backup_count() -> usize {
    std::fs::read_dir(crate::paths::migration_backups_dir())
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "strop"))
        .count()
}

fn license_chunks() -> Vec<String> {
    include_str!("../../../assets/third-party-licenses.txt")
        .lines()
        .collect::<Vec<_>>()
        .chunks(LICENSE_CHUNK_LINES)
        .map(|lines| lines.join("\n"))
        .collect()
}

pub fn content_fit_height() -> f32 {
    HEIGHT
}

pub struct AboutWindow {
    focus_handle: FocusHandle,
    editor: Entity<Editor>,
    editor_window: AnyWindowHandle,
    chunks: Vec<String>,
    motion: Rc<RefCell<MarkMotion>>,
    reduce_motion: bool,
    config_pending: bool,
}

impl Focusable for AboutWindow {
    fn focus_handle(&self, _: &App) -> FocusHandle { self.focus_handle.clone() }
}

impl AboutWindow {
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let bounds = window.bounds();
        let editor = self.editor.clone();
        let editor_window = self.editor_window;
        window.remove_window();
        save_position(bounds);
        editor.update_checked(cx, |editor, _| editor.about_closed());
        aux_window::restore_editor_focus(&editor, editor_window, cx);
    }

    fn request_quit(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let editor = self.editor.clone();
        let _ = self.editor_window.update(cx, move |_, editor_window, cx| {
            editor_window.activate_window();
            editor.update_checked(cx, |editor, cx| {
                editor.request_quit(&crate::Quit, editor_window, cx);
            });
        });
    }


    fn follow_mark(&mut self, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        if self.reduce_motion { return; }
        let mut motion = self.motion.borrow_mut();
        if !motion.grabbed { return; }
        let Some(pivot) = motion.pivot else { return };
        let now = Instant::now();
        let dt = motion.last_sample
            .map(|(_, at)| now.duration_since(at).as_secs_f32())
            .unwrap_or(0.);
        motion.state = pendulum::follow_grab(
            motion.state,
            (f32::from(ev.position.x), f32::from(ev.position.y)),
            pivot,
            dt,
        );
        motion.last_sample = Some((motion.state.theta, now));
        motion.last_frame = Some(now);
        cx.notify();
    }

    fn release_mark(&mut self, cx: &mut Context<Self>) {
        let mut motion = self.motion.borrow_mut();
        if !motion.grabbed { return; }
        motion.grabbed = false;
        motion.last_sample = None;
        motion.last_frame = Some(Instant::now());
        cx.notify();
    }
}

impl Render for AboutWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        let bounds = window.bounds();
        if self.config_pending {
            // `open` is called while Editor is leased by its action handler,
            // and GPUI draws a new window synchronously. Stay inert for that
            // first draw, then sample the setting on the requested frame.
            self.config_pending = false;
            window.request_animation_frame();
        } else {
            self.reduce_motion = self.editor.read(cx).config.reduce_motion;
        }
        let reduce_motion = self.reduce_motion;
        {
            let mut motion = self.motion.borrow_mut();
            if reduce_motion {
                *motion = MarkMotion::default();
            } else {
                let window_x = f32::from(bounds.origin.x);
                if let Some(previous_x) = motion.last_window_x {
                    let dx = window_x - previous_x;
                    if dx != 0. { motion.state = pendulum::impulse(motion.state, -0.006 * dx); }
                }
                motion.last_window_x = Some(window_x);
                if !motion.grabbed {
                    if let Some(previous) = motion.last_frame {
                        motion.state = pendulum::step(
                            motion.state, now.duration_since(previous).as_secs_f32());
                    }
                    motion.last_frame = Some(now);
                }
                if motion.state != pendulum::State::default() {
                    window.request_animation_frame();
                }
            }
        }
        let metrics = aux_window::metrics(window);
        let state = update::status();
        // Inert (dev build, store channel) shows no updater surface at all
        // (§5); Disabled keeps the row so its "checks are off" line can
        // point at the config, with the button muted.
        let show_updater = !matches!(state, UpdateState::Inert);
        let enabled = !matches!(state, UpdateState::Inert | UpdateState::Disabled);
        let status = update_text(&state, update::channel(), SystemTime::now());
        let notes = match &state {
            UpdateState::AppliedThisLaunch { notes_url, .. } => Some(notes_url.clone()),
            _ => None,
        };
        // A link looks clickable, so it IS clickable (blue text that does
        // nothing is a lie in link's clothing). occlude + stop_propagation
        // punch through the shell's whole-window drag surface, exactly the
        // close button's arrangement.
        let link = |id: &'static str, text: String, url: String| {
            div().id(id).occlude().cursor_pointer()
                .font_family("PT Mono").text_size(px(11.))
                .text_color(rgb(LINK_COLOR))
                .hover(|d| d.text_color(rgb(TEXT_COLOR)))
                .on_mouse_down(MouseButton::Left, move |_: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    cx.open_url(&url);
                })
                .child(text)
        };
        let entity = cx.entity();
        let motion = self.motion.clone();
        let theta = motion.borrow().state.theta;
        let translation = pendulum::pivot_translation(theta);
        let mark = div().on_children_prepainted({
                let motion = motion.clone();
                move |bounds, _, _| {
                    if let Some(bounds) = bounds.first() {
                        motion.borrow_mut().pivot = Some((
                            f32::from(bounds.origin.x + bounds.size.width / 2.),
                            f32::from(bounds.origin.y)));
                    }
                }
            })
            .id("about-mark").occlude().size(px(52.)).flex_shrink_0()
            .on_mouse_down(MouseButton::Left, cx.listener(
                |this, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    if this.reduce_motion { return; }
                    let mut motion = this.motion.borrow_mut();
                    motion.grabbed = true;
                    let theta = motion.state.theta;
                    motion.last_sample = Some((theta, Instant::now()));
                }))
            .child(icon(icons::STROP_MARK, 52., TEXT_COLOR).with_transformation(
                Transformation::rotate(radians(theta)).with_translation(
                    point(px(translation.0), px(translation.1)))));
        let body = div()
            .size_full().bg(rgb(AUX_BG)).text_color(rgb(TEXT_COLOR)).font_family("PT Sans")
            .px(px(42.)).pt(px(32.)).pb(px(48.)).flex().flex_col()
            .child(div().flex().items_center().gap(px(18.))
                .child(mark)
                .child(div().font_family("PT Serif").text_size(px(34.)).child("Strop")))
            .child(div().mt(px(18.)).font_family("PT Mono").text_size(px(11.)).line_height(px(17.))
                .child(format!("version {}", env!("CARGO_PKG_VERSION")))
                .child(format!("commit {}", option_env!("STROP_GIT_HASH").unwrap_or("unknown")))
                .child(channel_text(update::channel())))
            .when(show_updater, |d| d.child(div().mt(px(16.)).pt(px(12.))
                .border_t_1().border_color(rgb(RULE_COLOR))
                .flex().items_center().justify_between()
                .child(status.unwrap_or_default())
                .child(div().id("about-check").px(px(10.)).py(px(5.)).rounded(px(4.))
                    .border_1().border_color(rgb(RULE_COLOR))
                    .text_color(rgb(if enabled { TEXT_COLOR } else { MUTED_COLOR }))
                    .when(enabled, |d| d.cursor_pointer().on_mouse_down(MouseButton::Left,
                        |_: &MouseDownEvent, _, _| update::check_now()))
                    .child("Check now"))))
            .when_some(notes, |d, url| d.child(
                link("about-notes", url.clone(), url).mt(px(5.))))
            .child(div().mt(px(18.)).font_family("PT Serif").text_size(px(12.)).line_height(px(18.))
                .child("© 2026 Kirill Pimenov")
                .child("This program comes with absolutely no warranty.")
                .child("This is free software — you may redistribute it under GPL-3.0-or-later."))
            .child(link("about-copying", "COPYING".into(),
                "https://github.com/kirushik/strop/blob/main/COPYING".into()).mt(px(6.)))
            .child(link("about-repo", "https://github.com/kirushik/strop".into(),
                "https://github.com/kirushik/strop".into()))
            .child(div().mt(px(16.)).mb(px(5.)).text_size(px(10.)).text_color(rgb(MUTED_COLOR))
                .child("THIRD-PARTY LICENSES"))
            .child(div().id("about-licenses").h(px(LICENSE_HEIGHT)).overflow_y_scroll()
                .p(px(10.)).border_1().border_color(rgb(RULE_COLOR)).bg(rgb(0xFFFDF9))
                .font_family("PT Mono").text_size(px(10.)).line_height(px(15.))
                .children(self.chunks.iter().cloned().map(|chunk| div().child(chunk)))
                // For the one reader in ten thousand who scrolls every license
                // to the floor: a quiet nod, in serif, where only they will
                // ever stand. Sought, never announced.
                .child(div().mt(px(15.)).font_family("PT Serif").text_color(rgb(MUTED_COLOR))
                    .child("You read all of it. The blade is yours.")))
            .child(div().mt(px(12.)).text_size(px(11.)).child(backups_text(backup_count())))
            .child(div().mt(px(16.)).font_family("PT Serif").text_size(px(11.))
                .text_color(rgb(MUTED_COLOR)).child(format!(
                    "Set in PT Serif & PT Mono · typeset by Strop {}", env!("CARGO_PKG_VERSION"))));
        let content = div().id("about-content")
            .key_context(aux_window::KEY_CONTEXT).track_focus(&self.focus_handle)
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if !*hovered { this.release_mark(cx); }
            }))
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                this.follow_mark(ev, cx);
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(
                |this, _: &MouseUpEvent, _, cx| this.release_mark(cx)))
            .on_mouse_up_out(MouseButton::Left, cx.listener(
                |this, _: &MouseUpEvent, _, cx| this.release_mark(cx)))
            .on_action(cx.listener(|this, _: &EscapeMode, window, cx| this.close(window, cx)))
            .on_action(cx.listener(|this, _: &crate::AboutStrop, window, cx| this.close(window, cx)))
            .on_action(cx.listener(|this, _: &crate::Quit, window, cx| this.request_quit(window, cx)))
            .size_full().bg(rgb(AUX_BG)).font_family("PT Sans").text_color(rgb(TEXT_COLOR))
            .child(aux_window::titlebar(
                "About Strop", None, metrics.client, "about-close", "about-close",
                move |_: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    entity.update_checked(cx, |this, cx| this.close(window, cx));
                },
            ))
            .child(body);
        let motion = self.motion.clone();
        let content = div().size_full().child(content);
        aux_window::shell_with_move(content, metrics, Some(Box::new(
            move |ev, window, _| {
                if reduce_motion { return; }
                let midpoint = f32::from(window.bounds().size.width) / 2.;
                let kick = if f32::from(ev.position.x) < midpoint { 1.8 } else { -1.8 };
                let mut motion = motion.borrow_mut();
                motion.state = pendulum::impulse(motion.state, kick);
                motion.last_frame = Some(Instant::now());
                window.request_animation_frame();
            })))
    }
}

pub fn open(
    editor: Entity<Editor>,
    editor_window: AnyWindowHandle,
    editor_bounds: Bounds<Pixels>,
    cx: &mut App,
) -> Option<WindowHandle<AboutWindow>> {
    let width = WIDTH + 2. * aux_window::CSD_GUTTER;
    let height = content_fit_height() + aux_window::HEADER_HEIGHT
        + 2. * aux_window::CSD_GUTTER;
    let displays = cx.displays();
    let editor_tuple = (
        f32::from(editor_bounds.origin.x), f32::from(editor_bounds.origin.y),
        f32::from(editor_bounds.size.width), f32::from(editor_bounds.size.height));
    let display = aux_window::containing_display(editor_tuple, &displays.iter().map(|display| {
        let bounds = display.bounds();
        (f32::from(bounds.origin.x), f32::from(bounds.origin.y),
         f32::from(bounds.size.width), f32::from(bounds.size.height))
    }).collect::<Vec<_>>())
        .and_then(|ix| displays.get(ix).cloned())
        .or_else(|| cx.primary_display());
    let work = display.as_ref().map(|display| display.bounds()).unwrap_or_else(|| {
        Bounds::centered(None, size(px(width), px(height)), cx)
    });
    let work_tuple = (f32::from(work.origin.x), f32::from(work.origin.y),
        f32::from(work.size.width), f32::from(work.size.height));
    let remembered = load_position().unwrap_or((
        f32::from(editor_bounds.origin.x) + 36., f32::from(editor_bounds.origin.y) + 36.));
    let (x, y, w, h) = aux_window::clamp_bounds(
        (remembered.0, remembered.1, width, height), work_tuple);
    cx.open_window(WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: gpui::point(px(x), px(y)),
            size: size(px(w), px(h)),
        })),
        display_id: display.as_ref().map(|display| display.id()),
        ..aux_window::window_options("About Strop")
    }, move |window, cx| {
        window.set_window_title("About Strop");
        let view = cx.new(|cx| AboutWindow {
            focus_handle: cx.focus_handle(), editor: editor.clone(), editor_window,
            chunks: license_chunks(),
            motion: Rc::new(RefCell::new(MarkMotion::default())),
            reduce_motion: true, config_pending: true,
        });
        window.focus(&view.focus_handle(cx), cx);
        let weak = view.downgrade();
        cx.spawn(async move |cx| loop {
            cx.background_executor().timer(Duration::from_secs(30)).await;
            if weak.update(cx, |_, cx| cx.notify()).is_err() { break; }
        }).detach();
        let close_editor = editor.clone();
        let restore_window = editor_window;
        window.on_window_should_close(cx, move |window, cx| {
            save_position(window.bounds());
            close_editor.update_checked(cx, |editor, _| editor.about_closed());
            aux_window::restore_editor_focus(&close_editor, restore_window, cx);
            true
        });
        view
    }).ok()
}

fn state_file() -> std::path::PathBuf { crate::paths::state_dir().join("about-window.json") }
fn load_position() -> Option<(f32, f32)> {
    serde_json::from_str(&std::fs::read_to_string(state_file()).ok()?).ok()
}
fn save_position(bounds: Bounds<Pixels>) {
    let path = state_file();
    if let Some(dir) = path.parent() { let _ = std::fs::create_dir_all(dir); }
    let pos = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
    if let Ok(json) = serde_json::to_string(&pos) { let _ = std::fs::write(path, json); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pendulum_impulse_decays_by_half_cycles_and_rests() {
        let mut state = pendulum::impulse(pendulum::State::default(), 0.9);
        let mut previous_sign = state.omega.signum();
        let mut peaks = Vec::new();
        // ~9 s to full rest at ζ=0.10: visible motion dies in 3-4 s, the
        // sub-pixel tail runs longer; 10 simulated seconds bounds it.
        for _ in 0..(10 * 60) {
            state = pendulum::step(state, 1. / 60.);
            let sign = state.omega.signum();
            if sign != 0. && sign != previous_sign {
                peaks.push(state.theta.abs());
                previous_sign = sign;
            }
        }
        assert!(peaks.len() >= 4);
        assert!(peaks.windows(2).all(|pair| pair[1] < pair[0]));
        assert_eq!(state, pendulum::State::default());
    }

    #[test]
    fn pendulum_dt_and_angle_clamps_are_hard() {
        let state = pendulum::State { theta: 0.4, omega: 2. };
        assert_eq!(pendulum::step(state, 2.), pendulum::step(state, 1. / 30.));
        let high = pendulum::step(
            pendulum::State { theta: pendulum::LIMIT, omega: 2. }, 0.);
        let low = pendulum::step(
            pendulum::State { theta: -pendulum::LIMIT, omega: -2. }, 0.);
        assert_eq!(high, pendulum::State { theta: pendulum::LIMIT, omega: 0. });
        assert_eq!(low, pendulum::State { theta: -pendulum::LIMIT, omega: 0. });
        let grabbed = pendulum::follow_grab(
            pendulum::State::default(), (100., 1.), (0., 0.), 1. / 60.);
        assert_eq!(grabbed, pendulum::State { theta: pendulum::LIMIT, omega: 0. });
    }

    #[test]
    fn pivot_composition_keeps_the_ring_fixed() {
        assert_eq!(pendulum::pivot_translation(0.), (0., 0.));
        for theta in [-1.1_f32, -0.2, 0.2, 1.1] {
            let (tx, ty) = pendulum::pivot_translation(theta);
            // gpui's [[cos,-sin],[sin,cos]] carries (0,-26) to
            // (+26·sin θ, -26·cos θ); the ring must land back on itself.
            let pivot = (26. * theta.sin() + tx, -26. * theta.cos() + ty);
            assert!(pivot.0.abs() < 0.01);
            assert!((pivot.1 + 26.).abs() < 0.01);
        }
        let theta = 0.2_f32;
        let (tx, _) = pendulum::pivot_translation(theta);
        // The bottom of the mark (0, +26) must actually travel — the
        // translation compensates the ring, not the whole element.
        assert!(-26. * theta.sin() + tx < 0.);
        assert!(pendulum::follow_grab(
            pendulum::State::default(), (10., 20.), (0., 0.), 1.).theta > 0.);
    }

    #[test]
    fn reduce_motion_integration_seam_stays_zero() {
        let mut motion = MarkMotion::default();
        motion.state = pendulum::impulse(motion.state, 1.8);
        let reduce_motion = true;
        if reduce_motion { motion = MarkMotion::default(); }
        assert_eq!(motion.state, pendulum::State::default());
        assert!(!motion.grabbed);
    }

    #[test]
    fn every_update_state_has_the_spec_string() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(7200);
        let cases = [
            (UpdateState::Inert, None),
            (UpdateState::Disabled, Some("update checks are off ([update] in config.toml)")),
            (UpdateState::Idle { last_check: None }, Some("not checked yet")),
            (UpdateState::Idle { last_check: Some(now - Duration::from_secs(120)) }, Some("checked 2 min ago")),
            (UpdateState::Checking, Some("checking…")),
            (UpdateState::Available { version: "0.3.2".into() }, Some("0.3.2 is out")),
            (UpdateState::Staged { version: "0.3.2".into() }, Some("0.3.2 downloaded — next launch gets it")),
            (UpdateState::AppliedThisLaunch { from: "0.3.1".into(), to: "0.3.2".into(), notes_url: "notes".into() }, Some("updated 0.3.1 → 0.3.2 · what changed")),
            (UpdateState::Failed { attempted: Some("0.3.2".into()), kept: "0.3.1".into(), reason: "x".into() }, Some("couldn't apply 0.3.2 — kept 0.3.1")),
            (UpdateState::Failed { attempted: None, kept: "0.3.1".into(), reason: "x".into() }, Some("couldn't check for updates — running 0.3.1")),
        ];
        for (state, expected) in cases {
            assert_eq!(update_text(&state, Channel::GithubLinux, now).as_deref(), expected);
        }
    }

    #[test]
    fn package_manager_available_state_names_the_delivery_path() {
        let state = UpdateState::Available { version: "0.3.2".into() };
        for channel in [Channel::Deb, Channel::Rpm, Channel::Flathub] {
            assert_eq!(update_text(&state, channel, SystemTime::UNIX_EPOCH).as_deref(),
                Some("Strop 0.3.2 is available — it arrives through your package manager."));
        }
    }

    #[test]
    fn backup_counts_and_fixed_fit_contract() {
        assert_eq!(backups_text(0), "No migration backups");
        assert_eq!(backups_text(1), "1 migration backup");
        assert_eq!(backups_text(7), "7 migration backups");
        assert_eq!(content_fit_height(), HEIGHT);
    }

    #[test]
    fn escape_closes_the_about_sheet() {
        assert_eq!(toggle_decision(true), ToggleDecision::CloseAndRestore);
    }

    #[test]
    fn second_about_dispatch_is_a_toggle_close() {
        assert_eq!(toggle_decision(false), ToggleDecision::Open);
        assert_eq!(toggle_decision(true), ToggleDecision::CloseAndRestore);
    }
}
