use super::bits::BitWriter;
use super::tables::{
    ABS_A, ABS_B, CODEWORD, DIFF, DIFF_INDEX, DIFF_RANGE, PAIR_TABLE_4BAND, PAIR_TABLE_8BAND,
};
use super::{MAX_WIDTH, Params};

pub const HEADER_BYTES: usize = 3;
const MAX_FRAME_BYTES: usize = 32 * 16 * 56 / 8 + 64;
pub const MAX_PACKET: usize = 4 * (255 + 3);

fn reduced(b: u8) -> u32 {
    u32::from(if b > 1 { b - 1 } else { b })
}

fn write_absolute(w: &mut BitWriter, params: Params, bits: &[u8], blocks: usize) {
    w.put(params.alloc_header(), 4);
    let eight = params.subbands() == 8;
    let pair_b = if eight {
        &PAIR_TABLE_8BAND
    } else {
        &PAIR_TABLE_4BAND
    };
    let escapes = params.escapes();
    for (m, pair) in bits.as_chunks::<2>().0.iter().enumerate() {
        let table = if pair_b[m] { &ABS_B } else { &ABS_A };
        let (a, b) = (reduced(pair[0]), reduced(pair[1]));
        if escapes {
            w.code(table[((a.min(8) << 4) | b.min(8)) as usize]);
            for v in [a, b] {
                if v > 7 {
                    w.put(v - 8, 3);
                }
            }
        } else {
            debug_assert!(a <= 8 && b <= 8, "allocation over 9 bits without escapes");
            w.code(table[((a << 4) | b) as usize]);
        }
    }
    if escapes {
        zero_escape(w, bits, blocks);
    }
}

fn zero_escape(w: &mut BitWriter, bits: &[u8], blocks: usize) {
    if bits.iter().all(|&b| b == 0) {
        w.put(blocks as u32 - 1, 8);
    }
}

fn write_diff(w: &mut BitWriter, params: Params, prev: &[u8], bits: &[u8], blocks: usize) {
    let escapes = params.escapes();
    for (&b, &p) in bits.iter().zip(prev) {
        let (v, pv) = (reduced(b) as i32, reduced(p) as usize);
        let table = &DIFF[DIFF_INDEX[pv]];
        if escapes {
            let (lo, hi) = DIFF_RANGE[pv];
            let cl = v.clamp(lo, hi);
            w.code(table[((cl - pv as i32) & 15) as usize]);
            if v <= lo {
                w.put((lo - v) as u32, 3);
            } else if v >= hi {
                w.put((v - hi) as u32, 3);
            }
        } else {
            w.code(table[((v - pv as i32) & 15) as usize]);
        }
    }
    if escapes {
        zero_escape(w, bits, blocks);
    }
}

fn write_codewords(w: &mut BitWriter, params: Params, bits: &[u8], codes: &[i32]) {
    let width = params.width();
    for pair in codes.chunks_exact(2 * width) {
        let (b0, b1) = pair.split_at(width);
        for (i, &b) in bits.iter().enumerate() {
            if b == 0 {
                continue;
            }
            let e = &CODEWORD[usize::from(top_band_entry(params, i, b))];
            let (q0, q1) = (b0[i], b1[i]);
            let h0 = (q0 >> e.raw_lsbs) & e.msb_mask;
            let h1 = (q1 >> e.raw_lsbs) & e.msb_mask;
            w.code(e.table[((h0 << e.pair_shift) | h1) as usize]);
            if e.raw_lsbs > 0 {
                let m = (1u32 << e.raw_lsbs) - 1;
                w.put(q0 as u32 & m, e.raw_lsbs);
                w.put(q1 as u32 & m, e.raw_lsbs);
            }
        }
    }
}

fn top_band_entry(params: Params, i: usize, b: u8) -> u8 {
    if params.subbands() == 4 && b == 2 && i % 4 == 3 {
        1
    } else {
        b
    }
}

#[derive(Debug, Clone)]
pub struct Packer {
    params: Params,
    packet_len: usize,
    carry: Vec<u8>,
    payload: BitWriter,
    prev: Option<[u8; MAX_WIDTH]>,
    open: bool,
}

impl Packer {
    /// # Panics
    /// If `packet_len` is outside 12..=[`MAX_PACKET`] or not a multiple of 4; the encoder checks
    /// first.
    pub fn new(params: Params, packet_len: usize) -> Self {
        assert!(
            (12..=MAX_PACKET).contains(&packet_len) && packet_len.is_multiple_of(4),
            "packet size {packet_len}"
        );
        Self {
            params,
            packet_len,
            carry: Vec::with_capacity(MAX_PACKET + MAX_FRAME_BYTES),
            payload: BitWriter::with_capacity(2 * MAX_PACKET + MAX_FRAME_BYTES),
            prev: None,
            open: false,
        }
    }

    /// Back to a fresh packer for `params` and `packet_len`, keeping the buffers.
    ///
    /// # Panics
    /// As for [`Packer::new`].
    pub fn reset(&mut self, params: Params, packet_len: usize) {
        assert!(
            (12..=MAX_PACKET).contains(&packet_len) && packet_len.is_multiple_of(4),
            "packet size {packet_len}"
        );
        self.params = params;
        self.packet_len = packet_len;
        self.carry.clear();
        self.payload.clear();
        self.prev = None;
        self.open = false;
    }

    pub fn begin(&mut self) {
        self.payload.clear();
        self.payload.exp_golomb(self.carry.len() as u32);
        self.payload.bytes(&self.carry);
        self.prev = None;
        self.open = true;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn bit_len(&self) -> usize {
        self.payload.bit_len()
    }

    fn write_frame(&self, w: &mut BitWriter, bits: &[u8], codes: &[i32], blocks: usize) {
        let w_ = self.params.width();
        match &self.prev {
            None => write_absolute(w, self.params, bits, blocks),
            Some(prev) => write_diff(w, self.params, &prev[..w_], bits, blocks),
        }
        write_codewords(w, self.params, bits, codes);
    }

    pub fn push_frame(&mut self, bits: &[u8], codes: &[i32]) {
        if !self.open {
            self.begin();
        }
        let blocks = codes.len() / self.params.width();
        debug_assert!(blocks.is_multiple_of(2) && blocks > 0);
        let mut w = std::mem::take(&mut self.payload);
        self.write_frame(&mut w, bits, codes, blocks);
        self.payload = w;
        let mut prev = [0; MAX_WIDTH];
        prev[..bits.len()].copy_from_slice(bits);
        self.prev = Some(prev);
    }

    pub fn finish(&mut self, out: &mut [u8]) -> usize {
        if !self.open {
            self.begin();
        }
        let out = &mut out[..self.packet_len];
        let cap = self.packet_len - HEADER_BYTES;
        let payload = self.payload.as_bytes();
        let n = payload.len().min(cap);
        out[0] = 0;
        out[1] = self.params.frame_type();
        out[2] = u8::try_from(self.packet_len / 4 - 3).expect("checked in new");
        out[HEADER_BYTES..HEADER_BYTES + n].copy_from_slice(&payload[..n]);
        self.carry.clear();
        self.carry.extend_from_slice(&payload[n..]);
        let pad_len = out.len() - HEADER_BYTES - n;
        if pad_len > 0 {
            out[1] |= 0x80;
            let pad = &mut out[HEADER_BYTES + n..];
            pad.fill(0);
            pad[0] = 0x80;
            let t = pad.len() - 1;
            let ff = t / 255;
            let at = pad.len() - 1 - ff;
            pad[at + 1..].fill(0xFF);
            pad[at] = (t % 255) as u8;
        }
        self.open = false;
        pad_len
    }

    pub fn packet_len(&self) -> usize {
        self.packet_len
    }

    #[cfg(test)]
    pub fn capacities(&self) -> [usize; 2] {
        [self.carry.capacity(), self.payload.capacity()]
    }

    pub fn carry_len(&self) -> usize {
        self.carry.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::Variant;

    const CFG0: Params = Params {
        variant: Variant::Cfg0,
        sample_rate: 48_000,
    };

    #[test]
    fn header_byte_two_up_to_1032() {
        let mut out = [0; MAX_PACKET];
        for len in [12, 656, 1028, 1032] {
            let mut p = Packer::new(CFG0, len);
            p.finish(&mut out);
            assert_eq!(usize::from(out[2]), len / 4 - 3);
        }
        assert_eq!(out[2], 0xFF);
    }

    #[test]
    #[should_panic(expected = "packet size 1036")]
    fn rejects_1036() {
        Packer::new(CFG0, 1036);
    }
}
