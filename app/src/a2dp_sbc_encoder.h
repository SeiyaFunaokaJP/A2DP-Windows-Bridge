/*
 * SBC Encoder Wrapper
 *
 * Wraps BTstack's Bluedroid SBC encoder for encoding PCM audio to SBC frames.
 * SBC: mandatory A2DP codec, 44.1/48 kHz, stereo, up to ~345 kbps.
 *
 * Named a2dp_sbc_encoder.h to avoid collision with BTstack's internal
 * 3rd-party/bluedroid/encoder/include/sbc_encoder.h
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef A2DP_SBC_ENCODER_H
#define A2DP_SBC_ENCODER_H

#include "audio_encoder.h"
#include <cstdint>

class SbcEncoder : public AudioEncoder {
public:
    SbcEncoder();
    ~SbcEncoder() override;

    AudioCodec codec_type() const override { return AudioCodec::SBC; }
    const char *codec_name() const override { return "SBC"; }

    bool init(uint16_t mtu, EncoderQuality quality,
              uint32_t sample_rate, uint32_t channels) override;

    bool encode(const uint8_t *pcm_data, uint32_t pcm_bytes,
                uint8_t *out_data, uint32_t *out_size,
                uint32_t *out_frames) override;

    uint32_t get_pcm_frames_per_encode() const override;
    uint32_t get_bitrate_kbps() const override;

    void shutdown() override;

private:
    void *sbc_state_ = nullptr;         /* btstack_sbc_encoder_bluedroid_t* */
    const void *sbc_instance_ = nullptr; /* const btstack_sbc_encoder_t* */
    bool initialized_ = false;
    uint16_t mtu_ = 0;
    uint32_t sample_rate_ = 0;
    uint32_t channels_ = 0;
    uint8_t bitpool_ = 53;
    uint16_t sbc_frame_length_ = 0;
    uint16_t pcm_frames_per_sbc_frame_ = 0;
};

#endif /* A2DP_SBC_ENCODER_H */
