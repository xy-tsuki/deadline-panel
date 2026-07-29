import Foundation

enum NativeLanguage: String {
    case system
    case zh
    case ja
    case en

    static let defaultsKey = "app_language"
    static let didChangeNotification = Notification.Name("NativeLanguageDidChange")

    static var preference: NativeLanguage {
        let value = UserDefaults.standard.string(forKey: defaultsKey) ?? "system"
        return NativeLanguage(rawValue: value) ?? .system
    }

    static var resolved: NativeLanguage {
        let preference = Self.preference
        if preference != .system {
            return preference
        }
        let preferred = Locale.preferredLanguages.first?.lowercased() ?? ""
        if preferred.hasPrefix("ja") {
            return .ja
        }
        if preferred.hasPrefix("zh") {
            return .zh
        }
        return .en
    }

    @MainActor
    static func notifyChange() {
        NotificationCenter.default.post(name: didChangeNotification, object: nil)
    }
}

struct NativeStrings: Sendable {
    static var current: NativeStrings {
        strings(for: NativeLanguage.resolved)
    }

    static func strings(for language: NativeLanguage) -> NativeStrings {
        switch language {
        case .ja:
            return .ja
        case .en:
            return .en
        default:
            return .zh
        }
    }

    let noDeadlines: String
    let shownTotal: @Sendable (Int, Int) -> String
    let recentDeadline: String
    let currentSection: String
    let nearestDeadline: String
    let currentTask: String
    let noUrgentDeadlines: String
    let noTodo: String
    let completed: String
    let addDeadline: String
    let history: String
    let noHistory: String
    let cloudSync: String
    let settingsTools: String
    let importTitle: String
    let optionalCloudCopy: String
    let syncCode: String
    let saveSyncConfig: String
    let confirmGenerate: String
    let generateSyncCode: String
    let hideAdvanced: String
    let showAdvanced: String
    let syncing: String
    let syncNow: String
    let quickAddPlaceholder: String
    let parseAdd: String
    let quickAddReady: String
    let quickAddError: String
    let confirmQuickAdd: String
    let title: String
    let due: String
    let priority: String
    let notes: String
    let confirmAdd: String
    let copyPrompt: String
    let parsePreview: String
    let runCommand: String
    let confirmImport: String
    let importPromptCopied: String
    let pasteImportFirst: String
    let fixBeforeImport: String
    let previewReady: String
    let pasteSingleCommandFirst: String
    let nothingToImport: String
    let unknownCommand: String
    let addOnly: String
    let missingTitle: String
    let missingDue: String
    let dueFormatError: String
    let dueUnrecognized: String
    let priorityFormatError: String
    let commandNeedsSlash: String
    let commandNeedsTarget: String
    let commandTargetNotFound: String
    let supportedCommands: String
    let deadlineDuePrefix: String
    let postpone: String
    let plusOneDay: String
    let plusThreeDays: String
    let plusSevenDays: String
    let customPostpone: String
    let chooseDate: String
    let done: String
    let unknownTime: String
    let overdue: String
    let overdueHours: @Sendable (Int) -> String
    let overdueDays: @Sendable (Int) -> String
    let withinHour: String
    let hours: @Sendable (Int) -> String
    let days: @Sendable (Int) -> String
    let statusCompleted: String
    let statusPostponed: String
    let statusActive: String
    let settingsTitle: String
    let focusCount: String
    let focusCopy: String
    let language: String
    let languageCopy: String
    let theme: String
    let themeCopy: String
    let lightMode: String
    let darkMode: String
    let followSystem: String
    let chinese: String
    let japanese: String
    let english: String
    let autostart: String
    let autoHideFullscreen: String
    let autoHideFullscreenCopy: String
    let lowPowerMode: String
    let lowPowerModeCopy: String
    let showInDock: String
    let hideFromDock: String
    let requestNotifications: String
    let scheduleNotifications: String
    let resetPosition: String
    let resetDone: String
    let hide15: String
    let hide30: String
    let dataDirectory: String
    let backupDatabase: String
    let checkingUpdates: String
    let checkUpdates: String
    let importJSON: String
    let exportJSON: String
    let openDataFailed: String
    let databaseNotCreated: String
    let backupDone: @Sendable (String) -> String
    let backupFailed: String
    let noReleaseFound: String
    let latestVersion: @Sendable (String) -> String
    let upToDate: @Sendable (String) -> String
    let updateAvailable: @Sendable (String) -> String
    let downloadUpdate: String
    let viewRelease: String
    let cancel: String
    let updateDownloaded: @Sendable (String) -> String
    let updateCheckFailed: String
    let notificationUnchecked: String
    let notificationAllowed: String
    let notificationDenied: String
    let notificationNotDetermined: String
    let notificationProvisional: String
    let notificationEphemeral: String
    let notificationDueTitle: String
    let menuShowPanel: String
    let menuHidePanel: String
    let menuHide60: String
    let menuResetPosition: String
    let menuSettings: String
    let menuRestart: String
    let menuQuit: String
    let stripCurrentPrefix: String
    let stripTopPrefix: @Sendable (Int) -> String

    static let zh = NativeStrings(
        noDeadlines: "没有 Deadline",
        shownTotal: { "\($0) shown / \($1) total" },
        recentDeadline: "最近 Deadline",
        currentSection: "进行中",
        nearestDeadline: "最近截止",
        currentTask: "当前任务",
        noUrgentDeadlines: "没有紧急 Deadline",
        noTodo: "没有待办事项",
        completed: "已完成",
        addDeadline: "添加 Deadline",
        history: "历史记录",
        noHistory: "暂无历史记录",
        cloudSync: "云同步",
        settingsTools: "设置与工具",
        importTitle: "导入",
        optionalCloudCopy: "可选：使用同步码在多台设备之间同步任务。同步码相当于访问密钥，请妥善保存。",
        syncCode: "同步码",
        saveSyncConfig: "保存配置",
        confirmGenerate: "确认生成",
        generateSyncCode: "生成同步码",
        hideAdvanced: "隐藏高级",
        showAdvanced: "显示高级",
        syncing: "同步中...",
        syncNow: "立即同步",
        quickAddPlaceholder: "课程小测 明天 23:59 high",
        parseAdd: "解析",
        quickAddReady: "已识别，可确认添加",
        quickAddError: "无法识别，请写清标题和截止时间",
        confirmQuickAdd: "确认添加",
        title: "标题",
        due: "截止",
        priority: "优先级",
        notes: "备注",
        confirmAdd: "确认添加",
        copyPrompt: "复制识别 Prompt（外部 AI 转导入代码）",
        parsePreview: "解析预览",
        runCommand: "直接执行",
        confirmImport: "确认导入",
        importPromptCopied: "已复制识别 Prompt",
        pasteImportFirst: "先粘贴一行或多行 /add 命令",
        fixBeforeImport: "有命令需要修正后再导入",
        previewReady: "预览已生成，可先修改再导入",
        pasteSingleCommandFirst: "先粘贴一行命令",
        nothingToImport: "没有可导入的任务",
        unknownCommand: "无法识别命令",
        addOnly: "预览导入只接收 /add；其他命令可用直接执行",
        missingTitle: "缺少标题",
        missingDue: "缺少截止时间",
        dueFormatError: "截止时间格式应为 YYYY-MM-DD HH:mm",
        dueUnrecognized: "截止时间无法识别",
        priorityFormatError: "priority 只能是 low, medium, high, urgent",
        commandNeedsSlash: "命令需要以 / 开头",
        commandNeedsTarget: "命令需要 id 或 title",
        commandTargetNotFound: "没有找到对应任务",
        supportedCommands: "支持 /add, /update, /delete, /complete",
        deadlineDuePrefix: "截止",
        postpone: "延期",
        plusOneDay: "+1 天",
        plusThreeDays: "+3 天",
        plusSevenDays: "+7 天",
        customPostpone: "自定义日期",
        chooseDate: "选择日期时间",
        done: "完成",
        unknownTime: "时间未知",
        overdue: "已逾期",
        overdueHours: { "逾期 \($0) 小时" },
        overdueDays: { "逾期 \($0) 天" },
        withinHour: "1 小时内",
        hours: { "\($0) 小时" },
        days: { "\($0) 天" },
        statusCompleted: "已完成",
        statusPostponed: "已延期",
        statusActive: "未完成",
        settingsTitle: "Deadline Panel 设置",
        focusCount: "显示数量",
        focusCopy: "控制展开面板和收起条使用的 Top N",
        language: "语言",
        languageCopy: "应用界面、日期和导入提示会跟随这里切换。",
        theme: "主题",
        themeCopy: "控制面板、设置窗口和系统控件的浅色/深色外观。",
        lightMode: "浅色模式",
        darkMode: "深色模式",
        followSystem: "跟随系统",
        chinese: "中文",
        japanese: "日本語",
        english: "English",
        autostart: "开机启动",
        autoHideFullscreen: "全屏时自动隐藏",
        autoHideFullscreenCopy: "当前台应用进入全屏时隐藏收起条，离开全屏后自动恢复。",
        lowPowerMode: "低功耗模式",
        lowPowerModeCopy: "关闭过渡、缩放和淡入淡出动画，减少 CPU 与 GPU 占用。",
        showInDock: "在程序坞显示",
        hideFromDock: "从程序坞隐藏",
        requestNotifications: "请求通知权限",
        scheduleNotifications: "安排通知",
        resetPosition: "重置位置",
        resetDone: "已重新贴到右下角",
        hide15: "隐藏 15 分钟",
        hide30: "隐藏 30 分钟",
        dataDirectory: "数据目录",
        backupDatabase: "备份数据库",
        checkingUpdates: "检查中...",
        checkUpdates: "检查更新",
        importJSON: "导入 JSON",
        exportJSON: "导出 JSON",
        openDataFailed: "无法打开数据目录",
        databaseNotCreated: "数据库尚未创建",
        backupDone: { "已备份到 \($0)" },
        backupFailed: "备份失败",
        noReleaseFound: "暂未找到 release",
        latestVersion: { "最新版本：\($0)" },
        upToDate: { "已是最新版本 \($0)" },
        updateAvailable: { "发现新版本 \($0)" },
        downloadUpdate: "下载更新",
        viewRelease: "查看发布页",
        cancel: "取消",
        updateDownloaded: { "更新已下载到 \($0)" },
        updateCheckFailed: "检查更新失败",
        notificationUnchecked: "未检查",
        notificationAllowed: "已允许",
        notificationDenied: "已拒绝",
        notificationNotDetermined: "未询问",
        notificationProvisional: "临时允许",
        notificationEphemeral: "临时会话",
        notificationDueTitle: "Deadline 到期",
        menuShowPanel: "显示面板",
        menuHidePanel: "隐藏面板",
        menuHide60: "隐藏 60 分钟",
        menuResetPosition: "重新贴到右下角",
        menuSettings: "设置",
        menuRestart: "重启",
        menuQuit: "退出",
        stripCurrentPrefix: "当前",
        stripTopPrefix: { "Top \($0)" }
    )

    static let ja = NativeStrings(
        noDeadlines: "Deadline はありません",
        shownTotal: { "\($0) 件表示 / 全 \($1) 件" },
        recentDeadline: "直近の Deadline",
        currentSection: "進行中",
        nearestDeadline: "直近締切",
        currentTask: "現在のタスク",
        noUrgentDeadlines: "緊急の Deadline はありません",
        noTodo: "未完了の項目はありません",
        completed: "完了済み",
        addDeadline: "Deadline を追加",
        history: "履歴",
        noHistory: "履歴はありません",
        cloudSync: "クラウド同期",
        settingsTools: "設定とツール",
        importTitle: "インポート",
        optionalCloudCopy: "任意：同期コードを使って複数デバイス間でタスクを同期します。同期コードはアクセスキーとして扱ってください。",
        syncCode: "同期コード",
        saveSyncConfig: "設定を保存",
        confirmGenerate: "生成を確認",
        generateSyncCode: "同期コードを生成",
        hideAdvanced: "詳細を隠す",
        showAdvanced: "詳細を表示",
        syncing: "同期中...",
        syncNow: "今すぐ同期",
        quickAddPlaceholder: "小テスト 明日 23:59 high",
        parseAdd: "解析",
        quickAddReady: "認識しました。確認して追加できます",
        quickAddError: "認識できません。タイトルと締切を確認してください",
        confirmQuickAdd: "確認して追加",
        title: "タイトル",
        due: "締切",
        priority: "優先度",
        notes: "メモ",
        confirmAdd: "追加",
        copyPrompt: "認識 Prompt をコピー（外部 AI で取込コード化）",
        parsePreview: "プレビュー解析",
        runCommand: "直接実行",
        confirmImport: "インポート確定",
        importPromptCopied: "認識 Prompt をコピーしました",
        pasteImportFirst: "まず /add コマンドを 1 行以上貼り付けてください",
        fixBeforeImport: "修正が必要なコマンドがあります",
        previewReady: "プレビューを生成しました。編集してから取り込めます",
        pasteSingleCommandFirst: "まず 1 行のコマンドを貼り付けてください",
        nothingToImport: "取り込めるタスクがありません",
        unknownCommand: "コマンドを認識できません",
        addOnly: "プレビューは /add のみ対応です",
        missingTitle: "タイトルがありません",
        missingDue: "締切がありません",
        dueFormatError: "締切は YYYY-MM-DD HH:mm 形式で入力してください",
        dueUnrecognized: "締切を認識できません",
        priorityFormatError: "priority は low, medium, high, urgent のみです",
        commandNeedsSlash: "コマンドは / で始めてください",
        commandNeedsTarget: "id または title が必要です",
        commandTargetNotFound: "対象タスクが見つかりません",
        supportedCommands: "/add, /update, /delete, /complete に対応しています",
        deadlineDuePrefix: "締切",
        postpone: "延期",
        plusOneDay: "+1 日",
        plusThreeDays: "+3 日",
        plusSevenDays: "+7 日",
        customPostpone: "日時を指定",
        chooseDate: "日時を選択",
        done: "完了",
        unknownTime: "日時不明",
        overdue: "期限切れ",
        overdueHours: { "\($0) 時間超過" },
        overdueDays: { "\($0) 日超過" },
        withinHour: "1 時間以内",
        hours: { "\($0) 時間" },
        days: { "\($0) 日" },
        statusCompleted: "完了",
        statusPostponed: "延期済み",
        statusActive: "未完了",
        settingsTitle: "Deadline Panel 設定",
        focusCount: "表示件数",
        focusCopy: "展開パネルと折りたたみバーで使う Top N を指定します。",
        language: "言語",
        languageCopy: "UI、日付、インポート用 Prompt に反映されます。",
        theme: "テーマ",
        themeCopy: "パネル、設定ウィンドウ、システムコントロールの外観を切り替えます。",
        lightMode: "ライト",
        darkMode: "ダーク",
        followSystem: "システムに合わせる",
        chinese: "中文",
        japanese: "日本語",
        english: "English",
        autostart: "自動起動",
        autoHideFullscreen: "フルスクリーン時に自動で隠す",
        autoHideFullscreenCopy: "前面のアプリがフルスクリーンのときにバーを隠し、戻ると自動で表示します。",
        lowPowerMode: "低電力モード",
        lowPowerModeCopy: "トランジション、拡大縮小、フェードを無効にして CPU と GPU の負荷を抑えます。",
        showInDock: "Dock に表示",
        hideFromDock: "Dock から隠す",
        requestNotifications: "通知権限を要求",
        scheduleNotifications: "通知を予約",
        resetPosition: "位置をリセット",
        resetDone: "右下に戻しました",
        hide15: "15 分隠す",
        hide30: "30 分隠す",
        dataDirectory: "データフォルダ",
        backupDatabase: "DB をバックアップ",
        checkingUpdates: "確認中...",
        checkUpdates: "更新を確認",
        importJSON: "JSON をインポート",
        exportJSON: "JSON をエクスポート",
        openDataFailed: "データフォルダを開けません",
        databaseNotCreated: "データベースはまだ作成されていません",
        backupDone: { "\($0) にバックアップしました" },
        backupFailed: "バックアップに失敗しました",
        noReleaseFound: "release が見つかりません",
        latestVersion: { "最新バージョン：\($0)" },
        upToDate: { "最新バージョン \($0) を使用中です" },
        updateAvailable: { "新しいバージョン \($0) があります" },
        downloadUpdate: "更新をダウンロード",
        viewRelease: "リリースページを表示",
        cancel: "キャンセル",
        updateDownloaded: { "更新を \($0) にダウンロードしました" },
        updateCheckFailed: "更新確認に失敗しました",
        notificationUnchecked: "未確認",
        notificationAllowed: "許可済み",
        notificationDenied: "拒否済み",
        notificationNotDetermined: "未確認",
        notificationProvisional: "暫定許可",
        notificationEphemeral: "一時セッション",
        notificationDueTitle: "Deadline の締切",
        menuShowPanel: "パネルを表示",
        menuHidePanel: "パネルを隠す",
        menuHide60: "60 分隠す",
        menuResetPosition: "右下に戻す",
        menuSettings: "設定",
        menuRestart: "再起動",
        menuQuit: "終了",
        stripCurrentPrefix: "現在",
        stripTopPrefix: { "Top \($0)" }
    )

    static let en = NativeStrings(
        noDeadlines: "No Deadlines",
        shownTotal: { "\($0) shown / \($1) total" },
        recentDeadline: "Recent Deadlines",
        currentSection: "In Progress",
        nearestDeadline: "Nearest Deadline",
        currentTask: "Current Task",
        noUrgentDeadlines: "No urgent deadlines",
        noTodo: "No pending items",
        completed: "Completed",
        addDeadline: "Add Deadline",
        history: "History",
        noHistory: "No history yet",
        cloudSync: "Cloud Sync",
        settingsTools: "Settings & Tools",
        importTitle: "Import",
        optionalCloudCopy: "Optional: sync tasks across devices with a sync code. Treat it like an access key.",
        syncCode: "Sync code",
        saveSyncConfig: "Save config",
        confirmGenerate: "Confirm generate",
        generateSyncCode: "Generate sync code",
        hideAdvanced: "Hide advanced",
        showAdvanced: "Show advanced",
        syncing: "Syncing...",
        syncNow: "Sync now",
        quickAddPlaceholder: "Quiz tomorrow 23:59 high",
        parseAdd: "Parse",
        quickAddReady: "Recognized. Confirm to add",
        quickAddError: "Could not recognize a title and due time",
        confirmQuickAdd: "Confirm add",
        title: "Title",
        due: "Due",
        priority: "Priority",
        notes: "Notes",
        confirmAdd: "Add",
        copyPrompt: "Copy recognition prompt",
        parsePreview: "Parse preview",
        runCommand: "Run command",
        confirmImport: "Confirm import",
        importPromptCopied: "Recognition prompt copied",
        pasteImportFirst: "Paste one or more /add commands first",
        fixBeforeImport: "Some commands need fixes before import",
        previewReady: "Preview ready. You can edit before importing",
        pasteSingleCommandFirst: "Paste one command first",
        nothingToImport: "Nothing to import",
        unknownCommand: "Could not recognize command",
        addOnly: "Preview import accepts /add only",
        missingTitle: "Missing title",
        missingDue: "Missing due time",
        dueFormatError: "Due time must be YYYY-MM-DD HH:mm",
        dueUnrecognized: "Could not recognize due time",
        priorityFormatError: "priority must be low, medium, high, or urgent",
        commandNeedsSlash: "Command must start with /",
        commandNeedsTarget: "Command needs id or title",
        commandTargetNotFound: "No matching task found",
        supportedCommands: "Supported commands: /add, /update, /delete, /complete",
        deadlineDuePrefix: "Due",
        postpone: "Postpone",
        plusOneDay: "+1 day",
        plusThreeDays: "+3 days",
        plusSevenDays: "+7 days",
        customPostpone: "Custom date",
        chooseDate: "Choose date and time",
        done: "Done",
        unknownTime: "Unknown time",
        overdue: "Overdue",
        overdueHours: { "\($0)h overdue" },
        overdueDays: { "\($0)d overdue" },
        withinHour: "Within 1h",
        hours: { "\($0)h" },
        days: { "\($0)d" },
        statusCompleted: "Completed",
        statusPostponed: "Postponed",
        statusActive: "Pending",
        settingsTitle: "Deadline Panel Settings",
        focusCount: "Focus count",
        focusCopy: "Controls the Top N used by the expanded panel and strip.",
        language: "Language",
        languageCopy: "Applies to UI, dates, and import prompts.",
        theme: "Theme",
        themeCopy: "Controls the light or dark appearance of panels and system controls.",
        lightMode: "Light",
        darkMode: "Dark",
        followSystem: "Follow system",
        chinese: "中文",
        japanese: "日本語",
        english: "English",
        autostart: "Launch at startup",
        autoHideFullscreen: "Auto-hide in fullscreen",
        autoHideFullscreenCopy: "Hide the strip while the foreground app is fullscreen, then restore it when fullscreen ends.",
        lowPowerMode: "Low Power Mode",
        lowPowerModeCopy: "Disable transitions, scaling, and fades to reduce CPU and GPU usage.",
        showInDock: "Show in Dock",
        hideFromDock: "Hide from Dock",
        requestNotifications: "Request notifications",
        scheduleNotifications: "Schedule notifications",
        resetPosition: "Reset position",
        resetDone: "Moved back to lower-right",
        hide15: "Hide 15 minutes",
        hide30: "Hide 30 minutes",
        dataDirectory: "Data folder",
        backupDatabase: "Back up database",
        checkingUpdates: "Checking...",
        checkUpdates: "Check updates",
        importJSON: "Import JSON",
        exportJSON: "Export JSON",
        openDataFailed: "Could not open data folder",
        databaseNotCreated: "Database has not been created yet",
        backupDone: { "Backed up to \($0)" },
        backupFailed: "Backup failed",
        noReleaseFound: "No release found",
        latestVersion: { "Latest version: \($0)" },
        upToDate: { "You are up to date (\($0))" },
        updateAvailable: { "Version \($0) is available" },
        downloadUpdate: "Download update",
        viewRelease: "View release",
        cancel: "Cancel",
        updateDownloaded: { "Update downloaded to \($0)" },
        updateCheckFailed: "Update check failed",
        notificationUnchecked: "Unchecked",
        notificationAllowed: "Allowed",
        notificationDenied: "Denied",
        notificationNotDetermined: "Not asked",
        notificationProvisional: "Provisional",
        notificationEphemeral: "Ephemeral",
        notificationDueTitle: "Deadline due",
        menuShowPanel: "Show Panel",
        menuHidePanel: "Hide Panel",
        menuHide60: "Hide 60 minutes",
        menuResetPosition: "Reset to lower-right",
        menuSettings: "Settings",
        menuRestart: "Restart",
        menuQuit: "Quit",
        stripCurrentPrefix: "Current",
        stripTopPrefix: { "Top \($0)" }
    )
}
