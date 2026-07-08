# Deadline Panel v0.5.2 macOS Release 草稿

## 定位

- macOS 迁移探针版候选还原点。
- 用于在 `macos-port-probe` 分支上冻结当前可验证状态。
- 暂不发布 GitHub Release，暂不更新 `updates/latest.json`。

## macOS 主要变化

- 项目已可在 macOS 上编译并启动。
- 保留双窗口架构：收起条窗口和展开 panel 窗口独立运行。
- 收起条支持 hover 展开，离开后收起。
- 展开 panel 会贴近收起条，并根据屏幕空间向上或向下展开。
- 收起条支持拖拽移动并记忆位置。
- 支持全屏前台窗口自动隐藏。
- 支持 macOS 开机启动设置。
- 菜单栏图标替换为 macOS 菜单栏版本，并支持左键展示菜单。
- Dock 显示/隐藏入口已加入菜单栏菜单和右键菜单。
- 菜单语言在“跟随系统”时读取 macOS 系统语言。
- 收起条和 panel 可显示在所有桌面空间。
- 窗口使用 macOS 原生 `NSGlassEffectViewStyle::Regular` 作为当前 Liquid Glass 背景尝试，并叠加固定暗化层保证可读性。

## 已知边界

- 当前 React/WebView 内部控件只是视觉近似，不是 AppKit/SwiftUI 原生 Liquid Glass 控件。
- 完整原生控件迁移将在后续实验分支中验证。
- 当前 macOS bundle 使用 `LSUIElement=true` 默认隐藏 Dock 图标。
- macOS `.app` 当前为未签名调试/候选构建，正式发布前需要补齐签名、公证和发布资产流程。

## 验证建议

- 先退出旧实例，再启动新的 `.app`，避免 single-instance 插件把新启动转给旧进程。
- 验证菜单栏和右键菜单中的 Dock 显示/隐藏文案能随点击来回切换。
- 验证 Dock 显示/隐藏实际行为。
- 验证收起条 hover 展开、离开收起。
- 验证拖拽收起条后重启仍保留位置。
- 验证所有桌面空间中收起条可见。
- 验证全屏应用前台时自动隐藏，退出全屏后恢复。
- 验证中/日/英和“跟随系统”的菜单文案。
- 回归任务新增、编辑、完成、延期、导入、同步、历史记录等主流程。

## 发布前注意

- 不要从此草稿直接生成公开 release。
- 不要更新 `updates/latest.json` 指向该版本。
- 若要公开发布 macOS 版，需要先建立 macOS 签名、公证、DMG/ZIP 资产和 updater manifest 策略。
