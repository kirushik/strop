#!/bin/bash
# Run a Strop smoke script inside headless sway (no window on the user's
# desktop — docs/VISUAL-RIG.md). For doc setup, state mutations, etc.
#   scripts/wrun.sh file.strop [smoke keys...]
set -e
DOC="$1"; shift
KEYS="$*"
# One cleanup path for every exit — normal, set -e, or Ctrl-C: the
# background compositor and the temp dirs must never outlive the run.
SWAY=; CFG=; CFGHOME=; RUNDIR=
trap '[ -n "$SWAY" ] && kill $SWAY 2>/dev/null; rm -rf "$CFG" "$CFGHOME" "$RUNDIR"' EXIT
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
# A PRIVATE runtime dir for the headless compositor. Never share the
# session's $XDG_RUNTIME_DIR: globbing wayland-* there could fall back
# to the developer's LIVE compositor when sway fails to start, opening
# the rig window on the real desktop (focus steal) — the run must die
# instead. A private dir also makes parallel rig runs collision-free.
RUNDIR=$(mktemp -d); chmod 700 "$RUNDIR"
WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman \
  XDG_RUNTIME_DIR="$RUNDIR" sway -c "$CFG" > /dev/null 2>&1 &
SWAY=$!
# Wait for the socket to actually appear (up to 10s) instead of a blind
# sleep; if sway never comes up there is nothing safe to fall back to.
WD=
for _ in $(seq 1 50); do
  WD=$(ls "$RUNDIR" 2>/dev/null | grep -E '^wayland-[0-9]+$' | head -1)
  [ -n "$WD" ] && break
  sleep 0.2
done
if [ -z "$WD" ]; then
  echo "wrun: headless sway failed to start (no wayland socket in $RUNDIR)" >&2
  exit 2
fi
# WRUN_TAIL widens the output window (dump:ui smoke runs need every
# UI-DUMP line, not just the last two).
env -u DISPLAY XDG_RUNTIME_DIR="$RUNDIR" WAYLAND_DISPLAY="$WD" \
  STROP_SMOKE="$KEYS" XDG_CONFIG_HOME="$CFGHOME" \
  timeout 60 "$PWD/target/debug/strop" "$DOC" 2>&1 | tail -"${WRUN_TAIL:-2}"
