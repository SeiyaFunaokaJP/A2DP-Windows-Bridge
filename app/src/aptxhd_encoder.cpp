/*
 * aptX HD Encoder Wrapper - Implementation
 *
 * Uses libopenaptx to encode 24-bit PCM audio to aptX HD.
 * aptX HD encodes 4 stereo samples (4 per channel) into 6 bytes.
 * libopenaptx wants packed 24-bit LE input (LLLRRR x4 = 24 bytes per group);
 * see aptx_pcm_pack.h.
 *
 * Input:  interleaved PCM, stereo or mono (duplicated to L/R); signed 16-bit
 *         by default, or 32-bit MSB-aligned after set_bit_depth(24|32), which
 *         keeps the full 24-bit resolution aptX HD can carry
 * Output: aptX HD encoded frames (sent WITH an RTP header, like Android/PipeWire)
 *
 * SPDX-License-Identifier: MIT
 */

#include "aptxhd_encoder.h"
#include <cstdio>
#include <cstring>

#include "aptx_pcm_pack.h"

extern "C" {
#include "openaptx.h"
}

/*
 * aptX HD encoding parameters:
 *   - Processes 4 PCM samples per channel at a time
 *   - Output: 6 bytes per 4 stereo samples (24-bit aptX HD)
 *   - Fixed bitrate: 576 kbps at 48 kHz stereo
 */
static constexpr uint32_t APTXHD_ENCODED_BYTES_PER_FRAME = 6; /* 24-bit HD: 3 bytes/ch */

AptxHdEncoder::AptxHdEncoder() = default;

AptxHdEncoder::~AptxHdEncoder() {
    shutdown();
}

void AptxHdEncoder::set_bit_depth(int bits) {
    /* 24 or 32: 32-bit MSB-aligned container (see aptx_pcm_pack.h); else 16-bit */
    bytes_per_sample_ = (bits == 24 || bits == 32) ? 4u : 2u;
}

bool AptxHdEncoder::init(uint16_t mtu, EncoderQuality quality,
                          uint32_t sample_rate, uint32_t channels) {
    (void)quality; /* aptX HD has fixed bitrate */

    if (initialized_) {
        shutdown();
    }

    if (channels != 1 && channels != 2) {
        fprintf(stderr, "AptxHdEncoder: Only stereo (2 channels) is supported\n");
        return false;
    }

    if (sample_rate != 44100 && sample_rate != 48000) {
        fprintf(stderr, "AptxHdEncoder: Unsupported sample rate %u (use 44100 or 48000)\n",
                sample_rate);
        return false;
    }

    /* Create aptX HD encoder context */
    ctx_ = aptx_init(1); /* 1 = aptX HD mode */
    if (!ctx_) {
        fprintf(stderr, "AptxHdEncoder: Failed to initialize aptX HD context\n");
        return false;
    }

    mtu_ = mtu;
    sample_rate_ = sample_rate;
    channels_ = channels;
    initialized_ = true;

    fprintf(stderr, "AptxHdEncoder: Initialized (rate=%u, ch=%u, mtu=%u, bitrate=576kbps)\n",
           sample_rate, channels, mtu);
    return true;
}

bool AptxHdEncoder::encode(const uint8_t *pcm_data, uint32_t pcm_bytes,
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

    /* mtu_ is the media MTU with the RTP header already subtracted (aptX HD has no extra media header) */
    uint32_t max_frames = mtu_ / APTXHD_ENCODED_BYTES_PER_FRAME;
    if (num_frames > max_frames && max_frames > 0) {
        num_frames = max_frames;
    }

    uint32_t required_out = num_frames * APTXHD_ENCODED_BYTES_PER_FRAME;
    if (*out_size < required_out) {
        fprintf(stderr, "AptxHdEncoder: Output buffer too small (%u < %u)\n",
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
            out_data + out_offset, APTXHD_ENCODED_BYTES_PER_FRAME,
            &written);

        if (processed != sizeof(packed) || written != APTXHD_ENCODED_BYTES_PER_FRAME) {
            fprintf(stderr, "AptxHdEncoder: Encode failed at frame %u\n", f);
            return false;
        }

        out_offset += static_cast<uint32_t>(written);
    }

    *out_size = out_offset;
    *out_frames = num_frames;
    return true;
}

uint32_t AptxHdEncoder::get_pcm_frames_per_encode() const {
    /*
     * Batch 128 PCM frames per encode call (32 aptX HD frames).
     * Produces ~192 bytes per packet instead of 6 bytes, avoiding
     * massive RTP header overhead from per-4-sample packets.
     */
    return 128;
}

void AptxHdEncoder::shutdown() {
    if (ctx_) {
        aptx_finish(static_cast<struct aptx_context *>(ctx_));
        ctx_ = nullptr;
    }
    initialized_ = false;
}
