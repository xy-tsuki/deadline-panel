import Foundation

struct RustCoreEnvelope<T: Decodable>: Decodable {
    let ok: Bool
    let data: T?
    let error: String?
}

struct RustCoreVersion: Decodable {
    let version: String
}

struct DeadlineTask: Codable, Identifiable {
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

struct NewDeadlineInput: Codable {
    let title: String
    let dueAt: String
    let priority: String
    let notes: String?
    let source: String?
}

struct UpdateTaskInput: Encodable {
    let title: String?
    let dueAt: String?
    let priority: String?
    let notes: String?
    let status: String?
    let source: String?
    let isCurrent: Bool?
    let completedAt: String??
}

struct UpdateDeadlineRequest: Encodable {
    let id: String
    let fields: UpdateTaskInput
}

struct ImportResult: Decodable {
    let imported: Int
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

extension UpdateTaskInput {
    static func status(_ status: String, completedAt: String?? = nil) -> UpdateTaskInput {
        UpdateTaskInput(
            title: nil,
            dueAt: nil,
            priority: nil,
            notes: nil,
            status: status,
            source: nil,
            isCurrent: nil,
            completedAt: completedAt
        )
    }
}
