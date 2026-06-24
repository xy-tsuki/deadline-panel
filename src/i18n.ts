import { TaskStatus } from "./domain/task";

export type AppLanguage = "system" | "zh" | "ja" | "en";
export type ResolvedLanguage = Exclude<AppLanguage, "system">;

export const languageOptions: AppLanguage[] = ["system", "zh", "ja", "en"];

const localeByLanguage: Record<ResolvedLanguage, string> = {
  zh: "zh-CN",
  ja: "ja-JP",
  en: "en-US"
};

export function resolveLanguage(language: AppLanguage): ResolvedLanguage {
  if (language !== "system") return language;

  const preferred = navigator.language.toLowerCase();
  if (preferred.startsWith("ja")) return "ja";
  if (preferred.startsWith("zh")) return "zh";
  return "en";
}

export function localeForLanguage(language: AppLanguage): string {
  return localeByLanguage[resolveLanguage(language)];
}

export function languageName(language: AppLanguage, current: AppLanguage): string {
  const names = strings[resolveLanguage(current)].languageNames;
  return names[language];
}

export function getStrings(language: AppLanguage) {
  return strings[resolveLanguage(language)];
}

export function getImportPrompt(language: AppLanguage): string {
  return getStrings(language).importPrompt;
}

const strings = {
  zh: {
    languageNames: {
      system: "跟随系统",
      zh: "中文",
      ja: "日本語",
      en: "English"
    },
    common: {
      save: "保存",
      delete: "删除",
      remove: "移除",
      complete: "完成",
      restore: "恢复未完成",
      cancelCurrent: "取消当前任务",
      setCurrent: "设为当前任务",
      actions: "操作"
    },
    status: {
      active: "未完成",
      completed: "已完成",
      postponed: "已延期"
    } satisfies Record<TaskStatus, string>,
    panel: {
      restoring: "正在恢复 Deadline...",
      currentPrefix: "正在进行：",
      nearestPrefix: "最近截止：",
      emptyCollapsed: "Top 0｜暂无 Deadline",
      inProgress: "正在进行",
      hideTemporarily: "暂时隐藏",
      currentTask: "当前任务",
      nearestDeadline: "最近 Deadline",
      noUrgentDeadline: "没有迫近的 Deadline",
      recentDeadline: "最近 Deadline",
      emptyTaskList: "添加一个最近要交的东西就好。"
    },
    focus: {
      countLabel: "显示数量"
    },
    task: {
      due: "截止：",
      postponed: "延期",
      custom: "自定义",
      title: "标题",
      dueLabel: "截止",
      priority: "优先级",
      example: "例如 Graph Mining Quiz"
    },
    add: {
      title: "添加 Deadline",
      quickPlaceholder: "明天 23:59 Graph Mining Quiz high",
      quickParse: "解析",
      quickConfirm: "确认添加",
      quickEmpty: "先输入一句自然语言 Deadline",
      quickError: "无法识别，请写清标题和截止时间",
      quickReady: "已识别，可确认添加",
      manual: "手动填写"
    },
    history: {
      title: "历史记录",
      empty: "还没有完成记录。"
    },
    settings: {
      title: "设置与工具",
      focusCount: "重点数量",
      focusCopy: "收起条和主列表只保留最需要看的少数任务。",
      language: "语言",
      languageCopy: "应用界面、日期和导入提示会跟随这里切换。",
      autostart: "开机启动",
      autostartCopy: "打开电脑后自动显示右下角 Deadline 小条。",
      autostartReadFailed: "无法读取开机启动状态",
      dataPathReadFailed: "无法读取数据路径",
      autostartOn: "已开启开机启动",
      autostartOff: "已关闭开机启动",
      autostartFailed: "开机启动设置失败",
      backupDone: "已完成备份",
      backupTo: "已备份到",
      backupFailed: "备份失败",
      dataDirFailed: "无法打开数据目录",
      resetDone: "已重新贴到右下角",
      resetFailed: "复位失败",
      hideFailed: "暂时隐藏失败",
      resetPosition: "复位位置",
      hide15: "隐藏 15 分钟",
      hide30: "隐藏 30 分钟",
      hide60: "隐藏 60 分钟",
      dataDir: "数据目录",
      backup: "备份数据库"
    },
    import: {
      title: "导入",
      copyPrompt: "复制识别 Prompt",
      exportJson: "导出 JSON",
      importJson: "导入 JSON",
      promptCopied: "已复制识别 Prompt",
      promptCopyFailed: "复制失败，可以手动复制 README 里的导入格式",
      pasteFirst: "先粘贴一行或多行 /add 命令",
      unknownCommand: "无法识别命令",
      addOnly: "预览导入只接收 /add；其他命令可用直接执行",
      fixBeforeImport: "有命令需要修正后再导入",
      previewReady: "预览已生成，可先修改再导入",
      nothingToImport: "没有可导入的任务",
      exported: "已导出 JSON",
      noTasksInJson: "JSON 里没有找到 tasks",
      jsonImportFailed: "JSON 导入失败",
      parsePreview: "解析预览",
      runCommand: "直接执行",
      confirmImport: "确认导入",
      notes: "备注"
    },
    time: {
      unknown: "时间未知",
      overdueUnderHour: "已超时<1小时",
      overdueHours: "已超时{count}小时",
      overdueDays: "已超时{count}天",
      underHour: "不足1小时",
      hours: "{count}小时",
      days: "{count}天"
    },
    importPrompt: `你是 Deadline Panel 的任务提取助手。请从我提供的截图、PDF、网页、邮件或聊天内容中识别明确的待办事项和截止时间，并只输出可导入命令。

输出规则：
1. 每行一条命令，不要 Markdown，不要解释。
2. 只输出有明确标题和明确截止时间的任务；日期不确定就不要输出。
3. 格式必须是：
/add title="任务标题" due="YYYY-MM-DD HH:mm" priority="high" notes="来源或简短上下文"
4. priority 只能是 urgent / high / medium / low。
5. 如果没有具体时间，默认用 23:59。
6. 标题保持简短，去掉无关说明。`
  },
  ja: {
    languageNames: {
      system: "システムに合わせる",
      zh: "中文",
      ja: "日本語",
      en: "English"
    },
    common: {
      save: "保存",
      delete: "削除",
      remove: "外す",
      complete: "完了",
      restore: "未完了に戻す",
      cancelCurrent: "進行中から外す",
      setCurrent: "進行中にする",
      actions: "操作"
    },
    status: {
      active: "未完了",
      completed: "完了",
      postponed: "延期済み"
    } satisfies Record<TaskStatus, string>,
    panel: {
      restoring: "Deadline を復元中...",
      currentPrefix: "進行中：",
      nearestPrefix: "直近：",
      emptyCollapsed: "Top 0｜Deadline なし",
      inProgress: "進行中",
      hideTemporarily: "一時的に隠す",
      currentTask: "現在のタスク",
      nearestDeadline: "直近 Deadline",
      noUrgentDeadline: "差し迫った Deadline はありません",
      recentDeadline: "直近 Deadline",
      emptyTaskList: "まずは一番近い締切を1つだけ追加しましょう。"
    },
    focus: {
      countLabel: "表示件数"
    },
    task: {
      due: "締切：",
      postponed: "延期",
      custom: "カスタム",
      title: "タイトル",
      dueLabel: "締切",
      priority: "優先度",
      example: "例：Graph Mining Quiz"
    },
    add: {
      title: "Deadline を追加",
      quickPlaceholder: "明日 23:59 Graph Mining Quiz high",
      quickParse: "解析",
      quickConfirm: "確認して追加",
      quickEmpty: "自然文で Deadline を入力してください",
      quickError: "認識できません。タイトルと締切を明確にしてください",
      quickReady: "認識しました。確認して追加できます",
      manual: "手動入力"
    },
    history: {
      title: "履歴",
      empty: "完了した記録はまだありません。"
    },
    settings: {
      title: "設定とツール",
      focusCount: "重点件数",
      focusCopy: "小さなバーと一覧には、見るべき少数のタスクだけを残します。",
      language: "言語",
      languageCopy: "UI、日付、インポート用プロンプトに反映されます。",
      autostart: "自動起動",
      autostartCopy: "PC 起動後に右下の Deadline バーを自動表示します。",
      autostartReadFailed: "自動起動の状態を読み取れません",
      dataPathReadFailed: "データ保存先を読み取れません",
      autostartOn: "自動起動をオンにしました",
      autostartOff: "自動起動をオフにしました",
      autostartFailed: "自動起動の設定に失敗しました",
      backupDone: "バックアップしました",
      backupTo: "バックアップ先",
      backupFailed: "バックアップに失敗しました",
      dataDirFailed: "データフォルダを開けません",
      resetDone: "右下に戻しました",
      resetFailed: "位置のリセットに失敗しました",
      hideFailed: "一時非表示に失敗しました",
      resetPosition: "位置を戻す",
      hide15: "15分隠す",
      hide30: "30分隠す",
      hide60: "60分隠す",
      dataDir: "データフォルダ",
      backup: "DBバックアップ"
    },
    import: {
      title: "インポート",
      copyPrompt: "認識 Prompt をコピー",
      exportJson: "JSON 出力",
      importJson: "JSON 読込",
      promptCopied: "認識 Prompt をコピーしました",
      promptCopyFailed: "コピーに失敗しました。README の形式を手動でコピーしてください",
      pasteFirst: "/add コマンドを1行以上貼り付けてください",
      unknownCommand: "コマンドを認識できません",
      addOnly: "プレビューは /add のみ対応です",
      fixBeforeImport: "修正が必要なコマンドがあります",
      previewReady: "プレビューを作成しました。編集してから取り込めます",
      nothingToImport: "取り込めるタスクがありません",
      exported: "JSON を出力しました",
      noTasksInJson: "JSON に tasks が見つかりません",
      jsonImportFailed: "JSON の読み込みに失敗しました",
      parsePreview: "プレビュー解析",
      runCommand: "直接実行",
      confirmImport: "取り込む",
      notes: "メモ"
    },
    time: {
      unknown: "日時不明",
      overdueUnderHour: "期限超過<1時間",
      overdueHours: "期限超過{count}時間",
      overdueDays: "期限超過{count}日",
      underHour: "1時間未満",
      hours: "{count}時間",
      days: "{count}日"
    },
    importPrompt: `あなたは Deadline Panel のタスク抽出アシスタントです。スクリーンショット、PDF、Webページ、メール、チャットから、明確なタスクと締切だけを抽出し、インポート可能なコマンドのみを出力してください。

出力ルール：
1. 1行につき1コマンド。Markdown や説明は不要です。
2. タイトルと締切が明確なものだけ出力してください。不確実な日付は出力しないでください。
3. 形式は必ず：
/add title="タスク名" due="YYYY-MM-DD HH:mm" priority="high" notes="出典または短い文脈"
4. priority は urgent / high / medium / low のいずれか。
5. 時刻がない場合は 23:59 を使ってください。
6. タイトルは短く、不要な説明を除いてください。`
  },
  en: {
    languageNames: {
      system: "System",
      zh: "中文",
      ja: "日本語",
      en: "English"
    },
    common: {
      save: "Save",
      delete: "Delete",
      remove: "Remove",
      complete: "Complete",
      restore: "Restore",
      cancelCurrent: "Unset current",
      setCurrent: "Set current",
      actions: "actions"
    },
    status: {
      active: "Active",
      completed: "Completed",
      postponed: "Postponed"
    } satisfies Record<TaskStatus, string>,
    panel: {
      restoring: "Restoring deadlines...",
      currentPrefix: "In progress: ",
      nearestPrefix: "Nearest: ",
      emptyCollapsed: "Top 0 | No deadlines",
      inProgress: "In Progress",
      hideTemporarily: "Hide temporarily",
      currentTask: "Current task",
      nearestDeadline: "Nearest deadline",
      noUrgentDeadline: "No urgent deadlines",
      recentDeadline: "Recent Deadlines",
      emptyTaskList: "Add just the next thing you need to hand in."
    },
    focus: {
      countLabel: "Task count"
    },
    task: {
      due: "Due: ",
      postponed: "Postpone",
      custom: "Custom",
      title: "Title",
      dueLabel: "Due",
      priority: "Priority",
      example: "e.g. Graph Mining Quiz"
    },
    add: {
      title: "Add Deadline",
      quickPlaceholder: "tomorrow 23:59 Graph Mining Quiz high",
      quickParse: "Parse",
      quickConfirm: "Confirm add",
      quickEmpty: "Type a natural-language deadline first",
      quickError: "Could not parse it. Include a clear title and due time",
      quickReady: "Parsed. Review and confirm to add",
      manual: "Manual"
    },
    history: {
      title: "History",
      empty: "No completed records yet."
    },
    settings: {
      title: "Settings & Tools",
      focusCount: "Focus count",
      focusCopy: "The strip and main list only keep the few tasks worth seeing.",
      language: "Language",
      languageCopy: "Applies to UI text, dates, and the import prompt.",
      autostart: "Launch at startup",
      autostartCopy: "Show the lower-right deadline strip when the computer starts.",
      autostartReadFailed: "Could not read startup state",
      dataPathReadFailed: "Could not read data path",
      autostartOn: "Startup enabled",
      autostartOff: "Startup disabled",
      autostartFailed: "Startup setting failed",
      backupDone: "Backup complete",
      backupTo: "Backed up to",
      backupFailed: "Backup failed",
      dataDirFailed: "Could not open data folder",
      resetDone: "Moved back to the lower-right corner",
      resetFailed: "Position reset failed",
      hideFailed: "Temporary hide failed",
      resetPosition: "Reset position",
      hide15: "Hide 15 min",
      hide30: "Hide 30 min",
      hide60: "Hide 60 min",
      dataDir: "Data folder",
      backup: "Back up DB"
    },
    import: {
      title: "Import",
      copyPrompt: "Copy recognition prompt",
      exportJson: "Export JSON",
      importJson: "Import JSON",
      promptCopied: "Recognition prompt copied",
      promptCopyFailed: "Copy failed. You can copy the import format from README",
      pasteFirst: "Paste one or more /add commands first",
      unknownCommand: "Could not recognize command",
      addOnly: "Preview import accepts /add only",
      fixBeforeImport: "Some commands need fixes before import",
      previewReady: "Preview generated. You can edit before importing",
      nothingToImport: "No importable tasks",
      exported: "JSON exported",
      noTasksInJson: "No tasks found in JSON",
      jsonImportFailed: "JSON import failed",
      parsePreview: "Parse preview",
      runCommand: "Run command",
      confirmImport: "Confirm import",
      notes: "Notes"
    },
    time: {
      unknown: "Unknown time",
      overdueUnderHour: "Overdue <1h",
      overdueHours: "Overdue {count}h",
      overdueDays: "Overdue {count}d",
      underHour: "<1h",
      hours: "{count}h",
      days: "{count}d"
    },
    importPrompt: `You are the task extraction assistant for Deadline Panel. Extract only clear tasks and due times from screenshots, PDFs, web pages, emails, or chats, and output importable commands only.

Rules:
1. One command per line. No Markdown. No explanation.
2. Output only tasks with a clear title and clear due date/time. Skip uncertain dates.
3. Required format:
/add title="Task title" due="YYYY-MM-DD HH:mm" priority="high" notes="source or short context"
4. priority must be urgent / high / medium / low.
5. If no exact time is shown, use 23:59.
6. Keep titles short and remove irrelevant wording.`
  }
};
