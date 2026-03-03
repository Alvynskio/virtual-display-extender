import CoreGraphics
import Foundation
import CGVirtualDisplayPrivate

/// Creates and manages a virtual display using CGVirtualDisplay (private CoreGraphics API).
/// The virtual display appears as a real monitor in System Settings → Displays.
final class VirtualDisplayManager {
    private var virtualDisplay: CGVirtualDisplay?

    private(set) var displayID: CGDirectDisplayID = 0
    private(set) var width: Int
    private(set) var height: Int
    private(set) var refreshRate: Int

    init(width: Int = 1920, height: Int = 1080, refreshRate: Int = 60) {
        self.width = width
        self.height = height
        self.refreshRate = refreshRate
    }

    /// Creates the virtual display. Returns the CGDirectDisplayID on success.
    func create() throws -> CGDirectDisplayID {
        let descriptor = CGVirtualDisplayDescriptor()
        descriptor.name = "Virtual Display Extender"
        descriptor.maxPixelsWide = UInt32(width)
        descriptor.maxPixelsHigh = UInt32(height)
        descriptor.sizeInMillimeters = CGSize(width: 600, height: 340) // ~27" display
        descriptor.productID = 0xDE01
        descriptor.vendorID = 0xDE00
        descriptor.serialNum = 0x0001
        descriptor.setDispatchQueue(DispatchQueue.global(qos: .userInteractive))

        // Define the display mode
        let mode = CGVirtualDisplayMode(width: UInt32(width), height: UInt32(height), refreshRate: Double(refreshRate))

        let settings = CGVirtualDisplaySettings()
        settings.hiDPI = 0 // 0 = standard, 1 = HiDPI (retina)
        settings.modes = [mode]

        guard let display = CGVirtualDisplay(descriptor: descriptor) else {
            throw VirtualDisplayError.creationFailed
        }

        let applied = display.apply(settings)
        if !applied {
            print("[VirtualDisplay] Warning: applySettings returned false")
        }

        self.virtualDisplay = display
        self.displayID = display.displayID

        print("[VirtualDisplay] Created display ID: \(displayID) (\(width)x\(height) @ \(refreshRate)Hz)")
        return displayID
    }

    /// Tears down the virtual display.
    func destroy() {
        virtualDisplay = nil
        displayID = 0
        print("[VirtualDisplay] Destroyed")
    }

    deinit {
        destroy()
    }
}

enum VirtualDisplayError: Error, CustomStringConvertible {
    case creationFailed
    case displayNotFound

    var description: String {
        switch self {
        case .creationFailed:
            return "Failed to create virtual display. Ensure macOS 12+ and appropriate entitlements."
        case .displayNotFound:
            return "Virtual display not found in active display list."
        }
    }
}
