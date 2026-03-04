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

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 5004,
            monitor_index: 0,
            width: 1920,
            height: 1080,
            fps: 60,
            bitrate: 15_000_000,
        }
    }
}
