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
rm -f /tmp/strop-wshot.log
WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman \
  sway -c "$CFG" > /tmp/sway.log 2>&1 &
SWAY=$!
sleep 2
WD=$(ls -t "$XDG_RUNTIME_DIR" | grep -E '^wayland-[0-9]+$' | head -1)
if [ -n "$DOC" ]; then
  env -u DISPLAY WAYLAND_DISPLAY="$WD" STROP_SMOKE="${KEYS:-ctrl-home}" STROP_SMOKE_HOLD=1 $EXTRA_ENV \
    "$BIN" "$DOC" 2>/tmp/strop-wshot.log &
else
  env -u DISPLAY WAYLAND_DISPLAY="$WD" STROP_SMOKE="${KEYS:-ctrl-home}" STROP_SMOKE_HOLD=1 $EXTRA_ENV \
    "$BIN" 2>/tmp/strop-wshot.log &
fi
APP=$!
for _ in $(seq 1 60); do
  grep -q "SMOKE HOLD" /tmp/strop-wshot.log 2>/dev/null && break
  sleep 0.5
done
sleep 1.5
env -u DISPLAY WAYLAND_DISPLAY="$WD" grim -o HEADLESS-1 "$OUT"
kill $APP $SWAY 2>/dev/null || true
rm -f "$CFG"
echo "wshot: $OUT"
