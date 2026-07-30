# Deadline Panel v0.6.2 for macOS

Deadline Panel 0.6.2 是首个完整原生 macOS 版本。0.5.2 已有功能均已迁移至新版原生架构。

## 主要内容

- 界面与 macOS 系统交互完整迁移至 SwiftUI、AppKit，业务核心使用 Rust。
- 使用 macOS 26 Liquid Glass；macOS 14 及以上版本提供原生 Material 回退。
- 重新制作悬浮面板与菜单栏，使窗口材质和交互更符合 macOS。
- 全新设计“设置与工具”界面，重新整理显示偏好、数据管理与云同步等设置。
- 新增低功耗模式，可关闭界面动画以进一步降低 CPU/GPU 占用。
- 优化任务列表、悬浮窗口和动画更新性能。
- 兼容旧版数据、设置与同步码，无需重新建立任务。

## 系统要求

- macOS 14 或更高版本。
- 当前发布资产适用于 Apple Silicon Mac。
- Liquid Glass 完整效果需要 macOS 26；旧系统使用原生 Material 回退。

## 安装

1. 下载并打开 `Deadline-Panel-0.6.2-macOS-arm64.dmg`。
2. 将 `Deadline Panel.app` 拖入 `Applications`。
3. 本版本暂未经过 Apple Developer 签名。首次打开若被拦截，请前往“系统设置 → 隐私与安全性”，点击“仍要打开”。

## 数据说明

- 应用不会在迁移时删除旧版数据库。
- 正式包会迁移实验版的语言、主题、窗口位置、同步码等偏好设置。
- 建议升级前通过设置页面的“备份数据库”保留一份备份。

## 校验

发布页同时提供 SHA-256 校验文件。
