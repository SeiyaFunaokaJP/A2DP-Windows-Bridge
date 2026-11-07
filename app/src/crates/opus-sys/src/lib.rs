//! libopus (Xiph.Org, BSD-3-Clause), built from the git submodule
//! extern/opus.
//!
//! [`ffi`] has hand-written declarations matching include/opus.h and
//! include/opus_defines.h (only what BlueCaster uses). [`OpusEnc`] and
//! [`OpusDec`] own an encoder / decoder state and pass slice lengths along,
//! so that callers need no `unsafe`.

use std::fmt;
use std::ptr::NonNull;

/// Declarations from include/opus.h and include/opus_defines.h
#[allow(non_camel_case_types)]
pub mod ffi {
    use std::ffi::{c_char, c_int};
    use std::marker::{PhantomData, PhantomPinned};

    /// `OpusEncoder` (opaque)
    #[repr(C)]
    pub struct OpusEncoder {
        _data: (),
        _marker: PhantomData<(*mut u8, PhantomPinned)>,
    }

    /// `OpusDecoder` (opaque)
    #[repr(C)]
    pub struct OpusDecoder {
        _data: (),
        _marker: PhantomData<(*mut u8, PhantomPinned)>,
    }

    pub type opus_int16 = i16;
    pub type opus_int32 = i32;

    pub const OPUS_OK: c_int = 0;
    pub const OPUS_BAD_ARG: c_int = -1;
    pub const OPUS_BUFFER_TOO_SMALL: c_int = -2;
    pub const OPUS_INTERNAL_ERROR: c_int = -3;
    pub const OPUS_INVALID_PACKET: c_int = -4;
    pub const OPUS_UNIMPLEMENTED: c_int = -5;
    pub const OPUS_INVALID_STATE: c_int = -6;
    pub const OPUS_ALLOC_FAIL: c_int = -7;

    pub const OPUS_APPLICATION_VOIP: c_int = 2048;
    pub const OPUS_APPLICATION_AUDIO: c_int = 2049;
    pub const OPUS_APPLICATION_RESTRICTED_LOWDELAY: c_int = 2051;

    pub const OPUS_SET_BITRATE_REQUEST: c_int = 4002;
    pub const OPUS_GET_BITRATE_REQUEST: c_int = 4003;
    pub const OPUS_SET_VBR_REQUEST: c_int = 4006;
    pub const OPUS_GET_VBR_REQUEST: c_int = 4007;
    pub const OPUS_SET_COMPLEXITY_REQUEST: c_int = 4010;
    pub const OPUS_GET_COMPLEXITY_REQUEST: c_int = 4011;
    pub const OPUS_SET_VBR_CONSTRAINT_REQUEST: c_int = 4020;
    pub const OPUS_GET_VBR_CONSTRAINT_REQUEST: c_int = 4021;
    pub const OPUS_SET_SIGNAL_REQUEST: c_int = 4024;
    pub const OPUS_GET_LOOKAHEAD_REQUEST: c_int = 4027;
    pub const OPUS_RESET_STATE: c_int = 4028;

    pub const OPUS_SIGNAL_MUSIC: c_int = 3002;

    unsafe extern "C" {
        pub fn opus_get_version_string() -> *const c_char;
        pub fn opus_strerror(error: c_int) -> *const c_char;

        pub fn opus_encoder_create(
            fs: opus_int32,
            channels: c_int,
            application: c_int,
            error: *mut c_int,
        ) -> *mut OpusEncoder;
        pub fn opus_encoder_destroy(st: *mut OpusEncoder);
        pub fn opus_encode(
            st: *mut OpusEncoder,
            pcm: *const opus_int16,
            frame_size: c_int,
            data: *mut u8,
            max_data_bytes: opus_int32,
        ) -> opus_int32;
        /// Variadic: one `opus_int32` (SET) or `*mut opus_int32` (GET)
        /// after the request, or nothing (OPUS_RESET_STATE)
        pub fn opus_encoder_ctl(st: *mut OpusEncoder, request: c_int, ...) -> c_int;

        pub fn opus_decoder_create(fs: opus_int32, channels: c_int, error: *mut c_int) -> *mut OpusDecoder;
        pub fn opus_decoder_destroy(st: *mut OpusDecoder);
        pub fn opus_decode(
            st: *mut OpusDecoder,
            data: *const u8,
            len: opus_int32,
            pcm: *mut opus_int16,
            frame_size: c_int,
            decode_fec: c_int,
        ) -> c_int;

        pub fn opus_packet_get_nb_frames(packet: *const u8, len: opus_int32) -> c_int;
        pub fn opus_packet_get_samples_per_frame(data: *const u8, fs: opus_int32) -> c_int;
        pub fn opus_packet_get_nb_channels(data: *const u8) -> c_int;
    }
}

/// An `OPUS_*` error code
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error(pub i32);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // SAFETY: opus_strerror returns a static NUL-terminated string for
        // any code
        let s = unsafe { std::ffi::CStr::from_ptr(ffi::opus_strerror(self.0)) };
        write!(f, "{} ({})", s.to_string_lossy(), self.0)
    }
}

impl std::error::Error for Error {}

fn check(code: i32) -> Result<i32, Error> {
    if code < 0 { Err(Error(code)) } else { Ok(code) }
}

/// The libopus version, e.g. "libopus 1.5.2"
pub fn version() -> String {
    // SAFETY: returns a static NUL-terminated string
    unsafe { std::ffi::CStr::from_ptr(ffi::opus_get_version_string()) }.to_string_lossy().into_owned()
}

/// An Opus encoder state (opus_encoder_create)
pub struct OpusEnc {
    st: NonNull<ffi::OpusEncoder>,
    channels: usize,
}

// SAFETY: the state is plain memory owned by this value; libopus keeps no
// thread-local or global mutable state for it
unsafe impl Send for OpusEnc {}

impl OpusEnc {
    /// `sample_rate` 8000 / 12000 / 16000 / 24000 / 48000, `channels` 1 or 2,
    /// `application` one of `OPUS_APPLICATION_*`
    pub fn new(sample_rate: u32, channels: u32, application: i32) -> Result<OpusEnc, Error> {
        let mut err = 0;
        // SAFETY: plain values and a local for the error code; the
        // arguments are checked by libopus
        let st =
            unsafe { ffi::opus_encoder_create(sample_rate as i32, channels as std::ffi::c_int, application, &mut err) };
        check(err)?;
        let st = NonNull::new(st).ok_or(Error(ffi::OPUS_ALLOC_FAIL))?;
        Ok(OpusEnc { st, channels: channels as usize })
    }

    fn ptr(&self) -> *mut ffi::OpusEncoder {
        self.st.as_ptr()
    }

    /// A SET request with one `opus_int32` argument
    pub fn set(&mut self, request: i32, value: i32) -> Result<(), Error> {
        // SAFETY: the state is valid; every SET request libopus knows takes
        // exactly one opus_int32 (unknown requests return OPUS_UNIMPLEMENTED
        // without reading the argument)
        check(unsafe { ffi::opus_encoder_ctl(self.ptr(), request, value) }).map(|_| ())
    }

    /// A GET request with one `opus_int32 *` argument
    pub fn get(&self, request: i32) -> Result<i32, Error> {
        let mut value: i32 = 0;
        // SAFETY: the state is valid; GET requests write one opus_int32
        // through the pointer, a local
        check(unsafe { ffi::opus_encoder_ctl(self.ptr(), request, &mut value as *mut i32) })?;
        Ok(value)
    }

    pub fn set_bitrate(&mut self, bps: i32) -> Result<(), Error> {
        self.set(ffi::OPUS_SET_BITRATE_REQUEST, bps)
    }

    pub fn bitrate(&self) -> Result<i32, Error> {
        self.get(ffi::OPUS_GET_BITRATE_REQUEST)
    }

    /// Encoder delay in samples (OPUS_GET_LOOKAHEAD)
    pub fn lookahead(&self) -> Result<i32, Error> {
        self.get(ffi::OPUS_GET_LOOKAHEAD_REQUEST)
    }

    /// Encodes one frame of interleaved PCM (`frame_size` samples per
    /// channel: 2.5 to 60 ms) into `out`, at most `out.len()` bytes (capped
    /// at 1275 by libopus for one frame). Returns the packet length.
    pub fn encode(&mut self, pcm: &[i16], out: &mut [u8]) -> Result<usize, Error> {
        if self.channels == 0 || !pcm.len().is_multiple_of(self.channels) {
            return Err(Error(ffi::OPUS_BAD_ARG));
        }
        let frame_size = pcm.len() / self.channels;
        let max = out.len().min(i32::MAX as usize) as i32;
        // SAFETY: the state is valid; libopus reads frame_size * channels
        // samples from pcm (exactly its length) and writes at most `max`
        // bytes to out (its length)
        let n =
            unsafe { ffi::opus_encode(self.ptr(), pcm.as_ptr(), frame_size as std::ffi::c_int, out.as_mut_ptr(), max) };
        check(n).map(|n| n as usize)
    }
}

impl Drop for OpusEnc {
    fn drop(&mut self) {
        // SAFETY: the state came from opus_encoder_create and is owned by
        // self; nothing uses it after this
        unsafe { ffi::opus_encoder_destroy(self.ptr()) }
    }
}

/// An Opus decoder state (opus_decoder_create), for round-trip tests
pub struct OpusDec {
    st: NonNull<ffi::OpusDecoder>,
    channels: usize,
}

// SAFETY: as OpusEnc
unsafe impl Send for OpusDec {}

impl OpusDec {
    pub fn new(sample_rate: u32, channels: u32) -> Result<OpusDec, Error> {
        let mut err = 0;
        // SAFETY: plain values and a local for the error code
        let st = unsafe { ffi::opus_decoder_create(sample_rate as i32, channels as std::ffi::c_int, &mut err) };
        check(err)?;
        let st = NonNull::new(st).ok_or(Error(ffi::OPUS_ALLOC_FAIL))?;
        Ok(OpusDec { st, channels: channels as usize })
    }

    /// Decodes one packet into `pcm` (interleaved); returns the samples per
    /// channel decoded
    pub fn decode(&mut self, packet: &[u8], pcm: &mut [i16]) -> Result<usize, Error> {
        let frame_size = pcm.len() / self.channels.max(1);
        // SAFETY: the state is valid; libopus reads packet.len() bytes and
        // writes at most frame_size * channels samples (within pcm)
        let n = unsafe {
            ffi::opus_decode(
                self.st.as_ptr(),
                packet.as_ptr(),
                packet.len().min(i32::MAX as usize) as i32,
                pcm.as_mut_ptr(),
                frame_size as std::ffi::c_int,
                0,
            )
        };
        check(n).map(|n| n as usize)
    }
}

impl Drop for OpusDec {
    fn drop(&mut self) {
        // SAFETY: the state came from opus_decoder_create and is owned by
        // self
        unsafe { ffi::opus_decoder_destroy(self.st.as_ptr()) }
    }
}

/// Frames in an Opus packet (its TOC byte), or an error for a malformed one
pub fn packet_frames(packet: &[u8]) -> Result<usize, Error> {
    if packet.is_empty() {
        return Err(Error(ffi::OPUS_BAD_ARG));
    }
    // SAFETY: libopus reads at most packet.len() bytes
    check(unsafe { ffi::opus_packet_get_nb_frames(packet.as_ptr(), packet.len() as i32) }).map(|n| n as usize)
}

/// Samples per channel of each frame of an Opus packet at `sample_rate`
pub fn packet_samples_per_frame(packet: &[u8], sample_rate: u32) -> Option<usize> {
    // SAFETY: reads the TOC byte only (packet is not empty)
    (!packet.is_empty())
        .then(|| unsafe { ffi::opus_packet_get_samples_per_frame(packet.as_ptr(), sample_rate as i32) } as usize)
}

/// Channels coded in an Opus packet (its TOC byte)
pub fn packet_channels(packet: &[u8]) -> Option<usize> {
    // SAFETY: reads the TOC byte only (packet is not empty)
    (!packet.is_empty()).then(|| unsafe { ffi::opus_packet_get_nb_channels(packet.as_ptr()) } as usize)
}

#[cfg(test)]
mod tests {
    use super::ffi::*;
    use super::*;

    #[test]
    fn version_is_the_submodule() {
        assert!(version().starts_with("libopus 1.5"), "{}", version());
    }

    #[test]
    fn encode_decode_round_trip() {
        let mut enc = OpusEnc::new(48000, 2, OPUS_APPLICATION_AUDIO).unwrap();
        enc.set_bitrate(256_000).unwrap();
        assert_eq!(enc.bitrate(), Ok(256_000));
        assert!(enc.set(OPUS_SET_COMPLEXITY_REQUEST, 11).is_err());
        assert!(enc.lookahead().unwrap() > 0);
        let mut dec = OpusDec::new(48000, 2).unwrap();

        // 20 ms of a 1 kHz tone, both channels
        let pcm: Vec<i16> = (0..960)
            .flat_map(|i| {
                let v = (f64::sin(i as f64 * 2.0 * std::f64::consts::PI * 1000.0 / 48000.0) * 16000.0) as i16;
                [v, v]
            })
            .collect();
        let mut packet = [0u8; 1275];
        let mut out = vec![0i16; 960 * 2];
        let mut last_peak = 0;
        for _ in 0..10 {
            let n = enc.encode(&pcm, &mut packet).unwrap();
            assert!(n > 2 && n <= 1275);
            assert_eq!(packet_frames(&packet[..n]), Ok(1));
            assert_eq!(packet_samples_per_frame(&packet[..n], 48000), Some(960));
            assert_eq!(packet_channels(&packet[..n]), Some(2));
            assert_eq!(dec.decode(&packet[..n], &mut out), Ok(960));
            last_peak = out.iter().map(|s| s.unsigned_abs()).max().unwrap();
        }
        assert!((14000..18000).contains(&last_peak), "{last_peak}");

        // Partial frames and short output buffers are errors, not overruns
        assert!(enc.encode(&pcm[..959 * 2 + 1], &mut packet).is_err());
        assert_eq!(enc.encode(&pcm, &mut packet[..0]), Err(Error(OPUS_BAD_ARG)));
        // Below 3 bytes libopus gives up on the frame: a TOC-only packet the
        // decoder conceals
        assert_eq!(enc.encode(&pcm, &mut packet[..2]), Ok(1));
        assert!(packet_frames(&[]).is_err());
    }
}
