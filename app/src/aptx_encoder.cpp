/*
 * aptX (classic) Encoder Wrapper - Implementation
 *
 * Uses libopenaptx (aptx_init(0)) to encode PCM audio to classic aptX.
 *
 * libopenaptx API contract (openaptx.h):
 *   aptx_encode() consumes groups of 4 stereo samples given as PACKED 24-bit
 *   signed little-endian values (24 bytes: LLLRRRLLLRRRLLLRRRLLLRRR) and
 *   emits 4 bytes (LLRR) per group for aptX (6 bytes for aptX HD).
 *   Packing is done by aptx_pcm_pack.h.
 *
 * Input:  interleaved PCM, stereo or mono (duplicated to L/R); signed 16-bit
 *         by default, or 32-bit MSB-aligned after set_bit_depth(24|32)
 * Output: raw aptX stream. Classic aptX media packets carry NO RTP header
 *         (see BtStackTransport::send_media / CAN_SEND_MEDIA_PACKET_NOW).
 *
 * SPDX-License-Identifier: MIT
 */

#include "aptx_encoder.h"
#include <cstdio>
#include <cstring>

#include "aptx_pcm_pack.h"

extern "C" {
#include "openaptx.h"
}

static constexpr uint32_t APTX_ENCODED_BYTES_PER_FRAME = 4;  /* 2 bytes/ch */

AptxEncoder::AptxEncoder() = default;

AptxEncoder::~AptxEncoder() {
    shutdown();
}

void AptxEncoder::set_bit_depth(int bits) {
    /* 24 or 32: 32-bit MSB-aligned container (see aptx_pcm_pack.h); else 16-bit */
    bytes_per_sample_ = (bits == 24 || bits == 32) ? 4u : 2u;
}

bool AptxEncoder::init(uint16_t mtu, EncoderQuality quality,
                       uint32_t sample_rate, uint32_t channels) {
    (void)quality; /* aptX has a fixed bitrate */

    if (initialized_) {
        shutdown();
    }

    if (channels != 1 && channels != 2) {
        fprintf(stderr, "AptxEncoder: Unsupported channel count %u\n", channels);
        return false;
    }

    if (sample_rate != 44100 && sample_rate != 48000) {
        fprintf(stderr, "AptxEncoder: Unsupported sample rate %u (use 44100 or 48000)\n",
                sample_rate);
        return false;
    }

    ctx_ = aptx_init(0); /* 0 = classic aptX */
    if (!ctx_) {
        fprintf(stderr, "AptxEncoder: Failed to initialize aptX context\n");
        return false;
    }

    mtu_ = mtu;
    sample_rate_ = sample_rate;
    channels_ = channels;
    initialized_ = true;

    fprintf(stderr, "AptxEncoder: Initialized (rate=%u, in_ch=%u, mtu=%u, bitrate=%ukbps)\n",
            sample_rate, channels, mtu, get_bitrate_kbps());
    return true;
}

bool AptxEncoder::encode(const uint8_t *pcm_data, uint32_t pcm_bytes,
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

    /* mtu_ is the media MTU (RTP header already subtracted); conservative for no-RTP aptX */
    uint32_t max_frames = mtu_ / APTX_ENCODED_BYTES_PER_FRAME;
    if (num_frames > max_frames && max_frames > 0) {
        num_frames = max_frames;
    }

    uint32_t required_out = num_frames * APTX_ENCODED_BYTES_PER_FRAME;
    if (*out_size < required_out) {
        fprintf(stderr, "AptxEncoder: Output buffer too small (%u < %u)\n",
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
            fprintf(stderr, "AptxEncoder: Encode failed at frame %u\n", f);
            return false;
        }

        out_offset += static_cast<uint32_t>(written);
    }

    *out_size = out_offset;
    *out_frames = num_frames;
    return true;
}

uint32_t AptxEncoder::get_pcm_frames_per_encode() const {
    /* 128 PCM frames (32 aptX groups) -> 128 bytes per encode call */
    return 128;
}

void AptxEncoder::shutdown() {
    if (ctx_) {
        aptx_finish(static_cast<struct aptx_context *>(ctx_));
        ctx_ = nullptr;
    }
    initialized_ = false;
}
