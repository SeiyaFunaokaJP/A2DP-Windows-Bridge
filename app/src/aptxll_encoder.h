/*
 * aptX Low Latency Encoder Wrapper
 *
 * Wraps libopenaptx for encoding PCM audio to aptX Low Latency frames.
 * aptX LL: Qualcomm codec, 44.1/48kHz 16-bit stereo, 352 kbps, ~32ms latency.
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef APTXLL_ENCODER_H
#define APTXLL_ENCODER_H

#include "audio_encoder.h"
#include <cstdint>

class AptxLlEncoder : public AudioEncoder {
public:
    AptxLlEncoder();
    ~AptxLlEncoder() override;

    AudioCodec codec_type() const override { return AudioCodec::AptxLL; }
    const char *codec_name() const override { return "aptX Low Latency"; }

    bool init(uint16_t mtu, EncoderQuality quality,
              uint32_t sample_rate, uint32_t channels) override;

    bool encode(const uint8_t *pcm_data, uint32_t pcm_bytes,
                uint8_t *out_data, uint32_t *out_size,
                uint32_t *out_frames) override;

    uint32_t get_pcm_frames_per_encode() const override;
    uint32_t get_bitrate_kbps() const override { return 352; }

    void shutdown() override;

private:
    void *ctx_ = nullptr;       /* struct aptx_context * */
    bool initialized_ = false;
    uint16_t mtu_ = 0;
    uint32_t sample_rate_ = 0;
    uint32_t channels_ = 0;
};

#endif /* APTXLL_ENCODER_H */
