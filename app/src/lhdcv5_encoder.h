/*
 * LHDC V5 Encoder Wrapper
 *
 * Wraps the portable C port of the AOSP LHDC V5 encoder
 * (extern/lhdcv5-enc, Apache-2.0) for encoding PCM audio to LHDC V5 frames.
 * LHDC V5: Savitech codec, 44.1/48/96/192 kHz, 16/24-bit stereo, 5 ms frames,
 * 64 to 1000 kbps (A2DP) with optional adaptive bit rate (ABR).
 *
 * Experimental: follows the Android 17 (AOSP) implementation and the C port
 * was verified bit-exact against the AOSP Rust encoder, but it has not been
 * tested with real LHDC headphones yet.
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef LHDCV5_ENCODER_H
#define LHDCV5_ENCODER_H

#include "audio_encoder.h"
#include <cstdint>
#include <vector>

struct lhdcv5_enc;

class LhdcV5Encoder : public AudioEncoder {
public:
    LhdcV5Encoder();
    ~LhdcV5Encoder() override;

    /* Input sample format: 16 (default) = int16; 24 or 32 = int32 container,
     * MSB-aligned full scale (top 24 bits are encoded). Call before init(). */
    void set_bit_depth(int bits);

    /* Bit rate limits the sink advertised: its capability byte P7
     * (A2DP_LHDCV5_MAX_BIT_RATE / MIN_BIT_RATE bits). Call before init(). */
    void set_peer_bitrate_limits(uint8_t p7);

    /* Adaptive bit rate (Android's default for LHDC V5): the encoder starts
     * at 400 kbps and moves between 160 and 400 kbps depending on the send
     * queue; the quality mode passed to init() is ignored. Call before init(). */
    void set_abr(bool enabled) { abr_ = enabled; }
    bool is_abr_enabled() const { return abr_ && initialized_; }

    /* ABR step: call once per media packet sent with the current send queue
     * depth (packets), as Android does (lhdcv5BT_adjust_bitrate). */
    void abr_adjust(uint32_t queue_len);

    AudioCodec codec_type() const override { return AudioCodec::LHDCV5; }
    const char *codec_name() const override { return "LHDC V5"; }

    bool init(uint16_t mtu, EncoderQuality quality,
              uint32_t sample_rate, uint32_t channels) override;

    bool encode(const uint8_t *pcm_data, uint32_t pcm_bytes,
                uint8_t *out_data, uint32_t *out_size,
                uint32_t *out_frames) override;

    /* One LHDC frame (5 ms: 240 / 480 / 960 samples) per encode call */
    uint32_t get_pcm_frames_per_encode() const override { return block_size_; }
    uint32_t get_pcm_frames_per_codec_frame() const override { return block_size_; }

    /* Current target bit rate (changes under ABR) */
    uint32_t get_bitrate_kbps() const override;

    void shutdown() override;

private:
    struct lhdcv5_enc *handle_ = nullptr;
    bool initialized_ = false;
    uint32_t block_size_ = 0;         /* samples per frame per channel */
    uint32_t sample_rate_ = 0;
    uint32_t channels_ = 0;
    uint32_t bytes_per_sample_ = 2;   /* 2 = int16, 4 = int32 MSB-aligned */
    uint32_t encoder_bits_ = 16;      /* 16 or 24: what the encoder is fed */
    uint8_t peer_p7_ = 0;             /* 0 = no limits (64..1000 kbps) */
    bool abr_ = false;
    std::vector<uint8_t> packed_;     /* interleaved stereo s16le / packed s24le */

    /* ABR state (Android lhdcv5BT_enc.c policy) */
    const uint32_t *abr_table_ = nullptr;
    uint32_t abr_table_size_ = 0;
    uint32_t abr_table_index_ = 0;
    uint32_t abr_down_count_ = 0, abr_down_sum_ = 0;
    uint32_t abr_up_count_ = 0, abr_up_sum_ = 0;
};

#endif /* LHDCV5_ENCODER_H */
