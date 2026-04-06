/*
 * LDAC Encoder Wrapper
 *
 * Wraps libldac (AOSP) for encoding PCM audio to LDAC frames.
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef LDAC_ENCODER_H
#define LDAC_ENCODER_H

#include "audio_encoder.h"
#include <cstdint>

class LdacEncoder : public AudioEncoder {
public:
    LdacEncoder();
    ~LdacEncoder() override;

    AudioCodec codec_type() const override { return AudioCodec::LDAC; }
    const char *codec_name() const override { return "LDAC"; }

    bool init(uint16_t mtu, EncoderQuality quality,
              uint32_t sample_rate, uint32_t channels) override;

    bool encode(const uint8_t *pcm_data, uint32_t pcm_bytes,
                uint8_t *out_data, uint32_t *out_size,
                uint32_t *out_frames) override;

    uint32_t get_pcm_frames_per_encode() const override;

    uint32_t get_bitrate_kbps() const override { return bitrate_kbps_; }

    void shutdown() override;

    /* Set bit depth before calling init(). 24 uses S32 container format. */
    void set_bit_depth(int bits);

    /* ABR (Adaptive Bit Rate) support */
    bool init_abr(uint32_t check_interval_ms = 100);
    void shutdown_abr();
    bool abr_adjust(uint32_t tx_queue_length);
    bool is_abr_enabled() const { return abr_handle_ != nullptr; }

private:
    void *handle_ = nullptr;     /* HANDLE_LDAC_BT */
    void *abr_handle_ = nullptr; /* HANDLE_LDAC_ABR */
    bool initialized_ = false;
    uint32_t bitrate_kbps_ = 0;
    int sample_fmt_ = 2;         /* LDACBT_SMPL_FMT_S16=2, LDACBT_SMPL_FMT_S32=4 */
};

#endif /* LDAC_ENCODER_H */
