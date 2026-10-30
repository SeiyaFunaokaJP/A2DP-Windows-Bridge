//! liblc3 (Google, Apache-2.0), built from
//! extern/btstack/3rd-party/lc3-google (build.rs; LC3_DIR overrides it).
//!
//! [`ffi`] has hand-written declarations matching include/lc3.h (only what
//! BlueCaster uses). [`Lc3Encoder`] owns the encoder memory and passes slice
//! lengths along, so that callers need no `unsafe`. One encoder per LC3
//! channel (v1 Stream: one btstack_lc3_encoder_google_t per output
//! channel). [`Lc3Decoder`] is the matching decoder, for tests and tools.

use std::fmt;
use std::ptr::NonNull;

/// Declarations from include/lc3.h
#[allow(non_camel_case_types)]
pub mod ffi {
    use std::ffi::{c_int, c_uint, c_void};
    use std::marker::{PhantomData, PhantomPinned};

    /// `struct lc3_encoder` (opaque here; lives in caller-provided memory)
    #[repr(C)]
    pub struct lc3_encoder {
        _data: (),
        _marker: PhantomData<(*mut u8, PhantomPinned)>,
    }

    /// `struct lc3_decoder` (opaque here; lives in caller-provided memory)
    #[repr(C)]
    pub struct lc3_decoder {
        _data: (),
        _marker: PhantomData<(*mut u8, PhantomPinned)>,
    }

    pub type lc3_encoder_t = *mut lc3_encoder;
    pub type lc3_decoder_t = *mut lc3_decoder;

    /// `enum lc3_pcm_format`
    pub type lc3_pcm_format = c_int;
    pub const LC3_PCM_FORMAT_S16: lc3_pcm_format = 0;
    pub const LC3_PCM_FORMAT_S24: lc3_pcm_format = 1;
    pub const LC3_PCM_FORMAT_S24_3LE: lc3_pcm_format = 2;
    pub const LC3_PCM_FORMAT_FLOAT: lc3_pcm_format = 3;

    pub const LC3_MIN_FRAME_BYTES: c_int = 20;
    pub const LC3_MAX_FRAME_BYTES: c_int = 400;

    unsafe extern "C" {
        /// PCM samples per frame, -1 on bad parameters
        pub fn lc3_frame_samples(dt_us: c_int, sr_hz: c_int) -> c_int;
        /// Algorithmic delay in samples, -1 on bad parameters
        pub fn lc3_delay_samples(dt_us: c_int, sr_hz: c_int) -> c_int;

        /// Bytes of encoder memory, 0 on bad parameters
        pub fn lc3_encoder_size(dt_us: c_int, sr_hz: c_int) -> c_uint;
        /// `mem` aligned to a pointer, `lc3_encoder_size` bytes; NULL on bad
        /// parameters
        pub fn lc3_setup_encoder(dt_us: c_int, sr_hz: c_int, sr_pcm_hz: c_int, mem: *mut c_void) -> lc3_encoder_t;
        /// 0 on success, -1 on bad parameters
        pub fn lc3_encode(
            encoder: lc3_encoder_t,
            fmt: lc3_pcm_format,
            pcm: *const c_void,
            stride: c_int,
            nbytes: c_int,
            out: *mut c_void,
        ) -> c_int;

        /// Bytes of decoder memory, 0 on bad parameters
        pub fn lc3_decoder_size(dt_us: c_int, sr_hz: c_int) -> c_uint;
        pub fn lc3_setup_decoder(dt_us: c_int, sr_hz: c_int, sr_pcm_hz: c_int, mem: *mut c_void) -> lc3_decoder_t;
        /// 0 on success, 1 when PLC ran (`input` NULL), -1 on bad parameters
        pub fn lc3_decode(
            decoder: lc3_decoder_t,
            input: *const c_void,
            nbytes: c_int,
            fmt: lc3_pcm_format,
            pcm: *mut c_void,
            stride: c_int,
        ) -> c_int;
    }
}

/// Why an LC3 call failed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Frame duration / sample rate liblc3 does not support
    BadParameters,
    /// The PCM slice is shorter than one frame (with the stride), or the
    /// output length is outside 20..=400 octets
    BadLength,
    /// lc3_encode returned an error
    Encode(i32),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadParameters => f.write_str("unsupported LC3 frame duration or sample rate"),
            Error::BadLength => f.write_str("bad LC3 frame length"),
            Error::Encode(code) => write!(f, "LC3 encoding failed ({code})"),
        }
    }
}

impl std::error::Error for Error {}

/// Smallest and largest LC3 frame (octets)
pub const MIN_FRAME_BYTES: usize = ffi::LC3_MIN_FRAME_BYTES as usize;
pub const MAX_FRAME_BYTES: usize = ffi::LC3_MAX_FRAME_BYTES as usize;

/// PCM samples of one frame, None for a frame duration / rate liblc3 does
/// not support
pub fn frame_samples(frame_us: u32, sample_rate: u32) -> Option<usize> {
    let (dt, sr) = (i32::try_from(frame_us).ok()?, i32::try_from(sample_rate).ok()?);
    // SAFETY: plain values, checked by liblc3
    let n = unsafe { ffi::lc3_frame_samples(dt, sr) };
    usize::try_from(n).ok().filter(|&n| n > 0)
}

/// Algorithmic delay of encoder + decoder in samples, None for unsupported
/// parameters
pub fn delay_samples(frame_us: u32, sample_rate: u32) -> Option<usize> {
    let (dt, sr) = (i32::try_from(frame_us).ok()?, i32::try_from(sample_rate).ok()?);
    // SAFETY: plain values, checked by liblc3
    let n = unsafe { ffi::lc3_delay_samples(dt, sr) };
    usize::try_from(n).ok()
}

/// Encoder / decoder memory: liblc3 wants it aligned to a pointer; 16 bytes
/// covers that and any SIMD access
#[derive(Clone, Copy)]
#[repr(C, align(16))]
struct Block([u8; 16]);

fn alloc_blocks(bytes: usize) -> Box<[Block]> {
    vec![Block([0; 16]); bytes.div_ceil(size_of::<Block>())].into_boxed_slice()
}

/// Is `len` samples at `stride` enough for `frame` samples?
fn pcm_fits(len: usize, frame: usize, stride: usize) -> bool {
    stride >= 1 && i32::try_from(stride).is_ok() && (frame - 1).checked_mul(stride).is_some_and(|last| last < len)
}

fn check_params(frame_us: u32, sample_rate: u32) -> Result<(i32, i32, usize), Error> {
    let samples = frame_samples(frame_us, sample_rate).ok_or(Error::BadParameters)?;
    Ok((frame_us as i32, sample_rate as i32, samples))
}

/// An LC3 encoder of one channel
pub struct Lc3Encoder {
    frame_us: u32,
    sample_rate: u32,
    samples: usize,
    /// Points into `_mem` (a heap block that does not move with `self`)
    handle: NonNull<ffi::lc3_encoder>,
    _mem: Box<[Block]>,
}

// SAFETY: the state is plain memory owned by this value; liblc3 keeps no
// global mutable state
unsafe impl Send for Lc3Encoder {}

impl Lc3Encoder {
    /// `frame_us`: 7500 or 10000; `sample_rate`: 8000, 16000, 24000, 32000
    /// or 48000 Hz (the LC3 rate; the PCM must already be at it)
    pub fn new(frame_us: u32, sample_rate: u32) -> Result<Lc3Encoder, Error> {
        let (dt, sr, samples) = check_params(frame_us, sample_rate)?;
        // SAFETY: plain values
        let size = unsafe { ffi::lc3_encoder_size(dt, sr) } as usize;
        if size == 0 {
            return Err(Error::BadParameters);
        }
        let mut mem = alloc_blocks(size);
        // SAFETY: `mem` holds at least `size` bytes, aligned to 16; it is
        // owned by the encoder and outlives the handle
        let handle = unsafe { ffi::lc3_setup_encoder(dt, sr, 0, mem.as_mut_ptr().cast()) };
        let handle = NonNull::new(handle).ok_or(Error::BadParameters)?;
        Ok(Lc3Encoder { frame_us, sample_rate, samples, handle, _mem: mem })
    }

    pub fn frame_us(&self) -> u32 {
        self.frame_us
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// PCM samples of one frame (e.g. 480 at 48 kHz, 10 ms)
    pub fn frame_samples(&self) -> usize {
        self.samples
    }

    /// Encodes one frame of 16-bit PCM into `out` (its length is the octets
    /// per frame of the preset). `pcm` holds the channel's samples every
    /// `stride` samples (1 for mono data, 2 for one channel of interleaved
    /// stereo, starting at that channel's first sample).
    pub fn encode(&mut self, pcm: &[i16], stride: usize, out: &mut [u8]) -> Result<(), Error> {
        if !pcm_fits(pcm.len(), self.samples, stride) || !(MIN_FRAME_BYTES..=MAX_FRAME_BYTES).contains(&out.len()) {
            return Err(Error::BadLength);
        }
        // SAFETY: the handle is valid (set up in `new`, memory owned by
        // self); `pcm` holds the frame's samples at `stride` (checked
        // above) and `out` is writable for its length (20..=400)
        let r = unsafe {
            ffi::lc3_encode(
                self.handle.as_ptr(),
                ffi::LC3_PCM_FORMAT_S16,
                pcm.as_ptr().cast(),
                stride as i32,
                out.len() as i32,
                out.as_mut_ptr().cast(),
            )
        };
        if r != 0 { Err(Error::Encode(r)) } else { Ok(()) }
    }
}

impl fmt::Debug for Lc3Encoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Lc3Encoder").field("frame_us", &self.frame_us).field("sample_rate", &self.sample_rate).finish()
    }
}

/// An LC3 decoder of one channel (tests and tools)
pub struct Lc3Decoder {
    frame_us: u32,
    sample_rate: u32,
    samples: usize,
    handle: NonNull<ffi::lc3_decoder>,
    _mem: Box<[Block]>,
}

// SAFETY: as Lc3Encoder
unsafe impl Send for Lc3Decoder {}

impl Lc3Decoder {
    pub fn new(frame_us: u32, sample_rate: u32) -> Result<Lc3Decoder, Error> {
        let (dt, sr, samples) = check_params(frame_us, sample_rate)?;
        // SAFETY: plain values
        let size = unsafe { ffi::lc3_decoder_size(dt, sr) } as usize;
        if size == 0 {
            return Err(Error::BadParameters);
        }
        let mut mem = alloc_blocks(size);
        // SAFETY: as in Lc3Encoder::new
        let handle = unsafe { ffi::lc3_setup_decoder(dt, sr, 0, mem.as_mut_ptr().cast()) };
        let handle = NonNull::new(handle).ok_or(Error::BadParameters)?;
        Ok(Lc3Decoder { frame_us, sample_rate, samples, handle, _mem: mem })
    }

    pub fn frame_us(&self) -> u32 {
        self.frame_us
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn frame_samples(&self) -> usize {
        self.samples
    }

    /// Decodes one frame into `pcm` (every `stride` samples); `None` runs
    /// packet loss concealment. Ok(true) when PLC ran.
    pub fn decode(&mut self, frame: Option<&[u8]>, pcm: &mut [i16], stride: usize) -> Result<bool, Error> {
        if !pcm_fits(pcm.len(), self.samples, stride) {
            return Err(Error::BadLength);
        }
        let (input, nbytes) = match frame {
            Some(f) if (MIN_FRAME_BYTES..=MAX_FRAME_BYTES).contains(&f.len()) => (f.as_ptr(), f.len() as i32),
            Some(_) => return Err(Error::BadLength),
            None => (std::ptr::null(), 0),
        };
        // SAFETY: valid handle; `input` is NULL or `nbytes` readable bytes;
        // `pcm` is writable for the frame at `stride` (checked above)
        let r = unsafe {
            ffi::lc3_decode(
                self.handle.as_ptr(),
                input.cast(),
                nbytes,
                ffi::LC3_PCM_FORMAT_S16,
                pcm.as_mut_ptr().cast(),
                stride as i32,
            )
        };
        match r {
            0 => Ok(false),
            1 => Ok(true),
            r => Err(Error::Encode(r)),
        }
    }
}

impl fmt::Debug for Lc3Decoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Lc3Decoder").field("frame_us", &self.frame_us).field("sample_rate", &self.sample_rate).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_sizes() {
        assert_eq!(frame_samples(10000, 48000), Some(480));
        assert_eq!(frame_samples(7500, 48000), Some(360));
        assert_eq!(frame_samples(10000, 16000), Some(160));
        assert_eq!(frame_samples(7500, 24000), Some(180));
        assert_eq!(frame_samples(10000, 44100), None);
        assert_eq!(frame_samples(8000, 48000), None);
        let e = Lc3Encoder::new(7500, 32000).unwrap();
        assert_eq!((e.frame_us(), e.sample_rate(), e.frame_samples()), (7500, 32000, 240));
    }

    #[test]
    fn bad_parameters_and_lengths() {
        assert_eq!(Lc3Encoder::new(10000, 44100).unwrap_err(), Error::BadParameters);
        assert_eq!(Lc3Encoder::new(9000, 48000).unwrap_err(), Error::BadParameters);
        let mut e = Lc3Encoder::new(10000, 16000).unwrap();
        let pcm = vec![0i16; 320];
        let mut out = [0u8; 40];
        assert_eq!(e.encode(&pcm[..159], 1, &mut out), Err(Error::BadLength));
        assert_eq!(e.encode(&pcm[..318], 2, &mut out), Err(Error::BadLength));
        assert_eq!(e.encode(&pcm, 0, &mut out), Err(Error::BadLength));
        assert_eq!(e.encode(&pcm, 1, &mut out[..19]), Err(Error::BadLength));
        let mut big = [0u8; 401];
        assert_eq!(e.encode(&pcm, 1, &mut big), Err(Error::BadLength));
        assert_eq!(e.encode(&pcm[..160], 1, &mut out), Ok(()));
        assert_eq!(e.encode(&pcm[1..], 2, &mut out), Ok(()));
    }

    /// Interleaved stereo: 1 kHz left, 1.5 kHz right, amplitude 0.5 (as
    /// audio-capture's test tone)
    fn tone(rate: u32, frames: usize) -> Vec<i16> {
        let mut v = Vec::with_capacity(frames * 2);
        for n in 0..frames {
            let t = n as f64 / rate as f64;
            for hz in [1000.0, 1500.0] {
                v.push((0.5 * (std::f64::consts::TAU * hz * t).sin() * 32767.0).round() as i16);
            }
        }
        v
    }

    /// SNR in dB of `decoded` against `reference` delayed by `delay`
    /// samples (one channel of interleaved stereo each), over `from..`
    fn snr(reference: &[i16], decoded: &[i16], ch: usize, delay: usize, from: usize) -> f64 {
        let (mut sig, mut noise) = (0.0f64, 0.0f64);
        for n in from..decoded.len() / 2 {
            let r = reference[(n - delay) * 2 + ch] as f64;
            let d = decoded[n * 2 + ch] as f64;
            sig += r * r;
            noise += (d - r) * (d - r);
        }
        10.0 * (sig / noise.max(1e-9)).log10()
    }

    /// Encode -> decode of the test tone at every BAP rate with 7.5 and 10
    /// ms frames, at the octets of the BAP presets: the decoded tone matches
    /// the input (time-aligned by the codec delay). At the low bit rates
    /// LC3's noise filling dominates the waveform error (about 20 dB); the
    /// high-rate 48 kHz presets (BlueCaster's first choices) are near lossless
    /// for a tone.
    #[test]
    fn round_trip_snr() {
        // (preset, rate, frame, octets, minimum SNR in dB)
        let cases = [
            ("16_1", 16000, 7500, 30, 15.0),
            ("16_2", 16000, 10000, 40, 15.0),
            ("24_1", 24000, 7500, 45, 15.0),
            ("24_2", 24000, 10000, 60, 15.0),
            ("32_1", 32000, 7500, 60, 18.0),
            ("32_2", 32000, 10000, 80, 18.0),
            ("48_1", 48000, 7500, 75, 18.0),
            ("48_2", 48000, 10000, 100, 18.0),
            ("48_3", 48000, 7500, 90, 60.0),
            ("48_4", 48000, 10000, 120, 60.0),
            ("48_5", 48000, 7500, 117, 60.0),
            ("48_6", 48000, 10000, 155, 60.0),
        ];
        for (name, rate, frame_us, octets, min_snr) in cases {
            let mut enc = [Lc3Encoder::new(frame_us, rate).unwrap(), Lc3Encoder::new(frame_us, rate).unwrap()];
            let mut dec = [Lc3Decoder::new(frame_us, rate).unwrap(), Lc3Decoder::new(frame_us, rate).unwrap()];
            let ns = enc[0].frame_samples();
            let frames = 100;
            let input = tone(rate, ns * frames);
            let mut output = vec![0i16; input.len()];
            let mut buf = vec![0u8; octets];
            for f in 0..frames {
                let at = f * ns * 2;
                for ch in 0..2 {
                    enc[ch].encode(&input[at + ch..], 2, &mut buf).unwrap();
                    assert!(!dec[ch].decode(Some(&buf), &mut output[at + ch..], 2).unwrap());
                }
            }
            let delay = delay_samples(frame_us, rate).unwrap();
            // Skip the decoder's start-up
            let from = delay + 10 * ns;
            for ch in 0..2 {
                let s = snr(&input, &output, ch, delay, from);
                eprintln!("{name}: {rate} Hz, {frame_us} us, {octets} octets, channel {ch}: SNR {s:.1} dB");
                assert!(s > min_snr, "{name} channel {ch}: SNR {s:.1} dB (at least {min_snr})");
            }
        }
    }

    #[test]
    fn plc_runs_without_a_frame() {
        let mut dec = Lc3Decoder::new(10000, 48000).unwrap();
        let mut pcm = vec![0i16; 480];
        assert_eq!(dec.decode(None, &mut pcm, 1), Ok(true));
        assert_eq!(dec.decode(Some(&[0u8; 10]), &mut pcm, 1), Err(Error::BadLength));
    }

    #[test]
    fn encoder_is_send() {
        fn send<T: Send>() {}
        send::<Lc3Encoder>();
        send::<Lc3Decoder>();
    }
}
