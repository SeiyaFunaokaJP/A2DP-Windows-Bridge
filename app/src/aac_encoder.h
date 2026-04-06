/*
 * AAC Encoder Wrapper
 *
 * Wraps Fraunhofer fdk-aac for encoding PCM audio to AAC-LC frames.
 * AAC: optional A2DP codec, 44.1/48 kHz, stereo, up to 320 kbps.
 * Uses LATM transport for A2DP compatibility.
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef AAC_ENCODER_H
#define AAC_ENCODER_H

#include "audio_encoder.h"
#include <cstdint>

#ifdef AAC_ENCODER_AVAILABLE

class AacEncoder : public AudioEncoder {
public:
    AacEncoder();
    ~AacEncoder() override;

    AudioCodec codec_type() const override { return AudioCodec::AAC; }
    const char *codec_name() const override { return "AAC"; }

    bool init(uint16_t mtu, EncoderQuality quality,
              uint32_t sample_rate, uint32_t channels) override;

    bool encode(const uint8_t *pcm_data, uint32_t pcm_bytes,
                uint8_t *out_data, uint32_t *out_size,
                uint32_t *out_frames) override;

    uint32_t get_pcm_frames_per_encode() const override;
    uint32_t get_bitrate_kbps() const override;

    void shutdown() override;

private:
    void *aac_enc_ = nullptr;   /* HANDLE_AACENCODER */
    bool initialized_ = false;
    uint16_t mtu_ = 0;
    uint32_t sample_rate_ = 0;
    uint32_t channels_ = 0;
    uint32_t bitrate_ = 0;
    uint32_t frame_length_ = 0; /* PCM frames per AAC frame (typically 1024) */
    uint32_t max_out_size_ = 0;
};

#endif /* AAC_ENCODER_AVAILABLE */

#endif /* AAC_ENCODER_H */
