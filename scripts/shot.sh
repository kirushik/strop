#!/bin/bash
# Autonomous visual verification (Phase F): run Strop as an XWayland
# client (x11 backend; X windows are capturable under GNOME Wayland,
# unlike Wayland-native ones) and screenshot it with ImageMagick.
#   scripts/shot.sh out.png [file.strop] [smoke keys...]
set -e
OUT="$1"; shift
DOC="${1:-}"; [ $# -gt 0 ] && shift
KEYS="$*"
BIN="$PWD/target/debug/strop"
rm -f /tmp/strop-shot.log
if [ -n "$DOC" ]; then
    env -u WAYLAND_DISPLAY DISPLAY=:0 STROP_SMOKE="${KEYS:-ctrl-home}" STROP_SMOKE_HOLD=1 \
        "$BIN" "$DOC" 2>/tmp/strop-shot.log &
else
    env -u WAYLAND_DISPLAY DISPLAY=:0 STROP_SMOKE="${KEYS:-ctrl-home}" STROP_SMOKE_HOLD=1 \
        "$BIN" 2>/tmp/strop-shot.log &
fi
PID=$!
for _ in $(seq 1 40); do
    grep -q "SMOKE HOLD" /tmp/strop-shot.log 2>/dev/null && break
    sleep 0.5
done
sleep 1
WIN=$(env -u WAYLAND_DISPLAY DISPLAY=:0 xwininfo -root -tree 2>/dev/null \
    | grep -m1 'Strop' | awk '{print $1}')
if [ -n "$WIN" ]; then
    # Unfocused XWayland surfaces present frames lazily; the first capture
    # drains the stale queue, the second one sees the current state.
    env -u WAYLAND_DISPLAY DISPLAY=:0 import -window "$WIN" "$OUT"
    sleep 2
    env -u WAYLAND_DISPLAY DISPLAY=:0 import -window "$WIN" "$OUT"
else
    env -u WAYLAND_DISPLAY DISPLAY=:0 import -window root "$OUT"
fi
kill $PID 2>/dev/null || true
echo "shot: $OUT"
