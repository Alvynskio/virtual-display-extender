use gstreamer as gst;
use gstreamer::prelude::*;

use crate::config::StreamConfig;

/// Encoder candidates in preference order.
const ENCODERS: &[(&str, &str)] = &[
    // Media Foundation (Intel/AMD/Nvidia via MF)
    (
        "mfh264enc",
        "mfh264enc bitrate={bitrate} rc-mode=cbr low-latency=true cabac=true bframes=0 gop-size={gop}",
    ),
    // Nvidia NVENC
    (
        "nvh264enc",
        "nvh264enc bitrate={bitrate_kbps} rc-mode=cbr zerolatency=true bframes=0 gop-size={gop}",
    ),
    // Software fallback
    (
        "x264enc",
        "x264enc bitrate={bitrate_kbps} tune=zerolatency speed-preset=ultrafast bframes=0 key-int-max={gop} cabac=true",
    ),
];

/// Capture element candidates in preference order.
const CAPTURE_ELEMENTS: &[(&str, &str)] = &[
    (
        "d3d11screencapturesrc",
        "d3d11screencapturesrc monitor-index={monitor} ! video/x-raw(memory:D3D11Memory),framerate={fps}/1 ! d3d11convert",
    ),
    (
        "dx9screencapsrc",
        "dx9screencapsrc monitor={monitor} ! video/x-raw,framerate={fps}/1 ! videoconvert",
    ),
];

/// Find the first available GStreamer element from a list of candidates.
fn find_available(candidates: &[(&str, &str)]) -> Option<&'static str> {
    for &(element_name, template) in candidates {
        if gst::ElementFactory::find(element_name).is_some() {
            return Some(template);
        }
    }
    None
}

/// Build a GStreamer pipeline for capturing and streaming.
///
/// Returns the pipeline and a human-readable description of the chosen elements.
pub fn build_pipeline(config: &StreamConfig) -> Result<(gst::Pipeline, String), String> {
    let capture_template = find_available(CAPTURE_ELEMENTS)
        .ok_or("No screen capture element found. Install GStreamer bad/good plugins.")?;

    let encoder_template = find_available(ENCODERS)
        .ok_or("No H.264 encoder found. Install GStreamer ugly/bad plugins or x264.")?;

    let gop = config.fps; // 1 keyframe per second
    let bitrate_kbps = config.bitrate / 1000;

    let capture_part = capture_template
        .replace("{monitor}", &config.monitor_index.to_string())
        .replace("{fps}", &config.fps.to_string());

    let encoder_part = encoder_template
        .replace("{bitrate}", &config.bitrate.to_string())
        .replace("{bitrate_kbps}", &bitrate_kbps.to_string())
        .replace("{gop}", &gop.to_string());

    let pipeline_str = format!(
        "{capture} ! {encoder} ! video/x-h264,profile=high ! rtph264pay config-interval=-1 mtu=1200 pt=96 ! udpsink host={host} port={port} sync=false",
        capture = capture_part,
        encoder = encoder_part,
        host = config.host,
        port = config.port,
    );

    let description = format!(
        "capture: {}, encoder: {}",
        capture_template.split_whitespace().next().unwrap_or("?"),
        encoder_template.split_whitespace().next().unwrap_or("?"),
    );

    let pipeline = gst::parse::launch(&pipeline_str)
        .map_err(|e| format!("Failed to parse pipeline: {e}"))?
        .downcast::<gst::Pipeline>()
        .map_err(|_| "Top-level element is not a Pipeline".to_string())?;

    println!("[Sender] Pipeline: {pipeline_str}");

    Ok((pipeline, description))
}
