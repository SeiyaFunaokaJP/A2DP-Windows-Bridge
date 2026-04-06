/*
 * aptX HD Encoder Wrapper - Implementation
 *
 * Uses libopenaptx to encode 24-bit PCM audio to aptX HD.
 * aptX HD encodes 4 stereo samples (4 per channel) into 6 bytes.
 *
 * Input:  signed 16-bit PCM (upsampled to 24-bit internally)
 * Output: aptX HD encoded frames, packed for AVDTP media transport
 *
 * SPDX-License-Identifier: MIT
 */

#include "aptxhd_encoder.h"
#include <cstdio>
#include <cstring>
#include <vector>

extern "C" {
#include "openaptx.h"
}

/*
 * aptX HD encoding parameters:
 *   - Processes 4 PCM samples per channel at a time
 *   - Output: 6 bytes per 4 stereo samples (24-bit aptX HD)
 *   - Fixed bitrate: 576 kbps at 48 kHz stereo
 */
static constexpr uint32_t APTXHD_SAMPLES_PER_FRAME = 4;
static constexpr uint32_t APTXHD_ENCODED_BYTES_PER_FRAME = 6; /* 24-bit HD: 3 bytes/ch */

AptxHdEncoder::AptxHdEncoder() = default;

AptxHdEncoder::~AptxHdEncoder() {
    shutdown();
}

bool AptxHdEncoder::init(uint16_t mtu, EncoderQuality quality,
                          uint32_t sample_rate, uint32_t channels) {
    (void)quality; /* aptX HD has fixed bitrate */

    if (initialized_) {
        shutdown();
    }

    if (channels != 2) {
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

    /*
     * aptX HD expects 24-bit samples packed as int32_t (sign-extended).
     * We receive 16-bit PCM, so we left-shift by 8 to convert to 24-bit.
     *
     * Input layout (interleaved stereo): L0 R0 L1 R1 L2 R2 L3 R3
     * aptX HD processes 4 samples per channel per frame.
     */
    uint32_t samples_per_frame = APTXHD_SAMPLES_PER_FRAME * channels_;
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
    uint32_t max_frames = max_payload / APTXHD_ENCODED_BYTES_PER_FRAME;
    if (num_frames > max_frames && max_frames > 0) {
        num_frames = max_frames;
    }

    /* Check output buffer size */
    uint32_t required_out = num_frames * APTXHD_ENCODED_BYTES_PER_FRAME;
    if (*out_size < required_out) {
        fprintf(stderr, "AptxHdEncoder: Output buffer too small (%u < %u)\n",
                *out_size, required_out);
        return false;
    }

    /* Convert 16-bit PCM to 24-bit (as int32_t) and encode */
    const int16_t *pcm16 = reinterpret_cast<const int16_t *>(pcm_data);
    uint32_t out_offset = 0;

    for (uint32_t f = 0; f < num_frames; f++) {
        /* Prepare 4 stereo samples as int32_t[4][2] (24-bit, sign-extended) */
        int32_t samples[4][2];
        for (uint32_t s = 0; s < APTXHD_SAMPLES_PER_FRAME; s++) {
            uint32_t idx = (f * APTXHD_SAMPLES_PER_FRAME + s) * channels_;
            samples[s][0] = static_cast<int32_t>(pcm16[idx + 0]) << 8;     /* Left */
            samples[s][1] = static_cast<int32_t>(pcm16[idx + 1]) << 8;     /* Right */
        }

        /* Encode one aptX HD frame (4 stereo samples -> 6 bytes) */
        size_t written = 0;
        size_t processed = aptx_encode(
            static_cast<struct aptx_context *>(ctx_),
            reinterpret_cast<const unsigned char *>(samples),
            sizeof(samples),
            out_data + out_offset,
            APTXHD_ENCODED_BYTES_PER_FRAME,
            &written
        );

        if (processed == 0) {
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
