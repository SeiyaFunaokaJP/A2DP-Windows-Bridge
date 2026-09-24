/*
 * Audio Encoder Interface
 *
 * Abstract interface for Bluetooth audio codec encoders.
 * Implementations: LdacEncoder, AptxHdEncoder, AptxLlEncoder, AptxEncoder,
 *                  SbcEncoder, AacEncoder
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef AUDIO_ENCODER_H
#define AUDIO_ENCODER_H

#include <cstdint>

/* Codec type identifier */
enum class AudioCodec {
    LDAC,
    AptxHD,
    AptxLL,
    SBC,
    AAC,
    Aptx,      /* classic aptX (appended: keep existing enumerator values stable) */
};

/* Quality mode (codec-specific interpretation) */
enum class EncoderQuality {
    High = 0,      /* LDAC: 990kbps, SBC: bitpool 53, AAC: 256kbps */
    Standard = 1,  /* LDAC: 660kbps, SBC: bitpool 35, AAC: 192kbps */
    Mobile = 2     /* LDAC: 330kbps, SBC: bitpool 19, AAC: 128kbps */
};

class AudioEncoder {
public:
    virtual ~AudioEncoder() = default;

    /* Get the codec type */
    virtual AudioCodec codec_type() const = 0;

    /* Get human-readable codec name */
    virtual const char *codec_name() const = 0;

    /*
     * Initialize the encoder.
     * mtu: L2CAP MTU size for the media transport channel
     * quality: encoding quality mode
     * sample_rate: input PCM sample rate (44100 or 48000)
     * channels: number of channels (1 or 2)
     */
    virtual bool init(uint16_t mtu, EncoderQuality quality,
                      uint32_t sample_rate, uint32_t channels) = 0;

    /*
     * Encode PCM data.
     * pcm_data: input PCM samples (interleaved; format per encoder bit depth)
     * pcm_bytes: size of input data in bytes
     * out_data: output buffer for encoded data
     * out_size: [in] size of output buffer, [out] bytes written
     * out_frames: [out] number of codec frames produced
     * Returns true on success.
     */
    virtual bool encode(const uint8_t *pcm_data, uint32_t pcm_bytes,
                        uint8_t *out_data, uint32_t *out_size,
                        uint32_t *out_frames) = 0;

    /* Get the number of PCM frames consumed per encode call */
    virtual uint32_t get_pcm_frames_per_encode() const = 0;

    /* Get the current bitrate in kbps */
    virtual uint32_t get_bitrate_kbps() const = 0;

    virtual void shutdown() = 0;
};

#endif /* AUDIO_ENCODER_H */
