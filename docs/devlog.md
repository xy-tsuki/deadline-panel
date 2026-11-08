# 开发日志

按日期倒序，新的写在最上面。

## 2026-11-09 仓库整理
- 本地 `main` 是远程改写历史（移除两份内部文档）之前的旧版本，已对齐到 `origin/main`，旧版本留在本地分支 `main-old-backup`（`b9213d8`）；本地陈旧 tag 同步为远程版本。
- Windows 分支改名为 `feat/windows-0.6.2` 并推送，新建 `dev` 并合入 Windows 0.6.2。合并后 `npm run build`、`npm test` 通过。
- README：加入实际运行截图（浏览器模式下按真实窗口尺寸 372×44 / 372×600 截取）、安装和本地开发说明、中英文切换，平台改为 Windows / macOS。仓库加了 Topics。
- 添加 AGENTS.md / CLAUDE.md、完善 .gitignore、开发日志和决策记录、CI、PR 模板。

## 2026-10-04 ～ 2026-10-08 Windows 0.6.2 候选（根据历史补记）
- 同步 macOS 0.6.2 的功能（任务编辑、历史恢复、延期菜单、低功耗模式），重做 Windows UI 和深色半透明外观。
- 修复设置窗口、重启后悬浮条恢复、展开面板时悬浮条无法拖动；展开面板与悬浮条同步拖动。
- 强化自然语言快速解析。改动前打了 `windows-*-2026100x` 存档 tag。

## 2026-07 macOS 移植与 0.5.x（根据历史补记）
- 0.5.1：双窗口结构（折叠条和展开面板分成两个窗口）、运行时 watchdog。
- 制定跨平台开发规则（`docs/cross-platform-development-policy.md`）。
- macOS 移植在 `macos-port-probe`，后转向原生 SwiftUI（`macos-native-swiftui-experiment`），发布了 macOS 0.6.2。

## 2026-06 首次发布到 0.4.3（根据历史补记）
- 0.3.1 首次公开发布，README 本地化，许可证改为 GPL-3.0。
- 0.4.1 Supabase 云同步；0.4.2 自动更新；0.4.3 面板自适应展开方向。
