#!/usr/bin/env bash
set -e

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )/.." && pwd )"
cd "$DIR"

UNIVERSAL="${UNIVERSAL:-0}"
for arg in "$@"; do
    case "$arg" in
        --universal) UNIVERSAL=1 ;;
        *)
            echo "Unknown option: $arg" >&2
            exit 1
            ;;
    esac
done

export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-13.0}"
export PATH="$HOME/.cargo/bin:$PATH"

HOST_ARCH="$(uname -m)"
if [ "$UNIVERSAL" = "1" ]; then
    ARCHS=("arm64" "x86_64")
else
    ARCHS=("$HOST_ARCH")
fi

rust_target_for_arch() {
    case "$1" in
        arm64) echo "aarch64-apple-darwin" ;;
        x86_64) echo "x86_64-apple-darwin" ;;
        *)
            echo "Unsupported architecture: $1" >&2
            exit 1
            ;;
    esac
}

echo "=== 1. Building XingYu Rust Core Engine (${ARCHS[*]}, min macOS $MACOSX_DEPLOYMENT_TARGET) ==="
for arch in "${ARCHS[@]}"; do
    target="$(rust_target_for_arch "$arch")"
    echo "-> cargo build --release --target $target"
    cargo build --manifest-path rust_engine/Cargo.toml --release --target "$target"
done

echo "=== 2. Creating 行魚輸入法 (XingYuIME.app) Bundle ==="
APP_BUNDLE="build/XingYuIME.app"
CONTENTS_DIR="$APP_BUNDLE/Contents"
MACOS_DIR="$CONTENTS_DIR/MacOS"
RESOURCES_DIR="$CONTENTS_DIR/Resources"

rm -rf "$APP_BUNDLE"
mkdir -p "$MACOS_DIR"
mkdir -p "$RESOURCES_DIR"

cp swift_frontend/Resources/Info.plist "$CONTENTS_DIR/Info.plist"
if [ -n "${APP_VERSION:-}" ]; then
    /usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $APP_VERSION" "$CONTENTS_DIR/Info.plist"
fi
echo -n "APPL????" > "$CONTENTS_DIR/PkgInfo"
cp -R swift_frontend/Resources/*.lproj "$RESOURCES_DIR/"
cp swift_frontend/Resources/XingYuTemplate.tiff "$RESOURCES_DIR/"
cp swift_frontend/Resources/AppIcon.icns "$RESOURCES_DIR/"

echo "=== 3. Compiling Swift App with Rust Engine ==="
SWIFT_SOURCES=(
    swift_frontend/Sources/main.swift
    swift_frontend/Sources/AppDelegate.swift
    swift_frontend/Sources/EngineBridge.swift
    swift_frontend/Sources/CandidateWindow.swift
    swift_frontend/Sources/InputController.swift
    swift_frontend/Sources/SettingsView.swift
    swift_frontend/Sources/SettingsModel.swift
    swift_frontend/Sources/SettingsWindowController.swift
)

THIN_BINARIES=()
for arch in "${ARCHS[@]}"; do
    target="$(rust_target_for_arch "$arch")"
    out_bin="build/XingYuIME-${arch}"
    echo "-> swiftc -target ${arch}-apple-macos${MACOSX_DEPLOYMENT_TARGET}"
    swiftc \
        -O \
        -target "${arch}-apple-macos${MACOSX_DEPLOYMENT_TARGET}" \
        -import-objc-header swift_frontend/Sources/Bridging-Header.h \
        -I rust_engine/include \
        -L "rust_engine/target/${target}/release" \
        -lxingyu_engine \
        -framework Cocoa \
        -framework InputMethodKit \
        -framework SwiftUI \
        "${SWIFT_SOURCES[@]}" \
        -o "$out_bin"
    THIN_BINARIES+=("$out_bin")
done

if [ "${#THIN_BINARIES[@]}" -gt 1 ]; then
    echo "-> Creating Universal Binary with lipo"
    lipo -create "${THIN_BINARIES[@]}" -output "$MACOS_DIR/XingYuIME"
    rm -f "${THIN_BINARIES[@]}"
else
    mv "${THIN_BINARIES[0]}" "$MACOS_DIR/XingYuIME"
fi

echo "=== 4. Code Signing Bundle (Ad-hoc) ==="
codesign -s - --deep --force "$APP_BUNDLE"

echo "=== Build Complete! Bundle located at: $DIR/$APP_BUNDLE ==="
lipo -info "$MACOS_DIR/XingYuIME"
