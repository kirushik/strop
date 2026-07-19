//! The app's colophon (docs/releasing.md §8), and the updater's only UI.

use std::time::{Duration, SystemTime};

use gpui::{
    AnyWindowHandle, App, Bounds, Context, Entity, FocusHandle, Focusable, IntoElement,
    MouseButton, MouseDownEvent, Pixels, Render, Window, WindowBounds, WindowHandle,
    WindowOptions, div, px, rgb, size,
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

pub fn update_text(state: &UpdateState, now: SystemTime) -> Option<String> {
    match state {
        UpdateState::Inert => None,
        UpdateState::Disabled => Some("update checks are off ([update] in config.toml)".into()),
        UpdateState::Idle { last_check: None } => Some("not checked yet".into()),
        UpdateState::Idle { last_check: Some(at) } => Some(age_text(*at, now)),
        UpdateState::Checking => Some("checking…".into()),
        UpdateState::Available { version } => Some(format!("{version} is out")),
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
}

impl Render for AboutWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let metrics = aux_window::metrics(window);
        let state = update::status();
        // Inert (dev build, store channel) shows no updater surface at all
        // (§5); Disabled keeps the row so its "checks are off" line can
        // point at the config, with the button muted.
        let show_updater = !matches!(state, UpdateState::Inert);
        let enabled = !matches!(state, UpdateState::Inert | UpdateState::Disabled);
        let status = update_text(&state, SystemTime::now());
        let notes = match &state {
            UpdateState::AppliedThisLaunch { notes_url, .. } => Some(notes_url.clone()),
            _ => None,
        };
        let datum = |text: String| div().font_family("PT Mono").text_size(px(11.)).child(text);
        let entity = cx.entity();
        let body = div()
            .size_full().bg(rgb(AUX_BG)).text_color(rgb(TEXT_COLOR)).font_family("PT Sans")
            .px(px(42.)).pt(px(32.)).pb(px(48.)).flex().flex_col()
            .child(div().flex().items_center().gap(px(18.))
                .child(icon(icons::STROP_MARK, 52., TEXT_COLOR))
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
            .when_some(notes, |d, url| d.child(datum(url).mt(px(5.)).text_color(rgb(LINK_COLOR))))
            .child(div().mt(px(18.)).font_family("PT Serif").text_size(px(12.)).line_height(px(18.))
                .child("© 2026 Kirill Pimenov")
                .child("This program comes with absolutely no warranty.")
                .child("This is free software — you may redistribute it under GPL-3.0-or-later."))
            .child(datum("COPYING".into()).mt(px(6.)).text_color(rgb(LINK_COLOR)))
            .child(datum("https://github.com/kirushik/strop".into()).text_color(rgb(LINK_COLOR)))
            .child(div().mt(px(16.)).mb(px(5.)).text_size(px(10.)).text_color(rgb(MUTED_COLOR))
                .child("THIRD-PARTY LICENSES"))
            .child(div().id("about-licenses").h(px(LICENSE_HEIGHT)).overflow_y_scroll()
                .p(px(10.)).border_1().border_color(rgb(RULE_COLOR)).bg(rgb(0xFFFDF9))
                .font_family("PT Mono").text_size(px(10.)).line_height(px(15.))
                .children(self.chunks.iter().cloned().map(|chunk| div().child(chunk))))
            .child(div().mt(px(12.)).text_size(px(11.)).child(backups_text(backup_count())))
            .child(div().mt(px(16.)).font_family("PT Serif").text_size(px(11.))
                .text_color(rgb(MUTED_COLOR)).child(format!(
                    "Set in PT Serif & PT Mono · typeset by Strop {}", env!("CARGO_PKG_VERSION"))));
        let content = div().key_context(aux_window::KEY_CONTEXT).track_focus(&self.focus_handle)
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
        aux_window::shell(content, metrics)
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
        for (state, expected) in cases { assert_eq!(update_text(&state, now).as_deref(), expected); }
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
