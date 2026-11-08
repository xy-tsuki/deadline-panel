# 技术决策记录

每条写：背景、比较过什么、结论、以后什么情况会改。

## 用 Tauri 而不是 Electron（根据历史补记）
- 背景：常驻桌面的小工具，要省内存、安装包小，还要调 Windows 原生 API（材质、鼠标位置、全屏检测）。
- 比较：Electron 生态成熟但常驻内存大；Tauri 用系统 WebView，Rust 侧直接调 Win32。
- 结论：Tauri 2 + React。
- 何时重新考虑：WebView2 的透明窗口问题无法绕开，或 macOS 端需要的原生能力 Tauri 做不到（macOS 已经另起了 SwiftUI 原生版）。

## 本地 SQLite 为唯一数据来源，云同步可选（根据历史补记）
- 背景：离线也要能用，多窗口要看到同一份数据。
- 结论：数据存在 Rust 侧 SQLite，任何写入后广播 `deadline-state-changed`，各窗口重新加载。Supabase 同步可选，只在 `main` 窗口执行，用同步码哈希区分用户，删除走 tombstone 和重试。
- 何时重新考虑：需要账号体系或多人协作。

## 透明窗口固定尺寸，展开 / 折叠用 CSS 状态（根据历史补记）
- 背景：Windows + WebView2 的透明无边框窗口在显示、隐藏、改尺寸时会闪烁。
- 结论：原生窗口保持固定矩形，折叠时忽略鼠标事件，由 Rust 读取鼠标位置判断是否进入触发区。详见 `docs/window-behavior.md`。
- 0.5.1 起折叠条和展开面板拆成两个窗口（`main` / `panel`），拖动时两个窗口同步移动。
- 何时重新考虑：WebView2 修复透明窗口闪烁。

## 快速添加完全本地解析，不调用 AI（根据历史补记）
- 背景：隐私，以及离线可用、响应快。
- 结论：规则解析中 / 日 / 英日期、时间、优先级、备注，先预览再保存。规则见 `docs/quick-add-parsing.md`。
- 何时重新考虑：规则覆盖不了的写法太多。

## 许可证 GPL-3.0（根据历史补记）
- 2026-06-24 由初始许可证改为 GPL-3.0。

## Git 分支改为 main / dev / feat（2026-11-09）
- 背景：之前按平台开长期分支（`macos-port-probe`、`codex/*`），main 历史还被改写过一次，分支关系混乱。
- 结论：main 只通过 dev → main 的 PR 更新，功能在 `feat/*`，实验在 `spike/*`。`docs/cross-platform-development-policy.md` 里旧的分支规则作废，其余平台隔离规则仍然有效。
