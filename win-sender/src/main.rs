mod config;
mod monitor;
mod pipeline;
mod shortcut;
mod tray;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clap::Parser;
use gstreamer as gst;
use gstreamer::prelude::*;

use config::StreamConfig;

/// Virtual Display Extender - Windows Sender
///
/// Captures a display and streams it as RTP/UDP H.264 to the linux-receiver.
/// Uses GStreamer for capture, encoding, and RTP packetization.
#[derive(Parser, Debug)]
#[command(name = "win-sender", version, about)]
struct Args {
    /// Receiver IP address.
    #[arg(long, default_value = "10.0.0.21")]
    host: String,

    /// Receiver UDP port for RTP video.
    #[arg(long, default_value_t = 5004)]
    port: u16,

    /// Monitor index to capture (use --list-monitors to see available).
    #[arg(short, long, default_value_t = 0)]
    monitor: i32,

    /// Frames per second.
    #[arg(long, default_value_t = 60)]
    fps: u32,

    /// Target bitrate in bits/s (0 = auto-select based on monitor resolution).
    #[arg(long, default_value_t = 0)]
    bitrate: u32,

    /// List available monitors and exit.
    #[arg(long)]
    list_monitors: bool,

    /// Test mode: stream to localhost, count packets for 5 seconds, then exit.
    #[arg(long)]
    test_stream: bool,

    /// Run as a system tray application.
    #[arg(long)]
    tray: bool,

    /// Install a Start Menu shortcut that launches in tray mode.
    #[arg(long)]
    install_shortcut: bool,
}

fn main() {
    let args = Args::parse();

    // -- Install shortcut (runs before everything) ------------------------------
    if args.install_shortcut {
        match shortcut::install_start_menu_shortcut() {
            Ok(path) => println!("Shortcut installed: {}", path.display()),
            Err(e) => eprintln!("Failed to install shortcut: {e}"),
        }
        return;
    }

    // -- List monitors --------------------------------------------------------
    let monitors = monitor::list_monitors();
    monitor::print_monitors(&monitors);

    if args.list_monitors {
        return;
    }

    // -- Resolve bitrate from monitor resolution ------------------------------
    let bitrate = if args.bitrate == 0 {
        let (w, h) = monitors
            .iter()
            .find(|m| m.index == args.monitor as usize)
            .map(|m| (m.width, m.height))
            .unwrap_or((3840, 2160));
        StreamConfig::auto_bitrate(w, h)
    } else {
        args.bitrate
    };

    // -- Tray mode ------------------------------------------------------------
    if args.tray {
        let config = StreamConfig {
            host: args.host.clone(),
            port: args.port,
            monitor_index: args.monitor,
            fps: args.fps,
            bitrate,
        };
        tray::TrayApp::new(config).run();
        return;
    }

    // -- Init GStreamer --------------------------------------------------------
    gst::init().expect("[Sender] Failed to initialise GStreamer");
    println!("[Sender] GStreamer initialised");

    // -- Build config ---------------------------------------------------------
    let config = StreamConfig {
        host: if args.test_stream {
            "127.0.0.1".into()
        } else {
            args.host.clone()
        },
        port: args.port,
        monitor_index: args.monitor,
        fps: args.fps,
        bitrate,
    };

    println!(
        "[Sender] Bitrate: {} Mbps",
        config.bitrate / 1_000_000,
    );

    // -- Build pipeline -------------------------------------------------------
    let (pipeline, description) = match pipeline::build_pipeline(&config) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[Sender] Pipeline error: {e}");
            std::process::exit(1);
        }
    };

    println!("[Sender] Using: {description}");

    // -- Start pipeline -------------------------------------------------------
    pipeline
        .set_state(gst::State::Playing)
        .expect("[Sender] Failed to set pipeline to Playing");

    println!(
        "[Sender] Streaming monitor {} to {}:{} (@ {}fps, {} kbps)",
        config.monitor_index,
        config.host,
        config.port,
        config.fps,
        config.bitrate / 1000,
    );

    // -- Ctrl+C handling ------------------------------------------------------
    let running = Arc::new(AtomicBool::new(true));
    let running_ctrlc = Arc::clone(&running);

    ctrlc::set_handler(move || {
        println!("\n[Sender] Ctrl+C received, shutting down ...");
        running_ctrlc.store(false, Ordering::SeqCst);
    })
    .expect("[Sender] Failed to set Ctrl+C handler");

    // -- Test mode: run for 5 seconds then exit -------------------------------
    if args.test_stream {
        println!("[Sender] Test mode: streaming for 5 seconds ...");
        std::thread::sleep(std::time::Duration::from_secs(5));
        println!("[Sender] Test complete");
        shutdown(&pipeline);
        return;
    }

    // -- Main event loop ------------------------------------------------------
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

    shutdown(&pipeline);
}

fn shutdown(pipeline: &gst::Pipeline) {
    println!("[Sender] Stopping pipeline ...");
    pipeline
        .set_state(gst::State::Null)
        .expect("[Sender] Failed to set pipeline to Null");

    println!("[Sender] Pipeline stopped. Goodbye.");
}
