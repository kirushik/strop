# Upstream gpui_wgpu bug: persistent glyph corruption after a Wayland output scale change

Status: root cause isolated to the shared swash `ScaleContext` in
`gpui_wgpu::cosmic_text_system::render_glyph_image`; workaround shipped
in Strop as `patches/gpui_wgpu-scale-context.patch`, carried on the zed fork
(`patches/README.md`) — it was a vendored `vendor/gpui_wgpu` one-change patch
until 2026-06-19. Issue not yet filed upstream. This file is the issue draft
plus the full evidence trail.

## One-line summary

On Linux with the wgpu renderer, glyph rasterization is not a pure
function of `RenderGlyphParams`: after a font has been rasterized at
one device scale, rasterizing it at a new `font_size × scale_factor`
through the long-lived `swash_scale_context` produces subtly different
bitmaps (≈1px smaller, mis-hinted) than a fresh `ScaleContext` would.
The sprite atlas caches tiles by params, so these poisoned rasters
persist for the lifetime of the window — visible as wrong-size glyphs
substituted mid-word, window-wide, after a Wayland output scale change
(monitor migration between mixed-DPI displays, or `swaymsg output …
scale N` mid-session).

## Affected versions

- Reproduced on zed main `992f395c3d696d96946302cd0a8721a72954ba39`.
- First seen on tag v0.233.10 (gpui/gpui_platform split era).

## Symptoms

Move a window from a scale-2 output to a scale-1 output (or flip one
output's scale). Text shaped after the change renders with individual
glyphs at the wrong size mid-word ("Documen𝗍", "Exp𝗈rt", "Und𝗈"),
deterministically (byte-identical captures across runs), permanently
(only healed by re-visiting a scale whose rasters predate the change,
or restarting). Which glyphs corrupt depends on the *history* of
rasterizations in that window — adding or removing unrelated text
(e.g. a few list-marker glyphs) changes which other glyphs corrupt.
That history-dependence sent our first bisect down a wrong path; see
"Red herrings".

## Root cause (proven by instrumentation + intervention)

Instrumented `WgpuAtlas::get_or_insert_with` to log every tile
allocation with a content hash of the rasterized bytes, then compared
two boots of the same app on a headless sway rig:

- Boot A ("flip"): start on a scale-2 output, then flip the output to
  scale 1 → post-flip frames rasterize the UI's glyphs at
  `scale_factor: 1.0` keys.
- Boot B ("reference"): fresh start directly at scale 1 → the same
  glyphs rasterize at byte-identical `RenderGlyphParams`.

Result: of 328 scale-1 glyph tiles, **70 had different raster bytes
AND different raster dimensions** for identical params (e.g. 9x9 in
the reference, 8x8 after the flip; mostly 13px and 10px UI text).
Every mismatched tile was rasterized *after* the same font had been
scaled at the other device scale in the same process.

Intervention: replacing `self.swash_scale_context` with a fresh
`ScaleContext::new()` per `render_glyph_image` call makes the flip
boot byte-identical to the reference boot (our harness's absolute-
error metric drops from ~5,600/2,250 px to ≤6 px = sub-noise). With
the shared context restored, corruption returns, deterministic.

Conclusion: hidden state in swash's `ScaleContext` caches (hinting
data is the prime suspect) aliases across sizes/scales of the same
font. Whether the bug is in swash's cache keying or in how
`cosmic_text_system` reuses the context across scale changes is for
upstream to decide; the practical contract violation is that
`rasterize_glyph(params)` is not deterministic in `params`.

## Red herrings (so the issue thread doesn't repeat them)

- "Mixed WrappedLine + ShapedLine paints in one element" — our first
  bisect verdict. Moving the extra ShapedLine paints to a sibling
  element, then to the div text pipeline, produced *byte-identical*
  corrupted output: the paint route is irrelevant. The extra glyphs
  merely changed the rasterization history, which changed which tiles
  got poisoned.
- Atlas allocation: instrumented overlap detection found zero tile
  overlaps; etagere allocation is clean.
- Atlas keying: `RenderGlyphParams` hashes/compares all fields
  including `scale_factor` bits; no cross-scale key collisions.
- A Wayland window's first frames render at scale 1 before the
  surface `enter` event delivers the real scale, so a "scale-2"
  session already has a scale-1 raster era at startup — accounting
  for this is what isolated the post-flip rasters as the poisoned set.

## Repro harness (in the Strop repo)

- `scripts/wflip.sh` — deterministic failing test. One headless sway
  output (`WLR_BACKENDS=headless`), scale flipped 2→1→2 mid-session;
  oracle 1 byte-compares same-scale captures from the same process
  (state leakage), oracle 2 compares the post-flip capture against a
  fresh boot at that scale (the user-visible bug). Captures made
  deterministic via an env gate that freezes cursor blink and
  timestamps, isolated XDG dirs per boot.
- `scripts/wmigrate.sh` — the original two-output monitor-migration
  repro (same wl_surface scale path).
- Fixtures: `scripts/fixtures/flip-{list,footnote,plain}.md`.

## Workaround shipped in Strop

`patches/gpui_wgpu-scale-context.patch` — ONE change in
`render_glyph_image`: a fresh `ScaleContext` per call. Carried as a commit on
the zed fork the workspace pins (`patches/README.md`); until 2026-06-19 it was a
vendored copy of the crate (`vendor/gpui_wgpu`) plus a
`[patch."https://github.com/zed-industries/zed"]` override, both now removed.
Cost is a per-glyph hint-cache rebuild, paid only on sprite-atlas misses.
Suggested upstream fix: scope or key the scale context correctly (or reset it
on scale change); this patch is the maximally-naive version of that.
