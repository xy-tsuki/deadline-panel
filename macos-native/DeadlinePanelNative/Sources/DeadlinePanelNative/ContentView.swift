import AppKit
import SwiftUI

struct ContentView: View {
    @ObservedObject var viewModel: DeadlineViewModel
    @ObservedObject var cloudSyncController: NativeCloudSyncController
    let onHideTemporarily: (() -> Void)?
    let onImportJSON: (() -> Void)?
    let onExportJSON: (() -> Void)?
    let onOpenSettings: (() -> Void)?
    let onControlInteractionChanged: ((Bool) -> Void)?

    @AppStorage(NativeLanguage.defaultsKey) private var languagePreference = NativeLanguage.system.rawValue
    @AppStorage(NativeAppearance.defaultsKey) private var appearanceMode = "system"

    init(
        viewModel: DeadlineViewModel,
        cloudSyncController: NativeCloudSyncController,
        onHideTemporarily: (() -> Void)? = nil,
        onImportJSON: (() -> Void)? = nil,
        onExportJSON: (() -> Void)? = nil,
        onOpenSettings: (() -> Void)? = nil,
        onControlInteractionChanged: ((Bool) -> Void)? = nil
    ) {
        self.viewModel = viewModel
        self.cloudSyncController = cloudSyncController
        self.onHideTemporarily = onHideTemporarily
        self.onImportJSON = onImportJSON
        self.onExportJSON = onExportJSON
        self.onOpenSettings = onOpenSettings
        self.onControlInteractionChanged = onControlInteractionChanged
    }

    var body: some View {
        ZStack {
            NativeWindowBackground()

            ScrollViewReader { proxy in
                ScrollView {
                    VStack(spacing: 0) {
                        Color.clear
                            .frame(height: 0)
                            .id(PanelScrollAnchor.top)
                        notices
                        FocusSummarySection(
                            viewModel: viewModel,
                            onHideTemporarily: onHideTemporarily
                        )
                        RecentDeadlineSection(
                            viewModel: viewModel,
                            onUpdate: { task, title, dueAt, notes in
                                viewModel.updateManual(task: task, title: title, dueAt: dueAt, priority: task.priority, notes: notes)
                            },
                            onComplete: { viewModel.complete($0) },
                            onRestore: { viewModel.restore($0) },
                            onToggleCurrent: { viewModel.toggleCurrent($0) },
                            onUpdatePriority: { task, priority in
                                viewModel.updatePriority(task, priority: priority)
                            },
                            onPostpone: { task, days in viewModel.postpone(task, days: days) },
                            onPostponeDate: { task, date in viewModel.postpone(task, to: date) },
                            onDelete: { viewModel.delete($0) },
                            onControlInteractionChanged: onControlInteractionChanged
                        )
                        PanelToolsSection(
                            viewModel: viewModel,
                            cloudSyncController: cloudSyncController,
                            onUpdateCompleted: { task, title, dueAt, notes in
                                viewModel.updateManual(task: task, title: title, dueAt: dueAt, priority: task.priority, notes: notes)
                            },
                            onRestoreCompleted: { viewModel.restore($0) },
                            onDeleteCompleted: { viewModel.delete($0) },
                            onOpenSettings: onOpenSettings,
                            onContentCreated: {
                                DispatchQueue.main.async {
                                    withAnimation(.spring(response: 0.32, dampingFraction: 0.84)) {
                                        proxy.scrollTo(PanelScrollAnchor.top, anchor: .top)
                                    }
                                }
                            },
                            onControlInteractionChanged: onControlInteractionChanged
                        )
                        footer
                    }
                    .padding(12)
                }
            }
        }
        .task {
            viewModel.load()
        }
        .nativePreferredColorScheme(appearanceMode)
    }

    private enum PanelScrollAnchor: Hashable {
        case top
    }

    private var notices: some View {
        VStack(spacing: 6) {
            if let errorMessage = viewModel.errorMessage {
                Text(errorMessage)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            if let commandMessage = viewModel.commandMessage {
                Text(commandMessage)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }

    private var footer: some View {
        let strings = NativeStrings.current
        return HStack {
            Text(strings.shownTotal(viewModel.focusDeadlines.count, viewModel.deadlines.count))
            Spacer()
            Text("0.6.2")
        }
        .font(.caption.weight(.semibold))
        .foregroundStyle(.secondary)
        .padding(.horizontal, 3)
        .padding(.top, 10)
    }
}

struct FocusSummarySection: View {
    @ObservedObject var viewModel: DeadlineViewModel
    let onHideTemporarily: (() -> Void)?
    @Environment(\.colorScheme) private var colorScheme

    private var firstReminder: DeadlineTask? {
        viewModel.currentDeadlines.first ?? viewModel.focusDeadlines.first
    }

    var body: some View {
        let strings = NativeStrings.current
        PanelSection {
            HStack {
                Text(viewModel.currentDeadlines.isEmpty ? strings.nearestDeadline : strings.currentSection)
                    .font(.callout.weight(.semibold))
                    .foregroundStyle(.secondary)
                Spacer()
                Button {
                    onHideTemporarily?()
                } label: {
                    Image(systemName: "eye.slash")
                }
                .buttonStyle(.borderless)
                .foregroundStyle(.secondary)
            }

            if let task = firstReminder {
                Text(task.title)
                    .font(.system(size: 21, weight: .semibold))
                    .lineLimit(2)
                    .fixedSize(horizontal: false, vertical: true)
                    .foregroundStyle(focusTitleColor)
                    .padding(.top, 4)

                HStack(spacing: 6) {
                    SummaryPill(text: viewModel.currentDeadlines.isEmpty ? strings.nearestDeadline : strings.currentTask)
                    SummaryPill(text: relativeDueText(task.dueAt, strings: strings))
                    GlassTag(text: task.priority, color: priorityColor(task.priority))
                }
                .padding(.top, 6)

                if viewModel.currentDeadlines.count > 1 {
                    HStack(spacing: 6) {
                        ForEach(viewModel.currentDeadlines.dropFirst()) { task in
                            SummaryPill(text: task.title)
                        }
                    }
                    .padding(.top, 5)
                }
            } else {
                Text(strings.noUrgentDeadlines)
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .padding(.top, 4)
            }
        }
    }

    private var focusTitleColor: Color {
        colorScheme == .light ? Color.black.opacity(0.76) : Color.primary
    }
}

struct RecentDeadlineSection: View {
    @ObservedObject var viewModel: DeadlineViewModel
    let onUpdate: (DeadlineTask, String, Date, String) -> Void
    let onComplete: (DeadlineTask) -> Void
    let onRestore: (DeadlineTask) -> Void
    let onToggleCurrent: (DeadlineTask) -> Void
    let onUpdatePriority: (DeadlineTask, String) -> Void
    let onPostpone: (DeadlineTask, Int) -> Void
    let onPostponeDate: (DeadlineTask, Date) -> Void
    let onDelete: (DeadlineTask) -> Void
    let onControlInteractionChanged: ((Bool) -> Void)?

    var body: some View {
        let strings = NativeStrings.current
        PanelSection {
            HStack(spacing: 10) {
                Text(strings.recentDeadline)
                    .font(.callout.weight(.semibold))
                    .foregroundStyle(.secondary)
                Spacer()
                LiquidSegmentedControl(selection: $viewModel.focusLimit, values: [3, 5, 10])
            }

            if viewModel.focusDeadlines.isEmpty {
                Text(strings.noTodo)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .padding(.top, 8)
            } else {
                let tasks = viewModel.focusDeadlines
                VStack(spacing: 0) {
                    ForEach(Array(tasks.enumerated()), id: \.element.id) { index, task in
                        CompactTaskRow(
                            task: task,
                            index: index + 1,
                            showsDivider: index < tasks.count - 1,
                            onUpdate: onUpdate,
                            onComplete: onComplete,
                            onRestore: onRestore,
                            onToggleCurrent: onToggleCurrent,
                            onUpdatePriority: onUpdatePriority,
                            onPostpone: onPostpone,
                            onPostponeDate: onPostponeDate,
                            onDelete: onDelete,
                            onControlInteractionChanged: onControlInteractionChanged
                        )
                    }
                }
                .padding(.top, 4)
            }
        }
    }
}

struct CompletedSection: View {
    let tasks: [DeadlineTask]
    let onUpdate: (DeadlineTask, String, Date, String) -> Void
    let onRestore: (DeadlineTask) -> Void
    let onDelete: (DeadlineTask) -> Void
    let onControlInteractionChanged: ((Bool) -> Void)?

    var body: some View {
        let strings = NativeStrings.current
        if !tasks.isEmpty {
            PanelSection {
                Text(strings.completed)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)

                VStack(spacing: 0) {
                    ForEach(tasks.prefix(6)) { task in
                        CompactTaskRow(
                            task: task,
                            index: nil,
                            onUpdate: onUpdate,
                            onComplete: { _ in },
                            onRestore: onRestore,
                            onToggleCurrent: { _ in },
                            onUpdatePriority: { _, _ in },
                            onPostpone: { _, _ in },
                            onPostponeDate: { _, _ in },
                            onDelete: onDelete,
                            onControlInteractionChanged: onControlInteractionChanged
                        )
                    }
                }
                .padding(.top, 4)
            }
        }
    }
}

struct PanelToolsSection: View {
    @ObservedObject var viewModel: DeadlineViewModel
    @ObservedObject var cloudSyncController: NativeCloudSyncController
    let onUpdateCompleted: (DeadlineTask, String, Date, String) -> Void
    let onRestoreCompleted: (DeadlineTask) -> Void
    let onDeleteCompleted: (DeadlineTask) -> Void
    let onOpenSettings: (() -> Void)?
    let onContentCreated: () -> Void
    let onControlInteractionChanged: ((Bool) -> Void)?

    @State private var expandedSection: ExpandedSection?

    var body: some View {
        let strings = NativeStrings.current
        VStack(alignment: .leading, spacing: 0) {
            PanelSection {
                VStack(alignment: .leading, spacing: 10) {
                    LiquidToolButton(title: strings.addDeadline, systemImage: "plus") {
                        withAnimation(.spring(response: 0.28, dampingFraction: 0.78)) {
                            toggle(.add)
                        }
                    }

                    if expandedSection == .add {
                        InlineAddPanel(
                            viewModel: viewModel,
                            onCreated: finishCreatingContent,
                            onControlInteractionChanged: onControlInteractionChanged
                        )
                            .transition(.liquidDisclosure)
                    }

                    HStack(spacing: 8) {
                        LiquidToolButton(
                            title: strings.history,
                            systemImage: "clock.arrow.circlepath",
                            badge: String(viewModel.completedDeadlines.count)
                        ) {
                            withAnimation(.spring(response: 0.28, dampingFraction: 0.78)) {
                                toggle(.history)
                            }
                        }
                        Spacer()
                    }

                    if expandedSection == .history {
                        if viewModel.completedDeadlines.isEmpty {
                            Text(strings.noHistory)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .padding(.leading, 4)
                        } else {
                            VStack(spacing: 0) {
                                let tasks = viewModel.completedDeadlines
                                ForEach(Array(tasks.enumerated()), id: \.element.id) { index, task in
                                    CompactTaskRow(
                                        task: task,
                                        index: nil,
                                        showsDivider: index < tasks.count - 1,
                                        onUpdate: onUpdateCompleted,
                                        onComplete: { _ in },
                                        onRestore: onRestoreCompleted,
                                        onToggleCurrent: { _ in },
                                        onUpdatePriority: { _, _ in },
                                        onPostpone: { _, _ in },
                                        onPostponeDate: { _, _ in },
                                        onDelete: onDeleteCompleted,
                                        onControlInteractionChanged: onControlInteractionChanged
                                    )
                                }
                            }
                            .transition(.liquidDisclosure)
                        }
                    }

                    CloudSyncPanel(
                        controller: cloudSyncController,
                        isOpen: Binding(
                            get: { expandedSection == .cloud },
                            set: { expandedSection = $0 ? .cloud : nil }
                        )
                    )

                    LiquidToolButton(title: strings.settingsTools, systemImage: "gearshape") {
                        onOpenSettings?()
                    }

                    LiquidToolButton(title: strings.importTitle, systemImage: "plus") {
                        withAnimation(.spring(response: 0.28, dampingFraction: 0.78)) {
                            toggle(.importCommands)
                        }
                    }

                    if expandedSection == .importCommands {
                        ImportCommandPanel(
                            viewModel: viewModel,
                            onImported: finishCreatingContent
                        )
                            .transition(.liquidDisclosure)
                    }
                }
            }
        }
    }

    private enum ExpandedSection {
        case add
        case history
        case cloud
        case importCommands
    }

    private func toggle(_ section: ExpandedSection) {
        expandedSection = expandedSection == section ? nil : section
    }

    private func finishCreatingContent() {
        withAnimation(.spring(response: 0.28, dampingFraction: 0.82)) {
            expandedSection = nil
        }
        onContentCreated()
    }
}

struct CloudSyncPanel: View {
    @ObservedObject var controller: NativeCloudSyncController
    @Binding var isOpen: Bool

    var body: some View {
        let strings = NativeStrings.current
        VStack(alignment: .leading, spacing: 10) {
            LiquidToolButton(title: strings.cloudSync, systemImage: "cloud") {
                withAnimation(.spring(response: 0.28, dampingFraction: 0.82)) {
                    isOpen.toggle()
                }
            }

            if isOpen {
                GlassPanel(cornerRadius: 14, material: .thinMaterial) {
                    VStack(alignment: .leading, spacing: 10) {
                        Text(strings.optionalCloudCopy)
                            .font(.caption)
                            .foregroundStyle(.secondary)

                        HStack(spacing: 8) {
                            TextField(strings.syncCode, text: $controller.syncCode)
                                .textFieldStyle(.roundedBorder)
                            LiquidIconButton(systemImage: "doc.on.doc", tint: .secondary) {
                                controller.copySyncCode()
                            }
                        }

                        if controller.showAdvanced {
                            TextField("Supabase URL", text: $controller.syncURL)
                                .textFieldStyle(.roundedBorder)
                            SecureField("Supabase Anon Key", text: $controller.anonKey)
                                .textFieldStyle(.roundedBorder)
                        }

                        HStack(spacing: 8) {
                            LiquidToolButton(title: strings.saveSyncConfig, systemImage: "cloud") {
                                controller.saveSettings()
                            }
                            LiquidToolButton(
                                title: controller.isConfirmingCodeReset ? strings.confirmGenerate : strings.generateSyncCode,
                                systemImage: "arrow.clockwise"
                            ) {
                                controller.generateOrConfirmSyncCode()
                            }
                        }

                        HStack(spacing: 8) {
                            LiquidToolButton(
                                title: controller.showAdvanced ? strings.hideAdvanced : strings.showAdvanced,
                                systemImage: "key"
                            ) {
                                controller.showAdvanced.toggle()
                            }
                            LiquidToolButton(title: controller.isPending ? strings.syncing : strings.syncNow, systemImage: "arrow.triangle.2.circlepath", isDisabled: controller.isPending) {
                                controller.syncNow()
                            }
                        }

                        if let message = controller.message {
                            Text(message)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                    .padding(12)
                }
                .transition(.liquidDisclosure)
            }
        }
    }
}

struct InlineAddPanel: View {
    @ObservedObject var viewModel: DeadlineViewModel
    let onCreated: () -> Void
    let onControlInteractionChanged: ((Bool) -> Void)?
    @State private var quickInput = ""
    @State private var quickPreview: NewDeadlineInput?
    @State private var quickMessage: String?
    @State private var title = ""
    @State private var dueAt = Date().addingTimeInterval(2 * 24 * 60 * 60)
    @State private var priority = "medium"
    @State private var notes = ""

    var body: some View {
        let strings = NativeStrings.current
        GlassPanel(cornerRadius: 14, material: .thinMaterial) {
            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 8) {
                    TextField(strings.quickAddPlaceholder, text: $quickInput)
                        .textFieldStyle(.roundedBorder)
                        .onSubmit {
                            parseQuickInput()
                        }
                        .onChange(of: quickInput) { _, _ in
                            quickPreview = nil
                            quickMessage = nil
                        }
                    LiquidToolButton(title: strings.parseAdd, systemImage: "return") {
                        parseQuickInput()
                    }
                }

                if let quickPreview {
                    HStack(spacing: 10) {
                        VStack(alignment: .leading, spacing: 3) {
                            Text(quickPreview.title)
                                .font(.system(size: 13, weight: .semibold))
                                .lineLimit(1)
                            Text("\(formattedDue(quickPreview.dueAt)) · \(quickPreview.priority)")
                                .font(.system(size: 11))
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        LiquidToolButton(title: strings.confirmQuickAdd, systemImage: "checkmark") {
                            guard viewModel.createDeadline(quickPreview) else {
                                return
                            }
                            quickInput = ""
                            self.quickPreview = nil
                            quickMessage = nil
                            onCreated()
                        }
                    }
                    .padding(8)
                    .background(.white.opacity(0.06), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                }

                if let quickMessage {
                    Text(quickMessage)
                        .font(.caption)
                        .foregroundStyle(quickPreview == nil ? .red : .secondary)
                }

                Divider().opacity(0.28)

                TextField(strings.title, text: $title)
                    .textFieldStyle(.roundedBorder)

                DueInputControl(
                    label: strings.due,
                    date: $dueAt,
                    onControlInteractionChanged: nil
                )

                HStack(spacing: 8) {
                    Picker(strings.priority, selection: $priority) {
                        Text("urgent").tag("urgent")
                        Text("high").tag("high")
                        Text("medium").tag("medium")
                        Text("low").tag("low")
                    }
                    .pickerStyle(.menu)

                    TextField(strings.notes, text: $notes)
                        .textFieldStyle(.roundedBorder)
                }

                HStack {
                    Spacer()
                    LiquidToolButton(title: strings.confirmAdd, systemImage: "checkmark") {
                        let trimmedTitle = title.trimmingCharacters(in: .whitespacesAndNewlines)
                        guard !trimmedTitle.isEmpty else {
                            return
                        }
                        guard viewModel.createManual(title: trimmedTitle, dueAt: dueAt, priority: priority, notes: notes) else {
                            return
                        }
                        title = ""
                        notes = ""
                        priority = "medium"
                        onCreated()
                    }
                }
            }
            .padding(12)
        }
        .onAppear {
            onControlInteractionChanged?(true)
        }
        .onDisappear {
            onControlInteractionChanged?(false)
        }
    }

    private func parseQuickInput() {
        let strings = NativeStrings.current
        quickPreview = viewModel.parseQuickDeadline(quickInput)
        quickMessage = quickPreview == nil ? strings.quickAddError : strings.quickAddReady
    }
}

struct DueInputControl: View {
    let label: String
    @Binding var date: Date
    var font: Font = .footnote
    let onControlInteractionChanged: ((Bool) -> Void)?

    @State private var text = ""
    @FocusState private var isFocused: Bool

    var body: some View {
        HStack(spacing: 8) {
            Text(label)
                .font(font.weight(.semibold))
                .foregroundStyle(.secondary)
                .frame(width: 44, alignment: .leading)

            TextField("yyyy-MM-dd HH:mm", text: $text)
                .textFieldStyle(.roundedBorder)
                .font(font)
                .focused($isFocused)
                .onSubmit(commitText)

            DatePickerPopoverButton(
                date: $date,
                onControlInteractionChanged: onControlInteractionChanged
            )
        }
        .onAppear {
            text = formattedEditableDue(date)
        }
        .onChange(of: date) { _, nextDate in
            text = formattedEditableDue(nextDate)
        }
        .onChange(of: text) { _, nextText in
            if let parsedDate = parseEditableDue(nextText) {
                date = parsedDate
            }
        }
        .onChange(of: isFocused) { _, focused in
            if focused {
                onControlInteractionChanged?(true)
            }
            if !focused {
                commitText()
            }
        }
    }

    private func commitText() {
        guard let parsedDate = parseEditableDue(text) else {
            text = formattedEditableDue(date)
            return
        }
        date = parsedDate
        text = formattedEditableDue(parsedDate)
    }
}

struct DatePickerPopoverButton: View {
    @Binding var date: Date
    let onControlInteractionChanged: ((Bool) -> Void)?
    @State private var isPresented = false

    var body: some View {
        let strings = NativeStrings.current
        LiquidIconButton(systemImage: "calendar", tint: .secondary) {
            isPresented.toggle()
        }
        .help(strings.chooseDate)
        .popover(isPresented: $isPresented, arrowEdge: .bottom) {
            VStack(alignment: .trailing, spacing: 10) {
                DatePicker(
                    strings.chooseDate,
                    selection: $date,
                    displayedComponents: [.date, .hourAndMinute]
                )
                .datePickerStyle(.graphical)
                .labelsHidden()

                Button(strings.done) {
                    isPresented = false
                }
                .keyboardShortcut(.defaultAction)
            }
            .padding(14)
        }
        .onChange(of: isPresented) { _, presented in
            onControlInteractionChanged?(presented)
        }
    }
}

struct ImportCommandPanel: View {
    @ObservedObject var viewModel: DeadlineViewModel
    let onImported: () -> Void
    @State private var rawImport = ""
    @State private var previewRows: [ImportPreviewRow] = []
    @State private var localMessage: String?

    var body: some View {
        let strings = NativeStrings.current
        GlassPanel(cornerRadius: 14, material: .thinMaterial) {
            VStack(alignment: .leading, spacing: 10) {
                LiquidToolButton(title: strings.copyPrompt, systemImage: "doc.on.doc") {
                    copyImportPrompt()
                }

                ZStack(alignment: .topLeading) {
                    TextEditor(text: $rawImport)
                        .font(.system(size: 12, design: .monospaced))
                        .frame(minHeight: 86)
                        .scrollContentBackground(.hidden)

                    if rawImport.isEmpty {
                        Text(#"/add title="Graph Mining Quiz" due="2026-06-23 23:59" priority="high""#)
                            .font(.system(size: 12, design: .monospaced))
                            .foregroundStyle(.secondary.opacity(0.65))
                            .padding(.horizontal, 5)
                            .padding(.vertical, 8)
                            .allowsHitTesting(false)
                    }
                }
                .background(.black.opacity(0.16), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay(
                    RoundedRectangle(cornerRadius: 10, style: .continuous)
                        .stroke(.white.opacity(0.12), lineWidth: 1)
                )

                HStack(spacing: 8) {
                    LiquidToolButton(title: strings.parsePreview, systemImage: "sparkles") {
                        parsePreview()
                    }
                    LiquidToolButton(title: strings.runCommand, systemImage: "play") {
                        executeSingleCommand()
                    }
                }

                if !previewRows.isEmpty {
                    VStack(alignment: .leading, spacing: 8) {
                        ForEach($previewRows) { $row in
                            ImportPreviewRowView(row: $row) {
                                previewRows.removeAll { $0.id == row.id }
                            }
                        }

                        HStack {
                            Spacer()
                            LiquidToolButton(title: strings.confirmImport, systemImage: "checkmark") {
                                confirmImport()
                            }
                        }
                    }
                }

                if let localMessage {
                    Text(localMessage)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            .padding(12)
        }
    }

    private func copyImportPrompt() {
        let language = NativeLanguage.resolved
        let prompt: String
        switch language {
        case .ja:
            prompt = """
            あなたは Deadline Panel のタスク抽出アシスタントです。スクリーンショット、PDF、Web ページ、メール、チャットから明確なタスクと締切だけを抽出し、インポート可能なコマンドのみを出力してください。

            出力ルール：
            1. 1 行に 1 コマンド。Markdown や説明は不要。
            2. 明確なタイトルと締切があるタスクだけを出力。不確かな日付は出力しない。
            3. 形式：
            /add title="タスク名" due="YYYY-MM-DD HH:mm" priority="high" notes="出典や短い文脈"
            4. priority は urgent / high / medium / low のみ。
            5. 時刻がない場合は 23:59 を使う。
            6. タイトルは短く、不要な説明は削る。
            """
        case .en:
            prompt = """
            You are the task extraction assistant for Deadline Panel. Extract only clear tasks and due times from screenshots, PDFs, web pages, emails, or chats, and output importable commands only.

            Rules:
            1. One command per line. Do not output Markdown or explanations.
            2. Only output tasks with a clear title and due time. Skip uncertain dates.
            3. Format:
            /add title="Task title" due="YYYY-MM-DD HH:mm" priority="high" notes="source or short context"
            4. priority must be urgent / high / medium / low.
            5. If no specific time is provided, use 23:59.
            6. Keep titles short and remove irrelevant details.
            """
        default:
            prompt = """
            你是 Deadline Panel 的任务提取助手。请从我提供的截图、PDF、网页、邮件或聊天内容中识别明确的待办事项和截止时间，并只输出可导入命令。

            输出规则：
            1. 每行一条命令，不要 Markdown，不要解释。
            2. 只输出有明确标题和明确截止时间的任务；日期不确定就不要输出。
            3. 格式必须是：
            /add title="任务标题" due="YYYY-MM-DD HH:mm" priority="high" notes="来源或简短上下文"
            4. priority 只能是 urgent / high / medium / low。
            5. 如果没有具体时间，默认用 23:59。
            6. 标题保持简短，去掉无关说明。
            """
        }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(prompt, forType: .string)
        localMessage = NativeStrings.current.importPromptCopied
    }

    private func parsePreview() {
        let rows = rawImport
            .split(whereSeparator: \.isNewline)
            .map { String($0).trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
            .enumerated()
            .map { index, line in ImportPreviewRow.parse(line: line, lineNumber: index + 1) }

        previewRows = rows
        if rows.isEmpty {
            localMessage = NativeStrings.current.pasteImportFirst
        } else if rows.contains(where: { !$0.isValid }) {
            localMessage = NativeStrings.current.fixBeforeImport
        } else {
            localMessage = NativeStrings.current.previewReady
        }
    }

    private func executeSingleCommand() {
        let trimmed = rawImport.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            localMessage = NativeStrings.current.pasteSingleCommandFirst
            return
        }

        viewModel.runCommand(trimmed)
        previewRows = []
    }

    private func confirmImport() {
        let inputs = previewRows.compactMap(\.newDeadlineInput)
        guard !inputs.isEmpty else {
            localMessage = NativeStrings.current.nothingToImport
            return
        }

        guard viewModel.importDeadlineInputs(inputs) else {
            return
        }
        rawImport = ""
        previewRows = []
        localMessage = nil
        onImported()
    }
}

struct ImportPreviewRow: Identifiable {
    let id = UUID()
    let lineNumber: Int
    var title: String
    var dueAt: Date
    var priority: String
    var notes: String
    var error: String?

    var isValid: Bool {
        error == nil && !title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var newDeadlineInput: NewDeadlineInput? {
        guard isValid else {
            return nil
        }
        return NewDeadlineInput(
            title: title,
            dueAt: ISO8601DateFormatter.deadlinePanelString(from: dueAt),
            priority: priority,
            notes: notes.isEmpty ? nil : notes,
            source: "command"
        )
    }

    static func parse(line: String, lineNumber: Int) -> ImportPreviewRow {
        let strings = NativeStrings.current
        let result = NativeCommandParser.parse(line)
        guard let command = result.command, result.isValid else {
            return ImportPreviewRow(
                lineNumber: lineNumber,
                title: "",
                dueAt: Date(),
                priority: "medium",
                notes: "",
                error: result.error ?? strings.unknownCommand
            )
        }
        guard case .add(let input) = command else {
            return ImportPreviewRow(
                lineNumber: lineNumber,
                title: "",
                dueAt: Date(),
                priority: "medium",
                notes: "",
                error: strings.addOnly
            )
        }
        let date = ISO8601DateFormatter.deadlinePanelDate(from: input.dueAt) ?? Date()
        return ImportPreviewRow(
            lineNumber: lineNumber,
            title: input.title,
            dueAt: date,
            priority: input.priority,
            notes: input.notes ?? "",
            error: nil
        )
    }
}

struct ImportPreviewRowView: View {
    @Binding var row: ImportPreviewRow
    let onDelete: () -> Void

    var body: some View {
        let strings = NativeStrings.current
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                Text(String(row.lineNumber))
                    .font(.caption.weight(.semibold))
                    .frame(width: 22, height: 22)
                    .background(.white.opacity(0.08), in: RoundedRectangle(cornerRadius: 7, style: .continuous))

                TextField(strings.title, text: $row.title)
                    .textFieldStyle(.roundedBorder)

                LiquidIconButton(systemImage: "trash", tint: .secondary, action: onDelete)
            }

            if row.error == nil {
                DatePicker(strings.due, selection: $row.dueAt)
                    .datePickerStyle(.compact)

                HStack(spacing: 8) {
                    Picker(strings.priority, selection: $row.priority) {
                        Text("urgent").tag("urgent")
                        Text("high").tag("high")
                        Text("medium").tag("medium")
                        Text("low").tag("low")
                    }
                    .pickerStyle(.menu)

                    TextField(strings.notes, text: $row.notes)
                        .textFieldStyle(.roundedBorder)
                }
            } else {
                Text(row.error ?? "")
                    .font(.caption)
                    .foregroundStyle(.red)
            }
        }
        .padding(8)
        .background(.black.opacity(0.10), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }
}

struct CompactTaskRow: View {
    let task: DeadlineTask
    let index: Int?
    var showsDivider = true
    let onUpdate: (DeadlineTask, String, Date, String) -> Void
    let onComplete: (DeadlineTask) -> Void
    let onRestore: (DeadlineTask) -> Void
    let onToggleCurrent: (DeadlineTask) -> Void
    let onUpdatePriority: (DeadlineTask, String) -> Void
    let onPostpone: (DeadlineTask, Int) -> Void
    let onPostponeDate: (DeadlineTask, Date) -> Void
    let onDelete: (DeadlineTask) -> Void
    let onControlInteractionChanged: ((Bool) -> Void)?

    @State private var isHovered = false
    @State private var editingField: TaskEditField?
    @State private var draftTitle = ""
    @State private var draftDueAt = Date()
    @State private var draftDueText = ""
    @State private var draftNotes = ""
    @State private var showCustomPostpone = false
    @State private var customPostponeDate = Date()
    @FocusState private var focusedField: TaskEditField?
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let strings = NativeStrings.current
        VStack(spacing: 7) {
            HStack(alignment: .top, spacing: 10) {
                indexBadge

                VStack(alignment: .leading, spacing: 6) {
                    if editingField == .title {
                        TextField(strings.title, text: $draftTitle)
                            .textFieldStyle(.roundedBorder)
                            .font(.system(size: 15, weight: .semibold))
                            .focused($focusedField, equals: .title)
                            .onSubmit(commitTitle)
                    } else {
                        Text(task.title)
                            .font(.system(size: 15, weight: .semibold))
                            .lineLimit(2)
                            .foregroundStyle(primaryTaskText)
                            .strikethrough(task.status == "completed")
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .contentShape(Rectangle())
                            .onTapGesture {
                            beginTitleEdit()
                            }
                    }

                    if editingField == .due {
                        HStack(spacing: 6) {
                            TextField("yyyy-MM-dd HH:mm", text: $draftDueText)
                                .textFieldStyle(.roundedBorder)
                                .font(.system(size: 12))
                                .focused($focusedField, equals: .due)
                                .onSubmit(commitDue)
                                .frame(minWidth: 156)
                            LiquidIconButton(systemImage: "checkmark", tint: .secondary) {
                                commitDue()
                            }
                            DatePickerPopoverButton(
                                date: Binding(
                                    get: { draftDueAt },
                                    set: { nextDate in
                                        draftDueAt = nextDate
                                        draftDueText = formattedEditableDue(nextDate)
                                    }
                                ),
                                onControlInteractionChanged: { active in
                                    if active {
                                        onControlInteractionChanged?(true)
                                    }
                                }
                            )
                        }
                    } else {
                        Text("\(strings.deadlineDuePrefix) \(formattedDue(task.dueAt))")
                            .font(.system(size: 12))
                            .foregroundStyle(secondaryTaskText)
                            .lineLimit(1)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .contentShape(Rectangle())
                            .onTapGesture {
                            beginDueEdit()
                            }
                    }

                    if editingField == .notes {
                        TextField(strings.notes, text: $draftNotes)
                            .textFieldStyle(.roundedBorder)
                            .font(.system(size: 12))
                            .focused($focusedField, equals: .notes)
                            .onSubmit(commitNotes)
                    } else {
                        Text(task.notes.isEmpty ? strings.notes : task.notes)
                            .font(.system(size: 12))
                            .foregroundStyle(task.notes.isEmpty ? tertiaryTaskText.opacity(0.65) : tertiaryTaskText)
                            .lineLimit(1)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .contentShape(Rectangle())
                            .onTapGesture {
                            beginNotesEdit()
                            }
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)

                VStack(alignment: .trailing, spacing: 7) {
                    Text(relativeDueText(task.dueAt, strings: strings))
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(dueColor(task.dueAt))
                        .lineLimit(1)
                    GlassTag(text: task.priority, color: priorityColor(task.priority))
                        .overlay {
                            Menu {
                                ForEach(["urgent", "high", "medium", "low"], id: \.self) { priority in
                                    Button {
                                        onUpdatePriority(task, priority)
                                    } label: {
                                        if priority == task.priority {
                                            Label(priority, systemImage: "checkmark")
                                        } else {
                                            Text(priority)
                                        }
                                    }
                                }
                            } label: {
                                Color.clear
                                    .contentShape(Capsule())
                            }
                            .menuStyle(.borderlessButton)
                            .menuIndicator(.hidden)
                        }
                        .disabled(task.status == "completed")
                    Text(statusLabel(task.status, strings: strings))
                        .font(.system(size: 12))
                        .foregroundStyle(secondaryTaskText)
                }
                .frame(width: 96, alignment: .trailing)
            }

            if isHovered || showCustomPostpone {
                HStack(spacing: 11) {
                    LiquidIconButton(
                        systemImage: task.isCurrent ? "star.fill" : "star",
                        tint: task.isCurrent ? .yellow : .secondary,
                        isDisabled: task.status == "completed"
                    ) {
                        onToggleCurrent(task)
                    }

                    LiquidIconButton(
                        systemImage: task.status == "completed" ? "arrow.uturn.backward" : "checkmark",
                        tint: task.status == "completed" ? .orange : .primary
                    ) {
                        if task.status == "completed" {
                            onRestore(task)
                        } else {
                            onComplete(task)
                        }
                    }

                    LiquidIconButton(systemImage: "trash", tint: .secondary) {
                        onDelete(task)
                    }

                    Spacer()

                    Menu {
                        Button(strings.plusOneDay) { onPostpone(task, 1) }
                        Button(strings.plusThreeDays) { onPostpone(task, 3) }
                        Button(strings.plusSevenDays) { onPostpone(task, 7) }
                        Divider()
                        Button(strings.customPostpone) {
                            customPostponeDate = Calendar.current.date(
                                byAdding: .day,
                                value: 1,
                                to: ISO8601DateFormatter.deadlinePanelDate(from: task.dueAt) ?? Date()
                            ) ?? Date()
                            showCustomPostpone = true
                            onControlInteractionChanged?(true)
                        }
                    } label: {
                        HStack(spacing: 6) {
                            Image(systemName: "calendar.badge.clock")
                            Text(strings.postpone)
                        }
                        .font(.caption.weight(.semibold))
                        .padding(.horizontal, 10)
                        .frame(height: 30)
                        .background {
                            Capsule()
                                .fill(.regularMaterial)
                                .overlay(Color.black.opacity(0.10).clipShape(Capsule()))
                                .overlay(Capsule().stroke(.white.opacity(0.18), lineWidth: 1))
                        }
                    }
                    .menuStyle(.borderlessButton)
                    .disabled(task.status == "completed")
                }
                .font(.caption)
                .padding(.leading, 32)
                .transition(.opacity.combined(with: .move(edge: .top)))
            }

            if showCustomPostpone {
                HStack(spacing: 8) {
                    DueInputControl(
                        label: strings.due,
                        date: $customPostponeDate,
                        font: .caption,
                        onControlInteractionChanged: { active in
                            if active {
                                onControlInteractionChanged?(true)
                            }
                        }
                    )
                    LiquidIconButton(systemImage: "checkmark", tint: .secondary) {
                        onPostponeDate(task, customPostponeDate)
                        closeCustomPostpone()
                    }
                    LiquidIconButton(systemImage: "xmark", tint: .secondary) {
                        closeCustomPostpone()
                    }
                }
                .padding(.leading, 32)
                .transition(.opacity.combined(with: .move(edge: .top)))
            }
        }
        .padding(.vertical, 13)
        .contentShape(Rectangle())
        .onHover { hovering in
            withAnimation(.spring(response: 0.22, dampingFraction: 0.8)) {
                isHovered = hovering
            }
        }
        .overlay(alignment: .bottom) {
            if showsDivider {
                Rectangle()
                    .fill(rowDividerColor)
                    .frame(height: 1)
            }
        }
        .onAppear(perform: syncDrafts)
        .onDisappear {
            if editingField != nil || showCustomPostpone {
                onControlInteractionChanged?(false)
            }
        }
        .onChange(of: task.id) { _, _ in syncDrafts() }
        .onChange(of: focusedField) { oldField, newField in
            guard newField == nil,
                  let editingField,
                  oldField == editingField
            else {
                return
            }
            switch editingField {
            case .title:
                commitTitle()
            case .due:
                commitDue()
            case .notes:
                commitNotes()
            }
        }
    }

    private var primaryTaskText: Color {
        colorScheme == .light ? Color.black.opacity(0.72) : Color.primary
    }

    private var secondaryTaskText: Color {
        colorScheme == .light ? Color.black.opacity(0.52) : Color.secondary
    }

    private var tertiaryTaskText: Color {
        colorScheme == .light ? Color.black.opacity(0.38) : Color.secondary.opacity(0.75)
    }

    private var rowDividerColor: Color {
        colorScheme == .light ? Color.gray.opacity(0.24) : Color.white.opacity(0.10)
    }

    private func syncDrafts() {
        draftTitle = task.title
        draftDueAt = ISO8601DateFormatter.deadlinePanelDate(from: task.dueAt) ?? Date()
        draftDueText = formattedEditableDue(draftDueAt)
        draftNotes = task.notes
    }

    private func beginTitleEdit() {
        syncDrafts()
        editingField = .title
        onControlInteractionChanged?(true)
        DispatchQueue.main.async {
            focusedField = .title
        }
    }

    private func beginDueEdit() {
        syncDrafts()
        editingField = .due
        onControlInteractionChanged?(true)
        DispatchQueue.main.async {
            focusedField = .due
        }
    }

    private func beginNotesEdit() {
        syncDrafts()
        editingField = .notes
        onControlInteractionChanged?(true)
        DispatchQueue.main.async {
            focusedField = .notes
        }
    }

    private func commitTitle() {
        let title = draftTitle.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !title.isEmpty else {
            finishEditing()
            return
        }
        finishEditing()
        onUpdate(task, title, ISO8601DateFormatter.deadlinePanelDate(from: task.dueAt) ?? draftDueAt, task.notes)
    }

    private func commitDue() {
        let nextDate = parseEditableDue(draftDueText) ?? draftDueAt
        draftDueAt = nextDate
        draftDueText = formattedEditableDue(nextDate)
        finishEditing()
        onUpdate(task, task.title, nextDate, task.notes)
    }

    private func commitNotes() {
        finishEditing()
        onUpdate(task, task.title, ISO8601DateFormatter.deadlinePanelDate(from: task.dueAt) ?? draftDueAt, draftNotes)
    }

    private func finishEditing() {
        editingField = nil
        focusedField = nil
        onControlInteractionChanged?(false)
    }

    private func closeCustomPostpone() {
        showCustomPostpone = false
        onControlInteractionChanged?(false)
    }

    @ViewBuilder
    private var indexBadge: some View {
        if let index {
            Text(String(index))
                .font(.caption.weight(.semibold))
                .frame(width: 22, height: 22)
                .background(.white.opacity(task.isCurrent ? 0.18 : 0.08), in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                .foregroundStyle(task.isCurrent ? .blue : .secondary)
        } else {
            Image(systemName: "checkmark.circle.fill")
                .font(.system(size: 18))
                .frame(width: 22, height: 22)
                .foregroundStyle(.orange)
        }
    }
}

private enum TaskEditField: Hashable {
    case title
    case due
    case notes
}

struct PanelSection<Content: View>: View {
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            content
        }
        .padding(.vertical, 13)
        .padding(.horizontal, 3)
        .overlay(alignment: .bottom) {
            Rectangle()
                .fill(.white.opacity(0.11))
                .frame(height: 1)
        }
    }
}

struct SummaryPill: View {
    let text: String
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        Text(text)
            .font(.caption2.weight(.medium))
            .lineLimit(1)
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background {
                Capsule()
                    .fill(.white.opacity(pillFillOpacity))
                    .overlay(Capsule().stroke(.white.opacity(pillBorderOpacity), lineWidth: 1))
                    .shadow(color: pillShadowColor, radius: 5, x: 1, y: 2)
            }
    }

    private var pillShadowColor: Color {
        Color.black.opacity(colorScheme == .dark ? 0.32 : 0.24)
    }

    private var pillFillOpacity: Double {
        colorScheme == .dark ? 0.10 : 0.12
    }

    private var pillBorderOpacity: Double {
        colorScheme == .dark ? 0.34 : 0.30
    }
}

struct NativeWindowBackground: View {
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: 14, style: .continuous)
        if #available(macOS 26.0, *) {
            shape
                .fill(tintColor)
                .glassEffect(.regular, in: .rect(cornerRadius: 14))
                .overlay(Color.black.opacity(0.06).clipShape(shape))
                .overlay(shape.stroke(borderColor, lineWidth: 1))
                .overlay(alignment: .top) {
                    topHighlight(shape: shape)
                }
                .ignoresSafeArea()
        } else {
            shape
                .fill(.thinMaterial)
                .overlay(stripOverlayColor.clipShape(shape))
                .overlay(shape.stroke(borderColor, lineWidth: 1))
                .overlay(alignment: .top) {
                    topHighlight(shape: shape)
                }
                .ignoresSafeArea()
        }
    }

    private func topHighlight(shape: RoundedRectangle) -> some View {
        shape
            .fill(
                LinearGradient(
                    colors: [
                        Color.white.opacity(colorScheme == .dark ? 0.06 : 0.15),
                        Color.clear
                    ],
                    startPoint: .top,
                    endPoint: .bottom
                )
            )
            .frame(height: 24)
            .clipShape(shape)
    }

    private var stripOverlayColor: Color {
        colorScheme == .dark ? Color.black.opacity(0.08) : Color.white.opacity(0.04)
    }

    private var borderColor: Color {
        Color.white.opacity(colorScheme == .dark ? 0.12 : 0.20)
    }

    private var tintColor: Color {
        colorScheme == .dark ? Color.black.opacity(0.15) : Color.white.opacity(0.20)
    }
}

private func priorityColor(_ priority: String) -> Color {
    switch priority {
    case "urgent":
        return .red
    case "high":
        return .orange
    case "medium":
        return .blue
    default:
        return .green
    }
}

private func dueColor(_ dueAt: String) -> Color {
    guard let date = ISO8601DateFormatter.deadlinePanelDate(from: dueAt) else {
        return .secondary
    }
    if date < Date() {
        return .red
    }
    if date.timeIntervalSinceNow < 24 * 60 * 60 {
        return .orange
    }
    return .secondary
}

private func relativeDueText(_ dueAt: String, strings: NativeStrings = .current) -> String {
    guard let date = ISO8601DateFormatter.deadlinePanelDate(from: dueAt) else {
        return strings.unknownTime
    }
    let diff = date.timeIntervalSinceNow
    let absDiff = abs(diff)
    let hour: TimeInterval = 60 * 60
    let day: TimeInterval = 24 * hour
    if diff < 0 {
        if absDiff < hour { return strings.overdue }
        if absDiff < day { return strings.overdueHours(Int(ceil(absDiff / hour))) }
        return strings.overdueDays(Int(ceil(absDiff / day)))
    }
    if diff < hour { return strings.withinHour }
    if diff < day { return strings.hours(Int(ceil(diff / hour))) }
    return strings.days(Int(ceil(diff / day)))
}

private func formattedDue(_ dueAt: String) -> String {
    guard let date = ISO8601DateFormatter.deadlinePanelDate(from: dueAt) else {
        return trimmedDueFallback(dueAt)
    }
    return formattedEditableDue(date)
}

private func formattedEditableDue(_ date: Date) -> String {
    let formatter = DateFormatter()
    formatter.locale = Locale(identifier: "en_US_POSIX")
    formatter.dateFormat = "yyyy-MM-dd HH:mm"
    return formatter.string(from: date)
}

private func parseEditableDue(_ value: String) -> Date? {
    let formatter = DateFormatter()
    formatter.locale = Locale(identifier: "en_US_POSIX")
    formatter.dateFormat = "yyyy-MM-dd HH:mm"
    return formatter.date(from: value.trimmingCharacters(in: .whitespacesAndNewlines))
}

private func trimmedDueFallback(_ dueAt: String) -> String {
    let trimmed = dueAt.trimmingCharacters(in: .whitespacesAndNewlines)
    if trimmed.count >= 16 {
        let normalized = trimmed.replacingOccurrences(of: "T", with: " ")
        return String(normalized.prefix(16))
    }
    return trimmed
}

private func statusLabel(_ status: String, strings: NativeStrings = .current) -> String {
    switch status {
    case "completed":
        return strings.statusCompleted
    case "postponed":
        return strings.statusPostponed
    default:
        return strings.statusActive
    }
}

private extension AnyTransition {
    static var liquidDisclosure: AnyTransition {
        .asymmetric(
            insertion: .opacity.combined(with: .scale(scale: 0.985, anchor: .top)),
            removal: .opacity
        )
    }
}
