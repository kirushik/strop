# The visual verification rig

How Strop screenshots itself for autonomous UI verification, why each
piece exists, and what would make it better. Useful beyond this project:
the core problem (agent needs pixels; GNOME Wayland forbids external
capture) is universal.

## The problem

GNOME Wayland deliberately prevents one app from reading another's
pixels: `org.gnome.Shell.Screenshot` is allowlisted to GNOME's own
tools, the freedesktop Screenshot portal pops an interactive dialog,
and X11 tools (scrot/import) see only black for Wayland-native windows.
So an agent cannot photograph the app it is testing — every visual bug
needed a human screenshot. Two loopholes exist, and the rig uses both.

## Loophole 1: XWayland (scripts/shot.sh — the everyday rig)

X11 *clients'* window contents ARE readable by X11 tools (XGetImage),
even under GNOME Wayland, because XWayland keeps real backing for its
windows. So Strop builds with BOTH backends (wayland + x11 cargo
features; gpui picks at runtime: WAYLAND_DISPLAY set → wayland, else
DISPLAY → x11), and the rig launches it as an X11 client on the
session's XWayland (`env -u WAYLAND_DISPLAY DISPLAY=:0`), then captures
with ImageMagick `import -window <id>` (window found via `xwininfo
-root -tree | grep Strop`).

Details that matter:
- `STROP_SMOKE="keys…" STROP_SMOKE_HOLD=1` replays a keystroke script
  through the real dispatch path, then HOLDS the window open and prints
  "SMOKE HOLD" to stderr; the script polls for that marker, sleeps 1s,
  captures, kills. Deterministic state, no races.
- **Capture twice, keep the second**: unfocused XWayland surfaces
  present frames lazily; the first XGetImage drains a stale frame from
  1–2 keystrokes earlier. (Found empirically in F1.)
- **HiDPI testing**: the x11 backend derives scale from the X resource
  `Xft.dpi` (96 = 1×). `xrdb -query` to back up, merge `Xft.dpi: 192`
  for 2×, restore after. Affects only newly connecting X clients.
- `click:X,Y` smoke token injects real mouse events — but only works
  under the X11 env; occluded Wayland windows hit-test a stale frame.

KNOWN COST: the window appears on the user's desktop and can steal
focus (X11 maps it visibly; gpui's `focus:false` does not prevent the
WM from focusing a new window). Don't type while the rig runs — or
better, see the wishlist.

## Loophole 2: nested headless GNOME (the Wayland repro stand)

Bugs specific to the WAYLAND backend (e.g. the 2026-06 atlas-corruption
investigation) can't be seen through loophole 1. But GNOME Shell can run
as a *captive compositor we own*:

    MUTTER_DEBUG_DUMMY_MONITOR_SCALES=2 dbus-run-session -- \
      gnome-shell --headless --wayland --virtual-monitor 1280x1024 \
                  --wayland-display wayland-9 --no-x11

- `dbus-run-session` gives it a private session bus (its Shell,
  Screencast service, portals — all ours, no allowlist conflicts with
  the real session).
- `MUTTER_DEBUG_DUMMY_MONITOR_SCALES=2` makes the virtual monitor a
  true 2× HiDPI display — the env the bug lives in.
- Strop runs Wayland-NATIVE inside: `WAYLAND_DISPLAY=wayland-9 strop …`
- Capture: `org.gnome.Shell.Screenshot` is still allowlisted even here,
  but `org.gnome.Shell.Screencast.Screencast` works (returns
  `(true, path.mp4)`); extract a frame with ffmpeg. CAVEAT (open): on a
  headless monitor the encoder needs damage to produce frames — keep
  the recording window ≥8s and drive keystrokes during it; first
  attempts with a 3s static window yielded a 48-byte stub.

## Wishlist (one sudo line)

    sudo apt install sway grim xdotool xvfb

- **sway + grim** (the big one): sway runs headless
  (`WLR_BACKENDS=headless sway`) as a lightweight wlroots compositor;
  grim implements wlr-screencopy and captures it directly — single
  command, no D-Bus, no encoder, no damage caveat, and **fully isolated
  from the user's session** (no windows on the desktop, no focus
  stealing, parallel-safe). Replaces both loopholes for 90% of uses;
  `swaymsg output … scale 2` gives HiDPI.
- **xdotool**: synthetic clicks/keys on the X11 rig without patching
  the smoke harness for every interaction.
- **Xvfb**: same isolation for the X11 backend (rig currently borrows
  the session's XWayland at :0 — that's the focus-stealing).
