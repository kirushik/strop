#!/bin/bash
# Wayland-native visual rig (docs/VISUAL-RIG.md): headless sway + grim.
# Fully isolated from the user session — nothing appears on the desktop.
#   scripts/wshot.sh out.png scale [file.strop] [smoke keys...]
set -e
OUT="$1"; SCALE="${2:-2}"; shift 2
DOC="${1:-}"; [ $# -gt 0 ] && shift
KEYS="$*"
BIN="$PWD/target/debug/strop"
CFG=$(mktemp)
# WSHOT_MODE overrides the virtual output resolution (default 1600x1200).
# Headless sway TILES the window to fill the output, so this is how you test a
# given window WIDTH: WSHOT_MODE=1100x800 exercises the narrow-margin left-shift
# band, 850x800 the too-narrow notes pill.
printf 'output HEADLESS-1 mode %s\noutput HEADLESS-1 scale %s\n' "${WSHOT_MODE:-1600x1200}" "$SCALE" > "$CFG"
# WSHOT_BG paints the desktop behind the window a solid colour (default black).
# A CSD drop shadow is invisible against black — set a light desktop (e.g.
# WSHOT_BG=#f4f2ec) to judge the shadow the way a real GNOME desktop shows it.
[ -n "$WSHOT_BG" ] && printf 'output HEADLESS-1 bg %s solid_color\n' "$WSHOT_BG" >> "$CFG"
# WSHOT_FLOAT=WxH floats the window at that size instead of tiling it to fill
# the output — the only way to exercise client-side decorations (the shadow
# gutter / rounded corners are suppressed on tiled edges). Leaves a gap around
# the window so grim captures the gutter + shadow.
if [ -n "$WSHOT_FLOAT" ]; then
  FW="${WSHOT_FLOAT%x*}"; FH="${WSHOT_FLOAT#*x}"
  printf 'for_window [title=".*Strop"] floating enable, resize set %s %s, move position center\n' "$FW" "$FH" >> "$CFG"
fi
rm -f /tmp/strop-wshot.log
# A PRIVATE runtime dir, exactly like wrun.sh's: never glob wayland-*
# in the session's $XDG_RUNTIME_DIR — when sway is slow to make its
# socket, the newest one there is the developer's LIVE compositor, and
# the "headless" window opens on the real desktop, stealing focus
# (observed live, 2026-07-17 — this script, not the callers, was the
# leak). Private logs also make parallel runs collision-free: two rigs
# sharing /tmp/strop-wshot.log matched each other's SMOKE HOLD and
# shot blank frames.
RUNDIR=$(mktemp -d); chmod 700 "$RUNDIR"
LOG="$RUNDIR/strop.log"
APP=; SWAY=
trap 'kill $APP $SWAY 2>/dev/null; cp -f "$LOG" /tmp/strop-wshot.log 2>/dev/null; rm -rf "$CFG" "$RUNDIR"' EXIT
WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman \
  XDG_RUNTIME_DIR="$RUNDIR" sway -c "$CFG" > "$RUNDIR/sway.log" 2>&1 &
SWAY=$!
WD=
for _ in $(seq 1 50); do
  WD=$(ls "$RUNDIR" 2>/dev/null | grep -E '^wayland-[0-9]+$' | head -1)
  [ -n "$WD" ] && break
  sleep 0.2
done
if [ -z "$WD" ]; then
  echo "wshot: headless sway failed to start (no wayland socket in $RUNDIR)" >&2
  exit 2
fi
if [ -n "$DOC" ]; then
  env -u DISPLAY XDG_RUNTIME_DIR="$RUNDIR" WAYLAND_DISPLAY="$WD" \
    STROP_SMOKE="${KEYS:-ctrl-home}" STROP_SMOKE_HOLD=1 $EXTRA_ENV \
    "$BIN" "$DOC" 2>"$LOG" &
else
  env -u DISPLAY XDG_RUNTIME_DIR="$RUNDIR" WAYLAND_DISPLAY="$WD" \
    STROP_SMOKE="${KEYS:-ctrl-home}" STROP_SMOKE_HOLD=1 $EXTRA_ENV \
    "$BIN" 2>"$LOG" &
fi
APP=$!
# Readiness is a GATE, not advice (review finding): a crashed or hung
# app must fail the run, never yield a blank "successful" frame.
READY=
for _ in $(seq 1 60); do
  grep -q "SMOKE HOLD" "$LOG" 2>/dev/null && { READY=1; break; }
  kill -0 "$APP" 2>/dev/null || break
  sleep 0.5
done
if [ -z "$READY" ]; then
  echo "wshot: app never reached SMOKE HOLD — no shot taken; log tail:" >&2
  tail -20 "$LOG" >&2 2>/dev/null
  exit 3
fi
sleep 1.5
env -u DISPLAY XDG_RUNTIME_DIR="$RUNDIR" WAYLAND_DISPLAY="$WD" grim -o HEADLESS-1 "$OUT"
echo "wshot: $OUT"
