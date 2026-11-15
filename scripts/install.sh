#!/usr/bin/env bash
set -e

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )/.." && pwd )"
cd "$DIR"

TARGET_DIR="$HOME/Library/Input Methods"
APP_NAME="XingYuIME.app"
BUNDLE_ID="com.jerryyen.inputmethod.XingYuIME"

echo "=== 1. Building 行魚輸入法 (XingYuIME) ==="
./scripts/build.sh

echo "=== 2. Installing to $TARGET_DIR ==="
mkdir -p "$TARGET_DIR"
killall XingYuIME 2>/dev/null || true
killall SmartIME 2>/dev/null || true

# 清除舊版並安裝新版
rm -rf "$TARGET_DIR/SmartIME.app"
rm -rf "$TARGET_DIR/$APP_NAME"
cp -R "build/$APP_NAME" "$TARGET_DIR/$APP_NAME"

# 修正權限與清除隔離標記
chmod 755 "$TARGET_DIR"
chmod -R 755 "$TARGET_DIR/$APP_NAME"
xattr -cr "$TARGET_DIR/$APP_NAME" 2>/dev/null || true

# 註冊輸入法至系統並啟用
swift -e '
import Carbon
import Foundation

let path = NSString("'"$TARGET_DIR/$APP_NAME"'").expandingTildeInPath
let url = URL(fileURLWithPath: path)
let regStatus = TISRegisterInputSource(url as CFURL)
print("系統註冊狀態: \(regStatus == 0 ? "成功" : "代碼 \(regStatus)")")

let filter = [kTISPropertyBundleID as String: "'"$BUNDLE_ID"'"] as CFDictionary
if let list = TISCreateInputSourceList(filter, true)?.takeRetainedValue() as? [TISInputSource] {
    for s in list {
        let enableStatus = TISEnableInputSource(s)
        let idPtr = TISGetInputSourceProperty(s, kTISPropertyInputSourceID)
        let id = idPtr != nil ? Unmanaged<CFString>.fromOpaque(idPtr!).takeUnretainedValue() as String : ""
        print("已啟用輸入來源: \(id) (狀態: \(enableStatus))")
    }
}
'

# 同步寫入系統偏好輸入法清單
python3 -c '
import plistlib, os
plist_path = os.path.expanduser("~/Library/Preferences/com.apple.HIToolbox.plist")
if os.path.exists(plist_path):
    with open(plist_path, "rb") as f:
        data = plistlib.load(f)
    sources = data.get("AppleEnabledInputSources", [])
    bundle_id = "'"$BUNDLE_ID"'"
    mode_id = "'"$BUNDLE_ID"'.bopomofo"
    has_mode = any(s.get("Input Mode") == mode_id for s in sources)
    has_method = any(s.get("Bundle ID") == bundle_id and s.get("InputSourceKind") == "Keyboard Input Method" for s in sources)
    if not has_mode:
        sources.append({"Bundle ID": bundle_id, "Input Mode": mode_id, "InputSourceKind": "Input Mode"})
    if not has_method:
        sources.append({"Bundle ID": bundle_id, "InputSourceKind": "Keyboard Input Method"})
    data["AppleEnabledInputSources"] = sources
    with open(plist_path, "wb") as f:
        plistlib.dump(data, f)
' 2>/dev/null || true

# 重新載入系統輸入法選單程序
killall TextInputMenuAgent 2>/dev/null || true
killall TextInputSwitcher 2>/dev/null || true
touch "$TARGET_DIR"

echo "=== 3. 安裝與註冊完成！ ==="
echo "行魚輸入法 (XingYuIME) 已成功安裝至 $TARGET_DIR/$APP_NAME"
echo ""
echo "啟用方式："
echo "1. 請至 macOS「系統設定」->「鍵盤」"
echo "2. 在「文字輸入」區域點擊「編輯 (Edit)」"
echo "3. 點擊「+」新增輸入法，在「繁體中文」清單中即可看到「行魚輸入法」並選取「加入」"
echo "   (若已自動啟用，右上角選單列將可直接切換至行魚輸入法)"
echo ""
echo "偏好設定與自訂辭典："
echo "- 偏好設定：切換至行魚輸入法後，按 Command + , 或點擊輸入法狀態列選單「偏好設定...」"
echo "- 自訂辭典：~/Library/Application Support/XingYuIME/custom_dict.txt"
echo "- 學習資料庫：~/Library/Application Support/XingYuIME/user_learning.db"
