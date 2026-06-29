# Deadline Panel

Deadline Panel 是一个 Windows 桌面常驻 Deadline 看板。

它不是传统待办软件，而是一个低干扰、始终可见的环境提示工具：在电脑右下角常驻一个小信息条，提醒你现在正在做什么、最近的 Deadline 是什么、还剩多久。

## 核心目标

让你在使用电脑时，尽量不会忘记当前最重要的任务和截止时间。

它刻意避免传统 To Do 软件常见的问题：

- 不展示很长的待办列表
- 不做复杂分类
- 不做统计图
- 不做游戏化奖励
- 不依赖频繁通知
- 不要求用户主动打开软件查看

## 主要功能

- 支持中文 / 日文 / 英文
- 右下角桌面悬浮信息条
- 鼠标靠近时自动展开完整面板
- 永远置顶，全屏窗口前自动隐藏
- 支持拖动改变位置
- 支持开机启动
- 本地 SQLite 数据存储
- Top 3 / 5 / 10 重点任务显示
- 历史记录查看与恢复
- 快速添加自然语言解析
- 命令导入预览
- JSON 导入导出
- 可选 Supabase 云同步

## 快速添加示例

可以输入类似：

```text
明天 23:59 Graph Mining Quiz high
```

或：

```text
tomorrow 23:59 Read paper medium
```

或：

```text
明日 18:00 レポート high
```

解析后会显示预览，确认后才会添加。

## 命令导入格式

支持从 ChatGPT / Gemini / 其他工具生成 `"/add"` 命令后导入：

```text
/add title="Graph Mining Quiz" due="2026-06-23 23:59" priority="high" notes="LMS"
```

导入前会显示预览列表，可以修改标题、截止时间、优先级、备注，也可以删除不想导入的条目。

## Supabase 云同步

0.4.0 起支持可选 Supabase 同步码云同步。本地 SQLite 仍然是主存储；不启用同步时，应用完全离线可用。

使用步骤：

普通用户只需要：

1. 在第一台设备的“设置与工具”里生成同步码。
2. 复制同步码。
3. 在另一台设备粘贴同一个同步码。
4. 点击“立即同步”进行首次合并同步。

同步行为：

- 本机新增、修改、完成、删除事项时，会自动尝试推送到云端。
- 应用启动后约 10 秒会静默同步一次。
- 应用运行时约每 5 分钟静默检查并同步一次。
- 自动同步失败不会弹出提示；手动点击“立即同步”时会显示结果。

如果使用自己的 Supabase 项目：

1. 在 Supabase 创建项目。
2. 打开 Supabase SQL Editor，执行 [supabase/schema.sql](supabase/schema.sql)。
3. 在应用的“高级配置”里填写 Project URL 和 anon key。
4. 使用同一个同步码同步多台设备。

安全边界：

- 客户端只使用 anon key，不要填 service role key。
- 同步码等同于访问密钥：谁拿到同步码，谁就能同步这份数据。
- 应用只向 Supabase 发送同步码的 SHA-256 hash，不直接上传明文同步码。
- `deadline_sync_tasks` 表启用 RLS 且不开放直接访问，应用通过 RPC 函数按同步码 hash 同步。
- 同步策略是 last-write-wins：同一事项以 `updated_at` 较新的版本为准。
- 当前版本不会上传截图或附件。

## 技术栈

- React
- TypeScript
- Vite
- Tauri
- SQLite
- Zustand

## 开发

安装依赖：

```powershell
npm install
```

启动前端开发环境：

```powershell
npm run dev
```

启动桌面开发环境：

```powershell
npm run tauri:dev
```

如果安装 Rust 后 `cargo` 无法识别，请重启终端。

## 构建 Windows 安装包

```powershell
npm run tauri:build -- --bundles nsis
```

生成的安装包位于：

```text
src-tauri/target/release/bundle/nsis/
```

## 数据存储

桌面版使用本地 SQLite 数据库，不需要联网。

数据目录由 Tauri 的应用数据目录管理。应用内“设置与工具”里可以打开数据目录，也可以手动备份数据库。

## 设计原则

Deadline Panel 的设计重点是：

- 让最重要的信息一直可见
- 降低用户主动操作成本
- 避免任务列表过长带来的压力
- 优先显示 Deadline，而不是任务数量
- 只显示少数真正需要注意的任务

## License

GPL-3.0
