import Foundation
import UserNotifications

@MainActor
final class NativeNotificationController: ObservableObject {
    @Published private(set) var authorizationText = NativeStrings.current.notificationUnchecked
    @Published private(set) var errorMessage: String?

    func refreshAuthorizationStatus() {
        guard let center = notificationCenter else {
            authorizationText = Self.debugExecutableMessage
            errorMessage = nil
            return
        }

        center.getNotificationSettings { [weak self] settings in
            let status = settings.authorizationStatus
            Task { @MainActor in
                self?.authorizationText = Self.label(for: status)
            }
        }
    }

    func requestAuthorization() {
        guard let center = notificationCenter else {
            authorizationText = Self.debugExecutableMessage
            errorMessage = Self.debugExecutableMessage
            return
        }

        center.requestAuthorization(options: [.alert, .sound, .badge]) { [weak self] granted, error in
            Task { @MainActor in
                if let error {
                    self?.errorMessage = String(describing: error)
                } else {
                    self?.errorMessage = nil
                }
                self?.authorizationText = granted ? NativeStrings.current.notificationAllowed : NativeStrings.current.notificationDenied
            }
        }
    }

    func schedule(deadlines: [DeadlineTask]) {
        guard let center = notificationCenter else {
            authorizationText = Self.debugExecutableMessage
            return
        }

        center.removeAllPendingNotificationRequests()

        let activeDeadlines = deadlines
            .filter { $0.status != "completed" }
            .prefix(10)

        for task in activeDeadlines {
            guard let date = ISO8601DateFormatter.deadlinePanelDate(from: task.dueAt),
                  date > Date()
            else {
                continue
            }

            let content = UNMutableNotificationContent()
            content.title = NativeStrings.current.notificationDueTitle
            content.body = task.title
            content.sound = .default

            let components = Calendar.current.dateComponents(
                [.year, .month, .day, .hour, .minute],
                from: date
            )
            let trigger = UNCalendarNotificationTrigger(dateMatching: components, repeats: false)
            let request = UNNotificationRequest(
                identifier: "deadline-\(task.id)",
                content: content,
                trigger: trigger
            )
            center.add(request) { [weak self] error in
                guard let error else {
                    return
                }
                Task { @MainActor in
                    self?.errorMessage = String(describing: error)
                }
            }
        }
    }

    private static func label(for status: UNAuthorizationStatus) -> String {
        switch status {
        case .authorized:
            return NativeStrings.current.notificationAllowed
        case .denied:
            return NativeStrings.current.notificationDenied
        case .notDetermined:
            return NativeStrings.current.notificationNotDetermined
        case .provisional:
            return NativeStrings.current.notificationProvisional
        case .ephemeral:
            return NativeStrings.current.notificationEphemeral
        @unknown default:
            return NativeStrings.current.unknownTime
        }
    }

    private var notificationCenter: UNUserNotificationCenter? {
        guard Bundle.main.bundleURL.pathExtension == "app" else {
            return nil
        }
        return UNUserNotificationCenter.current()
    }

    private static var debugExecutableMessage: String {
        switch NativeLanguage.resolved {
        case .ja:
            return "Debug 実行では通知は無効です"
        case .en:
            return "Notifications unavailable in debug executable"
        default:
            return "Debug 运行中通知不可用"
        }
    }
}
