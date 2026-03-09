use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{SystemTime, UNIX_EPOCH};

use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_video::prelude::VideoOverlayExt;

use crate::AppEvent;

pub struct PipelineHandle {
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl PipelineHandle {
    pub fn stop(mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            t.join().ok();
        }
    }
}

impl Drop for PipelineHandle {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            t.join().ok();
        }
    }
}

/// UDP receive buffer size: 32MB to handle 500 Mbps all-intra bursts.
/// At 500 Mbps, 4MB fills in ~64ms (fewer than 4 frames at 60fps).
/// 32MB gives ~512ms of buffer — enough to absorb scheduling jitter.
pub const UDP_BUFFER_SIZE: u32 = 33_554_432;

/// Full decode-chain variants (decoder + sink) for CLI mode.
/// Fields: (description, required_factories, pipeline_fragment)
const DECODE_CHAIN_VARIANTS: &[(&str, &[&str], &str)] = &[
    (
        "vaapih264dec + vaapisink (full GPU zero-copy)",
        &["vaapih264dec", "vaapipostproc", "vaapisink"],
        concat!(
            "vaapih264dec ! ",
            "queue max-size-buffers=2 max-size-time=0 max-size-bytes=0 leaky=downstream ! ",
            "vaapipostproc ! vaapisink sync=true enable-last-sample=false",
        ),
    ),
    (
        "vaapih264dec + vaapipostproc + autovideosink",
        &["vaapih264dec", "vaapipostproc"],
        concat!(
            "vaapih264dec ! ",
            "queue max-size-buffers=2 max-size-time=0 max-size-bytes=0 leaky=downstream ! ",
            "vaapipostproc ! videoconvert ! autovideosink sync=true enable-last-sample=false",
        ),
    ),
    (
        "vaapih264dec + autovideosink",
        &["vaapih264dec"],
        concat!(
            "vaapih264dec ! ",
            "queue max-size-buffers=2 max-size-time=0 max-size-bytes=0 leaky=downstream ! ",
            "videoconvert ! autovideosink sync=true enable-last-sample=false",
        ),
    ),
    (
        "vah264dec + autovideosink",
        &["vah264dec"],
        concat!(
            "vah264dec ! ",
            "queue max-size-buffers=2 max-size-time=0 max-size-bytes=0 leaky=downstream ! ",
            "videoconvert ! autovideosink sync=true enable-last-sample=false",
        ),
    ),
    (
        "avdec_h264 max-threads (software)",
        &["avdec_h264"],
        concat!(
            "avdec_h264 max-threads=0 ! ",
            "queue max-size-buffers=2 max-size-time=0 max-size-bytes=0 leaky=downstream ! ",
            "videoconvert n-threads=0 ! autovideosink sync=true enable-last-sample=false",
        ),
    ),
];

/// Decode-only chain variants (no sink, ends with videoconvert producing video/x-raw)
/// for the input-selector pipeline in tray mode.
/// Fields: (description, required_factories, pipeline_fragment)
const DECODE_ONLY_VARIANTS: &[(&str, &[&str], &str)] = &[
    (
        "vaapih264dec + vaapipostproc (GPU)",
        &["vaapih264dec", "vaapipostproc"],
        concat!(
            "vaapih264dec ! ",
            "queue max-size-buffers=2 max-size-time=0 max-size-bytes=0 leaky=downstream ! ",
            "vaapipostproc ! videoconvert",
        ),
    ),
    (
        "vaapih264dec (GPU)",
        &["vaapih264dec"],
        concat!(
            "vaapih264dec ! ",
            "queue max-size-buffers=2 max-size-time=0 max-size-bytes=0 leaky=downstream ! ",
            "videoconvert",
        ),
    ),
    (
        "vah264dec (GPU)",
        &["vah264dec"],
        concat!(
            "vah264dec ! ",
            "queue max-size-buffers=2 max-size-time=0 max-size-bytes=0 leaky=downstream ! ",
            "videoconvert",
        ),
    ),
    (
        "avdec_h264 (software)",
        &["avdec_h264"],
        concat!(
            "avdec_h264 max-threads=0 ! ",
            "queue max-size-buffers=2 max-size-time=0 max-size-bytes=0 leaky=downstream ! ",
            "videoconvert n-threads=0",
        ),
    ),
];

/// Pick the best full decode+sink chain (for CLI mode).
pub fn pick_decode_chain() -> (&'static str, &'static str) {
    for &(description, required, fragment) in DECODE_CHAIN_VARIANTS {
        if required
            .iter()
            .all(|name| gst::ElementFactory::find(name).is_some())
        {
            println!("[Receiver] Decode chain selected: {description}");
            return (description, fragment);
        }
    }

    println!("[Receiver] Decode chain selected: avdec_h264 (fallback)");
    (
        "avdec_h264 (fallback)",
        "avdec_h264 ! videoconvert ! autovideosink sync=true enable-last-sample=false",
    )
}

/// Pick the best decode-only chain (for tray mode with input-selector).
fn pick_decode_only_chain() -> (&'static str, &'static str) {
    for &(description, required, fragment) in DECODE_ONLY_VARIANTS {
        if required
            .iter()
            .all(|name| gst::ElementFactory::find(name).is_some())
        {
            println!("[Receiver] Decode chain selected: {description}");
            return (description, fragment);
        }
    }

    println!("[Receiver] Decode chain selected: avdec_h264 (fallback)");
    ("avdec_h264 (fallback)", "avdec_h264 ! videoconvert")
}

pub fn start(
    port: u16,
    fullscreen: bool,
    jitter_latency: u32,
    event_tx: mpsc::Sender<AppEvent>,
) -> PipelineHandle {
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = Arc::clone(&running);

    let thread = thread::spawn(move || {
        run_pipeline(port, fullscreen, jitter_latency, &running_clone, &event_tx);
    });

    PipelineHandle {
        running,
        thread: Some(thread),
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// How long without a buffer before we consider the stream lost.
const STREAM_TIMEOUT_MS: i64 = 3000;

fn run_pipeline(
    port: u16,
    fullscreen: bool,
    jitter_latency: u32,
    running: &AtomicBool,
    event_tx: &mpsc::Sender<AppEvent>,
) {
    let (_desc, decode_chain) = pick_decode_only_chain();

    // Two-branch pipeline with input-selector:
    //   Branch 0 (status): videotestsrc + textoverlay → selector.sink_0
    //   Branch 1 (video):  udpsrc → decoder → videoconvert → selector.sink_1
    //   Output:            input-selector → autovideosink
    let pipeline_str = format!(
        concat!(
            "videotestsrc pattern=black is-live=true ",
            "! video/x-raw,width=640,height=360,framerate=1/1 ",
            "! textoverlay name=status_text valignment=center halignment=center ",
            "! selector.sink_0 ",
            "udpsrc port={port} buffer-size={buf} retrieve-sender-address=false ",
            "caps=\"application/x-rtp,media=video,encoding-name=H264,",
            "clock-rate=90000,payload=96\" ",
            "! rtpjitterbuffer latency={jitter} ",
            "! rtph264depay ",
            "! {decode_chain} ",
            "! selector.sink_1 ",
            "input-selector name=selector ",
            "! autovideosink sync=true enable-last-sample=false",
        ),
        port = port,
        buf = UDP_BUFFER_SIZE,
        jitter = jitter_latency,
        decode_chain = decode_chain,
    );

    println!("[Receiver] Pipeline: {}", pipeline_str);

    let pipeline = match gst::parse::launch(&pipeline_str) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[Receiver] Failed to parse pipeline: {e}");
            event_tx.send(AppEvent::PipelineError(e.to_string())).ok();
            return;
        }
    };

    let pipeline = match pipeline.downcast::<gst::Pipeline>() {
        Ok(p) => p,
        Err(_) => {
            eprintln!("[Receiver] Top-level element is not a Pipeline");
            event_tx
                .send(AppEvent::PipelineError("Not a pipeline".into()))
                .ok();
            return;
        }
    };

    // Get named elements for runtime control
    let selector = pipeline.by_name("selector").expect("selector not found");
    let status_text = pipeline
        .by_name("status_text")
        .expect("status_text not found");

    // Set status text properties programmatically (avoids escaping issues in pipeline string)
    status_text.set_property("text", format!("Waiting for stream on port {port}..."));
    status_text.set_property("font-desc", "Sans Bold 28");

    // Find the selector's sink pads
    let status_pad = selector
        .sink_pads()
        .into_iter()
        .find(|p| p.name().as_str() == "sink_0")
        .expect("sink_0 pad not found");
    let video_pad = selector
        .sink_pads()
        .into_iter()
        .find(|p| p.name().as_str() == "sink_1")
        .expect("sink_1 pad not found");

    // Track when the last video buffer was received
    let last_buffer_time = Arc::new(AtomicI64::new(0));
    let lbt = Arc::clone(&last_buffer_time);
    video_pad.add_probe(gst::PadProbeType::BUFFER, move |_, _| {
        lbt.store(now_ms(), Ordering::Relaxed);
        gst::PadProbeReturn::Ok
    });

    // Fullscreen handling via GstVideoOverlay
    let want_fullscreen = fullscreen;
    let bus = pipeline.bus().expect("Pipeline has no bus");

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

    if let Err(e) = pipeline.set_state(gst::State::Playing) {
        eprintln!("[Receiver] Failed to set pipeline to Playing: {e:?}");
        event_tx
            .send(AppEvent::PipelineError(format!("{e:?}")))
            .ok();
        return;
    }

    println!(
        "[Receiver] Listening on UDP port {} (jitter: {}ms, fullscreen: {})",
        port, jitter_latency, fullscreen
    );

    event_tx.send(AppEvent::PipelineStarted).ok();

    let bus = pipeline.bus().expect("Pipeline has no bus");
    let mut showing_video = false;

    while running.load(Ordering::SeqCst) {
        // Process bus messages
        if let Some(msg) = bus.timed_pop(gst::ClockTime::from_mseconds(100)) {
            use gst::MessageView;

            match msg.view() {
                MessageView::Eos(..) => {
                    println!("[Receiver] End of stream");
                    break;
                }
                MessageView::Error(err) => {
                    let msg_text = format!(
                        "Error from {}: {}",
                        err.src()
                            .map(|s| s.path_string().to_string())
                            .unwrap_or_else(|| "unknown".into()),
                        err.error()
                    );
                    eprintln!("[Receiver] {msg_text}");
                    event_tx.send(AppEvent::PipelineError(msg_text)).ok();
                    break;
                }
                MessageView::StateChanged(sc) => {
                    if sc.src().map(|s| s == &pipeline).unwrap_or(false) {
                        println!(
                            "[Receiver] Pipeline state: {:?} -> {:?}",
                            sc.old(),
                            sc.current()
                        );
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

        // Check stream status and switch input-selector as needed
        let last = last_buffer_time.load(Ordering::Relaxed);
        if last > 0 {
            let elapsed = now_ms() - last;
            if elapsed < STREAM_TIMEOUT_MS && !showing_video {
                // Stream arrived — switch to video
                println!("[Receiver] Stream detected, switching to video");
                selector.set_property("active-pad", &video_pad);
                showing_video = true;
                event_tx.send(AppEvent::StreamReceiving).ok();
            } else if elapsed >= STREAM_TIMEOUT_MS && showing_video {
                // Stream lost — switch back to status
                println!("[Receiver] Stream lost, switching to status screen");
                selector.set_property("active-pad", &status_pad);
                status_text
                    .set_property("text", format!("Connection lost. Waiting on port {port}..."));
                showing_video = false;
                event_tx.send(AppEvent::StreamLost).ok();
            }
        }
    }

    println!("[Receiver] Stopping pipeline ...");
    pipeline
        .set_state(gst::State::Null)
        .expect("Failed to set pipeline to Null");
    println!("[Receiver] Pipeline stopped.");

    event_tx.send(AppEvent::PipelineStopped).ok();
}
