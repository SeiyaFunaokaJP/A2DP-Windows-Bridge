//! LC3 for LE Audio (experimental), encoded by liblc3 (crate `lc3-sys`,
//! Apache-2.0).
//!
//! Not an A2DP codec: no [`crate::CodecKind`], no [`crate::Encoder`]. LE
//! Audio sends one LC3 frame per channel and frame interval (10 or 7.5 ms)
//! in ISO SDUs; the LE encode thread (pipeline's `le` module) drives one
//! [`Lc3ChannelEncoder`] per output channel (v1 Stream: one
//! btstack_lc3_encoder_google_t per channel) and lays the frames out per
//! CIS. The parameters come from the BAP preset of the stream plan.

pub use lc3_sys::Error as Lc3Error;
pub use lc3_sys::{MAX_FRAME_BYTES, MIN_FRAME_BYTES};

use lc3_sys::Lc3Encoder;

/// The LC3 configuration of a stream (from its BAP preset)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lc3Params {
    /// 8000, 16000, 24000, 32000 or 48000 Hz
    pub sample_rate: u32,
    /// 7500 or 10000
    pub frame_us: u32,
    /// Octets per frame and channel (20..=400)
    pub octets: u16,
}

impl Lc3Params {
    /// PCM samples per frame and channel, or the reason the parameters do not
    /// work
    pub fn frame_samples(&self) -> Result<usize, Lc3Error> {
        if !matches!(self.frame_us, 7500 | 10000) {
            return Err(Lc3Error::BadParameters);
        }
        if !(MIN_FRAME_BYTES..=MAX_FRAME_BYTES).contains(&usize::from(self.octets)) {
            return Err(Lc3Error::BadLength);
        }
        lc3_sys::frame_samples(self.frame_us, self.sample_rate).ok_or(Lc3Error::BadParameters)
    }

    /// Bit rate per channel in kbps (e.g. 80 for 100 octets every 10 ms)
    pub fn kbps(&self) -> u32 {
        (u32::from(self.octets) * 8000).checked_div(self.frame_us).unwrap_or(0)
    }
}

/// The encoder of one LC3 channel
#[derive(Debug)]
pub struct Lc3ChannelEncoder {
    enc: Lc3Encoder,
    octets: usize,
}

impl Lc3ChannelEncoder {
    pub fn new(params: Lc3Params) -> Result<Lc3ChannelEncoder, Lc3Error> {
        params.frame_samples()?;
        tracing::debug!(
            "LC3: encoder {} Hz, {} us, {} octets ({} kbps)",
            params.sample_rate,
            params.frame_us,
            params.octets,
            params.kbps()
        );
        let enc = Lc3Encoder::new(params.frame_us, params.sample_rate)?;
        Ok(Lc3ChannelEncoder { enc, octets: usize::from(params.octets) })
    }

    /// PCM samples of one frame
    pub fn frame_samples(&self) -> usize {
        self.enc.frame_samples()
    }

    /// Octets of one encoded frame
    pub fn octets(&self) -> usize {
        self.octets
    }

    /// Encodes one frame of 16-bit mono PCM (`frame_samples()` samples)
    /// into `out` (`octets()` long). No allocation.
    pub fn encode(&mut self, pcm: &[i16], out: &mut [u8]) -> Result<(), Lc3Error> {
        if pcm.len() != self.frame_samples() || out.len() != self.octets {
            return Err(Lc3Error::BadLength);
        }
        self.enc.encode(pcm, 1, out)
    }
}

/// One frame of digital silence, encoded by a fresh encoder (v1 encoded the
/// silence SDUs once per stream with separate encoder instances); `out` is
/// `params.octets` long
pub fn silence_frame(params: Lc3Params, out: &mut [u8]) -> Result<(), Lc3Error> {
    let mut enc = Lc3ChannelEncoder::new(params)?;
    let zeros = vec![0i16; enc.frame_samples()];
    enc.encode(&zeros, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const P48: Lc3Params = Lc3Params { sample_rate: 48000, frame_us: 10000, octets: 100 };

    #[test]
    fn params() {
        assert_eq!(P48.frame_samples(), Ok(480));
        assert_eq!(P48.kbps(), 80);
        let p = Lc3Params { sample_rate: 48000, frame_us: 7500, octets: 75 };
        assert_eq!((p.frame_samples(), p.kbps()), (Ok(360), 80));
        assert_eq!(Lc3Params { sample_rate: 44100, ..P48 }.frame_samples(), Err(Lc3Error::BadParameters));
        assert_eq!(Lc3Params { frame_us: 5000, ..P48 }.frame_samples(), Err(Lc3Error::BadParameters));
        assert_eq!(Lc3Params { octets: 19, ..P48 }.frame_samples(), Err(Lc3Error::BadLength));
        assert_eq!(Lc3Params { octets: 401, ..P48 }.frame_samples(), Err(Lc3Error::BadLength));
    }

    #[test]
    fn encodes_one_frame_per_call() {
        let mut enc = Lc3ChannelEncoder::new(P48).unwrap();
        assert_eq!((enc.frame_samples(), enc.octets()), (480, 100));
        let pcm: Vec<i16> = (0..480).map(|n| ((n as f64 * 0.13).sin() * 8000.0) as i16).collect();
        let mut out = [0u8; 100];
        enc.encode(&pcm, &mut out).unwrap();
        assert!(out.iter().any(|&b| b != 0));
        assert_eq!(enc.encode(&pcm[..479], &mut out), Err(Lc3Error::BadLength));
        assert_eq!(enc.encode(&pcm, &mut out[..99]), Err(Lc3Error::BadLength));
    }

    #[test]
    fn silence_is_deterministic() {
        let (mut a, mut b) = ([0u8; 100], [0u8; 100]);
        silence_frame(P48, &mut a).unwrap();
        silence_frame(P48, &mut b).unwrap();
        assert_eq!(a, b);
        assert!(silence_frame(P48, &mut a[..50]).is_err());
    }
}
