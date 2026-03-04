import CoreGraphics
import CoreMedia
import Foundation
import CGVirtualDisplayPrivate
import ScreenCaptureKit

/// Standalone test: creates a virtual display and checks if macOS sees it.
/// Run with: swift run VirtualDisplayStreamer --test-display
func testVirtualDisplay() {
    print("=== Phase 1: Virtual Display Test ===")
    print()

    // List displays before
    let displaysBefore = getDisplayList()
    print("Displays before: \(displaysBefore.count)")
    for id in displaysBefore {
        let bounds = CGDisplayBounds(id)
        print("  Display \(id): \(Int(bounds.width))x\(Int(bounds.height))")
    }

    print()
    print("Creating virtual display (1920x1080 @ 60Hz)...")
    let manager = VirtualDisplayManager(width: 1920, height: 1080, refreshRate: 60)

    do {
        let displayID = try manager.create()
        print("✓ Virtual display created with ID: \(displayID)")

        // Give macOS a moment to register it
        Thread.sleep(forTimeInterval: 1.0)

        // List displays after
        let displaysAfter = getDisplayList()
        print()
        print("Displays after: \(displaysAfter.count)")
        for id in displaysAfter {
            let bounds = CGDisplayBounds(id)
            print("  Display \(id): \(Int(bounds.width))x\(Int(bounds.height))")
        }

        let newDisplays = displaysAfter.filter { !displaysBefore.contains($0) }
        if !newDisplays.isEmpty {
            print()
            print("✓ NEW display(s) detected: \(newDisplays)")
        } else {
            print()
            print("⚠ Display was created but not yet visible in display list.")
        }

        // Keep it alive for 10 seconds so user can see it in System Settings
        print()
        print("Keeping virtual display alive for 10 seconds...")
        Thread.sleep(forTimeInterval: 10.0)

        print()
        print("Tearing down virtual display...")
        manager.destroy()
        print("✓ Phase 1 complete")

    } catch {
        print("✗ Failed: \(error)")
    }
}

/// Phase 2 test: creates virtual display + captures frames via ScreenCaptureKit.
/// Run with: swift run VirtualDisplayStreamer --test-capture
func testCapture() {
    print("=== Phase 2: Screen Capture Test ===")
    print()

    let manager = VirtualDisplayManager(width: 1920, height: 1080, refreshRate: 60)

    do {
        let displayID = try manager.create()
        print("✓ Virtual display created (ID: \(displayID))")
        Thread.sleep(forTimeInterval: 1.0)

        let captureManager = ScreenCaptureManager(displayID: displayID)
        var frameCount = 0
        let targetFrames = 120 // capture ~2 seconds at 60fps
        let semaphore = DispatchSemaphore(value: 0)
        var firstFrameTime: Date?
        var lastFrameTime: Date?

        print("Starting capture (targeting \(targetFrames) frames at 60fps)...")
        print("NOTE: Screen Recording permission required.")
        print()

        Task {
            do {
                try await captureManager.startCapture(
                    width: 1920,
                    height: 1080,
                    fps: 60
                ) { sampleBuffer in
                    frameCount += 1

                    if frameCount == 1 {
                        firstFrameTime = Date()
                        // Log first frame details
                        if let imageBuffer = CMSampleBufferGetImageBuffer(sampleBuffer) {
                            let w = CVPixelBufferGetWidth(imageBuffer)
                            let h = CVPixelBufferGetHeight(imageBuffer)
                            let fmt = CVPixelBufferGetPixelFormatType(imageBuffer)
                            print("  First frame: \(w)x\(h), format: \(fourCCString(fmt))")
                        }
                    }

                    if frameCount % 30 == 0 {
                        print("  Captured \(frameCount) frames...")
                    }

                    if frameCount >= targetFrames {
                        lastFrameTime = Date()
                        Task {
                            try? await captureManager.stopCapture()
                            semaphore.signal()
                        }
                    }
                }
            } catch {
                print("✗ Capture failed: \(error)")
                semaphore.signal()
            }
        }

        // Wait for capture to complete (timeout 10s)
        let result = semaphore.wait(timeout: .now() + 10.0)
        if result == .timedOut {
            print("⚠ Capture timed out after 10 seconds (got \(frameCount) frames)")
            print("  This usually means Screen Recording permission was denied.")
        }

        if let first = firstFrameTime, let last = lastFrameTime {
            let duration = last.timeIntervalSince(first)
            let actualFPS = Double(frameCount) / duration
            print()
            print("✓ Captured \(frameCount) frames in \(String(format: "%.2f", duration))s")
            print("  Actual FPS: \(String(format: "%.1f", actualFPS))")
        }

        print()
        manager.destroy()
        print("✓ Phase 2 complete")

    } catch {
        print("✗ Failed: \(error)")
    }
}

private func getDisplayList() -> [CGDirectDisplayID] {
    var displayCount: UInt32 = 0
    CGGetActiveDisplayList(16, nil, &displayCount)
    var displays = [CGDirectDisplayID](repeating: 0, count: Int(displayCount))
    CGGetActiveDisplayList(16, &displays, &displayCount)
    return Array(displays.prefix(Int(displayCount)))
}

private func fourCCString(_ code: OSType) -> String {
    let bytes: [UInt8] = [
        UInt8((code >> 24) & 0xFF),
        UInt8((code >> 16) & 0xFF),
        UInt8((code >> 8) & 0xFF),
        UInt8(code & 0xFF),
    ]
    return String(bytes: bytes, encoding: .ascii) ?? "\(code)"
}
