import Foundation

extension ISO8601DateFormatter {
    static func deadlinePanelDate(from value: String) -> Date? {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return deadlinePanelFormatter(withFractions: true).date(from: trimmed)
            ?? deadlinePanelFormatter(withFractions: false).date(from: trimmed)
            ?? legacyDeadlinePanelDate(from: trimmed)
    }

    static func deadlinePanelString(from date: Date) -> String {
        deadlinePanelFormatter(withFractions: false).string(from: date)
    }

    static func backupStampString(from date: Date) -> String {
        deadlinePanelFormatter(withFractions: false).string(from: date)
    }

    private static func deadlinePanelFormatter(withFractions: Bool) -> ISO8601DateFormatter {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = withFractions
            ? [.withInternetDateTime, .withFractionalSeconds]
            : [.withInternetDateTime]
        return formatter
    }

    private static func legacyDeadlinePanelDate(from value: String) -> Date? {
        let formats = [
            "yyyy-MM-dd HH:mm:ss.SSSXXXXX",
            "yyyy-MM-dd HH:mm:ssXXXXX",
            "yyyy-MM-dd HH:mm:ss.SSSX",
            "yyyy-MM-dd HH:mm:ssX",
            "yyyy-MM-dd HH:mm"
        ]

        for format in formats {
            let formatter = DateFormatter()
            formatter.locale = Locale(identifier: "en_US_POSIX")
            formatter.dateFormat = format
            if let date = formatter.date(from: value) {
                return date
            }
        }
        return nil
    }
}
