#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BINARY="$PROJECT_DIR/target/release/linux-receiver"

if [ ! -f "$BINARY" ]; then
    echo "Binary not found. Building release..."
    (cd "$PROJECT_DIR" && cargo build --release)
fi

INSTALL_DIR="$HOME/.local/bin"
ICON_DIR="$HOME/.local/share/icons/hicolor/scalable/apps"
DESKTOP_DIR="$HOME/.local/share/applications"

mkdir -p "$INSTALL_DIR" "$ICON_DIR" "$DESKTOP_DIR"

cp "$BINARY" "$INSTALL_DIR/virtual-display-receiver"
chmod +x "$INSTALL_DIR/virtual-display-receiver"

cp "$SCRIPT_DIR/virtual-display-receiver.svg" "$ICON_DIR/virtual-display-receiver.svg"

sed "s|Exec=virtual-display-receiver|Exec=$INSTALL_DIR/virtual-display-receiver|" \
    "$SCRIPT_DIR/virtual-display-receiver.desktop" > "$DESKTOP_DIR/virtual-display-receiver.desktop"

# Update icon cache if possible
gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true

echo "Installed successfully!"
echo "  Binary:  $INSTALL_DIR/virtual-display-receiver"
echo "  Icon:    $ICON_DIR/virtual-display-receiver.svg"
echo "  Desktop: $DESKTOP_DIR/virtual-display-receiver.desktop"
echo ""
echo "You should now see 'Virtual Display Receiver' in your application menu."
