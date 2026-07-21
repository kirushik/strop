#!/usr/bin/env bash
set -euo pipefail

die() { echo "release-sign: $*; draft unchanged. Next step: fix and rerun." >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "required tool '$1' is missing"; }
for tool in gh jq sha256sum stat minisign git mktemp; do need "$tool"; done
(( $# == 1 )) || die "usage: $0 VERSION (without v)"
# Independent verification defaults to the same committed key the binary
# bakes; override MINISIGN_PUBKEY only when bridging a rotation.
MINISIGN_PUBKEY=${MINISIGN_PUBKEY:-"$(git rev-parse --show-toplevel)/minisign.pub"}
[[ -f $MINISIGN_PUBKEY ]] || die "public key file $MINISIGN_PUBKEY is missing"
version=$1
tag="v$version"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
gh release view "$tag" --json isDraft --jq '.isDraft' | grep -qx true || die "$tag is not a draft"
gh release download "$tag" --dir "$work" || die "asset download failed"
[[ -f "$work/latest.json" ]] || die "latest.json is missing"
cp "$work/latest.json" "$work/latest.ci.json"
win="strop-$version-x86_64-pc-windows-msvc.exe"
mac="strop-$version-aarch64-apple-darwin-app.tar.gz"
for asset in "$win" "$mac"; do [[ -f "$work/$asset" ]] || die "missing $asset"; done
pub_date=$(jq -er '.pub_date' "$work/latest.ci.json") || die "pub_date is absent"
notes="https://github.com/kirushik/strop/releases/tag/$tag"
entry() {
  local file=$1
  jq -n --arg url "https://github.com/kirushik/strop/releases/download/$tag/$file" \
    --arg hash "$(sha256sum "$work/$file" | awk '{print $1}')" \
    --argjson size "$(stat -c %s "$work/$file")" \
    '{url:$url,sha256:$hash,size:$size}'
}
jq -n --arg version "$version" --arg date "$pub_date" --arg notes "$notes" \
  --argjson win "$(entry "$win")" --argjson mac "$(entry "$mac")" \
  '{product:"strop",updater_protocol:1,version:$version,pub_date:$date,notes_url:$notes,targets:{"github-win/x86_64-pc-windows-msvc/exe":$win,"github-mac/aarch64-apple-darwin/app-tar":$mac}}' > "$work/latest.json"
git diff --no-index --exit-code "$work/latest.ci.json" "$work/latest.json" || die "manifest differs from downloaded bytes"
if command -v spctl >/dev/null 2>&1; then
  mkdir "$work/mac-check"
  tar -xzf "$work/$mac" -C "$work/mac-check"
  xcrun stapler validate "$work/mac-check/Strop.app" || die "notarization staple is absent or pending"
fi
echo "Version: $version"
echo "Tag commit: $(git rev-list -n 1 "$tag")"
jq -r '.targets | to_entries[] | "Channel/triple/kind: \(.key)\nAsset: \(.value.url|split("/")[-1])\nSHA-256: \(.value.sha256)\nSize: \(.value.size)"' "$work/latest.json"
read -r -p "Type PUBLISH $tag to sign and publish: " confirmation
[[ $confirmation == "PUBLISH $tag" ]] || die "confirmation was not exact"
minisign -Sm "$work/latest.json" || die "primary signing failed"
gh release upload "$tag" "$work/latest.json.minisig" --clobber || die "signature upload failed"
if [[ -n ${MINISIGN_SECOND_KEY:-} ]]; then
  minisign -Sm "$work/latest.json" -s "$MINISIGN_SECOND_KEY" -x "$work/latest.json.minisig2" || die "second signing failed"
  gh release upload "$tag" "$work/latest.json.minisig2" --clobber || die "second signature upload failed"
fi
mkdir "$work/verify"
gh release download "$tag" --pattern 'latest.json.minisig*' --dir "$work/verify"
minisign -Vm "$work/latest.json" -x "$work/verify/latest.json.minisig" -p "$MINISIGN_PUBKEY" || die "uploaded signature is invalid"
if [[ -n ${MINISIGN_SECOND_KEY:-} ]]; then
  [[ -f "$work/verify/latest.json.minisig2" ]] || die "second signature is absent"
  [[ -n ${MINISIGN_SECOND_PUBKEY:-} ]] || die "MINISIGN_SECOND_PUBKEY is required"
  minisign -Vm "$work/latest.json" -x "$work/verify/latest.json.minisig2" -p "$MINISIGN_SECOND_PUBKEY" || die "second signature is invalid"
fi
gh release edit "$tag" --draft=false || die "publish failed"
echo "$tag published"
