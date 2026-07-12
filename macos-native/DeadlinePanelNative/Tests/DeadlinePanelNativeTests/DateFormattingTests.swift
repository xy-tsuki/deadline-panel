import Foundation
import Testing
@testable import DeadlinePanelNative

struct DateFormattingTests {
    @Test(arguments: [
        "2026-07-10T13:00:00.123+09:00",
        "2026-07-10T13:00:00+09:00",
        "2026-07-10 13:00:00.123+09:00",
        "2026-07-10 13:00:00+09:00",
        "2026-07-10 13:00"
    ])
    func parsesCurrentAndLegacyFormats(_ value: String) {
        #expect(ISO8601DateFormatter.deadlinePanelDate(from: value) != nil)
    }

    @Test
    func editableFormatRoundTrips() throws {
        let source = "2026-07-10 13:00"
        let date = try #require(ISO8601DateFormatter.deadlinePanelEditableDate(from: source))

        #expect(ISO8601DateFormatter.deadlinePanelEditableString(from: date) == source)
    }

    @Test(arguments: [
        "2026-07-10 13:00",
        "2026-07-10T13:00",
        "2026-07-10 13:00:45",
        "2026-07-10T13:00:45"
    ])
    func parsesCommandFormats(_ value: String) {
        #expect(ISO8601DateFormatter.deadlinePanelCommandDate(from: value) != nil)
    }

    @Test
    func outputOmitsFractionalSeconds() throws {
        let date = try #require(
            ISO8601DateFormatter.deadlinePanelDate(from: "2026-07-10T13:00:00.123+09:00")
        )
        let output = ISO8601DateFormatter.deadlinePanelString(from: date)

        #expect(!output.contains(".123"))
        #expect(ISO8601DateFormatter.deadlinePanelDate(from: output) != nil)
    }
}
