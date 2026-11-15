import Cocoa
import SwiftUI

public class SettingsWindowController: NSWindowController {
    public static let shared = SettingsWindowController()

    private var currentModel: SettingsModel?

    private init() {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 540, height: 440),
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

    public func show(engine: SmartEngine?, customDictPath: String) {
        let model = SettingsModel(engine: engine, customDictPath: customDictPath)
        self.currentModel = model
        let view = SettingsView(settingsModel: model)
        self.window?.contentView = NSHostingView(rootView: view)

        NSApp.activate(ignoringOtherApps: true)
        self.window?.makeKeyAndOrderFront(nil)
    }
}
