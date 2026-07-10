import AppKit
import CoreGraphics

@MainActor
final class NativeFullscreenMonitor {
    static let defaultsKey = "auto_hide_when_fullscreen"

    private weak var panelCoordinator: NativePanelCoordinator?
    private var timer: Timer?
    private var defaultsObserver: NSObjectProtocol?

    init(panelCoordinator: NativePanelCoordinator) {
        self.panelCoordinator = panelCoordinator
    }

    func start() {
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: 1.5, repeats: true) { [weak self] _ in
            Task { @MainActor in
                self?.poll()
            }
        }
        defaultsObserver = NotificationCenter.default.addObserver(
            forName: UserDefaults.didChangeNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                self?.poll()
            }
        }
        poll()
    }

    func stop() {
        timer?.invalidate()
        timer = nil
        if let defaultsObserver {
            NotificationCenter.default.removeObserver(defaultsObserver)
            self.defaultsObserver = nil
        }
    }

    private func poll() {
        guard isEnabled else {
            panelCoordinator?.setAutoHidden(false)
            return
        }
        panelCoordinator?.setAutoHidden(foregroundWindowAppearsFullscreen())
    }

    private func foregroundWindowAppearsFullscreen() -> Bool {
        let screenFrames = NSScreen.screens.map(\.frame)
        guard !screenFrames.isEmpty else {
            return false
        }
        let options: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
        guard let windows = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] else {
            return false
        }

        for window in windows {
            let layer = window[kCGWindowLayer as String] as? Int ?? -1
            guard layer == 0 else {
                continue
            }
            let ownerPID = window[kCGWindowOwnerPID as String] as? pid_t
            if ownerPID == ProcessInfo.processInfo.processIdentifier {
                continue
            }
            guard let bounds = window[kCGWindowBounds as String] as? [String: Any],
                  let x = numberValue(bounds["X"]),
                  let y = numberValue(bounds["Y"]),
                  let width = numberValue(bounds["Width"]),
                  let height = numberValue(bounds["Height"])
            else {
                continue
            }

            let cgRect = CGRect(x: x, y: y, width: width, height: height)
            if coversAnyScreen(cgRect, screenFrames: screenFrames) {
                return true
            }
        }

        return false
    }

    private var isEnabled: Bool {
        let defaults = UserDefaults.standard
        guard defaults.object(forKey: Self.defaultsKey) != nil else {
            return true
        }
        return defaults.bool(forKey: Self.defaultsKey)
    }

    private func coversAnyScreen(_ cgWindowFrame: CGRect, screenFrames: [CGRect]) -> Bool {
        for screenFrame in screenFrames {
            let appKitFrame = CGRect(
                x: cgWindowFrame.minX,
                y: screenFrame.maxY - cgWindowFrame.maxY,
                width: cgWindowFrame.width,
                height: cgWindowFrame.height
            )
            if coversScreen(cgWindowFrame, screenFrame: screenFrame)
                || coversScreen(appKitFrame, screenFrame: screenFrame)
            {
                return true
            }
        }
        return false
    }

    private func coversScreen(_ windowFrame: CGRect, screenFrame: CGRect) -> Bool {
        let tolerance: CGFloat = 8
        return abs(windowFrame.minX - screenFrame.minX) <= tolerance
            && abs(windowFrame.minY - screenFrame.minY) <= tolerance
            && abs(windowFrame.width - screenFrame.width) <= tolerance
            && abs(windowFrame.height - screenFrame.height) <= tolerance
    }

    private func numberValue(_ value: Any?) -> CGFloat? {
        if let value = value as? CGFloat {
            return value
        }
        if let value = value as? NSNumber {
            return CGFloat(truncating: value)
        }
        return nil
    }
}
