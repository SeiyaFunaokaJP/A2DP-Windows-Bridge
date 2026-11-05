#![expect(clippy::many_single_char_names, reason = "names follow the spec")]

use std::fs;
use std::path::PathBuf;

use super::decode::{coded, decode};
use super::{Engine, Params, Variant};

struct Vectors {
    dir: PathBuf,
    params: Params,
    packet_len: usize,
    blocks: usize,
    frames: Vec<(usize, usize, Vec<u8>)>,
}

fn numbers(s: &str) -> Vec<i64> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars().chain([' ']) {
        if c.is_ascii_digit() || (c == '-' && cur.is_empty()) {
            cur.push(c);
        } else if !cur.is_empty() {
            out.push(cur.parse().unwrap());
            cur.clear();
        }
    }
    out
}

impl Vectors {
    fn load(name: &str, variant: Variant, sample_rate: u32) -> Self {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/vectors/stages")
            .join(name);
        let meta = fs::read_to_string(dir.join("meta.json")).unwrap();
        let m = numbers(&meta);
        let params = Params {
            variant,
            sample_rate,
        };
        assert_eq!(m[3] as usize, params.subbands());
        let width = params.width();
        let f = numbers(&fs::read_to_string(dir.join("frames.json")).unwrap());
        let frames = f
            .chunks_exact(2 + width)
            .map(|c| {
                (
                    c[0] as usize,
                    c[1] as usize,
                    c[2..].iter().map(|&b| b as u8).collect(),
                )
            })
            .collect();
        Self {
            dir,
            params,
            packet_len: m[0] as usize,
            blocks: m[2] as usize,
            frames,
        }
    }

    fn s32(&self, file: &str) -> Vec<i32> {
        let bytes = fs::read(self.dir.join(file)).unwrap();
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| i32::from_le_bytes(*b))
            .collect()
    }

    fn block_bits(&self) -> Vec<&[u8]> {
        self.frames
            .iter()
            .flat_map(|(_, n, bits)| std::iter::repeat_n(bits.as_slice(), *n))
            .collect()
    }
}

fn first_mismatch(name: &str, what: &str, width: usize, got: &[i32], want: &[i32]) {
    assert!(
        want.iter().any(|&v| v != 0),
        "{name} {what}: vector is all zero"
    );
    if let Some(i) = got.iter().zip(want).position(|(g, w)| g != w) {
        let b = i / width;
        panic!(
            "{name} {what}: block {b} band {}: got {:?}, want {:?}",
            i % width,
            &got[b * width..(b + 1) * width],
            &want[b * width..(b + 1) * width]
        );
    }
    assert_eq!(got.len(), want.len(), "{name} {what} length");
}

fn check_analysis(v: &Vectors, name: &str) {
    let w = v.params.width();
    let pcm = v.s32("pcm_in.s32");
    let want = v.s32("analysis_out.s32");
    let mut e = Engine::new(v.params);
    let mut got = vec![0; v.blocks * w];
    for (p, x) in pcm.chunks_exact(w).zip(got.chunks_exact_mut(w)) {
        e.analyse(p, x);
    }
    first_mismatch(name, "analysis_out", w, &got, &want);
}

fn check_quant_predict(v: &Vectors, name: &str) {
    let w = v.params.width();
    let x = v.s32("analysis_out.s32");
    let bits = v.block_bits();
    assert_eq!(bits.len(), v.blocks);
    let mut e = Engine::new(v.params);
    let (mut codes, mut recon, mut pred) = (Vec::new(), Vec::new(), Vec::new());
    let mut q = vec![0; w];
    for (x, b) in x.chunks_exact(w).zip(bits) {
        let t = e.code_traced(x, b, &mut q);
        codes.extend_from_slice(&t.codes);
        recon.extend_from_slice(&t.recon);
        pred.extend_from_slice(&t.pred_out);
    }
    first_mismatch(name, "quant_codes", w, &codes, &v.s32("quant_codes.s32"));
    first_mismatch(name, "quant_recon", w, &recon, &v.s32("quant_recon.s32"));
    first_mismatch(name, "pred_out", w, &pred, &v.s32("pred_out.s32"));
}

fn check_decode(v: &Vectors, name: &str) {
    let w = v.params.width();
    let codes = v.s32("quant_codes.s32");
    let packets = fs::read(v.dir.join("packets.bin")).unwrap();
    let frames = decode(v.params, &packets, v.packet_len);
    let got: Vec<_> = frames
        .iter()
        .map(|f| (f.packet, f.blocks(), f.bits.clone()))
        .collect();
    let (head, tail) = (&got[..got.len() - 1], &got[got.len() - 1]);
    let (want_head, want_tail) = v.frames.split_at(v.frames.len() - 1);
    assert_eq!(head, want_head, "{name} frames");
    let want_tail = &want_tail[0];
    assert_eq!(
        (tail.0, &tail.2),
        (want_tail.0, &want_tail.2),
        "{name} last frame"
    );
    assert!(tail.1 <= want_tail.1, "{name} last frame blocks");
    let mut block = 0;
    for f in &frames {
        let want: Vec<i32> = codes[block * w..(block + f.blocks()) * w]
            .iter()
            .enumerate()
            .map(|(i, &q)| coded(q, f.bits[i % w]))
            .collect();
        assert_eq!(f.codes, want, "{name} codes from block {block}");
        block += f.blocks();
    }
}

macro_rules! stage_tests {
    ($($name:ident: $dir:literal, $variant:expr, $rate:literal;)*) => {
        mod analysis { use super::*; $(#[test] fn $name() { check_analysis(&Vectors::load($dir, $variant, $rate), $dir); })* }
        mod quant_predict { use super::*; $(#[test] fn $name() { check_quant_predict(&Vectors::load($dir, $variant, $rate), $dir); })* }
        mod decode { use super::*; $(#[test] fn $name() { check_decode(&Vectors::load($dir, $variant, $rate), $dir); })* }
    };
}

stage_tests! {
    cfg0_noise: "cfg0_48k_noise", Variant::Cfg0, 48_000;
    cfg0_sine: "cfg0_48k_sine", Variant::Cfg0, 48_000;
    cfg1_noise: "cfg1_48k_noise", Variant::Cfg1, 48_000;
    cfg3_noise: "cfg3_48k_noise", Variant::Cfg3, 48_000;
    cfg3_sine: "cfg3_48k_sine", Variant::Cfg3, 48_000;
    cfg3_96k_noise: "cfg3_96k_noise", Variant::Cfg3, 96_000;
}

#[cfg(test)]
mod vendor_capture {
    use super::*;
    use crate::codec::decode::Frame;

    #[test]
    fn parse_vendor_capture() {
        let path = match std::env::var("VENDOR_CAPTURE") {
            Ok(p) if !p.is_empty() => p,
            _ => return,
        };
        let data = fs::read(&path).unwrap();
        let mut payloads = Vec::new();
        let mut n = 0;
        for rec in data.as_chunks::<664>().0 {
            // raw order: undo the OTA pairwise byte swap
            let mut raw = [0u8; 656];
            let p = &rec[8..664];
            for i in (0..656).step_by(2) {
                raw[i] = p[i + 1];
                raw[i + 1] = p[i];
            }
            payloads.extend_from_slice(&raw);
            n += 1;
        }
        let params = Params {
            variant: Variant::Cfg3,
            sample_rate: 48_000,
        };
        let frames = decode(params, &payloads, 656);
        println!("{n} records -> {} frames", frames.len());
        let mut per_packet = std::collections::BTreeMap::new();
        for f in &frames {
            per_packet
                .entry(f.packet)
                .or_insert_with(Vec::new)
                .push((f.blocks(), f.bits.clone()));
        }
        for (pkt, v) in &per_packet {
            let sum: usize = v.iter().map(|x| x.0).sum();
            println!(
                "packet {pkt}: blocks {sum} frames {:?}",
                v.iter().map(|x| x.0).collect::<Vec<_>>()
            );
            if *pkt < 3 {
                for (b, bits) in v {
                    println!("    frame {b} alloc {bits:?}");
                }
            }
        }
        let all: usize = frames.iter().map(Frame::blocks).sum();
        println!(
            "  total blocks {all} over {} frames in {} packets",
            frames.len(),
            per_packet.len()
        );
    }
}
