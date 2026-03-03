import SwiftUI

/// Menu bar extra content — controls for the streaming pipeline.
struct MenuBarView: View {
    @ObservedObject var pipeline: StreamingPipeline

    @State private var receiverHost: String = "192.168.1.100"
    @State private var width: String = "1920"
    @State private var height: String = "1080"
    @State private var fps: String = "60"
    @State private var bitrate: String = "15"

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Virtual Display Extender")
                .font(.headline)

            Divider()

            // Status
            HStack {
                Circle()
                    .fill(pipeline.isStreaming ? .green : .gray)
                    .frame(width: 8, height: 8)
                Text(pipeline.statusMessage)
                    .font(.caption)
                    .lineLimit(1)
            }

            if pipeline.isStreaming {
                Text("Frames: \(pipeline.frameCount)")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }

            Divider()

            if !pipeline.isStreaming {
                // Configuration fields
                Group {
                    LabeledField(label: "Receiver IP:", text: $receiverHost)
                    HStack(spacing: 4) {
                        LabeledField(label: "W:", text: $width)
                        LabeledField(label: "H:", text: $height)
                    }
                    HStack(spacing: 4) {
                        LabeledField(label: "FPS:", text: $fps)
                        LabeledField(label: "Mbps:", text: $bitrate)
                    }
                }
                .font(.caption)
            }

            Divider()

            if pipeline.isStreaming {
                Button("Stop Streaming") {
                    Task {
                        await pipeline.stop()
                    }
                }
                .buttonStyle(.borderedProminent)
                .tint(.red)
            } else {
                Button("Start Streaming") {
                    pipeline.config.receiverHost = receiverHost
                    pipeline.config.width = Int(width) ?? 1920
                    pipeline.config.height = Int(height) ?? 1080
                    pipeline.config.fps = Int(fps) ?? 60
                    pipeline.config.bitrateMbps = Int(bitrate) ?? 15
                    Task {
                        await pipeline.start()
                    }
                }
                .buttonStyle(.borderedProminent)
                .disabled(receiverHost.isEmpty)
            }

            Divider()

            Button("Quit") {
                Task {
                    await pipeline.stop()
                    NSApplication.shared.terminate(nil)
                }
            }
        }
        .padding(12)
        .frame(width: 260)
    }
}

/// Small labeled text field for the menu bar popover.
struct LabeledField: View {
    let label: String
    @Binding var text: String

    var body: some View {
        HStack(spacing: 4) {
            Text(label)
                .frame(width: 30, alignment: .trailing)
            TextField("", text: $text)
                .textFieldStyle(.roundedBorder)
        }
    }
}
