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
    #[arg(long)]
    fullscreen: bool,

    /// Jitter buffer latency in milliseconds (lower = less delay, more drops).
    #[arg(long, default_value_t = 5)]
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

fn main() {
    let args = Args::parse();

    gst::init().expect("[Receiver] Failed to initialise GStreamer");
    println!("[Receiver] GStreamer initialised");

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
    let decoder = pick_decoder_cli();

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

fn pick_decoder_cli() -> &'static str {
    if gst::ElementFactory::find("vaapih264dec").is_some()
        && gst::ElementFactory::find("vaapipostproc").is_some()
    {
        println!("[Receiver] Decoder selected: vaapih264dec + vaapipostproc");
        return "vaapih264dec ! vaapipostproc";
    }

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

    println!("[Receiver] Decoder selected: avdec_h264 (fallback)");
    "avdec_h264"
}
