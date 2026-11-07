//! Hand-written declarations of the fdk-aac API BlueCaster uses: aacenc_lib.h
//! (encoder), the types of FDK_audio.h / machine_type.h it needs, and, with
//! the `decoder` feature, part of aacdecoder_lib.h.
//!
//! C enums are declared as `c_int` aliases with constants, never as Rust
//! enums: a value the library returns that Rust does not list would be
//! undefined behaviour in a Rust enum. The struct layouts are checked against
//! the C compiler by the unit tests (csrc/layout.c).

#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

use std::ffi::{c_int, c_uchar, c_uint, c_void};
use std::marker::{PhantomData, PhantomPinned};

// machine_type.h
pub type INT = c_int;
pub type UINT = c_uint;
pub type UCHAR = c_uchar;
/// `typedef SHORT INT_PCM;` (SAMPLE_BITS 16, the default build)
pub type INT_PCM = i16;

// FDK_audio.h

/// `TRANSPORT_TYPE`
pub type TRANSPORT_TYPE = c_int;
pub const TT_UNKNOWN: TRANSPORT_TYPE = -1;
pub const TT_MP4_RAW: TRANSPORT_TYPE = 0;
pub const TT_MP4_ADIF: TRANSPORT_TYPE = 1;
pub const TT_MP4_ADTS: TRANSPORT_TYPE = 2;
/// Audio Mux Elements with muxConfigPresent = 1 (in-band StreamMuxConfig)
pub const TT_MP4_LATM_MCP1: TRANSPORT_TYPE = 6;
/// Audio Mux Elements with muxConfigPresent = 0 (out-of-band StreamMuxConfig)
pub const TT_MP4_LATM_MCP0: TRANSPORT_TYPE = 7;
pub const TT_MP4_LOAS: TRANSPORT_TYPE = 10;

/// `AUDIO_OBJECT_TYPE` (only the value A2DP uses)
pub type AUDIO_OBJECT_TYPE = c_int;
pub const AOT_AAC_LC: AUDIO_OBJECT_TYPE = 2;

/// `CHANNEL_MODE` (only the values A2DP uses)
pub type CHANNEL_MODE = c_int;
pub const MODE_1: CHANNEL_MODE = 1;
pub const MODE_2: CHANNEL_MODE = 2;

// aacenc_lib.h

/// `AACENC_ERROR`
pub type AACENC_ERROR = c_int;
pub const AACENC_OK: AACENC_ERROR = 0x0000;
pub const AACENC_INVALID_HANDLE: AACENC_ERROR = 0x0020;
pub const AACENC_MEMORY_ERROR: AACENC_ERROR = 0x0021;
pub const AACENC_UNSUPPORTED_PARAMETER: AACENC_ERROR = 0x0022;
pub const AACENC_INVALID_CONFIG: AACENC_ERROR = 0x0023;
pub const AACENC_INIT_ERROR: AACENC_ERROR = 0x0040;
pub const AACENC_INIT_AAC_ERROR: AACENC_ERROR = 0x0041;
pub const AACENC_INIT_SBR_ERROR: AACENC_ERROR = 0x0042;
pub const AACENC_INIT_TP_ERROR: AACENC_ERROR = 0x0043;
pub const AACENC_INIT_META_ERROR: AACENC_ERROR = 0x0044;
pub const AACENC_INIT_MPS_ERROR: AACENC_ERROR = 0x0045;
pub const AACENC_ENCODE_ERROR: AACENC_ERROR = 0x0060;
pub const AACENC_ENCODE_EOF: AACENC_ERROR = 0x0080;

/// `AACENC_BufferIdentifier`
pub type AACENC_BufferIdentifier = c_int;
pub const IN_AUDIO_DATA: AACENC_BufferIdentifier = 0;
pub const IN_ANCILLRY_DATA: AACENC_BufferIdentifier = 1;
pub const IN_METADATA_SETUP: AACENC_BufferIdentifier = 2;
pub const OUT_BITSTREAM_DATA: AACENC_BufferIdentifier = 3;
pub const OUT_AU_SIZES: AACENC_BufferIdentifier = 4;

/// `AACENC_PARAM`
pub type AACENC_PARAM = c_int;
pub const AACENC_AOT: AACENC_PARAM = 0x0100;
pub const AACENC_BITRATE: AACENC_PARAM = 0x0101;
pub const AACENC_BITRATEMODE: AACENC_PARAM = 0x0102;
pub const AACENC_SAMPLERATE: AACENC_PARAM = 0x0103;
pub const AACENC_SBR_MODE: AACENC_PARAM = 0x0104;
pub const AACENC_GRANULE_LENGTH: AACENC_PARAM = 0x0105;
pub const AACENC_CHANNELMODE: AACENC_PARAM = 0x0106;
pub const AACENC_CHANNELORDER: AACENC_PARAM = 0x0107;
pub const AACENC_SBR_RATIO: AACENC_PARAM = 0x0108;
pub const AACENC_AFTERBURNER: AACENC_PARAM = 0x0200;
pub const AACENC_BANDWIDTH: AACENC_PARAM = 0x0203;
pub const AACENC_PEAK_BITRATE: AACENC_PARAM = 0x0207;
pub const AACENC_TRANSMUX: AACENC_PARAM = 0x0300;
pub const AACENC_HEADER_PERIOD: AACENC_PARAM = 0x0301;
pub const AACENC_SIGNALING_MODE: AACENC_PARAM = 0x0302;
pub const AACENC_TPSUBFRAMES: AACENC_PARAM = 0x0303;
pub const AACENC_AUDIOMUXVER: AACENC_PARAM = 0x0304;
pub const AACENC_PROTECTION: AACENC_PARAM = 0x0306;
pub const AACENC_ANCILLARY_BITRATE: AACENC_PARAM = 0x0500;
pub const AACENC_METADATA_MODE: AACENC_PARAM = 0x0600;
pub const AACENC_CONTROL_STATE: AACENC_PARAM = 0xFF00;
pub const AACENC_NONE: AACENC_PARAM = 0xFFFF;

/// `struct AACENCODER` (opaque)
#[repr(C)]
pub struct AACENCODER {
    _private: [u8; 0],
    _marker: PhantomData<(*mut u8, PhantomPinned)>,
}
pub type HANDLE_AACENCODER = *mut AACENCODER;

/// `AACENC_InfoStruct`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AACENC_InfoStruct {
    pub maxOutBufBytes: UINT,
    pub maxAncBytes: UINT,
    pub inBufFillLevel: UINT,
    pub inputChannels: UINT,
    pub frameLength: UINT,
    pub nDelay: UINT,
    pub nDelayCore: UINT,
    pub confBuf: [UCHAR; 64],
    pub confSize: UINT,
}

impl Default for AACENC_InfoStruct {
    fn default() -> Self {
        AACENC_InfoStruct {
            maxOutBufBytes: 0,
            maxAncBytes: 0,
            inBufFillLevel: 0,
            inputChannels: 0,
            frameLength: 0,
            nDelay: 0,
            nDelayCore: 0,
            confBuf: [0; 64],
            confSize: 0,
        }
    }
}

/// `AACENC_BufDesc`
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AACENC_BufDesc {
    pub numBufs: INT,
    pub bufs: *mut *mut c_void,
    pub bufferIdentifiers: *mut INT,
    pub bufSizes: *mut INT,
    pub bufElSizes: *mut INT,
}

/// `AACENC_InArgs`
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct AACENC_InArgs {
    pub numInSamples: INT,
    pub numAncBytes: INT,
}

/// `AACENC_OutArgs`
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct AACENC_OutArgs {
    pub numOutBytes: INT,
    pub numInSamples: INT,
    pub numAncBytes: INT,
    pub bitResState: INT,
}

unsafe extern "C" {
    /// `AACENC_ERROR aacEncOpen(HANDLE_AACENCODER *phAacEncoder, const UINT encModules,
    /// const UINT maxChannels);`
    pub fn aacEncOpen(phAacEncoder: *mut HANDLE_AACENCODER, encModules: UINT, maxChannels: UINT) -> AACENC_ERROR;

    /// `AACENC_ERROR aacEncClose(HANDLE_AACENCODER *phAacEncoder);`
    pub fn aacEncClose(phAacEncoder: *mut HANDLE_AACENCODER) -> AACENC_ERROR;

    /// `AACENC_ERROR aacEncEncode(const HANDLE_AACENCODER hAacEncoder,
    /// const AACENC_BufDesc *inBufDesc, const AACENC_BufDesc *outBufDesc,
    /// const AACENC_InArgs *inargs, AACENC_OutArgs *outargs);`
    pub fn aacEncEncode(
        hAacEncoder: HANDLE_AACENCODER,
        inBufDesc: *const AACENC_BufDesc,
        outBufDesc: *const AACENC_BufDesc,
        inargs: *const AACENC_InArgs,
        outargs: *mut AACENC_OutArgs,
    ) -> AACENC_ERROR;

    /// `AACENC_ERROR aacEncInfo(const HANDLE_AACENCODER hAacEncoder, AACENC_InfoStruct *pInfo);`
    pub fn aacEncInfo(hAacEncoder: HANDLE_AACENCODER, pInfo: *mut AACENC_InfoStruct) -> AACENC_ERROR;

    /// `AACENC_ERROR aacEncoder_SetParam(const HANDLE_AACENCODER hAacEncoder,
    /// const AACENC_PARAM param, const UINT value);`
    pub fn aacEncoder_SetParam(hAacEncoder: HANDLE_AACENCODER, param: AACENC_PARAM, value: UINT) -> AACENC_ERROR;

    /// `UINT aacEncoder_GetParam(const HANDLE_AACENCODER hAacEncoder, const AACENC_PARAM param);`
    pub fn aacEncoder_GetParam(hAacEncoder: HANDLE_AACENCODER, param: AACENC_PARAM) -> UINT;
}

// aacdecoder_lib.h

/// `AAC_DECODER_ERROR`
pub type AAC_DECODER_ERROR = c_int;
pub const AAC_DEC_OK: AAC_DECODER_ERROR = 0x0000;
pub const AAC_DEC_OUT_OF_MEMORY: AAC_DECODER_ERROR = 0x0002;
pub const AAC_DEC_UNKNOWN: AAC_DECODER_ERROR = 0x0005;
pub const AAC_DEC_TRANSPORT_SYNC_ERROR: AAC_DECODER_ERROR = 0x1001;
pub const AAC_DEC_NOT_ENOUGH_BITS: AAC_DECODER_ERROR = 0x1002;

/// aacDecoder_DecodeFrame() flags
pub const AACDEC_CONCEAL: UINT = 1;
pub const AACDEC_FLUSH: UINT = 2;
pub const AACDEC_INTR: UINT = 4;
pub const AACDEC_CLRHIST: UINT = 8;

/// `struct AAC_DECODER_INSTANCE` (opaque)
#[repr(C)]
pub struct AAC_DECODER_INSTANCE {
    _private: [u8; 0],
    _marker: PhantomData<(*mut u8, PhantomPinned)>,
}
pub type HANDLE_AACDECODER = *mut AAC_DECODER_INSTANCE;

/// The leading members of `CStreamInfo` ("the only really relevant ones for
/// the user"). Only ever read through the pointer aacDecoder_GetStreamInfo()
/// returns, never created or copied whole, so the members after these need
/// not be declared.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CStreamInfoHead {
    pub sampleRate: INT,
    pub frameSize: INT,
    pub numChannels: INT,
}

#[cfg(feature = "decoder")]
unsafe extern "C" {
    /// `HANDLE_AACDECODER aacDecoder_Open(TRANSPORT_TYPE transportFmt, UINT nrOfLayers);`
    pub fn aacDecoder_Open(transportFmt: TRANSPORT_TYPE, nrOfLayers: UINT) -> HANDLE_AACDECODER;

    /// `AAC_DECODER_ERROR aacDecoder_Fill(HANDLE_AACDECODER self, UCHAR *pBuffer[],
    /// const UINT bufferSize[], UINT *bytesValid);`
    pub fn aacDecoder_Fill(
        self_: HANDLE_AACDECODER,
        pBuffer: *mut *mut UCHAR,
        bufferSize: *const UINT,
        bytesValid: *mut UINT,
    ) -> AAC_DECODER_ERROR;

    /// `AAC_DECODER_ERROR aacDecoder_DecodeFrame(HANDLE_AACDECODER self, INT_PCM *pTimeData,
    /// const INT timeDataSize, const UINT flags);`
    pub fn aacDecoder_DecodeFrame(
        self_: HANDLE_AACDECODER,
        pTimeData: *mut INT_PCM,
        timeDataSize: INT,
        flags: UINT,
    ) -> AAC_DECODER_ERROR;

    /// `void aacDecoder_Close(HANDLE_AACDECODER self);`
    pub fn aacDecoder_Close(self_: HANDLE_AACDECODER);

    /// `CStreamInfo *aacDecoder_GetStreamInfo(HANDLE_AACDECODER self);`
    pub fn aacDecoder_GetStreamInfo(self_: HANDLE_AACDECODER) -> *mut CStreamInfoHead;
}
