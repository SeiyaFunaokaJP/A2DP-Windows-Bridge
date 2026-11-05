//! Per-packet loop (spec `a2dp-integration.md` §5) around an encoder core.

use crate::frame;
use crate::tables::{self, DEFAULT_QUALITY, Link, Profile, Row};
use crate::{Config, Encoder, Error, Progress, Result, WINDOW_SLACK};

pub trait Core {
    /// # Errors
    /// Encoder failures.
    fn encode_packet(&mut self, pcm: &[i32], window: &mut [u8]) -> Result<Progress>;
    /// # Errors
    /// Encoder failures.
    fn flush(&mut self, window: &mut [u8]) -> Result<usize>;
    fn set_bitrate(&mut self, bps: u32);
    /// # Errors
    /// Encoder failures.
    fn prepare(&mut self, _row: &Row) -> Result<()> {
        Ok(())
    }
    /// # Errors
    /// `UnsupportedConfig` if this core can't (the default), or encoder failures; on error
    /// nothing changes.
    fn reset(&mut self, _config: Config, _row: &Row) -> Result<()> {
        Err(Error::UnsupportedConfig)
    }
    fn period_changed(&mut self, _samples: usize) {}
}

/// §7J.
const DEFICIT_LIMIT: i64 = 96;

/// Queued input the stream holds without growing: four of the longest packets (40 ms at
/// 96 kHz stereo), far more than one push of a period leaves behind.
const PCM_CAPACITY: usize = 4 * 2 * 3840;

#[derive(Debug)]
pub struct Stream<C> {
    core: C,
    config: Config,
    row: Row,
    samples_per_packet: usize,
    pcm: Vec<i32>,
    window: Vec<u8>,
    consumed: usize,
    deficit: i64,
    budget: usize,
    level: u8,
    pending: Option<u8>,
    hold: bool,
    bitrate: u32,
    before_step: (Row, usize, u8, u32),
    /// §7L / §8.2: a request for level 1 is treated as 2. Off only for the raw-row vectors.
    map_level_1: bool,
}

impl Stream<Encoder> {
    /// # Errors
    /// Whatever the encoder rejects.
    pub fn new(config: Config, budget: usize) -> Result<Self> {
        let row = table_row(config, budget)?;
        let core = Encoder::new(config, row.bitrate)?;
        Ok(Self::with_core(core, config, row).with_budget(budget))
    }
}

/// # Errors
/// `UnsupportedConfig` if there's no row or encoder for `config`, `PacketTooSmall` if the
/// fitted row's bitrate is below what the encoder runs at, which would grow the backlog every
/// period (`codec-core.md` §8.8, [`crate::min_packet_bytes`]).
pub fn table_row(config: Config, budget: usize) -> Result<Row> {
    table_row_at_level(config, budget, DEFAULT_QUALITY)
}

/// # Errors
/// As for [`table_row`], `UnsupportedConfig` for a level outside 1..=7, and
/// `PacketTooSmall` for an LL row bigger than `budget`.
pub fn table_row_at_level(config: Config, budget: usize, level: u8) -> Result<Row> {
    let link = Link {
        rate_96k: config.sample_rate == 96_000,
        ..Link::default()
    };
    let index = tables::index(config.profile, config.level, link);
    let row = tables::row(config.profile, index, level)
        .ok_or(Error::UnsupportedConfig)?
        .fit(config.profile, budget);
    if row.packet_bytes > budget || row.bitrate < crate::min_bitrate(config)? {
        return Err(Error::PacketTooSmall);
    }
    Ok(row)
}

impl<C: Core> Stream<C> {
    pub fn with_core(mut core: C, config: Config, row: Row) -> Self {
        let _ = core.prepare(&row);
        let pcm = Vec::with_capacity(PCM_CAPACITY);
        let samples_per_packet =
            row.interleaved_samples(config.sample_rate, config.channels.into());
        Self {
            core,
            config,
            row,
            samples_per_packet,
            pcm,
            window: vec![0; crate::MAX_PACKET + WINDOW_SLACK],
            consumed: 0,
            deficit: 0,
            budget: row.packet_bytes,
            level: DEFAULT_QUALITY,
            pending: None,
            hold: false,
            bitrate: row.bitrate,
            before_step: (row, samples_per_packet, DEFAULT_QUALITY, row.bitrate),
            map_level_1: true,
        }
    }

    /// Switches to `config` and `row` in place (§7K): resets the core, zeroes the loop
    /// counters, sets the level to 3 and drops any pending or held level request. Queued input
    /// is kept. No allocation, given a core whose reset doesn't allocate. On error nothing
    /// changes.
    ///
    /// # Errors
    /// Whatever the core's reset returns.
    pub fn reset(&mut self, config: Config, row: Row) -> Result<()> {
        self.core.reset(config, &row)?;
        self.config = config;
        self.row = row;
        self.samples_per_packet =
            row.interleaved_samples(config.sample_rate, config.channels.into());
        self.consumed = 0;
        self.deficit = 0;
        self.budget = self.budget.max(row.packet_bytes);
        self.level = DEFAULT_QUALITY;
        self.pending = None;
        self.bitrate = row.bitrate;
        self.before_step = (row, self.samples_per_packet, DEFAULT_QUALITY, row.bitrate);
        Ok(())
    }

    #[must_use]
    pub fn with_budget(mut self, budget: usize) -> Self {
        self.budget = budget.max(self.row.packet_bytes);
        self
    }

    #[must_use]
    pub fn budget(&self) -> usize {
        self.budget
    }

    #[must_use]
    pub fn level(&self) -> u8 {
        self.level
    }

    #[must_use]
    pub fn pending_level(&self) -> Option<u8> {
        self.pending
    }

    #[must_use]
    pub fn level_row(&self, level: u8) -> Option<Row> {
        let row = table_row_at_level(self.config, self.budget, level).ok()?;
        (row.packet_bytes == self.row.packet_bytes).then_some(row)
    }

    pub fn request_level(&mut self, level: u8) -> bool {
        let level = match level {
            1 if self.map_level_1 => 2,
            0 | 8.. => return false,
            l => l,
        };
        if self.level_row(level).is_none() {
            return false;
        }
        self.pending = Some(level);
        true
    }

    pub fn hold_level(&mut self, hold: bool) {
        self.hold = hold;
    }

    #[cfg(test)]
    pub(crate) fn set_map_level_1(&mut self, on: bool) {
        self.map_level_1 = on;
    }

    #[cfg(test)]
    pub(crate) fn deficit(&self) -> i64 {
        self.deficit
    }

    fn apply_level(&mut self) {
        if self.hold {
            return;
        }
        let Some(level) = self.pending.take() else {
            return;
        };
        let Some(row) = self.level_row(level) else {
            return;
        };
        self.level = level;
        self.row = row;
        if row.bitrate != self.bitrate {
            self.set_bitrate(row.bitrate);
            self.samples_per_packet =
                row.interleaved_samples(self.config.sample_rate, self.config.channels.into());
            self.core.period_changed(self.samples_per_packet);
        }
    }

    fn set_bitrate(&mut self, bps: u32) {
        self.bitrate = bps;
        self.core.set_bitrate(bps);
    }

    #[must_use]
    pub fn row(&self) -> Row {
        self.row
    }

    #[must_use]
    pub fn config(&self) -> Config {
        self.config
    }

    #[must_use]
    pub fn samples_per_packet(&self) -> usize {
        self.samples_per_packet
    }

    #[cfg(any(test, feature = "fake"))]
    pub fn core(&self) -> &C {
        &self.core
    }

    /// # Errors
    /// `OutputTooSmall` if `out` can't hold a packet, or encoder failures.
    pub fn push(&mut self, pcm: &[i32], out: &mut [u8]) -> Result<Option<usize>> {
        if out.len() < self.row.packet_bytes {
            return Err(Error::OutputTooSmall);
        }
        self.pcm.extend_from_slice(pcm);
        let mut window = std::mem::take(&mut self.window);
        let res = self.encode(&mut window, out);
        self.window = window;
        res
    }

    fn encode(&mut self, window: &mut [u8], out: &mut [u8]) -> Result<Option<usize>> {
        let window = &mut window[..self.row.packet_bytes + WINDOW_SLACK];
        let p = self.core.encode_packet(&self.pcm, window)?;
        self.pcm.drain(..p.consumed);
        self.consumed += p.consumed;

        let due = self.consumed as i64 >= self.samples_per_packet as i64 + self.deficit;
        let written = match p.written {
            0 if !due => return Ok(None),
            0 => self.core.flush(window)?,
            n => n,
        };
        if written == 0 {
            return Ok(None);
        }
        if written > out.len() {
            return Err(Error::OutputTooSmall);
        }
        self.deficit += self.samples_per_packet as i64 - self.consumed as i64;
        self.consumed = 0;
        frame::swap16(&window[..written], out);

        // §7K, codec-core §10.C: LL never carries a deficit. The reference's extra flush here
        // hits a full window and never produces anything, so it isn't made.
        if self.config.profile == Profile::Ll {
            self.deficit = 0;
        }

        self.after_packet();
        Ok(Some(written))
    }

    fn after_packet(&mut self) {
        self.before_step = (self.row, self.samples_per_packet, self.level, self.bitrate);
        self.apply_level();
        // §7J: the 25 % drop clamps at the encoder's floor (200 kbps in stereo).
        let table = self.row.bitrate;
        let floor = crate::min_bitrate(self.config).unwrap_or(0);
        self.set_bitrate(if self.deficit > DEFICIT_LIMIT {
            (table - table / 4).max(floor.min(table))
        } else {
            table
        });
    }

    pub fn apply_level_now(&mut self) -> bool {
        if self.consumed != 0 || self.hold || self.pending.is_none() {
            return false;
        }
        // Set-bitrate starts the rate control afresh, so redoing the step from the value it
        // replaced leaves the core as if the level had been applied with the packet.
        let row_changed = self.before_step.1 != self.samples_per_packet;
        (self.row, self.samples_per_packet, self.level, self.bitrate) = self.before_step;
        if row_changed {
            self.core.period_changed(self.samples_per_packet);
        }
        self.after_packet();
        true
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;
    use crate::fake::{self, Fake};
    use crate::{DecoderLevel, packet_budget};

    const PX7: Config = Config {
        profile: Profile::Hq,
        sample_rate: 48_000,
        channels: 2,
        level: DecoderLevel::R1,
    };

    fn px7() -> Stream<Fake> {
        fake::stream(PX7, packet_budget(895)).unwrap()
    }

    fn vectors() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/vectors/r2")
    }

    fn read_pcm(name: &str) -> Vec<i32> {
        let bytes = fs::read(vectors().join(name)).unwrap();
        let (samples, _) = bytes.as_chunks::<4>();
        samples.iter().map(|b| i32::from_le_bytes(*b)).collect()
    }

    fn run(s: &mut Stream<Fake>, pcm: &[i32], frames_per_block: usize) -> Vec<usize> {
        let mut out = [0; 2008];
        pcm.chunks(frames_per_block * 2)
            .filter_map(|b| s.push(b, &mut out).unwrap())
            .collect()
    }

    #[test]
    fn one_second_gives_71_forced_packets() {
        let pcm = read_pcm("sine1k.s32le");
        assert_eq!(pcm.len(), 96_000);
        for block in [672, 48, 100] {
            let mut s = px7();
            assert_eq!(run(&mut s, &pcm, block), vec![656; 71], "block {block}");
            assert_eq!(s.core().forced, vec![true; 71]);
            assert!(s.core().bitrates.iter().all(|&b| b == 364_000));
        }
    }

    #[test]
    fn reproduces_vector_sizes() {
        for name in ["silence", "sine1k", "noise"] {
            let pcm = read_pcm(&format!("{name}.s32le"));
            for level in ["r1", "r2"] {
                let path = vectors().join(format!("{name}.{level}.sizes"));
                let want: Vec<(usize, bool)> = fs::read_to_string(path)
                    .unwrap()
                    .lines()
                    .enumerate()
                    .map(|(i, l)| {
                        let f: Vec<usize> = l.split(' ').map(|f| f.parse().unwrap()).collect();
                        assert_eq!(f[0], i);
                        (f[1], f[2] == 1)
                    })
                    .collect();
                let mut s = px7();
                for (i, _) in want.iter().enumerate().filter(|(_, w)| !w.1) {
                    s.core.early.insert(i, 1344);
                }
                let sizes = run(&mut s, &pcm, 672);
                let got: Vec<_> = sizes.into_iter().zip(s.core().forced.clone()).collect();
                assert_eq!(got, want, "{name}.{level}");
                let bin = fs::read(vectors().join(format!("{name}.{level}.bin"))).unwrap();
                assert_eq!(bin.len(), want.iter().map(|w| w.0).sum::<usize>());
            }
        }
    }

    #[test]
    fn packets_are_swapped() {
        let mut s = px7();
        let mut out = [0; 700];
        let mut pcm = [0; 1344];
        pcm[..2].copy_from_slice(&[0x12, 0x34]);
        assert_eq!(s.push(&pcm, &mut out).unwrap(), Some(656));
        assert_eq!(out[..2], [0x34, 0x12]);
    }

    #[test]
    fn early_packet_carries_deficit_and_lowers_bitrate() {
        let mut s = px7();
        s.core.early.extend([(0, 1000), (1, 1688)]);
        let mut out = [0; 656];
        assert_eq!(s.push(&[0; 1344], &mut out).unwrap(), Some(656));
        assert_eq!(s.deficit, 344);
        assert_eq!(s.core().bitrates, [273_000]);
        // 344 samples carried over; the next packet is due at 1344 + 344 consumed.
        assert_eq!(s.push(&[0; 1000], &mut out).unwrap(), None);
        assert_eq!(s.push(&[0; 344], &mut out).unwrap(), Some(656));
        assert_eq!(s.deficit, 0);
        assert_eq!(s.core().bitrates, [273_000, 364_000]);
    }

    #[test]
    fn rejects_packets_that_cant_hold_a_period() {
        assert_eq!(table_row(PX7, 128).unwrap_err(), Error::PacketTooSmall);
        let need = crate::min_packet_bytes(PX7, 14).unwrap();
        assert_eq!(table_row(PX7, need).unwrap().packet_bytes, need);
        assert!(table_row(PX7, need - 4).is_err());
    }

    /// An R1 sink in LL (config 0 on the LL row, 960-sample period, `codec-core.md` §10.D) fed
    /// 1344 samples a call for 30 s, draining with empty pushes: the queue stays under two
    /// periods. LL drops the deficit (§10.C), so a packet that took more than a period isn't
    /// made up for: there are never more packets than periods, and never fewer than one per
    /// push.
    #[test]
    fn oversized_pushes_stay_bounded_when_drained() {
        let config = Config {
            profile: Profile::Ll,
            ..PX7
        };
        let row = table_row(config, packet_budget(895)).unwrap();
        let mut s = Stream::with_core(Encoder::new(config, row.bitrate).unwrap(), config, row);
        assert_eq!(s.samples_per_packet(), 960);
        let capacity = s.pcm.capacity();
        let mut out = [0; 2008];
        let chunk: Vec<i32> = (0..1344).map(|i| (i * 2654) % 65_536 - 32_768).collect();
        let (mut fed, mut packets, mut most) = (0, 0, 0);
        while fed < 30 * 96_000 {
            packets += usize::from(s.push(&chunk, &mut out).unwrap().is_some());
            while s.push(&[], &mut out).unwrap().is_some() {
                packets += 1;
            }
            fed += chunk.len();
            most = most.max(s.pcm.len());
        }
        assert!(most < 2 * 960, "queue reached {most}");
        assert_eq!(s.pcm.capacity(), capacity);
        assert!(
            packets <= fed / 960 && packets >= fed / chunk.len(),
            "{packets} packets for {fed}"
        );
    }

    /// §7K: in LL the deficit is never carried. 30 s in 2 ms blocks is one packet per 10 ms
    /// and the bitrate never drops.
    #[test]
    fn ll_never_carries_a_deficit() {
        let row = table_row(LL, packet_budget(895)).unwrap();
        let mut s = Stream::with_core(Encoder::new(LL, row.bitrate).unwrap(), LL, row);
        let mut out = [0; 348];
        let mut seed = 1u32;
        let (mut fed, mut packets) = (0, 0);
        while fed < 30 * 96_000 {
            let block: Vec<i32> = (0..192)
                .map(|_| {
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    (seed as i32) >> 9
                })
                .collect();
            packets += usize::from(s.push(&block, &mut out).unwrap().is_some());
            assert_eq!(s.deficit, 0);
            fed += block.len();
        }
        assert!(
            packets <= fed / 960 && packets + 1 >= fed / 960,
            "{packets} packets for {fed}"
        );
    }

    /// §7K, `codec-core.md` §10.C, driven with a fake: an early LL packet leaves a deficit,
    /// which is zeroed at once, so there's no bitrate drop and the next packet is due a full
    /// period later. HQ keeps it (§5).
    #[test]
    fn ll_zeroes_the_deficit() {
        let row = table_row(LL, packet_budget(895)).unwrap();
        let mut s = Stream::with_core(Fake::new(LL, row), LL, row);
        s.core.early.insert(0, 600);
        let mut out = [0; 348];
        assert_eq!(s.push(&[0; 960], &mut out).unwrap(), Some(348));
        assert_eq!(s.deficit, 0);
        assert_eq!(s.core().forced, [false]);
        assert_eq!(s.core().bitrates, [265_000]);
        let mut s = px7();
        s.core.early.insert(0, 1000);
        let mut out = [0; 656];
        s.push(&[0; 1344], &mut out).unwrap();
        assert_eq!(s.deficit, 344);
    }

    /// §7J: the 25 % drop clamps at the 200 kbps stereo floor, so LL-sized rates don't go
    /// under it. HQ at 364 kbps is unaffected (273 kbps).
    #[test]
    fn deficit_drop_clamps_at_floor() {
        let row = Row {
            bitrate: 265_000,
            ..table_row(PX7, packet_budget(895)).unwrap()
        };
        let mut s = Stream::with_core(Fake::new(PX7, row), PX7, row);
        s.core.early.insert(0, 1000);
        let mut out = [0; 656];
        assert_eq!(s.push(&[0; 1344], &mut out).unwrap(), Some(656));
        assert_eq!(s.core().bitrates, [200_000]);
    }

    const LL: Config = Config {
        profile: Profile::Ll,
        ..PX7
    };

    /// §4.2: an R1 sink's LL row is index 0, 265 kbps / 10 ms / 480 samples, 348-byte packets,
    /// cfg 0.
    #[test]
    fn ll_r1_row() {
        let row = table_row(LL, packet_budget(895)).unwrap();
        assert_eq!(
            row,
            Row {
                bitrate: 265_000,
                period_ms: 10,
                samples_per_channel: 480,
                packet_bytes: 348,
                cfg: 0,
            }
        );
        assert_eq!(row.interleaved_samples(48_000, 2), 960);
    }

    /// §7J: R1 LL is the config 0 (HQ) bitstream on the LL row. 1 s of 48 kHz noise and a
    /// sine in 2 ms blocks gives 100 packets of 348 bytes, which the test decoder parses back
    /// as config 0 frames covering the input.
    #[test]
    fn ll_real_encoder_packets_decode() {
        use crate::codec::decode::{Frame, decode};
        let row = table_row(LL, packet_budget(895)).unwrap();
        let params = crate::params(LL).unwrap();
        assert_eq!(params.variant, crate::codec::Variant::Cfg0);
        let mut seed = 7u32;
        let noise: Vec<i32> = (0..96_000)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed as i32) >> 10
            })
            .collect();
        for pcm in [noise, read_pcm("sine1k.s32le")] {
            let mut s = Stream::with_core(Encoder::new(LL, row.bitrate).unwrap(), LL, row);
            let mut out = [0; 348];
            let mut packets = Vec::new();
            for block in pcm.chunks(192) {
                if let Some(len) = s.push(block, &mut out).unwrap() {
                    assert_eq!(len, 348);
                    let mut p = [0; 348];
                    frame::swap16(&out, &mut p);
                    // Frame type 1 (config 0), 0x80 if padded; Lp/4 − 3 = 84.
                    assert_eq!((p[0], p[1] & 0x7F, p[2]), (0, 1, 84));
                    packets.extend_from_slice(&p);
                }
            }
            assert_eq!(packets.len(), 100 * 348);
            let frames = decode(params, &packets, 348);
            let blocks: usize = frames.iter().map(Frame::blocks).sum();
            // 4 subbands × 2 channels per block: 8 interleaved samples. The last packet's
            // carry isn't out yet.
            let all = 96_000 / 8;
            assert!(blocks <= all && all - blocks < 128, "{blocks} of {all}");
            assert!(frames.iter().all(|f| f.bits.len() == 8));
        }
    }

    fn packet(s: &mut Stream<Fake>) -> (usize, u8, usize) {
        let (level, p) = (s.level(), s.samples_per_packet());
        let mut out = [0; 656];
        let n = s.push(&vec![0; p], &mut out).unwrap().unwrap();
        (n, level, p)
    }

    /// §7L requests: 0 and > 7 ignored, 1 → 2, the last one before a packet wins, applied
    /// after the next packet with its bitrate set before the per-packet step.
    #[test]
    fn level_request_semantics() {
        let mut s = px7();
        assert!(!s.request_level(0));
        assert!(!s.request_level(8));
        assert!(!s.request_level(255));
        assert_eq!(s.pending_level(), None);
        assert!(s.request_level(1));
        assert_eq!(s.pending_level(), Some(2));
        assert!(s.request_level(4));
        assert!(s.request_level(2));
        assert_eq!((s.level(), s.samples_per_packet()), (3, 1344));
        assert_eq!(packet(&mut s), (656, 3, 1344));
        assert_eq!(
            (s.level(), s.samples_per_packet(), s.pending_level()),
            (2, 1536, None)
        );
        assert_eq!(s.core().bitrates, [318_000, 318_000]);
        assert_eq!(packet(&mut s), (656, 2, 1536));
        assert_eq!(s.core().bitrates, [318_000, 318_000, 318_000]);
        assert_eq!(s.deficit, 0);
    }

    /// §7L rule 4: a row with the bitrate already set calls nothing and leaves P alone. R1 HQ
    /// levels 4–7 share a row, so 4 → 5 is that case.
    #[test]
    fn equal_bitrate_keeps_p() {
        let mut s = px7();
        s.request_level(4);
        packet(&mut s);
        assert_eq!((s.level(), s.samples_per_packet()), (4, 1152));
        let calls = s.core().bitrates.len();
        s.request_level(5);
        assert_eq!(packet(&mut s), (656, 4, 1152));
        assert_eq!((s.level(), s.samples_per_packet()), (5, 1152));
        assert_eq!(s.core().bitrates.len(), calls + 1);
        // Against a ¾-reduced bitrate the same row isn't equal: it's set again.
        let mut s = px7();
        s.core.early.extend([(0, 1000), (1, 1688)]);
        let mut out = [0; 656];
        s.push(&[0; 1344], &mut out).unwrap();
        assert_eq!(s.core().bitrates, [273_000]);
        s.request_level(3);
        assert_eq!(s.push(&[0; 1344], &mut out).unwrap(), Some(656));
        assert_eq!(s.core().bitrates, [273_000, 364_000, 364_000]);
    }

    /// §7L: the deficit carries straight over a level change.
    #[test]
    fn deficit_carries_over() {
        let mut s = px7();
        s.core.early.extend([(0, 1000), (1, 1880)]);
        s.request_level(2);
        let mut out = [0; 656];
        s.push(&[0; 1344], &mut out).unwrap();
        assert_eq!((s.deficit, s.samples_per_packet()), (344, 1536));
        assert_eq!(s.core().bitrates, [318_000, 238_500]);
        assert_eq!(s.push(&[0; 1535], &mut out).unwrap(), None);
        assert_eq!(s.push(&[0; 1], &mut out).unwrap(), Some(656));
        assert_eq!(s.deficit, 0);
    }

    /// §7L: a request waits while held (an HQ↔LL switch), and a reset puts the level back to 3
    /// and drops it.
    #[test]
    fn level_waits_for_switch_and_reset_restores_3() {
        let mut s = px7();
        s.hold_level(true);
        s.request_level(4);
        packet(&mut s);
        assert_eq!((s.level(), s.pending_level()), (3, Some(4)));
        s.hold_level(false);
        packet(&mut s);
        assert_eq!(s.level(), 4);
        s.request_level(2);
        let row = table_row(PX7, packet_budget(895)).unwrap();
        s.reset(PX7, row).unwrap();
        assert_eq!(
            (s.level(), s.pending_level(), s.samples_per_packet()),
            (3, None, 1344)
        );
    }

    /// R1 LL row 0 is the same at every level: a request is stored and applied, but the
    /// bitrate is equal so nothing is called and P stays.
    #[test]
    fn ll_r1_levels_change_nothing() {
        let ll = Config {
            profile: Profile::Ll,
            ..PX7
        };
        let mut s = fake::stream(ll, packet_budget(895)).unwrap();
        assert!(s.request_level(4));
        s.push(&[0; 960], &mut [0; 348]).unwrap();
        assert_eq!((s.level(), s.samples_per_packet()), (4, 960));
        assert_eq!(s.core().bitrates, [265_000]);
    }

    /// The other levels are fitted to the starting packet size, so a small MTU caps them the
    /// same way (§7J: same period, lower bitrate).
    #[test]
    fn level_rows_fit_the_budget() {
        let mut s = fake::stream(PX7, 600).unwrap();
        assert_eq!(s.row().packet_bytes, 600);
        let r4 = s.level_row(4).unwrap();
        assert_eq!((r4.packet_bytes, r4.period_ms), (600, 12));
        assert_eq!(r4.bitrate, 600 * 8000 / 12 - 25_000);
        s.request_level(4);
        let mut out = [0; 600];
        s.push(&[0; 1344], &mut out).unwrap();
        assert_eq!(s.samples_per_packet(), 1152);
    }

    #[test]
    fn rejects_small_output() {
        let mut s = px7();
        assert_eq!(s.push(&[0; 2], &mut [0; 655]), Err(Error::OutputTooSmall));
    }

    /// The real encoder through this loop, 48-frame blocks: every r2 vector byte for byte,
    /// after undoing the swap.
    #[test]
    fn real_encoder_gives_the_vectors() {
        for level in [DecoderLevel::R1, DecoderLevel::R2] {
            let tag = if level == DecoderLevel::R1 {
                "r1"
            } else {
                "r2"
            };
            let config = Config { level, ..PX7 };
            for name in ["silence", "sine1k", "noise"] {
                let row = table_row(PX7, 887).unwrap();
                let core = Encoder::new(config, row.bitrate).unwrap();
                let mut s = Stream::with_core(core, config, row);
                assert_eq!(s.samples_per_packet(), 1344);
                let want = fs::read(vectors().join(format!("{name}.{tag}.bin"))).unwrap();
                let mut got = Vec::new();
                let mut out = [0; 656];
                for block in read_pcm(&format!("{name}.s32le")).chunks(96) {
                    if let Some(len) = s.push(block, &mut out).unwrap() {
                        let mut packet = [0; 656];
                        frame::swap16(&out[..len], &mut packet);
                        got.extend_from_slice(&packet);
                    }
                }
                assert!(got == want, "{name}.{tag}");
            }
        }
    }
}
