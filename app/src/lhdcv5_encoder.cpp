/*
 * LHDC V5 Encoder Wrapper - Implementation
 *
 * Uses the C port of the AOSP LHDC V5 encoder (extern/lhdcv5-enc). The
 * encoder takes exactly one 5 ms frame of interleaved stereo PCM per call
 * (packed s16le or packed s24le) and returns a complete packet (one or more
 * LHDC frames, each with its own 2-byte frame header) every N-th call, sized
 * for the MTU it was given.
 *
 * Input:  interleaved PCM, stereo or mono (duplicated to L/R); signed 16-bit
 *         by default, or 32-bit MSB-aligned after set_bit_depth(24|32)
 * Output: LHDC V5 frames (sent WITH an RTP header and the 2-byte LHDC media
 *         payload header, added by BtStackTransport::send_media)
 *
 * Parameters follow Android's a2dp_vendor_lhdcv5_encoder.cc: 5 ms frames,
 * 20 ms encode interval (the low-latency 10 ms mode is not used), MTU minus
 * the media payload header, peer bit rate limits applied after init, and
 * the ABR policy of lhdcv5BT_enc.c (both Apache-2.0, AOSP android17-release).
 *
 * SPDX-License-Identifier: MIT
 */

#include "lhdcv5_encoder.h"
#include <cstdio>
#include <cstring>

extern "C" {
#include "lhdcv5_enc.h"
}

/* A2DP media payload header A2DPWB prepends (A2DP_LHDC_MPL_HDR_LEN) */
static constexpr uint32_t LHDCV5_MPL_HDR_LEN = 2;

/* ABR policy constants and tables (Android lhdcv5BT_enc.c). A "tick" is one
 * media packet sent. */
static constexpr uint32_t ABR_UP_RATE_TIME_CNT = 3000;      /* ticks between up checks */
static constexpr uint32_t ABR_DOWN_RATE_TIME_CNT = 4;       /* ticks between down checks */
static constexpr uint32_t ABR_UP_QUEUE_LENGTH_THRESHOLD = 1;
static constexpr uint32_t ABR_DOWN_QUEUE_LENGTH_THRESHOLD = 0;
static constexpr uint32_t ABR_DOWN_TARGET_STAGE = 0;
static const uint32_t ABR_TABLE_44K[]  = {160, 192, 240, 320, 400, 400};
static const uint32_t ABR_TABLE_48K[]  = {160, 192, 256, 320, 400, 400};
static const uint32_t ABR_TABLE_96K[]  = {256, 320, 400, 400, 400, 400};
static const uint32_t ABR_TABLE_192K[] = {256, 320, 400, 400, 400, 400};

LhdcV5Encoder::LhdcV5Encoder() = default;

LhdcV5Encoder::~LhdcV5Encoder() {
    shutdown();
}

void LhdcV5Encoder::set_bit_depth(int bits) {
    /* 24 or 32: 32-bit MSB-aligned container (as LDAC / aptX HD); else 16-bit */
    bool wide = (bits == 24 || bits == 32);
    bytes_per_sample_ = wide ? 4u : 2u;
    encoder_bits_ = wide ? 24u : 16u;
}

void LhdcV5Encoder::set_peer_bitrate_limits(uint8_t p7) {
    peer_p7_ = p7;
}

/* Sink capability P7[5:4] -> encoder quality index (A2DP_MaxBitRatetoQualityLevelLhdcV5) */
static uint32_t peer_max_bitrate_index(uint8_t p7) {
    switch (p7 & 0x30) {
    case 0x10: return LHDC_QUALITY_LOW;    /* 400 kbps */
    case 0x20: return LHDC_QUALITY_MID;    /* 500 kbps */
    case 0x30: return LHDC_QUALITY_HIGH;   /* 900 kbps */
    default:   return LHDC_QUALITY_HIGH1;  /* 1000 kbps = no upper limit */
    }
}

/* Sink capability P7[7:6] -> encoder quality index (A2DP_MinBitRatetoQualityLevelLhdcV5) */
static uint32_t peer_min_bitrate_index(uint8_t p7) {
    switch (p7 & 0xC0) {
    case 0x40: return LHDC_QUALITY_LOW1;   /* 160 kbps */
    case 0x80: return LHDC_QUALITY_LOW3;   /* 256 kbps */
    case 0xC0: return LHDC_QUALITY_LOW;    /* 400 kbps */
    default:   return LHDC_QUALITY_LOW0;   /* 64 kbps = no lower limit */
    }
}

bool LhdcV5Encoder::init(uint16_t mtu, EncoderQuality quality,
                         uint32_t sample_rate, uint32_t channels) {
    if (initialized_) {
        shutdown();
    }

    if (channels != 1 && channels != 2) {
        fprintf(stderr, "LhdcV5Encoder: Only mono or stereo input is supported\n");
        return false;
    }
    if (sample_rate != 44100 && sample_rate != 48000 &&
        sample_rate != 96000 && sample_rate != 192000) {
        fprintf(stderr, "LhdcV5Encoder: Unsupported sample rate %u "
                "(use 44100, 48000, 96000 or 192000)\n", sample_rate);
        return false;
    }

    /* Quality mode -> target bit rate index (clamped to the sink's limits
     * below). ABR starts at 400 kbps regardless (LHDC_QUALITY_AUTO). */
    uint32_t bitrate_inx;
    if (abr_) {
        bitrate_inx = LHDC_QUALITY_AUTO;
    } else {
        switch (quality) {
        case EncoderQuality::Standard: bitrate_inx = LHDC_QUALITY_MID;  break; /* 500 kbps */
        case EncoderQuality::Mobile:   bitrate_inx = LHDC_QUALITY_LOW4; break; /* 320 kbps */
        default:                       bitrate_inx = LHDC_QUALITY_HIGH1; break; /* 1000 kbps */
        }
    }

    /* The encoder sizes its packets for this MTU: our media MTU (RTP header
     * already excluded) minus the 2-byte LHDC media payload header */
    uint32_t enc_mtu = (mtu > LHDCV5_MPL_HDR_LEN) ? (uint32_t)mtu - LHDCV5_MPL_HDR_LEN : 0;
    if (enc_mtu < LHDC_MTU_MIN) {
        fprintf(stderr, "LhdcV5Encoder: media MTU %u too small (LHDC needs %u)\n",
                mtu, LHDC_MTU_MIN + LHDCV5_MPL_HDR_LEN);
        return false;
    }
    if (enc_mtu > LHDC_MTU_MAX) enc_mtu = LHDC_MTU_MAX;

    handle_ = lhdcv5_enc_new(LHDC_VERSION_1);
    if (!handle_) {
        fprintf(stderr, "LhdcV5Encoder: Failed to allocate encoder context\n");
        return false;
    }

    int32_t rc = lhdcv5_enc_init_encoder(handle_, sample_rate, encoder_bits_, bitrate_inx,
                                         enc_mtu, LHDC_ENC_INTERVAL_20MS);
    if (rc != LHDC_FRET_SUCCESS) {
        fprintf(stderr, "LhdcV5Encoder: lhdcv5_enc_init_encoder failed (%d)\n", (int)rc);
        shutdown();
        return false;
    }

    /* Peer bit rate limits, applied after init as Android does */
    uint32_t max_inx = peer_max_bitrate_index(peer_p7_);
    uint32_t min_inx = peer_min_bitrate_index(peer_p7_);
    rc = lhdcv5_enc_set_max_bitrate_index(handle_, max_inx);
    if (rc != LHDC_FRET_SUCCESS)
        fprintf(stderr, "LhdcV5Encoder: set_max_bitrate_index(%u) failed (%d)\n", max_inx, (int)rc);
    rc = lhdcv5_enc_set_min_bitrate_index(handle_, min_inx);
    if (rc != LHDC_FRET_SUCCESS)
        fprintf(stderr, "LhdcV5Encoder: set_min_bitrate_index(%u) failed (%d)\n", min_inx, (int)rc);

    rc = lhdcv5_enc_get_block_size(handle_, &block_size_);
    if (rc != LHDC_FRET_SUCCESS || block_size_ == 0) {
        fprintf(stderr, "LhdcV5Encoder: lhdcv5_enc_get_block_size failed (%d)\n", (int)rc);
        shutdown();
        return false;
    }

    sample_rate_ = sample_rate;
    channels_ = channels;
    packed_.assign((size_t)block_size_ * 2 * (encoder_bits_ / 8), 0);

    /* ABR table for this sample rate; start at its top (400 kbps) */
    if (abr_) {
        switch (sample_rate) {
        case 44100:  abr_table_ = ABR_TABLE_44K;  abr_table_size_ = 6; break;
        case 96000:  abr_table_ = ABR_TABLE_96K;  abr_table_size_ = 6; break;
        case 192000: abr_table_ = ABR_TABLE_192K; abr_table_size_ = 6; break;
        default:     abr_table_ = ABR_TABLE_48K;  abr_table_size_ = 6; break;
        }
        abr_table_index_ = abr_table_size_ - 1;
        abr_down_count_ = abr_down_sum_ = 0;
        abr_up_count_ = abr_up_sum_ = 0;
    }

    initialized_ = true;

    fprintf(stderr, "LhdcV5Encoder: Initialized (rate=%u, ch=%u, %u-bit, mtu=%u, "
            "frame=%u samples, bitrate=%u kbps%s, peer limits idx %u..%u)\n",
            sample_rate, channels, encoder_bits_, enc_mtu, block_size_,
            get_bitrate_kbps(), abr_ ? " ABR" : "", min_inx, max_inx);
    return true;
}

bool LhdcV5Encoder::encode(const uint8_t *pcm_data, uint32_t pcm_bytes,
                           uint8_t *out_data, uint32_t *out_size,
                           uint32_t *out_frames) {
    if (!initialized_ || !handle_) {
        return false;
    }

    uint32_t in_frame_bytes = block_size_ * channels_ * bytes_per_sample_;
    if (pcm_bytes < in_frame_bytes) {
        /* Less than one LHDC frame: nothing to do (the caller batches whole frames) */
        *out_size = 0;
        *out_frames = 0;
        return true;
    }

    /* Repack to what the encoder wants: interleaved stereo, s16le or packed
     * s24le (3 bytes per sample, taken from the top of the int32 container) */
    const uint8_t *in = pcm_data;
    size_t in_len = packed_.size();
    if (bytes_per_sample_ == 2 && channels_ == 2) {
        in = pcm_data;  /* already interleaved stereo s16le */
    } else {
        uint8_t *dst = packed_.data();
        for (uint32_t i = 0; i < block_size_; i++) {
            for (uint32_t ch = 0; ch < 2; ch++) {
                uint32_t src_ch = (channels_ == 2) ? ch : 0;
                const uint8_t *s = pcm_data + ((size_t)i * channels_ + src_ch) * bytes_per_sample_;
                if (bytes_per_sample_ == 2) {
                    dst[0] = s[0]; dst[1] = s[1];
                    dst += 2;
                } else {
                    /* int32 LE, MSB-aligned: bytes 1..3 are the 24-bit sample */
                    dst[0] = s[1]; dst[1] = s[2]; dst[2] = s[3];
                    dst += 3;
                }
            }
        }
        in = packed_.data();
    }

    uint32_t written = 0, frames = 0;
    int32_t rc = lhdcv5_enc_encode(handle_, in, in_len, out_data, *out_size, &written, &frames);
    if (rc != LHDC_FRET_SUCCESS) {
        fprintf(stderr, "LhdcV5Encoder: lhdcv5_enc_encode failed (%d, out buffer %u bytes)\n",
                (int)rc, *out_size);
        return false;
    }

    *out_size = written;
    *out_frames = frames;
    return true;
}

uint32_t LhdcV5Encoder::get_bitrate_kbps() const {
    uint32_t kbps = 0;
    if (handle_ && lhdcv5_enc_get_last_bitrate(handle_, &kbps) == LHDC_FRET_SUCCESS)
        return kbps;
    return 0;
}

/*
 * ABR policy of Android's lhdcv5BT_enc.c (lhdcv5_enc_abr_adjust_bitrate):
 * every ABR_DOWN_RATE_TIME_CNT ticks, an average queue depth above 0 drops
 * the bit rate to the lowest table stage; every ABR_UP_RATE_TIME_CNT ticks
 * with an empty queue throughout, the bit rate moves up one stage.
 */
void LhdcV5Encoder::abr_adjust(uint32_t queue_len) {
    if (!initialized_ || !handle_ || !abr_ || !abr_table_) return;

    auto current_index = [this](uint32_t *inx) {
        uint32_t kbps = 0;
        if (lhdcv5_enc_get_last_bitrate(handle_, &kbps) != LHDC_FRET_SUCCESS) return false;
        return lhdcv5_enc_get_bitrate_index(handle_, kbps, inx) == LHDC_FRET_SUCCESS;
    };

    if (abr_down_count_ >= ABR_DOWN_RATE_TIME_CNT) {
        uint32_t avg = abr_down_sum_ / abr_down_count_;
        abr_down_count_ = 0;
        abr_down_sum_ = 0;
        if (avg > ABR_DOWN_QUEUE_LENGTH_THRESHOLD) {
            uint32_t last_inx = 0, new_inx = 0;
            uint32_t new_stage = ABR_DOWN_TARGET_STAGE;
            if (current_index(&last_inx) &&
                lhdcv5_enc_get_bitrate_index(handle_, abr_table_[new_stage], &new_inx) == LHDC_FRET_SUCCESS &&
                new_inx <= last_inx && new_stage < abr_table_index_) {
                if (lhdcv5_enc_set_bitrate_index(handle_, new_inx, false) == LHDC_FRET_SUCCESS) {
                    fprintf(stderr, "LhdcV5Encoder: ABR down %u -> %u kbps (queue avg %u)\n",
                            abr_table_[abr_table_index_], abr_table_[new_stage], avg);
                    abr_up_count_ = 0;
                    abr_up_sum_ = 0;
                    abr_table_index_ = new_stage;
                }
            }
        }
    }

    if (abr_up_count_ >= ABR_UP_RATE_TIME_CNT) {
        uint32_t sum = abr_up_sum_;
        abr_up_count_ = 0;
        abr_up_sum_ = 0;
        if (sum < ABR_UP_QUEUE_LENGTH_THRESHOLD) {
            uint32_t new_stage = 0;
            if (abr_table_index_ < abr_table_size_ - 1) new_stage = abr_table_index_ + 1;
            uint32_t last_inx = 0, new_inx = 0;
            if (current_index(&last_inx) &&
                lhdcv5_enc_get_bitrate_index(handle_, abr_table_[new_stage], &new_inx) == LHDC_FRET_SUCCESS &&
                new_inx >= last_inx && new_stage > abr_table_index_) {
                if (lhdcv5_enc_set_bitrate_index(handle_, new_inx, false) == LHDC_FRET_SUCCESS) {
                    fprintf(stderr, "LhdcV5Encoder: ABR up %u -> %u kbps\n",
                            abr_table_[abr_table_index_], abr_table_[new_stage]);
                    abr_down_count_ = 0;
                    abr_down_sum_ = 0;
                    abr_table_index_ = new_stage;
                }
            }
        }
    }

    if (queue_len > 0) {
        abr_up_sum_ += queue_len;
        abr_down_sum_ += queue_len;
    }
    abr_up_count_++;
    abr_down_count_++;
}

void LhdcV5Encoder::shutdown() {
    if (handle_) {
        lhdcv5_enc_free(handle_);
        handle_ = nullptr;
    }
    initialized_ = false;
    block_size_ = 0;
    abr_table_ = nullptr;
}
