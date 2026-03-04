import CoreGraphics
import Foundation
import CGVirtualDisplayPrivate

/// Standalone test: creates a virtual display and checks if macOS sees it.
/// Run with: swift run VirtualDisplayStreamer --test-display
func testVirtualDisplay() {
    print("=== Virtual Display Test ===")
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
            print("✓ Check System Settings → Displays — you should see a second monitor!")
        } else {
            print()
            print("⚠ Display was created but not yet visible in display list.")
            print("  This may be normal — check System Settings → Displays manually.")
        }

        // Keep it alive for 15 seconds so user can see it in System Settings
        print()
        print("Keeping virtual display alive for 15 seconds...")
        print("Open System Settings → Displays now to verify.")
        Thread.sleep(forTimeInterval: 15.0)

        print()
        print("Tearing down virtual display...")
        manager.destroy()
        print("✓ Done")

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
