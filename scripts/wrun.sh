#!/bin/bash
# Run a Strop smoke script inside headless sway (no window on the user's
# desktop — docs/VISUAL-RIG.md). For doc setup, state mutations, etc.
#   scripts/wrun.sh file.strop [smoke keys...]
set -e
DOC="$1"; shift
KEYS="$*"
CFG=$(mktemp); printf 'output HEADLESS-1 mode 1600x1200\n' > "$CFG"
# Hermetic config: the app must never read the developer's real
# ~/.config/strop — the editor-face assertions test the DELIBERATE
# NeedsSetup > Ready priority, so a machine with (or without) a real
# provider configured would flip them. A throwaway XDG_CONFIG_HOME
# carries a configured-but-unreachable provider (port 9, discard):
# `configured()` is true, and nothing in a smoke run ever dials it.
CFGHOME=$(mktemp -d)
mkdir -p "$CFGHOME/strop"
printf '[ai]\nbase_url = "http://127.0.0.1:9/v1"\nmodel = "rig-offline"\n' \
  > "$CFGHOME/strop/config.toml"
WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman \
  sway -c "$CFG" > /dev/null 2>&1 &
SWAY=$!
sleep 2
WD=$(ls -t "$XDG_RUNTIME_DIR" | grep -E '^wayland-[0-9]+$' | head -1)
# WRUN_TAIL widens the output window (dump:ui smoke runs need every
# UI-DUMP line, not just the last two).
env -u DISPLAY WAYLAND_DISPLAY="$WD" STROP_SMOKE="$KEYS" \
  XDG_CONFIG_HOME="$CFGHOME" \
  timeout 60 "$PWD/target/debug/strop" "$DOC" 2>&1 | tail -"${WRUN_TAIL:-2}"
kill $SWAY 2>/dev/null || true
rm -rf "$CFG" "$CFGHOME"
