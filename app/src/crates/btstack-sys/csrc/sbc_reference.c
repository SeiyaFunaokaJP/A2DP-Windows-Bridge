/*
 * sbc_reference.c - the Bluedroid SBC encoder (C) as a reference for the
 * bit-exactness test of codec's Rust SBC encoder (feature "sbc-reference").
 * Not used by the application.
 *
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 */

#include <stdlib.h>
#include <string.h>

#include "sbc_encoder.h"

typedef struct {
    SBC_ENC_PARAMS params;
    UINT8 packet[1024];
} sbc_reference_t;

void *bluecaster_sbc_ref_new(UINT16 sample_rate, int channel_mode, int blocks, int subbands, int allocation,
                         int bitpool) {
    sbc_reference_t *r = (sbc_reference_t *)calloc(1, sizeof(sbc_reference_t));
    if (r == NULL) return NULL;
    SBC_ENC_PARAMS *p = &r->params;
    p->s16NumOfBlocks = (SINT16)blocks;
    p->s16NumOfSubBands = (SINT16)subbands;
    p->s16AllocationMethod = (SINT16)allocation;
    p->s16BitPool = (SINT16)bitpool;
    p->mSBCEnabled = 0;
    p->s16ChannelMode = (SINT16)channel_mode;
    p->s16NumOfChannels = channel_mode == SBC_MONO ? 1 : 2;
    switch (sample_rate) {
    case 16000: p->s16SamplingFreq = SBC_sf16000; break;
    case 32000: p->s16SamplingFreq = SBC_sf32000; break;
    case 44100: p->s16SamplingFreq = SBC_sf44100; break;
    default:    p->s16SamplingFreq = SBC_sf48000; break;
    }
    p->pu8Packet = r->packet;
    SBC_Encoder_Init(p);
    return r;
}

/* One frame from blocks * subbands interleaved samples per channel;
 * returns its length, or -1 if out_cap is too small */
int bluecaster_sbc_ref_encode(void *state, const SINT16 *pcm, UINT8 *out, int out_cap) {
    sbc_reference_t *r = (sbc_reference_t *)state;
    r->params.ps16PcmBuffer = (SINT16 *)pcm;
    SBC_Encoder(&r->params);
    int n = r->params.u16PacketLength;
    if (n > out_cap) return -1;
    memcpy(out, r->packet, (size_t)n);
    return n;
}

void bluecaster_sbc_ref_free(void *state) {
    free(state);
}
