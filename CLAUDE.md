# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Virtual Display Extender streams a virtual second display from a sender machine (macOS or Windows) to a Linux/Ubuntu machine over the local network via RTP/UDP H.264. It is a multi-component system: a Swift macOS menu bar app (mac-sender), a Rust GStreamer app (win-sender), and a Rust GStreamer receiver (linux-receiver).

## Build & Run

### macOS sender (Swift Package Manager)
```bash
cd mac-sender
swift build                          # debug build
swift run VirtualDisplayStreamer      # run the menu bar app
```

### Windows sender (Cargo)
```bash
cd win-sender
cargo build --release
cargo run --release -- --host RECEIVER_IP --monitor 0
cargo run --release -- --list-monitors              # show available displays
cargo run --release -- --virtual-display --host IP  # create & stream virtual display
cargo run --release -- --test-stream                # verification: stream to localhost 5s
```

### Linux receiver (Cargo)
```bash
cd linux-receiver
cargo build --release
cargo run --release                  # listens on UDP 5004 by default
cargo run --release -- --port 5004 --fullscreen
```

### Setup scripts
- `scripts/setup-mac.sh` — checks macOS version/Xcode, builds mac-sender
- `scripts/setup-windows.ps1` — checks Rust/GStreamer, builds win-sender (use `-InstallVirtualDisplay` for driver setup)
- `scripts/setup-linux.sh` — installs GStreamer + Rust deps, builds linux-receiver

### Phase verification tests (macOS only)
```bash
cd mac-sender
swift run VirtualDisplayStreamer --test-display   # Phase 1: virtual display creation
swift run VirtualDisplayStreamer --test-capture   # Phase 2: ScreenCaptureKit capture
swift run VirtualDisplayStreamer --test-stream    # Phase 3: full capture→encode→RTP/UDP to localhost
```

## Architecture

### mac-sender (Swift, SPM)

The pipeline flows: **VirtualDisplay → ScreenCapture → H264Encoder → RTPStreamer**

- `StreamingPipeline` — orchestrator that owns and wires together all stages; `@MainActor`, `ObservableObject` for SwiftUI binding
- `VirtualDisplayManager` — creates a virtual monitor using **CGVirtualDisplay private API** (reverse-engineered ObjC headers in `CGVirtualDisplayPrivate` target). The display appears in System Settings → Displays
- `ScreenCaptureManager` — captures frames from the virtual display via `SCStream` (ScreenCaptureKit); delivers `CMSampleBuffer` at configured FPS
- `H264Encoder` — hardware-accelerated VideoToolbox compression session; extracts NALUs (including SPS/PPS on keyframes) from AVCC format and delivers raw NAL units
- `RTPStreamer` — sends NALUs over UDP using Apple `Network.framework`; implements RFC 6184 FU-A fragmentation for NALUs > 1200 bytes
- `App.swift` — entry point; `MenuBarExtra` app, dispatches `--test-*` flags to test functions in `TestVirtualDisplay.swift`
- `MenuBarView` — SwiftUI popover for configuring receiver IP, resolution, FPS, bitrate

The `CGVirtualDisplayPrivate` SPM target is an ObjC module that provides headers for private CoreGraphics classes. The `.m` file is intentionally empty — classes are loaded from CoreGraphics at runtime.

### win-sender (Rust, GStreamer)

GStreamer handles the full pipeline: `d3d11screencapturesrc → d3d11convert → mfh264enc → rtph264pay → udpsink`

- `main.rs` — CLI entry point, arg parsing, event loop, Ctrl+C handling (mirrors linux-receiver pattern)
- `pipeline.rs` — builds GStreamer pipeline string with encoder/capture fallbacks: `mfh264enc` → `nvh264enc` → `x264enc`; `d3d11screencapturesrc` → `dx9screencapsrc`
- `monitor.rs` — Win32 `EnumDisplayMonitors` to list and select capture targets
- `virtual_display.rs` — manages IddCx Virtual Display Driver (config writing, device refresh, monitor detection)
- `config.rs` — `StreamConfig` struct with protocol defaults

### linux-receiver (Rust)

Single-file app (`src/main.rs`) that builds a GStreamer pipeline from a launch string:
`udpsrc → rtpjitterbuffer → rtph264depay → avdec_h264 → videoconvert → autovideosink`

Uses `clap` for CLI args (`--port`, `--fullscreen`). Handles Ctrl+C gracefully via `ctrlc` crate.

### Streaming protocol

Defined in `shared/protocol.md`. Key details:
- RTP over UDP, ports 5004 (video) / 5005 (RTCP) / 5353 (mDNS)
- H.264 High profile, CABAC, 0 B-frames, realtime latency mode
- Payload type 96, clock rate 90kHz
- Max RTP payload 1200 bytes; FU-A fragmentation for larger NALUs
- RTCP custom APP messages (subtype "VDXT") for resolution negotiation

## Key Constraints

- macOS sender requires **Screen Recording permission** (System Settings → Privacy & Security)
- CGVirtualDisplay is a **private API** — no public headers exist; the ObjC bridge in `CGVirtualDisplayPrivate` is reverse-engineered
- macOS 12.3+ required (ScreenCaptureKit dependency); Swift 5.9+ / `.macOS(.v13)` platform
- Windows sender requires GStreamer 1.20+ MSVC runtime + dev packages; `d3d11screencapturesrc` needs Windows 10+
- Windows virtual display requires the [IddCx Virtual Display Driver](https://github.com/itsmikethetech/Virtual-Display-Driver) (optional, for headless use)
- Linux receiver requires GStreamer 1.20+ with good/ugly/libav plugins installed
