/*
 * AAC Encoder Wrapper - Implementation
 *
 * Uses Fraunhofer fdk-aac to encode PCM audio to AAC-LC.
 * AAC-LC frame size: 1024 PCM samples.
 * Output uses LATM transport for A2DP media streaming.
 *
 * Input:  signed 16-bit PCM, interleaved
 * Output: LATM-framed AAC-LC encoded audio
 *
 * SPDX-License-Identifier: MIT
 */

#ifdef AAC_ENCODER_AVAILABLE

#include "aac_encoder.h"
#include <cstdio>
#include <cstring>

extern "C" {
#include <aacenc_lib.h>
}

/* Quality -> target bitrate mapping */
static uint32_t quality_to_bitrate(EncoderQuality q) {
    switch (q) {
    case EncoderQuality::High:     return 256000;
    case EncoderQuality::Standard: return 192000;
    case EncoderQuality::Mobile:   return 128000;
    }
    return 256000;
}

AacEncoder::AacEncoder() = default;

AacEncoder::~AacEncoder() {
    shutdown();
}

bool AacEncoder::init(uint16_t mtu, EncoderQuality quality,
                      uint32_t sample_rate, uint32_t channels) {
    if (initialized_) {
        shutdown();
    }

    if (channels != 1 && channels != 2) {
        fprintf(stderr, "AacEncoder: Unsupported channel count %u\n", channels);
        return false;
    }

    if (sample_rate != 44100 && sample_rate != 48000) {
        fprintf(stderr, "AacEncoder: Unsupported sample rate %u (use 44100 or 48000)\n",
                sample_rate);
        return false;
    }

    bitrate_ = quality_to_bitrate(quality);

    /* Open fdk-aac encoder */
    HANDLE_AACENCODER enc = nullptr;
    AACENC_ERROR err = aacEncOpen(&enc, 0, static_cast<UINT>(channels));
    if (err != AACENC_OK) {
        fprintf(stderr, "AacEncoder: aacEncOpen failed (%d)\n", err);
        return false;
    }

    /* Configure AAC-LC profile (required by A2DP) */
    aacEncoder_SetParam(enc, AACENC_AOT, AOT_AAC_LC);
    aacEncoder_SetParam(enc, AACENC_SAMPLERATE, sample_rate);
    aacEncoder_SetParam(enc, AACENC_CHANNELMODE, (channels == 1) ? MODE_1 : MODE_2);
    aacEncoder_SetParam(enc, AACENC_BITRATE, bitrate_);
    aacEncoder_SetParam(enc, AACENC_TRANSMUX, TT_MP4_LATM_MCP1);
    aacEncoder_SetParam(enc, AACENC_AFTERBURNER, 1);  /* Higher quality */

    /* Initialize the encoder */
    err = aacEncEncode(enc, nullptr, nullptr, nullptr, nullptr);
    if (err != AACENC_OK) {
        fprintf(stderr, "AacEncoder: Encoder initialization failed (%d)\n", err);
        aacEncClose(&enc);
        return false;
    }

    /* Get encoder info */
    AACENC_InfoStruct info = {};
    err = aacEncInfo(enc, &info);
    if (err != AACENC_OK) {
        fprintf(stderr, "AacEncoder: aacEncInfo failed (%d)\n", err);
        aacEncClose(&enc);
        return false;
    }

    frame_length_ = info.frameLength;
    max_out_size_ = info.maxOutBufBytes;

    aac_enc_ = enc;
    mtu_ = mtu;
    sample_rate_ = sample_rate;
    channels_ = channels;
    initialized_ = true;

    fprintf(stderr, "AacEncoder: Initialized (rate=%u, ch=%u, mtu=%u, bitrate=%ukbps, "
           "frame_len=%u, max_out=%u)\n",
           sample_rate, channels, mtu, bitrate_ / 1000,
           frame_length_, max_out_size_);
    return true;
}

bool AacEncoder::encode(const uint8_t *pcm_data, uint32_t pcm_bytes,
                        uint8_t *out_data, uint32_t *out_size,
                        uint32_t *out_frames) {
    if (!initialized_ || !aac_enc_) {
        return false;
    }

    HANDLE_AACENCODER enc = static_cast<HANDLE_AACENCODER>(aac_enc_);

    /* Input: PCM samples for one AAC frame */
    uint32_t bytes_per_frame_input = frame_length_ * channels_ * sizeof(int16_t);
    if (pcm_bytes < bytes_per_frame_input) {
        *out_size = 0;
        *out_frames = 0;
        return true;
    }

    /* Set up input buffer descriptor */
    AACENC_BufDesc in_buf = {};
    AACENC_BufDesc out_buf = {};
    AACENC_InArgs in_args = {};
    AACENC_OutArgs out_args = {};

    void *in_ptr = const_cast<uint8_t *>(pcm_data);
    INT in_identifier = IN_AUDIO_DATA;
    INT in_size = static_cast<INT>(bytes_per_frame_input);
    INT in_elem_size = static_cast<INT>(sizeof(int16_t));

    in_buf.numBufs = 1;
    in_buf.bufs = &in_ptr;
    in_buf.bufferIdentifiers = &in_identifier;
    in_buf.bufSizes = &in_size;
    in_buf.bufElSizes = &in_elem_size;

    in_args.numInSamples = static_cast<INT>(frame_length_ * channels_);

    /* Set up output buffer descriptor */
    void *out_ptr = out_data;
    INT out_identifier = OUT_BITSTREAM_DATA;
    INT out_buf_size = static_cast<INT>(*out_size);
    INT out_elem_size = 1;

    out_buf.numBufs = 1;
    out_buf.bufs = &out_ptr;
    out_buf.bufferIdentifiers = &out_identifier;
    out_buf.bufSizes = &out_buf_size;
    out_buf.bufElSizes = &out_elem_size;

    AACENC_ERROR err = aacEncEncode(enc, &in_buf, &out_buf, &in_args, &out_args);
    if (err != AACENC_OK) {
        fprintf(stderr, "AacEncoder: Encode failed (%d)\n", err);
        return false;
    }

    *out_size = static_cast<uint32_t>(out_args.numOutBytes);
    *out_frames = (*out_size > 0) ? 1 : 0;
    return true;
}

uint32_t AacEncoder::get_pcm_frames_per_encode() const {
    /* AAC-LC: 1024 PCM frames per AAC frame */
    return frame_length_;
}

uint32_t AacEncoder::get_bitrate_kbps() const {
    return bitrate_ / 1000;
}

void AacEncoder::shutdown() {
    if (aac_enc_) {
        HANDLE_AACENCODER enc = static_cast<HANDLE_AACENCODER>(aac_enc_);
        aacEncClose(&enc);
        aac_enc_ = nullptr;
    }
    initialized_ = false;
}

#endif /* AAC_ENCODER_AVAILABLE */
