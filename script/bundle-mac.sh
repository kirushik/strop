#!/usr/bin/env bash
set -euo pipefail

if (( $# < 4 || $# > 5 )); then
  echo "usage: $0 BINARY VERSION OUTPUT_DIR ENTITLEMENTS [NAME]" >&2
  exit 2
fi
binary=$1
version=$2
out=$3
entitlements=$4
name=${5:-Strop}
app="$out/$name.app"
contents="$app/Contents"
mkdir -p "$contents/MacOS" "$contents/Resources"
install -m 755 "$binary" "$contents/MacOS/strop"
install -m 644 packaging/generated/strop.icns "$contents/Resources/strop.icns"
cat > "$contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>strop</string>
<key>CFBundleIconFile</key><string>strop</string>
<key>CFBundleIdentifier</key><string>cc.pimenov.strop</string>
<key>CFBundleName</key><string>$name</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundleVersion</key><string>$version</string>
<key>NSHighResolutionCapable</key><true/>
<key>UTExportedTypeDeclarations</key><array><dict>
<key>UTTypeIdentifier</key><string>cc.pimenov.strop.document</string>
<key>UTTypeConformsTo</key><array><string>public.data</string><string>public.content</string></array>
<key>UTTypeDescription</key><string>Strop document</string>
<key>UTTypeTagSpecification</key><dict><key>public.filename-extension</key><array><string>strop</string></array></dict>
</dict></array>
<key>CFBundleDocumentTypes</key><array><dict>
<key>CFBundleTypeName</key><string>Strop document</string>
<key>LSItemContentTypes</key><array><string>cc.pimenov.strop.document</string></array>
<key>CFBundleTypeRole</key><string>Editor</string>
<key>LSHandlerRank</key><string>Owner</string>
</dict></array>
</dict></plist>
PLIST

if [[ -n ${MACOS_SIGN_IDENTITY:-} ]]; then
  command -v codesign >/dev/null || { echo "codesign is required for signing" >&2; exit 1; }
  codesign --force --deep --options runtime --timestamp \
    --entitlements "$entitlements" --sign "$MACOS_SIGN_IDENTITY" "$app"
fi
mkdir -p "$out/dmg-root"
cp -R "$app" "$out/dmg-root/"
if [[ $(uname -s) == Darwin ]]; then
  ln -s /Applications "$out/dmg-root/Applications"
  ditto -c -k --keepParent "$app" "$out/strop-$version-notary.zip"
  if [[ -n ${APPLE_API_KEY:-} && -n ${APPLE_API_KEY_ID:-} && -n ${APPLE_API_ISSUER:-} ]]; then
    xcrun notarytool submit "$out/strop-$version-notary.zip" --wait \
      --key "$APPLE_API_KEY" --key-id "$APPLE_API_KEY_ID" \
      --issuer "$APPLE_API_ISSUER"
    xcrun stapler staple "$app"
    rm -rf "$out/dmg-root/$name.app"
    cp -R "$app" "$out/dmg-root/"
  fi
  hdiutil create -quiet -format UDZO -srcfolder "$out/dmg-root" \
    "$out/strop-$version-aarch64-apple-darwin.dmg"
else
  echo "non-Darwin host: bundle/plist created; Darwin tools skipped" >&2
fi
tar -C "$out" -czf "$out/strop-$version-aarch64-apple-darwin-app.tar.gz" "$name.app"
