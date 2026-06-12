#!/bin/bash
# Multi-monitor scale-change repro (docs/VISUAL-RIG.md): launch Strop on a
# 2x headless output, move it to a 1x output, capture before/after.
#   scripts/wmigrate.sh prefix file.strop [smoke keys...]
set -e
PRE="$1"; shift
DOC="$1"; shift
KEYS="$*"
CFG=$(mktemp)
printf 'output HEADLESS-1 mode 1600x1200\noutput HEADLESS-1 scale 2\n' > "$CFG"
rm -f /tmp/strop-wshot.log
WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman \
  sway -c "$CFG" > /tmp/sway.log 2>&1 &
SWAY=$!
sleep 2
WD=$(ls -t "$XDG_RUNTIME_DIR" | grep -E '^wayland-[0-9]+$' | head -1)
SOCK=$(ls -t "$XDG_RUNTIME_DIR"/sway-ipc.* 2>/dev/null | head -1)
swaymsg -s "$SOCK" create_output >/dev/null
swaymsg -s "$SOCK" "output HEADLESS-2 mode 1600x1200" >/dev/null
swaymsg -s "$SOCK" "output HEADLESS-2 scale 1" >/dev/null
swaymsg -s "$SOCK" "focus output HEADLESS-1" >/dev/null
env -u DISPLAY WAYLAND_DISPLAY="$WD" STROP_SMOKE="${KEYS:-ctrl-home}" STROP_SMOKE_HOLD=1 $EXTRA_ENV \
  "$PWD/target/debug/strop" "$DOC" 2>/tmp/strop-wshot.log &
APP=$!
for _ in $(seq 1 60); do grep -q "SMOKE HOLD" /tmp/strop-wshot.log 2>/dev/null && break; sleep 0.5; done
sleep 1.5
env -u DISPLAY WAYLAND_DISPLAY="$WD" grim -o HEADLESS-1 "${PRE}-on2x.png"
swaymsg -s "$SOCK" "[title=.*Strop.*] move container to output HEADLESS-2" >/dev/null \
  || swaymsg -s "$SOCK" "move container to output HEADLESS-2" >/dev/null
sleep 2
env -u DISPLAY WAYLAND_DISPLAY="$WD" grim -o HEADLESS-2 "${PRE}-on1x.png"
# move back: does it heal?
swaymsg -s "$SOCK" "[title=.*Strop.*] move container to output HEADLESS-1" >/dev/null || true
sleep 2
env -u DISPLAY WAYLAND_DISPLAY="$WD" grim -o HEADLESS-1 "${PRE}-back2x.png"
kill $APP $SWAY 2>/dev/null || true
rm -f "$CFG"
echo "migrate shots: ${PRE}-on2x.png ${PRE}-on1x.png ${PRE}-back2x.png"
