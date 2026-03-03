import VideoToolbox
import CoreMedia
import CoreVideo
import Foundation

/// Hardware-accelerated H.264 encoder using VideoToolbox.
/// Configured for low-latency real-time encoding.
final class H264Encoder {
    private var compressionSession: VTCompressionSession?
    private var onEncodedNALU: ((_ naluData: Data, _ isKeyframe: Bool, _ pts: CMTime) -> Void)?
    private var frameCount: Int64 = 0

    let width: Int
    let height: Int
    let fps: Int
    let bitrate: Int // bits per second

    init(width: Int, height: Int, fps: Int = 60, bitrateMbps: Int = 15) {
        self.width = width
        self.height = height
        self.fps = fps
        self.bitrate = bitrateMbps * 1_000_000
    }

    /// Sets up the VideoToolbox compression session.
    func setup(onEncodedNALU: @escaping (_ naluData: Data, _ isKeyframe: Bool, _ pts: CMTime) -> Void) throws {
        self.onEncodedNALU = onEncodedNALU

        let encoderSpec: CFDictionary = [
            kVTVideoEncoderSpecification_EnableHardwareAcceleratedVideoEncoder: true
        ] as CFDictionary

        let pixelBufferAttrs: CFDictionary = [
            kCVPixelBufferPixelFormatTypeKey: kCVPixelFormatType_32BGRA,
            kCVPixelBufferWidthKey: width,
            kCVPixelBufferHeightKey: height,
        ] as CFDictionary

        var session: VTCompressionSession?
        let status = VTCompressionSessionCreate(
            allocator: kCFAllocatorDefault,
            width: Int32(width),
            height: Int32(height),
            codecType: kCMVideoCodecType_H264,
            encoderSpecification: encoderSpec,
            imageBufferAttributes: pixelBufferAttrs,
            compressedDataAllocator: nil,
            outputCallback: compressionOutputCallback,
            refcon: Unmanaged.passUnretained(self).toOpaque(),
            compressionSessionOut: &session
        )

        guard status == noErr, let session = session else {
            throw EncoderError.sessionCreationFailed(status)
        }

        // Configure for low-latency streaming
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_RealTime, value: kCFBooleanTrue)
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_ProfileLevel, value: kVTProfileLevel_H264_High_AutoLevel)
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_AverageBitRate, value: bitrate as CFNumber)
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_MaxKeyFrameInterval, value: (fps * 1) as CFNumber)
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_AllowFrameReordering, value: kCFBooleanFalse)
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_H264EntropyMode, value: kVTH264EntropyMode_CABAC)

        let dataRateLimit = [Double(bitrate) / 8.0, 1.0] as CFArray
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_DataRateLimits, value: dataRateLimit)

        VTCompressionSessionPrepareToEncodeFrames(session)
        self.compressionSession = session
        print("[Encoder] H.264 session created: \(width)x\(height) @ \(bitrate / 1_000_000)Mbps")
    }

    /// Encodes a single frame from the captured pixel buffer.
    func encode(sampleBuffer: CMSampleBuffer) {
        guard let session = compressionSession else { return }
        guard let pixelBuffer = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }

        let pts = CMTime(value: frameCount, timescale: CMTimeScale(fps))
        let duration = CMTime(value: 1, timescale: CMTimeScale(fps))
        frameCount += 1

        var flags = VTEncodeInfoFlags()
        let status = VTCompressionSessionEncodeFrame(
            session,
            imageBuffer: pixelBuffer,
            presentationTimeStamp: pts,
            duration: duration,
            frameProperties: nil,
            sourceFrameRefcon: nil,
            infoFlagsOut: &flags
        )

        if status != noErr {
            print("[Encoder] Encode failed: \(status)")
        }
    }

    fileprivate func handleEncodedFrame(status: OSStatus, flags: VTEncodeInfoFlags, sampleBuffer: CMSampleBuffer?) {
        guard status == noErr, let sampleBuffer = sampleBuffer else { return }

        // Detect keyframe via sample attachments
        let attachments = CMSampleBufferGetSampleAttachmentsArray(sampleBuffer, createIfNecessary: false)
        var isKeyframe = true
        if let attachments = attachments, CFArrayGetCount(attachments) > 0 {
            let dict = unsafeBitCast(CFArrayGetValueAtIndex(attachments, 0), to: CFDictionary.self)
            if CFDictionaryContainsKey(dict, Unmanaged.passUnretained(kCMSampleAttachmentKey_NotSync).toOpaque()) {
                isKeyframe = false
            }
        }

        let pts = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)

        // For keyframes, emit SPS/PPS first
        if isKeyframe, let formatDesc = CMSampleBufferGetFormatDescription(sampleBuffer) {
            emitParameterSets(formatDesc: formatDesc, pts: pts)
        }

        // Extract NALUs from the sample buffer
        guard let dataBuffer = CMSampleBufferGetDataBuffer(sampleBuffer) else { return }
        var totalLength = 0
        var dataPointer: UnsafeMutablePointer<CChar>?
        CMBlockBufferGetDataPointer(dataBuffer, atOffset: 0, lengthAtOffsetOut: nil, totalLengthOut: &totalLength, dataPointerOut: &dataPointer)

        guard let dataPointer = dataPointer else { return }

        var offset = 0
        while offset < totalLength {
            // Read AVCC length prefix (4 bytes big-endian)
            var naluLength: UInt32 = 0
            memcpy(&naluLength, dataPointer + offset, 4)
            naluLength = naluLength.bigEndian
            offset += 4

            guard Int(naluLength) + offset <= totalLength else { break }

            let naluData = Data(bytes: dataPointer + offset, count: Int(naluLength))
            onEncodedNALU?(naluData, isKeyframe, pts)
            offset += Int(naluLength)
        }
    }

    private func emitParameterSets(formatDesc: CMFormatDescription, pts: CMTime) {
        var parameterSetCount = 0
        CMVideoFormatDescriptionGetH264ParameterSetAtIndex(
            formatDesc, parameterSetIndex: 0,
            parameterSetPointerOut: nil, parameterSetSizeOut: nil,
            parameterSetCountOut: &parameterSetCount, nalUnitHeaderLengthOut: nil
        )

        for i in 0..<parameterSetCount {
            var parameterSetPointer: UnsafePointer<UInt8>?
            var parameterSetSize = 0
            let status = CMVideoFormatDescriptionGetH264ParameterSetAtIndex(
                formatDesc,
                parameterSetIndex: i,
                parameterSetPointerOut: &parameterSetPointer,
                parameterSetSizeOut: &parameterSetSize,
                parameterSetCountOut: nil,
                nalUnitHeaderLengthOut: nil
            )
            if status == noErr, let pointer = parameterSetPointer {
                let data = Data(bytes: pointer, count: parameterSetSize)
                onEncodedNALU?(data, true, pts)
            }
        }
    }

    /// Tears down the compression session.
    func teardown() {
        if let session = compressionSession {
            VTCompressionSessionInvalidate(session)
            compressionSession = nil
            print("[Encoder] Session torn down")
        }
    }

    deinit {
        teardown()
    }
}

/// C callback for VTCompressionSession output.
private func compressionOutputCallback(
    outputCallbackRefCon: UnsafeMutableRawPointer?,
    sourceFrameRefCon: UnsafeMutableRawPointer?,
    status: OSStatus,
    infoFlags: VTEncodeInfoFlags,
    sampleBuffer: CMSampleBuffer?
) {
    guard let refCon = outputCallbackRefCon else { return }
    let encoder = Unmanaged<H264Encoder>.fromOpaque(refCon).takeUnretainedValue()
    encoder.handleEncodedFrame(status: status, flags: infoFlags, sampleBuffer: sampleBuffer)
}

enum EncoderError: Error, CustomStringConvertible {
    case sessionCreationFailed(OSStatus)

    var description: String {
        switch self {
        case .sessionCreationFailed(let status):
            return "Failed to create VTCompressionSession: OSStatus \(status)"
        }
    }
}
