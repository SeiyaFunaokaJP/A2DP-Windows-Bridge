/*
 * SBC Encoder Wrapper - Implementation
 *
 * Uses BTstack's bundled Bluedroid SBC encoder.
 * SBC encodes blocks of (num_blocks * num_subbands) samples per frame.
 * With 16 blocks, 8 subbands: 128 PCM frames per SBC frame.
 *
 * Input:  signed 16-bit PCM, interleaved
 * Output: SBC encoded frames, packed for AVDTP media transport
 *
 * SPDX-License-Identifier: MIT
 */

#include "a2dp_sbc_encoder.h"
#include <cstdio>
#include <cstring>
#include <cstdlib>

extern "C" {
#include "classic/btstack_sbc.h"
#include "classic/btstack_sbc_bluedroid.h"
}

/* SBC configuration constants */
static constexpr uint8_t SBC_NUM_BLOCKS   = 16;
static constexpr uint8_t SBC_NUM_SUBBANDS = 8;

/* Quality -> bitpool mapping (Joint Stereo) */
static uint8_t quality_to_bitpool(EncoderQuality q) {
    switch (q) {
    case EncoderQuality::High:     return 53;
    case EncoderQuality::Standard: return 35;
    case EncoderQuality::Mobile:   return 19;
    }
    return 53;
}

SbcEncoder::SbcEncoder() = default;

SbcEncoder::~SbcEncoder() {
    shutdown();
}

bool SbcEncoder::init(uint16_t mtu, EncoderQuality quality,
                      uint32_t sample_rate, uint32_t channels) {
    if (initialized_) {
        shutdown();
    }

    if (channels != 1 && channels != 2) {
        fprintf(stderr, "SbcEncoder: Unsupported channel count %u\n", channels);
        return false;
    }

    if (sample_rate != 44100 && sample_rate != 48000) {
        fprintf(stderr, "SbcEncoder: Unsupported sample rate %u (use 44100 or 48000)\n",
                sample_rate);
        return false;
    }

    bitpool_ = quality_to_bitpool(quality);

    /* Allocate and initialize the Bluedroid SBC encoder */
    auto *state = static_cast<btstack_sbc_encoder_bluedroid_t *>(
        calloc(1, sizeof(btstack_sbc_encoder_bluedroid_t)));
    if (!state) {
        fprintf(stderr, "SbcEncoder: Failed to allocate encoder state\n");
        return false;
    }
    sbc_state_ = state;

    const btstack_sbc_encoder_t *inst = btstack_sbc_encoder_bluedroid_init_instance(state);
    if (!inst) {
        fprintf(stderr, "SbcEncoder: Failed to initialize Bluedroid SBC encoder\n");
        free(state);
        sbc_state_ = nullptr;
        return false;
    }
    sbc_instance_ = inst;

    btstack_sbc_channel_mode_t ch_mode = (channels == 1)
        ? SBC_CHANNEL_MODE_MONO
        : SBC_CHANNEL_MODE_JOINT_STEREO;

    uint8_t status = inst->configure(
        state, SBC_MODE_STANDARD,
        SBC_NUM_BLOCKS, SBC_NUM_SUBBANDS,
        SBC_ALLOCATION_METHOD_LOUDNESS,
        static_cast<uint16_t>(sample_rate),
        bitpool_, ch_mode);

    if (status != 0) {
        fprintf(stderr, "SbcEncoder: configure failed (status=%u)\n", status);
        free(state);
        sbc_state_ = nullptr;
        sbc_instance_ = nullptr;
        return false;
    }

    pcm_frames_per_sbc_frame_ = inst->num_audio_frames(state);
    sbc_frame_length_ = inst->sbc_buffer_length(state);

    /*
     * Bluedroid SBC encoder only populates u16PacketLength after the first
     * SBC_Encoder() call, so sbc_buffer_length() returns 0 here.
     * Calculate the frame length from the SBC spec (A2DP v1.3, Section 12.9):
     *   Joint Stereo: 4 + 4*subbands*ch/8 + ceil((subbands + blocks*bitpool) / 8)
     *   Mono/Dual:    4 + 4*subbands*ch/8 + ceil((blocks * ch * bitpool) / 8)
     *   Stereo:       4 + 4*subbands*ch/8 + ceil((blocks * bitpool) / 8)
     */
    if (sbc_frame_length_ == 0) {
        uint32_t header = 4 + (4u * SBC_NUM_SUBBANDS * channels) / 8;
        uint32_t data_bits;
        if (channels == 2) {
            /* Joint Stereo */
            data_bits = SBC_NUM_SUBBANDS + (uint32_t)SBC_NUM_BLOCKS * bitpool_;
        } else {
            /* Mono */
            data_bits = (uint32_t)SBC_NUM_BLOCKS * channels * bitpool_;
        }
        sbc_frame_length_ = header + (data_bits + 7) / 8;
    }

    mtu_ = mtu;
    sample_rate_ = sample_rate;
    channels_ = channels;
    initialized_ = true;

    fprintf(stderr, "SbcEncoder: Initialized (rate=%u, ch=%u, mtu=%u, bitpool=%u, "
           "frame_len=%u, pcm_frames=%u, bitrate=%ukbps)\n",
           sample_rate, channels, mtu, bitpool_,
           sbc_frame_length_, pcm_frames_per_sbc_frame_, get_bitrate_kbps());
    return true;
}

bool SbcEncoder::encode(const uint8_t *pcm_data, uint32_t pcm_bytes,
                        uint8_t *out_data, uint32_t *out_size,
                        uint32_t *out_frames) {
    if (!initialized_ || !sbc_instance_) {
        return false;
    }

    auto *state = static_cast<btstack_sbc_encoder_bluedroid_t *>(sbc_state_);
    auto *inst = static_cast<const btstack_sbc_encoder_t *>(sbc_instance_);

    /*
     * Each SBC frame consumes pcm_frames_per_sbc_frame_ stereo/mono samples.
     * Each sample is int16_t (2 bytes) * channels.
     */
    uint32_t bytes_per_sbc_input = pcm_frames_per_sbc_frame_ * channels_ * sizeof(int16_t);
    uint32_t num_frames = pcm_bytes / bytes_per_sbc_input;
    if (num_frames == 0) {
        *out_size = 0;
        *out_frames = 0;
        return true;
    }

    /* Limit output to fit MTU (leave room for RTP header + 1-byte SBC media header) */
    uint32_t max_payload = mtu_ > 16 ? mtu_ - 16 : mtu_;
    uint32_t max_frames = max_payload / sbc_frame_length_;
    if (max_frames == 0) max_frames = 1;
    if (num_frames > max_frames) {
        num_frames = max_frames;
    }

    /* Check output buffer */
    uint32_t required_out = num_frames * sbc_frame_length_;
    if (*out_size < required_out) {
        fprintf(stderr, "SbcEncoder: Output buffer too small (%u < %u)\n",
                *out_size, required_out);
        return false;
    }

    const int16_t *pcm16 = reinterpret_cast<const int16_t *>(pcm_data);
    uint32_t out_offset = 0;

    for (uint32_t f = 0; f < num_frames; f++) {
        const int16_t *frame_pcm = pcm16 + f * pcm_frames_per_sbc_frame_ * channels_;

        uint8_t status = inst->encode_signed_16(
            state, frame_pcm, out_data + out_offset);

        if (status != 0) {
            fprintf(stderr, "SbcEncoder: Encode failed at frame %u (status=%u)\n", f, status);
            return false;
        }

        /* After the first real encode, Bluedroid populates the actual frame length.
         * Update our value in case the spec-based calculation drifted. */
        if (f == 0) {
            uint16_t actual_len = inst->sbc_buffer_length(state);
            if (actual_len > 0 && actual_len != sbc_frame_length_) {
                fprintf(stderr, "SbcEncoder: Adjusting frame_len %u -> %u\n",
                        sbc_frame_length_, actual_len);
                sbc_frame_length_ = actual_len;
            }
        }

        out_offset += sbc_frame_length_;
    }

    *out_size = out_offset;
    *out_frames = num_frames;
    return true;
}

uint32_t SbcEncoder::get_pcm_frames_per_encode() const {
    /*
     * Return the number of PCM frames consumed per SBC frame.
     * With 16 blocks x 8 subbands = 128 audio frames per SBC frame.
     */
    return pcm_frames_per_sbc_frame_;
}

uint32_t SbcEncoder::get_bitrate_kbps() const {
    if (!initialized_ || sbc_frame_length_ == 0 || pcm_frames_per_sbc_frame_ == 0) return 0;
    /* bitrate = 8 * frame_length * sample_rate / (subbands * blocks) / 1000 */
    return (8u * sbc_frame_length_ * sample_rate_) /
           (SBC_NUM_SUBBANDS * SBC_NUM_BLOCKS) / 1000;
}

void SbcEncoder::shutdown() {
    if (sbc_state_) {
        free(sbc_state_);
        sbc_state_ = nullptr;
    }
    sbc_instance_ = nullptr;
    initialized_ = false;
}
