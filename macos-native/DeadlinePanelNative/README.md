# Deadline Panel Native

Native macOS 0.6.2 rewrite experiment.

This package contains the SwiftUI/AppKit shell. The app links to the Rust core
crate at `../../crates/deadline-core`.

Open this directory in Xcode to inspect and edit the Swift package:

```bash
open macos-native/DeadlinePanelNative/Package.swift
```

For now, final app bundling is handled by:

```bash
macos-native/scripts/build-native-skeleton.sh
```

The build script:

1. Builds the Rust core release dylib.
2. Compiles the SwiftUI/AppKit executable.
3. Creates `Deadline Panel Native.app`.
4. Copies `libdeadline_core.dylib` into `Contents/Frameworks`.
5. Rewrites dynamic library paths to use `@rpath`.
