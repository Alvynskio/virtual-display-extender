import Foundation
import Network
import CoreMedia

/// Sends H.264 NALUs as RTP packets over UDP.
/// Implements RFC 6184 (H.264 RTP payload) with FU-A fragmentation.
final class RTPStreamer {
    private var connection: NWConnection?
    private var sequenceNumber: UInt16 = 0
    private let ssrc: UInt32
    private let payloadType: UInt8 = 96 // dynamic PT for H.264
    private let clockRate: UInt32 = 90000
    private let maxPayloadSize = 1200 // bytes, leaving room for headers

    let host: String
    let port: UInt16

    init(host: String, port: UInt16 = 5004) {
        self.host = host
        self.port = port
        self.ssrc = UInt32.random(in: 0...UInt32.max)
    }

    /// Opens the UDP connection to the receiver.
    func start() {
        let endpoint = NWEndpoint.hostPort(
            host: NWEndpoint.Host(host),
            port: NWEndpoint.Port(rawValue: port)!
        )
        let params = NWParameters.udp
        params.allowLocalEndpointReuse = true

        let connection = NWConnection(to: endpoint, using: params)
        connection.stateUpdateHandler = { state in
            switch state {
            case .ready:
                print("[Streamer] Connected to \(self.host):\(self.port)")
            case .failed(let error):
                print("[Streamer] Connection failed: \(error)")
            default:
                break
            }
        }
        connection.start(queue: DispatchQueue(label: "rtp-streamer", qos: .userInteractive))
        self.connection = connection
    }

    /// Sends a NALU as one or more RTP packets.
    /// - Parameters:
    ///   - naluData: Raw NALU bytes (including NAL header byte)
    ///   - isLastNALUInFrame: Set true for the last NALU of an access unit (sets RTP marker bit)
    ///   - timestamp: Presentation timestamp, converted to 90kHz RTP clock
    func send(naluData: Data, isLastNALUInFrame: Bool, timestamp: CMTime) {
        let rtpTimestamp = UInt32(CMTimeGetSeconds(timestamp) * Double(clockRate))

        if naluData.count <= maxPayloadSize {
            // Single NAL unit packet
            sendRTPPacket(payload: naluData, marker: isLastNALUInFrame, timestamp: rtpTimestamp)
        } else {
            // FU-A fragmentation
            sendFragmented(naluData: naluData, marker: isLastNALUInFrame, timestamp: rtpTimestamp)
        }
    }

    private func sendFragmented(naluData: Data, marker: Bool, timestamp: UInt32) {
        guard naluData.count > 0 else { return }

        let nalHeader = naluData[naluData.startIndex]
        let nalRefIdc = nalHeader & 0x60
        let nalType = nalHeader & 0x1F
        let fuIndicator: UInt8 = nalRefIdc | 28 // FU-A type = 28

        // Skip the NAL header byte for the payload
        let payload = naluData.dropFirst()
        let fragmentCount = (payload.count + maxPayloadSize - 2) / (maxPayloadSize - 2) // -2 for FU indicator + header

        var offset = payload.startIndex
        for i in 0..<fragmentCount {
            let isFirst = (i == 0)
            let isLast = (i == fragmentCount - 1)
            let chunkSize = min(maxPayloadSize - 2, payload.endIndex - offset)

            var fuHeader: UInt8 = nalType
            if isFirst { fuHeader |= 0x80 } // Start bit
            if isLast { fuHeader |= 0x40 }  // End bit

            var fragment = Data(capacity: 2 + chunkSize)
            fragment.append(fuIndicator)
            fragment.append(fuHeader)
            fragment.append(payload[offset..<(offset + chunkSize)])

            let setMarker = isLast && marker
            sendRTPPacket(payload: fragment, marker: setMarker, timestamp: timestamp)
            offset += chunkSize
        }
    }

    private func sendRTPPacket(payload: Data, marker: Bool, timestamp: UInt32) {
        var header = Data(count: 12)

        // Byte 0: V=2, P=0, X=0, CC=0
        header[0] = 0x80

        // Byte 1: M + PT
        header[1] = (marker ? 0x80 : 0x00) | payloadType

        // Bytes 2-3: Sequence number (big-endian)
        header[2] = UInt8(sequenceNumber >> 8)
        header[3] = UInt8(sequenceNumber & 0xFF)
        sequenceNumber &+= 1

        // Bytes 4-7: Timestamp (big-endian)
        header[4] = UInt8((timestamp >> 24) & 0xFF)
        header[5] = UInt8((timestamp >> 16) & 0xFF)
        header[6] = UInt8((timestamp >> 8) & 0xFF)
        header[7] = UInt8(timestamp & 0xFF)

        // Bytes 8-11: SSRC (big-endian)
        header[8] = UInt8((ssrc >> 24) & 0xFF)
        header[9] = UInt8((ssrc >> 16) & 0xFF)
        header[10] = UInt8((ssrc >> 8) & 0xFF)
        header[11] = UInt8(ssrc & 0xFF)

        var packet = header
        packet.append(payload)

        connection?.send(content: packet, completion: .contentProcessed({ error in
            if let error = error {
                print("[Streamer] Send error: \(error)")
            }
        }))
    }

    /// Closes the UDP connection.
    func stop() {
        connection?.cancel()
        connection = nil
        print("[Streamer] Stopped")
    }
}
