# Vendored gpui 0.2.2 + backported text fixes

This is the gpui 0.2.2 crates.io source (the last release ever published
there; zed moved on without publishing) with upstream fixes backported.
Strop hit migrating glyph corruption — wrong glyphs whose location shifted
when style runs changed, plus per-glyph vertical jitter in small UI text —
that survived a full font-family replacement, proving the bug was in the
text pipeline, not the fonts. `strop-app --example shape_audit` showed
isolated shaping was cmap-exact and order-independent, narrowing it to the
layers these patches touch.

Applied, in order:

1. **zed PR #41224** — "Fix `TextLayout::layout` producing invalid text
   runs". The measured-layout closure runs more than once; it captured the
   run list mutably and `truncate_line` truncated it in place, so every
   re-measure of truncated text (e.g. `.truncate()` history rows) used
   already-mangled runs: broken byte boundaries, garbage glyphs, baseline
   jitter. Runs are now threaded immutably (`Cow`), and
   `request_measured_layout` requires `Fn`, not `FnMut`.
   Files: `src/elements/text.rs`, `src/text_system.rs`,
   `src/text_system/line_wrapper.rs`, `src/window.rs`, `src/styled.rs`.

2. **zed PR #43856** — "Further fix extraction of font runs from text
   runs". In `layout_line`, a stale `last_font` shortcut coalesced a run
   with a *different font* into the previous font run whenever decorations
   matched — weight/face changes between runs were silently dropped (the
   shape_text twin of this bug was fixed by #40840, which 0.2.2 already
   has). File: `src/text_system.rs`.

3. **zed PR #48504 (adapted)** — cosmic-text 0.14 → 0.17.2. The fixes live
   in cosmic-text itself: fallback may use faces with mismatched stretch or
   style (pre-0.17 it silently picked a wrong font), and the ASCII fast
   path is skipped for words with incompatible spans (pre-0.16 it emitted
   wrong glyphs when a style run boundary cut through a word). Adaptation
   beyond the PR (whose base had already replaced 0.2.2's raster path):
   `get_font(id, Weight::NORMAL)` and `CacheKey::new(..,
   Weight::NORMAL, ..)` — we bundle real faces for every weight we use and
   never request synthetic bold, so `NORMAL` (no synthesis) is correct at
   both load and raster sites. `Hinting::Disabled` matches upstream.
   Files: `Cargo.toml`, `src/platform/linux/text_system.rs`.

Also removed: `examples/`, `tests/`, `docs/` and their manifest targets
(the published crate's examples reference dev-deps it doesn't ship).

NOT backported (post-rewrite architecture, would be a rewrite not a patch):
- PR #54878 — proper `font_fallbacks` wiring with per-span codepoint
  coverage (the canonical Linux fallback fix). Mitigated app-side instead:
  Strop's UI chrome no longer uses characters outside the bundled PT fonts
  (drawn divs replace ○●↑↓↺□✕✓→), so fallback only engages for document
  content the user types.
- PR #45423 / #46857 — subpixel rasterization rewrite + HiDPI emoji fix.

Exit strategy: a git pin on zed-industries/zed after the gpui/gpui_platform
split (tracked in docs/ROADMAP.md).
