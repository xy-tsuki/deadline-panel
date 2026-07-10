import Foundation

@_silgen_name("deadline_core_version_json")
private func deadline_core_version_json() -> UnsafeMutablePointer<CChar>?

@_silgen_name("deadline_initialize_json")
private func deadline_initialize_json(_ databasePath: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>?

@_silgen_name("deadline_migrate_legacy_database_json")
private func deadline_migrate_legacy_database_json(_ legacyDatabasePath: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>?

@_silgen_name("deadline_list_json")
private func deadline_list_json() -> UnsafeMutablePointer<CChar>?

@_silgen_name("deadline_create_json")
private func deadline_create_json(_ input: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>?

@_silgen_name("deadline_delete")
private func deadline_delete(_ id: UnsafePointer<CChar>) -> Bool

@_silgen_name("deadline_free_string")
private func deadline_free_string(_ ptr: UnsafeMutablePointer<CChar>?)

struct RustCoreEnvelope<T: Decodable>: Decodable {
    let ok: Bool
    let data: T?
    let error: String?
}

struct RustCoreVersion: Decodable {
    let version: String
}

struct DeadlineTask: Decodable {
    let id: String
    let title: String
    let dueAt: String
    let priority: String
    let status: String
    let notes: String
    let source: String
    let isCurrent: Bool
    let createdAt: String
    let updatedAt: String
    let completedAt: String?
}

struct NewDeadlineInput: Encodable {
    let title: String
    let dueAt: String
    let priority: String
    let notes: String?
    let source: String?
}

enum RustCoreClientError: Error, CustomStringConvertible {
    case nullPointer
    case rustError(String)
    case missingData

    var description: String {
        switch self {
        case .nullPointer:
            return "Rust returned a null pointer"
        case .rustError(let message):
            return "Rust error: \(message)"
        case .missingData:
            return "Rust response did not contain data"
        }
    }
}

final class RustCoreClient {
    private let decoder = JSONDecoder()
    private let encoder = JSONEncoder()
    private var didInitialize = false

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

    @discardableResult
    func deleteDeadline(id: String) -> Bool {
        id.withCString { pointer in
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
        let databaseURL = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("deadline-core-smoke-\(UUID().uuidString).sqlite3")
        try initialize(databasePath: databaseURL.path)
    }
}
