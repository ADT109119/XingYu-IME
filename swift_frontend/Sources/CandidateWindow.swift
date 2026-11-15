import SwiftUI
import AppKit

struct CandidateView: View {
    let pageCandidates: [CandidateItem]
    let pageSelectedIndex: Int
    let currentPage: Int
    let totalPages: Int
    let isVertical: Bool
    let onSelectOffset: (Int) -> Void
    let onPageStep: (Int) -> Void

    var body: some View {
        Group {
            if isVertical {
                VStack(alignment: .leading, spacing: 4) {
                    ForEach(Array(pageCandidates.enumerated()), id: \.offset) { idx, item in
                        candidatePill(idx: idx, item: item, expandWidth: true)
                    }
                    if totalPages > 1 {
                        Divider().padding(.vertical, 2)
                        HStack {
                            Spacer()
                            paginationControls
                        }
                    }
                }
                .frame(minWidth: 180)
            } else {
                HStack(spacing: 6) {
                    ForEach(Array(pageCandidates.enumerated()), id: \.offset) { idx, item in
                        candidatePill(idx: idx, item: item, expandWidth: false)
                    }
                    if totalPages > 1 {
                        Divider().frame(height: 18)
                        paginationControls
                    }
                }
            }
        }
        .padding(6)
        .background(.ultraThinMaterial)
        .cornerRadius(10)
        .shadow(color: .black.opacity(0.18), radius: 8, x: 0, y: 4)
    }

    @ViewBuilder
    private func candidatePill(idx: Int, item: CandidateItem, expandWidth: Bool) -> some View {
        let isSelected = idx == pageSelectedIndex
        HStack(spacing: 4) {
            Text("\(idx + 1).")
                .font(.system(size: 13, weight: .bold, design: .monospaced))
                .foregroundColor(isSelected ? .white.opacity(0.85) : .secondary)

            Text(item.text)
                .font(.system(size: 15, weight: .medium))
                .foregroundColor(isSelected ? .white : .primary)

            if expandWidth {
                Spacer(minLength: 8)
            }

            if let tag = item.annotation {
                Text(tag)
                    .font(.system(size: 10, weight: .semibold))
                    .padding(.horizontal, 4)
                    .padding(.vertical, 1)
                    .background(tagColor(tag: tag, isSelected: isSelected))
                    .cornerRadius(4)
                    .foregroundColor(.white)
            }
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 5)
        .frame(maxWidth: expandWidth ? .infinity : nil, alignment: .leading)
        .background(isSelected ? Color.accentColor : Color.clear)
        .cornerRadius(6)
        .contentShape(Rectangle())
        .onTapGesture {
            onSelectOffset(idx)
        }
    }

    private var paginationControls: some View {
        HStack(spacing: 4) {
            Button {
                onPageStep(-1)
            } label: {
                Image(systemName: "chevron.left")
                    .font(.system(size: 10, weight: .semibold))
                    .frame(width: 16, height: 18)
            }
            .buttonStyle(.plain)
            .disabled(currentPage <= 0)
            .foregroundColor(currentPage <= 0 ? .secondary.opacity(0.4) : .secondary)

            Text("\(currentPage + 1)/\(totalPages)")
                .font(.system(size: 11, weight: .medium, design: .monospaced))
                .foregroundColor(.secondary)

            Button {
                onPageStep(1)
            } label: {
                Image(systemName: "chevron.right")
                    .font(.system(size: 10, weight: .semibold))
                    .frame(width: 16, height: 18)
            }
            .buttonStyle(.plain)
            .disabled(currentPage + 1 >= totalPages)
            .foregroundColor(currentPage + 1 >= totalPages ? .secondary.opacity(0.4) : .secondary)
        }
        .padding(.horizontal, 4)
    }

    private func tagColor(tag: String, isSelected: Bool) -> Color {
        if isSelected {
            return Color.white.opacity(0.25)
        }
        switch tag {
        case "自訂":
            return Color.orange
        case "英":
            return Color.blue
        default:
            return Color.gray
        }
    }
}

public class CandidatePanel: NSPanel {
    private var hostingView: NSHostingView<CandidateView>?

    public init() {
        super.init(
            contentRect: NSRect(x: 0, y: 0, width: 300, height: 44),
            styleMask: [.nonactivatingPanel, .fullSizeContentView],
            backing: .buffered,
            defer: true
        )
        self.isOpaque = false
        self.backgroundColor = .clear
        self.hasShadow = false
        self.level = .popUpMenu
        self.isFloatingPanel = true
        self.collectionBehavior = [.canJoinAllSpaces, .stationary, .ignoresCycle]
    }

    public func update(
        candidates: [CandidateItem],
        selectedIndex: Int,
        pageSize: Int = 9,
        orientation: String = "horizontal",
        at point: NSPoint,
        onSelect: ((Int) -> Void)? = nil,
        onPageStep: ((Int) -> Void)? = nil
    ) {
        if candidates.isEmpty {
            self.orderOut(nil)
            return
        }

        let safePageSize = max(1, min(9, pageSize))
        let clampedSelected = max(0, min(candidates.count - 1, selectedIndex))
        let currentPage = clampedSelected / safePageSize
        let totalPages = (candidates.count + safePageSize - 1) / safePageSize
        let pageStart = currentPage * safePageSize
        let pageEnd = min(candidates.count, pageStart + safePageSize)
        let slice = Array(candidates[pageStart..<pageEnd])
        let localSelected = clampedSelected - pageStart

        let view = CandidateView(
            pageCandidates: slice,
            pageSelectedIndex: localSelected,
            currentPage: currentPage,
            totalPages: totalPages,
            isVertical: orientation == "vertical",
            onSelectOffset: { offset in
                onSelect?(pageStart + offset)
            },
            onPageStep: { delta in
                onPageStep?(delta)
            }
        )
        if hostingView == nil {
            hostingView = NSHostingView(rootView: view)
            self.contentView = hostingView
        } else {
            hostingView?.rootView = view
        }

        let fittingSize = hostingView?.fittingSize ?? NSSize(width: 200, height: 40)
        var targetX = point.x
        var targetY = point.y - fittingSize.height - 8

        if let screen = NSScreen.main {
            let visibleFrame = screen.visibleFrame
            if targetX + fittingSize.width > visibleFrame.maxX {
                targetX = max(visibleFrame.minX + 10, visibleFrame.maxX - fittingSize.width - 10)
            }
            if targetY < visibleFrame.minY {
                targetY = point.y + 24
            }
        }

        let frame = NSRect(
            x: targetX,
            y: targetY,
            width: fittingSize.width,
            height: fittingSize.height
        )
        self.setFrame(frame, display: true)
        self.orderFront(nil)
    }

    public func hide() {
        self.orderOut(nil)
    }
}
