import AppKit
import Testing
@testable import DeadlinePanelNative

struct NativePanelPlacementTests {
    private let screenFrame = NSRect(x: 0, y: 0, width: 1512, height: 982)
    private let panelSize = NSSize(width: 372, height: 44)
    private let margin: CGFloat = 18

    @Test
    func allowsBottomDockSideSpace() {
        let visibleFrame = NSRect(x: 0, y: 70, width: 1512, height: 888)
        let proposed = NSRect(x: 20, y: -40, width: 372, height: 44)

        let result = NativePanelPlacement.clampedCollapsedFrame(
            proposed,
            screenFrame: screenFrame,
            visibleFrame: visibleFrame,
            size: panelSize,
            margin: margin
        )

        #expect(result.minY == screenFrame.minY + margin)
        #expect(result.minY < visibleFrame.minY)
    }

    @Test
    func keepsPanelBelowMenuBar() {
        let visibleFrame = NSRect(x: 0, y: 70, width: 1512, height: 888)
        let proposed = NSRect(x: 400, y: 2000, width: 372, height: 44)

        let result = NativePanelPlacement.clampedCollapsedFrame(
            proposed,
            screenFrame: screenFrame,
            visibleFrame: visibleFrame,
            size: panelSize,
            margin: margin
        )

        #expect(result.maxY == visibleFrame.maxY - margin)
    }

    @Test
    func keepsPanelClearOfSideDock() {
        let visibleFrame = NSRect(x: 80, y: 0, width: 1432, height: 958)
        let proposed = NSRect(x: 0, y: 100, width: 372, height: 44)

        let result = NativePanelPlacement.clampedCollapsedFrame(
            proposed,
            screenFrame: screenFrame,
            visibleFrame: visibleFrame,
            size: panelSize,
            margin: margin
        )

        #expect(result.minX == visibleFrame.minX + margin)
    }
}
