use super::Params;
use super::pack::HEADER_BYTES;
use super::tables::{
    ABS_A, ABS_B, CODEWORD, Code, DIFF, DIFF_INDEX, DIFF_RANGE, PAIR_TABLE_4BAND, PAIR_TABLE_8BAND,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub packet: usize,
    pub bits: Vec<u8>,
    pub codes: Vec<i32>,
}

impl Frame {
    pub fn blocks(&self) -> usize {
        self.codes.len() / self.bits.len()
    }
}

struct Reader {
    bits: Vec<bool>,
    pos: usize,
    ended: bool,
}

impl Reader {
    fn left(&self) -> usize {
        self.bits.len() - self.pos
    }

    fn bit(&mut self) -> bool {
        let b = self.bits.get(self.pos).copied();
        self.pos += 1;
        self.ended |= b.is_none();
        b.unwrap_or(false)
    }

    fn get(&mut self, n: u32) -> u32 {
        (0..n).fold(0, |v, _| (v << 1) | u32::from(self.bit()))
    }

    fn huffman(&mut self, table: &[Code]) -> usize {
        let mut v = 0u32;
        for len in 1..=24u8 {
            v = (v << 1) | u32::from(self.bit());
            if let Some(sym) = table.iter().position(|c| c.len == len && c.code == v) {
                return sym;
            }
            if self.ended {
                return 0;
            }
        }
        panic!("no Huffman code at bit {}", self.pos);
    }
}

fn to_bits(bytes: &[u8]) -> Vec<bool> {
    bytes
        .iter()
        .flat_map(|b| (0..8).rev().map(move |i| (b >> i) & 1 == 1))
        .collect()
}

fn unreduce(v: u32) -> u8 {
    if v > 0 { v as u8 + 1 } else { 0 }
}

fn payload(packet: &[u8]) -> &[u8] {
    let body = &packet[HEADER_BYTES..];
    if packet[1] & 0x80 == 0 {
        return body;
    }
    let ff = body.iter().rev().take_while(|&&b| b == 0xFF).count();
    let t = 255 * ff + usize::from(body[body.len() - 1 - ff]);
    let start = body.len() - (t + 1);
    assert_eq!(body[start], if t == 0 { 0 } else { 0x80 }, "pad marker");
    &body[..start]
}

fn split(params: Params, packets: &[u8], packet_len: usize) -> Vec<(Vec<bool>, usize)> {
    let mut out: Vec<(Vec<bool>, usize)> = Vec::new();
    for p in packets.chunks_exact(packet_len) {
        assert_eq!(p[0], 0);
        assert_eq!(p[1] & 0x7F, params.frame_type());
        assert_eq!(usize::from(p[2]), packet_len / 4 - 3);
        let mut r = Reader {
            bits: to_bits(payload(p)),
            pos: 0,
            ended: false,
        };
        let mut zeros = 0;
        while !r.bits[r.pos] {
            zeros += 1;
            r.pos += 1;
        }
        let carried = r.get(zeros + 1) as usize - 1;
        let carry = r.bits[r.pos..r.pos + 8 * carried].to_vec();
        r.pos += 8 * carried;
        if let Some(prev) = out.last_mut() {
            prev.0.extend(carry);
        } else {
            assert_eq!(carried, 0);
        }
        out.push((r.bits[r.pos..].to_vec(), carried));
    }
    out
}

fn read_alloc(r: &mut Reader, params: Params, prev: Option<&[u8]>) -> Vec<u8> {
    let w = params.width();
    let eight = params.subbands() == 8;
    let escapes = params.escapes();
    let mut bits = Vec::with_capacity(w);
    match prev {
        None => {
            assert_eq!(r.get(4), params.alloc_header());
            let pair_b = if eight {
                &PAIR_TABLE_8BAND
            } else {
                &PAIR_TABLE_4BAND
            };
            for &b in &pair_b[..w / 2] {
                let sym = r.huffman(if b { &ABS_B } else { &ABS_A }) as u32;
                let (mut a, mut c) = (sym >> 4, sym & 15);
                if escapes {
                    if a == 8 {
                        a += r.get(3);
                    }
                    if c == 8 {
                        c += r.get(3);
                    }
                }
                bits.extend([unreduce(a), unreduce(c)]);
            }
        }
        Some(prev) => {
            for &p in prev {
                let pv = u32::from(if p > 1 { p - 1 } else { p }) as i32;
                let sym = r.huffman(&DIFF[DIFF_INDEX[pv as usize]]) as i32;
                let mut v = (pv + sym) & 15;
                if escapes {
                    let (lo, hi) = DIFF_RANGE[pv as usize];
                    if v > hi {
                        v -= 16;
                    }
                    if v == lo {
                        v = lo - r.get(3) as i32;
                    } else if v == hi {
                        v = hi + r.get(3) as i32;
                    }
                }
                bits.push(unreduce(v as u32));
            }
        }
    }
    bits
}

fn read_pair(r: &mut Reader, params: Params, bits: &[u8], out: &mut Vec<i32>) {
    let w = params.width();
    let base = out.len();
    out.resize(base + 2 * w, 0);
    for (i, &b) in bits.iter().enumerate() {
        if b == 0 {
            continue;
        }
        let bc = if params.subbands() == 4 && b == 2 && i % 4 == 3 {
            1
        } else {
            b
        };
        let e = &CODEWORD[usize::from(bc)];
        let sym = r.huffman(e.table) as i32;
        let (h0, h1) = (sym >> e.pair_shift, sym & ((1 << e.pair_shift) - 1));
        let (r0, r1) = (r.get(e.raw_lsbs) as i32, r.get(e.raw_lsbs) as i32);
        out[base + i] = (h0 << e.raw_lsbs) | r0;
        out[base + w + i] = (h1 << e.raw_lsbs) | r1;
    }
}

fn only_padding(r: &Reader) -> bool {
    r.left() < 8 && r.bits[r.pos..].iter().all(|&b| !b)
}

pub fn decode(params: Params, packets: &[u8], packet_len: usize) -> Vec<Frame> {
    assert_ne!(packets, [] as [u8; 0]);
    let mut frames = Vec::new();
    let split = split(params, packets, packet_len);
    let last = split.len() - 1;
    for (packet, (bits, _)) in split.into_iter().enumerate() {
        let mut r = Reader {
            bits,
            pos: 0,
            ended: false,
        };
        let mut prev: Option<Vec<u8>> = None;
        while !r.ended && !only_padding(&r) {
            let bits = read_alloc(&mut r, params, prev.as_deref());
            let escape = params.escapes() && bits.iter().all(|&b| b == 0);
            let max = if escape { r.get(8) as usize + 1 } else { 64 };
            let mut codes = Vec::new();
            while codes.len() / params.width() < max {
                if !escape && only_padding(&r) {
                    break;
                }
                read_pair(&mut r, params, &bits, &mut codes);
                if r.ended {
                    assert_eq!(packet, last, "packet {packet} data ends mid-pair");
                    codes.truncate(codes.len() - 2 * params.width());
                    break;
                }
            }
            let f = Frame {
                packet,
                bits: bits.clone(),
                codes,
            };
            assert!(
                f.blocks() <= 64 && f.blocks().is_multiple_of(2),
                "packet {packet}: frame of {} blocks",
                f.blocks()
            );
            frames.push(f);
            prev = Some(bits);
            if escape && only_padding(&r) {
                break;
            }
        }
    }
    frames
}

pub fn coded(q: i32, bits: u8) -> i32 {
    if bits == 0 { 0 } else { q & ((1 << bits) - 1) }
}
