import Cocoa
import InputMethodKit

// 全域保留 IMKServer 實例避免被 ARC 提前釋放
var server: IMKServer?

autoreleasepool {
    let app = NSApplication.shared
    let delegate = AppDelegate()
    app.delegate = delegate

    guard let bundleId = Bundle.main.bundleIdentifier else {
        fatalError("行魚輸入法 (XingYuIME): 無法取得 Bundle Identifier")
    }

    let connectionName = (Bundle.main.infoDictionary?["InputMethodConnectionName"] as? String) ?? "XingYuIME_1_Connection"
    NSLog("行魚輸入法 (XingYuIME) 啟動中... ConnectionName=\(connectionName), BundleId=\(bundleId)")

    server = IMKServer(name: connectionName, bundleIdentifier: bundleId)
    if server == nil {
        NSLog("行魚輸入法 (XingYuIME) 嚴重錯誤: 無法初始化 IMKServer (Connection=\(connectionName))")
    } else {
        NSLog("行魚輸入法 (XingYuIME) IMKServer 成功註冊並就緒！")
    }

    app.run()
}
