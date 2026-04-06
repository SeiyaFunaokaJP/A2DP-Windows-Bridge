/*
 * aptX Low Latency Encoder Wrapper - Implementation
 *
 * Uses libopenaptx to encode 16-bit PCM audio to standard aptX.
 * aptX LL uses the same encoding as standard aptX (4 stereo samples -> 4 bytes)
 * but signals low-latency mode in the A2DP codec configuration so that the
 * sink device uses a smaller buffer/latency target (~32ms vs ~150ms).
 *
 * Input:  signed 16-bit PCM, interleaved stereo
 * Output: aptX encoded frames for AVDTP media transport
 *
 * SPDX-License-Identifier: MIT
 */

#include "aptxll_encoder.h"
#include <cstdio>
#include <cstring>

extern "C" {
#include "openaptx.h"
}

/*
 * Standard aptX encoding parameters (same encoding as aptX LL):
 *   - Processes 4 PCM samples per channel at a time
 *   - Output: 4 bytes per 4 stereo samples (16-bit aptX)
 *   - Fixed bitrate: 352 kbps at 44.1 kHz stereo
 */
static constexpr uint32_t APTX_SAMPLES_PER_FRAME = 4;
static constexpr uint32_t APTX_ENCODED_BYTES_PER_FRAME = 4; /* 16-bit: 2 bytes/ch */

AptxLlEncoder::AptxLlEncoder() = default;

AptxLlEncoder::~AptxLlEncoder() {
    shutdown();
}

bool AptxLlEncoder::init(uint16_t mtu, EncoderQuality quality,
                          uint32_t sample_rate, uint32_t channels) {
    (void)quality; /* aptX LL has fixed bitrate */

    if (initialized_) {
        shutdown();
    }

    if (channels != 2) {
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

    /*
     * Standard aptX expects int32_t samples (16-bit sign-extended to 32-bit).
     * Input layout (interleaved stereo): L0 R0 L1 R1 L2 R2 L3 R3
     * aptX processes 4 samples per channel per frame.
     */
    uint32_t samples_per_frame = APTX_SAMPLES_PER_FRAME * channels_;
    uint32_t bytes_per_sample = sizeof(int16_t);
    uint32_t bytes_per_frame_input = samples_per_frame * bytes_per_sample;

    uint32_t num_frames = pcm_bytes / bytes_per_frame_input;
    if (num_frames == 0) {
        *out_size = 0;
        *out_frames = 0;
        return true;
    }

    /* Limit output to fit MTU (leave room for RTP + media payload header) */
    uint32_t max_payload = mtu_ > 15 ? mtu_ - 15 : mtu_;
    uint32_t max_frames = max_payload / APTX_ENCODED_BYTES_PER_FRAME;
    if (num_frames > max_frames && max_frames > 0) {
        num_frames = max_frames;
    }

    uint32_t required_out = num_frames * APTX_ENCODED_BYTES_PER_FRAME;
    if (*out_size < required_out) {
        fprintf(stderr, "AptxLlEncoder: Output buffer too small (%u < %u)\n",
                *out_size, required_out);
        return false;
    }

    const int16_t *pcm16 = reinterpret_cast<const int16_t *>(pcm_data);
    uint32_t out_offset = 0;

    for (uint32_t f = 0; f < num_frames; f++) {
        /* Prepare 4 stereo samples as int32_t[4][2] (sign-extended from 16-bit) */
        int32_t samples[4][2];
        for (uint32_t s = 0; s < APTX_SAMPLES_PER_FRAME; s++) {
            uint32_t idx = (f * APTX_SAMPLES_PER_FRAME + s) * channels_;
            samples[s][0] = static_cast<int32_t>(pcm16[idx + 0]);   /* Left */
            samples[s][1] = static_cast<int32_t>(pcm16[idx + 1]);   /* Right */
        }

        /* Encode one aptX frame (4 stereo samples -> 4 bytes) */
        size_t written = 0;
        size_t processed = aptx_encode(
            static_cast<struct aptx_context *>(ctx_),
            reinterpret_cast<const unsigned char *>(samples),
            sizeof(samples),
            out_data + out_offset,
            APTX_ENCODED_BYTES_PER_FRAME,
            &written
        );

        if (processed == 0) {
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
