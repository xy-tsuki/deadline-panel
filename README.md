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
