//! Draw-pass discipline, mechanically enforced.
//!
//! The rule (docs/VISUAL-RIG.md, extracted from the 2026-06-12 Wayland
//! corruption hunt): NEVER mutate app state from inside a draw pass. An
//! `Entity::update` whose closure calls `cx.notify()` during element
//! request_layout/prepaint/paint — or inside a `canvas` closure — re-dirties
//! the window mid-frame; under Wayland's frame-callback scheduling that
//! tears the renderer's per-frame bookkeeping. The two sanctioned patterns:
//!
//! 1. geometry capture through an `Rc<RefCell<…>>` (see
//!    `Editor::zone_row_bounds`), or
//! 2. [`EntityUpdateExt::update_in_draw`] — an entity write whose closure
//!    receives `&mut T` ONLY: with no `Context` in scope, `cx.notify()` is
//!    unreachable by construction.
//!
//! Everything else goes through [`EntityUpdateExt::update_checked`], which
//! debug-asserts that no draw pass is on the stack. Raw
//! `gpui::Entity::update` / `gpui::canvas` are banned crate-wide by
//! `clippy.toml` (`disallowed-methods`) + `#![deny(clippy::disallowed_methods)]`.
//!
//! The IN_DRAW flag is maintained by [`DrawGuard`] RAII instances at the
//! top of every custom element's request_layout/prepaint/paint
//! (EditorElement, NoteInputElement) and inside [`capture_canvas`]'s
//! closure wrappers. Thread-local because draw passes are main-thread.

use std::cell::Cell;

use gpui::{App, AppContext, Bounds, Canvas, Context, Entity, Pixels, Window};

thread_local! {
    static IN_DRAW: Cell<bool> = const { Cell::new(false) };
}

/// Is a draw pass (element layout/prepaint/paint or canvas closure)
/// currently on this thread's stack?
pub fn in_draw() -> bool {
    IN_DRAW.with(|c| c.get())
}

/// RAII marker for "we are inside a draw pass". Nestable (elements draw
/// children); restores the previous value on drop, panic-safe.
pub struct DrawGuard {
    prev: bool,
}

impl DrawGuard {
    #[must_use = "the guard must live for the duration of the draw method"]
    pub fn enter() -> Self {
        Self {
            prev: IN_DRAW.with(|c| c.replace(true)),
        }
    }
}

impl Drop for DrawGuard {
    fn drop(&mut self) {
        IN_DRAW.with(|c| c.set(self.prev));
    }
}

/// The crate's two sanctioned ways to write to an entity. See module docs.
pub trait EntityUpdateExt<T: 'static> {
    /// `Entity::update` for everywhere EXCEPT draw passes (event handlers,
    /// action handlers, spawned task callbacks). Debug-asserts that no
    /// draw pass is on the stack — catching by test what the clippy ban
    /// can't see through indirection.
    fn update_checked<R, C: AppContext>(
        &self,
        cx: &mut C,
        f: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R;

    /// The only sanctioned entity write DURING a draw pass: the closure
    /// gets `&mut T` alone — no `Context`, so `cx.notify()` (the
    /// mid-frame re-dirty that caused the corruption) cannot be expressed.
    /// For plain state mirrors like `Editor::last_frame`.
    fn update_in_draw<R, C: AppContext>(&self, cx: &mut C, f: impl FnOnce(&mut T) -> R) -> R;
}

/// The assertion behind [`EntityUpdateExt::update_checked`], extracted so
/// the unit test can exercise it without constructing an `App`.
fn assert_not_in_draw() {
    debug_assert!(
        !in_draw(),
        "Entity::update inside a draw pass — a notify here re-dirties the window \
         mid-frame (the 2026-06-12 Wayland corruption class). Capture geometry via \
         Rc<RefCell> or use update_in_draw (docs/VISUAL-RIG.md)."
    );
}

impl<T: 'static> EntityUpdateExt<T> for Entity<T> {
    #[allow(clippy::disallowed_methods)] // the wrapper the ban points to
    fn update_checked<R, C: AppContext>(
        &self,
        cx: &mut C,
        f: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> R {
        assert_not_in_draw();
        self.update(cx, f)
    }

    #[allow(clippy::disallowed_methods)] // the wrapper the ban points to
    fn update_in_draw<R, C: AppContext>(&self, cx: &mut C, f: impl FnOnce(&mut T) -> R) -> R {
        self.update(cx, |this, _| f(this))
    }
}

/// `gpui::canvas`, with both closures running under a [`DrawGuard`] so a
/// stray `update_checked` inside them trips the assertion. Use for every
/// bounds-capture canvas (e.g. the footnote zone's row-geometry probe).
#[allow(clippy::disallowed_methods)] // the wrapper the ban points to
pub fn capture_canvas<T: 'static>(
    prepaint: impl FnOnce(Bounds<Pixels>, &mut Window, &mut App) -> T + 'static,
    paint: impl FnOnce(Bounds<Pixels>, T, &mut Window, &mut App) + 'static,
) -> Canvas<T> {
    gpui::canvas(
        move |bounds, window, cx| {
            let _guard = DrawGuard::enter();
            prepaint(bounds, window, cx)
        },
        move |bounds, state, window, cx| {
            let _guard = DrawGuard::enter();
            paint(bounds, state, window, cx)
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_nests_and_restores() {
        assert!(!in_draw());
        {
            let _outer = DrawGuard::enter();
            assert!(in_draw());
            {
                let _inner = DrawGuard::enter();
                assert!(in_draw());
            }
            assert!(in_draw());
        }
        assert!(!in_draw());
    }

    #[test]
    #[should_panic(expected = "inside a draw pass")]
    fn checked_update_asserts_in_draw() {
        let _guard = DrawGuard::enter();
        assert_not_in_draw();
    }

    #[test]
    fn checked_update_quiet_outside_draw() {
        assert_not_in_draw();
    }
}
