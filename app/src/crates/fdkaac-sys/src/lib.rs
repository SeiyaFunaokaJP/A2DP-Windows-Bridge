//! The Fraunhofer FDK AAC library: the AAC-LC encoder BlueCaster streams AAC
//! with, and (feature `decoder`) the decoder for round-trip tests.
//!
//! # Licence
//!
//! fdk-aac is under the "Software License for The Fraunhofer FDK AAC Codec
//! Library for Android" (extern/fdk-aac/NOTICE, SPDX `FDK-AAC`): free
//! redistribution in source and binary form with the licence text, no
//! endorsement by Fraunhofer, and no patent licence (AAC patent licensing is
//! the distributor's matter; THIRD_PARTY_LICENSES.md §3). It is kept in this
//! crate of its own like the other C codecs. The Rust code here is
//! Apache-2.0 OR MIT like the rest of BlueCaster.
//!
//! # Layout
//!
//! - [`ffi`]: the C API declared by hand (layouts checked by the tests).
//! - [`Encoder`]: a safe owner of a `HANDLE_AACENCODER`.
//! - [`Decoder`] (feature `decoder`): a safe owner of a `HANDLE_AACDECODER`.

pub mod ffi;

use std::ffi::c_void;
use std::fmt;
use std::ptr::{self, NonNull};

/// An `AACENC_ERROR` other than AACENC_OK
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncError(pub ffi::AACENC_ERROR);

impl fmt::Display for EncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self.0 {
            ffi::AACENC_INVALID_HANDLE => "AACENC_INVALID_HANDLE",
            ffi::AACENC_MEMORY_ERROR => "AACENC_MEMORY_ERROR",
            ffi::AACENC_UNSUPPORTED_PARAMETER => "AACENC_UNSUPPORTED_PARAMETER",
            ffi::AACENC_INVALID_CONFIG => "AACENC_INVALID_CONFIG",
            ffi::AACENC_INIT_ERROR => "AACENC_INIT_ERROR",
            ffi::AACENC_INIT_AAC_ERROR => "AACENC_INIT_AAC_ERROR",
            ffi::AACENC_INIT_SBR_ERROR => "AACENC_INIT_SBR_ERROR",
            ffi::AACENC_INIT_TP_ERROR => "AACENC_INIT_TP_ERROR",
            ffi::AACENC_INIT_META_ERROR => "AACENC_INIT_META_ERROR",
            ffi::AACENC_INIT_MPS_ERROR => "AACENC_INIT_MPS_ERROR",
            ffi::AACENC_ENCODE_ERROR => "AACENC_ENCODE_ERROR",
            ffi::AACENC_ENCODE_EOF => "AACENC_ENCODE_EOF",
            _ => "AACENC error",
        };
        write!(f, "{name} (0x{:04X})", self.0)
    }
}

impl std::error::Error for EncError {}

fn enc_result(err: ffi::AACENC_ERROR) -> Result<(), EncError> {
    if err == ffi::AACENC_OK { Ok(()) } else { Err(EncError(err)) }
}

/// What one [`Encoder::encode`] call did
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EncodeOutput {
    /// Bitstream bytes written to the output (0 while the encoder is still
    /// filling its input buffer)
    pub out_bytes: usize,
    /// Input samples (all channels) the encoder took
    pub in_samples: usize,
}

/// An fdk-aac encoder instance
pub struct Encoder {
    handle: NonNull<ffi::AACENCODER>,
}

// SAFETY: the instance is heap state owned by this value; fdk-aac keeps no
// thread-local or unsynchronized global state, so it may move between threads.
unsafe impl Send for Encoder {}

impl Encoder {
    /// `aacEncOpen`: `enc_modules` 0 = all modules, `max_channels` 0 = all
    pub fn open(enc_modules: u32, max_channels: u32) -> Result<Encoder, EncError> {
        let mut handle: ffi::HANDLE_AACENCODER = ptr::null_mut();
        // SAFETY: aacEncOpen stores a new instance into the handle it is given
        enc_result(unsafe { ffi::aacEncOpen(&mut handle, enc_modules, max_channels) })?;
        NonNull::new(handle).map(|handle| Encoder { handle }).ok_or(EncError(ffi::AACENC_MEMORY_ERROR))
    }

    /// `aacEncoder_SetParam`; takes effect at the next [`Encoder::init`] or
    /// [`Encoder::encode`]
    pub fn set_param(&mut self, param: ffi::AACENC_PARAM, value: u32) -> Result<(), EncError> {
        // SAFETY: self.handle is a live instance; unknown parameters are
        // rejected by the library
        enc_result(unsafe { ffi::aacEncoder_SetParam(self.handle.as_ptr(), param, value) })
    }

    /// `aacEncoder_GetParam` (the effective value after initialization)
    pub fn param(&self, param: ffi::AACENC_PARAM) -> u32 {
        // SAFETY: self.handle is a live instance; GetParam only reads it
        unsafe { ffi::aacEncoder_GetParam(self.handle.as_ptr(), param) }
    }

    /// Applies the parameters: `aacEncEncode` with no buffers
    pub fn init(&mut self) -> Result<(), EncError> {
        // SAFETY: self.handle is a live instance; null descriptors and args
        // ask for (re)initialization only
        enc_result(unsafe {
            ffi::aacEncEncode(self.handle.as_ptr(), ptr::null(), ptr::null(), ptr::null(), ptr::null_mut())
        })
    }

    /// `aacEncInfo`
    pub fn info(&self) -> Result<ffi::AACENC_InfoStruct, EncError> {
        let mut info = ffi::AACENC_InfoStruct::default();
        // SAFETY: self.handle is a live instance; info is a valid struct of
        // the C layout (checked by the layout test) that aacEncInfo fills
        enc_result(unsafe { ffi::aacEncInfo(self.handle.as_ptr(), &mut info) })?;
        Ok(info)
    }

    /// Encodes interleaved 16-bit `pcm` (all of it is offered; see
    /// [`EncodeOutput::in_samples`] for what was taken) into `out`. The
    /// output must hold a whole access unit (`maxOutBufBytes` of
    /// [`Encoder::info`]), or the call fails with AACENC_ENCODE_ERROR.
    /// Does not allocate.
    pub fn encode(&mut self, pcm: &[i16], out: &mut [u8]) -> Result<EncodeOutput, EncError> {
        let too_big = EncError(ffi::AACENC_UNSUPPORTED_PARAMETER);
        let num_samples = ffi::INT::try_from(pcm.len()).map_err(|_| too_big)?;
        let in_bytes = ffi::INT::try_from(std::mem::size_of_val(pcm)).map_err(|_| too_big)?;
        let out_bytes = ffi::INT::try_from(out.len()).unwrap_or(ffi::INT::MAX);

        // fdk-aac takes the input through a non-const void ** but only reads
        // it (FDK_deinterleave from the buffer into its own)
        let mut in_ptr = pcm.as_ptr().cast_mut().cast::<c_void>();
        let mut in_id = ffi::IN_AUDIO_DATA;
        let mut in_size = in_bytes;
        let mut in_el_size = std::mem::size_of::<ffi::INT_PCM>() as ffi::INT;
        let in_desc = ffi::AACENC_BufDesc {
            numBufs: 1,
            bufs: &mut in_ptr,
            bufferIdentifiers: &mut in_id,
            bufSizes: &mut in_size,
            bufElSizes: &mut in_el_size,
        };
        let mut out_ptr = out.as_mut_ptr().cast::<c_void>();
        let mut out_id = ffi::OUT_BITSTREAM_DATA;
        let mut out_size = out_bytes;
        let mut out_el_size: ffi::INT = 1;
        let out_desc = ffi::AACENC_BufDesc {
            numBufs: 1,
            bufs: &mut out_ptr,
            bufferIdentifiers: &mut out_id,
            bufSizes: &mut out_size,
            bufElSizes: &mut out_el_size,
        };
        let in_args = ffi::AACENC_InArgs { numInSamples: num_samples, numAncBytes: 0 };
        let mut out_args = ffi::AACENC_OutArgs::default();

        // SAFETY: self.handle is a live instance. The descriptors point at
        // locals that outlive the call. fdk-aac reads at most numInSamples
        // INT_PCM values from in_ptr (= pcm.len(), all within pcm) and never
        // writes them; it clears and writes at most bufSizes bytes at out_ptr
        // (out.len(), clamped to INT_MAX), returning AACENC_ENCODE_ERROR
        // instead when the access unit does not fit.
        let err = unsafe { ffi::aacEncEncode(self.handle.as_ptr(), &in_desc, &out_desc, &in_args, &mut out_args) };
        enc_result(err)?;
        Ok(EncodeOutput {
            out_bytes: usize::try_from(out_args.numOutBytes).unwrap_or(0),
            in_samples: usize::try_from(out_args.numInSamples).unwrap_or(0),
        })
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        let mut handle = self.handle.as_ptr();
        // SAFETY: the handle came from aacEncOpen and is closed only here
        unsafe { ffi::aacEncClose(&mut handle) };
    }
}

/// An `AAC_DECODER_ERROR` other than AAC_DEC_OK
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecError(pub ffi::AAC_DECODER_ERROR);

impl fmt::Display for DecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self.0 {
            ffi::AAC_DEC_OUT_OF_MEMORY => "AAC_DEC_OUT_OF_MEMORY",
            ffi::AAC_DEC_UNKNOWN => "AAC_DEC_UNKNOWN",
            ffi::AAC_DEC_TRANSPORT_SYNC_ERROR => "AAC_DEC_TRANSPORT_SYNC_ERROR",
            ffi::AAC_DEC_NOT_ENOUGH_BITS => "AAC_DEC_NOT_ENOUGH_BITS",
            _ => "AAC decoder error",
        };
        write!(f, "{name} (0x{:04X})", self.0)
    }
}

impl std::error::Error for DecError {}

/// The leading members of `CStreamInfo`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamInfo {
    pub sample_rate: i32,
    pub frame_size: i32,
    pub num_channels: i32,
}

/// An fdk-aac decoder instance
#[cfg(feature = "decoder")]
pub struct Decoder {
    handle: NonNull<ffi::AAC_DECODER_INSTANCE>,
}

// SAFETY: as for Encoder: heap state owned by this value, no thread affinity
#[cfg(feature = "decoder")]
unsafe impl Send for Decoder {}

#[cfg(feature = "decoder")]
impl Decoder {
    /// `aacDecoder_Open`; `None` when the library refuses (out of memory)
    pub fn open(transport: ffi::TRANSPORT_TYPE, layers: u32) -> Option<Decoder> {
        // SAFETY: aacDecoder_Open only creates a new instance
        let handle = unsafe { ffi::aacDecoder_Open(transport, layers) };
        NonNull::new(handle).map(|handle| Decoder { handle })
    }

    /// `aacDecoder_Fill`: copies `data` into the decoder's input buffer.
    /// Returns how many bytes did not fit (offer them again later).
    pub fn fill(&mut self, data: &[u8]) -> Result<usize, DecError> {
        let size = ffi::UINT::try_from(data.len()).map_err(|_| DecError(ffi::AAC_DEC_UNKNOWN))?;
        // The API takes UCHAR *[] but only reads the buffers
        let mut buf = data.as_ptr().cast_mut();
        let mut valid = size;
        // SAFETY: self.handle is live; one buffer of `size` readable bytes is
        // passed, which aacDecoder_Fill only copies from; valid is in/out
        let err = unsafe { ffi::aacDecoder_Fill(self.handle.as_ptr(), &mut buf, &size, &mut valid) };
        if err != ffi::AAC_DEC_OK {
            return Err(DecError(err));
        }
        Ok(valid as usize)
    }

    /// `aacDecoder_DecodeFrame` into interleaved 16-bit `out`
    pub fn decode_frame(&mut self, out: &mut [i16], flags: u32) -> Result<(), DecError> {
        let size = ffi::INT::try_from(out.len()).unwrap_or(ffi::INT::MAX);
        // SAFETY: self.handle is live; the decoder writes at most
        // timeDataSize INT_PCM samples (out.len(), clamped) into out
        let err = unsafe { ffi::aacDecoder_DecodeFrame(self.handle.as_ptr(), out.as_mut_ptr(), size, flags) };
        if err != ffi::AAC_DEC_OK {
            return Err(DecError(err));
        }
        Ok(())
    }

    /// The decoded stream's parameters (zero before the first frame)
    pub fn stream_info(&self) -> Option<StreamInfo> {
        // SAFETY: self.handle is live; the returned pointer (or NULL) points
        // into the instance and is read here before any other call
        unsafe {
            let info = ffi::aacDecoder_GetStreamInfo(self.handle.as_ptr());
            info.as_ref().map(|i| StreamInfo {
                sample_rate: i.sampleRate,
                frame_size: i.frameSize,
                num_channels: i.numChannels,
            })
        }
    }
}

#[cfg(feature = "decoder")]
impl Drop for Decoder {
    fn drop(&mut self) {
        // SAFETY: the handle came from aacDecoder_Open and is closed only here
        unsafe { ffi::aacDecoder_Close(self.handle.as_ptr()) }
    }
}

#[cfg(test)]
mod tests {
    use super::ffi::*;
    use super::*;
    use std::collections::BTreeMap;
    use std::ffi::{CStr, c_char};
    use std::mem::{offset_of, size_of};

    unsafe extern "C" {
        // csrc/layout.c
        fn bluecaster_fdkaac_layout_count() -> usize;
        fn bluecaster_fdkaac_layout_key(i: usize) -> *const c_char;
        fn bluecaster_fdkaac_layout_value(i: usize) -> usize;
    }

    /// The layout the C compiler gives the hand-declared types (csrc/layout.c)
    fn c_layout() -> BTreeMap<String, usize> {
        // SAFETY: plain functions over a static table; every key below the
        // count is a NUL-terminated string literal
        unsafe {
            (0..bluecaster_fdkaac_layout_count())
                .map(|i| {
                    let key = CStr::from_ptr(bluecaster_fdkaac_layout_key(i)).to_str().unwrap().to_owned();
                    (key, bluecaster_fdkaac_layout_value(i))
                })
                .collect()
        }
    }

    macro_rules! size {
        ($m:ident, $t:ty) => {
            $m.insert(concat!("sizeof ", stringify!($t)).to_owned(), size_of::<$t>());
        };
    }
    macro_rules! off {
        ($m:ident, $c:literal, $t:ty, $f:ident) => {
            $m.insert(concat!($c, ".", stringify!($f)).to_owned(), offset_of!($t, $f));
        };
    }

    #[test]
    fn struct_layouts_match_c() {
        let mut r = BTreeMap::new();
        size!(r, INT);
        size!(r, UINT);
        size!(r, UCHAR);
        size!(r, INT_PCM);
        size!(r, AACENC_ERROR);
        size!(r, AACENC_PARAM);
        size!(r, AACENC_BufferIdentifier);
        size!(r, TRANSPORT_TYPE);
        size!(r, AAC_DECODER_ERROR);
        size!(r, HANDLE_AACENCODER);
        size!(r, HANDLE_AACDECODER);
        size!(r, AACENC_BufDesc);
        off!(r, "AACENC_BufDesc", AACENC_BufDesc, numBufs);
        off!(r, "AACENC_BufDesc", AACENC_BufDesc, bufs);
        off!(r, "AACENC_BufDesc", AACENC_BufDesc, bufferIdentifiers);
        off!(r, "AACENC_BufDesc", AACENC_BufDesc, bufSizes);
        off!(r, "AACENC_BufDesc", AACENC_BufDesc, bufElSizes);
        size!(r, AACENC_InArgs);
        off!(r, "AACENC_InArgs", AACENC_InArgs, numInSamples);
        off!(r, "AACENC_InArgs", AACENC_InArgs, numAncBytes);
        size!(r, AACENC_OutArgs);
        off!(r, "AACENC_OutArgs", AACENC_OutArgs, numOutBytes);
        off!(r, "AACENC_OutArgs", AACENC_OutArgs, numInSamples);
        off!(r, "AACENC_OutArgs", AACENC_OutArgs, numAncBytes);
        off!(r, "AACENC_OutArgs", AACENC_OutArgs, bitResState);
        size!(r, AACENC_InfoStruct);
        off!(r, "AACENC_InfoStruct", AACENC_InfoStruct, maxOutBufBytes);
        off!(r, "AACENC_InfoStruct", AACENC_InfoStruct, maxAncBytes);
        off!(r, "AACENC_InfoStruct", AACENC_InfoStruct, inBufFillLevel);
        off!(r, "AACENC_InfoStruct", AACENC_InfoStruct, inputChannels);
        off!(r, "AACENC_InfoStruct", AACENC_InfoStruct, frameLength);
        off!(r, "AACENC_InfoStruct", AACENC_InfoStruct, nDelay);
        off!(r, "AACENC_InfoStruct", AACENC_InfoStruct, nDelayCore);
        off!(r, "AACENC_InfoStruct", AACENC_InfoStruct, confBuf);
        off!(r, "AACENC_InfoStruct", AACENC_InfoStruct, confSize);
        off!(r, "CStreamInfo", CStreamInfoHead, sampleRate);
        off!(r, "CStreamInfo", CStreamInfoHead, frameSize);
        off!(r, "CStreamInfo", CStreamInfoHead, numChannels);

        let c = c_layout();
        assert_eq!(r, c, "Rust (left) and C (right) layouts differ");
    }

    #[test]
    fn open_configure_close() {
        let mut enc = Encoder::open(0, 2).unwrap();
        enc.set_param(AACENC_AOT, AOT_AAC_LC as u32).unwrap();
        enc.set_param(AACENC_SAMPLERATE, 48000).unwrap();
        enc.set_param(AACENC_CHANNELMODE, MODE_2 as u32).unwrap();
        enc.set_param(AACENC_BITRATE, 256000).unwrap();
        enc.set_param(AACENC_TRANSMUX, TT_MP4_LATM_MCP1 as u32).unwrap();
        enc.init().unwrap();
        let info = enc.info().unwrap();
        assert_eq!(info.frameLength, 1024);
        assert_eq!(info.inputChannels, 2);
        assert!(info.maxOutBufBytes >= 768 * 2);
        assert_eq!(enc.param(AACENC_BITRATE), 256000);
        // An unknown parameter is an error, not a crash
        assert_eq!(enc.set_param(0x7777, 1), Err(EncError(AACENC_UNSUPPORTED_PARAMETER)));
        // Output too small for an access unit
        let pcm = [0i16; 2048];
        assert_eq!(enc.encode(&pcm, &mut [0u8; 4]), Err(EncError(AACENC_ENCODE_ERROR)));
    }
}
