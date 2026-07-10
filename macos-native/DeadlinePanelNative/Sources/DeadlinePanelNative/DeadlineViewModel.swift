import Foundation

@MainActor
final class DeadlineViewModel: ObservableObject {
    @Published private(set) var coreVersion = "0.6.1"
    @Published private(set) var deadlines: [DeadlineTask] = []
    @Published private(set) var errorMessage: String?
    @Published private(set) var commandMessage: String?
    @Published var focusLimit = UserDefaults.standard.integer(forKey: "focus_limit") == 0
        ? 3
        : UserDefaults.standard.integer(forKey: "focus_limit") {
        didSet {
            UserDefaults.standard.set(focusLimit, forKey: "focus_limit")
        }
    }
    @Published var quickAddText = ""

    private let repository: DeadlineRepository

    init(repository: DeadlineRepository = RustDeadlineRepository()) {
        self.repository = repository
    }

    func load() {
        do {
            coreVersion = try repository.coreVersion()
            deadlines = try repository.listDeadlines()
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

    var completedDeadlines: [DeadlineTask] {
        deadlines.filter { $0.status == "completed" }
    }

    var currentDeadlines: [DeadlineTask] {
        Array(
            deadlines
                .filter { $0.status != "completed" && $0.isCurrent }
                .prefix(2)
        )
    }

    func addQuickDeadline() {
        let text = quickAddText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else {
            errorMessage = NativeLanguage.resolved == .en ? "Enter a deadline first" : NativeLanguage.resolved == .ja ? "Deadline を入力してください" : "请输入 Deadline"
            return
        }

        do {
            let input = try repository.parseQuickAdd(text)
            _ = try repository.createDeadline(input)
            quickAddText = ""
            deadlines = try repository.listDeadlines()
            commandMessage = NativeLanguage.resolved == .en ? "Deadline added" : NativeLanguage.resolved == .ja ? "Deadline を追加しました" : "已添加 Deadline"
            errorMessage = nil
        } catch {
            errorMessage = String(describing: error)
        }
    }

    func createManual(title: String, dueAt: Date, priority: String, notes: String) {
        do {
            _ = try repository.createDeadline(
                NewDeadlineInput(
                    title: title,
                    dueAt: ISO8601DateFormatter.deadlinePanelString(from: dueAt),
                    priority: priority,
                    notes: notes,
                    source: "manual"
                )
            )
            deadlines = try repository.listDeadlines()
            commandMessage = NativeLanguage.resolved == .en ? "Deadline added" : NativeLanguage.resolved == .ja ? "Deadline を追加しました" : "已添加 Deadline"
            errorMessage = nil
        } catch {
            errorMessage = String(describing: error)
        }
    }

    func createDeadline(_ input: NewDeadlineInput) {
        do {
            _ = try repository.createDeadline(input)
            deadlines = try repository.listDeadlines()
            commandMessage = NativeLanguage.resolved == .en ? "Deadline added" : NativeLanguage.resolved == .ja ? "Deadline を追加しました" : "已添加 Deadline"
            errorMessage = nil
        } catch {
            errorMessage = String(describing: error)
        }
    }

    func importDeadlineInputs(_ inputs: [NewDeadlineInput]) {
        guard !inputs.isEmpty else {
            errorMessage = NativeStrings.current.nothingToImport
            return
        }

        do {
            for input in inputs {
                _ = try repository.createDeadline(input)
            }
            deadlines = try repository.listDeadlines()
            commandMessage = NativeLanguage.resolved == .en ? "Imported \(inputs.count) items" : NativeLanguage.resolved == .ja ? "\(inputs.count) 件をインポートしました" : "已导入 \(inputs.count) 条事项"
            errorMessage = nil
        } catch {
            errorMessage = String(describing: error)
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
                _ = try repository.updateDeadline(
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
                deadlines = try repository.listDeadlines()
                commandMessage = NativeLanguage.resolved == .en ? "Updated" : NativeLanguage.resolved == .ja ? "更新しました" : "已更新"
                errorMessage = nil
            } catch {
                errorMessage = String(describing: error)
            }
        }
    }

    func replaceTasksAfterCloudSync(_ tasks: [DeadlineTask]) {
        do {
            let currentIDs = Set(deadlines.map(\.id))
            let nextIDs = Set(tasks.map(\.id))
            for id in currentIDs.subtracting(nextIDs) {
                _ = repository.deleteDeadline(id: id)
            }
            _ = try repository.importDeadlines(tasks)
            deadlines = try repository.listDeadlines()
            commandMessage = NativeLanguage.resolved == .en ? "Synced \(deadlines.count) items" : NativeLanguage.resolved == .ja ? "\(deadlines.count) 件を同期しました" : "已同步 \(deadlines.count) 条事项"
            errorMessage = nil
        } catch {
            errorMessage = String(describing: error)
        }
    }

    func updateManual(task: DeadlineTask, title: String, dueAt: Date, priority: String, notes: String) {
        do {
            _ = try repository.updateDeadline(
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
            deadlines = try repository.listDeadlines()
            commandMessage = NativeLanguage.resolved == .en ? "Updated" : NativeLanguage.resolved == .ja ? "更新しました" : "已更新"
            errorMessage = nil
        } catch {
            errorMessage = String(describing: error)
        }
    }

    func complete(_ task: DeadlineTask) {
        do {
            _ = try repository.completeDeadline(id: task.id)
            deadlines = try repository.listDeadlines()
            commandMessage = NativeLanguage.resolved == .en ? "Completed" : NativeLanguage.resolved == .ja ? "完了しました" : "已完成"
            errorMessage = nil
        } catch {
            errorMessage = String(describing: error)
        }
    }

    func restore(_ task: DeadlineTask) {
        do {
            _ = try repository.restoreDeadline(id: task.id)
            deadlines = try repository.listDeadlines()
            commandMessage = NativeLanguage.resolved == .en ? "Restored" : NativeLanguage.resolved == .ja ? "復元しました" : "已恢复"
            errorMessage = nil
        } catch {
            errorMessage = String(describing: error)
        }
    }

    func toggleCurrent(_ task: DeadlineTask) {
        do {
            _ = try repository.toggleCurrentDeadline(id: task.id)
            deadlines = try repository.listDeadlines()
            commandMessage = nil
            errorMessage = nil
        } catch {
            errorMessage = String(describing: error)
        }
    }

    func postpone(_ task: DeadlineTask, days: Int) {
        do {
            guard let date = ISO8601DateFormatter.deadlinePanelDate(from: task.dueAt),
                  let nextDate = Calendar.current.date(byAdding: .day, value: days, to: date)
            else {
                errorMessage = NativeStrings.current.unknownTime
                return
            }
            _ = try repository.updateDeadline(
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
            deadlines = try repository.listDeadlines()
            commandMessage = NativeLanguage.resolved == .en ? "Postponed" : NativeLanguage.resolved == .ja ? "延期しました" : "已延期"
            errorMessage = nil
        } catch {
            errorMessage = String(describing: error)
        }
    }

    func delete(_ task: DeadlineTask) {
        _ = repository.deleteDeadline(id: task.id)
        do {
            deadlines = try repository.listDeadlines()
            commandMessage = NativeLanguage.resolved == .en ? "Deleted" : NativeLanguage.resolved == .ja ? "削除しました" : "已删除"
            errorMessage = nil
        } catch {
            errorMessage = String(describing: error)
        }
    }

    func importTasks(from url: URL) {
        do {
            let data = try Data(contentsOf: url)
            let tasks = try JSONDecoder().decode([DeadlineTask].self, from: data)
            let count = try repository.importDeadlines(tasks)
            deadlines = try repository.listDeadlines()
            commandMessage = NativeLanguage.resolved == .en ? "Imported \(count) items" : NativeLanguage.resolved == .ja ? "\(count) 件をインポートしました" : "已导入 \(count) 条事项"
            errorMessage = nil
        } catch {
            errorMessage = String(describing: error)
        }
    }

    func exportTasks(to url: URL) {
        do {
            let data = try Self.exportEncoder().encode(deadlines)
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

    private func findTask(idOrTitle: String) -> DeadlineTask? {
        let needle = idOrTitle.lowercased()
        return deadlines.first { task in
            task.id == idOrTitle || task.title.lowercased() == needle
        }
    }
}
