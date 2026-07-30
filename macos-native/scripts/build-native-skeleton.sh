#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUST_RELEASE_DIR="$ROOT_DIR/crates/deadline-core/target/release"
BUILD_DIR="$ROOT_DIR/macos-native/.build/native"
MODULE_CACHE_DIR="$ROOT_DIR/macos-native/.build/module-cache"
SOURCES_DIR="$ROOT_DIR/macos-native/DeadlinePanelNative/Sources/DeadlinePanelNative"
APP_DIR="$BUILD_DIR/Deadline Panel.app"
CONTENTS_DIR="$APP_DIR/Contents"
MACOS_DIR="$CONTENTS_DIR/MacOS"
FRAMEWORKS_DIR="$CONTENTS_DIR/Frameworks"
RESOURCES_DIR="$CONTENTS_DIR/Resources"
EXECUTABLE_PATH="$MACOS_DIR/DeadlinePanelNative"
APP_ICON_SOURCE="$ROOT_DIR/src-tauri/icons/icon.png"

cargo build --manifest-path "$ROOT_DIR/crates/deadline-core/Cargo.toml" --release

mkdir -p "$BUILD_DIR" "$MODULE_CACHE_DIR"
rm -rf "$APP_DIR"
mkdir -p "$MACOS_DIR" "$FRAMEWORKS_DIR" "$RESOURCES_DIR"

xcrun swiftc \
  -target arm64-apple-macosx14.0 \
  -module-cache-path "$MODULE_CACHE_DIR" \
  "$SOURCES_DIR/DeadlineModels.swift" \
  "$SOURCES_DIR/DateFormatting.swift" \
  "$SOURCES_DIR/NativeLocalization.swift" \
  "$SOURCES_DIR/NativePreferenceMigration.swift" \
  "$SOURCES_DIR/NativeAppearance.swift" \
  "$SOURCES_DIR/GlassComponents.swift" \
  "$SOURCES_DIR/RustCoreClient.swift" \
  "$SOURCES_DIR/DeadlineRepository.swift" \
  "$SOURCES_DIR/DeadlineViewModel.swift" \
  "$SOURCES_DIR/NativeCommandParser.swift" \
  "$SOURCES_DIR/FileImportExportController.swift" \
  "$SOURCES_DIR/ContentView.swift" \
  "$SOURCES_DIR/NativePanelViews.swift" \
  "$SOURCES_DIR/NativePanelCoordinator.swift" \
  "$SOURCES_DIR/NativeFullscreenMonitor.swift" \
  "$SOURCES_DIR/LoginItemController.swift" \
  "$SOURCES_DIR/NativeCloudSyncController.swift" \
  "$SOURCES_DIR/NativeNotificationController.swift" \
  "$SOURCES_DIR/NativeSettingsController.swift" \
  "$SOURCES_DIR/NativeRelaunch.swift" \
  "$SOURCES_DIR/NativeMenuBarController.swift" \
  "$SOURCES_DIR/AppDelegate.swift" \
  "$SOURCES_DIR/DeadlinePanelNativeApp.swift" \
  -L "$RUST_RELEASE_DIR" \
  -ldeadline_core \
  -Xlinker -rpath \
  -Xlinker "@executable_path/../Frameworks" \
  -framework SwiftUI \
  -framework AppKit \
  -framework CoreGraphics \
  -framework ServiceManagement \
  -framework UserNotifications \
  -framework UniformTypeIdentifiers \
  -o "$EXECUTABLE_PATH"

cp "$RUST_RELEASE_DIR/libdeadline_core.dylib" "$FRAMEWORKS_DIR/"
cp "$ROOT_DIR/macos-native/DeadlinePanelNative/Info.plist" "$CONTENTS_DIR/"
cp "$ROOT_DIR/macos-native/DeadlinePanelNative/Sources/DeadlinePanelNative/Resources/menuBarIcon.png" "$RESOURCES_DIR/"

sips -s format icns "$APP_ICON_SOURCE" --out "$RESOURCES_DIR/AppIcon.icns" >/dev/null

OLD_DYLIB_ID="$(otool -D "$FRAMEWORKS_DIR/libdeadline_core.dylib" | sed -n '2p')"
install_name_tool -id "@rpath/libdeadline_core.dylib" "$FRAMEWORKS_DIR/libdeadline_core.dylib"
install_name_tool -change "$OLD_DYLIB_ID" "@rpath/libdeadline_core.dylib" "$EXECUTABLE_PATH"

xattr -cr "$APP_DIR"
codesign --force --sign - "$APP_DIR" >/dev/null

echo "$APP_DIR"
