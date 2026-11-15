#!/usr/bin/env bash
set -euo pipefail

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )/.." && pwd )"
cd "$DIR"

UNIVERSAL_FLAG=1
ARCH_LABEL="universal"

for arg in "$@"; do
    case "$arg" in
        --host-arch)
            UNIVERSAL_FLAG=0
            ARCH_LABEL="$(uname -m)"
            ;;
        --universal)
            UNIVERSAL_FLAG=1
            ARCH_LABEL="universal"
            ;;
        *)
            echo "Unknown option: $arg" >&2
            echo "Usage: $0 [--universal | --host-arch]" >&2
            exit 1
            ;;
    esac
done

UNIVERSAL="$UNIVERSAL_FLAG" ./scripts/build.sh

APP_BUNDLE="$DIR/build/XingYuIME.app"
VERSION="${APP_VERSION:-$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$APP_BUNDLE/Contents/Info.plist")}"
ARCHIVE_NAME="XingYuIME-${VERSION}-${ARCH_LABEL}.zip"
ARCHIVE_PATH="$DIR/build/${ARCHIVE_NAME}"
SHA_PATH="${ARCHIVE_PATH}.sha256"

echo "=== Verifying Code Signature ==="
codesign --verify --deep --strict --verbose=2 "$APP_BUNDLE"

echo "=== Packaging $ARCHIVE_NAME ==="
rm -f "$ARCHIVE_PATH" "$SHA_PATH"
(
    cd "$DIR/build"
    ditto -c -k --sequesterRsrc --keepParent "XingYuIME.app" "$ARCHIVE_PATH"
    shasum -a 256 "$ARCHIVE_NAME" > "$SHA_PATH"
)

echo "=== Package Complete ==="
echo "Archive: $ARCHIVE_PATH"
echo "SHA-256: $(cat "$SHA_PATH")"
