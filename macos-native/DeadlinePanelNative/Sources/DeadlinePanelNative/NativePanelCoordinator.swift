import AppKit
import SwiftUI

@MainActor
final class NativePanelCoordinator {
    private enum Layout {
        static let width: CGFloat = 372
        static let collapsedPadding: CGFloat = 0
        static let collapsedWindowWidth: CGFloat = width + collapsedPadding * 2
        static let collapsedHeight: CGFloat = 44
        static let collapsedWindowHeight: CGFloat = collapsedHeight + collapsedPadding * 2
        static let expandedHeight: CGFloat = 600
        static let gap: CGFloat = 10
        static let margin: CGFloat = 18
        static let cornerRadius: CGFloat = 14
    }

    private enum DefaultsKey {
        static let collapsedX = "nativePanelCollapsedX"
        static let collapsedY = "nativePanelCollapsedY"
    }

    private let viewModel: DeadlineViewModel
    private let cloudSyncController: NativeCloudSyncController
    private lazy var fileController = FileImportExportController(viewModel: viewModel)
    private lazy var collapsedPanel = makePanel(
        width: Layout.collapsedWindowWidth,
        height: Layout.collapsedWindowHeight,
        acceptsKeyboardInput: false,
        hasWindowShadow: true
    )
    private lazy var expandedPanel = makePanel(
        width: Layout.width,
        height: Layout.expandedHeight,
        acceptsKeyboardInput: true,
        hasWindowShadow: true
    )
    private var isExpanded = false
    private var openDown = false
    private var isPanelVisible = false
    private var manuallyHidden = false
    private var autoHidden = false
    private var dragStartOrigin: CGPoint?
    private var dragStartMouseLocation: CGPoint?
    private var temporaryHideTimer: Timer?
    private var collapseTimer: Timer?
    private var isDraggingCollapsedStrip = false
    private var isControlInteractionActive = false
    private var appearanceObserver: NSObjectProtocol?
    private var languageObserver: NSObjectProtocol?
    var onOpenSettings: (() -> Void)?

    init(viewModel: DeadlineViewModel, cloudSyncController: NativeCloudSyncController) {
        self.viewModel = viewModel
        self.cloudSyncController = cloudSyncController
        configureContent()
        appearanceObserver = NotificationCenter.default.addObserver(
            forName: NativeAppearance.didChangeNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                self?.reloadPanelContentForAppearance()
            }
        }
        languageObserver = NotificationCenter.default.addObserver(
            forName: NativeLanguage.didChangeNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                self?.reloadPanelContentForAppearance()
            }
        }
    }

    func showCollapsed() {
        temporaryHideTimer?.invalidate()
        temporaryHideTimer = nil
        cancelScheduledCollapse()
        viewModel.load()
        positionCollapsedPanel()
        collapsedPanel.orderFrontRegardless()
        expandedPanel.orderOut(nil)
        isExpanded = false
        isPanelVisible = true
        manuallyHidden = false
        autoHidden = false
    }

    func hidePanel() {
        temporaryHideTimer?.invalidate()
        temporaryHideTimer = nil
        cancelScheduledCollapse()
        collapsedPanel.orderOut(nil)
        expandedPanel.orderOut(nil)
        isExpanded = false
        isPanelVisible = false
        manuallyHidden = true
    }

    func hideTemporarily(minutes: TimeInterval? = nil) {
        cancelScheduledCollapse()
        collapsedPanel.orderOut(nil)
        expandedPanel.orderOut(nil)
        isExpanded = false
        isPanelVisible = false
        manuallyHidden = true
        temporaryHideTimer?.invalidate()

        guard let minutes else {
            return
        }

        temporaryHideTimer = Timer.scheduledTimer(withTimeInterval: minutes * 60, repeats: false) { [weak self] _ in
            Task { @MainActor in
                self?.showCollapsed()
            }
        }
    }

    func resetPanelPosition() {
        let defaults = UserDefaults.standard
        defaults.removeObject(forKey: DefaultsKey.collapsedX)
        defaults.removeObject(forKey: DefaultsKey.collapsedY)
        positionCollapsedPanel()
        if isExpanded {
            positionExpandedPanel()
        }
    }

    func togglePanelVisibility() {
        if isPanelVisible {
            hidePanel()
        } else {
            showCollapsed()
        }
    }

    func setAutoHidden(_ hidden: Bool) {
        guard autoHidden != hidden else {
            return
        }
        autoHidden = hidden
        if hidden {
            collapsedPanel.orderOut(nil)
            expandedPanel.orderOut(nil)
            isExpanded = false
            isPanelVisible = false
        } else if !manuallyHidden {
            showCollapsed()
        }
    }

    func expand() {
        cancelScheduledCollapse()
        guard !isExpanded else {
            return
        }
        isExpanded = true
        cloudSyncController.syncAfterExpansion()
        positionExpandedPanel()
        expandedPanel.orderFrontRegardless()
        expandedPanel.makeKey()
    }

    func collapse() {
        cancelScheduledCollapse()
        guard isExpanded else {
            return
        }
        isExpanded = false
        expandedPanel.orderOut(nil)
    }

    private func configureContent() {
        collapsedPanel.contentView = makeHostingView(
            CollapsedStripView(
                viewModel: viewModel,
                onHover: { [weak self] hovering in
                    guard self?.isDraggingCollapsedStrip != true else {
                        return
                    }
                    if hovering {
                        self?.cancelScheduledCollapse()
                        self?.expand()
                    } else {
                        self?.scheduleCollapse()
                    }
                },
                onDragStart: { [weak self] in
                    self?.beginCollapsedDrag()
                },
                onDragChange: { [weak self] in
                    self?.dragCollapsedPanel()
                },
                onDragEnd: { [weak self] in
                    self?.endCollapsedDrag()
                },
                onCommand: { [weak self] command in
                    self?.handle(command)
                }
            )
        )
        expandedPanel.contentView = makeHostingView(
            ExpandedPanelView(
                viewModel: viewModel,
                cloudSyncController: cloudSyncController,
                onHover: { [weak self] hovering in
                    if hovering {
                        self?.cancelScheduledCollapse()
                    } else {
                        self?.scheduleCollapseIfAllowed()
                    }
                },
                onControlInteractionChanged: { [weak self] active in
                    self?.setControlInteractionActive(active)
                },
                onCommand: { [weak self] command in
                    self?.handle(command)
                },
                onImportJSON: { [weak self] in self?.fileController.importJSON() },
                onExportJSON: { [weak self] in self?.fileController.exportJSON() }
            )
        )
    }

    private func makeHostingView<Content: View>(_ rootView: Content) -> NSHostingView<Content> {
        let hostingView = NSHostingView(rootView: rootView)
        hostingView.wantsLayer = true
        hostingView.layer?.backgroundColor = NSColor.clear.cgColor
        return hostingView
    }

    private func handle(_ command: PanelCommand) {
        switch command {
        case .show:
            showCollapsed()
        case .hide:
            hideTemporarily()
        case .hideMinutes(let minutes):
            hideTemporarily(minutes: minutes)
        case .resetPosition:
            resetPanelPosition()
        case .settings:
            onOpenSettings?()
        case .restart:
            NativeRelaunch.restart()
        case .quit:
            NSApp.terminate(nil)
        }
    }

    private func scheduleCollapse() {
        guard !isControlInteractionActive else {
            cancelScheduledCollapse()
            return
        }
        collapseTimer?.invalidate()
        collapseTimer = Timer.scheduledTimer(withTimeInterval: 0.22, repeats: false) { [weak self] _ in
            Task { @MainActor in
                guard self?.isControlInteractionActive != true else {
                    self?.cancelScheduledCollapse()
                    return
                }
                self?.collapse()
            }
        }
    }

    private func scheduleCollapseIfAllowed() {
        guard !isControlInteractionActive else {
            cancelScheduledCollapse()
            return
        }
        scheduleCollapse()
    }

    private func cancelScheduledCollapse() {
        collapseTimer?.invalidate()
        collapseTimer = nil
    }

    private func setControlInteractionActive(_ active: Bool) {
        isControlInteractionActive = active
        if active {
            cancelScheduledCollapse()
        } else if isExpanded && !expandedPanel.frame.contains(NSEvent.mouseLocation) {
            scheduleCollapse()
        }
    }

    private func reloadPanelContentForAppearance() {
        configureContent()
        collapsedPanel.invalidateShadow()
        expandedPanel.invalidateShadow()
        if isPanelVisible {
            collapsedPanel.orderFrontRegardless()
        }
        if isExpanded {
            positionExpandedPanel()
            expandedPanel.orderFrontRegardless()
        }
    }

    private func makePanel(width: CGFloat, height: CGFloat, acceptsKeyboardInput: Bool, hasWindowShadow: Bool) -> NSPanel {
        let styleMask: NSWindow.StyleMask = acceptsKeyboardInput ? [.borderless] : [.borderless, .nonactivatingPanel]
        let panel = DeadlineFloatingPanel(
            contentRect: NSRect(x: 0, y: 0, width: width, height: height),
            styleMask: styleMask,
            backing: .buffered,
            defer: false
        )
        panel.acceptsKeyboardInput = acceptsKeyboardInput
        panel.becomesKeyOnlyIfNeeded = !acceptsKeyboardInput
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = hasWindowShadow
        panel.level = .floating
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.hidesOnDeactivate = false
        panel.isReleasedWhenClosed = false
        return panel
    }

    private func positionCollapsedPanel() {
        let visibleFrame = NSScreen.main?.visibleFrame ?? .zero
        let savedFrame = savedCollapsedFrame(in: visibleFrame)
        let frame = savedFrame ?? defaultCollapsedFrame(in: visibleFrame)
        collapsedPanel.setFrame(frame, display: true)
    }

    private func beginCollapsedDrag() {
        isDraggingCollapsedStrip = true
        cancelScheduledCollapse()
        collapse()
        dragStartOrigin = collapsedPanel.frame.origin
        dragStartMouseLocation = NSEvent.mouseLocation
    }

    private func dragCollapsedPanel() {
        guard let dragStartOrigin,
              let dragStartMouseLocation
        else {
            return
        }
        let mouseLocation = NSEvent.mouseLocation
        let deltaX = mouseLocation.x - dragStartMouseLocation.x
        let deltaY = mouseLocation.y - dragStartMouseLocation.y
        let visibleFrame = collapsedPanel.screen?.visibleFrame ?? NSScreen.main?.visibleFrame ?? .zero
        let proposed = NSRect(
            x: dragStartOrigin.x + deltaX,
            y: dragStartOrigin.y + deltaY,
            width: Layout.collapsedWindowWidth,
            height: Layout.collapsedWindowHeight
        )
        collapsedPanel.setFrame(clampedCollapsedFrame(proposed, in: visibleFrame), display: false)
    }

    private func endCollapsedDrag() {
        dragStartOrigin = nil
        dragStartMouseLocation = nil
        isDraggingCollapsedStrip = false
        saveCollapsedFrame(collapsedPanel.frame)
    }

    private func defaultCollapsedFrame(in visibleFrame: NSRect) -> NSRect {
        let x = visibleFrame.maxX - Layout.collapsedWindowWidth - Layout.margin
        let y = visibleFrame.minY + Layout.margin
        return NSRect(x: x, y: y, width: Layout.collapsedWindowWidth, height: Layout.collapsedWindowHeight)
    }

    private func savedCollapsedFrame(in visibleFrame: NSRect) -> NSRect? {
        let defaults = UserDefaults.standard
        guard defaults.object(forKey: DefaultsKey.collapsedX) != nil,
              defaults.object(forKey: DefaultsKey.collapsedY) != nil
        else {
            return nil
        }
        let frame = NSRect(
            x: defaults.double(forKey: DefaultsKey.collapsedX),
            y: defaults.double(forKey: DefaultsKey.collapsedY),
            width: Layout.collapsedWindowWidth,
            height: Layout.collapsedWindowHeight
        )
        guard visibleFrame.intersects(frame) else {
            return nil
        }
        return clampedCollapsedFrame(frame, in: visibleFrame)
    }

    private func saveCollapsedFrame(_ frame: NSRect) {
        let defaults = UserDefaults.standard
        defaults.set(frame.minX, forKey: DefaultsKey.collapsedX)
        defaults.set(frame.minY, forKey: DefaultsKey.collapsedY)
    }

    private func clampedCollapsedFrame(_ frame: NSRect, in visibleFrame: NSRect) -> NSRect {
        let x = min(max(frame.minX, visibleFrame.minX + Layout.margin), visibleFrame.maxX - Layout.collapsedWindowWidth - Layout.margin)
        let y = min(max(frame.minY, visibleFrame.minY + Layout.margin), visibleFrame.maxY - Layout.collapsedWindowHeight - Layout.margin)
        return NSRect(x: x, y: y, width: Layout.collapsedWindowWidth, height: Layout.collapsedWindowHeight)
    }

    private func positionExpandedPanel() {
        let visibleFrame = collapsedPanel.screen?.visibleFrame ?? NSScreen.main?.visibleFrame ?? .zero
        let stripFrame = collapsedPanel.frame.insetBy(dx: Layout.collapsedPadding, dy: Layout.collapsedPadding)
        let spaceBelow = stripFrame.minY - visibleFrame.minY
        let spaceAbove = visibleFrame.maxY - stripFrame.maxY
        openDown = spaceBelow >= Layout.expandedHeight + Layout.gap || spaceBelow > spaceAbove

        let x = min(max(stripFrame.minX, visibleFrame.minX + Layout.margin), visibleFrame.maxX - Layout.width - Layout.margin)
        let y: CGFloat
        if openDown {
            y = max(visibleFrame.minY + Layout.margin, stripFrame.minY - Layout.gap - Layout.expandedHeight)
        } else {
            y = min(visibleFrame.maxY - Layout.expandedHeight - Layout.margin, stripFrame.maxY + Layout.gap)
        }

        expandedPanel.setFrame(
            NSRect(x: x, y: y, width: Layout.width, height: Layout.expandedHeight),
            display: true
        )
    }
}

private final class DeadlineFloatingPanel: NSPanel {
    var acceptsKeyboardInput = false

    override var canBecomeKey: Bool {
        acceptsKeyboardInput || super.canBecomeKey
    }

    override var canBecomeMain: Bool {
        acceptsKeyboardInput || super.canBecomeMain
    }
}
