import AppKit
import CryptoKit
import Foundation

@MainActor
final class NativeCloudSyncController: ObservableObject {
    @Published var syncURL = ""
    @Published var anonKey = ""
    @Published var syncCode = ""
    @Published var showAdvanced = true
    @Published var isPending = false
    @Published var message: String?
    @Published var isConfirmingCodeReset = false

    private enum DefaultsKey {
        static let syncURL = "sync_supabase_url"
        static let anonKey = "sync_supabase_anon_key"
        static let syncCode = "sync_code"
    }
    private enum Defaults {
        static let syncURL = "https://fuchvkdnmveelvwxtbjq.supabase.co"
        static let anonKey = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6ImZ1Y2h2a2RubXZlZWx2d3h0YmpxIiwicm9sZSI6ImFub24iLCJpYXQiOjE3ODIyODExMDgsImV4cCI6MjA5Nzg1NzEwOH0.GSTD79_2zSWpBhZFJon-kvOw_tXU9MErdx4QJKSuSx8"
    }

    private let viewModel: DeadlineViewModel

    init(viewModel: DeadlineViewModel) {
        self.viewModel = viewModel
        loadSettings()
    }

    func loadSettings() {
        let defaults = UserDefaults.standard
        syncURL = defaults.string(forKey: DefaultsKey.syncURL) ?? Defaults.syncURL
        anonKey = defaults.string(forKey: DefaultsKey.anonKey) ?? Defaults.anonKey
        syncCode = defaults.string(forKey: DefaultsKey.syncCode) ?? ""
        showAdvanced = syncURL.isEmpty || anonKey.isEmpty
    }

    func saveSettings() {
        let code = syncCode.trimmingCharacters(in: .whitespacesAndNewlines)
        let nextCode = code.isEmpty ? Self.generateSyncCode() : code
        syncCode = nextCode
        let defaults = UserDefaults.standard
        defaults.set(Self.normalizedSupabaseURL(syncURL), forKey: DefaultsKey.syncURL)
        defaults.set(anonKey.trimmingCharacters(in: .whitespacesAndNewlines), forKey: DefaultsKey.anonKey)
        defaults.set(nextCode, forKey: DefaultsKey.syncCode)
        message = NativeLanguage.resolved == .en ? "Sync config saved" : NativeLanguage.resolved == .ja ? "同期設定を保存しました" : "同步配置已保存"
    }

    func generateOrConfirmSyncCode() {
        if !isConfirmingCodeReset {
            isConfirmingCodeReset = true
            message = NativeLanguage.resolved == .en ? "Click confirm generate again to replace the current sync code" : NativeLanguage.resolved == .ja ? "もう一度クリックすると現在の同期コードを置き換えます" : "再次点击确认生成，将替换当前同步码"
            return
        }
        syncCode = Self.generateSyncCode()
        isConfirmingCodeReset = false
        message = NativeLanguage.resolved == .en ? "New sync code generated" : NativeLanguage.resolved == .ja ? "新しい同期コードを生成しました" : "已生成新的同步码"
    }

    func copySyncCode() {
        let code = syncCode.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !code.isEmpty else {
            return
        }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(code, forType: .string)
        message = NativeLanguage.resolved == .en ? "Sync code copied" : NativeLanguage.resolved == .ja ? "同期コードをコピーしました" : "同步码已复制"
    }

    func syncNow() {
        Task {
            await performSync()
        }
    }

    private func performSync() async {
        isPending = true
        message = NativeStrings.current.syncing
        do {
            saveSettings()
            let settings = try currentSettings()
            let codeHash = Self.sha256(settings.syncCode)
            let remoteTasks = try await pullTasks(settings: settings, codeHash: codeHash)
            let mergedTasks = merge(localTasks: viewModel.deadlines, remoteTasks: remoteTasks)
            try await upsertTasks(settings: settings, codeHash: codeHash, tasks: mergedTasks)
            viewModel.replaceTasksAfterCloudSync(mergedTasks)
            message = NativeLanguage.resolved == .en ? "Sync complete" : NativeLanguage.resolved == .ja ? "同期しました" : "同步完成"
        } catch {
            message = NativeLanguage.resolved == .en ? "Sync failed. Check the Supabase config and RLS table" : NativeLanguage.resolved == .ja ? "同期に失敗しました。Supabase 設定と RLS テーブルを確認してください" : "同步失败，请检查 Supabase 配置和 RLS 表"
        }
        isPending = false
    }

    private func currentSettings() throws -> SyncSettings {
        let url = Self.normalizedSupabaseURL(syncURL)
        let key = anonKey.trimmingCharacters(in: .whitespacesAndNewlines)
        let code = syncCode.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !url.isEmpty, !key.isEmpty, !code.isEmpty else {
            throw SyncError.missingConfiguration
        }
        return SyncSettings(url: url, anonKey: key, syncCode: code)
    }

    private func pullTasks(settings: SyncSettings, codeHash: String) async throws -> [DeadlineTask] {
        let request = try makeRequest(settings: settings, rpcName: "deadline_sync_pull", body: [
            "p_sync_code_hash": codeHash
        ])
        let (data, response) = try await URLSession.shared.data(for: request)
        try validate(response: response, data: data)
        return try JSONDecoder().decode([DeadlineTaskRow].self, from: data).map(\.task)
    }

    private func upsertTasks(settings: SyncSettings, codeHash: String, tasks: [DeadlineTask]) async throws {
        let rows = tasks.map(DeadlineTaskRow.init(task:))
        let body = SyncUpsertRequest(p_sync_code_hash: codeHash, p_tasks: rows)
        let data = try JSONEncoder().encode(body)
        var request = try makeRequest(settings: settings, rpcName: "deadline_sync_upsert", bodyData: data)
        request.httpBody = data
        let (responseData, response) = try await URLSession.shared.data(for: request)
        try validate(response: response, data: responseData)
    }

    private func makeRequest(settings: SyncSettings, rpcName: String, body: [String: String]) throws -> URLRequest {
        try makeRequest(settings: settings, rpcName: rpcName, bodyData: JSONEncoder().encode(body))
    }

    private func makeRequest(settings: SyncSettings, rpcName: String, bodyData: Data) throws -> URLRequest {
        guard let url = URL(string: "\(settings.url)/rest/v1/rpc/\(rpcName)") else {
            throw SyncError.invalidURL
        }
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue(settings.anonKey, forHTTPHeaderField: "apikey")
        request.setValue("Bearer \(settings.anonKey)", forHTTPHeaderField: "Authorization")
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = bodyData
        return request
    }

    private func validate(response: URLResponse, data: Data) throws {
        guard let httpResponse = response as? HTTPURLResponse,
              (200..<300).contains(httpResponse.statusCode)
        else {
            throw SyncError.requestFailed
        }
    }

    private func merge(localTasks: [DeadlineTask], remoteTasks: [DeadlineTask]) -> [DeadlineTask] {
        var merged: [String: DeadlineTask] = [:]
        for task in remoteTasks {
            merged[task.id] = task
        }
        for task in localTasks {
            if let remote = merged[task.id] {
                if parsedDate(task.updatedAt) >= parsedDate(remote.updatedAt) {
                    merged[task.id] = task
                }
            } else {
                merged[task.id] = task
            }
        }
        return merged.values.sorted { left, right in
            parsedDate(left.dueAt) < parsedDate(right.dueAt)
        }
    }

    private func parsedDate(_ value: String) -> Date {
        ISO8601DateFormatter.deadlinePanelDate(from: value) ?? .distantPast
    }

    private static func sha256(_ value: String) -> String {
        let digest = SHA256.hash(data: Data(value.trimmingCharacters(in: .whitespacesAndNewlines).utf8))
        return digest.map { String(format: "%02x", $0) }.joined()
    }

    private static func generateSyncCode() -> String {
        let body = [UUID().uuidString, UUID().uuidString, UUID().uuidString]
            .joined()
            .replacingOccurrences(of: "-", with: "")
            .lowercased()
        return "dp_sync_v1_\(body)"
    }

    private static func normalizedSupabaseURL(_ value: String) -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let url = URL(string: trimmed), let scheme = url.scheme, let host = url.host else {
            return trimmed
        }
        return "\(scheme)://\(host)"
    }
}

private struct SyncSettings {
    let url: String
    let anonKey: String
    let syncCode: String
}

private enum SyncError: Error {
    case invalidURL
    case missingConfiguration
    case requestFailed
}

private struct SyncUpsertRequest: Encodable {
    let p_sync_code_hash: String
    let p_tasks: [DeadlineTaskRow]
}

private struct DeadlineTaskRow: Codable {
    let task_id: String
    let title: String
    let due_at: String
    let priority: String
    let status: String
    let notes: String
    let source: String
    let is_current: Bool
    let created_at: String
    let updated_at: String
    let completed_at: String?

    init(task: DeadlineTask) {
        task_id = task.id
        title = task.title
        due_at = task.dueAt
        priority = task.priority
        status = task.status
        notes = task.notes
        source = task.source
        is_current = task.isCurrent
        created_at = task.createdAt
        updated_at = task.updatedAt
        completed_at = task.completedAt
    }

    var task: DeadlineTask {
        DeadlineTask(
            id: task_id,
            title: title,
            dueAt: due_at,
            priority: priority,
            status: status,
            notes: notes,
            source: source,
            isCurrent: is_current,
            createdAt: created_at,
            updatedAt: updated_at,
            completedAt: completed_at
        )
    }
}
