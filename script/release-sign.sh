#!/usr/bin/env bash
set -euo pipefail

# Before any upload this run has mutated nothing; after the first signature
# upload "draft unchanged" would be a lie (assembly review 2026-07-22).
mutated=0
die() {
  if (( mutated )); then
    echo "release-sign: $*; payloads are unchanged, but signature assets uploaded by this run remain on the draft and will be re-verified on rerun. Next step: fix and rerun." >&2
  else
    echo "release-sign: $*; draft unchanged. Next step: fix and rerun." >&2
  fi
  exit 1
}
published_die() {
  echo "release-sign: $*; the release is already published and cannot be rolled back. Next step: fix the published assets if they are byte-identical to the approved release, or re-cut a higher version; do not delete the published release." >&2
  exit 1
}
need() { command -v "$1" >/dev/null 2>&1 || die "required tool '$1' is missing"; }
for tool in gh jq sha256sum stat minisign git mktemp curl cmp awk; do need "$tool"; done
repo=kirushik/strop

# usage: release-sign.sh [VERSION] [--sign-only]
#   VERSION omitted → the CURRENT draft is used, and "current" is never a
#   guess: §12 serializes releases (one tag, one signer at a time), so
#   EXACTLY ONE draft may exist. Zero or several → die, listing them.
#   --sign-only → verify + sign + upload + re-verify the signature, then
#   STOP with the draft unpublished (the v0.0.N rehearsal's mode).
version=
sign_only=0
for arg in "$@"; do
  case $arg in
    --sign-only) sign_only=1 ;;
    -*) die "unknown flag $arg (usage: $0 [VERSION] [--sign-only])" ;;
    *) [[ -z $version ]] || die "usage: $0 [VERSION] [--sign-only]"; version=$arg ;;
  esac
done
if [[ -z $version ]]; then
  drafts=$(gh release list --repo "$repo" --json tagName,isDraft \
    --jq '[.[] | select(.isDraft)] | map(.tagName) | join(" ")')
  read -r -a draft_tags <<<"$drafts"
  (( ${#draft_tags[@]} == 1 )) \
    || die "expected exactly one draft release, found ${#draft_tags[@]}${drafts:+ ($drafts)} — name the VERSION explicitly"
  version=${draft_tags[0]#v}
  echo "release-sign: current draft is ${draft_tags[0]}"
fi
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.][0-9A-Za-z.-]+)?$ ]] || die "VERSION is not a release version"
tag="v$version"

# Independent verification defaults to the same committed key the binary
# bakes; override MINISIGN_PUBKEY only when bridging a rotation.
MINISIGN_PUBKEY=${MINISIGN_PUBKEY:-"$(git rev-parse --show-toplevel)/minisign.pub"}
[[ -f $MINISIGN_PUBKEY ]] || die "public key file $MINISIGN_PUBKEY is missing"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

release_id=$(gh release view "$tag" --repo "$repo" --json databaseId,isDraft \
  --jq 'select(.isDraft == true) | .databaseId') || die "$tag is not a draft"
[[ -n $release_id ]] || die "$tag is not a draft"

# Exact stage 5/6 inventory derived from .github/workflows/release.yml.
# The portable ZIP is deliberately absent by owner decision W-C. The branch
# workflow still constructs/uploads it; its presence is therefore a fatal
# workflow discrepancy, not part of the approved release inventory.
expected=(
  "strop-$version-x86_64-unknown-linux-gnu.tar.gz"
  "strop-app_${version}-1_amd64.deb"
  "strop-app-${version}-1.x86_64.rpm"
  "strop-$version-aarch64-apple-darwin.dmg"
  "strop-$version-aarch64-apple-darwin-app.tar.gz"
  "strop-$version-x86_64-pc-windows-msvc.exe"
  "strop-$version-x86_64-windows-installer.exe"
  latest.json
)

asset_json() {
  gh api --paginate "repos/$repo/releases/$release_id/assets?per_page=100" \
    --jq '.[] | {id,name,size,digest}' | jq -sc 'sort_by(.name, .id)'
}
asset_json > "$work/assets.initial.json" || die "could not list draft assets"
jq -r '.[].name' "$work/assets.initial.json" > "$work/names.actual"
printf '%s\n' "${expected[@]}" | sort > "$work/names.expected"

# A prior failed signing attempt may leave signatures. They are admitted only
# after verification below; remove them solely from the base inventory diff.
grep -v -E '^latest\.json\.minisig2?$' "$work/names.actual" \
  | sort > "$work/names.base" || true
if ! cmp -s "$work/names.expected" "$work/names.base"; then
  echo "release-sign: exact inventory mismatch (portable ZIP is cut even though release.yml still uploads it):" >&2
  diff -u "$work/names.expected" "$work/names.base" >&2 || true
  die "missing, duplicate, or unexpected draft assets"
fi
[[ $(wc -l < "$work/names.actual") -eq $(sort -u "$work/names.actual" | wc -l) ]] \
  || die "duplicate asset names are present"
if grep -qx latest.json.minisig2 "$work/names.actual" \
    && [[ -z ${MINISIGN_SECOND_PUBKEY:-} ]]; then
  die "a prior second signature exists but MINISIGN_SECOND_PUBKEY is unavailable; re-cut after correcting the signing-key configuration"
fi

mkdir "$work/assets"
gh release download "$tag" --repo "$repo" --dir "$work/assets" \
  --pattern '*' || die "asset download failed"
for asset in "${expected[@]}"; do
  [[ -f $work/assets/$asset ]] || die "missing downloaded $asset"
done
cp "$work/assets/latest.json" "$work/latest.ci.json"

win="strop-$version-x86_64-pc-windows-msvc.exe"
mac="strop-$version-aarch64-apple-darwin-app.tar.gz"
for asset in "$win" "$mac"; do
  size=$(stat -c %s "$work/assets/$asset")
  (( size > 0 && size <= 256*1024*1024 )) \
    || die "$asset is $size bytes — outside the clients' (0, 256 MiB] cap"
done
(( $(stat -c %s "$work/latest.ci.json") <= 1024*1024 )) \
  || die "latest.json exceeds the clients' 1 MiB manifest cap"

pub_date=$(jq -er '.pub_date' "$work/latest.ci.json") || die "pub_date is absent"
notes="https://github.com/$repo/releases/tag/$tag"
entry() {
  local file=$1
  jq -n --arg url "https://github.com/$repo/releases/download/$tag/$file" \
    --arg hash "$(sha256sum "$work/assets/$file" | awk '{print $1}')" \
    --argjson size "$(stat -c %s "$work/assets/$file")" \
    '{url:$url,sha256:$hash,size:$size}'
}
jq -n --arg version "$version" --arg date "$pub_date" --arg notes "$notes" \
  --argjson win "$(entry "$win")" --argjson mac "$(entry "$mac")" \
  '{product:"strop",updater_protocol:1,version:$version,pub_date:$date,notes_url:$notes,targets:{"github-win/x86_64-pc-windows-msvc/exe":$win,"github-mac/aarch64-apple-darwin/app-tar":$mac}}' \
  > "$work/latest.approved.json"
cmp -s "$work/latest.ci.json" "$work/latest.approved.json" \
  || die "manifest differs from downloaded bytes"

# Stage 7 attests assets/*, which includes every payload and latest.json.
for asset in "${expected[@]}"; do
  gh attestation verify "$work/assets/$asset" --repo "$repo" >/dev/null \
    || die "provenance verification failed for $asset"
done

if grep -qx latest.json.minisig "$work/names.actual"; then
  minisign -Vm "$work/latest.approved.json" \
    -x "$work/assets/latest.json.minisig" -p "$MINISIGN_PUBKEY" >/dev/null \
    || die "a prior primary signature does not authenticate this byte-identical manifest; re-cut a higher version"
fi
if grep -qx latest.json.minisig2 "$work/names.actual"; then
  minisign -Vm "$work/latest.approved.json" \
    -x "$work/assets/latest.json.minisig2" -p "$MINISIGN_SECOND_PUBKEY" >/dev/null \
    || die "a prior second signature does not authenticate this byte-identical manifest; re-cut a higher version"
fi

if command -v spctl >/dev/null 2>&1; then
  mkdir "$work/mac-check"
  tar -xzf "$work/assets/$mac" -C "$work/mac-check"
  xcrun stapler validate "$work/mac-check/Strop.app" \
    || die "notarization staple is absent or pending"
fi

printf '%-54s %11s  %-12s  %s\n' ASSET BYTES SHA256 ATTESTED
for asset in "${expected[@]}"; do
  hash=$(sha256sum "$work/assets/$asset" | awk '{print $1}')
  printf '%-54s %11s  %.12s  yes\n' "$asset" \
    "$(stat -c %s "$work/assets/$asset")" "$hash"
done
echo "Tag commit: $(git rev-list -n 1 "$tag")"
if (( sign_only )); then
  read -r -p "Type SIGN $tag to sign the manifest (draft stays unpublished): " confirmation
  [[ $confirmation == "SIGN $tag" ]] || die "confirmation was not exact"
else
  read -r -p "Type PUBLISH $tag to sign and publish: " confirmation
  [[ $confirmation == "PUBLISH $tag" ]] || die "confirmation was not exact"
fi

minisign -Sm "$work/latest.approved.json" \
  -x "$work/latest.json.minisig" || die "primary signing failed"
mutated=1
gh release upload "$tag" --repo "$repo" "$work/latest.json.minisig" --clobber \
  || die "signature upload failed"
signatures=(latest.json.minisig)
if [[ -n ${MINISIGN_SECOND_KEY:-} ]]; then
  [[ -n ${MINISIGN_SECOND_PUBKEY:-} ]] \
    || die "MINISIGN_SECOND_PUBKEY is required"
  minisign -Sm "$work/latest.approved.json" -s "$MINISIGN_SECOND_KEY" \
    -x "$work/latest.json.minisig2" || die "second signing failed"
  gh release upload "$tag" --repo "$repo" "$work/latest.json.minisig2" --clobber \
    || die "second signature upload failed"
  signatures+=(latest.json.minisig2)
elif grep -qx latest.json.minisig2 "$work/names.actual"; then
  cp "$work/assets/latest.json.minisig2" "$work/latest.json.minisig2"
  signatures+=(latest.json.minisig2)
fi

# The final API snapshot is taken immediately before publish. Base asset
# identity/size/digest must be unchanged; only the signatures may differ.
asset_json > "$work/assets.final.json" || die "could not take final asset snapshot"
jq '[.[] | select(.name | test("^latest\\.json\\.minisig2?$") | not)]' \
  "$work/assets.initial.json" > "$work/base.initial.json"
jq '[.[] | select(.name | test("^latest\\.json\\.minisig2?$") | not)]' \
  "$work/assets.final.json" > "$work/base.final.json"
cmp -s "$work/base.initial.json" "$work/base.final.json" \
  || die "draft asset IDs, names, sizes, or digests drifted after approval"
printf '%s\n' "${expected[@]}" "${signatures[@]}" | sort > "$work/names.final.expected"
jq -r '.[].name' "$work/assets.final.json" | sort > "$work/names.final"
cmp -s "$work/names.final.expected" "$work/names.final" \
  || die "final draft inventory drifted after signing"

mkdir "$work/final-download"
gh release download "$tag" --repo "$repo" --dir "$work/final-download" \
  --pattern latest.json --pattern 'latest.json.minisig*' \
  || die "could not re-download final manifest and signatures"
cmp -s "$work/latest.approved.json" "$work/final-download/latest.json" \
  || die "latest.json drifted after approval"
for signature in "${signatures[@]}"; do
  cmp -s "$work/$signature" "$work/final-download/$signature" \
    || die "$signature drifted after signing"
done
minisign -Vm "$work/final-download/latest.json" \
  -x "$work/final-download/latest.json.minisig" -p "$MINISIGN_PUBKEY" >/dev/null \
  || die "final primary signature verification failed"

if (( sign_only )); then
  echo "$tag manifest signed, signature uploaded and re-verified; the draft remains UNPUBLISHED (--sign-only). Rehearsal complete: delete the draft, the tag, and the branch when done."
  exit 0
fi

gh release edit "$tag" --repo "$repo" --draft=false --latest \
  || die "publish failed"

public_base="https://github.com/$repo/releases/latest/download"
canary_ok=false
for _attempt in {1..12}; do
  if curl -fLsS "$public_base/latest.json" -o "$work/public.latest.json" \
      && curl -fLsS "$public_base/latest.json.minisig" -o "$work/public.latest.json.minisig" \
      && [[ $(jq -er '.version' "$work/public.latest.json" 2>/dev/null || true) == "$version" ]] \
      && minisign -Vm "$work/public.latest.json" \
        -x "$work/public.latest.json.minisig" -p "$MINISIGN_PUBKEY" >/dev/null 2>&1; then
    canary_ok=true
    break
  fi
  sleep 5
done
$canary_ok || published_die "the public latest manifest/signature canary did not serve and authenticate $version within 60 seconds"

canary_asset=$(jq -er '.targets | to_entries[0].value.url | split("/")[-1]' \
  "$work/public.latest.json") || published_die "the public manifest has no payload"
canary_hash=$(jq -er '.targets | to_entries[0].value.sha256' \
  "$work/public.latest.json") || published_die "the public manifest payload has no hash"
curl -fLsS "$public_base/$canary_asset" -o "$work/public.payload" \
  || published_die "the public latest payload $canary_asset could not be downloaded"
[[ $(sha256sum "$work/public.payload" | awk '{print $1}') == "$canary_hash" ]] \
  || published_die "the public latest payload $canary_asset failed its manifest hash"
echo "$tag published and public latest canary verified"
