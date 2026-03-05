use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clap::Parser;
use gstreamer as gst;
use gstreamer_video::prelude::VideoOverlayExt;
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

    /// Jitter buffer latency in milliseconds (lower = less delay, more drops).
    #[arg(long, default_value_t = 5)]
    jitter_latency: u32,
}

/// Try to find a hardware-accelerated H.264 decoder, falling back to software.
fn pick_decoder() -> &'static str {
    let candidates = [
        ("vaapih264dec", "vaapih264dec"),
        ("vah264dec", "vah264dec"),
        ("avdec_h264", "avdec_h264 output-corrupt=true"),
    ];

    for (element_name, pipeline_fragment) in candidates {
        if gst::ElementFactory::find(element_name).is_some() {
            println!("[Receiver] Decoder selected: {element_name}");
            return pipeline_fragment;
        }
    }

    // Ultimate fallback (avdec_h264 should always be present with libav plugin).
    println!("[Receiver] Decoder selected: avdec_h264 (fallback)");
    "avdec_h264"
}

fn main() {
    let args = Args::parse();

    // -- Initialise GStreamer --------------------------------------------------
    gst::init().expect("[Receiver] Failed to initialise GStreamer");
    println!("[Receiver] GStreamer initialised");

    // -- Pick decoder ---------------------------------------------------------
    let decoder = pick_decoder();

    // -- Build the pipeline from a launch string ------------------------------
    let pipeline_str = format!(
        concat!(
            "udpsrc port={port} retrieve-sender-address=false ",
            "caps=\"application/x-rtp,media=video,encoding-name=H264,",
            "clock-rate=90000,payload=96\" ",
            "! rtpjitterbuffer latency={jitter} drop-on-latency=true ",
            "! rtph264depay ",
            "! {decoder} ",
            "! videoconvert ",
            "! autovideosink sync=false"
        ),
        port = args.port,
        jitter = args.jitter_latency,
        decoder = decoder,
    );

    println!("[Receiver] Pipeline: {}", pipeline_str);

    let pipeline = gst::parse::launch(&pipeline_str)
        .expect("[Receiver] Failed to parse GStreamer pipeline");

    let pipeline = pipeline
        .downcast::<gst::Pipeline>()
        .expect("[Receiver] Top-level element is not a Pipeline");

    // -- Fullscreen handling via GstVideoOverlay ------------------------------
    let want_fullscreen = args.fullscreen;

    let bus = pipeline.bus().expect("[Receiver] Pipeline has no bus");

    bus.set_sync_handler(move |_bus, msg| {
        if msg.type_() == gst::MessageType::Element {
            if let Some(structure) = msg.structure() {
                if structure.name().as_str() == "prepare-window-handle" {
                    if want_fullscreen {
                        println!("[Receiver] Window handle ready -- requesting fullscreen");
                        if let Some(src) = msg.src() {
                            if let Ok(overlay) =
                                src.dynamic_cast_ref::<gstreamer_video::VideoOverlay>()
                                    .ok_or(())
                            {
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
        "[Receiver] Listening for RTP H.264 stream on UDP port {} (jitter buffer: {}ms) ...",
        args.port, args.jitter_latency,
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
