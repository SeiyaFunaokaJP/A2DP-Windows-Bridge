/*
 * aptX PCM packing helper (shared by the aptX / aptX HD / aptX LL encoders)
 *
 * libopenaptx's aptx_encode() consumes groups of 4 stereo samples given as
 * PACKED 24-bit signed little-endian values: 24 bytes LLLRRRLLLRRRLLLRRRLLLRRR
 * (openaptx.h aptx_encode(); openaptx.c reads input[ipos+0..2] per sample).
 * It emits 4 bytes per group for aptX and 6 bytes for aptX HD.
 *
 * Supported encoder input formats (interleaved, 1 or 2 channels):
 *   bytes_per_sample == 2: signed 16-bit PCM            -> 24-bit via << 8
 *   bytes_per_sample == 4: signed 32-bit container, MSB-aligned full scale
 *                          (as produced by convert_f32_to_i32, i.e. the
 *                          "24-bit" path used for LDAC)  -> 24-bit via >> 8
 * Mono input is duplicated to L and R (aptX is always sent as stereo).
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef APTX_PCM_PACK_H
#define APTX_PCM_PACK_H

#include <cstdint>
#include <cstring>

static constexpr uint32_t APTX_GROUP_SAMPLES = 4;                          /* per channel */
static constexpr uint32_t APTX_GROUP_PACKED_BYTES = APTX_GROUP_SAMPLES * 2 * 3; /* 24 */

/* Read one sample as a 24-bit value (stored in the low 3 bytes of a uint32). */
static inline uint32_t aptx_read_sample24(const uint8_t *p, uint32_t bytes_per_sample) {
    if (bytes_per_sample == 4) {
        int32_t v;
        memcpy(&v, p, sizeof(v));
        return static_cast<uint32_t>(v >> 8);           /* keep top 24 bits */
    }
    int16_t v;
    memcpy(&v, p, sizeof(v));
    return static_cast<uint32_t>(static_cast<int32_t>(v) * 256); /* 16 -> 24 bit */
}

/*
 * Pack PCM group `group` (4 sample-frames) into out[24] as LLLRRR x4.
 * pcm points at the start of the interleaved buffer.
 */
static inline void aptx_pack_group(const uint8_t *pcm, uint32_t group,
                                   uint32_t channels, uint32_t bytes_per_sample,
                                   uint8_t out[APTX_GROUP_PACKED_BYTES]) {
    const uint32_t frame_bytes = channels * bytes_per_sample;
    const uint8_t *src = pcm + group * APTX_GROUP_SAMPLES * frame_bytes;
    uint32_t p = 0;
    for (uint32_t s = 0; s < APTX_GROUP_SAMPLES; s++) {
        const uint8_t *f = src + s * frame_bytes;
        uint32_t l = aptx_read_sample24(f, bytes_per_sample);
        uint32_t r = (channels >= 2) ? aptx_read_sample24(f + bytes_per_sample, bytes_per_sample) : l;
        out[p++] = static_cast<uint8_t>(l);
        out[p++] = static_cast<uint8_t>(l >> 8);
        out[p++] = static_cast<uint8_t>(l >> 16);
        out[p++] = static_cast<uint8_t>(r);
        out[p++] = static_cast<uint8_t>(r >> 8);
        out[p++] = static_cast<uint8_t>(r >> 16);
    }
}

#endif /* APTX_PCM_PACK_H */
