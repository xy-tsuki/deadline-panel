import AppKit
import SwiftUI

@MainActor
final class NativeSettingsController {
    private let loginItemController = LoginItemController()
    private let notificationController: NativeNotificationController
    private let viewModel: DeadlineViewModel
    private lazy var fileController = FileImportExportController(viewModel: viewModel)
    private var window: NSWindow?
    private var appearanceObserver: NSObjectProtocol?
    private let onShowDock: () -> Void
    private let onHideDock: () -> Void
    private let onShowPanel: () -> Void
    private let onResetPosition: () -> Void
    private let onHideMinutes: (TimeInterval) -> Void

    init(
        viewModel: DeadlineViewModel,
        notificationController: NativeNotificationController,
        onShowDock: @escaping () -> Void,
        onHideDock: @escaping () -> Void,
        onShowPanel: @escaping () -> Void,
        onResetPosition: @escaping () -> Void,
        onHideMinutes: @escaping (TimeInterval) -> Void
    ) {
        self.viewModel = viewModel
        self.notificationController = notificationController
        self.onShowDock = onShowDock
        self.onHideDock = onHideDock
        self.onShowPanel = onShowPanel
        self.onResetPosition = onResetPosition
        self.onHideMinutes = onHideMinutes
        appearanceObserver = NotificationCenter.default.addObserver(
            forName: NativeAppearance.didChangeNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                self?.reloadSettingsWindowForAppearance()
            }
        }
    }

    func show() {
        if let window {
            window.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            return
        }

        let view = makeSettingsView()
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 520, height: 520),
            styleMask: [.titled, .closable, .miniaturizable],
            backing: .buffered,
            defer: false
        )
        window.title = NativeStrings.current.settingsTitle
        window.contentView = NSHostingView(rootView: view)
        window.center()
        window.isReleasedWhenClosed = false
        self.window = window
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    private func makeSettingsView() -> SettingsView {
        SettingsView(
            viewModel: viewModel,
            loginItemController: loginItemController,
            notificationController: notificationController,
            onShowDock: onShowDock,
            onHideDock: onHideDock,
            onShowPanel: onShowPanel,
            onResetPosition: onResetPosition,
            onHideMinutes: onHideMinutes,
            onScheduleNotifications: { [viewModel, notificationController] in
                viewModel.load()
                notificationController.schedule(deadlines: viewModel.deadlines)
            },
            onImportJSON: { [weak self] in self?.fileController.importJSON() },
            onExportJSON: { [weak self] in self?.fileController.exportJSON() }
        )
    }

    private func reloadSettingsWindowForAppearance() {
        guard let window else {
            return
        }
        let frame = window.frame
        window.appearance = NSApp.appearance
        window.title = NativeStrings.current.settingsTitle
        window.contentView = NSHostingView(rootView: makeSettingsView())
        window.setFrame(frame, display: true)
    }
}

struct SettingsView: View {
    @ObservedObject var viewModel: DeadlineViewModel
    @ObservedObject var loginItemController: LoginItemController
    @ObservedObject var notificationController: NativeNotificationController
    let onShowDock: () -> Void
    let onHideDock: () -> Void
    let onShowPanel: () -> Void
    let onResetPosition: () -> Void
    let onHideMinutes: (TimeInterval) -> Void
    let onScheduleNotifications: () -> Void
    let onImportJSON: () -> Void
    let onExportJSON: () -> Void
    @AppStorage("app_language") private var language = "system"
    @AppStorage(NativeAppearance.defaultsKey) private var appearanceMode = "system"
    @AppStorage(NativeFullscreenMonitor.defaultsKey) private var autoHideFullscreen = true
    @State private var message: String?
    @State private var isCheckingUpdates = false

    var body: some View {
        let strings = NativeStrings.current
        ZStack {
            Rectangle()
                .fill(.ultraThinMaterial)
                .opacity(0.9)
                .overlay(Color.black.opacity(0.05))
                .ignoresSafeArea()

            ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("Deadline Panel")
                    .font(.system(size: 22, weight: .semibold))

                GlassPanel(cornerRadius: 16, material: .thinMaterial) {
                    VStack(alignment: .leading, spacing: 14) {
                        HStack {
                            VStack(alignment: .leading, spacing: 3) {
                                Text(strings.focusCount)
                                    .font(.system(size: 15, weight: .semibold))
                                Text(strings.focusCopy)
                                    .font(.system(size: 12))
                                    .foregroundStyle(.secondary)
                            }
                            Spacer()
                            LiquidSegmentedControl(selection: $viewModel.focusLimit, values: [3, 5, 10])
                        }

                        Divider().opacity(0.25)

                        HStack {
                            VStack(alignment: .leading, spacing: 3) {
                                Text(strings.language)
                                    .font(.system(size: 15, weight: .semibold))
                                Text(strings.languageCopy)
                                    .font(.system(size: 12))
                                    .foregroundStyle(.secondary)
                            }
                            Spacer()
                            LiquidTextSegmentedControl(
                                selection: $language,
                                options: [
                                    .init(id: "system", title: strings.followSystem),
                                    .init(id: "zh", title: strings.chinese),
                                    .init(id: "ja", title: strings.japanese),
                                    .init(id: "en", title: strings.english)
                                ]
                            )
                        }

                        Divider().opacity(0.25)

                        HStack {
                            VStack(alignment: .leading, spacing: 3) {
                                Text(strings.theme)
                                    .font(.system(size: 15, weight: .semibold))
                                Text(strings.themeCopy)
                                    .font(.system(size: 12))
                                    .foregroundStyle(.secondary)
                            }
                            Spacer()
                            LiquidTextSegmentedControl(
                                selection: $appearanceMode,
                                options: [
                                    .init(id: "system", title: strings.followSystem),
                                    .init(id: "light", title: strings.lightMode),
                                    .init(id: "dark", title: strings.darkMode)
                                ]
                            )
                        }

                        Divider().opacity(0.25)

                        Toggle(
                            strings.autostart,
                            isOn: Binding(
                                get: { loginItemController.isEnabled },
                                set: { loginItemController.setEnabled($0) }
                            )
                        )

                        Toggle(strings.autoHideFullscreen, isOn: $autoHideFullscreen)

                        Text(strings.autoHideFullscreenCopy)
                            .font(.system(size: 12))
                            .foregroundStyle(.secondary)

                        HStack(spacing: 10) {
                            LiquidToolButton(title: strings.showInDock, systemImage: "dock.rectangle") {
                                onShowDock()
                            }
                            LiquidToolButton(title: strings.hideFromDock, systemImage: "dock.arrow.down.rectangle") {
                                onHideDock()
                            }
                        }
                    }
                    .padding(16)
                }

                GlassPanel(cornerRadius: 16, material: .thinMaterial) {
                    VStack(alignment: .leading, spacing: 12) {
                        HStack(spacing: 10) {
                            LiquidToolButton(title: strings.requestNotifications, systemImage: "bell") {
                                notificationController.requestAuthorization()
                            }
                            LiquidToolButton(title: strings.scheduleNotifications, systemImage: "calendar.badge.clock") {
                                onScheduleNotifications()
                            }
                        }
                        Text(notificationController.authorizationText)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .padding(16)
                }

                GlassPanel(cornerRadius: 16, material: .thinMaterial) {
                    Grid(horizontalSpacing: 10, verticalSpacing: 12) {
                        GridRow {
                            LiquidToolButton(title: strings.menuShowPanel, systemImage: "sidebar.leading", fillsAvailableWidth: true) {
                                onShowPanel()
                            }
                            .frame(maxWidth: .infinity)
                            LiquidToolButton(title: strings.hide15, systemImage: "eye.slash", fillsAvailableWidth: true) {
                                onHideMinutes(15)
                            }
                            .frame(maxWidth: .infinity)
                            LiquidToolButton(title: strings.hide30, systemImage: "eye.slash", fillsAvailableWidth: true) {
                                onHideMinutes(30)
                            }
                            .frame(maxWidth: .infinity)
                        }

                        GridRow {
                            LiquidToolButton(title: strings.resetPosition, systemImage: "arrow.down.right.and.arrow.up.left", fillsAvailableWidth: true) {
                                onResetPosition()
                                message = strings.resetDone
                            }
                            .frame(maxWidth: .infinity)
                            LiquidToolButton(title: strings.dataDirectory, systemImage: "folder", fillsAvailableWidth: true) {
                                openDataDirectory()
                            }
                            .frame(maxWidth: .infinity)
                            LiquidToolButton(title: strings.backupDatabase, systemImage: "externaldrive", fillsAvailableWidth: true) {
                                backupDatabase()
                            }
                            .frame(maxWidth: .infinity)
                        }

                        GridRow {
                            LiquidToolButton(title: isCheckingUpdates ? strings.checkingUpdates : strings.checkUpdates, systemImage: "arrow.clockwise", isDisabled: isCheckingUpdates, fillsAvailableWidth: true) {
                                checkUpdates()
                            }
                            .frame(maxWidth: .infinity)
                            LiquidToolButton(title: strings.importJSON, systemImage: "square.and.arrow.up", fillsAvailableWidth: true) {
                                onImportJSON()
                            }
                            .frame(maxWidth: .infinity)
                            LiquidToolButton(title: strings.exportJSON, systemImage: "square.and.arrow.down", fillsAvailableWidth: true) {
                                onExportJSON()
                            }
                            .frame(maxWidth: .infinity)
                        }
                    }
                    .padding(16)
                }

                if let errorMessage = loginItemController.errorMessage {
                    Text(errorMessage)
                        .font(.caption)
                        .foregroundStyle(.red)
                }
                if let errorMessage = notificationController.errorMessage {
                    Text(errorMessage)
                        .font(.caption)
                        .foregroundStyle(.red)
                }
                if let message {
                    Text(message)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                Spacer()
            }
            .padding(22)
            }
        }
        .frame(width: 520, height: 520)
        .nativePreferredColorScheme(appearanceMode)
        .onChange(of: appearanceMode) { _, _ in
            NativeAppearance.notifyChange()
        }
        .onChange(of: language) { _, _ in
            NativeLanguage.notifyChange()
        }
    }

    private func dataDirectoryURL() throws -> URL {
        let appSupport = try FileManager.default.url(
            for: .applicationSupportDirectory,
            in: .userDomainMask,
            appropriateFor: nil,
            create: true
        )
        let directory = appSupport.appendingPathComponent("DeadlinePanelNative", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }

    private func openDataDirectory() {
        do {
            NSWorkspace.shared.activateFileViewerSelecting([try dataDirectoryURL()])
            message = nil
        } catch {
            message = NativeStrings.current.openDataFailed
        }
    }

    private func backupDatabase() {
        do {
            let directory = try dataDirectoryURL()
            let database = directory.appendingPathComponent("deadline-panel.sqlite3")
            let backups = directory.appendingPathComponent("backups", isDirectory: true)
            try FileManager.default.createDirectory(at: backups, withIntermediateDirectories: true)
            let stamp = ISO8601DateFormatter.backupStampString(from: Date()).replacingOccurrences(of: ":", with: "-")
            let target = backups.appendingPathComponent("deadline-panel-\(stamp).sqlite3")
            if FileManager.default.fileExists(atPath: database.path) {
                try FileManager.default.copyItem(at: database, to: target)
                message = NativeStrings.current.backupDone(target.path)
            } else {
                message = NativeStrings.current.databaseNotCreated
            }
        } catch {
            message = NativeStrings.current.backupFailed
        }
    }

    private func checkUpdates() {
        isCheckingUpdates = true
        message = NativeStrings.current.checkingUpdates
        Task {
            defer {
                Task { @MainActor in isCheckingUpdates = false }
            }
            do {
                let url = URL(string: "https://api.github.com/repos/xy-tsuki/deadline-panel/releases/latest")!
                let (data, response) = try await URLSession.shared.data(from: url)
                guard let httpResponse = response as? HTTPURLResponse,
                      httpResponse.statusCode == 200
                else {
                    await MainActor.run { message = NativeStrings.current.noReleaseFound }
                    return
                }
                let release = try JSONDecoder().decode(GitHubRelease.self, from: data)
                await MainActor.run {
                    message = NativeStrings.current.latestVersion(release.tag_name)
                }
            } catch {
                await MainActor.run {
                    message = NativeStrings.current.updateCheckFailed
                }
            }
        }
    }
}

private struct GitHubRelease: Decodable {
    let tag_name: String
}
