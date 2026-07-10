import AppKit

@MainActor
final class FileImportExportController {
    private let viewModel: DeadlineViewModel

    init(viewModel: DeadlineViewModel) {
        self.viewModel = viewModel
    }

    func importJSON() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.json]
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        panel.canChooseFiles = true
        guard panel.runModal() == .OK,
              let url = panel.url
        else {
            return
        }
        viewModel.importTasks(from: url)
    }

    func exportJSON() {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.json]
        panel.nameFieldStringValue = "deadline-panel-\(Self.dateStamp()).json"
        guard panel.runModal() == .OK,
              let url = panel.url
        else {
            return
        }
        viewModel.exportTasks(to: url)
    }

    private static func dateStamp() -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"
        return formatter.string(from: Date())
    }
}
