import Foundation
import CoreMedia

/// Orchestrates the full pipeline: virtual display → capture → encode → stream.
@MainActor
final class StreamingPipeline: ObservableObject {
    @Published var isStreaming = false
    @Published var statusMessage = "Idle"
    @Published var frameCount: UInt64 = 0

    private var displayManager: VirtualDisplayManager?
    private var captureManager: ScreenCaptureManager?
    private var encoder: H264Encoder?
    private var streamer: RTPStreamer?

    struct Config {
        var width: Int = 1920
        var height: Int = 1080
        var fps: Int = 60
        var bitrateMbps: Int = 15
        var receiverHost: String = ""
        var receiverPort: UInt16 = 5004
    }

    var config = Config()

    func start() async {
        guard !isStreaming else { return }

        do {
            statusMessage = "Creating virtual display..."

            // 1. Create virtual display
            let displayManager = VirtualDisplayManager(
                width: config.width,
                height: config.height,
                refreshRate: config.fps
            )
            let displayID = try displayManager.create()
            self.displayManager = displayManager

            // 2. Set up encoder
            statusMessage = "Setting up encoder..."
            let encoder = H264Encoder(
                width: config.width,
                height: config.height,
                fps: config.fps,
                bitrateMbps: config.bitrateMbps
            )

            // 3. Set up streamer
            let streamer = RTPStreamer(host: config.receiverHost, port: config.receiverPort)
            streamer.start()
            self.streamer = streamer

            // Wire encoder output to streamer
            try encoder.setup { [weak streamer] naluData, isKeyframe, pts in
                streamer?.send(naluData: naluData, isLastNALUInFrame: true, timestamp: pts)
            }
            self.encoder = encoder

            // 4. Start capture
            statusMessage = "Starting capture..."
            let captureManager = ScreenCaptureManager(displayID: displayID)

            // Capture needs to call encoder from a non-isolated context
            let encoderRef = encoder
            var localFrameCount: UInt64 = 0

            try await captureManager.startCapture(
                width: config.width,
                height: config.height,
                fps: config.fps
            ) { sampleBuffer in
                encoderRef.encode(sampleBuffer: sampleBuffer)
                localFrameCount += 1
                if localFrameCount % 60 == 0 {
                    Task { @MainActor in
                        self.frameCount = localFrameCount
                    }
                }
            }
            self.captureManager = captureManager

            isStreaming = true
            statusMessage = "Streaming to \(config.receiverHost):\(config.receiverPort)"
            print("[Pipeline] Started streaming \(config.width)x\(config.height) @ \(config.fps)fps → \(config.receiverHost):\(config.receiverPort)")

        } catch {
            statusMessage = "Error: \(error)"
            print("[Pipeline] Start failed: \(error)")
            await stop()
        }
    }

    func stop() async {
        if let captureManager = captureManager {
            try? await captureManager.stopCapture()
            self.captureManager = nil
        }
        encoder?.teardown()
        encoder = nil
        streamer?.stop()
        streamer = nil
        displayManager?.destroy()
        displayManager = nil

        isStreaming = false
        frameCount = 0
        statusMessage = "Idle"
        print("[Pipeline] Stopped")
    }
}
