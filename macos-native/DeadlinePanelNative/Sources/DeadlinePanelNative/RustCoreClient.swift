import Foundation

@_silgen_name("deadline_core_version_json")
private func deadline_core_version_json() -> UnsafeMutablePointer<CChar>?

@_silgen_name("deadline_initialize_json")
private func deadline_initialize_json(_ databasePath: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>?

@_silgen_name("deadline_migrate_legacy_database_json")
private func deadline_migrate_legacy_database_json(_ legacyDatabasePath: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>?

@_silgen_name("deadline_read_legacy_settings_json")
private func deadline_read_legacy_settings_json(_ legacyDatabasePath: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>?

@_silgen_name("deadline_import_tasks_json")
private func deadline_import_tasks_json(_ input: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>?

@_silgen_name("deadline_list_json")
private func deadline_list_json() -> UnsafeMutablePointer<CChar>?

@_silgen_name("deadline_create_json")
private func deadline_create_json(_ input: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>?

@_silgen_name("deadline_update_json")
private func deadline_update_json(_ input: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>?

@_silgen_name("deadline_delete")
private func deadline_delete(_ id: UnsafePointer<CChar>) -> Bool

@_silgen_name("deadline_complete_json")
private func deadline_complete_json(_ id: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>?

@_silgen_name("deadline_restore_json")
private func deadline_restore_json(_ id: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>?

@_silgen_name("deadline_toggle_current_json")
private func deadline_toggle_current_json(_ id: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>?

@_silgen_name("deadline_parse_quick_add_json")
private func deadline_parse_quick_add_json(_ input: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>?

@_silgen_name("deadline_free_string")
private func deadline_free_string(_ ptr: UnsafeMutablePointer<CChar>?)

final class RustCoreClient {
    private let decoder = JSONDecoder()
    private let encoder = JSONEncoder()
    private var didInitialize = false
    private var didAttemptMigration = false

    func version() throws -> String {
        let response: RustCoreEnvelope<RustCoreVersion> = try decode(deadline_core_version_json())
        return try unwrap(response).version
    }

    func initialize(databasePath: String) throws {
        let response: RustCoreEnvelope<[String: String]> = try databasePath.withCString { pointer in
            try decode(deadline_initialize_json(pointer))
        }
        _ = try unwrap(response)
        didInitialize = true
        try migrateLegacyDatabaseIfNeeded(nativeDatabasePath: databasePath)
    }

    func listDeadlines() throws -> [DeadlineTask] {
        try ensureInitialized()
        let response: RustCoreEnvelope<[DeadlineTask]> = try decode(deadline_list_json())
        return try unwrap(response)
    }

    func createDeadline(_ input: NewDeadlineInput) throws -> DeadlineTask {
        try ensureInitialized()
        let data = try encoder.encode(input)
        let json = String(decoding: data, as: UTF8.self)
        let response: RustCoreEnvelope<DeadlineTask> = try json.withCString { pointer in
            try decode(deadline_create_json(pointer))
        }
        return try unwrap(response)
    }

    func updateDeadline(id: String, fields: UpdateTaskInput) throws -> DeadlineTask {
        try ensureInitialized()
        let data = try encoder.encode(UpdateDeadlineRequest(id: id, fields: fields))
        let json = String(decoding: data, as: UTF8.self)
        let response: RustCoreEnvelope<DeadlineTask> = try json.withCString { pointer in
            try decode(deadline_update_json(pointer))
        }
        return try unwrap(response)
    }

    func completeDeadline(id: String) throws -> DeadlineTask {
        try ensureInitialized()
        let response: RustCoreEnvelope<DeadlineTask> = try id.withCString { pointer in
            try decode(deadline_complete_json(pointer))
        }
        return try unwrap(response)
    }

    func restoreDeadline(id: String) throws -> DeadlineTask {
        try ensureInitialized()
        let response: RustCoreEnvelope<DeadlineTask> = try id.withCString { pointer in
            try decode(deadline_restore_json(pointer))
        }
        return try unwrap(response)
    }

    func toggleCurrentDeadline(id: String) throws -> DeadlineTask {
        try ensureInitialized()
        let response: RustCoreEnvelope<DeadlineTask> = try id.withCString { pointer in
            try decode(deadline_toggle_current_json(pointer))
        }
        return try unwrap(response)
    }

    func parseQuickAdd(_ text: String) throws -> NewDeadlineInput {
        try ensureInitialized()
        let response: RustCoreEnvelope<NewDeadlineInput> = try text.withCString { pointer in
            try decode(deadline_parse_quick_add_json(pointer))
        }
        return try unwrap(response)
    }

    func importDeadlines(_ tasks: [DeadlineTask]) throws -> Int {
        try ensureInitialized()
        let data = try encoder.encode(tasks)
        let json = String(decoding: data, as: UTF8.self)
        let response: RustCoreEnvelope<ImportResult> = try json.withCString { pointer in
            try decode(deadline_import_tasks_json(pointer))
        }
        return try unwrap(response).imported
    }

    @discardableResult
    func deleteDeadline(id: String) -> Bool {
        try? ensureInitialized()
        return id.withCString { pointer in
            deadline_delete(pointer)
        }
    }

    private func decode<T: Decodable>(_ ptr: UnsafeMutablePointer<CChar>?) throws -> T {
        guard let ptr else {
            throw RustCoreClientError.nullPointer
        }
        defer {
            deadline_free_string(ptr)
        }
        let json = String(cString: ptr)
        return try decoder.decode(T.self, from: Data(json.utf8))
    }

    private func unwrap<T>(_ response: RustCoreEnvelope<T>) throws -> T {
        if response.ok, let data = response.data {
            return data
        }
        throw RustCoreClientError.rustError(response.error ?? "unknown")
    }

    private func ensureInitialized() throws {
        if didInitialize {
            return
        }
        let appSupport = try FileManager.default.url(
            for: .applicationSupportDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: true
        )
        let appDirectory = appSupport.appendingPathComponent("DeadlinePanelNative", isDirectory: true)
        try FileManager.default.createDirectory(at: appDirectory, withIntermediateDirectories: true)
        try initialize(databasePath: appDirectory.appendingPathComponent("deadline-panel.sqlite3").path)
    }

    private func migrateLegacyDatabaseIfNeeded(nativeDatabasePath: String) throws {
        if didAttemptMigration {
            return
        }
        didAttemptMigration = true

        let taskMigrationKey = "nativeDidAttemptLegacyDatabaseMigration"
        let settingsMigrationKey = "nativeDidAttemptLegacySettingsMigration"
        let defaults = UserDefaults.standard
        let didMigrateTasks = defaults.bool(forKey: taskMigrationKey)
        let didMigrateSettings = defaults.bool(forKey: settingsMigrationKey)
        if didMigrateTasks && didMigrateSettings {
            return
        }

        guard let legacyURL = legacyDatabaseURL(nativeDatabasePath: nativeDatabasePath),
              FileManager.default.fileExists(atPath: legacyURL.path)
        else {
            defaults.set(true, forKey: taskMigrationKey)
            defaults.set(true, forKey: settingsMigrationKey)
            return
        }

        if !didMigrateTasks {
            try backupLegacyDatabase(legacyURL)
            let response: RustCoreEnvelope<LegacyMigrationResult> = try legacyURL.path.withCString { pointer in
                try decode(deadline_migrate_legacy_database_json(pointer))
            }
            let result = try unwrap(response)
            migrateLegacySettings(result.settings)
            defaults.set(true, forKey: taskMigrationKey)
            defaults.set(true, forKey: settingsMigrationKey)
            return
        }

        let response: RustCoreEnvelope<[String: String]> = try legacyURL.path.withCString { pointer in
            try decode(deadline_read_legacy_settings_json(pointer))
        }
        migrateLegacySettings(try unwrap(response))
        defaults.set(true, forKey: settingsMigrationKey)
    }

    private func legacyDatabaseURL(nativeDatabasePath: String) -> URL? {
        let nativeURL = URL(fileURLWithPath: nativeDatabasePath)
        let appSupport = nativeURL
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let candidates = [
            appSupport.appendingPathComponent("local.adhd-deadline-panel/deadline-panel.sqlite3"),
            appSupport.appendingPathComponent("Deadline Panel/deadline-panel.sqlite3")
        ]
        return candidates.first { FileManager.default.fileExists(atPath: $0.path) }
    }

    private func backupLegacyDatabase(_ legacyURL: URL) throws {
        let backupDirectory = legacyURL
            .deletingLastPathComponent()
            .appendingPathComponent("backups", isDirectory: true)
        try FileManager.default.createDirectory(at: backupDirectory, withIntermediateDirectories: true)
        let stamp = ISO8601DateFormatter.backupStampString(from: Date())
            .replacingOccurrences(of: ":", with: "-")
        let backupURL = backupDirectory.appendingPathComponent("deadline-panel-native-import-\(stamp).sqlite3")
        if !FileManager.default.fileExists(atPath: backupURL.path) {
            try FileManager.default.copyItem(at: legacyURL, to: backupURL)
        }
    }

    private func migrateLegacySettings(_ settings: [String: String]) {
        let supportedKeys = [
            "focus_limit",
            "app_language",
            "sync_supabase_url",
            "sync_supabase_anon_key",
            "sync_code"
        ]
        let defaults = UserDefaults.standard
        for key in supportedKeys where defaults.object(forKey: key) == nil {
            guard let value = settings[key], !value.isEmpty else {
                continue
            }
            if key == "focus_limit", let limit = Int(value) {
                defaults.set(limit, forKey: key)
            } else {
                defaults.set(value, forKey: key)
            }
        }
    }
}

private struct LegacyMigrationResult: Decodable {
    let imported: Int
    let settings: [String: String]
}
