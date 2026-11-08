# Deadline Panel

## 项目是什么
ADHD 友好的桌面常驻 Deadline 看板：屏幕右下角常驻一条小信息条，显示当前任务和最近的截止时间，鼠标靠近时展开成完整面板。给容易被打断、容易忘事的人用，强调低干扰、不靠强通知。

## 技术栈
- 前端：React 18 + TypeScript + Vite 5，状态管理 Zustand，图标 lucide-react
- 桌面壳：Tauri 2（Rust），本地数据 SQLite（rusqlite，bundled）
- Windows 原生能力：`windows` crate（DWM 材质、鼠标位置、全屏检测、Toast 通知、注册表自启动）
- 可选云同步：Supabase（通过 RPC 函数，见 `supabase/schema.sql`）
- 打包：NSIS 安装包 + Tauri updater（签名公钥在 `src-tauri/updater.key.pub`）

## 目录结构
- `src/main.tsx`：前端入口
- `src/App.tsx`：几乎所有 UI。按 URL 参数 `?window=` 渲染三种窗口：折叠信息条（默认，Tauri 窗口 `main`）、展开面板（`panel`）、设置（`settings`）
- `src/store/deadlineStore.ts`：Zustand 全局状态和所有任务操作（增删改、完成、延期、导入、同步）
- `src/storage/storage.ts`：存储层。Tauri 下调 Rust 命令读写 SQLite；浏览器下退回 `localStorage` 并写入示例数据
- `src/runtime/tauri.ts`：所有窗口 / 面板相关 Tauri 命令的封装，非 Tauri 环境下返回无操作的默认值
- `src/domain/`：纯逻辑，不依赖 UI
  - `task.ts` 任务类型；`taskSorting.ts` 排序；`seed.ts` 示例任务
  - `quickAddParser.ts` 自然语言快速添加解析（规则见 `docs/quick-add-parsing.md`）
  - `commandParser.ts` `/add ...` 命令导入解析；`taskImport.ts` JSON 导入校验
  - `appearance.ts` 外观偏好；`date.ts` / `timestamp.ts` 时间工具
- `src/sync/`：Supabase 同步。`supabaseSync.ts` 拉取 / 合并 / 上传；`owner.ts` 只让 `main` 窗口执行同步，其他窗口通过事件请求；`transport.ts` 超时和取消；`defaultConfig.ts` 默认公共同步项目（anon key，公开的）
- `src/i18n.ts`：中 / 日 / 英文案
- `src/*.css`：`styles.css` 主样式，`glass.css` 玻璃材质，`macos-parity.css` macOS 对齐样式
- `src-tauri/src/lib.rs`：Rust 主体。Tauri 命令（任务 CRUD、设置、窗口定位、拖动、隐藏、托盘、更新）、SQLite 访问、事件广播
- `src-tauri/src/panel_state.rs`：面板展开 / 方向状态；`material.rs` 窗口材质；`notifications.rs` 系统通知；`autostart.rs` 开机启动
- `src-tauri/tauri.conf.json`：窗口定义（`main` 372×44，`panel` 372×600，都透明、无边框、默认隐藏）、updater 配置
- `scripts/`：`build-release.mjs` 签名打包；`make-latest-json.mjs` 生成 updater 清单；`patch-nsis-update-mode.mjs` NSIS 补丁；`verify-candidate.mjs` 校验安装包签名；`test-*.mjs` 测试
- `supabase/schema.sql`：云同步的表和 RPC 函数
- `updates/latest.json`：线上 updater 清单（发版时才改）
- `docs/`：设计和发布文档（窗口行为、数据库、快速解析规则、跨平台策略、各版本发布检查）

## 常用命令
以下都在 Windows 上实际跑过（2026-11-09）：
- 安装依赖：`npm install`
- 开发运行（桌面）：`npm run tauri:dev`
- 只看前端界面：`npm run dev`，浏览器打开 `http://127.0.0.1:1420/`（折叠条）或 `/?window=panel`（展开面板）。数据在 localStorage，原生窗口功能都是空操作
- 前端构建：`npm run build`（含 `tsc`）
- 类型检查：`npx tsc --noEmit`
- 前端测试：`npm test`（同步、迁移、网络、快速解析，纯 Node 脚本）
- Rust 编译检查：`cargo check --manifest-path src-tauri/Cargo.toml`
- Rust 测试：`cargo test --manifest-path src-tauri/Cargo.toml`
- 打包发布：`npm run tauri:build`（需要本地 `src-tauri/updater.key` 签名私钥）
- 没有 lint 配置

## 架构要点
- **多窗口**：折叠条和展开面板是两个独立的透明 Tauri 窗口，各自加载同一个前端，用 `?window=` 区分。设置也是单独窗口。
- **状态同步**：数据的唯一来源是 Rust 侧 SQLite。任何写操作后 Rust 广播 `deadline-state-changed`，各窗口收到后重新 `load`。前端不要在窗口之间直接传任务数据。
- **云同步只在 `main` 窗口执行**（`src/sync/owner.ts`），避免多窗口重复上传。删除走 tombstone + 重试队列。
- **悬浮展开**：Tauri 环境下由 Rust 读取全局鼠标位置判断是否在触发区（`panel_pointer_state`），再决定展开；浏览器环境直接用 pointerenter。折叠时窗口忽略鼠标事件，不挡下面的程序。背景见 `docs/window-behavior.md`。
- **两种隐藏**：手动隐藏（按钮 / 托盘，只能托盘恢复）和自动隐藏（前台全屏时，退出全屏自动恢复）。
- **排序规则**：未完成优先 → 截止早优先 → 优先级高优先 → active 先于 postponed → 创建早优先（`docs/database.md`）。
- 快速添加完全本地解析，不调用 AI。

## 已知问题
- `src-tauri/src/lib.rs`（约 2700 行）和 `src/App.tsx`（约 1850 行）过大，建议逐步拆分（例如把窗口 / 拖动、SQLite、托盘分到独立模块；App 拆出 ExpandedPanel、SettingsPanel）。
- `docs/cross-platform-development-policy.md` 里写的分支规则（`macos-port-probe`、`windows/*`）已过时，现在按全局约定走 dev / feat；文中的 `src-tauri/src/platform/` 拆分也还没做。
- macOS 原生（SwiftUI）版在远程分支 `macos-native-swiftui-experiment`（含 `macos-native/` 目录），没有合进 dev / main，两条线的关系待确认。
- 没有 lint；React 组件没有测试，测试只覆盖 domain / sync 逻辑。

## 项目约定
- 平台相关的 Rust 代码必须放在 `#[cfg(windows)]` / `#[cfg(target_os = "macos")]` 后面，改一个平台不能破坏另一个平台的编译。
- `updates/latest.json` 只在发版时改，且只能指向已上传到 GitHub Release 的文件；一个平台的发版脚本不能删掉另一个平台的条目。
- 永远不提交 `src-tauri/updater.key`（签名私钥）、`.env`。
- 新增界面文案要同时补中 / 日 / 英三种（`src/i18n.ts`）。
- 原生窗口相关改动（拖动、展开、隐藏、透明）在浏览器里验证不了，必须 `npm run tauri:dev` 实际试。
