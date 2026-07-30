import XCTest
@testable import DeadlinePanelNative

final class CloudSyncReconcilerTests: XCTestCase {
    func testNewerTaskWinsAcrossDevices() {
        let local = task(id: "shared", title: "Local", updatedAt: "2026-07-30T10:00:00Z")
        let remote = task(id: "shared", title: "Remote", updatedAt: "2026-07-30T11:00:00Z")

        let merged = CloudSyncReconciler.merge(
            localTasks: [local],
            remoteTasks: [remote],
            deletedIDs: []
        )

        XCTAssertEqual(merged.map(\.title), ["Remote"])
    }

    func testRemoteTombstoneRemovesStaleLocalTask() {
        let local = task(id: "deleted", title: "Offline copy", updatedAt: "2026-07-30T10:00:00Z")

        let merged = CloudSyncReconciler.merge(
            localTasks: [local],
            remoteTasks: [],
            deletedIDs: ["deleted"]
        )

        XCTAssertTrue(merged.isEmpty)
    }

    func testPendingLocalDeleteSuppressesRemoteTask() {
        let remote = task(id: "pending", title: "Remote copy", updatedAt: "2026-07-30T11:00:00Z")

        let merged = CloudSyncReconciler.merge(
            localTasks: [],
            remoteTasks: [remote],
            deletedIDs: ["pending"]
        )

        XCTAssertTrue(merged.isEmpty)
    }

    private func task(id: String, title: String, updatedAt: String) -> DeadlineTask {
        DeadlineTask(
            id: id,
            title: title,
            dueAt: "2026-08-01T12:00:00Z",
            priority: "medium",
            status: "active",
            notes: "",
            source: "manual",
            isCurrent: false,
            createdAt: "2026-07-30T09:00:00Z",
            updatedAt: updatedAt,
            completedAt: nil
        )
    }
}
