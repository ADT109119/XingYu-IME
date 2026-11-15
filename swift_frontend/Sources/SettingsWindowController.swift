import Cocoa
import SwiftUI

public class SettingsWindowController: NSWindowController {
    public static let shared = SettingsWindowController()

    private var currentModel: SettingsModel?

    private init() {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 580, height: 500),
            styleMask: [.titled, .closable, .miniaturizable],
            backing: .buffered,
            defer: false
        )
        window.title = "行魚輸入法偏好設定"
        window.center()
        window.isReleasedWhenClosed = false
        super.init(window: window)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    public func show(engine: SmartEngine?, customDictPath: String, tab: SettingsTab = .general) {
        let model = SettingsModel(engine: engine, customDictPath: customDictPath)
        self.currentModel = model
        let view = SettingsView(settingsModel: model, initialTab: tab)
        self.window?.contentView = NSHostingView(rootView: view)

        NSApp.activate(ignoringOtherApps: true)
        self.window?.makeKeyAndOrderFront(nil)
    }
}
