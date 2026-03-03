import SwiftUI

@main
struct VirtualDisplayStreamerApp: App {
    @StateObject private var pipeline = StreamingPipeline()

    var body: some Scene {
        MenuBarExtra("VDExt", systemImage: "display.2") {
            MenuBarView(pipeline: pipeline)
        }
        .menuBarExtraStyle(.window)
    }
}
