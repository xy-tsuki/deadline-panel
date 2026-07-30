import Foundation

enum NativePreferenceMigration {
    private static let migrationKey = "nativeDidMigrateExperimentalBundleDefaults"
    private static let experimentalBundleIdentifier = "local.deadline-panel.native-dev"

    static func migrateExperimentalBundleDefaultsIfNeeded() {
        let defaults = UserDefaults.standard
        guard !defaults.bool(forKey: migrationKey) else {
            return
        }

        copyMissingValues(
            defaults.persistentDomain(forName: experimentalBundleIdentifier) ?? [:],
            into: defaults
        )
        defaults.set(true, forKey: migrationKey)
    }

    static func copyMissingValues(_ legacyValues: [String: Any], into defaults: UserDefaults) {
        for (key, value) in legacyValues where defaults.object(forKey: key) == nil {
            defaults.set(value, forKey: key)
        }
    }
}
