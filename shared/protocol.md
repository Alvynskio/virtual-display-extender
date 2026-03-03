# RTP/UDP Streaming Protocol

## Overview

H.264 video is streamed from macOS sender to Linux receiver using RTP over UDP, following RFC 3550 (RTP) and RFC 6184 (RTP Payload Format for H.264).

## Connection Flow

1. Sender starts listening for receiver announcements via mDNS (`_vdext._udp.local.`)
2. Receiver advertises itself with its IP, preferred resolution, and listening port
3. Sender creates the virtual display at the negotiated resolution
4. Sender begins streaming RTP packets to the receiver's IP:port
5. Receiver sends periodic RTCP receiver reports for quality feedback

## Default Ports

| Port  | Protocol | Purpose              |
|-------|----------|----------------------|
| 5004  | UDP      | RTP video stream     |
| 5005  | UDP      | RTCP control channel |
| 5353  | UDP      | mDNS discovery       |

## RTP Packet Format

Standard RTP header (12 bytes) per RFC 3550:

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|V=2|P|X|  CC   |M|     PT      |       sequence number         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                           timestamp                           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                             SSRC                              |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

- **Payload Type (PT):** 96 (dynamic, H.264)
- **Clock rate:** 90000 Hz
- **SSRC:** randomly generated per session

## H.264 NAL Unit Packaging (RFC 6184)

### Single NAL Unit Mode
For NALUs ≤ 1200 bytes (fits in single UDP packet with headers):
- RTP payload = single NALU (including NAL header byte)

### Fragmentation Unit (FU-A) Mode
For NALUs > 1200 bytes:
- Split into fragments, each prefixed with FU indicator + FU header
- FU indicator: `(nal_ref_idc & 0x60) | 28`
- FU header: `S|E|R|Type` (Start, End, Reserved, original NAL type)
- RTP marker bit (M) set on last fragment of an access unit

## MTU Considerations

- Target MTU: 1400 bytes (safe for most networks)
- Max RTP payload: 1200 bytes (leaving room for RTP/UDP/IP headers)

## Encoder Configuration

| Parameter            | Value                    |
|----------------------|--------------------------|
| Profile              | High                     |
| Level                | 4.1                      |
| Entropy coding       | CABAC                    |
| Rate control         | CBR                      |
| Target bitrate       | 15 Mbps (1080p) / 30 Mbps (1440p) |
| Keyframe interval    | 60 frames (1 per second at 60fps) |
| B-frames             | 0 (low latency)          |
| Slice mode            | Single slice per frame   |
| Latency mode         | Real-time                |

## Control Messages (RTCP)

### Receiver Report (sent by Linux receiver)
- Packet loss statistics
- Jitter measurements
- Used by sender to adapt bitrate

### Custom App-Defined Messages
Used for resolution negotiation and session control:

```
Type: APP (204)
Name: "VDXT"
Subtypes:
  0x01 = Resolution request  (receiver → sender)
  0x02 = Resolution confirm  (sender → receiver)
  0x03 = Session end          (either direction)
```

Resolution request payload:
```
| width (u16) | height (u16) | fps (u8) | reserved (3 bytes) |
```
