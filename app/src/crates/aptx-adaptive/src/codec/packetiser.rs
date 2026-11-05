//! Turns PCM into packets using `codec-core.md` §8.1–§8.8.
//!
//! Like the reference, it never drops frames or bounds the carry (§8.8). Packets too small for
//! the bitrate show up as input left unconsumed; the caller picks the bitrate from the packet
//! size so that doesn't happen.

use super::alloc;
use super::pack::{HEADER_BYTES, Packer};
use super::{Engine, MAX_WIDTH, Params};

pub const MAX_FRAME_BLOCKS: usize = 64;

#[derive(Debug, Clone, Copy)]
struct Rate {
    nominal: u32,
    target: u32,
    a: u32,
    ap: u32,
    bq: u32,
    bp: u32,
}

impl Rate {
    fn new(b: u32) -> Self {
        Self {
            nominal: b,
            target: b,
            a: b,
            ap: b,
            bq: b,
            bp: b,
        }
    }
}

#[derive(Debug)]
pub struct Packetiser {
    engine: Engine,
    packer: Packer,
    frame: Vec<i32>,
    codes: Vec<i32>,
    pad_hist: i64,
    pad1: i64,
    pad2: i64,
    pending_pad: Option<i64>,
    ov_hist: i64,
    ov_prev: i64,
    p_est: u32,
    rate: Rate,
    carry_in: usize,
    packet_blocks: usize,
    #[cfg(test)]
    sent_frames: Vec<(Vec<u8>, Vec<i32>)>,
}

impl Packetiser {
    pub fn new(params: Params, packet_len: usize, bitrate: u32) -> Self {
        let (lo, hi) = alloc::bitrate_range(params);
        let bitrate = bitrate.clamp(lo, hi);
        // Sized for the widest variant, so `reset` to any variant never allocates.
        Self {
            engine: Engine::new(params),
            packer: Packer::new(params, packet_len),
            frame: Vec::with_capacity(MAX_FRAME_BLOCKS * MAX_WIDTH),
            codes: Vec::with_capacity(MAX_FRAME_BLOCKS * MAX_WIDTH),
            pad_hist: 0,
            pad1: 0,
            pad2: 0,
            pending_pad: None,
            ov_hist: 0,
            ov_prev: 0,
            p_est: alloc::pool(params, bitrate),
            rate: Rate::new(bitrate),
            carry_in: 0,
            packet_blocks: 0,
            #[cfg(test)]
            sent_frames: Vec::new(),
        }
    }

    pub fn reset(&mut self, params: Params, packet_len: usize, bitrate: u32) {
        let (lo, hi) = alloc::bitrate_range(params);
        let bitrate = bitrate.clamp(lo, hi);
        self.engine = Engine::new(params);
        self.packer.reset(params, packet_len);
        self.frame.clear();
        self.codes.clear();
        self.pad_hist = 0;
        self.pad1 = 0;
        self.pad2 = 0;
        self.pending_pad = None;
        self.ov_hist = 0;
        self.ov_prev = 0;
        self.p_est = alloc::pool(params, bitrate);
        self.rate = Rate::new(bitrate);
        self.carry_in = 0;
        self.packet_blocks = 0;
        #[cfg(test)]
        self.sent_frames.clear();
    }

    pub fn packet_len(&self) -> usize {
        self.packer.packet_len()
    }

    fn params(&self) -> Params {
        self.engine.params()
    }

    pub fn set_bitrate(&mut self, bitrate: u32) {
        let (lo, hi) = alloc::bitrate_range(self.params());
        self.rate = Rate::new(bitrate.clamp(lo, hi));
    }

    fn frame_blocks(&self) -> usize {
        self.frame.len() / self.params().width()
    }

    fn cap(&self) -> i64 {
        (self.packet_len() - HEADER_BYTES) as i64
    }

    fn frame_size(&self) -> (usize, bool) {
        let (space, hold) = if self.packer.is_open() {
            (
                self.cap() - (self.packer.bit_len() / 8) as i64,
                self.ov_hist,
            )
        } else {
            let carry = self.packer.carry_len() as i64;
            (self.cap() - carry, carry + self.ov_hist)
        };
        let v = space - hold + self.pad_hist.min(30);
        if v < 0 {
            return (8, true);
        }
        let mut x = 8 * v;
        let Rate {
            nominal, target, ..
        } = self.rate;
        if target > nominal {
            let k = i64::from(nominal / (target - nominal));
            if k != 0 {
                x += x / k;
            }
        }
        match x / i64::from(self.p_est) {
            48.. => (64, false),
            ..=7 => (8, true),
            8..=15 => (16, true),
            16..=31 => (32, true),
            _ => (48, true),
        }
    }

    fn code_frame(&mut self) {
        let params = self.params();
        let w = params.width();
        let blocks = self.frame_blocks();
        let mut bits = [0u8; MAX_WIDTH];
        let bits = &mut bits[..w];
        self.p_est = alloc::allocate(
            params,
            self.rate.target,
            self.rate.nominal,
            &self.frame,
            bits,
        );
        self.codes.resize(self.frame.len(), 0);
        for (xb, qb) in self
            .frame
            .chunks_exact(w)
            .zip(self.codes.chunks_exact_mut(w))
        {
            self.engine.code(xb, bits, qb);
        }
        let first = !self.packer.is_open();
        if first {
            self.carry_in = self.packer.carry_len();
            self.packer.begin();
        }
        self.packer.push_frame(bits, &self.codes);
        #[cfg(test)]
        self.sent_frames.push((bits.to_vec(), self.codes.clone()));
        self.frame.clear();
        self.packet_blocks += blocks;
        if first && let Some(pad) = self.pending_pad.take() {
            self.pad2 = self.pad1;
            self.pad1 = pad;
            self.pad_hist = (410 * (self.pad_hist + self.pad2) + 205 * self.pad1) >> 10;
        }
    }

    fn close(&mut self, out: &mut [u8]) -> usize {
        let written = self.packer.bit_len() / 8;
        let pad = self.packer.finish(out);
        let carry = self.packer.carry_len() as i64;
        self.ov_hist = (410 * (self.ov_hist + self.ov_prev) + 205 * carry) >> 10;
        self.ov_prev = carry;
        self.pending_pad = Some(pad as i64);
        let w = (written + HEADER_BYTES - self.carry_in) as u32;
        self.rate_check(w, pad as u32);
        self.carry_in = 0;
        self.packet_blocks = 0;
        self.packet_len()
    }

    /// §8.1a, in unsigned 32-bit arithmetic, with `bytes` the packet's own bytes (`W`).
    fn rate_check(&mut self, bytes: u32, pad: u32) {
        let blocks = self.packet_blocks as u32;
        if blocks == 0 {
            return;
        }
        let params = self.params();
        let per_s = 2 * params.sample_rate / (params.subbands() as u32 * blocks);
        let pps = (per_s & 1) + (per_s >> 1);
        let smooth = |cur: u32, prev: u32, bytes: u32| {
            (410u32.wrapping_mul(prev.wrapping_add(cur)))
                .wrapping_add(1640u32.wrapping_mul(bytes).wrapping_mul(pps))
                >> 10
        };
        let rate = &mut self.rate;
        let a_new = smooth(rate.a, rate.ap, bytes + pad);
        (rate.ap, rate.a) = (rate.a, a_new);
        let b_new = smooth(rate.bq, rate.bp, bytes);
        (rate.bp, rate.bq) = (rate.bq, b_new);
        let measured = if a_new > rate.nominal { a_new } else { b_new };
        let v = rate
            .nominal
            .wrapping_sub(measured)
            .wrapping_add(rate.target);
        rate.target = if rate.nominal > v {
            rate.nominal
        } else if params.subbands() == 8 {
            v.min((5 * rate.nominal) >> 2)
        } else {
            v
        };
    }

    pub fn encode(&mut self, pcm: &[i32], out: &mut [u8]) -> (usize, usize) {
        let w = self.params().width();
        let mut pos = 0;
        loop {
            let (req, close) = self.frame_size();
            let have = self.frame_blocks();
            if have < req {
                let take = (req - have).min((pcm.len() - pos) / w) & !1;
                let mut x = [0; MAX_WIDTH];
                for block in pcm[pos..pos + take * w].chunks_exact(w) {
                    self.engine.analyse(block, &mut x);
                    self.frame.extend_from_slice(&x[..w]);
                }
                pos += take * w;
                if have + take < req {
                    return (pos, 0);
                }
            }
            self.code_frame();
            if close || (self.packer.bit_len() / 8) as i64 >= self.cap() {
                return (pos, self.close(out));
            }
        }
    }

    pub fn flush(&mut self, out: &mut [u8]) -> usize {
        if self.frame.is_empty() && !self.packer.is_open() {
            return 0;
        }
        if !self.frame.is_empty() {
            self.code_frame();
        }
        self.close(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::Variant;
    use crate::codec::decode::{Frame, decode};

    const CFG0: Params = Params {
        variant: Variant::Cfg0,
        sample_rate: 48_000,
    };

    fn noise(frames: usize) -> Vec<i32> {
        let mut s = 1u32;
        (0..frames * 2)
            .map(|_| {
                s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (s as i32) >> 8
            })
            .collect()
    }

    fn sine(frames: usize, rate: f64) -> Vec<i32> {
        (0..frames)
            .flat_map(|n| {
                let t = f64::from(n as u32) / rate;
                let l = 0.3 * (2.0 * std::f64::consts::PI * 1000.0 * t).sin();
                let r = 0.2 * (2.0 * std::f64::consts::PI * 3100.0 * t).sin();
                [l, r].map(|v| (v * 8_388_607.0) as i32)
            })
            .collect()
    }

    fn run(p: &mut Packetiser, period: usize, pcm: &[i32]) -> Vec<u8> {
        let len = p.packet_len();
        let mut out = vec![0; len];
        let mut packets = Vec::new();
        let mut buf = Vec::new();
        for chunk in pcm.chunks_exact(period) {
            buf.extend_from_slice(chunk);
            let (consumed, written) = p.encode(&buf, &mut out);
            buf.drain(..consumed);
            if written == 0 {
                p.flush(&mut out);
            }
            packets.extend_from_slice(&out);
        }
        packets
    }

    fn parses(params: Params, len: usize, bitrate: u32, period: usize, pcm: &[i32]) {
        let mut p = Packetiser::new(params, len, bitrate);
        let packets = run(&mut p, period, pcm);
        let frames = decode(params, &packets, len);
        let blocks: usize = frames.iter().map(Frame::blocks).sum();
        let all = pcm.len() / params.width();
        // The last packet's carry isn't out yet, and an odd block may wait.
        assert!(all - blocks < 2 * MAX_FRAME_BLOCKS, "{blocks} of {all}");
        for f in &frames {
            assert!(f.bits.iter().all(|&b| b != 1), "{:?}", f.bits);
        }
    }

    #[test]
    fn packets_parse_back() {
        for pcm in [sine(48_000, 48_000.0), noise(48_000), vec![0; 96_000]] {
            parses(CFG0, 656, 364_000, 1344, &pcm);
        }
        let cfg1 = Params {
            variant: Variant::Cfg1,
            ..CFG0
        };
        parses(cfg1, 348, 265_000, 960, &noise(24_000));
        let cfg3 = Params {
            variant: Variant::Cfg3,
            ..CFG0
        };
        parses(cfg3, 656, 270_000, 1824, &sine(24_000, 48_000.0));
        let cfg3_96k = Params {
            variant: Variant::Cfg3,
            sample_rate: 96_000,
        };
        parses(cfg3_96k, 656, 340_000, 2880, &noise(48_000));
    }

    /// Config 4 (§10.F) round trip, including allocations and codes.
    #[test]
    fn config_4_round_trip() {
        for sample_rate in [44_100, 48_000] {
            let cfg4 = Params {
                variant: Variant::Cfg4,
                sample_rate,
            };
            let rate = f64::from(sample_rate);
            let mut pcm = sine(12_000, rate);
            pcm.extend(vec![0; 12_000]);
            pcm.extend(noise(12_000));
            for (len, bitrate) in [(348, 270_000), (560, 320_000)] {
                parses(cfg4, len, bitrate, 960, &pcm);
            }

            let mut p = Packetiser::new(cfg4, 348, 270_000);
            let packets = run(&mut p, 960, &pcm);
            let frames = decode(cfg4, &packets, 348);
            let sent = &p.sent_frames;
            assert!(frames.len() > 10 && frames.len() <= sent.len());
            // No digital black on config 4, so even silence keeps band 0 of each channel.
            assert!(sent.iter().all(|(b, _)| b[0] >= 2 && b[8] >= 2));
            for (f, (bits, codes)) in frames.iter().zip(sent) {
                assert_eq!(&f.bits, bits);
                let w = cfg4.width();
                let want: Vec<i32> = codes
                    .iter()
                    .enumerate()
                    .map(|(i, &q)| crate::codec::decode::coded(q, bits[i % w]))
                    .collect();
                assert_eq!(f.codes, want[..f.codes.len()]);
            }
        }
    }

    /// §8.8: undersized packets leave input unconsumed without dropping frames.
    #[test]
    fn small_packets_leave_input_unconsumed() {
        for len in [64, 128, 200] {
            let mut p = Packetiser::new(CFG0, len, 364_000);
            let mut out = vec![0; len];
            let (mut buf, mut max_carry, mut backlog) = (Vec::new(), 0, Vec::new());
            for chunk in noise(48_000).as_chunks::<1344>().0 {
                buf.extend_from_slice(chunk);
                let (consumed, written) = p.encode(&buf, &mut out);
                buf.drain(..consumed);
                if written == 0 {
                    p.flush(&mut out);
                }
                max_carry = max_carry.max(p.packer.carry_len());
                backlog.push(buf.len());
            }
            assert!(max_carry <= 128, "{len}: carry {max_carry}");
            assert!(backlog.windows(2).all(|b| b[1] >= b[0]), "{len}");
            assert!(backlog[backlog.len() - 1] > 10 * 1344, "{len}: {backlog:?}");
        }
    }

    /// §8.1a check value from the spec side: config 0 48k noise stage run, A = 349063 and
    /// B = 347103 after packet 3 (zero-based).
    #[test]
    fn rate_check_value() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/vectors/stages/cfg0_48k_noise/pcm_in.s32");
        let bytes = std::fs::read(dir).unwrap();
        let pcm: Vec<i32> = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| i32::from_le_bytes(*b))
            .collect();
        let mut p = Packetiser::new(CFG0, 656, 364_000);
        let mut out = [0; 656];
        let mut pos = 0;
        let mut ab = Vec::new();
        while ab.len() < 5 {
            let (consumed, written) = p.encode(&pcm[pos..], &mut out);
            pos += consumed;
            assert!(written > 0);
            ab.push((p.rate.a, p.rate.bq));
        }
        // Packet 3 is the first padded one, so A and B part there.
        assert_eq!(ab[3], (349_063, 347_103), "{ab:?}");
        assert!(ab[..3].iter().all(|(a, b)| a == b));
    }

    #[test]
    fn buffers_never_grow() {
        let cfg3 = Params {
            variant: Variant::Cfg3,
            ..CFG0
        };
        let cfg4 = Params {
            variant: Variant::Cfg4,
            ..CFG0
        };
        for (params, len, bitrate) in [
            (CFG0, 656, 364_000),
            (CFG0, 64, 364_000),
            (cfg3, 128, 270_000),
            (cfg4, 348, 270_000),
            (cfg4, 64, 600_000),
        ] {
            let mut p = Packetiser::new(params, len, bitrate);
            let caps = |p: &Packetiser| {
                (
                    p.frame.capacity(),
                    p.codes.capacity(),
                    p.packer.capacities(),
                )
            };
            let before = caps(&p);
            run(&mut p, 1344, &noise(48_000));
            run(&mut p, 1344, &sine(48_000, 48_000.0));
            assert_eq!(caps(&p), before);
        }
    }

    /// 48- and 672-frame chunks give the same output at 48 kHz. Other chunkings can differ
    /// (`codec-core.md` §10.B; `c0_48k_L3_chunk13` in the parity vectors).
    #[test]
    fn some_chunkings_agree() {
        let pcm = noise(9600);
        let encode = |step: usize| {
            let mut p = Packetiser::new(CFG0, 656, 364_000);
            let mut out = vec![0; 656];
            let (mut packets, mut buf, mut consumed) = (Vec::new(), Vec::new(), 0);
            for chunk in pcm.chunks(step) {
                buf.extend_from_slice(chunk);
                let (c, w) = p.encode(&buf, &mut out);
                buf.drain(..c);
                consumed += c;
                let w = if w == 0 && consumed >= 1344 {
                    p.flush(&mut out)
                } else {
                    w
                };
                if w > 0 {
                    packets.extend_from_slice(&out);
                    consumed = 0;
                }
            }
            packets
        };
        let whole = encode(1344);
        assert_eq!(whole.len(), 14 * 656);
        assert!(encode(96).iter().eq(whole.iter()));
    }

    #[test]
    fn reconstruction_tracks_signal() {
        let w = CFG0.width();
        let pcm = sine(9600, 48_000.0);
        let mut engine = Engine::new(CFG0);
        let mut x = vec![0; pcm.len() / 2];
        for (p, xb) in pcm.chunks_exact(w).zip(x.chunks_exact_mut(w)) {
            engine.analyse(p, xb);
        }
        let (mut sig, mut err) = (0f64, 0f64);
        let mut pred = vec![0; w];
        for frame in x.chunks_exact(64 * w) {
            let mut bits = [0; 8];
            alloc::allocate(CFG0, 364_000, 364_000, frame, &mut bits);
            let mut q = vec![0; w];
            for xb in frame.chunks_exact(w) {
                let trace = engine.code_traced(xb, &bits, &mut q);
                for i in 0..w {
                    let want = f64::from(xb[i] >> 8);
                    let got = f64::from(pred[i] + trace.recon[i]);
                    sig += want * want;
                    err += (want - got) * (want - got);
                }
                pred.clone_from(&trace.pred_out);
            }
        }
        let snr = 10.0 * (sig / err).log10();
        assert!(snr > 30.0, "subband SNR {snr:.1} dB");
    }
}
