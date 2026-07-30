#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VERSION="${VERSION:-0.6.2}"
ARCH="${ARCH:-arm64}"
SIGN_IDENTITY="${SIGN_IDENTITY:--}"
NOTARY_PROFILE="${NOTARY_PROFILE:-}"
BUILD_SCRIPT="$ROOT_DIR/macos-native/scripts/build-native-skeleton.sh"
APP_DIR="$ROOT_DIR/macos-native/.build/native/Deadline Panel.app"
DIST_DIR="$ROOT_DIR/macos-native/.build/release"
DMG_ROOT="$ROOT_DIR/macos-native/.build/dmg-root"
ASSET_NAME="Deadline-Panel-${VERSION}-macOS-${ARCH}"
DMG_PATH="$DIST_DIR/$ASSET_NAME.dmg"
CHECKSUM_PATH="$DMG_PATH.sha256"
LEGACY_ZIP_PATH="$DIST_DIR/$ASSET_NAME.zip"

bash "$BUILD_SCRIPT"
mkdir -p "$DIST_DIR"

if [[ "$SIGN_IDENTITY" != "-" ]]; then
  codesign --force --timestamp --options runtime \
    --sign "$SIGN_IDENTITY" \
    "$APP_DIR/Contents/Frameworks/libdeadline_core.dylib"
  codesign --force --timestamp --options runtime \
    --sign "$SIGN_IDENTITY" \
    "$APP_DIR"
else
  echo "warning: no Developer ID identity supplied; producing an ad-hoc signed disk image" >&2
fi

codesign --verify --deep --strict --verbose=2 "$APP_DIR"
rm -rf "$DMG_ROOT"
mkdir -p "$DMG_ROOT"
ditto "$APP_DIR" "$DMG_ROOT/Deadline Panel.app"
ln -s /Applications "$DMG_ROOT/Applications"
rm -f "$DMG_PATH" "$CHECKSUM_PATH" "$LEGACY_ZIP_PATH" "$LEGACY_ZIP_PATH.sha256"
hdiutil create \
  -volname "Deadline Panel $VERSION" \
  -srcfolder "$DMG_ROOT" \
  -format UDZO \
  -ov \
  "$DMG_PATH"

if [[ "$SIGN_IDENTITY" != "-" ]]; then
  codesign --force --timestamp \
    --sign "$SIGN_IDENTITY" \
    --identifier "local.adhd-deadline-panel.dmg" \
    "$DMG_PATH"
  if [[ -n "$NOTARY_PROFILE" ]]; then
    xcrun notarytool submit "$DMG_PATH" --keychain-profile "$NOTARY_PROFILE" --wait
    xcrun stapler staple "$DMG_PATH"
  fi
elif [[ -n "$NOTARY_PROFILE" ]]; then
  echo "NOTARY_PROFILE requires a Developer ID SIGN_IDENTITY" >&2
  exit 1
fi

hdiutil verify "$DMG_PATH"
(
  cd "$DIST_DIR"
  shasum -a 256 "$(basename "$DMG_PATH")"
) > "$CHECKSUM_PATH"
echo "$DMG_PATH"
echo "$CHECKSUM_PATH"
