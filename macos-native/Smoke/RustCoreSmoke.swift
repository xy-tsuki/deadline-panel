import Foundation

@main
struct RustCoreSmoke {
    static func main() throws {
        let client = RustCoreClient()
        let version = try client.version()
        let before = try client.listDeadlines()
        let created = try client.createDeadline(
            NewDeadlineInput(
                title: "Swift smoke deadline",
                dueAt: "2026-07-10T23:59:00Z",
                priority: "high",
                notes: "Created by macos-native smoke test",
                source: "manual"
            )
        )
        let after = try client.listDeadlines()
        _ = client.deleteDeadline(id: created.id)

        print("deadline-core version: \(version)")
        print("deadlines before: \(before.count)")
        print("created deadline: \(created.title) [\(created.priority)]")
        print("deadlines after create: \(after.count)")
    }
}
