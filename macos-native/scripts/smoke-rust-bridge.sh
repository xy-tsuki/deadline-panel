#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUST_RELEASE_DIR="$ROOT_DIR/crates/deadline-core/target/release"
BUILD_DIR="$ROOT_DIR/macos-native/.build/smoke"
MODULE_CACHE_DIR="$ROOT_DIR/macos-native/.build/module-cache"

cargo build --manifest-path "$ROOT_DIR/crates/deadline-core/Cargo.toml" --release

mkdir -p "$BUILD_DIR" "$MODULE_CACHE_DIR"

xcrun swiftc \
  -target arm64-apple-macosx14.0 \
  -module-cache-path "$MODULE_CACHE_DIR" \
  "$ROOT_DIR/macos-native/Smoke/RustCoreClient.swift" \
  "$ROOT_DIR/macos-native/Smoke/RustCoreSmoke.swift" \
  -L "$RUST_RELEASE_DIR" \
  -ldeadline_core \
  -Xlinker -rpath \
  -Xlinker "$RUST_RELEASE_DIR" \
  -o "$BUILD_DIR/rust-core-smoke"

"$BUILD_DIR/rust-core-smoke"
