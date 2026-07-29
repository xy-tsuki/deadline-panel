import AppKit
import SwiftUI

enum NativeAppearance {
    static let defaultsKey = "appearance_mode"
    static let didChangeNotification = Notification.Name("NativeAppearanceDidChange")

    static func colorScheme(for value: String) -> ColorScheme? {
        switch value {
        case "light":
            return .light
        case "dark":
            return .dark
        default:
            return nil
        }
    }

    @MainActor
    static func applyAppAppearance(_ value: String = UserDefaults.standard.string(forKey: defaultsKey) ?? "system") {
        switch value {
        case "light":
            NSApp.appearance = NSAppearance(named: .aqua)
        case "dark":
            NSApp.appearance = NSAppearance(named: .darkAqua)
        default:
            NSApp.appearance = nil
        }
    }

    @MainActor
    static func notifyChange() {
        applyAppAppearance()
        NotificationCenter.default.post(name: didChangeNotification, object: nil)
    }
}

enum NativeMotion {
    static let lowPowerDefaultsKey = "native_low_power_mode"
}

private struct NativeLowPowerModeKey: EnvironmentKey {
    static let defaultValue = false
}

extension EnvironmentValues {
    var nativeLowPowerMode: Bool {
        get { self[NativeLowPowerModeKey.self] }
        set { self[NativeLowPowerModeKey.self] = newValue }
    }
}

extension View {
    func nativePreferredColorScheme(_ value: String) -> some View {
        preferredColorScheme(NativeAppearance.colorScheme(for: value))
    }

    func nativeLowPowerMode(_ enabled: Bool) -> some View {
        environment(\.nativeLowPowerMode, enabled)
            .transaction { transaction in
                guard enabled else {
                    return
                }
                transaction.animation = nil
                transaction.disablesAnimations = true
            }
    }
}
