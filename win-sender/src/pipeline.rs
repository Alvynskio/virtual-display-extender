use gstreamer as gst;
use gstreamer::prelude::*;

use crate::config::StreamConfig;

/// Unified pipeline variants — each pairs a capture chain with a compatible encoder
/// so GPU memory types are correct end-to-end.
///
/// Fields: (description, required_factories, capture_chain_template, encoder_template)
const PIPELINE_VARIANTS: &[(&str, &[&str], &str, &str)] = &[
    // 1. NVENC + CUDA zero-copy (GPU frames stay on GPU the whole way)
    (
        "nvh264enc (CUDA zero-copy)",
        &["d3d11screencapturesrc", "d3d11convert", "cudaupload", "cudaconvert", "nvh264enc"],
        "d3d11screencapturesrc monitor-index={monitor} show-cursor=true do-timestamp=true ! video/x-raw(memory:D3D11Memory),framerate={fps}/1 ! d3d11convert ! cudaupload ! cudaconvert",
        "nvh264enc bitrate={bitrate_kbps} rc-mode=cbr preset=p1 tune=ultra-low-latency zerolatency=true repeat-sequence-header=true aud=false bframes=0 gop-size=1 rc-lookahead=0",
    ),
    // 2. NVENC + download fallback (GPU capture → CPU → NVENC)
    (
        "nvh264enc (download fallback)",
        &["d3d11screencapturesrc", "d3d11convert", "d3d11download", "videoconvert", "nvh264enc"],
        "d3d11screencapturesrc monitor-index={monitor} show-cursor=true do-timestamp=true ! video/x-raw(memory:D3D11Memory),framerate={fps}/1 ! d3d11convert ! d3d11download ! videoconvert",
        "nvh264enc bitrate={bitrate_kbps} rc-mode=cbr preset=p1 tune=ultra-low-latency zerolatency=true repeat-sequence-header=true aud=false bframes=0 gop-size=1 rc-lookahead=0",
    ),
    // 3. Media Foundation (Intel/AMD/Nvidia via MF)
    (
        "mfh264enc",
        &["d3d11screencapturesrc", "d3d11convert", "d3d11download", "videoconvert", "mfh264enc"],
        "d3d11screencapturesrc monitor-index={monitor} show-cursor=true do-timestamp=true ! video/x-raw(memory:D3D11Memory),framerate={fps}/1 ! d3d11convert ! d3d11download ! videoconvert",
        "mfh264enc bitrate={bitrate_kbps} rc-mode=cbr low-latency=true cabac=true bframes=0 gop-size=1 quality-vs-speed=0",
    ),
    // 4. x264 software (D3D11 capture)
    (
        "x264enc (D3D11 capture)",
        &["d3d11screencapturesrc", "d3d11convert", "d3d11download", "videoconvert", "x264enc"],
        "d3d11screencapturesrc monitor-index={monitor} show-cursor=true do-timestamp=true ! video/x-raw(memory:D3D11Memory),framerate={fps}/1 ! d3d11convert ! d3d11download ! videoconvert",
        "x264enc bitrate={bitrate_kbps} tune=zerolatency speed-preset=ultrafast bframes=0 key-int-max=1 cabac=true",
    ),
    // 5. x264 software (DX9 capture — oldest fallback)
    (
        "x264enc (DX9 capture)",
        &["dx9screencapsrc", "videoconvert", "x264enc"],
        "dx9screencapsrc monitor={monitor} do-timestamp=true ! video/x-raw,framerate={fps}/1 ! videoconvert",
        "x264enc bitrate={bitrate_kbps} tune=zerolatency speed-preset=ultrafast bframes=0 key-int-max=1 cabac=true",
    ),
];

/// Find the first pipeline variant where ALL required GStreamer element factories exist.
fn find_available_variant() -> Option<(&'static str, &'static str, &'static str)> {
    for &(description, required, capture_tpl, encoder_tpl) in PIPELINE_VARIANTS {
        if required.iter().all(|name| gst::ElementFactory::find(name).is_some()) {
            return Some((description, capture_tpl, encoder_tpl));
        }
    }
    None
}

/// Build a GStreamer pipeline for capturing and streaming.
///
/// Returns the pipeline and a human-readable description of the chosen elements.
pub fn build_pipeline(config: &StreamConfig) -> Result<(gst::Pipeline, String), String> {
    let (description, capture_template, encoder_template) = find_available_variant()
        .ok_or("No usable capture+encoder combination found. Install GStreamer plugins.")?;

    let bitrate_kbps = config.bitrate / 1000;

    let capture_part = capture_template
        .replace("{monitor}", &config.monitor_index.to_string())
        .replace("{fps}", &config.fps.to_string());

    let encoder_part = encoder_template
        .replace("{bitrate}", &config.bitrate.to_string())
        .replace("{bitrate_kbps}", &bitrate_kbps.to_string());

    let pipeline_str = format!(
        "{capture} ! {encoder} ! video/x-h264,profile=high ! rtph264pay config-interval=-1 mtu=1200 pt=96 ! udpsink host={host} port={port} sync=false async=false buffer-size=2097152",
        capture = capture_part,
        encoder = encoder_part,
        host = config.host,
        port = config.port,
    );

    let pipeline = gst::parse::launch(&pipeline_str)
        .map_err(|e| format!("Failed to parse pipeline: {e}"))?
        .downcast::<gst::Pipeline>()
        .map_err(|_| "Top-level element is not a Pipeline".to_string())?;

    println!("[Sender] Pipeline: {pipeline_str}");

    Ok((pipeline, description.to_string()))
}
