#!/usr/bin/env bash
set -euo pipefail

# Pin packaging/flatpak/cc.pimenov.strop.yml to a PUBLISHED release:
# download the flathub tarball for VERSION, verify its provenance
# attestation, and rewrite the manifest's url: + sha256: to those exact
# bytes. Run after release-sign.sh publishes (ritual §12.5); the result
# is what goes to the flathub/cc.pimenov.strop repo (or, for the first
# submission, into the flathub/flathub new-pr branch).
#
# usage: flathub-pin.sh VERSION

die() { echo "flathub-pin: $*" >&2; exit 1; }
for tool in gh sha256sum mktemp git python3; do
  command -v "$tool" >/dev/null 2>&1 || die "required tool '$tool' is missing"
done

(( $# == 1 )) || die "usage: $0 VERSION"
version=$1
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "VERSION is not a release version"
repo=kirushik/strop
tag="v$version"
asset="strop-$version-x86_64-unknown-linux-gnu-flathub.tar.gz"
manifest="$(git rev-parse --show-toplevel)/packaging/flatpak/cc.pimenov.strop.yml"
[[ -f $manifest ]] || die "manifest $manifest is missing"

# Published AND not a prerelease: the bot on the Flathub side reads
# releases/latest, which excludes both — pinning to something it cannot
# see would advance the manifest past the steady-state automation.
state=$(gh release view "$tag" --repo "$repo" --json isDraft,isPrerelease \
  --jq '"\(.isDraft) \(.isPrerelease)"') \
  || die "$tag is absent, or its state could not be read"
case $state in
  "false false") ;;
  "true "*) die "$tag is still a draft — publish first, pin after" ;;
  *" true") die "$tag is flagged as a prerelease, which releases/latest omits; clear the flag before pinning" ;;
  *) die "$tag is in an unexpected state ($state)" ;;
esac

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
gh release download "$tag" --repo "$repo" --dir "$work" --pattern "$asset" \
  || die "could not download $asset from $tag"
gh attestation verify "$work/$asset" --repo "$repo" >/dev/null \
  || die "provenance verification failed for $asset"
hash=$(sha256sum "$work/$asset" | cut -d' ' -f1)

url="https://github.com/$repo/releases/download/$tag/$asset"
python3 - "$manifest" "$url" "$hash" <<'EOF'
import re, sys
path, url, hash_ = sys.argv[1:]
text = open(path).read()
# The sha256 stays quoted: an all-digit hex string would otherwise be
# read back as a YAML integer (the linter caught exactly that on the
# original placeholder).
text, n_url = re.subn(r'(?m)^(        url: )\S+$', r'\g<1>' + url, text, count=1)
text, n_sha = re.subn(r'(?m)^(        sha256: )"?[0-9a-f]+"?$', r'\g<1>"' + hash_ + '"', text, count=1)
if n_url != 1 or n_sha != 1:
    sys.exit("manifest url:/sha256: lines not found where expected")
open(path, "w").write(text)
EOF

echo "pinned $asset"
echo "  url:    $url"
echo "  sha256: $hash"
echo "next: copy packaging/flatpak/cc.pimenov.strop.yml into the Flathub repo (see packaging/flatpak/README.md)"
