import Foundation
import Testing
@testable import DeadlinePanelNative

struct DeadlineViewModelCacheTests {
    @Test @MainActor
    func derivesCompletedAndCurrentTasksWhenDataLoads() {
        let tasks = [
            Self.task(id: "current-1", status: "active", isCurrent: true),
            Self.task(id: "completed-old", status: "completed", completedAt: "2026-07-10T10:00:00+09:00"),
            Self.task(id: "current-2", status: "active", isCurrent: true),
            Self.task(id: "current-3", status: "active", isCurrent: true),
            Self.task(id: "completed-new", status: "completed", completedAt: "2026-07-11T10:00:00+09:00")
        ]
        let viewModel = DeadlineViewModel(repository: TestDeadlineRepository(tasks: tasks))

        viewModel.load()

        #expect(viewModel.completedDeadlines.map { $0.id } == ["completed-new", "completed-old"])
        #expect(viewModel.currentDeadlines.map { $0.id } == ["current-1", "current-2"])
        #expect(viewModel.activeDeadlines.map(\.id) == ["current-1", "current-2", "current-3"])
        #expect(viewModel.focusDeadlines.count == 3)

        viewModel.focusLimit = 10

        #expect(viewModel.completedDeadlines.map { $0.id } == ["completed-new", "completed-old"])
        #expect(viewModel.currentDeadlines.map { $0.id } == ["current-1", "current-2"])
        #expect(viewModel.focusDeadlines.count == 3)
    }

    private static func task(
        id: String,
        status: String,
        isCurrent: Bool = false,
        completedAt: String? = nil
    ) -> DeadlineTask {
        DeadlineTask(
            id: id,
            title: id,
            dueAt: "2026-07-20T10:00:00+09:00",
            priority: "medium",
            status: status,
            notes: "",
            source: "test",
            isCurrent: isCurrent,
            createdAt: "2026-07-01T10:00:00+09:00",
            updatedAt: "2026-07-01T10:00:00+09:00",
            completedAt: completedAt
        )
    }
}

private struct TestDeadlineRepository: DeadlineRepository {
    let tasks: [DeadlineTask]

    func coreVersion() throws -> String { "test" }
    func listDeadlines() throws -> [DeadlineTask] { tasks }
    func createDeadline(_ input: NewDeadlineInput) throws -> DeadlineTask { throw TestError.unused }
    func updateDeadline(id: String, fields: UpdateTaskInput) throws -> DeadlineTask { throw TestError.unused }
    func completeDeadline(id: String) throws -> DeadlineTask { throw TestError.unused }
    func restoreDeadline(id: String) throws -> DeadlineTask { throw TestError.unused }
    func toggleCurrentDeadline(id: String) throws -> DeadlineTask { throw TestError.unused }
    func parseQuickAdd(_ text: String) throws -> NewDeadlineInput { throw TestError.unused }
    func importDeadlines(_ tasks: [DeadlineTask]) throws -> Int { throw TestError.unused }
    func deleteDeadline(id: String) -> Bool { false }

    private enum TestError: Error {
        case unused
    }
}
