import AppKit

@MainActor
final class NativeMenuBarController: NSObject {
    private let statusItem: NSStatusItem
    private let panelCoordinator: NativePanelCoordinator
    private let viewModel: DeadlineViewModel
    private let notificationController: NativeNotificationController
    private lazy var fileController = FileImportExportController(viewModel: viewModel)
    private var dockVisible = false
    private var languageObserver: NSObjectProtocol?
    private lazy var settingsController = NativeSettingsController(
        viewModel: viewModel,
        notificationController: notificationController,
        onShowDock: { [weak self] in self?.setDockVisible(true) },
        onHideDock: { [weak self] in self?.setDockVisible(false) },
        onShowPanel: { [weak self] in self?.panelCoordinator.showCollapsed() },
        onResetPosition: { [weak self] in self?.panelCoordinator.resetPanelPosition() },
        onHideMinutes: { [weak self] minutes in self?.panelCoordinator.hideTemporarily(minutes: minutes) }
    )

    init(
        panelCoordinator: NativePanelCoordinator,
        viewModel: DeadlineViewModel,
        notificationController: NativeNotificationController
    ) {
        self.panelCoordinator = panelCoordinator
        self.viewModel = viewModel
        self.notificationController = notificationController
        self.statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        super.init()
        configureStatusItem()
        rebuildMenu()
        languageObserver = NotificationCenter.default.addObserver(
            forName: UserDefaults.didChangeNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                self?.rebuildMenu()
            }
        }
    }

    private func configureStatusItem() {
        guard let button = statusItem.button else {
            return
        }
        button.image = menuBarImage()
        button.imagePosition = .imageOnly
        button.target = self
        button.action = #selector(openMenu(_:))
        button.sendAction(on: [.leftMouseUp, .rightMouseUp])
    }

    private func menuBarImage() -> NSImage? {
        let image = Bundle.main.url(forResource: "menuBarIcon", withExtension: "png")
            .flatMap { NSImage(contentsOf: $0) }
            ?? NSImage(systemSymbolName: "clock", accessibilityDescription: "Deadline Panel")
        image?.isTemplate = true
        image?.size = NSSize(width: 18, height: 18)
        return image
    }

    private func rebuildMenu() {
        let strings = NativeStrings.current
        let menu = NSMenu()
        menu.addItem(
            NSMenuItem(
                title: strings.menuShowPanel,
                action: #selector(showPanel),
                keyEquivalent: ""
            )
        )
        menu.addItem(
            NSMenuItem(
                title: strings.menuHidePanel,
                action: #selector(hidePanel),
                keyEquivalent: ""
            )
        )
        menu.addItem(
            NSMenuItem(
                title: strings.hide15,
                action: #selector(hide15Minutes),
                keyEquivalent: ""
            )
        )
        menu.addItem(
            NSMenuItem(
                title: strings.hide30,
                action: #selector(hide30Minutes),
                keyEquivalent: ""
            )
        )
        menu.addItem(
            NSMenuItem(
                title: strings.menuHide60,
                action: #selector(hide60Minutes),
                keyEquivalent: ""
            )
        )
        menu.addItem(
            NSMenuItem(
                title: strings.menuResetPosition,
                action: #selector(resetPanelPosition),
                keyEquivalent: ""
            )
        )
        menu.addItem(.separator())
        menu.addItem(
            NSMenuItem(
                title: strings.importJSON,
                action: #selector(importJSON),
                keyEquivalent: "i"
            )
        )
        menu.addItem(
            NSMenuItem(
                title: strings.exportJSON,
                action: #selector(exportJSON),
                keyEquivalent: "e"
            )
        )
        menu.addItem(.separator())
        menu.addItem(
            NSMenuItem(
                title: strings.menuSettings,
                action: #selector(showSettings),
                keyEquivalent: ","
            )
        )
        menu.addItem(
            NSMenuItem(
                title: strings.scheduleNotifications,
                action: #selector(scheduleNotifications),
                keyEquivalent: ""
            )
        )
        menu.addItem(
            NSMenuItem(
                title: dockVisible ? strings.hideFromDock : strings.showInDock,
                action: #selector(toggleDockVisibility),
                keyEquivalent: ""
            )
        )
        menu.addItem(.separator())
        menu.addItem(
            NSMenuItem(
                title: strings.menuRestart,
                action: #selector(restart),
                keyEquivalent: ""
            )
        )
        menu.addItem(
            NSMenuItem(
                title: strings.menuQuit,
                action: #selector(quit),
                keyEquivalent: "q"
            )
        )

        for item in menu.items {
            item.target = self
        }
        statusItem.menu = menu
    }

    @objc private func openMenu(_ sender: Any?) {
        statusItem.button?.performClick(nil)
    }

    @objc private func showPanel() {
        panelCoordinator.showCollapsed()
    }

    @objc private func hidePanel() {
        panelCoordinator.hideTemporarily()
    }

    @objc private func hide15Minutes() {
        panelCoordinator.hideTemporarily(minutes: 15)
    }

    @objc private func hide30Minutes() {
        panelCoordinator.hideTemporarily(minutes: 30)
    }

    @objc private func hide60Minutes() {
        panelCoordinator.hideTemporarily(minutes: 60)
    }

    @objc private func resetPanelPosition() {
        panelCoordinator.resetPanelPosition()
    }

    @objc private func importJSON() {
        fileController.importJSON()
    }

    @objc private func exportJSON() {
        fileController.exportJSON()
    }

    @objc private func toggleDockVisibility() {
        setDockVisible(!dockVisible)
    }

    @objc private func showSettings() {
        showSettingsWindow()
    }

    func showSettingsWindow() {
        settingsController.show()
    }

    @objc private func scheduleNotifications() {
        notificationController.schedule(deadlines: viewModel.deadlines)
    }

    @objc private func restart() {
        NativeRelaunch.restart()
    }

    private func setDockVisible(_ visible: Bool) {
        dockVisible = visible
        NSApp.setActivationPolicy(visible ? .regular : .accessory)
        if visible {
            NSApp.activate(ignoringOtherApps: true)
        }
        rebuildMenu()
    }

    @objc private func quit() {
        NSApp.terminate(nil)
    }
}
