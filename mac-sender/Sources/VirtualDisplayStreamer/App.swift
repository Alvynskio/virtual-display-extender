import SwiftUI

@main
struct VirtualDisplayStreamerApp: App {
    @StateObject private var pipeline = StreamingPipeline()

    init() {
        // Handle --test-display flag before launching the full app
        if CommandLine.arguments.contains("--test-display") {
            testVirtualDisplay()
            exit(0)
        }
    }

    var body: some Scene {
        MenuBarExtra("VDExt", systemImage: "display.2") {
            MenuBarView(pipeline: pipeline)
        }
        .menuBarExtraStyle(.window)
    }
}
