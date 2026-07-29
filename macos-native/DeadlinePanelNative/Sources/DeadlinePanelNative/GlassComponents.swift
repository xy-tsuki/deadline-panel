import SwiftUI

struct GlassPanel<Content: View>: View {
    let cornerRadius: CGFloat
    let material: Material
    let materialOpacity: Double
    let dimming: Double
    let tintOpacity: Double
    let borderOpacity: Double
    let highlightOpacity: Double
    let outerShadowOpacity: Double
    let outerShadowRadius: CGFloat
    @ViewBuilder let content: Content
    @Environment(\.colorScheme) private var colorScheme

    init(
        cornerRadius: CGFloat = 28,
        material: Material = .regularMaterial,
        materialOpacity: Double = 1,
        dimming: Double = 0.05,
        tintOpacity: Double = 0.10,
        borderOpacity: Double = 0.28,
        highlightOpacity: Double = 0.22,
        outerShadowOpacity: Double = 0.16,
        outerShadowRadius: CGFloat = 24,
        @ViewBuilder content: () -> Content
    ) {
        self.cornerRadius = cornerRadius
        self.material = material
        self.materialOpacity = materialOpacity
        self.dimming = dimming
        self.tintOpacity = tintOpacity
        self.borderOpacity = borderOpacity
        self.highlightOpacity = highlightOpacity
        self.outerShadowOpacity = outerShadowOpacity
        self.outerShadowRadius = outerShadowRadius
        self.content = content()
    }

    var body: some View {
        content
            .background(background)
    }

    @ViewBuilder
    private var background: some View {
        let shape = RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
        if #available(macOS 26.0, *) {
            shape
                .fill(.regularMaterial)
                .glassEffect(.regular, in: .rect(cornerRadius: cornerRadius))
                .overlay(tintColor.clipShape(shape))
                .overlay(Color.black.opacity(dimming).clipShape(shape))
                .overlay(shape.stroke(borderColor, lineWidth: 1))
                .overlay(alignment: .top) {
                    topHighlight(shape: shape)
                }
                .shadow(color: outerShadowColor, radius: outerShadowRadius, x: 0, y: outerShadowRadius * 0.5)
        } else {
            shape
                .fill(material)
                .opacity(materialOpacity)
                .overlay(tintColor.clipShape(shape))
                .overlay(Color.black.opacity(dimming).clipShape(shape))
                .overlay(shape.stroke(borderColor, lineWidth: 1))
                .overlay(alignment: .top) {
                    topHighlight(shape: shape)
                }
                .shadow(color: outerShadowColor, radius: outerShadowRadius, x: 0, y: outerShadowRadius * 0.5)
        }
    }

    private func topHighlight(shape: RoundedRectangle) -> some View {
        shape
            .fill(
                LinearGradient(
                    colors: [
                        Color.white.opacity(highlightOpacity),
                        Color.white.opacity(max(highlightOpacity * 0.18, 0.03)),
                        Color.clear
                    ],
                    startPoint: .top,
                    endPoint: .bottom
                )
            )
            .frame(height: min(40, max(24, cornerRadius * 1.45)))
            .blur(radius: 8)
            .clipShape(shape)
    }

    private var tintColor: Color {
        Color.white.opacity(colorScheme == .dark ? max(min(tintOpacity, 0.10), 0.08) : max(tintOpacity, 0.14))
    }

    private var borderColor: Color {
        Color.white.opacity(colorScheme == .dark ? max(min(borderOpacity, 0.24), 0.20) : max(borderOpacity, 0.34))
    }

    private var outerShadowColor: Color {
        Color.black.opacity(colorScheme == .dark ? max(outerShadowOpacity, 0.28) : outerShadowOpacity)
    }
}

struct GlassListContainer<Content: View>: View {
    @ViewBuilder let content: Content

    var body: some View {
        content
            .background {
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .fill(.ultraThinMaterial)
                    .overlay(Color.black.opacity(0.04).clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous)))
                    .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).stroke(.white.opacity(0.14), lineWidth: 1))
            }
    }
}

struct GlassTag: View {
    let text: String
    let color: Color
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        Text(text)
            .font(.caption.weight(.semibold))
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .foregroundStyle(color)
            .background {
                Capsule()
                    .fill(color.opacity(0.16))
                    .overlay(Capsule().stroke(color.opacity(tagBorderOpacity), lineWidth: 1))
                    .shadow(color: tagShadowColor, radius: 4, x: 1, y: 2)
            }
    }

    private var tagShadowColor: Color {
        Color.black.opacity(colorScheme == .dark ? 0.22 : 0.16)
    }

    private var tagBorderOpacity: Double {
        colorScheme == .dark ? 0.28 : 0.22
    }
}

struct LiquidSegmentedControl: View {
    @Binding var selection: Int
    let values: [Int]
    @Namespace private var namespace
    @State private var hoveredValue: Int?
    @State private var pressedValue: Int?
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.nativeLowPowerMode) private var lowPowerMode

    var body: some View {
        HStack(spacing: 2) {
            ForEach(values, id: \.self) { value in
                Button {
                    if selection != value {
                        withAnimation(lowPowerMode ? nil : .spring(response: 0.28, dampingFraction: 0.72)) {
                            selection = value
                        }
                    }
                } label: {
                    Text("Top \(value)")
                        .font(.system(size: 12, weight: selection == value ? .semibold : .medium))
                        .lineLimit(1)
                        .frame(width: 48, height: 28)
                        .foregroundStyle(segmentTextColor(isSelected: selection == value))
                        .background {
                            if selection == value {
                                LiquidCapsuleBackground(isHovered: true, dimming: 0.04)
                                    .matchedGeometryEffect(id: "selected-segment", in: namespace)
                            } else if !lowPowerMode, hoveredValue == value {
                                Capsule()
                                    .fill(.white.opacity(0.07))
                            }
                        }
                }
                .buttonStyle(.plain)
                .scaleEffect(lowPowerMode ? 1 : (pressedValue == value ? 1.10 : hoveredValue == value ? 1.04 : 1))
                .animation(lowPowerMode ? nil : .spring(response: 0.22, dampingFraction: 0.68), value: pressedValue)
                .animation(lowPowerMode ? nil : .spring(response: 0.24, dampingFraction: 0.74), value: hoveredValue)
                .onHover { hovering in
                    guard !lowPowerMode else {
                        hoveredValue = nil
                        return
                    }
                    hoveredValue = hovering ? value : (hoveredValue == value ? nil : hoveredValue)
                }
            }
        }
        .padding(4)
        .background {
            LiquidCapsuleBackground(
                isHovered: !lowPowerMode && (hoveredValue != nil || pressedValue != nil),
                dimming: 0.06
            )
        }
        .simultaneousGesture(
            DragGesture(minimumDistance: 0)
                .onChanged { drag in
                    let nextValue = value(at: drag.location.x)
                    if !lowPowerMode {
                        pressedValue = nextValue
                    }
                    if selection != nextValue {
                        withAnimation(lowPowerMode ? nil : .spring(response: 0.24, dampingFraction: 0.7)) {
                            selection = nextValue
                        }
                    }
                }
                .onEnded { _ in
                    pressedValue = nil
                }
        )
    }

    private func value(at xPosition: CGFloat) -> Int {
        let segmentWidth: CGFloat = 50
        let clampedX = min(max(xPosition - 4, 0), segmentWidth * CGFloat(values.count) - 1)
        let index = min(max(Int(clampedX / segmentWidth), 0), values.count - 1)
        return values[index]
    }

    private func segmentTextColor(isSelected: Bool) -> Color {
        if colorScheme == .light {
            return Color.black.opacity(isSelected ? 0.74 : 0.50)
        }
        return isSelected ? Color.primary : Color.secondary
    }
}

struct LiquidTextSegmentedControl: View {
    struct Option: Identifiable {
        let id: String
        let title: String
    }

    @Binding var selection: String
    let options: [Option]
    @Namespace private var namespace
    @State private var hoveredID: String?
    @State private var pressedID: String?
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.nativeLowPowerMode) private var lowPowerMode

    var body: some View {
        HStack(spacing: 2) {
            ForEach(options) { option in
                Button {
                    if selection != option.id {
                        withAnimation(lowPowerMode ? nil : .spring(response: 0.28, dampingFraction: 0.72)) {
                            selection = option.id
                        }
                    }
                } label: {
                    Text(option.title)
                        .font(.system(size: 12, weight: selection == option.id ? .semibold : .medium))
                        .lineLimit(1)
                        .frame(minWidth: 54, minHeight: 28)
                        .padding(.horizontal, 2)
                        .foregroundStyle(segmentTextColor(isSelected: selection == option.id))
                        .background {
                            if selection == option.id {
                                LiquidCapsuleBackground(isHovered: true, dimming: 0.04)
                                    .matchedGeometryEffect(id: "selected-text-segment", in: namespace)
                            } else if !lowPowerMode, hoveredID == option.id {
                                Capsule()
                                    .fill(.white.opacity(0.07))
                            }
                        }
                }
                .buttonStyle(.plain)
                .scaleEffect(lowPowerMode ? 1 : (pressedID == option.id ? 1.10 : hoveredID == option.id ? 1.04 : 1))
                .animation(lowPowerMode ? nil : .spring(response: 0.22, dampingFraction: 0.68), value: pressedID)
                .animation(lowPowerMode ? nil : .spring(response: 0.24, dampingFraction: 0.74), value: hoveredID)
                .onHover { hovering in
                    guard !lowPowerMode else {
                        hoveredID = nil
                        return
                    }
                    hoveredID = hovering ? option.id : (hoveredID == option.id ? nil : hoveredID)
                }
            }
        }
        .padding(4)
        .background {
            LiquidCapsuleBackground(
                isHovered: !lowPowerMode && (hoveredID != nil || pressedID != nil),
                dimming: 0.06
            )
        }
        .simultaneousGesture(
            DragGesture(minimumDistance: 0)
                .onChanged { drag in
                    let nextID = optionID(at: drag.location.x)
                    if !lowPowerMode {
                        pressedID = nextID
                    }
                    if selection != nextID {
                        withAnimation(lowPowerMode ? nil : .spring(response: 0.24, dampingFraction: 0.7)) {
                            selection = nextID
                        }
                    }
                }
                .onEnded { _ in pressedID = nil }
        )
    }

    private func optionID(at xPosition: CGFloat) -> String {
        guard !options.isEmpty else {
            return selection
        }
        let segmentWidth: CGFloat = 64
        let clampedX = min(max(xPosition - 4, 0), segmentWidth * CGFloat(options.count) - 1)
        let index = min(max(Int(clampedX / segmentWidth), 0), options.count - 1)
        return options[index].id
    }

    private func segmentTextColor(isSelected: Bool) -> Color {
        if colorScheme == .light {
            return Color.black.opacity(isSelected ? 0.74 : 0.50)
        }
        return isSelected ? Color.primary : Color.secondary
    }
}

struct LiquidToolButton: View {
    let title: String
    let systemImage: String
    var badge: String?
    var isDisabled = false
    var fillsAvailableWidth = false
    let action: () -> Void

    @State private var hovering = false
    @State private var pressing = false
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.nativeLowPowerMode) private var lowPowerMode

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                Image(systemName: systemImage)
                    .font(.system(size: 13, weight: .semibold))
                Text(title)
                    .font(.system(size: 13, weight: .semibold))
                    .lineLimit(1)
                if let badge {
                    Text(badge)
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 3)
                        .background(.white.opacity(0.08), in: Capsule())
                }
            }
            .padding(.horizontal, 12)
            .frame(maxWidth: fillsAvailableWidth ? .infinity : nil)
            .frame(height: 34)
            .foregroundStyle(buttonTextColor)
            .background {
                LiquidCapsuleBackground(isHovered: !lowPowerMode && hovering, dimming: 0.06)
            }
        }
        .buttonStyle(.plain)
        .disabled(isDisabled)
        .scaleEffect(lowPowerMode ? 1 : (pressing ? 1.06 : hovering ? 1.025 : 1))
        .opacity(isDisabled ? 0.55 : 1)
        .animation(lowPowerMode ? nil : .spring(response: 0.22, dampingFraction: 0.7), value: hovering)
        .animation(lowPowerMode ? nil : .spring(response: 0.18, dampingFraction: 0.65), value: pressing)
        .onHover { hovering = lowPowerMode ? false : $0 }
        .simultaneousGesture(
            DragGesture(minimumDistance: 0)
                .onChanged { _ in pressing = !lowPowerMode }
                .onEnded { _ in pressing = false }
        )
    }

    private var buttonTextColor: Color {
        if isDisabled {
            return .secondary
        }
        return colorScheme == .light ? Color.black.opacity(0.76) : Color.primary
    }
}

struct LiquidIconButton: View {
    let systemImage: String
    var tint: Color = .primary
    var isDisabled = false
    let action: () -> Void

    @State private var hovering = false
    @State private var pressing = false
    @Environment(\.nativeLowPowerMode) private var lowPowerMode

    var body: some View {
        Button(action: action) {
            Image(systemName: systemImage)
                .font(.system(size: 13, weight: .semibold))
                .frame(width: 30, height: 30)
                .foregroundStyle(tint)
                .background {
                    LiquidCircleBackground(isHovered: !lowPowerMode && hovering, dimming: 0.05)
                }
        }
        .buttonStyle(.plain)
        .disabled(isDisabled)
        .opacity(isDisabled ? 0.45 : 1)
        .scaleEffect(lowPowerMode ? 1 : (pressing ? 1.12 : hovering ? 1.05 : 1))
        .animation(lowPowerMode ? nil : .spring(response: 0.2, dampingFraction: 0.66), value: hovering)
        .animation(lowPowerMode ? nil : .spring(response: 0.16, dampingFraction: 0.62), value: pressing)
        .onHover { hovering = lowPowerMode ? false : $0 }
        .simultaneousGesture(
            DragGesture(minimumDistance: 0)
                .onChanged { _ in pressing = !lowPowerMode }
                .onEnded { _ in pressing = false }
        )
    }
}

private struct LiquidCapsuleBackground: View {
    var isHovered: Bool
    var dimming: Double
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let shape = Capsule()
        if #available(macOS 26.0, *) {
            shape
                .fill(.regularMaterial)
                .glassEffect(.regular, in: .rect(cornerRadius: 28))
                .overlay(tintColor.clipShape(shape))
                .overlay(Color.black.opacity(adjustedDimming).clipShape(shape))
                .overlay(shape.stroke(borderColor, lineWidth: 1))
                .overlay(alignment: .top) {
                    capsuleHighlight(shape: shape)
                }
                .shadow(color: outerShadowColor, radius: 4, x: 1, y: 1)
                .shadow(color: innerGlowColor, radius: 3, x: 1, y: 1)
        } else {
            shape
                .fill(.regularMaterial)
                .overlay(tintColor.clipShape(shape))
                .overlay(Color.black.opacity(adjustedDimming).clipShape(shape))
                .overlay(shape.stroke(borderColor, lineWidth: 1))
                .overlay(alignment: .top) {
                    capsuleHighlight(shape: shape)
                }
                .shadow(color: outerShadowColor, radius: 4, x: 1, y: 1)
                .shadow(color: innerGlowColor, radius: 3, x: 1, y: 1)
        }
    }

    private func capsuleHighlight(shape: Capsule) -> some View {
        shape
            .fill(
                LinearGradient(
                    colors: [
                        Color.white.opacity(highlightOpacity),
                        Color.white.opacity(colorScheme == .dark ? 0.06 : 0.10),
                        Color.clear
                    ],
                    startPoint: .top,
                    endPoint: .bottom
                )
            )
            .frame(height: 24)
            .blur(radius: 8)
            .clipShape(shape)
    }

    private var tintColor: Color {
        Color.white.opacity(colorScheme == .dark ? 0.10 : 0.16)
    }

    private var adjustedDimming: Double {
        colorScheme == .dark ? dimming * 0.58 : dimming
    }

    private var borderColor: Color {
        let base = colorScheme == .dark ? 0.24 : 0.36
        return Color.white.opacity(isHovered ? min(base + 0.12, 0.52) : base)
    }

    private var highlightOpacity: Double {
        let base = colorScheme == .dark ? 0.20 : 0.34
        return isHovered ? min(base + 0.10, 0.48) : base
    }

    private var outerShadowColor: Color {
        Color.black.opacity(colorScheme == .dark ? 0.28 : 0.16)
    }

    private var innerGlowColor: Color {
        Color.white.opacity(colorScheme == .dark ? 0.04 : 0.10)
    }
}

private struct LiquidCircleBackground: View {
    var isHovered: Bool
    var dimming: Double
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        let shape = Circle()
        if #available(macOS 26.0, *) {
            shape
                .fill(.regularMaterial)
                .glassEffect(.regular, in: .rect(cornerRadius: 15))
                .overlay(tintColor.clipShape(shape))
                .overlay(Color.black.opacity(adjustedDimming).clipShape(shape))
                .overlay(shape.stroke(borderColor, lineWidth: 1))
                .overlay(alignment: .top) {
                    circleHighlight(shape: shape)
                }
                .shadow(color: outerShadowColor, radius: 4, x: 1, y: 1)
                .shadow(color: innerGlowColor, radius: 3, x: 1, y: 1)
        } else {
            shape
                .fill(.regularMaterial)
                .overlay(tintColor.clipShape(shape))
                .overlay(Color.black.opacity(adjustedDimming).clipShape(shape))
                .overlay(shape.stroke(borderColor, lineWidth: 1))
                .overlay(alignment: .top) {
                    circleHighlight(shape: shape)
                }
                .shadow(color: outerShadowColor, radius: 4, x: 1, y: 1)
                .shadow(color: innerGlowColor, radius: 3, x: 1, y: 1)
        }
    }

    private func circleHighlight(shape: Circle) -> some View {
        shape
            .fill(
                LinearGradient(
                    colors: [
                        Color.white.opacity(highlightOpacity),
                        Color.white.opacity(colorScheme == .dark ? 0.06 : 0.10),
                        Color.clear
                    ],
                    startPoint: .top,
                    endPoint: .bottom
                )
            )
            .frame(height: 20)
            .blur(radius: 7)
            .clipShape(shape)
    }

    private var tintColor: Color {
        Color.white.opacity(colorScheme == .dark ? 0.10 : 0.16)
    }

    private var adjustedDimming: Double {
        colorScheme == .dark ? dimming * 0.58 : dimming
    }

    private var borderColor: Color {
        let base = colorScheme == .dark ? 0.24 : 0.36
        return Color.white.opacity(isHovered ? min(base + 0.12, 0.52) : base)
    }

    private var highlightOpacity: Double {
        let base = colorScheme == .dark ? 0.20 : 0.34
        return isHovered ? min(base + 0.10, 0.48) : base
    }

    private var outerShadowColor: Color {
        Color.black.opacity(colorScheme == .dark ? 0.28 : 0.16)
    }

    private var innerGlowColor: Color {
        Color.white.opacity(colorScheme == .dark ? 0.04 : 0.10)
    }
}
