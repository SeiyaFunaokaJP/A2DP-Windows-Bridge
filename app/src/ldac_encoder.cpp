/*
 * LDAC Encoder Wrapper - Implementation
 * SPDX-License-Identifier: MIT
 */

#include "ldac_encoder.h"
#include <cstdio>
#include <cstring>

extern "C" {
#include "ldacBT.h"
#ifdef LDAC_ABR_AVAILABLE
#include "ldacBT_abr.h"
#endif
}

LdacEncoder::LdacEncoder() = default;

void LdacEncoder::set_bit_depth(int bits) {
    /* LDACBT_SMPL_FMT_S16=2, LDACBT_SMPL_FMT_S24=3, LDACBT_SMPL_FMT_S32=4 */
    if (bits == 24 || bits == 32)
        sample_fmt_ = LDACBT_SMPL_FMT_S32;
    else
        sample_fmt_ = LDACBT_SMPL_FMT_S16;
}

LdacEncoder::~LdacEncoder() {
    shutdown();
}

bool LdacEncoder::init(uint16_t mtu, EncoderQuality quality,
                       uint32_t sample_rate, uint32_t channels) {
    if (initialized_) {
        shutdown();
    }

    handle_ = ldacBT_get_handle();
    if (!handle_) {
        fprintf(stderr, "LdacEncoder: Failed to get LDAC handle\n");
        return false;
    }

    /* Map quality enum to libldac quality mode index */
    int eqmid;
    switch (quality) {
    case EncoderQuality::High:     eqmid = LDACBT_EQMID_HQ; bitrate_kbps_ = 990; break;
    case EncoderQuality::Standard: eqmid = LDACBT_EQMID_SQ; bitrate_kbps_ = 660; break;
    case EncoderQuality::Mobile:   eqmid = LDACBT_EQMID_MQ; bitrate_kbps_ = 330; break;
    default:                       eqmid = LDACBT_EQMID_HQ; bitrate_kbps_ = 990; break;
    }

    /* Determine channel mode */
    int cm;
    switch (channels) {
    case 1:  cm = LDACBT_CHANNEL_MODE_MONO; break;
    case 2:  cm = LDACBT_CHANNEL_MODE_STEREO; break;
    default:
        fprintf(stderr, "LdacEncoder: Unsupported channel count: %u\n", channels);
        ldacBT_free_handle(static_cast<HANDLE_LDAC_BT>(handle_));
        handle_ = nullptr;
        return false;
    }

    int ret = ldacBT_init_handle_encode(
        static_cast<HANDLE_LDAC_BT>(handle_),
        static_cast<int>(mtu),
        eqmid,
        cm,
        static_cast<LDACBT_SMPL_FMT_T>(sample_fmt_),
        static_cast<int>(sample_rate)
    );

    if (ret != 0) {
        int err = ldacBT_get_error_code(static_cast<HANDLE_LDAC_BT>(handle_));
        fprintf(stderr, "LdacEncoder: Init failed, error code: 0x%04x\n", err);
        ldacBT_free_handle(static_cast<HANDLE_LDAC_BT>(handle_));
        handle_ = nullptr;
        return false;
    }

    initialized_ = true;
    sample_rate_ = sample_rate;
    fprintf(stderr, "LdacEncoder: Initialized (rate=%u, ch=%u, quality=%d, mtu=%u, bitrate=%ukbps)\n",
           sample_rate, channels, eqmid, mtu, bitrate_kbps_);
    return true;
}

bool LdacEncoder::encode(const uint8_t *pcm_data, uint32_t pcm_bytes,
                         uint8_t *out_data, uint32_t *out_size,
                         uint32_t *out_frames) {
    if (!initialized_ || !handle_) {
        return false;
    }

    int encoded_bytes = 0;
    int encoded_frames = 0;

    int ret = ldacBT_encode(
        static_cast<HANDLE_LDAC_BT>(handle_),
        const_cast<void *>(static_cast<const void *>(pcm_data)),
        &encoded_bytes,
        out_data,
        reinterpret_cast<int *>(out_size),
        &encoded_frames
    );

    if (ret < 0) {
        int err = ldacBT_get_error_code(static_cast<HANDLE_LDAC_BT>(handle_));
        fprintf(stderr, "LdacEncoder: Encode failed, error: 0x%04x\n", err);
        return false;
    }

    if (out_frames) {
        *out_frames = static_cast<uint32_t>(encoded_frames);
    }

    (void)pcm_bytes;
    return true;
}

uint32_t LdacEncoder::get_pcm_frames_per_encode() const {
    /* LDAC encodes 128 samples per frame at all sample rates */
    return 128;
}

uint32_t LdacEncoder::get_pcm_frames_per_codec_frame() const {
    /* An LDAC frame holds 128 samples at 44.1/48 kHz and 256 at 88.2/96 kHz */
    return (sample_rate_ > 48000) ? 256 : 128;
}

void LdacEncoder::shutdown() {
    shutdown_abr();
    if (handle_) {
        ldacBT_close_handle(static_cast<HANDLE_LDAC_BT>(handle_));
        ldacBT_free_handle(static_cast<HANDLE_LDAC_BT>(handle_));
        handle_ = nullptr;
    }
    initialized_ = false;
    bitrate_kbps_ = 0;
}

bool LdacEncoder::init_abr(uint32_t check_interval_ms) {
#ifdef LDAC_ABR_AVAILABLE
    if (!initialized_ || !handle_) {
        fprintf(stderr, "LdacEncoder: Cannot init ABR before encoder init\n");
        return false;
    }

    shutdown_abr();

    HANDLE_LDAC_ABR abr = ldac_ABR_get_handle();
    if (!abr) {
        fprintf(stderr, "LdacEncoder: Failed to get ABR handle\n");
        return false;
    }

    int ret = ldac_ABR_Init(abr, static_cast<unsigned int>(check_interval_ms));
    if (ret != 0) {
        fprintf(stderr, "LdacEncoder: ABR init failed (ret=%d)\n", ret);
        ldac_ABR_free_handle(abr);
        return false;
    }

    /* Set thresholds for 64-slot queue: critical=12, danger=8, safety=4 */
    ldac_ABR_set_thresholds(abr, 12, 8, 4);

    abr_handle_ = abr;
    fprintf(stderr, "LdacEncoder: ABR enabled (interval=%ums)\n", check_interval_ms);
    return true;
#else
    (void)check_interval_ms;
    fprintf(stderr, "LdacEncoder: ABR not available (libldac built without ABR module)\n");
    return false;
#endif
}

void LdacEncoder::shutdown_abr() {
#ifdef LDAC_ABR_AVAILABLE
    if (abr_handle_) {
        ldac_ABR_free_handle(static_cast<HANDLE_LDAC_ABR>(abr_handle_));
        abr_handle_ = nullptr;
    }
#endif
}

bool LdacEncoder::abr_adjust(uint32_t tx_queue_length) {
#ifdef LDAC_ABR_AVAILABLE
    if (!abr_handle_ || !handle_) return false;

    int ret = ldac_ABR_Proc(
        static_cast<HANDLE_LDAC_BT>(handle_),
        static_cast<HANDLE_LDAC_ABR>(abr_handle_),
        static_cast<unsigned int>(tx_queue_length),
        0 /* flag: 0 = normal */
    );

    if (ret == 0) {
        /* Update bitrate based on current EQMID */
        int eqmid = ldacBT_get_eqmid(static_cast<HANDLE_LDAC_BT>(handle_));
        switch (eqmid) {
        case LDACBT_EQMID_HQ: bitrate_kbps_ = 990; break;
        case LDACBT_EQMID_SQ: bitrate_kbps_ = 660; break;
        case LDACBT_EQMID_MQ: bitrate_kbps_ = 330; break;
        }
    }

    return ret == 0;
#else
    (void)tx_queue_length;
    return false;
#endif
}
