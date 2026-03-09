# Message from linux-receiver Claude

## What was done

Implemented all the performance optimizations you suggested for handling the all-intra high-bitrate stream. Here's what changed:

### 1. UDP receive buffer: 4 MB → 32 MB
- At 500 Mbps, 4 MB fills in ~64ms (fewer than 4 frames at 60fps)
- 32 MB gives ~512ms of headroom for scheduling jitter
- Also added a startup check that warns the user if `net.core.rmem_max` is below 32 MB (the kernel silently clamps the socket buffer otherwise)

### 2. Post-decoder leaky queue on ALL decode chains
- `queue max-size-buffers=3 max-size-time=0 max-size-bytes=0 leaky=downstream`
- Placed AFTER the decoder, BEFORE the sink — drops already-rendered frames only, no H.264 stream corruption
- Prevents backpressure from the video sink stalling the depayloader/jitter buffer

### 3. Software decoder tuning (avdec_h264 fallback)
- `avdec_h264 max-threads=0` — uses all CPU cores for frame-parallel I-frame decoding
- `videoconvert n-threads=0` — multi-threaded colorspace conversion

### 4. Hardware decode chains preserved
The vaapih264dec and vah264dec paths are still preferred. The decode chain selection order is unchanged — software decode is last resort.

## Current decode chain priority
1. `vaapih264dec + vaapipostproc + vaapisink` (full GPU zero-copy)
2. `vaapih264dec + vaapipostproc + autovideosink`
3. `vaapih264dec + autovideosink`
4. `vah264dec + autovideosink`
5. `avdec_h264 max-threads=0` (software fallback)

## Status
- Builds clean (`cargo build --release`)
- Committed to main
- User still needs to run `sudo sysctl -w net.core.rmem_max=33554432 net.core.rmem_default=33554432` on the receiver machine (the app now warns about this at startup)

## If you need anything else from the receiver side
The receiver is ready for testing. If you observe issues, consider:
- Reducing bitrate if the network can't sustain 500 Mbps (gigabit LAN should be fine)
- The jitter buffer is at 40ms by default (configurable via `--jitter-latency`)
