//! libldac, the LDAC encoder of AOSP (Sony, Apache-2.0), with its adaptive
//! bit rate module (ldacBT_abr), built from the git submodule extern/libldac.
//!
//! [`ffi`] has hand-written declarations of the functions BlueCaster uses,
//! matching inc/ldacBT.h and abr/inc/ldacBT_abr.h. [`LdacBt`] and [`LdacAbr`]
//! own the C handles and check what the C code cannot (input and output
//! buffer sizes, sample format), so that callers need no `unsafe`.

use std::fmt;
use std::ptr::NonNull;

/// Declarations from inc/ldacBT.h and abr/inc/ldacBT_abr.h
#[allow(non_camel_case_types, non_snake_case)]
pub mod ffi {
    use std::ffi::{c_int, c_uint, c_void};
    use std::marker::{PhantomData, PhantomPinned};

    /// `struct _st_ldacbt_handle` (opaque)
    #[repr(C)]
    pub struct LdacBtHandle {
        _data: (),
        _marker: PhantomData<(*mut u8, PhantomPinned)>,
    }

    /// `struct _ldacbt_abr_param` (opaque)
    #[repr(C)]
    pub struct LdacAbrHandle {
        _data: (),
        _marker: PhantomData<(*mut u8, PhantomPinned)>,
    }

    pub type HANDLE_LDAC_BT = *mut LdacBtHandle;
    pub type HANDLE_LDAC_ABR = *mut LdacAbrHandle;

    /// `LDACBT_SMPL_FMT_T` (a C enum: int-sized with MSVC and the usual ABIs)
    pub type LDACBT_SMPL_FMT_T = c_int;
    pub const LDACBT_SMPL_FMT_S16: LDACBT_SMPL_FMT_T = 0x2;
    pub const LDACBT_SMPL_FMT_S24: LDACBT_SMPL_FMT_T = 0x3;
    pub const LDACBT_SMPL_FMT_S32: LDACBT_SMPL_FMT_T = 0x4;
    pub const LDACBT_SMPL_FMT_F32: LDACBT_SMPL_FMT_T = 0x5;

    /// Encode quality mode indexes (the ones ldacBT_init_handle_encode takes)
    pub const LDACBT_EQMID_HQ: c_int = 0;
    pub const LDACBT_EQMID_SQ: c_int = 1;
    pub const LDACBT_EQMID_MQ: c_int = 2;
    /// Hidden modes ABR steps through between SQ and MQ (src/ldacBT_ex.h):
    /// 492 and 396 kbps at 48 / 96 kHz
    pub const LDACBT_EQMID_Q0: c_int = 3;
    pub const LDACBT_EQMID_Q1: c_int = 4;

    pub const LDACBT_EQMID_INC_QUALITY: c_int = 1;
    pub const LDACBT_EQMID_INC_CONNECTION: c_int = -1;

    pub const LDACBT_CHANNEL_MODE_MONO: c_int = 0x04;
    pub const LDACBT_CHANNEL_MODE_DUAL_CHANNEL: c_int = 0x02;
    pub const LDACBT_CHANNEL_MODE_STEREO: c_int = 0x01;

    /// PCM frames (samples per channel) ldacBT_encode takes per call, at
    /// every sample rate
    pub const LDACBT_ENC_LSU: usize = 128;
    /// Bytes of the frame header (sync word, sampling rate, channel config,
    /// frame length, frame status) in front of each LDAC frame
    pub const LDAC_FRMHDRBYTES: usize = 3;
    /// Frame sync word
    pub const LDAC_SYNCWORD: u8 = 0xAA;
    /// Smallest transport (L2CAP) MTU ldacBT_init_handle_encode accepts
    /// (src/ldacBT_internal.h LDACBT_MTU_REQUIRED)
    pub const LDACBT_MTU_REQUIRED: usize = 679;
    /// What libldac subtracts from the transport MTU for the L2CAP / RTP /
    /// media payload headers (src/ldacBT_internal.h LDACBT_TX_HEADER_SIZE)
    pub const LDACBT_TX_HEADER_SIZE: usize = 18;
    /// Size of libldac's internal output buffer (src/ldacBT_internal.h
    /// LDACBT_ENC_STREAM_BUF_SZ): one ldacBT_encode call never writes more
    pub const LDACBT_ENC_STREAM_BUF_SZ: usize = 1024;

    unsafe extern "C" {
        pub fn ldacBT_get_handle() -> HANDLE_LDAC_BT;
        pub fn ldacBT_free_handle(hLdacBt: HANDLE_LDAC_BT);
        pub fn ldacBT_close_handle(hLdacBt: HANDLE_LDAC_BT);
        pub fn ldacBT_get_version() -> c_int;
        pub fn ldacBT_get_sampling_freq(hLdacBt: HANDLE_LDAC_BT) -> c_int;
        pub fn ldacBT_get_bitrate(hLdacBt: HANDLE_LDAC_BT) -> c_int;
        pub fn ldacBT_init_handle_encode(
            hLdacBt: HANDLE_LDAC_BT,
            mtu: c_int,
            eqmid: c_int,
            cm: c_int,
            fmt: LDACBT_SMPL_FMT_T,
            sf: c_int,
        ) -> c_int;
        pub fn ldacBT_set_eqmid(hLdacBt: HANDLE_LDAC_BT, eqmid: c_int) -> c_int;
        pub fn ldacBT_get_eqmid(hLdacBt: HANDLE_LDAC_BT) -> c_int;
        pub fn ldacBT_alter_eqmid_priority(hLdacBt: HANDLE_LDAC_BT, priority: c_int) -> c_int;
        pub fn ldacBT_encode(
            hLdacBt: HANDLE_LDAC_BT,
            p_pcm: *mut c_void,
            pcm_used: *mut c_int,
            p_stream: *mut u8,
            stream_sz: *mut c_int,
            frame_num: *mut c_int,
        ) -> c_int;
        pub fn ldacBT_get_error_code(hLdacBt: HANDLE_LDAC_BT) -> c_int;

        pub fn ldac_ABR_get_handle() -> HANDLE_LDAC_ABR;
        pub fn ldac_ABR_free_handle(hLdacAbr: HANDLE_LDAC_ABR);
        pub fn ldac_ABR_Init(hLdacAbr: HANDLE_LDAC_ABR, interval_ms: c_uint) -> c_int;
        pub fn ldac_ABR_set_thresholds(
            hLdacAbr: HANDLE_LDAC_ABR,
            thCritical: c_uint,
            thDangerousTrend: c_uint,
            thSafety4HQSQ: c_uint,
        ) -> c_int;
        pub fn ldac_ABR_Proc(
            hLdacBt: HANDLE_LDAC_BT,
            hLdacAbr: HANDLE_LDAC_ABR,
            TxQueueDepth: c_uint,
            flagEnable: c_uint,
        ) -> c_int;
    }
}

/// Input sample format of the safe layer: interleaved 16-bit, or 32-bit
/// (full scale; BlueCaster feeds 24-bit audio MSB-aligned in it)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleFormat {
    S16,
    S32,
}

impl SampleFormat {
    fn raw(self) -> ffi::LDACBT_SMPL_FMT_T {
        match self {
            SampleFormat::S16 => ffi::LDACBT_SMPL_FMT_S16,
            SampleFormat::S32 => ffi::LDACBT_SMPL_FMT_S32,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// libldac could not allocate a handle
    Alloc,
    /// The handle has not been initialized for encoding
    NotInitialized,
    /// The input is not in the sample format the handle was initialized with
    WrongFormat,
    InputLength {
        expected: usize,
        got: usize,
    },
    OutputTooSmall {
        have: usize,
        need: usize,
    },
    /// libldac failed: its error code (ldacBT_get_error_code), or -1 from ABR
    Ldac(i32),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Error::Alloc => f.write_str("out of memory"),
            Error::NotInitialized => f.write_str("handle not initialized for encoding"),
            Error::WrongFormat => f.write_str("input sample format differs from the initialized one"),
            Error::InputLength { expected, got } => write!(f, "expected {expected} samples, got {got}"),
            Error::OutputTooSmall { have, need } => write!(f, "output buffer too small ({have} < {need} bytes)"),
            Error::Ldac(code) => write!(
                f,
                "libldac error 0x{code:08x} (api {}, handle {}, block {})",
                (code >> 20) & 0x0FFF,
                (code >> 10) & 0x03FF,
                code & 0x03FF
            ),
        }
    }
}

impl std::error::Error for Error {}

/// What one [`LdacBt::encode_i16`] / [`LdacBt::encode_i32`] call produced
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EncodeOutput {
    /// Input bytes libldac took (0 only if its PCM ring was full)
    pub pcm_used: usize,
    /// Bytes written to the output: 0, or a whole packet's worth of frames
    pub bytes: usize,
    /// LDAC frames in them
    pub frames: u32,
}

/// An LDAC encoder handle (`HANDLE_LDAC_BT`), closed and freed on drop
pub struct LdacBt {
    handle: NonNull<ffi::LdacBtHandle>,
    /// Format and channel count of a successful init_encode: what
    /// ldacBT_encode reads from the input
    layout: Option<(SampleFormat, usize)>,
}

// SAFETY: the handle is plain heap memory owned by this value, with no
// thread affinity or thread-local state in libldac; moving it to another
// thread is fine. It is not Sync: libldac is not thread-safe, and &mut self
// on every mutating call keeps calls on one thread at a time.
unsafe impl Send for LdacBt {}

impl LdacBt {
    /// ldacBT_get_handle
    pub fn new() -> Result<LdacBt, Error> {
        // SAFETY: no arguments; returns an owned handle or NULL
        let raw = unsafe { ffi::ldacBT_get_handle() };
        NonNull::new(raw).map(|handle| LdacBt { handle, layout: None }).ok_or(Error::Alloc)
    }

    pub fn as_ptr(&self) -> ffi::HANDLE_LDAC_BT {
        self.handle.as_ptr()
    }

    /// ldacBT_init_handle_encode. `mtu` is the transport (L2CAP) MTU, at
    /// least [`ffi::LDACBT_MTU_REQUIRED`]; `channel_mode` one of
    /// `LDACBT_CHANNEL_MODE_*`; `eqmid` HQ, SQ or MQ.
    pub fn init_encode(
        &mut self,
        mtu: i32,
        eqmid: i32,
        channel_mode: i32,
        format: SampleFormat,
        sample_rate: i32,
    ) -> Result<(), Error> {
        self.layout = None;
        // SAFETY: the handle is valid (owned by self); the other arguments
        // are plain values that libldac validates
        let ret = unsafe {
            ffi::ldacBT_init_handle_encode(self.as_ptr(), mtu, eqmid, channel_mode, format.raw(), sample_rate)
        };
        if ret != 0 {
            return Err(Error::Ldac(self.error_code()));
        }
        let channels = if channel_mode == ffi::LDACBT_CHANNEL_MODE_MONO { 1 } else { 2 };
        self.layout = Some((format, channels));
        Ok(())
    }

    /// Encodes [`ffi::LDACBT_ENC_LSU`] frames of interleaved 16-bit PCM. The
    /// output needs [`ffi::LDACBT_ENC_STREAM_BUF_SZ`] bytes.
    pub fn encode_i16(&mut self, pcm: &[i16], out: &mut [u8]) -> Result<EncodeOutput, Error> {
        self.check(SampleFormat::S16, pcm.len(), out.len())?;
        self.encode_raw(pcm.as_ptr().cast(), out)
    }

    /// Encodes [`ffi::LDACBT_ENC_LSU`] frames of interleaved 32-bit PCM. The
    /// output needs [`ffi::LDACBT_ENC_STREAM_BUF_SZ`] bytes.
    pub fn encode_i32(&mut self, pcm: &[i32], out: &mut [u8]) -> Result<EncodeOutput, Error> {
        self.check(SampleFormat::S32, pcm.len(), out.len())?;
        self.encode_raw(pcm.as_ptr().cast(), out)
    }

    fn check(&self, format: SampleFormat, samples: usize, out_len: usize) -> Result<(), Error> {
        let (fmt, channels) = self.layout.ok_or(Error::NotInitialized)?;
        if fmt != format {
            return Err(Error::WrongFormat);
        }
        let expected = ffi::LDACBT_ENC_LSU * channels;
        if samples != expected {
            return Err(Error::InputLength { expected, got: samples });
        }
        if out_len < ffi::LDACBT_ENC_STREAM_BUF_SZ {
            return Err(Error::OutputTooSmall { have: out_len, need: ffi::LDACBT_ENC_STREAM_BUF_SZ });
        }
        Ok(())
    }

    /// `pcm` must hold LDACBT_ENC_LSU frames in the initialized layout and
    /// `out` LDACBT_ENC_STREAM_BUF_SZ bytes (checked by [`Self::check`])
    fn encode_raw(&mut self, pcm: *const std::ffi::c_void, out: &mut [u8]) -> Result<EncodeOutput, Error> {
        let (mut used, mut written, mut frames) = (0, 0, 0);
        // SAFETY: the handle is valid and initialized for encoding. libldac
        // reads exactly LDACBT_ENC_LSU * channels * word length bytes from
        // pcm (checked against the initialized format and channel count by
        // the caller) and only reads it, so passing a pointer derived from a
        // shared slice as `void *` is sound. It writes at most
        // LDACBT_ENC_STREAM_BUF_SZ bytes (the size of the internal buffer it
        // copies from) to out, which has at least that many. The three out
        // parameters point to locals.
        let ret = unsafe {
            ffi::ldacBT_encode(self.as_ptr(), pcm.cast_mut(), &mut used, out.as_mut_ptr(), &mut written, &mut frames)
        };
        if ret < 0 {
            return Err(Error::Ldac(self.error_code()));
        }
        Ok(EncodeOutput {
            pcm_used: usize::try_from(used).unwrap_or(0),
            bytes: usize::try_from(written).unwrap_or(0).min(out.len()),
            frames: u32::try_from(frames).unwrap_or(0),
        })
    }

    /// ldacBT_get_eqmid: the target encode quality mode index (which ABR
    /// may have changed), -1 before init
    pub fn eqmid(&self) -> i32 {
        // SAFETY: the handle is valid; the call reads it (and sets the
        // error code when not initialized)
        unsafe { ffi::ldacBT_get_eqmid(self.as_ptr()) }
    }

    /// ldacBT_set_eqmid
    pub fn set_eqmid(&mut self, eqmid: i32) -> Result<(), Error> {
        // SAFETY: the handle is valid; libldac validates eqmid
        let ret = unsafe { ffi::ldacBT_set_eqmid(self.as_ptr(), eqmid) };
        if ret != 0 { Err(Error::Ldac(self.error_code())) } else { Ok(()) }
    }

    /// ldacBT_get_bitrate: the bit rate in kbps of the frames being encoded
    /// (a quality change takes effect at the next packet boundary), -1
    /// before init
    pub fn bitrate(&self) -> i32 {
        // SAFETY: the handle is valid; the call reads it
        unsafe { ffi::ldacBT_get_bitrate(self.as_ptr()) }
    }

    /// ldacBT_get_sampling_freq, -1 before init
    pub fn sampling_freq(&self) -> i32 {
        // SAFETY: the handle is valid; the call reads it
        unsafe { ffi::ldacBT_get_sampling_freq(self.as_ptr()) }
    }

    /// ldacBT_get_error_code: the code of the last failure
    pub fn error_code(&self) -> i32 {
        // SAFETY: the handle is valid; the call reads it
        unsafe { ffi::ldacBT_get_error_code(self.as_ptr()) }
    }
}

impl Drop for LdacBt {
    fn drop(&mut self) {
        // SAFETY: the handle is valid and owned by self; nothing uses it
        // after this (as v1 LdacEncoder::shutdown: close, then free)
        unsafe {
            ffi::ldacBT_close_handle(self.as_ptr());
            ffi::ldacBT_free_handle(self.as_ptr());
        }
    }
}

/// libldac version (ldacBT_get_version): major << 16 | minor << 8 | branch
pub fn version() -> i32 {
    // SAFETY: no arguments, returns a constant
    unsafe { ffi::ldacBT_get_version() }
}

/// An LDAC ABR handle (`HANDLE_LDAC_ABR`), freed on drop
pub struct LdacAbr {
    handle: NonNull<ffi::LdacAbrHandle>,
}

// SAFETY: as for LdacBt: owned heap memory without thread affinity
unsafe impl Send for LdacAbr {}

impl LdacAbr {
    /// ldac_ABR_get_handle + ldac_ABR_Init: [`Self::process`] is to be
    /// called every `interval_ms` (1..=500)
    pub fn new(interval_ms: u32) -> Result<LdacAbr, Error> {
        // SAFETY: no arguments; returns an owned handle or NULL
        let raw = unsafe { ffi::ldac_ABR_get_handle() };
        let abr = NonNull::new(raw).map(|handle| LdacAbr { handle }).ok_or(Error::Alloc)?;
        // SAFETY: the handle is valid; on failure `abr` is dropped, which
        // frees it (ldac_ABR_free_handle copes with the history buffer not
        // being allocated: ldac_ABR_get_handle sets it to NULL)
        let ret = unsafe { ffi::ldac_ABR_Init(abr.handle.as_ptr(), interval_ms) };
        if ret != 0 {
            return Err(Error::Ldac(ret));
        }
        Ok(abr)
    }

    /// ldac_ABR_set_thresholds (packets in the send queue); needs
    /// critical >= dangerous_trend >= safety_hq_sq
    pub fn set_thresholds(&mut self, critical: u32, dangerous_trend: u32, safety_hq_sq: u32) -> Result<(), Error> {
        // SAFETY: the handle is valid and initialized; libldac validates the
        // values
        let ret =
            unsafe { ffi::ldac_ABR_set_thresholds(self.handle.as_ptr(), critical, dangerous_trend, safety_hq_sq) };
        if ret != 0 { Err(Error::Ldac(ret)) } else { Ok(()) }
    }

    /// ldac_ABR_Proc: one ABR step with the send queue depth (packets).
    /// With `enable` false it only observes. Returns the encode quality
    /// mode index now targeted.
    pub fn process(&mut self, encoder: &mut LdacBt, tx_queue_depth: u32, enable: bool) -> Result<i32, Error> {
        // The history keeps depths as unsigned char: clamp, so its running
        // sum stays consistent with what it later subtracts
        let depth = tx_queue_depth.min(u32::from(u8::MAX));
        // SAFETY: both handles are valid and owned (the encoder borrowed
        // mutably, as ABR may change its quality mode). An encoder that is
        // not initialized makes ldac_ABR_Proc return -1 without other effect.
        let ret = unsafe { ffi::ldac_ABR_Proc(encoder.as_ptr(), self.handle.as_ptr(), depth, u32::from(enable)) };
        if ret < 0 { Err(Error::Ldac(ret)) } else { Ok(ret) }
    }
}

impl Drop for LdacAbr {
    fn drop(&mut self) {
        // SAFETY: the handle is valid and owned by self; nothing uses it
        // after this
        unsafe { ffi::ldac_ABR_free_handle(self.handle.as_ptr()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_2_0_2() {
        assert_eq!(version(), 0x02_00_02);
    }

    #[test]
    fn encode_before_init_is_rejected() {
        let mut bt = LdacBt::new().unwrap();
        let mut out = [0u8; ffi::LDACBT_ENC_STREAM_BUF_SZ];
        assert_eq!(bt.encode_i16(&[0; 256], &mut out), Err(Error::NotInitialized));
        assert_eq!(bt.eqmid(), -1);
    }

    #[test]
    fn init_checks_and_encode() {
        let mut bt = LdacBt::new().unwrap();
        // Below the minimum transport MTU
        assert!(matches!(
            bt.init_encode(600, ffi::LDACBT_EQMID_HQ, ffi::LDACBT_CHANNEL_MODE_STEREO, SampleFormat::S16, 48000),
            Err(Error::Ldac(_))
        ));
        bt.init_encode(679, ffi::LDACBT_EQMID_SQ, ffi::LDACBT_CHANNEL_MODE_STEREO, SampleFormat::S16, 48000).unwrap();
        assert_eq!(bt.eqmid(), ffi::LDACBT_EQMID_SQ);
        assert_eq!(bt.bitrate(), 660);
        assert_eq!(bt.sampling_freq(), 48000);

        let mut out = [0u8; ffi::LDACBT_ENC_STREAM_BUF_SZ];
        assert_eq!(bt.encode_i32(&[0; 256], &mut out), Err(Error::WrongFormat));
        assert_eq!(bt.encode_i16(&[0; 128], &mut out), Err(Error::InputLength { expected: 256, got: 128 }));
        assert_eq!(
            bt.encode_i16(&[0; 256], &mut out[..100]),
            Err(Error::OutputTooSmall { have: 100, need: ffi::LDACBT_ENC_STREAM_BUF_SZ })
        );
        let mut frames = 0;
        for _ in 0..32 {
            let r = bt.encode_i16(&[0; 256], &mut out).unwrap();
            assert_eq!(r.pcm_used, 512);
            if r.bytes > 0 {
                assert_eq!(out[0], ffi::LDAC_SYNCWORD);
            }
            frames += r.frames;
        }
        assert!(frames > 0);
    }

    #[test]
    fn abr_lowers_quality_under_a_full_queue() {
        let mut bt = LdacBt::new().unwrap();
        bt.init_encode(679, ffi::LDACBT_EQMID_HQ, ffi::LDACBT_CHANNEL_MODE_STEREO, SampleFormat::S16, 48000).unwrap();
        let mut abr = LdacAbr::new(100).unwrap();
        abr.set_thresholds(12, 8, 4).unwrap();
        assert!(abr.set_thresholds(4, 8, 12).is_err());
        // Observing only: no change
        assert_eq!(abr.process(&mut bt, 20, false), Ok(ffi::LDACBT_EQMID_HQ));
        // Critical: two steps down (HQ -> SQ -> Q0)
        assert_eq!(abr.process(&mut bt, 20, true), Ok(ffi::LDACBT_EQMID_Q0));
        assert!(LdacAbr::new(0).is_err());
    }
}
