import ScreenCaptureKit
import CoreMedia
import CoreVideo
import Foundation

/// Captures the virtual display's framebuffer using ScreenCaptureKit.
final class ScreenCaptureManager: NSObject, SCStreamDelegate, SCStreamOutput {
    private var stream: SCStream?
    private var displayID: CGDirectDisplayID
    private var onFrame: ((CMSampleBuffer) -> Void)?

    init(displayID: CGDirectDisplayID) {
        self.displayID = displayID
        super.init()
    }

    /// Starts capturing the virtual display. Calls `onFrame` for each captured frame.
    func startCapture(width: Int, height: Int, fps: Int, onFrame: @escaping (CMSampleBuffer) -> Void) async throws {
        self.onFrame = onFrame

        // Get shareable content and find our virtual display
        let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: false)
        guard let display = content.displays.first(where: { $0.displayID == displayID }) else {
            throw ScreenCaptureError.displayNotFound(displayID)
        }

        print("[ScreenCapture] Found display: \(display.width)x\(display.height) (ID: \(display.displayID))")

        // Configure the capture
        let filter = SCContentFilter(display: display, excludingWindows: [])
        let config = SCStreamConfiguration()
        config.width = width
        config.height = height
        config.minimumFrameInterval = CMTime(value: 1, timescale: CMTimeScale(fps))
        config.pixelFormat = kCVPixelFormatType_32BGRA
        config.queueDepth = 3
        config.showsCursor = true

        // Create and start the stream
        let stream = SCStream(filter: filter, configuration: config, delegate: self)
        try stream.addStreamOutput(self, type: .screen, sampleHandlerQueue: DispatchQueue(label: "screen-capture", qos: .userInteractive))
        try await stream.startCapture()

        self.stream = stream
        print("[ScreenCapture] Capture started at \(width)x\(height) @ \(fps)fps")
    }

    /// Stops the capture session.
    func stopCapture() async throws {
        if let stream = stream {
            try await stream.stopCapture()
            self.stream = nil
            print("[ScreenCapture] Capture stopped")
        }
    }

    // MARK: - SCStreamOutput

    func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .screen else { return }
        guard sampleBuffer.isValid else { return }
        onFrame?(sampleBuffer)
    }

    // MARK: - SCStreamDelegate

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        print("[ScreenCapture] Stream stopped with error: \(error.localizedDescription)")
    }
}

enum ScreenCaptureError: Error, CustomStringConvertible {
    case displayNotFound(CGDirectDisplayID)

    var description: String {
        switch self {
        case .displayNotFound(let id):
            return "Display ID \(id) not found in shareable content. Is the virtual display active?"
        }
    }
}
