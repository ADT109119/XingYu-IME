#!/usr/bin/env bash
set -e

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )/.." && pwd )"
cd "$DIR"

echo "=== 1. Building XingYu Rust Core Engine ==="
export PATH="$HOME/.cargo/bin:$PATH"
cd rust_engine
cargo build --release
cd ..

echo "=== 2. Creating 行魚輸入法 (XingYuIME.app) Bundle ==="
APP_BUNDLE="build/XingYuIME.app"
CONTENTS_DIR="$APP_BUNDLE/Contents"
MACOS_DIR="$CONTENTS_DIR/MacOS"
RESOURCES_DIR="$CONTENTS_DIR/Resources"

rm -rf "$APP_BUNDLE"
mkdir -p "$MACOS_DIR"
mkdir -p "$RESOURCES_DIR"

cp swift_frontend/Resources/Info.plist "$CONTENTS_DIR/Info.plist"
echo -n "APPL????" > "$CONTENTS_DIR/PkgInfo"
cp -R swift_frontend/Resources/*.lproj "$RESOURCES_DIR/"
cp swift_frontend/Resources/XingYuTemplate.tiff "$RESOURCES_DIR/"
cp swift_frontend/Resources/AppIcon.icns "$RESOURCES_DIR/"

echo "=== 3. Compiling Swift App with Rust Engine ==="
swiftc \
    -O \
    -import-objc-header swift_frontend/Sources/Bridging-Header.h \
    -I rust_engine/include \
    -L rust_engine/target/release \
    -lxingyu_engine \
    -framework Cocoa \
    -framework InputMethodKit \
    -framework SwiftUI \
    swift_frontend/Sources/main.swift \
    swift_frontend/Sources/AppDelegate.swift \
    swift_frontend/Sources/EngineBridge.swift \
    swift_frontend/Sources/CandidateWindow.swift \
    swift_frontend/Sources/InputController.swift \
    swift_frontend/Sources/SettingsView.swift \
    swift_frontend/Sources/SettingsModel.swift \
    swift_frontend/Sources/SettingsWindowController.swift \
    -o "$MACOS_DIR/XingYuIME"

echo "=== 4. Code Signing Bundle (Ad-hoc) ==="
codesign -s - --deep --force "$APP_BUNDLE"

echo "=== Build Complete! Bundle located at: $DIR/$APP_BUNDLE ==="
