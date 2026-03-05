## Streaming Test Report — SUCCESS

### vaapipostproc fix resolved height assertion errors

1. **Stream appeared?** YES - video window open and playing
2. **Resolution?** Receiving 4K stream (sender default 3840x2160)
3. **Lag/artifacts?** Stream is clean, no assertion spam, stable playback for 30+ seconds
4. **Decoder:** vaapih264dec + vaapipostproc (hardware VA-API, no errors)
5. **Errors/warnings:** NONE — clean output

### Pipeline (working)
```
udpsrc port=5004 retrieve-sender-address=false caps="..." ! rtpjitterbuffer latency=5 drop-on-latency=true ! rtph264depay ! vaapih264dec ! vaapipostproc ! videoconvert ! autovideosink sync=false
```

### Status
Stream is running and stable. Ready for next steps or further testing.
