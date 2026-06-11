#!/bin/sh
# Register Strop with the desktop: .strop files double-click open in it
# (PLAN.md E2). User-local only; safe to re-run.
set -e
cd "$(dirname "$0")/.."
BIN="$PWD/target/release/strop"
[ -x "$BIN" ] || BIN="$PWD/target/debug/strop"
[ -x "$BIN" ] || { echo "build first: cargo build --release"; exit 1; }

APPS="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
MIME="${XDG_DATA_HOME:-$HOME/.local/share}/mime"
mkdir -p "$APPS" "$MIME/packages"

sed "s|@BIN@|$BIN|" assets/desktop/strop.desktop.in > "$APPS/strop.desktop"
cp assets/desktop/strop-mime.xml "$MIME/packages/strop.xml"

update-mime-database "$MIME" >/dev/null 2>&1 || true
update-desktop-database "$APPS" >/dev/null 2>&1 || true
xdg-mime default strop.desktop application/x-strop || true
echo "Registered: .strop files now open in $BIN"
