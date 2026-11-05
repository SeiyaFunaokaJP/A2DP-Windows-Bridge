#![forbid(unsafe_code)]

mod codec;
#[cfg(any(test, feature = "fake"))]
pub mod fake;
mod frame;
mod stream;
mod tables;
#[cfg(test)]
mod vectors;

use std::fmt;

use codec::packetiser::Packetiser;
use codec::{Params, Variant, alloc};

pub use stream::{Core, Stream, table_row, table_row_at_level};
pub use tables::{DecoderLevel, Profile, Row, capped_bitrate, packet_budget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    pub profile: Profile,
    pub sample_rate: u32,
    pub channels: u8,
    pub level: DecoderLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    UnsupportedConfig,
    /// Encoder window not a 12..=1032-byte packet (a multiple of 4) plus slack, or a different
    /// packet size from the first call.
    WindowSize,
    /// Caller's buffer can't hold a packet.
    OutputTooSmall,
    /// The packet size the link allows gives a bitrate below what the encoder runs at, so
    /// unencoded audio would pile up (`codec-core.md` §8.8).
    PacketTooSmall,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedConfig => f.write_str("unsupported encoder configuration"),
            Self::WindowSize => f.write_str("encoder window size out of range"),
            Self::OutputTooSmall => f.write_str("output buffer too small"),
            Self::PacketTooSmall => {
                f.write_str("packet size too small for the encoder's lowest bitrate")
            }
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Progress {
    pub consumed: usize,
    pub written: usize,
}

const MIN_WINDOW: usize = 12;
/// The packet header's size byte tops out at 1032 bytes (`codec-core.md` §7.1).
pub const MAX_PACKET: usize = codec::pack::MAX_PACKET;
/// Room the encoder gets past the packet size (§3.4).
pub const WINDOW_SLACK: usize = 64;

pub struct Encoder {
    params: Params,
    bitrate: u32,
    packetiser: Option<Box<Packetiser>>,
}

impl fmt::Debug for Encoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Encoder")
            .field("params", &self.params)
            .field("bitrate", &self.bitrate)
            .finish_non_exhaustive()
    }
}

/// The bitstream variant for a config (`codec-core.md` §1, §10.D, §10.F,
/// `a2dp-integration.md` §7J): HQ at 88.2/96 kHz is config 3 whatever the decoder level; LL
/// there isn't allowed. An R1 sink gets config 0 in both modes (in LL on the LL table row,
/// with the LL loop), and an R2 sink in LL gets config 4. The low-power config 1 (create
/// profile 1) is never given to a sink, so it's only reachable from tests.
fn params(config: Config) -> Result<Params> {
    let high_rate = match config.sample_rate {
        44_100 | 48_000 => false,
        88_200 | 96_000 => true,
        _ => return Err(Error::UnsupportedConfig),
    };
    let variant = match (config.profile, config.level, high_rate) {
        (Profile::Hq, _, true) | (Profile::Hq, DecoderLevel::R2, false) => Variant::Cfg3,
        (_, DecoderLevel::R1, false) => Variant::Cfg0,
        (Profile::Ll, DecoderLevel::R2, false) => Variant::Cfg4,
        (Profile::Ll, _, true) => return Err(Error::UnsupportedConfig),
    };
    Ok(Params {
        variant,
        sample_rate: config.sample_rate,
    })
}

/// The lowest bitrate the encoder runs at: set-bitrate clamps anything lower up to this
/// (`codec-core.md` §8.1a).
///
/// # Errors
/// As for [`Encoder::new`].
pub fn min_bitrate(config: Config) -> Result<u32> {
    Ok(alloc::bitrate_range(params(config)?).0)
}

/// The filterbank delay in sample frames (`a2dp-integration.md` §7I), not counting the packet
/// wait. `None` for config 4, whose delay isn't measured yet.
///
/// # Errors
/// As for [`Encoder::new`].
pub fn delay_frames(config: Config) -> Result<Option<u32>> {
    Ok(match params(config)?.variant {
        Variant::Cfg0 => Some(90),
        Variant::Cfg1 => Some(66),
        Variant::Cfg3 => Some(255),
        Variant::Cfg4 => None,
    })
}

/// The smallest packet for a `period_ms` row whose MTU-fitted bitrate ([`Row::fit`]) the
/// encoder can run at. Below it the encoder codes faster than the packets carry, so the
/// unconsumed input grows every period (`codec-core.md` §8.8).
///
/// # Errors
/// As for [`Encoder::new`].
pub fn min_packet_bytes(config: Config, period_ms: u32) -> Result<usize> {
    let lo = u64::from(min_bitrate(config)?);
    // Inverse of `Row::fit` (the integer form equals its float32 one while positive):
    // size·8000/period − 25000 ≥ lo.
    let bytes = ((lo + 25_000) * u64::from(period_ms)).div_ceil(8000);
    Ok((bytes as usize).next_multiple_of(4))
}

impl Encoder {
    /// # Errors
    /// `UnsupportedConfig` for anything but stereo configs 0, 3 and 4 at their sample rates,
    /// or a zero bitrate.
    pub fn new(config: Config, bitrate: u32) -> Result<Self> {
        Self::with_params(config, params(config)?, bitrate)
    }

    fn with_params(config: Config, params: Params, bitrate: u32) -> Result<Self> {
        if config.channels != 2 || bitrate == 0 {
            return Err(Error::UnsupportedConfig);
        }
        Ok(Self {
            params,
            bitrate,
            packetiser: None,
        })
    }

    /// # Errors
    /// `WindowSize` unless `packet_len` is a multiple of 4 in 12..=[`MAX_PACKET`], or differs
    /// from the size already in use.
    pub fn prepare(&mut self, packet_len: usize) -> Result<()> {
        self.packetiser(&vec![0; packet_len + WINDOW_SLACK])
            .map(|_| ())
    }

    /// Back to a freshly created encoder for `config`, `bitrate` and `packet_len`-byte
    /// packets, in place: no allocation once [`Encoder::prepare`] has run (the reference resets
    /// in place on an HQ↔LL switch too, `a2dp-integration.md` §7K). On error nothing changes.
    ///
    /// # Errors
    /// As for [`Encoder::new`], or `WindowSize` for a bad packet size.
    pub fn reset(&mut self, config: Config, bitrate: u32, packet_len: usize) -> Result<()> {
        if config.channels != 2 || bitrate == 0 {
            return Err(Error::UnsupportedConfig);
        }
        let params = params(config)?;
        check_window_len(packet_len)?;
        self.params = params;
        self.bitrate = bitrate;
        match &mut self.packetiser {
            Some(p) => p.reset(params, packet_len, bitrate),
            None => self.packetiser = Some(Box::new(Packetiser::new(params, packet_len, bitrate))),
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn low_power(config: Config, bitrate: u32) -> Result<Self> {
        if !matches!(config.sample_rate, 44_100 | 48_000) {
            return Err(Error::UnsupportedConfig);
        }
        let params = Params {
            variant: Variant::Cfg1,
            sample_rate: config.sample_rate,
        };
        Self::with_params(config, params, bitrate)
    }

    fn packetiser(&mut self, window: &[u8]) -> Result<&mut Packetiser> {
        let size = check_window(window)?;
        let (params, bitrate) = (self.params, self.bitrate);
        let p = self
            .packetiser
            .get_or_insert_with(|| Box::new(Packetiser::new(params, size, bitrate)));
        if p.packet_len() == size {
            Ok(p)
        } else {
            Err(Error::WindowSize)
        }
    }

    /// # Errors
    /// `WindowSize` unless the packet size is a multiple of 4 in 12..=[`MAX_PACKET`] bytes and
    /// the same on every call.
    pub fn encode_packet(&mut self, pcm: &[i32], window: &mut [u8]) -> Result<Progress> {
        let (consumed, written) = self.packetiser(window)?.encode(pcm, window);
        Ok(Progress { consumed, written })
    }

    /// # Errors
    /// As for [`Encoder::encode_packet`].
    pub fn flush(&mut self, window: &mut [u8]) -> Result<usize> {
        Ok(self.packetiser(window)?.flush(window))
    }

    pub fn set_bitrate(&mut self, bps: u32) {
        match &mut self.packetiser {
            Some(p) => p.set_bitrate(bps),
            None => self.bitrate = bps,
        }
    }
}

fn check_window(window: &[u8]) -> Result<usize> {
    check_window_len(window.len().saturating_sub(WINDOW_SLACK))
}

fn check_window_len(size: usize) -> Result<usize> {
    if (MIN_WINDOW..=MAX_PACKET).contains(&size) && size.is_multiple_of(4) {
        Ok(size)
    } else {
        Err(Error::WindowSize)
    }
}

impl Core for Encoder {
    fn encode_packet(&mut self, pcm: &[i32], window: &mut [u8]) -> Result<Progress> {
        Encoder::encode_packet(self, pcm, window)
    }
    fn flush(&mut self, window: &mut [u8]) -> Result<usize> {
        Encoder::flush(self, window)
    }
    fn set_bitrate(&mut self, bps: u32) {
        Encoder::set_bitrate(self, bps);
    }
    fn prepare(&mut self, row: &Row) -> Result<()> {
        Encoder::prepare(self, row.packet_bytes)
    }
    fn reset(&mut self, config: Config, row: &Row) -> Result<()> {
        Encoder::reset(self, config, row.bitrate, row.packet_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEREO_48K: Config = Config {
        profile: Profile::Hq,
        sample_rate: 48_000,
        channels: 2,
        level: DecoderLevel::R1,
    };

    #[test]
    fn rejects_unsupported_config() {
        for cfg in [
            Config {
                channels: 1,
                ..STEREO_48K
            },
            Config {
                profile: Profile::Ll,
                level: DecoderLevel::R2,
                sample_rate: 88_200,
                ..STEREO_48K
            },
            Config {
                profile: Profile::Ll,
                sample_rate: 96_000,
                ..STEREO_48K
            },
            Config {
                sample_rate: 32_000,
                ..STEREO_48K
            },
        ] {
            assert_eq!(
                Encoder::new(cfg, 364_000).unwrap_err(),
                Error::UnsupportedConfig
            );
        }
        assert_eq!(
            Encoder::new(STEREO_48K, 0).unwrap_err(),
            Error::UnsupportedConfig
        );
    }

    /// §1: HQ at 88.2/96 kHz is config 3 for any decoder level, R1 included.
    #[test]
    fn hq_high_rate_is_config_3() {
        for level in [DecoderLevel::R1, DecoderLevel::R2] {
            for sample_rate in [88_200, 96_000] {
                let cfg = Config {
                    sample_rate,
                    level,
                    ..STEREO_48K
                };
                assert_eq!(params(cfg).unwrap().variant, Variant::Cfg3);
                let mut enc = Encoder::new(cfg, 420_000).unwrap();
                let mut out = [0u8; 656 + WINDOW_SLACK];
                enc.encode_packet(&[0; 2880], &mut out).unwrap();
                assert_eq!(enc.flush(&mut out), Ok(656));
                assert_eq!(out[1] & 0x7F, 3);
            }
        }
        let r1_48k = params(STEREO_48K).unwrap();
        assert_eq!(r1_48k.variant, Variant::Cfg0);
    }

    /// Config 4 has no allocation escapes, so it must never allocate over 9 bits, whatever
    /// bitrate it's created or set to (§10.F). Loud noise with the rate check lifting the
    /// target by 5/4, then packets that decode.
    #[test]
    fn config_4_survives_any_bitrate() {
        use codec::decode::decode;
        let config = Config {
            profile: Profile::Ll,
            level: DecoderLevel::R2,
            ..STEREO_48K
        };
        let mut s = 1u32;
        let pcm: Vec<i32> = (0..96_000)
            .map(|i| {
                s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let n = (s as i32) >> 8;
                if i < 48_000 {
                    n
                } else {
                    let t = f64::from(i / 2) / 48_000.0;
                    let tone = 8_000_000.0 * (2.0 * std::f64::consts::PI * 700.0 * t).sin();
                    tone as i32 + (n >> 8)
                }
            })
            .collect();
        for (create, set) in [
            (600_000, None),
            (u32::MAX, None),
            (270_000, Some(600_000)),
            (270_000, Some(u32::MAX)),
        ] {
            let mut enc = Encoder::new(config, create).unwrap();
            let mut window = [0u8; 560 + WINDOW_SLACK];
            let mut packets = Vec::new();
            for chunk in pcm.chunks(960) {
                let mut pos = 0;
                loop {
                    let p = enc.encode_packet(&chunk[pos..], &mut window).unwrap();
                    pos += p.consumed;
                    if p.written == 0 {
                        break;
                    }
                    packets.extend_from_slice(&window[..560]);
                }
                enc.flush(&mut window).unwrap();
                packets.extend_from_slice(&window[..560]);
                if let Some(b) = set {
                    enc.set_bitrate(b);
                }
            }
            let frames = decode(params(config).unwrap(), &packets, 560);
            assert!(frames.len() > 50, "{create} {set:?}");
            assert!(frames.iter().all(|f| f.bits.iter().all(|&b| b <= 9)));
        }
    }

    /// §10.D / §7J: an R1 sink in LL gets config 0, an R2 sink config 4.
    #[test]
    fn ll_variants() {
        for sample_rate in [44_100, 48_000] {
            let ll = |level| Config {
                profile: Profile::Ll,
                sample_rate,
                level,
                ..STEREO_48K
            };
            assert_eq!(params(ll(DecoderLevel::R1)).unwrap().variant, Variant::Cfg0);
            assert_eq!(params(ll(DecoderLevel::R2)).unwrap().variant, Variant::Cfg4);
        }
    }

    #[test]
    fn delays() {
        let r2 = |profile, sample_rate| Config {
            profile,
            sample_rate,
            level: DecoderLevel::R2,
            ..STEREO_48K
        };
        let ll_r1 = Config {
            profile: Profile::Ll,
            ..STEREO_48K
        };
        assert_eq!(delay_frames(STEREO_48K), Ok(Some(90)));
        assert_eq!(delay_frames(ll_r1), Ok(Some(90)));
        assert_eq!(delay_frames(r2(Profile::Hq, 48_000)), Ok(Some(255)));
        assert_eq!(delay_frames(r2(Profile::Hq, 96_000)), Ok(Some(255)));
        assert_eq!(delay_frames(r2(Profile::Ll, 48_000)), Ok(None));
    }

    /// §7J: LL with an R1 sink is config 0 (frame type 1), not config 1.
    #[test]
    fn r1_ll_is_config_0() {
        let ll = Config {
            profile: Profile::Ll,
            ..STEREO_48K
        };
        assert_eq!(params(ll).unwrap().variant, Variant::Cfg0);
        let mut enc = Encoder::new(ll, 265_000).unwrap();
        let mut out = [0u8; 348 + WINDOW_SLACK];
        enc.encode_packet(&[0; 960], &mut out).unwrap();
        assert_eq!(enc.flush(&mut out), Ok(348));
        assert_eq!(out[..3], [0x00, 0x81, 84]);
    }

    #[test]
    fn reset_matches_new() {
        let ll = Config {
            profile: Profile::Ll,
            ..STEREO_48K
        };
        let pcm: Vec<i32> = (0..9600).map(|i| ((i * 7919) % 20_000) - 10_000).collect();
        let run = |enc: &mut Encoder, len: usize| {
            let mut out = vec![0u8; len + WINDOW_SLACK];
            let mut got = Vec::new();
            for chunk in pcm.chunks(960) {
                enc.encode_packet(chunk, &mut out).unwrap();
                let n = enc.flush(&mut out).unwrap();
                got.extend_from_slice(&out[..n]);
            }
            got
        };
        let want = run(&mut Encoder::new(ll, 265_000).unwrap(), 348);
        let mut enc = Encoder::new(STEREO_48K, 364_000).unwrap();
        enc.prepare(656).unwrap();
        run(&mut enc, 656);
        enc.reset(ll, 265_000, 348).unwrap();
        assert_eq!(run(&mut enc, 348), want);
        let ll_96k = Config {
            sample_rate: 96_000,
            ..ll
        };
        assert_eq!(
            enc.reset(ll_96k, 265_000, 348),
            Err(Error::UnsupportedConfig)
        );
        assert_eq!(enc.reset(ll, 265_000, 350), Err(Error::WindowSize));
        assert_eq!(enc.flush(&mut [0; 348 + WINDOW_SLACK]), Ok(0));
    }

    #[test]
    fn min_packet_is_under_the_px7_packet() {
        let need = min_packet_bytes(STEREO_48K, 14).unwrap();
        assert!(need <= 656 && need > 128, "{need}");
        let row = Row {
            bitrate: 364_000,
            period_ms: 14,
            samples_per_channel: 672,
            packet_bytes: 656,
            cfg: 1,
        };
        assert!(row.fit(Profile::Hq, need).bitrate >= min_bitrate(STEREO_48K).unwrap());
        assert!(row.fit(Profile::Hq, need - 4).bitrate < min_bitrate(STEREO_48K).unwrap());
    }

    #[test]
    fn encode_buffers_and_flush_emits() {
        let mut enc = Encoder::new(STEREO_48K, 364_000).unwrap();
        let mut out = [0u8; 656 + WINDOW_SLACK];
        assert_eq!(
            enc.encode_packet(&[0; 1344], &mut out),
            Ok(Progress {
                consumed: 1344,
                written: 0
            })
        );
        assert_eq!(enc.flush(&mut out), Ok(656));
        assert_eq!(out[..3], [0x00, 0x81, 0xA1]);
        assert_eq!(out[3] >> 3, 0b1_1010);
    }

    #[test]
    fn window_bounds() {
        let mut enc = Encoder::new(STEREO_48K, 364_000).unwrap();
        assert_eq!(enc.flush(&mut [0; 11 + 64]), Err(Error::WindowSize));
        assert_eq!(enc.flush(&mut [0; 2017 + 64]), Err(Error::WindowSize));
        assert_eq!(enc.flush(&mut [0; 658 + 64]), Err(Error::WindowSize));
        assert_eq!(enc.flush(&mut [0; 1036 + 64]), Err(Error::WindowSize));
        let mut window = [0; 1032 + 64];
        assert_eq!(enc.flush(&mut window), Ok(0));
        enc.encode_packet(&[0; 16], &mut window).unwrap();
        assert_eq!(enc.flush(&mut window), Ok(1032));
        assert_eq!(window[2], 0xFF);
        assert_eq!(enc.flush(&mut [0; 12 + 64]), Err(Error::WindowSize));
    }
}
