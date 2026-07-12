import Foundation

@MainActor
final class DeadlineFocusLimitState: ObservableObject {
    @Published var value: Int {
        didSet {
            guard value != oldValue else {
                return
            }
            defaults.set(value, forKey: "focus_limit")
        }
    }

    private let defaults: UserDefaults

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        let stored = defaults.integer(forKey: "focus_limit")
        value = [3, 5, 10].contains(stored) ? stored : 3
    }
}

@MainActor
final class DeadlineViewModel: ObservableObject {
    @Published private(set) var coreVersion = "0.6.2"
    @Published private(set) var deadlines: [DeadlineTask] = []
    @Published private(set) var errorMessage: String?
    @Published private(set) var commandMessage: String?
    private let repository: DeadlineRepository
    let focusLimitState: DeadlineFocusLimitState
    private(set) var completedDeadlines: [DeadlineTask] = []
    private(set) var currentDeadlines: [DeadlineTask] = []
    var onTasksUpserted: (([DeadlineTask]) -> Void)?
    var onTaskDeleted: ((String) -> Void)?

    init(repository: DeadlineRepository = RustDeadlineRepository()) {
        self.repository = repository
        focusLimitState = DeadlineFocusLimitState()
    }

    var focusLimit: Int {
        get { focusLimitState.value }
        set { focusLimitState.value = newValue }
    }

    func load() {
        do {
            coreVersion = try repository.coreVersion()
            applyDeadlines(try repository.listDeadlines())
            errorMessage = nil
        } catch {
            errorMessage = String(describing: error)
        }
    }

    var focusDeadlines: [DeadlineTask] {
        Array(
            deadlines
                .filter { $0.status != "completed" }
                .prefix(focusLimit)
        )
    }

    func parseQuickDeadline(_ rawText: String) -> NewDeadlineInput? {
        let text = rawText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else {
            errorMessage = nil
            return nil
        }

        do {
            let input = try repository.parseQuickAdd(text)
            errorMessage = nil
            return input
        } catch {
            errorMessage = nil
            return nil
        }
    }

    @discardableResult
    func createManual(title: String, dueAt: Date, priority: String, notes: String) -> Bool {
        do {
            let task = try repository.createDeadline(
                NewDeadlineInput(
                    title: title,
                    dueAt: ISO8601DateFormatter.deadlinePanelString(from: dueAt),
                    priority: priority,
                    notes: notes,
                    source: "manual"
                )
            )
            applyDeadlines(try repository.listDeadlines())
            commandMessage = NativeLanguage.resolved == .en ? "Deadline added" : NativeLanguage.resolved == .ja ? "Deadline を追加しました" : "已添加 Deadline"
            errorMessage = nil
            onTasksUpserted?([task])
            return true
        } catch {
            errorMessage = String(describing: error)
            return false
        }
    }

    @discardableResult
    func createDeadline(_ input: NewDeadlineInput) -> Bool {
        do {
            let task = try repository.createDeadline(input)
            applyDeadlines(try repository.listDeadlines())
            commandMessage = NativeLanguage.resolved == .en ? "Deadline added" : NativeLanguage.resolved == .ja ? "Deadline を追加しました" : "已添加 Deadline"
            errorMessage = nil
            onTasksUpserted?([task])
            return true
        } catch {
            errorMessage = String(describing: error)
            return false
        }
    }

    @discardableResult
    func importDeadlineInputs(_ inputs: [NewDeadlineInput]) -> Bool {
        guard !inputs.isEmpty else {
            errorMessage = NativeStrings.current.nothingToImport
            return false
        }

        do {
            let created = try inputs.map { try repository.createDeadline($0) }
            applyDeadlines(try repository.listDeadlines())
            commandMessage = NativeLanguage.resolved == .en ? "Imported \(inputs.count) items" : NativeLanguage.resolved == .ja ? "\(inputs.count) 件をインポートしました" : "已导入 \(inputs.count) 条事项"
            errorMessage = nil
            onTasksUpserted?(created)
            return true
        } catch {
            errorMessage = String(describing: error)
            return false
        }
    }

    func runCommand(_ rawInput: String) {
        let result = NativeCommandParser.parse(rawInput)
        guard result.isValid, let command = result.command else {
            commandMessage = result.error ?? NativeStrings.current.unknownCommand
            return
        }

        switch command {
        case .add(let input):
            createDeadline(input)
        case .complete(let idOrTitle):
            guard let target = findTask(idOrTitle: idOrTitle) else {
                commandMessage = NativeStrings.current.commandTargetNotFound
                return
            }
            complete(target)
        case .delete(let idOrTitle):
            guard let target = findTask(idOrTitle: idOrTitle) else {
                commandMessage = NativeStrings.current.commandTargetNotFound
                return
            }
            delete(target)
        case .update(let idOrTitle, let fields):
            guard let target = findTask(idOrTitle: idOrTitle) else {
                commandMessage = NativeStrings.current.commandTargetNotFound
                return
            }
            do {
                let updated = try repository.updateDeadline(
                    id: target.id,
                    fields: UpdateTaskInput(
                        title: fields.title,
                        dueAt: fields.dueAt,
                        priority: fields.priority,
                        notes: fields.notes,
                        status: nil,
                        source: nil,
                        isCurrent: nil,
                        completedAt: nil
                    )
                )
                applyDeadlines(try repository.listDeadlines())
                commandMessage = NativeLanguage.resolved == .en ? "Updated" : NativeLanguage.resolved == .ja ? "更新しました" : "已更新"
                errorMessage = nil
                onTasksUpserted?([updated])
            } catch {
                errorMessage = String(describing: error)
            }
        }
    }

    func replaceTasksAfterCloudSync(_ tasks: [DeadlineTask], silent: Bool = false) {
        do {
            let currentIDs = Set(deadlines.map(\.id))
            let nextIDs = Set(tasks.map(\.id))
            for id in currentIDs.subtracting(nextIDs) {
                _ = repository.deleteDeadline(id: id)
            }
            _ = try repository.importDeadlines(tasks)
            applyDeadlines(try repository.listDeadlines())
            if !silent {
                commandMessage = NativeLanguage.resolved == .en ? "Synced \(deadlines.count) items" : NativeLanguage.resolved == .ja ? "\(deadlines.count) 件を同期しました" : "已同步 \(deadlines.count) 条事项"
            }
            errorMessage = nil
        } catch {
            errorMessage = String(describing: error)
        }
    }

    func updateManual(task: DeadlineTask, title: String, dueAt: Date, priority: String, notes: String) {
        do {
            let updated = try repository.updateDeadline(
                id: task.id,
                fields: UpdateTaskInput(
                    title: title,
                    dueAt: ISO8601DateFormatter.deadlinePanelString(from: dueAt),
                    priority: priority,
                    notes: notes,
                    status: nil,
                    source: nil,
                    isCurrent: nil,
                    completedAt: nil
                )
            )
            applyDeadlines(try repository.listDeadlines())
            commandMessage = NativeLanguage.resolved == .en ? "Updated" : NativeLanguage.resolved == .ja ? "更新しました" : "已更新"
            errorMessage = nil
            onTasksUpserted?([updated])
        } catch {
            errorMessage = String(describing: error)
        }
    }

    func complete(_ task: DeadlineTask) {
        do {
            let updated = try repository.completeDeadline(id: task.id)
            applyDeadlines(try repository.listDeadlines())
            commandMessage = NativeLanguage.resolved == .en ? "Completed" : NativeLanguage.resolved == .ja ? "完了しました" : "已完成"
            errorMessage = nil
            onTasksUpserted?([updated])
        } catch {
            errorMessage = String(describing: error)
        }
    }

    func restore(_ task: DeadlineTask) {
        do {
            let updated = try repository.restoreDeadline(id: task.id)
            applyDeadlines(try repository.listDeadlines())
            commandMessage = NativeLanguage.resolved == .en ? "Restored" : NativeLanguage.resolved == .ja ? "復元しました" : "已恢复"
            errorMessage = nil
            onTasksUpserted?([updated])
        } catch {
            errorMessage = String(describing: error)
        }
    }

    func toggleCurrent(_ task: DeadlineTask) {
        do {
            let updated = try repository.toggleCurrentDeadline(id: task.id)
            applyDeadlines(try repository.listDeadlines())
            commandMessage = nil
            errorMessage = nil
            onTasksUpserted?([updated])
        } catch {
            errorMessage = String(describing: error)
        }
    }

    func postpone(_ task: DeadlineTask, days: Int) {
        guard let date = ISO8601DateFormatter.deadlinePanelDate(from: task.dueAt),
              let nextDate = Calendar.current.date(byAdding: .day, value: days, to: date)
        else {
            errorMessage = NativeStrings.current.unknownTime
            return
        }
        postpone(task, to: nextDate)
    }

    func postpone(_ task: DeadlineTask, to nextDate: Date) {
        do {
            let updated = try repository.updateDeadline(
                id: task.id,
                fields: UpdateTaskInput(
                    title: nil,
                    dueAt: ISO8601DateFormatter.deadlinePanelString(from: nextDate),
                    priority: nil,
                    notes: nil,
                    status: "postponed",
                    source: nil,
                    isCurrent: nil,
                    completedAt: nil
                )
            )
            applyDeadlines(try repository.listDeadlines())
            commandMessage = NativeLanguage.resolved == .en ? "Postponed" : NativeLanguage.resolved == .ja ? "延期しました" : "已延期"
            errorMessage = nil
            onTasksUpserted?([updated])
        } catch {
            errorMessage = String(describing: error)
        }
    }

    func updatePriority(_ task: DeadlineTask, priority: String) {
        guard priority != task.priority else {
            return
        }
        do {
            let updated = try repository.updateDeadline(
                id: task.id,
                fields: UpdateTaskInput(
                    title: nil,
                    dueAt: nil,
                    priority: priority,
                    notes: nil,
                    status: nil,
                    source: nil,
                    isCurrent: nil,
                    completedAt: nil
                )
            )
            applyDeadlines(try repository.listDeadlines())
            commandMessage = NativeLanguage.resolved == .en ? "Priority updated" : NativeLanguage.resolved == .ja ? "優先度を更新しました" : "已更新优先级"
            errorMessage = nil
            onTasksUpserted?([updated])
        } catch {
            errorMessage = String(describing: error)
        }
    }

    func delete(_ task: DeadlineTask) {
        let deleted = repository.deleteDeadline(id: task.id)
        do {
            applyDeadlines(try repository.listDeadlines())
            commandMessage = NativeLanguage.resolved == .en ? "Deleted" : NativeLanguage.resolved == .ja ? "削除しました" : "已删除"
            errorMessage = nil
            if deleted {
                onTaskDeleted?(task.id)
            }
        } catch {
            errorMessage = String(describing: error)
        }
    }

    func importTasks(from url: URL) {
        do {
            let data = try Data(contentsOf: url)
            let decoder = JSONDecoder()
            let tasks: [DeadlineTask]
            if let envelope = try? decoder.decode(DeadlineExportEnvelope.self, from: data) {
                tasks = envelope.tasks
            } else {
                tasks = try decoder.decode([DeadlineTask].self, from: data)
            }
            let count = try repository.importDeadlines(tasks)
            applyDeadlines(try repository.listDeadlines())
            commandMessage = NativeLanguage.resolved == .en ? "Imported \(count) items" : NativeLanguage.resolved == .ja ? "\(count) 件をインポートしました" : "已导入 \(count) 条事项"
            errorMessage = nil
            onTasksUpserted?(tasks)
        } catch {
            errorMessage = String(describing: error)
        }
    }

    func exportTasks(to url: URL) {
        do {
            let envelope = DeadlineExportEnvelope(
                version: 1,
                exportedAt: ISO8601DateFormatter.deadlinePanelString(from: Date()),
                tasks: deadlines
            )
            let data = try Self.exportEncoder().encode(envelope)
            try data.write(to: url, options: .atomic)
            commandMessage = NativeLanguage.resolved == .en ? "Exported \(deadlines.count) items" : NativeLanguage.resolved == .ja ? "\(deadlines.count) 件をエクスポートしました" : "已导出 \(deadlines.count) 条事项"
            errorMessage = nil
        } catch {
            errorMessage = String(describing: error)
        }
    }
    private static func exportEncoder() -> JSONEncoder {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        return encoder
    }

    private func applyDeadlines(_ nextDeadlines: [DeadlineTask]) {
        completedDeadlines = nextDeadlines
            .filter { $0.status == "completed" }
            .sorted { left, right in
                let leftDate = ISO8601DateFormatter.deadlinePanelDate(
                    from: left.completedAt ?? left.updatedAt
                ) ?? .distantPast
                let rightDate = ISO8601DateFormatter.deadlinePanelDate(
                    from: right.completedAt ?? right.updatedAt
                ) ?? .distantPast
                return leftDate > rightDate
            }
        currentDeadlines = Array(
            nextDeadlines
                .filter { $0.status != "completed" && $0.isCurrent }
                .prefix(2)
        )
        deadlines = nextDeadlines
    }

    private func findTask(idOrTitle: String) -> DeadlineTask? {
        let needle = idOrTitle.lowercased()
        return deadlines.first { task in
            task.id == idOrTitle || task.title.lowercased() == needle
        }
    }
}
