/*
 * aptX (classic) Encoder Wrapper
 *
 * Wraps libopenaptx for encoding PCM audio to classic aptX frames.
 * aptX: Qualcomm (vendor 0x0000004F, codec 0x0001), 44.1/48kHz stereo,
 * 4:1 compression of 16-bit PCM (352 kbps @ 44.1kHz, 384 kbps @ 48kHz).
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef APTX_ENCODER_H
#define APTX_ENCODER_H

#include "audio_encoder.h"
#include <cstdint>

class AptxEncoder : public AudioEncoder {
public:
    AptxEncoder();
    ~AptxEncoder() override;

    /* Input sample format: 16 (default) = int16; 24 or 32 = int32 container,
     * MSB-aligned full scale (top 24 bits are encoded). Call before encode(). */
    void set_bit_depth(int bits);

    AudioCodec codec_type() const override { return AudioCodec::Aptx; }
    const char *codec_name() const override { return "aptX"; }

    bool init(uint16_t mtu, EncoderQuality quality,
              uint32_t sample_rate, uint32_t channels) override;

    bool encode(const uint8_t *pcm_data, uint32_t pcm_bytes,
                uint8_t *out_data, uint32_t *out_size,
                uint32_t *out_frames) override;

    uint32_t get_pcm_frames_per_encode() const override;

    /* 4 bytes out per 4 stereo sample-frames: sample_rate * 2ch * 16bit / 4 */
    uint32_t get_bitrate_kbps() const override { return sample_rate_ * 8 / 1000; }

    void shutdown() override;

private:
    void *ctx_ = nullptr;       /* struct aptx_context * */
    bool initialized_ = false;
    uint16_t mtu_ = 0;
    uint32_t sample_rate_ = 0;
    uint32_t channels_ = 0;     /* input channels (1 or 2); output is always stereo */
    uint32_t bytes_per_sample_ = 2;  /* 2 = int16, 4 = int32 MSB-aligned */
};

#endif /* APTX_ENCODER_H */
