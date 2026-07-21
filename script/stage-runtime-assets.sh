#!/usr/bin/env bash
set -euo pipefail

if (( $# != 1 )); then
  echo "usage: $0 DEST" >&2
  exit 2
fi

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
source_root="$repo/assets"
dest=$1
files=(
  fonts/coldread/URWBookman-Light.otf
  fonts/coldread/URWBookman-LightItalic.otf
  fonts/coldread/URWBookman-Demi.otf
  fonts/coldread/LICENSE
  hyphenation/en-us.standard.bincode
  hyphenation/ru.standard.bincode
  hyphenation/ATTRIBUTION.txt
  paper-noise-256.png
)

for rel in "${files[@]}"; do
  if [[ ! -f "$source_root/$rel" ]]; then
    echo "missing runtime asset: $source_root/$rel" >&2
    exit 1
  fi
done

for rel in "${files[@]}"; do
  mkdir -p "$dest/$(dirname "$rel")"
  cp "$source_root/$rel" "$dest/$rel"
done
