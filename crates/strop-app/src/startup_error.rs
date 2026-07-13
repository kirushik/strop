//! A deliberately small stopgap for document-open failures.
//!
//! It is not an editor with an error painted over it: no document entity,
//! autosave loop, history, or misleading filename title exists. The longer-
//! term recovery/open design is tracked explicitly in docs/ROADMAP.md.

use std::io;
use std::path::{Path, PathBuf};

use gpui::{
    App, Bounds, Context, CursorStyle, IntoElement, MouseButton, MouseDownEvent, Render, SharedString,
    TitlebarOptions, Window, WindowBounds, WindowDecorations, WindowOptions, div, prelude::*, px,
    rgb, size,
};

use crate::theme::{BG_COLOR, ERROR, MUTED_COLOR, RULE_COLOR, TEXT_COLOR};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenOperation {
    Open,
    Import,
}

#[derive(Debug, Clone)]
pub struct OpenFailure {
    operation: OpenOperation,
    path: PathBuf,
    summary: SharedString,
    cause: SharedString,
    launch_error: Option<SharedString>,
}

impl OpenFailure {
    pub fn from_io(operation: OpenOperation, path: PathBuf, error: &io::Error) -> Self {
        let summary = friendly_summary(error).into();
        Self {
            operation,
            path,
            summary,
            cause: error.to_string().into(),
            launch_error: None,
        }
    }

    fn headline(&self) -> &'static str {
        match self.operation {
            OpenOperation::Open => "Couldn’t open this document",
            OpenOperation::Import => "Couldn’t import this document",
        }
    }

    fn launch(&mut self, path: &Path, cx: &mut Context<Self>) {
        let result = std::env::current_exe().and_then(|exe| {
            std::process::Command::new(exe).arg(path).spawn().map(|_| ())
        });
        match result {
            Ok(()) => cx.quit(),
            Err(error) => {
                self.launch_error = Some(format!("Couldn’t start Strop: {error}").into());
                cx.notify();
            }
        }
    }

    fn try_again(&mut self, _: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let path = self.path.clone();
        self.launch(&path, cx);
    }

    fn open_another(&mut self, _: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open another document".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.first()
            {
                this.update(cx, |view, cx| view.launch(path, cx)).ok();
            }
        })
        .detach();
    }

    fn button(
        &self,
        id: &'static str,
        label: &'static str,
        primary: bool,
        on_click: impl Fn(&mut Self, &MouseDownEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(id)
            .px(px(14.))
            .py(px(7.))
            .rounded(px(6.))
            .border_1()
            .border_color(rgb(if primary { TEXT_COLOR } else { RULE_COLOR }))
            .bg(rgb(if primary { TEXT_COLOR } else { BG_COLOR }))
            .text_color(rgb(if primary { BG_COLOR } else { TEXT_COLOR }))
            .cursor(CursorStyle::PointingHand)
            .hover(|d| d.opacity(0.82))
            .on_mouse_down(MouseButton::Left, cx.listener(on_click))
            .child(label)
    }
}

impl Render for OpenFailure {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let path = self.path.display().to_string();
        div()
            .id("startup-error")
            .size_full()
            .bg(rgb(BG_COLOR))
            .font_family("PT Serif")
            .text_size(px(14.))
            .text_color(rgb(TEXT_COLOR))
            .p(px(30.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(div().h(px(2.)).w(px(42.)).bg(rgb(ERROR)))
            .child(div().text_size(px(22.)).child(self.headline()))
            .child(div().text_color(rgb(MUTED_COLOR)).child(self.summary.clone()))
            .child(
                div()
                    .px(px(10.))
                    .py(px(8.))
                    .rounded(px(5.))
                    .border_1()
                    .border_color(rgb(RULE_COLOR))
                    .font_family("PT Mono")
                    .text_size(px(12.))
                    .child(path),
            )
            .child(div().text_size(px(12.)).text_color(rgb(MUTED_COLOR)).child(self.cause.clone()))
            .when_some(self.launch_error.clone(), |root, error| {
                root.child(div().text_color(rgb(ERROR)).child(error))
            })
            .child(
                div()
                    .mt_auto()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(self.button("startup-retry", "Try Again", true, Self::try_again, cx))
                    .child(self.button(
                        "startup-open-another",
                        "Open Another…",
                        false,
                        Self::open_another,
                        cx,
                    ))
                    .child(
                        div()
                            .id("startup-close")
                            .ml_auto()
                            .px(px(10.))
                            .py(px(7.))
                            .text_color(rgb(MUTED_COLOR))
                            .cursor(CursorStyle::PointingHand)
                            .hover(|d| d.text_color(rgb(TEXT_COLOR)))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.quit())
                            .child("Close"),
                    ),
            )
    }
}

fn friendly_summary(error: &io::Error) -> &'static str {
    match error.kind() {
        io::ErrorKind::NotFound => "The file is no longer at this location.",
        io::ErrorKind::PermissionDenied => "Strop does not have permission to read this file.",
        io::ErrorKind::InvalidData => "The file is damaged or is not a readable Strop document.",
        io::ErrorKind::Unsupported => "This document was written by a newer version of Strop.",
        _ => "Strop could not read the file safely.",
    }
}

pub fn show(failure: OpenFailure, cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(560.), px(300.)), cx);
    let window = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Couldn’t open — Strop".into()),
                ..Default::default()
            }),
            focus: true,
            is_resizable: false,
            window_decorations: Some(WindowDecorations::Server),
            ..Default::default()
        },
        |_, cx| cx.new(|_| failure),
    );
    if let Err(error) = window {
        eprintln!("strop: could not show the open-error window: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_copy_names_the_useful_failure_class() {
        assert!(friendly_summary(&io::Error::from(io::ErrorKind::NotFound)).contains("no longer"));
        assert!(
            friendly_summary(&io::Error::from(io::ErrorKind::PermissionDenied))
                .contains("permission")
        );
        assert!(
            friendly_summary(&io::Error::from(io::ErrorKind::Unsupported)).contains("newer")
        );
    }
}
