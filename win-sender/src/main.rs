mod config;
mod monitor;
mod pipeline;
mod virtual_display;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clap::Parser;
use gstreamer as gst;
use gstreamer::prelude::*;

use config::StreamConfig;

/// Virtual Display Extender - Windows Sender
///
/// Captures a display (real or virtual) and streams it as RTP/UDP H.264 to
/// the linux-receiver. Uses GStreamer for capture, encoding, and RTP packetization.
#[derive(Parser, Debug)]
#[command(name = "win-sender", version, about)]
struct Args {
    /// Receiver IP address.
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Receiver UDP port for RTP video.
    #[arg(long, default_value_t = 5004)]
    port: u16,

    /// Monitor index to capture (use --list-monitors to see available).
    #[arg(long, default_value_t = 0)]
    monitor: i32,

    /// Horizontal resolution (used when creating a virtual display).
    #[arg(long, default_value_t = 1920)]
    width: u32,

    /// Vertical resolution (used when creating a virtual display).
    #[arg(long, default_value_t = 1080)]
    height: u32,

    /// Frames per second.
    #[arg(long, default_value_t = 60)]
    fps: u32,

    /// Target bitrate in bits/s.
    #[arg(long, default_value_t = 15_000_000)]
    bitrate: u32,

    /// List available monitors and exit.
    #[arg(long)]
    list_monitors: bool,

    /// Create a virtual display and capture it (requires IddCx driver).
    #[arg(long)]
    virtual_display: bool,

    /// Test mode: stream to localhost, count packets for 5 seconds, then exit.
    #[arg(long)]
    test_stream: bool,
}

fn main() {
    let args = Args::parse();

    // -- List monitors ----------------------------------------------------
    let monitors = monitor::list_monitors();
    monitor::print_monitors(&monitors);

    if args.list_monitors {
        return;
    }

    // -- Init GStreamer ----------------------------------------------------
    gst::init().expect("[Sender] Failed to initialise GStreamer");
    println!("[Sender] GStreamer initialised");

    // -- Virtual display (optional) ---------------------------------------
    let monitor_index = if args.virtual_display {
        match virtual_display::create_virtual_monitor(args.width, args.height, args.fps) {
            Ok(idx) => idx,
            Err(e) => {
                eprintln!("[Sender] Virtual display error: {e}");
                std::process::exit(1);
            }
        }
    } else {
        args.monitor
    };

    // -- Build config -----------------------------------------------------
    let config = StreamConfig {
        host: if args.test_stream {
            "127.0.0.1".into()
        } else {
            args.host.clone()
        },
        port: args.port,
        monitor_index,
        width: args.width,
        height: args.height,
        fps: args.fps,
        bitrate: args.bitrate,
    };

    // -- Build pipeline ---------------------------------------------------
    let (pipeline, description) = match pipeline::build_pipeline(&config) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[Sender] Pipeline error: {e}");
            if args.virtual_display {
                virtual_display::destroy_virtual_monitor();
            }
            std::process::exit(1);
        }
    };

    println!("[Sender] Using: {description}");

    // -- Start pipeline ---------------------------------------------------
    pipeline
        .set_state(gst::State::Playing)
        .expect("[Sender] Failed to set pipeline to Playing");

    println!(
        "[Sender] Streaming monitor {monitor_index} to {}:{} ({}x{} @ {}fps, {} kbps)",
        config.host,
        config.port,
        config.width,
        config.height,
        config.fps,
        config.bitrate / 1000,
    );

    // -- Ctrl+C handling --------------------------------------------------
    let running = Arc::new(AtomicBool::new(true));
    let running_ctrlc = Arc::clone(&running);
    let has_virtual = args.virtual_display;

    ctrlc::set_handler(move || {
        println!("\n[Sender] Ctrl+C received, shutting down ...");
        running_ctrlc.store(false, Ordering::SeqCst);
    })
    .expect("[Sender] Failed to set Ctrl+C handler");

    // -- Test mode: run for 5 seconds then exit ---------------------------
    if args.test_stream {
        println!("[Sender] Test mode: streaming for 5 seconds ...");
        std::thread::sleep(std::time::Duration::from_secs(5));
        println!("[Sender] Test complete");
        shutdown(&pipeline, has_virtual);
        return;
    }

    // -- Main event loop --------------------------------------------------
    let bus = pipeline.bus().expect("[Sender] Pipeline has no bus");

    while running.load(Ordering::SeqCst) {
        if let Some(msg) = bus.timed_pop(gst::ClockTime::from_mseconds(100)) {
            use gst::MessageView;

            match msg.view() {
                MessageView::Eos(..) => {
                    println!("[Sender] End of stream");
                    break;
                }
                MessageView::Error(err) => {
                    eprintln!(
                        "[Sender] Error from {}: {}",
                        err.src()
                            .map(|s| s.path_string().to_string())
                            .unwrap_or_else(|| "unknown".into()),
                        err.error()
                    );
                    if let Some(debug) = err.debug() {
                        eprintln!("[Sender] Debug info: {debug}");
                    }
                    break;
                }
                MessageView::StateChanged(sc) => {
                    if sc.src().map(|s| s == &pipeline).unwrap_or(false) {
                        println!(
                            "[Sender] Pipeline state: {:?} -> {:?}",
                            sc.old(),
                            sc.current()
                        );
                    }
                }
                MessageView::Warning(warn) => {
                    eprintln!(
                        "[Sender] Warning from {}: {}",
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

    shutdown(&pipeline, has_virtual);
}

fn shutdown(pipeline: &gst::Pipeline, has_virtual_display: bool) {
    println!("[Sender] Stopping pipeline ...");
    pipeline
        .set_state(gst::State::Null)
        .expect("[Sender] Failed to set pipeline to Null");

    if has_virtual_display {
        virtual_display::destroy_virtual_monitor();
    }

    println!("[Sender] Pipeline stopped. Goodbye.");
}
