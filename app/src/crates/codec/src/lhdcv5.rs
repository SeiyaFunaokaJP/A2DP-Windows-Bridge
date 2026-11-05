//! LHDC V5: Savitech's high-resolution Bluetooth codec (44.1 / 48 / 96 /
//! 192 kHz, 16 or 24-bit stereo, 5 ms frames, 64 to 1000 kbps over A2DP),
//! encoded by a portable C port of the encoder Android ships (crate
//! `lhdcv5-sys`; the port and the AOSP original are Apache-2.0; LHDC itself
//! is Savitech's, whose licensing terms for products advertising LHDC are
//! separate from the code licence).
//!
//! The encoder takes one 5 ms frame of interleaved stereo PCM per call
//! (packed s16le or s24le: mono is duplicated, 24-bit input is repacked
//! from the top of the int32 container) and returns a complete packet of
//! one or more frames, each with its own 2-byte frame header, every few
//! calls. Parameters follow Android's a2dp_vendor_lhdcv5_encoder.cc as v1
//! did: 20 ms encode interval, packets sized for the MTU minus the 2-byte
//! media payload header, the sink's bit rate limits applied after init, and
//! the ABR policy of lhdcv5BT_enc.c ([`LhdcV5Encoder::abr_adjust`]).

use lhdcv5_sys::{LhdcV5Enc, ffi};

use crate::{CodecKind, Encoded, Encoder, EncoderConfig, EncoderError, Pcm, PcmFormat, Quality};

const CODEC: CodecKind = CodecKind::LhdcV5;

/// The A2DP media payload header in front of the frames
/// (A2DP_LHDC_MPL_HDR_LEN)
const MPL_HDR_LEN: usize = 2;

// ABR policy of Android's lhdcv5BT_enc.c. A "tick" is one media packet sent.
/// Ticks between checks for going up
const ABR_UP_RATE_TIME_CNT: u32 = 3000;
/// Ticks between checks for going down
const ABR_DOWN_RATE_TIME_CNT: u32 = 4;
const ABR_UP_QUEUE_LENGTH_THRESHOLD: u32 = 1;
const ABR_DOWN_QUEUE_LENGTH_THRESHOLD: u32 = 0;
const ABR_DOWN_TARGET_STAGE: usize = 0;
const ABR_TABLE_44K: [u32; 6] = [160, 192, 240, 320, 400, 400];
const ABR_TABLE_48K: [u32; 6] = [160, 192, 256, 320, 400, 400];
const ABR_TABLE_96K: [u32; 6] = [256, 320, 400, 400, 400, 400];
const ABR_TABLE_192K: [u32; 6] = [256, 320, 400, 400, 400, 400];

/// Adaptive bit rate state (Android lhdcv5BT_enc.c)
struct Abr {
    table: &'static [u32; 6],
    index: usize,
    down_count: u32,
    down_sum: u32,
    up_count: u32,
    up_sum: u32,
}

pub struct LhdcV5Encoder {
    enc: LhdcV5Enc,
    format: PcmFormat,
    channels: usize,
    /// PCM frames per LHDC frame (240 / 480 / 960)
    block_size: usize,
    /// MTU the encoder sizes its packets for (media MTU - payload header)
    enc_mtu: usize,
    /// Input repacked for the encoder: interleaved stereo s16le or packed
    /// s24le, one frame
    packed: Vec<u8>,
    abr: Option<Abr>,
    /// The sink's maximum bit rate (P7) is below the quality mode's
    peer_capped: bool,
}

/// Sink capability P7[5:4] -> encoder quality index
/// (A2DP_MaxBitRatetoQualityLevelLhdcV5)
fn peer_max_bitrate_index(p7: u8) -> u32 {
    match p7 & 0x30 {
        0x10 => ffi::LHDC_QUALITY_LOW,  // 400 kbps
        0x20 => ffi::LHDC_QUALITY_MID,  // 500 kbps
        0x30 => ffi::LHDC_QUALITY_HIGH, // 900 kbps
        _ => ffi::LHDC_QUALITY_HIGH1,   // 1000 kbps = no upper limit
    }
}

/// Whether the sink's maximum bit rate (P7) lowers the fixed bit rate index
/// a quality mode asks for. Never under ABR: it stays at 400 kbps or below
/// whatever the quality, as on Android.
fn peer_caps_bitrate(abr: bool, bitrate_inx: u32, p7: u8) -> bool {
    !abr && peer_max_bitrate_index(p7) < bitrate_inx
}

/// Sink capability P7[7:6] -> encoder quality index
/// (A2DP_MinBitRatetoQualityLevelLhdcV5)
fn peer_min_bitrate_index(p7: u8) -> u32 {
    match p7 & 0xC0 {
        0x40 => ffi::LHDC_QUALITY_LOW1, // 160 kbps
        0x80 => ffi::LHDC_QUALITY_LOW3, // 256 kbps
        0xC0 => ffi::LHDC_QUALITY_LOW,  // 400 kbps
        _ => ffi::LHDC_QUALITY_LOW0,    // 64 kbps = no lower limit
    }
}

/// The encoder's bit rate table (kbps) from 64 kbps up to
/// LHDC_QUALITY_HIGH1 (lhdcv5_api.c g_bitrate_table_*)
fn bitrate_table(sample_rate: u32) -> [u32; 9] {
    if sample_rate == 44100 {
        [64, 160, 192, 240, 320, 400, 480, 900, 1000]
    } else {
        [64, 160, 192, 256, 320, 400, 500, 900, 1000]
    }
}

/// Highest bit rate index up to LHDC_QUALITY_HIGH1 whose frames fit `mtu`.
/// The encoder sizes a frame for the bit rate over the frame's duration
/// (block_size / sample_rate), and the frame header rounds it up to an even
/// size plus 2 bytes; this estimate is that, rounded up once more.
fn fitting_bitrate_index(sample_rate: u32, block_size: usize, mtu: usize) -> u32 {
    let table = bitrate_table(sample_rate);
    let mut inx = ffi::LHDC_QUALITY_HIGH1;
    while inx > ffi::LHDC_QUALITY_LOW {
        let payload = (u64::from(table[inx as usize]) * 125 * block_size as u64).div_ceil(u64::from(sample_rate));
        if 2 + 2 * payload.div_ceil(2) as usize <= mtu {
            break;
        }
        inx -= 1;
    }
    inx
}

impl LhdcV5Encoder {
    pub fn new(config: &EncoderConfig) -> Result<LhdcV5Encoder, EncoderError> {
        let unsupported = |what: String| EncoderError::Unsupported { codec: CODEC, what };
        if !matches!(config.sample_rate, 44100 | 48000 | 96000 | 192000) {
            return Err(unsupported(format!(
                "sample rate {} Hz (use 44100, 48000, 96000 or 192000)",
                config.sample_rate
            )));
        }
        if !matches!(config.channels, 1 | 2) {
            return Err(unsupported(format!("{} channels", config.channels)));
        }
        let (format, encoder_bits) = match config.bits_per_sample {
            16 => (PcmFormat::I16, ffi::LHDCBT_SMPL_FMT_S16),
            24 => (PcmFormat::I32, ffi::LHDCBT_SMPL_FMT_S24),
            n => return Err(unsupported(format!("{n}-bit samples (use 16 or 24)"))),
        };

        // Quality mode -> target bit rate index (clamped to the sink's
        // limits below), the four fixed levels Android offers. ABR starts at
        // 400 kbps regardless (LHDC_QUALITY_AUTO).
        let bitrate_inx = if config.abr {
            ffi::LHDC_QUALITY_AUTO
        } else {
            match config.quality {
                Quality::High | Quality::LowLatency => ffi::LHDC_QUALITY_HIGH1, // 1000 kbps
                Quality::MediumHigh => ffi::LHDC_QUALITY_HIGH,                  // 900 kbps
                Quality::Standard => ffi::LHDC_QUALITY_MID,                     // 500 kbps (480 at 44.1 kHz)
                Quality::Mobile => ffi::LHDC_QUALITY_LOW,                       // 400 kbps
            }
        };

        // The encoder sizes its packets for this MTU: the media MTU (RTP
        // header already excluded) minus the 2-byte LHDC media payload header
        let enc_mtu = usize::from(config.mtu).saturating_sub(MPL_HDR_LEN);
        if enc_mtu < ffi::LHDC_MTU_MIN as usize {
            return Err(unsupported(format!(
                "media MTU {} (LHDC V5 needs at least {})",
                config.mtu,
                ffi::LHDC_MTU_MIN as usize + MPL_HDR_LEN
            )));
        }
        let enc_mtu = enc_mtu.min(ffi::LHDC_MTU_MAX as usize);

        let init_err = |e: lhdcv5_sys::Error| EncoderError::Init { codec: CODEC, what: e.to_string() };
        let mut enc = LhdcV5Enc::new().map_err(init_err)?;
        enc.init_encoder(config.sample_rate, encoder_bits, bitrate_inx, enc_mtu as u32, ffi::LHDC_ENC_INTERVAL_20MS)
            .map_err(init_err)?;

        let block_size = match enc.block_size() {
            Ok(n) if n > 0 => n as usize,
            r => {
                return Err(EncoderError::Init { codec: CODEC, what: format!("no frame size ({r:?})") });
            }
        };

        // Peer bit rate limits, applied after init as Android does; a
        // failure leaves the encoder's own limits (as v1)
        let p7 = config.peer.lhdcv5_bitrate_limits;
        let (mut max_inx, min_inx) = (peer_max_bitrate_index(p7), peer_min_bitrate_index(p7));
        let peer_capped = peer_caps_bitrate(config.abr, bitrate_inx, p7);
        // The encoder never splits a frame: one that does not fit the MTU
        // goes out whole, over the sink's MTU, and is dropped. Only 1000 kbps
        // at 44.1 kHz with the smallest MTUs gets there (a 5 ms frame sized
        // for 1000 kbps lasts 5.44 ms there: 682 bytes), so cap the bit rate
        // at what fits (900 kbps then). v1 sent those packets anyway.
        let fitting = fitting_bitrate_index(config.sample_rate, block_size, enc_mtu);
        tracing::debug!(
            "LHDC V5: quality {:?}, abr {} -> bitrate index {bitrate_inx}; sink limits byte 0x{p7:02x} -> index \
             {min_inx}..{max_inx}, fits the {enc_mtu}-byte mtu up to index {fitting}, frame {block_size} samples",
            config.quality,
            config.abr
        );
        if fitting < max_inx {
            tracing::info!(
                "LHDC V5: {} kbps frames do not fit the {enc_mtu}-byte MTU at {} Hz, capping the bit rate",
                bitrate_table(config.sample_rate)[max_inx as usize],
                config.sample_rate
            );
            max_inx = fitting;
        }
        if let Err(e) = enc.set_max_bitrate_index(max_inx) {
            tracing::warn!("LHDC V5: set_max_bitrate_index({max_inx}) failed: {e}");
        }
        if let Err(e) = enc.set_min_bitrate_index(min_inx) {
            tracing::warn!("LHDC V5: set_min_bitrate_index({min_inx}) failed: {e}");
        }

        let packed = vec![0u8; block_size * 2 * (encoder_bits as usize / 8)];

        // ABR table for this sample rate; start at its top (400 kbps)
        let abr = config.abr.then(|| {
            let table = match config.sample_rate {
                44100 => &ABR_TABLE_44K,
                96000 => &ABR_TABLE_96K,
                192000 => &ABR_TABLE_192K,
                _ => &ABR_TABLE_48K,
            };
            Abr { table, index: table.len() - 1, down_count: 0, down_sum: 0, up_count: 0, up_sum: 0 }
        });

        let encoder = LhdcV5Encoder {
            enc,
            format,
            channels: config.channels as usize,
            block_size,
            enc_mtu,
            packed,
            abr,
            peer_capped,
        };
        tracing::info!(
            "LHDC V5: initialized (rate={}, ch={}, {}-bit, mtu={}, frame={} samples, bitrate={} kbps{}, peer limits idx {}..{}{})",
            config.sample_rate,
            config.channels,
            encoder_bits,
            enc_mtu,
            block_size,
            encoder.bitrate_kbps(),
            if config.abr { " ABR" } else { "" },
            min_inx,
            max_inx,
            if peer_capped { ", capped by the sink's maximum" } else { "" }
        );
        Ok(encoder)
    }

    /// Whether adaptive bit rate is active
    pub fn abr_enabled(&self) -> bool {
        self.abr.is_some()
    }

    /// Index in the encoder's bit rate table of its current target
    fn current_index(&self) -> Option<u32> {
        self.enc.bitrate_index(self.enc.last_bitrate()).ok()
    }

    /// Repacks one frame of input into `packed`: interleaved stereo, s16le
    /// or packed s24le (the top 24 bits of the MSB-aligned int32)
    fn pack(&mut self, pcm: Pcm<'_>) {
        let channels = self.channels;
        match pcm {
            Pcm::I16(s) => {
                for (frame, dst) in s.chunks_exact(channels).zip(self.packed.as_chunks_mut::<4>().0) {
                    let (l, r) = (frame[0], frame[channels - 1]);
                    dst[..2].copy_from_slice(&l.to_le_bytes());
                    dst[2..].copy_from_slice(&r.to_le_bytes());
                }
            }
            Pcm::I32(s) => {
                for (frame, dst) in s.chunks_exact(channels).zip(self.packed.as_chunks_mut::<6>().0) {
                    let (l, r) = (frame[0].to_le_bytes(), frame[channels - 1].to_le_bytes());
                    dst[..3].copy_from_slice(&l[1..]);
                    dst[3..].copy_from_slice(&r[1..]);
                }
            }
        }
    }
}

impl Encoder for LhdcV5Encoder {
    fn kind(&self) -> CodecKind {
        CODEC
    }

    fn pcm_format(&self) -> PcmFormat {
        self.format
    }

    /// One LHDC frame (5 ms: 240 / 480 / 960 PCM frames) per call
    fn pcm_frames_per_encode(&self) -> usize {
        self.block_size
    }

    /// Packets never exceed the MTU the encoder was given (media MTU - 2)
    fn max_output_bytes(&self) -> usize {
        self.enc_mtu
    }

    fn encode(&mut self, pcm: Pcm<'_>, out: &mut [u8]) -> Result<Encoded, EncoderError> {
        let expected = self.block_size * self.channels;
        if pcm.samples() != expected {
            return Err(EncoderError::InputLength { expected, got: pcm.samples() });
        }
        let format_ok = matches!((pcm, self.format), (Pcm::I16(_), PcmFormat::I16) | (Pcm::I32(_), PcmFormat::I32));
        if !format_ok {
            return Err(EncoderError::Encode { codec: CODEC, what: format!("input is not {:?} PCM", self.format) });
        }
        if out.len() < self.enc_mtu {
            return Err(EncoderError::OutputTooSmall { have: out.len(), need: self.enc_mtu });
        }
        self.pack(pcm);
        let r = self.enc.encode(&self.packed, out).map_err(|e| EncoderError::Encode {
            codec: CODEC,
            what: format!("{e} (output buffer {} bytes)", out.len()),
        })?;
        Ok(Encoded { bytes: r.bytes, frames: r.frames })
    }

    /// The current target bit rate (changes under ABR)
    fn bitrate_kbps(&self) -> u32 {
        self.enc.last_bitrate()
    }

    fn bitrate_capped(&self) -> bool {
        self.peer_capped
    }

    /// ABR policy of Android's lhdcv5BT_enc.c (lhdcv5_enc_abr_adjust_bitrate).
    /// Call once per media packet sent, with the send queue depth (packets)
    /// seen before this round's packets were enqueued, as Android does
    /// (lhdcv5BT_adjust_bitrate) and v1 did. Every 4 calls an average depth
    /// above 0 drops the bit rate to the lowest stage of the table (160 kbps,
    /// 256 at 96 / 192 kHz); every 3000 calls with the queue empty
    /// throughout raise it one stage, up to 400 kbps. Does nothing when ABR
    /// is off.
    fn abr_adjust(&mut self, queue_depth: u32) {
        let Some(mut abr) = self.abr.take() else { return };

        if abr.down_count >= ABR_DOWN_RATE_TIME_CNT {
            let avg = abr.down_sum / abr.down_count;
            abr.down_count = 0;
            abr.down_sum = 0;
            if avg > ABR_DOWN_QUEUE_LENGTH_THRESHOLD {
                let new_stage = ABR_DOWN_TARGET_STAGE;
                if let (Some(last_inx), Ok(new_inx)) =
                    (self.current_index(), self.enc.bitrate_index(abr.table[new_stage]))
                    && new_inx <= last_inx
                    && new_stage < abr.index
                    && self.enc.set_bitrate_index(new_inx, false).is_ok()
                {
                    tracing::debug!(
                        "LHDC V5: ABR down {} -> {} kbps (queue avg {avg})",
                        abr.table[abr.index],
                        abr.table[new_stage]
                    );
                    abr.up_count = 0;
                    abr.up_sum = 0;
                    abr.index = new_stage;
                }
            }
        }

        if abr.up_count >= ABR_UP_RATE_TIME_CNT {
            let sum = abr.up_sum;
            abr.up_count = 0;
            abr.up_sum = 0;
            if sum < ABR_UP_QUEUE_LENGTH_THRESHOLD {
                let new_stage = if abr.index < abr.table.len() - 1 { abr.index + 1 } else { 0 };
                if let (Some(last_inx), Ok(new_inx)) =
                    (self.current_index(), self.enc.bitrate_index(abr.table[new_stage]))
                    && new_inx >= last_inx
                    && new_stage > abr.index
                    && self.enc.set_bitrate_index(new_inx, false).is_ok()
                {
                    tracing::debug!("LHDC V5: ABR up {} -> {} kbps", abr.table[abr.index], abr.table[new_stage]);
                    abr.down_count = 0;
                    abr.down_sum = 0;
                    abr.index = new_stage;
                }
            }
        }

        if queue_depth > 0 {
            abr.up_sum = abr.up_sum.saturating_add(queue_depth);
            abr.down_sum = abr.down_sum.saturating_add(queue_depth);
        }
        abr.up_count += 1;
        abr.down_count += 1;
        self.abr = Some(abr);
    }

    fn abr_active(&self) -> bool {
        self.abr.is_some()
    }

    /// Back to the top of the ABR table (400 kbps), counters cleared
    fn abr_reset(&mut self) {
        let Some(abr) = self.abr.as_mut() else { return };
        let top = abr.table.len() - 1;
        *abr = Abr { table: abr.table, index: top, down_count: 0, down_sum: 0, up_count: 0, up_sum: 0 };
        let rate = abr.table[top];
        match self.enc.bitrate_index(rate) {
            Ok(inx) if self.current_index() != Some(inx) => {
                if let Err(e) = self.enc.set_bitrate_index(inx, false) {
                    tracing::warn!("LHDC V5: ABR reset to {rate} kbps failed: {e}");
                }
            }
            Ok(_) => {}
            Err(e) => tracing::warn!("LHDC V5: ABR reset to {rate} kbps failed: {e}"),
        }
    }
}
