//! Sign-sign LMS pole-zero predictor (spec `codec-core.md` §6).

#![expect(clippy::many_single_char_names, reason = "names follow the spec")]

use super::arith::{cr32, sat24, sext24};

pub const MAX_TAPS: usize = 24;

#[derive(Debug, Clone, Copy)]
pub struct Predictor {
    s0: i32,
    s1: i32,
    a1: i32,
    a2: i32,
    taps: usize,
    b: [i32; MAX_TAPS],
    hist: [i32; MAX_TAPS],
    y1: i32,
    z: i32,
    pub pr: i32,
}

impl Predictor {
    pub fn new(taps: usize) -> Self {
        assert!((1..=MAX_TAPS).contains(&taps));
        Self {
            s0: 1,
            s1: 1,
            a1: 0,
            a2: 0,
            taps,
            b: [0; MAX_TAPS],
            hist: [0; MAX_TAPS],
            y1: 0,
            z: 0,
            pr: 0,
        }
    }

    pub fn run(&mut self, x: i32) -> i32 {
        let t = i64::from(self.z) + i64::from(x);
        let (cur, p1, p2) = match t.signum() {
            -1 => (-1, -self.s0, -self.s1),
            1 => (1, self.s0, self.s1),
            _ => (1, 0, 0),
        };
        self.s1 = self.s0;
        self.s0 = cur;

        let w = cr32(
            255i32
                .wrapping_mul(self.a1)
                .wrapping_add(12_582_912i32.wrapping_mul(p1)),
            8,
        );
        let k = cr32((-p1).wrapping_mul(self.a1), 1).clamp(-(1 << 20), 1 << 20);
        let a2 = cr32(
            (16i32.wrapping_mul(k.wrapping_add(p2.wrapping_mul(1 << 19))) & !0xFF)
                .wrapping_add(254i32.wrapping_mul(self.a2)),
            8,
        )
        .clamp(-3_145_728, 3_145_728);
        let a1 = w.clamp(a2 - 3_932_160, 3_932_160 - a2);
        self.a1 = a1;
        self.a2 = a2;

        let y = sat24(i64::from(self.pr) + i64::from(x));
        let pole =
            ((i64::from(self.y1) * i64::from(a2) + i64::from(a1) * i64::from(y)) >> 21) as i32;
        self.y1 = y;

        let e: i32 = if x == 0 { 0 } else { x.signum() << 23 };
        let n = self.taps;
        for (b, &h) in self.b[..n].iter_mut().zip(&self.hist[..n]) {
            let term = if h < 0 { -e } else { e };
            *b = sext24(b.wrapping_add(cr32(term.wrapping_sub(*b), 8)));
        }

        let mut acc = i64::from(self.b[0]) * i64::from(x);
        for k in 1..n {
            acc += i64::from(self.b[k]) * i64::from(self.hist[k - 1]);
        }
        self.z = sat24(i64::from((acc >> 22) as i32));

        let rnd = (i64::from(pole) + 1) >> 1;
        self.pr = sat24(i64::from(self.z) + i64::from(sat24(rnd)));
        self.hist.copy_within(0..n - 1, 1);
        self.hist[0] = x;
        self.pr
    }
}
