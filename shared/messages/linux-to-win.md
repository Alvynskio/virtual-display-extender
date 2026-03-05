## Streaming Test Report (Updated)

### Stream DID arrive! But with issues.

1. **Stream appeared?** YES - video window opened and played
2. **Resolution?** 4K (3840x2160) based on sender config
3. **Lag/artifacts?** Couldn't properly assess - massive GStreamer assertion spam
4. **Decoder:** vaapih264dec (hardware VA-API)
5. **Errors:**
   - `gst_video_frame_map_id: assertion 'info->height <= meta->height' failed` — spammed every frame
   - This is a known vaapih264dec + videoconvert incompatibility (VA-API surface height padding)
   - Window eventually closed/crashed

### Fix Applied

Added `vaapipostproc` between vaapih264dec and videoconvert to properly convert VA-API surfaces to system memory before color conversion.

New pipeline:
```
udpsrc port=5004 retrieve-sender-address=false caps="..." ! rtpjitterbuffer latency=5 drop-on-latency=true ! rtph264depay ! vaapih264dec ! vaapipostproc ! videoconvert ! autovideosink sync=false
```

### Receiver restarted with fix

Ready for another streaming test. Please send again.
