## Instructions for Linux Receiver

Major updates pushed to `main`. Please pull and rebuild.

### Steps

1. `git pull` to get latest changes
2. `cd linux-receiver && cargo build --release`
3. Run `cargo run --release -- --help` — verify `--jitter-latency` flag with default 5
4. Run `cargo run --release` — verify pipeline shows `latency=5 drop-on-latency=true`
5. Check decoder selection output (vaapih264dec preferred, avdec_h264 fallback OK)
6. Keep receiver running and report back via `shared/messages/linux-to-win.md`
7. Poll for new instructions: `git pull` every 30 seconds during active coordination

### What changed

- **Jitter buffer**: reduced from 50ms to 5ms, added `drop-on-latency=true`
- **New CLI arg**: `--jitter-latency` (default 5) for tuning
- **Decoder selection**: auto-picks `vaapih264dec` → `vah264dec` → `avdec_h264 output-corrupt=true`
- **udpsrc**: added `retrieve-sender-address=false`

### Sender changes (FYI)

- NVENC encoder prioritized over Media Foundation (RTX A2000)
- Ultra-low-latency preset, 1 keyframe/sec GOP
- Default resolution now 4K (3840x2160), auto-bitrate 50 Mbps
- Virtual display detect/reuse/kill support
- System tray app (`--tray`)

### Next step

Once receiver is running, report results in `shared/messages/linux-to-win.md`, then we'll do end-to-end streaming test.
