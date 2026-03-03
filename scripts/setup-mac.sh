#!/bin/bash
set -euo pipefail

echo "=== Virtual Display Extender — macOS Setup ==="

# Check macOS version
MACOS_VERSION=$(sw_vers -productVersion)
MAJOR=$(echo "$MACOS_VERSION" | cut -d. -f1)
MINOR=$(echo "$MACOS_VERSION" | cut -d. -f2)

if [[ "$MAJOR" -lt 12 ]] || { [[ "$MAJOR" -eq 12 ]] && [[ "$MINOR" -lt 3 ]]; }; then
    echo "ERROR: macOS 12.3+ required (found $MACOS_VERSION)"
    exit 1
fi
echo "✓ macOS $MACOS_VERSION"

# Check Xcode / CLI tools
if ! xcode-select -p &>/dev/null; then
    echo "Installing Xcode Command Line Tools..."
    xcode-select --install
    echo "Re-run this script after installation completes."
    exit 0
fi
echo "✓ Xcode CLI tools installed"

# Check Swift
if ! command -v swift &>/dev/null; then
    echo "ERROR: Swift not found. Install Xcode or Swift toolchain."
    exit 1
fi
SWIFT_VERSION=$(swift --version 2>&1 | head -1)
echo "✓ $SWIFT_VERSION"

# Build
echo ""
echo "Building mac-sender..."
cd "$(dirname "$0")/../mac-sender"
swift build

echo ""
echo "=== Setup complete ==="
echo "Run with: cd mac-sender && swift run VirtualDisplayStreamer"
echo ""
echo "NOTE: You must grant Screen Recording permission in"
echo "  System Settings → Privacy & Security → Screen Recording"
