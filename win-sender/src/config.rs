/// Streaming configuration with defaults from shared/protocol.md.
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// Receiver IP address.
    pub host: String,
    /// Receiver UDP port for RTP video.
    pub port: u16,
    /// GStreamer monitor-index to capture.
    pub monitor_index: i32,
    /// Horizontal resolution.
    pub width: u32,
    /// Vertical resolution.
    pub height: u32,
    /// Frames per second.
    pub fps: u32,
    /// Target bitrate in bits/s.
    pub bitrate: u32,
}

impl StreamConfig {
    /// Auto-select bitrate based on resolution.
    /// Returns bitrate in bits/s.
    pub fn auto_bitrate(width: u32, height: u32) -> u32 {
        let pixels = width * height;
        if pixels >= 3840 * 2160 {
            50_000_000 // 50 Mbps for 4K
        } else if pixels >= 2560 * 1440 {
            30_000_000 // 30 Mbps for 1440p
        } else {
            15_000_000 // 15 Mbps for 1080p and below
        }
    }
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            host: "10.0.0.21".into(),
            port: 5004,
            monitor_index: 0,
            width: 3840,
            height: 2160,
            fps: 60,
            bitrate: 50_000_000,
        }
    }
}
