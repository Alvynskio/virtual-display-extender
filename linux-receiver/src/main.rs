mod pipeline;
mod tray;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;

use clap::Parser;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_video::prelude::VideoOverlayExt;

/// Virtual Display Extender - Linux Receiver
///
/// Receives an RTP/UDP H.264 video stream and renders it using GStreamer.
/// Runs as a system tray application by default.
#[derive(Parser, Debug)]
#[command(name = "virtual-display-receiver", version, about)]
struct Args {
    /// UDP port to listen on for incoming RTP packets.
    #[arg(long, default_value_t = 5004)]
    port: u16,

    /// Attempt to display the video window in fullscreen mode.
    #[arg(long, default_value_t = true)]
    fullscreen: bool,

    /// Jitter buffer latency in milliseconds (lower = less delay, higher = fewer drops).
    #[arg(long, default_value_t = 40)]
    jitter_latency: u32,

    /// Run in CLI mode without the system tray (headless / SSH use).
    #[arg(long)]
    cli: bool,
}

#[derive(Debug)]
pub enum AppEvent {
    StartStop,
    FullscreenToggled,
    Quit,
    PipelineStarted,
    StreamReceiving,
    PipelineStopped,
    PipelineError(String),
}

fn check_udp_buffer_sysctl() {
    let required = pipeline::UDP_BUFFER_SIZE;
    let path = "/proc/sys/net/core/rmem_max";
    match std::fs::read_to_string(path) {
        Ok(contents) => {
            if let Ok(current) = contents.trim().parse::<u32>() {
                if current < required {
                    eprintln!(
                        "[Receiver] WARNING: net.core.rmem_max = {} ({:.0} MB) is below the \
                         requested UDP buffer size of {} ({:.0} MB).",
                        current,
                        current as f64 / 1_048_576.0,
                        required,
                        required as f64 / 1_048_576.0,
                    );
                    eprintln!(
                        "[Receiver] The kernel will clamp the socket buffer. To fix, run:"
                    );
                    eprintln!(
                        "[Receiver]   sudo sysctl -w net.core.rmem_max={required} net.core.rmem_default={required}"
                    );
                } else {
                    println!(
                        "[Receiver] OS UDP buffer limit: {} MB (OK)",
                        current / 1_048_576
                    );
                }
            }
        }
        Err(_) => {
            eprintln!("[Receiver] Could not read {path} — unable to verify UDP buffer limits");
        }
    }
}

fn main() {
    let args = Args::parse();

    gst::init().expect("[Receiver] Failed to initialise GStreamer");
    println!("[Receiver] GStreamer initialised");
    check_udp_buffer_sysctl();

    if args.cli {
        cli_mode(args);
    } else {
        tray_mode(args);
    }
}

fn tray_mode(args: Args) {
    use ksni::blocking::TrayMethods;

    let (event_tx, event_rx) = mpsc::channel();

    let tray_obj =
        tray::ReceiverTray::new(event_tx.clone(), args.port, args.fullscreen, args.jitter_latency);

    let handle = match tray_obj.spawn() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("[Receiver] Failed to start system tray: {e}");
            eprintln!("[Receiver] Falling back to CLI mode. Install a StatusNotifierItem host");
            eprintln!("[Receiver]   (e.g. GNOME extension 'AppIndicator') or use --cli.");
            cli_mode(args);
            return;
        }
    };

    println!(
        "[Receiver] System tray started (port: {}, fullscreen: {})",
        args.port, args.fullscreen
    );

    let mut pipeline_handle: Option<pipeline::PipelineHandle> = None;

    loop {
        let event = match event_rx.recv() {
            Ok(e) => e,
            Err(_) => break,
        };

        match event {
            AppEvent::StartStop => {
                if pipeline_handle.is_some() {
                    println!("[Receiver] Stopping receiver...");
                    let ph = pipeline_handle.take().unwrap();
                    ph.stop();
                    handle.update(|tray| {
                        tray.state = tray::ReceiverState::Idle;
                    });
                } else {
                    let (port, fullscreen, jitter) = {
                        let mut port = args.port;
                        let mut fullscreen = args.fullscreen;
                        let mut jitter = args.jitter_latency;
                        handle.update(|tray| {
                            port = tray.port;
                            fullscreen = tray.fullscreen;
                            jitter = tray.jitter_latency;
                            tray.state = tray::ReceiverState::Running;
                        });
                        (port, fullscreen, jitter)
                    };
                    println!("[Receiver] Starting receiver on port {}...", port);
                    let ph = pipeline::start(port, fullscreen, jitter, event_tx.clone());
                    pipeline_handle = Some(ph);
                }
            }
            AppEvent::FullscreenToggled => {}
            AppEvent::PipelineStarted => {
                handle.update(|tray| {
                    tray.state = tray::ReceiverState::Running;
                });
            }
            AppEvent::StreamReceiving => {
                handle.update(|tray| {
                    tray.state = tray::ReceiverState::Receiving;
                });
            }
            AppEvent::PipelineStopped => {
                pipeline_handle = None;
                handle.update(|tray| {
                    tray.state = tray::ReceiverState::Idle;
                });
            }
            AppEvent::PipelineError(msg) => {
                eprintln!("[Receiver] Pipeline error: {msg}");
                pipeline_handle = None;
                handle.update(|tray| {
                    tray.state = tray::ReceiverState::Idle;
                });
            }
            AppEvent::Quit => {
                println!("[Receiver] Quitting...");
                if let Some(ph) = pipeline_handle.take() {
                    ph.stop();
                }
                handle.shutdown().wait();
                break;
            }
        }
    }

    println!("[Receiver] Goodbye.");
}

fn cli_mode(args: Args) {
    let (_desc, decode_chain) = pipeline::pick_decode_chain();

    let pipeline_str = format!(
        concat!(
            "udpsrc port={port} buffer-size={buf} retrieve-sender-address=false ",
            "caps=\"application/x-rtp,media=video,encoding-name=H264,",
            "clock-rate=90000,payload=96\" ",
            "! rtpjitterbuffer latency={jitter} ",
            "! rtph264depay ",
            "! {decode_chain}"
        ),
        port = args.port,
        buf = pipeline::UDP_BUFFER_SIZE,
        jitter = args.jitter_latency,
        decode_chain = decode_chain,
    );

    println!("[Receiver] Pipeline: {}", pipeline_str);

    let pipeline = gst::parse::launch(&pipeline_str)
        .expect("[Receiver] Failed to parse GStreamer pipeline");

    let pipeline = pipeline
        .downcast::<gst::Pipeline>()
        .expect("[Receiver] Top-level element is not a Pipeline");

    let want_fullscreen = args.fullscreen;
    let bus = pipeline.bus().expect("[Receiver] Pipeline has no bus");

    bus.set_sync_handler(move |_bus, msg| {
        if msg.type_() == gst::MessageType::Element {
            if let Some(structure) = msg.structure() {
                if structure.name().as_str() == "prepare-window-handle" && want_fullscreen {
                    if let Some(src) = msg.src() {
                        if let Ok(overlay) = src
                            .dynamic_cast_ref::<gstreamer_video::VideoOverlay>()
                            .ok_or(())
                        {
                            overlay.set_render_rectangle(-1, -1, -1, -1).ok();
                        }
                    }
                }
            }
        }
        gst::BusSyncReply::Pass
    });

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

    let running = Arc::new(AtomicBool::new(true));
    let running_ctrlc = Arc::clone(&running);

    ctrlc::set_handler(move || {
        println!("\n[Receiver] Ctrl+C received, shutting down ...");
        running_ctrlc.store(false, Ordering::SeqCst);
    })
    .expect("[Receiver] Failed to set Ctrl+C handler");

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

    println!("[Receiver] Stopping pipeline ...");
    pipeline
        .set_state(gst::State::Null)
        .expect("[Receiver] Failed to set pipeline to Null");
    println!("[Receiver] Pipeline stopped. Goodbye.");
}

