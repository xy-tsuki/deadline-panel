import SwiftUI

enum PanelCommand {
    case show
    case hide
    case hideMinutes(TimeInterval)
    case resetPosition
    case settings
    case restart
    case quit
}

struct CollapsedStripView: View {
    @ObservedObject var viewModel: DeadlineViewModel
    @State private var isDragging = false
    @AppStorage(NativeAppearance.defaultsKey) private var appearanceMode = "system"
    let onHover: (Bool) -> Void
    let onDragStart: () -> Void
    let onDragChange: () -> Void
    let onDragEnd: () -> Void
    let onCommand: (PanelCommand) -> Void

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "clock")
                .foregroundStyle(.blue)

            Text(title)
                .font(.system(size: 14, weight: .medium))
                .lineLimit(1)
                .foregroundStyle(.primary)

            Spacer()

            Image(systemName: "chevron.up")
                .foregroundStyle(.blue)
        }
        .padding(.horizontal, 18)
        .frame(width: 372, height: 44)
        .background {
            CollapsedStripBackground()
        }
        .onHover(perform: onHover)
        .gesture(
            DragGesture(minimumDistance: 1)
                .onChanged { _ in
                    if !isDragging {
                        isDragging = true
                        onDragStart()
                    }
                    onDragChange()
                }
                .onEnded { _ in
                    isDragging = false
                    onDragEnd()
                }
        )
        .transaction { transaction in
            if isDragging {
                transaction.animation = nil
            }
        }
        .contextMenu {
            PanelContextMenu(onCommand: onCommand)
        }
        .nativePreferredColorScheme(appearanceMode)
    }

    private var title: String {
        let strings = NativeStrings.current
        if let task = viewModel.currentDeadlines.first {
            let titles = viewModel.currentDeadlines.map(\.title).joined(separator: " + ")
            return "\(strings.stripCurrentPrefix) | \(titles)（\(relativeStripDueText(task.dueAt))）"
        }
        guard let task = viewModel.focusDeadlines.first else {
            return strings.noDeadlines
        }
        return "\(strings.stripTopPrefix(viewModel.focusLimit)) | \(task.title)（\(relativeStripDueText(task.dueAt))）"
    }
}

private struct CollapsedStripBackground: View {
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: 14, style: .continuous)
        if #available(macOS 26.0, *) {
            shape
                .fill(tintColor)
                .glassEffect(.regular, in: .rect(cornerRadius: 14))
                .overlay(Color.black.opacity(0.06).clipShape(shape))
                .overlay(shape.stroke(borderColor, lineWidth: 1))
                .overlay(alignment: .top) {
                    topHighlight(shape: shape)
                }
        } else {
            shape
                .fill(.thinMaterial)
                .overlay(stripOverlayColor.clipShape(shape))
                .overlay(shape.stroke(borderColor, lineWidth: 1))
                .overlay(alignment: .top) {
                    topHighlight(shape: shape)
                }
        }
    }

    private func topHighlight(shape: RoundedRectangle) -> some View {
        shape
            .fill(
                LinearGradient(
                    colors: [
                        Color.white.opacity(colorScheme == .dark ? 0.06 : 0.15),
                        Color.clear
                    ],
                    startPoint: .top,
                    endPoint: .bottom
                )
            )
            .frame(height: 24)
            .clipShape(shape)
    }

    private var stripOverlayColor: Color {
        colorScheme == .dark ? Color.black.opacity(0.08) : Color.white.opacity(0.04)
    }

    private var borderColor: Color {
        Color.white.opacity(colorScheme == .dark ? 0.12 : 0.20)
    }
    
    private var tintColor: Color {
        colorScheme == .dark ? Color.black.opacity(0.15) : Color.white.opacity(0.20)
    }
}

private func relativeStripDueText(_ dueAt: String) -> String {
    let strings = NativeStrings.current
    guard let date = ISO8601DateFormatter.deadlinePanelDate(from: dueAt) else {
        return strings.unknownTime
    }
    let diff = date.timeIntervalSinceNow
    let absDiff = abs(diff)
    let hour: TimeInterval = 60 * 60
    let day: TimeInterval = 24 * hour
    if diff < 0 {
        if absDiff < hour { return strings.overdue }
        if absDiff < day { return strings.overdueHours(Int(ceil(absDiff / hour))) }
        return strings.overdueDays(Int(ceil(absDiff / day)))
    }
    if diff < hour { return strings.withinHour }
    if diff < day { return strings.hours(Int(ceil(diff / hour))) }
    return strings.days(Int(ceil(diff / day)))
}

struct ExpandedPanelView: View {
    @ObservedObject var viewModel: DeadlineViewModel
    @ObservedObject var cloudSyncController: NativeCloudSyncController
    @AppStorage(NativeAppearance.defaultsKey) private var appearanceMode = "system"
    let onHover: (Bool) -> Void
    let onControlInteractionChanged: (Bool) -> Void
    let onCommand: (PanelCommand) -> Void
    let onImportJSON: (() -> Void)?
    let onExportJSON: (() -> Void)?

    var body: some View {
        ContentView(
            viewModel: viewModel,
            cloudSyncController: cloudSyncController,
            onHideTemporarily: { onCommand(.hide) },
            onImportJSON: onImportJSON,
            onExportJSON: onExportJSON,
            onOpenSettings: { onCommand(.settings) },
            onControlInteractionChanged: onControlInteractionChanged
        )
            .frame(width: 372, height: 600)
            .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
            .nativePreferredColorScheme(appearanceMode)
            .onHover(perform: onHover)
            .contextMenu {
                PanelContextMenu(onCommand: onCommand)
            }
    }
}

struct PanelContextMenu: View {
    let onCommand: (PanelCommand) -> Void

    var body: some View {
        let strings = NativeStrings.current
        Button(strings.menuShowPanel) {
            onCommand(.show)
        }
        Button(strings.menuHidePanel) {
            onCommand(.hide)
        }
        Button(strings.hide15) {
            onCommand(.hideMinutes(15))
        }
        Button(strings.hide30) {
            onCommand(.hideMinutes(30))
        }
        Button(strings.menuHide60) {
            onCommand(.hideMinutes(60))
        }
        Divider()
        Button(strings.menuResetPosition) {
            onCommand(.resetPosition)
        }
        Divider()
        Button(strings.menuRestart) {
            onCommand(.restart)
        }
        Button(strings.menuQuit) {
            onCommand(.quit)
        }
    }
}
