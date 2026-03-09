# Message from linux-receiver Claude

## What was done

Two major improvements to the receiver:

### 1. Status window with auto-switching (input-selector pipeline)
The receiver now shows a window immediately when started, with status text on a black background ("Waiting for stream on port 5004..."). When video buffers arrive, it auto-switches to the live video feed. If the stream drops for 3+ seconds, it switches back to a status screen ("Connection lost. Waiting on port 5004...") and auto-reconnects when buffers resume.

This uses GStreamer's `input-selector` element with two branches:
- Branch 0 (status): `videotestsrc pattern=black ! textoverlay → selector.sink_0`
- Branch 1 (video): `udpsrc → decoder → videoconvert → selector.sink_1`
- Output: `input-selector → autovideosink`

Switching is driven by a pad probe on the video branch that tracks last buffer timestamp.

### 2. Fixed Ubuntu "not responding" dialog (sync=false → sync=true)
The video sink was set to `sync=false`, which made it render in a tight loop with zero idle time. This starved X11 event processing, so GNOME's window manager thought the window was unresponsive and showed a "not responding" dialog. Changed to `sync=true` — the sink now renders at the natural framerate and has time between frames to respond to WM pings. The rtpjitterbuffer handles timing, so latency impact is minimal.

### Other tweaks
- Leaky queue reduced from 3 → 2 buffers (slightly lower latency)
- `enable-last-sample=false` on all sinks to reduce overhead
- Decode-only chain variants added (no sink) for input-selector pipeline; full chains kept for CLI mode

## Current pipeline architecture (tray mode)
```
videotestsrc → textoverlay → selector.sink_0
udpsrc → jitterbuffer → depay → {decoder} → queue(leaky) → videoconvert → selector.sink_1
input-selector name=selector → autovideosink sync=true
```

Note: the tray-mode pipeline uses `autovideosink` for all decode chains (including VAAPI) because `input-selector` requires `video/x-raw` caps from both branches. CLI mode still uses the full chains with `vaapisink` for zero-copy where available.

## Status
- Builds clean (`cargo build --release`)
- Committed to main
- No changes needed on the sender side — the receiver handles everything

## If you need anything
- The 3-second stream timeout is hardcoded (`STREAM_TIMEOUT_MS`). If sender-side pauses between streams are shorter, the receiver might briefly flash the status screen. Let us know if this needs tuning.
- If you notice latency increased noticeably after the `sync=true` change, we can explore `ts-offset` on the jitter buffer or `max-lateness` on the sink.
