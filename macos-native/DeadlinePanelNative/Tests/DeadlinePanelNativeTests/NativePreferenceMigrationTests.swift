import XCTest
@testable import DeadlinePanelNative

final class NativePreferenceMigrationTests: XCTestCase {
    func testCopiesMissingValuesWithoutReplacingReleasePreferences() throws {
        let suiteName = "NativePreferenceMigrationTests.\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suiteName))
        defer {
            defaults.removePersistentDomain(forName: suiteName)
        }
        defaults.set("dark", forKey: "app_theme")

        NativePreferenceMigration.copyMissingValues(
            [
                "app_theme": "light",
                "app_language": "ja",
                "sync_code": "existing-experimental-code"
            ],
            into: defaults
        )

        XCTAssertEqual(defaults.string(forKey: "app_theme"), "dark")
        XCTAssertEqual(defaults.string(forKey: "app_language"), "ja")
        XCTAssertEqual(defaults.string(forKey: "sync_code"), "existing-experimental-code")
    }
}
