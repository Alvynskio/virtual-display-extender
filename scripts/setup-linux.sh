#!/bin/bash
set -euo pipefail

echo "=== Virtual Display Extender — Linux Setup ==="

# Check OS
if [[ ! -f /etc/os-release ]]; then
    echo "WARNING: Cannot detect OS. Proceeding anyway."
else
    . /etc/os-release
    echo "✓ $PRETTY_NAME"
fi

# Install GStreamer
echo ""
echo "Installing GStreamer and plugins..."
if command -v apt-get &>/dev/null; then
    sudo apt-get update
    sudo apt-get install -y \
        libgstreamer1.0-dev \
        libgstreamer-plugins-base1.0-dev \
        gstreamer1.0-plugins-base \
        gstreamer1.0-plugins-good \
        gstreamer1.0-plugins-ugly \
        gstreamer1.0-plugins-bad \
        gstreamer1.0-libav \
        gstreamer1.0-tools \
        libglib2.0-dev \
        pkg-config
elif command -v dnf &>/dev/null; then
    sudo dnf install -y \
        gstreamer1-devel \
        gstreamer1-plugins-base-devel \
        gstreamer1-plugins-good \
        gstreamer1-plugins-ugly \
        gstreamer1-plugins-bad-free \
        glib2-devel \
        pkg-config
else
    echo "ERROR: Unsupported package manager. Install GStreamer manually."
    exit 1
fi
echo "✓ GStreamer installed"

# Check Rust
if ! command -v cargo &>/dev/null; then
    echo ""
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi
RUST_VERSION=$(rustc --version)
echo "✓ $RUST_VERSION"

# Build
echo ""
echo "Building linux-receiver..."
cd "$(dirname "$0")/../linux-receiver"
cargo build --release

echo ""
echo "=== Setup complete ==="
echo "Run with: cd linux-receiver && cargo run --release"
