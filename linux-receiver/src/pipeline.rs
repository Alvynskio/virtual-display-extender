use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

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

/// Try to find a hardware-accelerated H.264 decoder, falling back to software.
fn pick_decoder() -> &'static str {
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

fn run_pipeline(
    port: u16,
    fullscreen: bool,
    jitter_latency: u32,
    running: &AtomicBool,
    event_tx: &mpsc::Sender<AppEvent>,
) {
    let decoder = pick_decoder();

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
        port = port,
        jitter = jitter_latency,
        decoder = decoder,
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
    let mut stream_started = false;

    while running.load(Ordering::SeqCst) {
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
                MessageView::StreamStart(..) => {
                    if !stream_started {
                        println!("[Receiver] Stream started -- receiving video");
                        stream_started = true;
                        event_tx.send(AppEvent::StreamReceiving).ok();
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
        .expect("Failed to set pipeline to Null");
    println!("[Receiver] Pipeline stopped.");

    event_tx.send(AppEvent::PipelineStopped).ok();
}
