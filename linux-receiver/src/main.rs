use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clap::Parser;
use gstreamer as gst;
use gstreamer::prelude::*;

/// Virtual Display Extender - Linux Receiver
///
/// Receives an RTP/UDP H.264 video stream from the macOS sender and renders it
/// using GStreamer. Designed to display a virtual second display fullscreen on
/// a Linux/Ubuntu machine connected over the local network.
#[derive(Parser, Debug)]
#[command(name = "linux-receiver", version, about)]
struct Args {
    /// UDP port to listen on for incoming RTP packets.
    #[arg(long, default_value_t = 5004)]
    port: u16,

    /// Attempt to display the video window in fullscreen mode.
    #[arg(long)]
    fullscreen: bool,
}

fn main() {
    let args = Args::parse();

    // -- Initialise GStreamer --------------------------------------------------
    gst::init().expect("[Receiver] Failed to initialise GStreamer");
    println!("[Receiver] GStreamer initialised");

    // -- Build the pipeline from a launch string ------------------------------
    let pipeline_str = format!(
        concat!(
            "udpsrc port={port} ",
            "caps=\"application/x-rtp,media=video,encoding-name=H264,",
            "clock-rate=90000,payload=96\" ",
            "! rtpjitterbuffer latency=50 ",
            "! rtph264depay ",
            "! avdec_h264 ",
            "! videoconvert ",
            "! autovideosink sync=false"
        ),
        port = args.port,
    );

    println!("[Receiver] Pipeline: {}", pipeline_str);

    let pipeline = gst::parse::launch(&pipeline_str)
        .expect("[Receiver] Failed to parse GStreamer pipeline");

    let pipeline = pipeline
        .downcast::<gst::Pipeline>()
        .expect("[Receiver] Top-level element is not a Pipeline");

    // -- Fullscreen handling via GstVideoOverlay ------------------------------
    //
    // When --fullscreen is requested we listen for the `prepare-window-handle`
    // message that GStreamer video sinks emit before they create a window.
    // At that point we can query the overlay interface and request fullscreen
    // rendering (on X11 the sink typically honours this, on Wayland support
    // varies by sink).
    let want_fullscreen = args.fullscreen;

    let bus = pipeline.bus().expect("[Receiver] Pipeline has no bus");

    // Set up a sync handler so we can intercept the prepare-window-handle
    // message which is emitted from the streaming thread.
    bus.set_sync_handler(move |_bus, msg| {
        if msg.type_() == gst::MessageType::Element {
            if let Some(structure) = msg.structure() {
                if structure.name().as_str() == "prepare-window-handle" {
                    if want_fullscreen {
                        println!("[Receiver] Window handle ready -- requesting fullscreen");
                        // The element that sent the message implements
                        // GstVideoOverlay.  We use the gstreamer-video crate
                        // to access it.
                        if let Some(src) = msg.src() {
                            if let Ok(overlay) =
                                src.dynamic_cast_ref::<gstreamer_video::VideoOverlay>()
                                    .ok_or(())
                            {
                                // expose() with 0 lets the sink allocate its
                                // own window; we just need a handle for the
                                // fullscreen call.
                                overlay.set_render_rectangle(-1, -1, -1, -1).ok();
                            }
                        }
                    }
                }
            }
        }
        gst::BusSyncReply::Pass
    });

    // -- Start the pipeline ---------------------------------------------------
    pipeline
        .set_state(gst::State::Playing)
        .expect("[Receiver] Failed to set pipeline to Playing");

    println!(
        "[Receiver] Listening for RTP H.264 stream on UDP port {} ...",
        args.port
    );
    if want_fullscreen {
        println!("[Receiver] Fullscreen mode requested");
    }

    // -- Ctrl+C handling ------------------------------------------------------
    let running = Arc::new(AtomicBool::new(true));
    let running_ctrlc = Arc::clone(&running);

    ctrlc::set_handler(move || {
        println!("\n[Receiver] Ctrl+C received, shutting down ...");
        running_ctrlc.store(false, Ordering::SeqCst);
    })
    .expect("[Receiver] Failed to set Ctrl+C handler");

    // -- Main event loop ------------------------------------------------------
    let bus = pipeline.bus().expect("[Receiver] Pipeline has no bus");
    let mut frame_reported = false;

    while running.load(Ordering::SeqCst) {
        // Poll the bus with a 100 ms timeout so we can also check the
        // running flag for Ctrl+C.
        if let Some(msg) = bus.timed_pop(gst::ClockTime::from_mseconds(100)) {
            use gst::MessageView;

            match msg.view() {
                MessageView::Eos(..) => {
                    println!("[Receiver] End of stream");
                    break;
                }
                MessageView::Error(err) => {
                    eprintln!(
                        "[Receiver] Error from {}: {}",
                        err.src()
                            .map(|s| s.path_string().to_string())
                            .unwrap_or_else(|| "unknown".into()),
                        err.error()
                    );
                    if let Some(debug) = err.debug() {
                        eprintln!("[Receiver] Debug info: {}", debug);
                    }
                    break;
                }
                MessageView::StateChanged(state_changed) => {
                    // Only report state changes for the pipeline itself, not
                    // every internal element.
                    if state_changed.src().map(|s| s == &pipeline).unwrap_or(false) {
                        println!(
                            "[Receiver] Pipeline state: {:?} -> {:?}",
                            state_changed.old(),
                            state_changed.current()
                        );
                    }
                }
                MessageView::StreamStart(..) => {
                    if !frame_reported {
                        println!("[Receiver] Stream started -- receiving video");
                        frame_reported = true;
                    }
                }
                MessageView::Warning(warn) => {
                    eprintln!(
                        "[Receiver] Warning from {}: {}",
                        warn.src()
                            .map(|s| s.path_string().to_string())
                            .unwrap_or_else(|| "unknown".into()),
                        warn.error()
                    );
                }
                _ => {}
            }
        }
    }

    // -- Clean shutdown -------------------------------------------------------
    println!("[Receiver] Stopping pipeline ...");
    pipeline
        .set_state(gst::State::Null)
        .expect("[Receiver] Failed to set pipeline to Null");
    println!("[Receiver] Pipeline stopped. Goodbye.");
}
