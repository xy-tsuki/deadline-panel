import Foundation

protocol DeadlineRepository {
    func coreVersion() throws -> String
    func listDeadlines() throws -> [DeadlineTask]
    func createDeadline(_ input: NewDeadlineInput) throws -> DeadlineTask
    func updateDeadline(id: String, fields: UpdateTaskInput) throws -> DeadlineTask
    func completeDeadline(id: String) throws -> DeadlineTask
    func restoreDeadline(id: String) throws -> DeadlineTask
    func toggleCurrentDeadline(id: String) throws -> DeadlineTask
    func parseQuickAdd(_ text: String) throws -> NewDeadlineInput
    func importDeadlines(_ tasks: [DeadlineTask]) throws -> Int
    func deleteDeadline(id: String) -> Bool
}

final class RustDeadlineRepository: DeadlineRepository {
    private let client: RustCoreClient

    init(client: RustCoreClient = RustCoreClient()) {
        self.client = client
    }

    func coreVersion() throws -> String {
        try client.version()
    }

    func listDeadlines() throws -> [DeadlineTask] {
        try client.listDeadlines()
    }

    func createDeadline(_ input: NewDeadlineInput) throws -> DeadlineTask {
        try client.createDeadline(input)
    }

    func updateDeadline(id: String, fields: UpdateTaskInput) throws -> DeadlineTask {
        try client.updateDeadline(id: id, fields: fields)
    }

    func completeDeadline(id: String) throws -> DeadlineTask {
        try client.completeDeadline(id: id)
    }

    func restoreDeadline(id: String) throws -> DeadlineTask {
        try client.restoreDeadline(id: id)
    }

    func toggleCurrentDeadline(id: String) throws -> DeadlineTask {
        try client.toggleCurrentDeadline(id: id)
    }

    func parseQuickAdd(_ text: String) throws -> NewDeadlineInput {
        try client.parseQuickAdd(text)
    }

    func importDeadlines(_ tasks: [DeadlineTask]) throws -> Int {
        try client.importDeadlines(tasks)
    }

    func deleteDeadline(id: String) -> Bool {
        client.deleteDeadline(id: id)
    }
}
