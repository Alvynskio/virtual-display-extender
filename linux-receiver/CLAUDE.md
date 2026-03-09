# Task for Claude on linux-receiver

## Context

We just switched the win-sender to all-intra encoding (GOP=1) so every frame is a complete I-frame. This eliminates inter-frame prediction artifacts on scene changes (browser tab switches etc). However, bitrates are now much higher:

- 1080p: 150 Mbps
- 1440p: 250 Mbps
- 4K: 500 Mbps

## Problem

The linux-receiver can't handle the higher bitrate all-intra stream well. You need to diagnose and fix the receiver-side bottleneck.

## What to investigate

1. **UDP receive buffer size** — `buffer-size=4194304` (4MB) on `udpsrc` may be too small for 500 Mbps bursts. At 500 Mbps, 4MB fills in ~64ms (fewer than 4 frames at 60fps). Consider increasing to 16-32MB. Also check/set the OS-level socket buffer: `sudo sysctl -w net.core.rmem_max=33554432` and `net.core.rmem_default=33554432`.

2. **Decoder performance** — All-intra at high bitrates is much more demanding on the decoder since every frame is a full I-frame decode. Check which decode chain is selected:
   - If it's `avdec_h264` (software), it may not keep up at 4K 500Mbps. Ensure a hardware decoder (vaapih264dec or vah264dec) is being used.
   - Consider adding `max-threads` or other tuning to the software decoder fallback.

3. **Pipeline buffering** — With larger frames, the pipeline may need more buffering between elements. Consider adding a `queue` AFTER the decoder (not before — we removed the pre-decoder leaky queue because it was causing artifacts by dropping encoded NALUs). A post-decoder queue with `leaky=downstream` is safe since it only drops rendered frames.

4. **Video sink** — Ensure `sync=false` is set (it already is). If using autovideosink, check what actual sink it picks. For best performance on Linux, vaapisink or a direct GL sink is ideal.

## Architecture reference

See the root `CLAUDE.md` for full project architecture. The receiver pipeline is:

```
udpsrc → rtpjitterbuffer → rtph264depay → {decoder} → {sink}
```

Key files:
- `src/pipeline.rs` — decode chain selection and pipeline construction
- `src/main.rs` — CLI mode pipeline (duplicated, same structure)

## Important

- Do NOT add a `leaky=downstream` queue BEFORE the decoder — we just removed that because it caused massive artifacts by dropping encoded H.264 NALUs
- A leaky queue AFTER the decoder is fine (drops decoded frames, doesn't corrupt the decode chain)
- The sender is already committed to all-intra/GOP=1 — the fix must be on the receiver side
- Build with `cargo build --release` and test with the win-sender streaming to this machine
