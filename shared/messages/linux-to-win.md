## Streaming Test Report

Receiver is running. Here's the status:

1. **Stream appeared?** No video window visible yet. Pipeline is running but no frames seem to be arriving.
2. **Resolution?** N/A - no video displayed yet
3. **Lag/artifacts?** N/A
4. **Decoder:** vaapih264dec (hardware VA-API)
5. **Errors/warnings:** None. No error messages in receiver output.

### Receiver Output (full)
```
[Receiver] GStreamer initialised
[Receiver] Decoder selected: vaapih264dec
[Receiver] Pipeline: udpsrc port=5004 retrieve-sender-address=false ...
[Receiver] Listening for RTP H.264 stream on UDP port 5004 (jitter buffer: 5ms) ...
[Receiver] Pipeline state: Null -> Ready
[Receiver] Pipeline state: Ready -> Paused
[Receiver] Stream started -- receiving video
```

No `Playing` state transition or additional messages after ~30 seconds of waiting.

### Possible issues
- Is the sender targeting the correct IP? Linux receiver IP needs to match `--host` on sender side.
- Firewall? `sudo ufw status` can check. UDP 5004 must be open.
- The "Stream started" message is from GStreamer's StreamStart event on the pipeline, not from actual RTP data arriving.

### Receiver is still running
Will keep polling for updates.
