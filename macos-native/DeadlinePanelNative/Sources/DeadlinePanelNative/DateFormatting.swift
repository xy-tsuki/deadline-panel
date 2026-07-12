import Foundation

extension ISO8601DateFormatter {
    static func deadlinePanelDate(from value: String) -> Date? {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return DeadlinePanelDateFormatterCache.shared.deadlinePanelDate(from: trimmed)
    }

    static func deadlinePanelString(from date: Date) -> String {
        DeadlinePanelDateFormatterCache.shared.deadlinePanelString(from: date)
    }

    static func backupStampString(from date: Date) -> String {
        DeadlinePanelDateFormatterCache.shared.deadlinePanelString(from: date)
    }

    static func deadlinePanelEditableString(from date: Date) -> String {
        DeadlinePanelDateFormatterCache.shared.editableString(from: date)
    }

    static func deadlinePanelEditableDate(from value: String) -> Date? {
        DeadlinePanelDateFormatterCache.shared.editableDate(
            from: value.trimmingCharacters(in: .whitespacesAndNewlines)
        )
    }

    static func deadlinePanelCommandDate(from value: String) -> Date? {
        DeadlinePanelDateFormatterCache.shared.commandDate(from: value)
    }
}

private final class DeadlinePanelDateFormatterCache: @unchecked Sendable {
    static let shared = DeadlinePanelDateFormatterCache()

    private let lock = NSLock()
    private let isoWithFractions = isoFormatter(withFractions: true)
    private let isoWithoutFractions = isoFormatter(withFractions: false)
    private let editable = dateFormatter(format: "yyyy-MM-dd HH:mm")
    private let legacyInput = [
        "yyyy-MM-dd HH:mm:ss.SSSXXXXX",
        "yyyy-MM-dd HH:mm:ssXXXXX",
        "yyyy-MM-dd HH:mm:ss.SSSX",
        "yyyy-MM-dd HH:mm:ssX",
        "yyyy-MM-dd HH:mm"
    ].map(dateFormatter)
    private let commandInput = [
        "yyyy-MM-dd HH:mm",
        "yyyy-MM-dd'T'HH:mm",
        "yyyy-MM-dd HH:mm:ss",
        "yyyy-MM-dd'T'HH:mm:ss"
    ].map(dateFormatter)

    func deadlinePanelDate(from value: String) -> Date? {
        withLock {
            isoWithFractions.date(from: value)
                ?? isoWithoutFractions.date(from: value)
                ?? firstDate(from: value, using: legacyInput)
        }
    }

    func deadlinePanelString(from date: Date) -> String {
        withLock { isoWithoutFractions.string(from: date) }
    }

    func editableString(from date: Date) -> String {
        withLock { editable.string(from: date) }
    }

    func editableDate(from value: String) -> Date? {
        withLock { editable.date(from: value) }
    }

    func commandDate(from value: String) -> Date? {
        withLock { firstDate(from: value, using: commandInput) }
    }

    private func firstDate(from value: String, using formatters: [DateFormatter]) -> Date? {
        for formatter in formatters {
            if let date = formatter.date(from: value) {
                return date
            }
        }
        return nil
    }

    private func withLock<Result>(_ operation: () -> Result) -> Result {
        lock.lock()
        defer { lock.unlock() }
        return operation()
    }

    private static func isoFormatter(withFractions: Bool) -> ISO8601DateFormatter {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = withFractions
            ? [.withInternetDateTime, .withFractionalSeconds]
            : [.withInternetDateTime]
        return formatter
    }

    private static func dateFormatter(format: String) -> DateFormatter {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = format
        return formatter
    }
}
