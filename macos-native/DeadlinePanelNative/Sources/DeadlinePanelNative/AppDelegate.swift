import AppKit

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private let viewModel = DeadlineViewModel()
    private let notificationController = NativeNotificationController()
    private var panelCoordinator: NativePanelCoordinator?
    private var menuBarController: NativeMenuBarController?
    private var fullscreenMonitor: NativeFullscreenMonitor?

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
        NativeAppearance.applyAppAppearance()
        configureApplicationIcon()
        let coordinator = NativePanelCoordinator(viewModel: viewModel)
        panelCoordinator = coordinator
        menuBarController = NativeMenuBarController(
            panelCoordinator: coordinator,
            viewModel: viewModel,
            notificationController: notificationController
        )
        coordinator.onOpenSettings = { [weak self] in
            self?.menuBarController?.showSettingsWindow()
        }
        let monitor = NativeFullscreenMonitor(panelCoordinator: coordinator)
        fullscreenMonitor = monitor
        coordinator.showCollapsed()
        notificationController.refreshAuthorizationStatus()
        monitor.start()
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        false
    }

    private func configureApplicationIcon() {
        if let iconURL = Bundle.main.url(forResource: "AppIcon", withExtension: "icns"),
           let icon = NSImage(contentsOf: iconURL)
        {
            NSApp.applicationIconImage = icon
        }
    }
}
