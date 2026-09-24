/*
 * aptX Low Latency Encoder Wrapper - Implementation
 *
 * Uses libopenaptx to encode PCM audio to standard aptX.
 * aptX LL uses the same encoding as standard aptX (4 stereo samples -> 4 bytes)
 * but signals low-latency mode in the A2DP codec configuration so that the
 * sink device uses a smaller buffer/latency target (~32ms vs ~150ms).
 * libopenaptx wants packed 24-bit LE input (LLLRRR x4 = 24 bytes per group);
 * see aptx_pcm_pack.h.
 *
 * Input:  interleaved PCM, stereo or mono (duplicated to L/R); signed 16-bit
 *         by default, or 32-bit MSB-aligned after set_bit_depth(24|32)
 * Output: raw aptX stream. Like classic aptX, aptX LL media packets carry NO
 *         RTP header (PipeWire a2dp-codec-aptx.c: RTP only for aptX HD).
 *
 * SPDX-License-Identifier: MIT
 */

#include "aptxll_encoder.h"
#include <cstdio>
#include <cstring>

#include "aptx_pcm_pack.h"

extern "C" {
#include "openaptx.h"
}

/*
 * Standard aptX encoding parameters (same encoding as aptX LL):
 *   - Processes 4 PCM samples per channel at a time
 *   - Output: 4 bytes per 4 stereo samples (16-bit aptX)
 *   - Fixed bitrate: 352 kbps at 44.1 kHz stereo
 */
static constexpr uint32_t APTX_ENCODED_BYTES_PER_FRAME = 4; /* 16-bit: 2 bytes/ch */

AptxLlEncoder::AptxLlEncoder() = default;

AptxLlEncoder::~AptxLlEncoder() {
    shutdown();
}

void AptxLlEncoder::set_bit_depth(int bits) {
    /* 24 or 32: 32-bit MSB-aligned container (see aptx_pcm_pack.h); else 16-bit */
    bytes_per_sample_ = (bits == 24 || bits == 32) ? 4u : 2u;
}

bool AptxLlEncoder::init(uint16_t mtu, EncoderQuality quality,
                          uint32_t sample_rate, uint32_t channels) {
    (void)quality; /* aptX LL has fixed bitrate */

    if (initialized_) {
        shutdown();
    }

    if (channels != 1 && channels != 2) {
        fprintf(stderr, "AptxLlEncoder: Only stereo (2 channels) is supported\n");
        return false;
    }

    if (sample_rate != 44100 && sample_rate != 48000) {
        fprintf(stderr, "AptxLlEncoder: Unsupported sample rate %u (use 44100 or 48000)\n",
                sample_rate);
        return false;
    }

    /* Create standard aptX encoder context (0 = standard aptX, not HD) */
    ctx_ = aptx_init(0);
    if (!ctx_) {
        fprintf(stderr, "AptxLlEncoder: Failed to initialize aptX context\n");
        return false;
    }

    mtu_ = mtu;
    sample_rate_ = sample_rate;
    channels_ = channels;
    initialized_ = true;

    fprintf(stderr, "AptxLlEncoder: Initialized (rate=%u, ch=%u, mtu=%u, bitrate=352kbps, low-latency)\n",
           sample_rate, channels, mtu);
    return true;
}

bool AptxLlEncoder::encode(const uint8_t *pcm_data, uint32_t pcm_bytes,
                         uint8_t *out_data, uint32_t *out_size,
                         uint32_t *out_frames) {
    if (!initialized_ || !ctx_) {
        return false;
    }

    /* One aptX group = 4 sample-frames of the (interleaved) input */
    uint32_t bytes_per_frame_input = APTX_GROUP_SAMPLES * channels_ * bytes_per_sample_;
    uint32_t num_frames = pcm_bytes / bytes_per_frame_input;
    if (num_frames == 0) {
        *out_size = 0;
        *out_frames = 0;
        return true;
    }

    /* mtu_ is the media MTU (RTP header already subtracted); conservative for no-RTP aptX LL */
    uint32_t max_frames = mtu_ / APTX_ENCODED_BYTES_PER_FRAME;
    if (num_frames > max_frames && max_frames > 0) {
        num_frames = max_frames;
    }

    uint32_t required_out = num_frames * APTX_ENCODED_BYTES_PER_FRAME;
    if (*out_size < required_out) {
        fprintf(stderr, "AptxLlEncoder: Output buffer too small (%u < %u)\n",
                *out_size, required_out);
        return false;
    }

    uint32_t out_offset = 0;
    for (uint32_t f = 0; f < num_frames; f++) {
        /* libopenaptx wants packed 24-bit LE LLLRRR x4 (24 bytes) */
        uint8_t packed[APTX_GROUP_PACKED_BYTES];
        aptx_pack_group(pcm_data, f, channels_, bytes_per_sample_, packed);

        size_t written = 0;
        size_t processed = aptx_encode(
            static_cast<struct aptx_context *>(ctx_),
            packed, sizeof(packed),
            out_data + out_offset, APTX_ENCODED_BYTES_PER_FRAME,
            &written);

        if (processed != sizeof(packed) || written != APTX_ENCODED_BYTES_PER_FRAME) {
            fprintf(stderr, "AptxLlEncoder: Encode failed at frame %u\n", f);
            return false;
        }

        out_offset += static_cast<uint32_t>(written);
    }

    *out_size = out_offset;
    *out_frames = num_frames;
    return true;
}

uint32_t AptxLlEncoder::get_pcm_frames_per_encode() const {
    /*
     * Batch 128 PCM frames per encode call (32 aptX frames).
     * Produces ~128 bytes per packet instead of 4 bytes, avoiding
     * massive RTP header overhead from per-4-sample packets.
     */
    return 128;
}

void AptxLlEncoder::shutdown() {
    if (ctx_) {
        aptx_finish(static_cast<struct aptx_context *>(ctx_));
        ctx_ = nullptr;
    }
    initialized_ = false;
}
