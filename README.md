# Virtual Display Extender

Stream a virtual second display from a MacBook (macOS) to an iMac (Linux/Ubuntu) over the local network, functioning as a true extended desktop.

## Architecture

- **mac-sender** — macOS Swift menu bar app that creates a virtual display via `CGVirtualDisplay`, captures it with `ScreenCaptureKit`, encodes H.264 via `VideoToolbox`, and streams RTP/UDP.
- **linux-receiver** — Rust app using GStreamer to receive, decode, and render the stream fullscreen.

## Requirements

### macOS (sender)
- macOS 12.3+ (ScreenCaptureKit requirement)
- Apple Silicon or Intel Mac
- Screen Recording permission granted in System Settings → Privacy & Security
- Xcode 14+ / Swift 5.7+

### Linux (receiver)
- Ubuntu 22.04+ (or equivalent)
- Rust 1.70+
- GStreamer 1.20+ with good/ugly/libav plugins
- Both machines on the same local network

## Building

### macOS sender
```bash
cd mac-sender
swift build
swift run VirtualDisplayStreamer
```

### Linux receiver
```bash
cd linux-receiver
cargo build --release
cargo run --release
```

## Build Phases

1. **Virtual display creation** — CGVirtualDisplay on macOS, visible in System Settings → Displays
2. **Screen capture** — ScreenCaptureKit targeting the virtual display at 30-60fps
3. **Encode and stream** — VideoToolbox H.264 → RTP/UDP to receiver IP
4. **Receive and render** — GStreamer pipeline on Linux, fullscreen
5. **Polish** — mDNS discovery, resolution negotiation, reconnection, menu bar controls
