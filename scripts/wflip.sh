#!/bin/bash
# Scale-FLIP harness (docs/VISUAL-RIG.md): ONE headless output whose scale
# flips mid-session. `swaymsg "output HEADLESS-1 scale 1"` drives the same
# wl_surface buffer-scale path as moving the window between a 2x and a 1x
# monitor (scripts/wmigrate.sh) — but in one output, so captures from the
# same scale can be byte-compared.
#
#   scripts/wflip.sh [list] [footnote] [plain]     (default: all three)
#
# Protocol per fixture (scripts/fixtures/flip-*.md):
#   boot sway @2x -> load fixture -> capture A1
#   swaymsg scale 1 -> 2 frames    -> capture B
#   swaymsg scale 2 -> 2 frames    -> capture A2
#   fresh sway @1x -> same fixture -> capture REF
#
# Oracle 1: A1 == A2. Same process, same renderer, same scale — ANY pixel
#           difference is state leaked across the scale flip.
# Oracle 2: B == REF. The post-flip render must match a fresh boot at the
#           new scale — this is literally the user's monitor-migration bug.
#
# Determinism: STROP_TEST_STILL=1 (no cursor blink, frozen timestamps),
# isolated XDG_STATE_HOME/XDG_CONFIG_HOME per boot, fixture imported from
# .md fresh for every boot (a sibling .strop would lose spans/blocks unless
# an edit forced a save — never reuse one across boots).
# Tolerance: WFLIP_AE_TOL (default 50 px of 1.92M) absorbs single-pixel
# jitter; corruption measures in the tens of thousands.
set -u
BIN="$PWD/target/debug/strop"
FIXDIR="$PWD/scripts/fixtures"
OUTDIR="${WFLIP_OUT:-/tmp/wflip}"
TOL="${WFLIP_AE_TOL:-50}"
mkdir -p "$OUTDIR"

# Every fixture opens the palette (ctrl-shift-p). The corruption poisons
# the renderer's sprite bookkeeping, but it MANIFESTS in glyphs shaped
# fresh after the flip — the palette's overlay text is that surface.
# Isolation runs (2026-06-12): footnote doc without palette = AE~0 despite
# the poisoned state; plain doc WITH palette = AE~1 (palette innocent);
# list+palette with STROP_NO_MARKER_PAINT=1 = AE~4 (marker paint is the
# trigger). So palette-open keeps (a),(b) failing and (c) is the control.
declare -A FIX_MD=([list]=flip-list.md [footnote]=flip-footnote.md [plain]=flip-plain.md)
declare -A FIX_KEYS=(
  [list]="ctrl-home ctrl-shift-p"
  [footnote]="ctrl-home ctrl-shift-p"
  [plain]="ctrl-home ctrl-shift-p"
)

SWAY="" APP=""
cleanup() { kill "$APP" "$SWAY" 2>/dev/null; }
trap cleanup EXIT

boot_sway() { # $1 = scale
  local cfg
  cfg=$(mktemp)
  printf 'output HEADLESS-1 mode 1600x1200\noutput HEADLESS-1 scale %s\n' "$1" >"$cfg"
  WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman \
    sway -c "$cfg" >/tmp/wflip-sway.log 2>&1 &
  SWAY=$!
  sleep 2
  WD=$(ls -t "$XDG_RUNTIME_DIR" | grep -E '^wayland-[0-9]+$' | head -1)
  SOCK=$(ls -t "$XDG_RUNTIME_DIR"/sway-ipc.* 2>/dev/null | head -1)
  rm -f "$cfg"
}

launch_strop() { # $1 = doc, $2 = smoke keys; fresh ISO dir per boot
  ISO=$(mktemp -d)
  rm -f /tmp/strop-wflip.log
  env -u DISPLAY WAYLAND_DISPLAY="$WD" \
    XDG_STATE_HOME="$ISO/state" XDG_CONFIG_HOME="$ISO/config" \
    STROP_TEST_STILL=1 STROP_SMOKE="$2" STROP_SMOKE_HOLD=1 ${EXTRA_ENV:-} \
    "$BIN" "$1" 2>/tmp/strop-wflip.log &
  APP=$!
  for _ in $(seq 1 60); do
    grep -q "SMOKE HOLD" /tmp/strop-wflip.log 2>/dev/null && break
    sleep 0.5
  done
  sleep 1.5
}

stop_session() {
  kill "$APP" "$SWAY" 2>/dev/null
  wait "$APP" "$SWAY" 2>/dev/null
  APP="" SWAY=""
  sleep 1
}

shoot() { env -u DISPLAY WAYLAND_DISPLAY="$WD" grim -o HEADLESS-1 "$1"; }

ae() { # absolute-error pixel count between two images, as an integer
  local v
  v=$(compare -metric AE "$1" "$2" null: 2>&1) || true
  awk -v v="$v" 'BEGIN { printf "%d\n", v + 0 }'
}

FIXTURES=("$@")
[ ${#FIXTURES[@]} -eq 0 ] && FIXTURES=(list footnote plain)
FAILED=()

for name in "${FIXTURES[@]}"; do
  md="$FIXDIR/${FIX_MD[$name]}"
  keys="${FIX_KEYS[$name]}"
  [ -f "$md" ] || { echo "wflip: unknown fixture '$name'" >&2; exit 2; }

  echo "== wflip $name: flip session (2x -> 1x -> 2x)"
  work=$(mktemp -d)
  cp "$md" "$work/fixture.md"
  boot_sway 2
  launch_strop "$work/fixture.md" "$keys"
  shoot "$OUTDIR/$name-A1.png"
  swaymsg -s "$SOCK" "output HEADLESS-1 scale 1" >/dev/null
  sleep 2
  shoot "$OUTDIR/$name-B.png"
  swaymsg -s "$SOCK" "output HEADLESS-1 scale 2" >/dev/null
  sleep 2
  shoot "$OUTDIR/$name-A2.png"
  stop_session

  echo "== wflip $name: reference session (fresh boot @1x)"
  work=$(mktemp -d)
  cp "$md" "$work/fixture.md"
  boot_sway 1
  launch_strop "$work/fixture.md" "$keys"
  shoot "$OUTDIR/$name-REF1x.png"
  stop_session

  ae1=$(ae "$OUTDIR/$name-A1.png" "$OUTDIR/$name-A2.png")
  ae2=$(ae "$OUTDIR/$name-B.png" "$OUTDIR/$name-REF1x.png")
  if [ "$ae1" -le "$TOL" ]; then
    echo "   oracle 1 (A1 vs A2, state leakage): PASS (AE=$ae1)"
  else
    echo "   oracle 1 (A1 vs A2, state leakage): FAIL (AE=$ae1) — $OUTDIR/$name-A1.png vs $name-A2.png"
    FAILED+=("$name/oracle1")
  fi
  if [ "$ae2" -le "$TOL" ]; then
    echo "   oracle 2 (B vs fresh-1x, the user's bug): PASS (AE=$ae2)"
  else
    echo "   oracle 2 (B vs fresh-1x, the user's bug): FAIL (AE=$ae2) — $OUTDIR/$name-B.png vs $name-REF1x.png"
    FAILED+=("$name/oracle2")
  fi
done

if [ ${#FAILED[@]} -gt 0 ]; then
  echo "wflip: FAILED: ${FAILED[*]}"
  exit 1
fi
echo "wflip: all fixtures clean"
