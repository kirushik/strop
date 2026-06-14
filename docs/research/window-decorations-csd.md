# Window boundary / shadow — cross-platform CSD research (2026-06-14)

> Why Strop's window has no visible edge on Wayland, what "best practice"
> actually is per platform, and exactly what gpui (pinned rev `992f395`)
> gives us to fix it.
>
> **Status: IMPLEMENTED (2026-06-14); shadow REVISED (2026-06-15).** `editor.rs`
> `render` wraps the content in the shadow gutter under `Decorations::Client`;
> `main.rs` sets `WindowBackgroundAppearance::Transparent`. Verified on a
> *floating* sway window with a light desktop (the headless rig tiles by default
> and paints black by default — `WSHOT_FLOAT=WxH` floats it, `WSHOT_BG=#f1eee7`
> gives a light desktop so the shadow is actually visible).
>
> **Shadow v1 was wrong** (Kirill, 2026-06-15: "too thick, too omnidirectional,
> gradient feels odd, corner artifacts"). Root cause: a *single* `BoxShadow` at
> offset (0,2), blur 10, alpha **0.35**, in a gutter that was *also* 10px — so the
> blur was clipped at the surface edge into a hard slab, and a lone 0.35 layer with
> a near-zero offset haloed uniformly instead of grounding the window. **v2** (the
> shadow-theory research, `docs/research/` siblings + sources in this file's spirit):
> three layered shadows — contact `(0,1) blur 2 @0.14` + cast `(0,3) blur 8 @0.10`
> + ambient `(0,6) blur 14 @0.07` — all biased downward (light from above), summing
> *softer* than the old single slab; the gutter widened to **22px** (`CSD_GUTTER`)
> so the 20px-deep ambient layer never clips; and the **content node is rounded to
> the same radius** as the border so its background can't poke a square corner
> through the arc (the corner artifact — same class as Zed #58162). Restrained,
> GNOME/libadwaita-scale rather than macOS-scale halo.
>
> Every frame-anchored overlay (margin card, gutter toolbar, pill) still
> pixel-aligned to the column — `set_client_inset` keeps the geometry correct,
> exactly as §5 predicted. One deviation: the resize affordance reuses the existing
> discrete `resize_handles` strips on the outer backdrop rather than Zed's
> `canvas`-based cursor tracking, because the project bans raw `canvas` (draw-pass
> discipline) — so the shadow band resizes but shows no resize cursor. (Kirill chose
> rounded corners and NO grip glyph, 2026-06-15.)
>
> **Resize grab fix (2026-06-15):** the strips were `RESIZE_INSET` (8px) thick at
> the *surface* edge — i.e. only the outer 8px of the 22px shadow gutter. The
> visible window border (a gutter-width in) had no grab region, so you could only
> resize by catching the faint outer fringe of the shadow (Kirill). The bands now
> span `CSD_GUTTER (+ RESIZE_INSET inner lip)` from the surface edge, so the whole
> gutter through the visible border is draggable; the TOP band stays gutter-only so
> it never steals clicks from the titlebar/window controls just inside it.

## 1. The symptom

On the dev rig (sway / Wayland) the window blends into a light background:
no shadow, no border, no rounded corners. The boundary is invisible. (Image
#2 in the 2026-06-14 nits thread: a neighbouring native window casts a clear
drop shadow; Strop casts none.)

## 2. Current state (verified in-repo)

- `main.rs:~234` — `WindowDecorations::Client`, `TitlebarOptions { title }`.
  `window_background` is left at its default (**Opaque**).
- The window draws its **own** 36px titlebar (`editor.rs:render_titlebar`)
  and its **own** 8px invisible resize bands on every edge/corner
  (`editor.rs:resize_strip` / `resize_handles`, gated on
  `Decorations::Client`).
- Content is **flush to the window edge** — no inset, opaque, no shadow/
  border/rounding anywhere on the root.

## 3. Why it's invisible — the actual mechanism

On Wayland, decoration is negotiated (`xdg-decoration`,
`zxdg_decoration_manager_v1`): the compositor may do **server-side
decorations (SSD)** or hand the client **client-side decorations (CSD)**.
GNOME's Mutter and wlroots compositors (sway) do **not** draw SSD for normal
toplevels — the app gets CSD and is responsible for its **own** shadow,
rounded corners, and border. Strop already knows this (the H2 comment in
`main.rs`: "GNOME Wayland does no server-side decorations").

The trap: a gpui element `box-shadow` is clipped to the **window surface**.
An *outer* drop shadow needs space *outside* the content to live in. With an
opaque, edge-to-edge window there is no such space, so `.shadow_lg()` on the
root paints nothing visible beyond the edge. **To cast an outer shadow the
window surface must be larger than the content, with a transparent margin —
the "shadow gutter" — and the shadow painted into it.** This is exactly how
GTK/libadwaita and Qt draw CSD shadows, and how Zed draws its own on Linux.

## 4. Best practice is per-platform (do NOT draw one shadow everywhere)

| Platform | Decoration | What to draw |
|---|---|---|
| macOS | Server-side | **Nothing.** Free shadow + rounded corners. Drawing your own double-shadows and looks wrong. |
| Windows | Server-side (DWM) | **Nothing.** DWM draws the shadow. |
| Linux X11 | WM-side | **Nothing** (the WM decorates). |
| Linux Wayland + SSD | Server-side | **Nothing** (rare for our targets, but possible). |
| Linux Wayland + CSD | Client-side | **Draw it:** transparent gutter + soft shadow + rounded corners + 1px border. GNOME/sway land here. |

So the feature is **conditional**, keyed on the live decoration mode we
already branch on for resize handles: `Decorations::Client { tiling }` →
draw; `Decorations::Server` → draw nothing.

## 5. What gpui rev `992f395` gives us (verified by grep)

There is **no** turnkey `client_side_decorations` helper at this rev
(searched the whole checkout — none). Zed builds its CSD chrome from
primitives; so will we. The primitives that exist:

- `WindowOptions.window_background: WindowBackgroundAppearance`
  (`Opaque | Transparent | Blurred`) — set **Transparent** so the gutter
  shows through. (`window.rs` sets `set_background_appearance` at init.)
- `window.set_client_inset(Pixels)` / `window.client_inset()`
  (`window.rs:2315`) — tells the platform window how big the CSD inset is so
  **hit-testing and coordinates account for it**. This is the answer to the
  H2 worry ("a reserved inset would shift every overlay's window-origin
  coordinates") — the platform owns the inset, gpui's geometry stays correct.
- `window.paint_drop_shadows(bounds, corner_radii, &[BoxShadow])`
  (`window.rs:3627`) + element `box-shadow` + `Corners<Pixels>` rounding —
  the actual drawing.
- `Decorations::Client { tiling }` (already consumed at `editor.rs:~8894`) —
  `tiling` says which edges are snapped, so we know where to **drop** the
  inset/rounding (no shadow on a tiled/maximized edge — GTK behaviour).

## 6. Recommended implementation (the later pass)

1. `window_background: WindowBackgroundAppearance::Transparent` in
   `WindowOptions`.
2. Only under `Decorations::Client`, wrap the root in a **shadow gutter**:
   an outer transparent container; inner content inset by `SHADOW_PAD`
   (≈16–24px) on each *non-tiled* edge; `window.set_client_inset(pad)`.
3. Inner content: `rounded(px(8.))`, `border_1()` hairline (`RULE_COLOR`),
   and a soft `box_shadow` (low-alpha black, blur ≈24, y-offset ≈8).
4. **Tiling-aware:** for each edge in `tiling`, set that edge's pad/rounding
   to 0 (a snapped edge has no shadow). Maximized → no gutter at all.
5. Move the existing 8px resize bands into the gutter (they already exist;
   just re-origin them).
6. `Decorations::Server` (macOS/Windows/X11/SSD): inset 0, no shadow, no
   rounding — the system provides them.

## 7. Risks / things to test

- Transparent surfaces cost a little alpha blending; negligible here.
- The inset moves the content origin — **must** use `set_client_inset` so
  overlay/popover/hit-test math (the H2 concern) stays correct. Verify the
  selection popover, margin lane, palette, and resize bands after.
- Compositor variance: some draw a faint shadow even for CSD; test sway vs
  GNOME so we don't double up.
- Rounded corners + the custom titlebar: the top corners must clip the
  titlebar too, or it'll poke out square.

## Sources

- Wayland `xdg-decoration` protocol (`zxdg_decoration_manager_v1`) — SSD/CSD
  negotiation; GNOME Mutter & wlroots decline SSD for toplevels.
- GTK / libadwaita CSD: client draws shadow + rounded corners in a
  transparent margin around the content; no shadow on tiled edges.
- Qt `QtWayland` client-side decoration plugin — same gutter approach.
- Zed's Linux window path — rolls CSD chrome from gpui primitives (no shared
  helper at rev `992f395`).
- gpui rev `992f395` `crates/gpui/src/window.rs`: `set_client_inset` (2315),
  `paint_drop_shadows` (3627), `WindowBackgroundAppearance`, `Decorations`.
