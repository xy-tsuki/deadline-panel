# Deadline Panel Native

Deadline Panel 的原生 macOS 应用，使用 SwiftUI、AppKit 与 Rust Core。

最低支持 macOS 14，macOS 26 上使用 Liquid Glass。

在 Xcode 中打开 Swift Package：

```bash
open macos-native/DeadlinePanelNative/Package.swift
```

生成本地 `.app`：

```bash
bash macos-native/scripts/build-native-skeleton.sh
```

生成发布 DMG 与 SHA-256：

```bash
macos-native/scripts/package-macos-release.sh
```

未设置签名参数时会生成 ad-hoc 签名包。正式分发时使用：

```bash
SIGN_IDENTITY="Developer ID Application: ..." \
NOTARY_PROFILE="deadline-panel-notary" \
macos-native/scripts/package-macos-release.sh
```
