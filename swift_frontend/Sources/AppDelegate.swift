import Cocoa
import InputMethodKit

class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSLog("行魚輸入法 (XingYuIME) AppDelegate: applicationDidFinishLaunching")
    }

    func applicationWillTerminate(_ notification: Notification) {
        NSLog("行魚輸入法 (XingYuIME) AppDelegate: applicationWillTerminate")
    }
}
