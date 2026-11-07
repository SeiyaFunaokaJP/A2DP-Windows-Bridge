/*
 * sizeof / offsetof of the fdk-aac structs and types that src/ffi.rs
 * declares by hand, as the C compiler lays them out. The unit tests of the
 * crate compare every entry with the Rust layout, so a header change or a
 * typo in a #[repr(C)] struct fails the tests instead of corrupting memory.
 *
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 */

#include <stddef.h>

#include "aacenc_lib.h"
#include "aacdecoder_lib.h"

#define SIZE(T) { "sizeof " #T, sizeof(T) }
#define OFF(T, f) { #T "." #f, offsetof(T, f) }

static const struct {
    const char *key;
    size_t value;
} layout[] = {
    SIZE(INT),
    SIZE(UINT),
    SIZE(UCHAR),
    SIZE(INT_PCM),
    SIZE(AACENC_ERROR),
    SIZE(AACENC_PARAM),
    SIZE(AACENC_BufferIdentifier),
    SIZE(TRANSPORT_TYPE),
    SIZE(AAC_DECODER_ERROR),
    SIZE(HANDLE_AACENCODER),
    SIZE(HANDLE_AACDECODER),

    SIZE(AACENC_BufDesc),
    OFF(AACENC_BufDesc, numBufs),
    OFF(AACENC_BufDesc, bufs),
    OFF(AACENC_BufDesc, bufferIdentifiers),
    OFF(AACENC_BufDesc, bufSizes),
    OFF(AACENC_BufDesc, bufElSizes),

    SIZE(AACENC_InArgs),
    OFF(AACENC_InArgs, numInSamples),
    OFF(AACENC_InArgs, numAncBytes),

    SIZE(AACENC_OutArgs),
    OFF(AACENC_OutArgs, numOutBytes),
    OFF(AACENC_OutArgs, numInSamples),
    OFF(AACENC_OutArgs, numAncBytes),
    OFF(AACENC_OutArgs, bitResState),

    SIZE(AACENC_InfoStruct),
    OFF(AACENC_InfoStruct, maxOutBufBytes),
    OFF(AACENC_InfoStruct, maxAncBytes),
    OFF(AACENC_InfoStruct, inBufFillLevel),
    OFF(AACENC_InfoStruct, inputChannels),
    OFF(AACENC_InfoStruct, frameLength),
    OFF(AACENC_InfoStruct, nDelay),
    OFF(AACENC_InfoStruct, nDelayCore),
    OFF(AACENC_InfoStruct, confBuf),
    OFF(AACENC_InfoStruct, confSize),

    /* Rust declares only the leading members it reads (CStreamInfoHead) */
    OFF(CStreamInfo, sampleRate),
    OFF(CStreamInfo, frameSize),
    OFF(CStreamInfo, numChannels),
};

size_t bluecaster_fdkaac_layout_count(void) {
    return sizeof(layout) / sizeof(layout[0]);
}

const char *bluecaster_fdkaac_layout_key(size_t i) {
    return i < bluecaster_fdkaac_layout_count() ? layout[i].key : NULL;
}

size_t bluecaster_fdkaac_layout_value(size_t i) {
    return i < bluecaster_fdkaac_layout_count() ? layout[i].value : (size_t)-1;
}
