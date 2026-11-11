#!/usr/bin/env bash
# Ad-hoc sign the .app inside a macOS DMG and write the DMG back out.
#
# cargo-packager always passes --timestamp to codesign, which is not valid for
# ad-hoc signatures, so the signing has to happen after the DMG is built.
#
# This does not remove the Gatekeeper "unidentified developer" prompt (that
# needs a Developer ID certificate and notarization), but it keeps Apple
# Silicon from rejecting the unsigned bundle as damaged.
#
# Usage: bash tools/sign_macos_dmg.sh <path-to-dmg> "<App Name>"

set -euo pipefail

dmg="${1:?usage: sign_macos_dmg.sh <path-to-dmg> <app-name>}"
app_name="${2:?usage: sign_macos_dmg.sh <path-to-dmg> <app-name>}"

workdir="$(mktemp -d)"
mountpoint="$workdir/mount"
readwrite_dmg="$workdir/readwrite.dmg"
final_dmg="$workdir/final.dmg"
local_app="$workdir/$app_name.app"

cleanup() {
    hdiutil detach "$mountpoint" -force >/dev/null 2>&1 || true
    rm -rf "$workdir"
}
trap cleanup EXIT

mkdir -p "$mountpoint"

echo "Converting $dmg to a writable image..."
hdiutil convert "$dmg" -format UDRW -o "$readwrite_dmg" -quiet

# The bundle is larger now that sherpa-onnx is linked in. Give the writable
# image headroom so replacing the app cannot run out of space while signing.
hdiutil resize -size 2g "$readwrite_dmg" >/dev/null 2>&1 || true

echo "Mounting the writable image..."
hdiutil attach "$readwrite_dmg" -mountpoint "$mountpoint" -nobrowse -quiet

app="$mountpoint/$app_name.app"
if [ ! -d "$app" ]; then
    echo "Could not find $app in the DMG" >&2
    exit 1
fi

# Sign a copy outside the disk image. codesign can fail with "internal error in
# Code Signing subsystem" when it rewrites a bundle on a mounted image.
echo "Copying $app_name.app out of the image..."
cp -R "$app" "$local_app"
chmod -R u+w "$local_app"

# Clear attributes and any previous bundle signature before re-signing.
xattr -cr "$local_app"
rm -rf "$local_app/Contents/_CodeSignature"

sign_retry() {
    local target="$1"
    local attempt
    for attempt in 1 2 3; do
        if codesign --force --sign - --timestamp=none "$target"; then
            return 0
        fi
        echo "codesign attempt $attempt failed for $target" >&2
        sleep 2
    done
    return 1
}

# Sign nested executables first (inside out), then the bundle. `--deep` is
# deprecated and fragile; the bundle only contains the two binaries.
for exe in "$local_app/Contents/MacOS/"*; do
    if [ -f "$exe" ] && [ -x "$exe" ]; then
        echo "Signing $(basename "$exe")..."
        sign_retry "$exe"
    fi
done

echo "Ad-hoc signing $app_name.app..."
sign_retry "$local_app"
codesign --verify --strict "$local_app"

echo "Replacing the app inside the image..."
rm -rf "$app"
cp -R "$local_app" "$app"

echo "Repacking the signed DMG..."
hdiutil detach "$mountpoint" -quiet
hdiutil convert "$readwrite_dmg" -format UDZO -o "$final_dmg" -quiet
mv -f "$final_dmg" "$dmg"

echo "Signed $dmg"
