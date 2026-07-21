//! The icon plate (docs/iconography.md): the app's few pictorial marks as
//! embedded single-color SVGs, rasterized by gpui's own resvg pipeline and
//! tinted with the element's text color. This path never touches the font
//! system, so it is immune to the garbled-glyph bug class that forbade
//! non-PT glyph labels (the rule the old drawn-div idiom worked around).
//!
//! Two formal families share one hand:
//!   - PICTORIAL (round caps, 2.25 stroke on the 24 grid): document things —
//!     the history clock, the menu, the note card, the headstone, the chain.
//!   - WINDOW (butt caps, 2.1 stroke): the OS verbs as pure geometry — line,
//!     outline square, saltire. Pictures mean the document; bare geometry
//!     means the window; the two never borrow from each other.
//!
//! Icons carry FORM only. Color stays the element's decision (muted at
//! rest, ink when active, LINK_COLOR on the link mark) so the color
//! language (color-language.md) keeps speaking through one throat.

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString, Styled, px, rgb, svg};

/// The embedded icon table: `include_bytes!` like the PT fonts, so a
/// missing file is a compile error, not a blank control at runtime.
static ICONS: &[(&str, &[u8])] = &[
    ("icon/strop-mark.svg", include_bytes!("../../../assets/icon/strop-mark.svg")),
    ("icon/strop-mark-mono.svg", include_bytes!("../../../assets/icon/strop-mark-mono.svg")),
    ("icons/book.svg", include_bytes!("../../../assets/icons/book.svg")),
    ("icons/caret-down.svg", include_bytes!("../../../assets/icons/caret-down.svg")),
    ("icons/dismiss.svg", include_bytes!("../../../assets/icons/dismiss.svg")),
    ("icons/grave.svg", include_bytes!("../../../assets/icons/grave.svg")),
    ("icons/history.svg", include_bytes!("../../../assets/icons/history.svg")),
    ("icons/link.svg", include_bytes!("../../../assets/icons/link.svg")),
    ("icons/menu.svg", include_bytes!("../../../assets/icons/menu.svg")),
    ("icons/note.svg", include_bytes!("../../../assets/icons/note.svg")),
    ("icons/squiggle.svg", include_bytes!("../../../assets/icons/squiggle.svg")),
    ("icons/win-close.svg", include_bytes!("../../../assets/icons/win-close.svg")),
    ("icons/win-maximize.svg", include_bytes!("../../../assets/icons/win-maximize.svg")),
    ("icons/win-minimize.svg", include_bytes!("../../../assets/icons/win-minimize.svg")),
];

// Path constants so a typo is a compile error at the call site, not a
// silently empty svg().
pub const BOOK: &str = "icons/book.svg";
// The mono silhouette — svg() paints one color, so About takes the
// ink variant; the full-color master is the OS-icon pipeline's input.
pub const STROP_MARK: &str = "icon/strop-mark-mono.svg";
pub const CARET_DOWN: &str = "icons/caret-down.svg";
pub const DISMISS: &str = "icons/dismiss.svg";
pub const GRAVE: &str = "icons/grave.svg";
pub const HISTORY: &str = "icons/history.svg";
pub const LINK: &str = "icons/link.svg";
pub const MENU: &str = "icons/menu.svg";
pub const NOTE: &str = "icons/note.svg";
pub const SQUIGGLE: &str = "icons/squiggle.svg";
pub const WIN_CLOSE: &str = "icons/win-close.svg";
pub const WIN_MAXIMIZE: &str = "icons/win-maximize.svg";
pub const WIN_MINIMIZE: &str = "icons/win-minimize.svg";

/// The app's asset source (registered in main.rs via `with_assets`). Only
/// icons live here — fonts and runtime data keep their existing loaders.
pub struct StropAssets;

impl AssetSource for StropAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(ICONS
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(ICONS
            .iter()
            .filter(|(name, _)| name.starts_with(path))
            .map(|(name, _)| SharedString::from(*name))
            .collect())
    }
}

/// One icon, sized and inked. gpui tints the whole SVG with the text
/// color — the mark is a form, never a palette.
pub fn icon(path: &'static str, size: f32, color: u32) -> gpui::Svg {
    svg()
        .path(path)
        .size(px(size))
        .flex_shrink_0()
        .text_color(rgb(color))
}
