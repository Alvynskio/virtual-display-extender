/// Streaming configuration with defaults from shared/protocol.md.
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// Receiver IP address.
    pub host: String,
    /// Receiver UDP port for RTP video.
    pub port: u16,
    /// GStreamer monitor-index to capture.
    pub monitor_index: i32,
    /// Frames per second.
    pub fps: u32,
    /// Target bitrate in bits/s.
    pub bitrate: u32,
}

impl StreamConfig {
    /// Auto-select bitrate based on resolution.
    /// Returns bitrate in bits/s.
    ///
    /// All-intra encoding (GOP=1) needs higher bitrates since every frame
    /// is an I-frame. These values are sized for gigabit LAN.
    pub fn auto_bitrate(width: u32, height: u32) -> u32 {
        let pixels = width * height;
        if pixels >= 3840 * 2160 {
            500_000_000 // 500 Mbps for 4K all-intra
        } else if pixels >= 2560 * 1440 {
            250_000_000 // 250 Mbps for 1440p all-intra
        } else {
            150_000_000 // 150 Mbps for 1080p and below all-intra
        }
    }
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            host: "10.0.0.21".into(),
            port: 5004,
            monitor_index: 0,
            fps: 60,
            bitrate: 500_000_000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_bitrate_4k() {
        assert_eq!(StreamConfig::auto_bitrate(3840, 2160), 500_000_000);
    }

    #[test]
    fn auto_bitrate_1440p() {
        assert_eq!(StreamConfig::auto_bitrate(2560, 1440), 250_000_000);
    }

    #[test]
    fn auto_bitrate_1080p() {
        assert_eq!(StreamConfig::auto_bitrate(1920, 1080), 150_000_000);
    }

    #[test]
    fn auto_bitrate_720p() {
        assert_eq!(StreamConfig::auto_bitrate(1280, 720), 150_000_000);
    }

    #[test]
    fn auto_bitrate_above_1440p_below_4k() {
        // 3440x1440 ultrawide — above 1440p threshold
        assert_eq!(StreamConfig::auto_bitrate(3440, 1440), 250_000_000);
    }

    #[test]
    fn default_config() {
        let config = StreamConfig::default();
        assert_eq!(config.host, "10.0.0.21");
        assert_eq!(config.port, 5004);
        assert_eq!(config.monitor_index, 0);
        assert_eq!(config.fps, 60);
        assert_eq!(config.bitrate, 500_000_000);
    }
}
