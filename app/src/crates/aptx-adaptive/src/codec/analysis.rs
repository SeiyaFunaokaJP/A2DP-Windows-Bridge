#![expect(clippy::many_single_char_names, reason = "names follow the spec")]

use super::arith::{add, r23, r64, sat24, sat64, sub};
use super::tables::{DCT, FRONTEND, INNER_12, INNER_16, OUTER_12, OUTER_16, PROTO_128, PROTO_256};

#[derive(Debug, Clone, Copy)]
struct Qmf2 {
    coef: &'static [i32],
    a: [i32; 16],
    b: [i32; 16],
}

impl Qmf2 {
    fn new(coef: &'static [i32]) -> Self {
        Self {
            coef,
            a: [0; 16],
            b: [0; 16],
        }
    }

    fn run(&mut self, x: i32, y: i32) -> (i32, i32) {
        let n = self.coef.len();
        self.a.copy_within(0..n - 1, 1);
        self.b.copy_within(0..n - 1, 1);
        self.a[0] = x;
        self.b[0] = y;
        let mut p = 0i64;
        let mut q = 0i64;
        for (k, &c) in self.coef.iter().enumerate() {
            p += i64::from(c) * i64::from(self.a[k]);
            q += i64::from(c) * i64::from(self.b[n - 1 - k]);
        }
        let (p, q) = (i64::from(r23(p)), i64::from(r23(q)));
        (sat24(p + q), sat24(q - p))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Tree4 {
    outer: Qmf2,
    low: Qmf2,
    high: Qmf2,
}

impl Tree4 {
    pub fn new(taps12: bool) -> Self {
        let (outer, inner): (&'static [i32], &'static [i32]) = if taps12 {
            (&OUTER_12, &INNER_12)
        } else {
            (&OUTER_16, &INNER_16)
        };
        Self {
            outer: Qmf2::new(outer),
            low: Qmf2::new(inner),
            high: Qmf2::new(inner),
        }
    }

    pub fn block(&mut self, s: &[i32], out: &mut [i32]) {
        let (sum0, diff0) = self.outer.run(s[0], s[1]);
        let (sum1, diff1) = self.outer.run(s[2], s[3]);
        let (b0, b1) = self.low.run(sum0, sum1);
        let (b3, b2) = self.high.run(diff0, diff1);
        for (o, b) in out.iter_mut().zip([b0, b1, b2, b3]) {
            *o = b << 8;
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Cosine8 {
    proto: &'static [i32],
    hist: [i32; 256],
}

fn dct_mul(v: i32, k: i32) -> i32 {
    r64(i128::from((2 * i64::from(v)).wrapping_mul(i64::from(k))))
}

impl Cosine8 {
    pub fn new(taps128: bool) -> Self {
        Self {
            proto: if taps128 { &PROTO_128 } else { &PROTO_256 },
            hist: [0; 256],
        }
    }

    pub fn block(&mut self, s: &[i32], out: &mut [i32]) {
        let taps = self.proto.len();
        self.hist.copy_within(0..taps - 8, 8);
        for m in 0..8 {
            self.hist[m] = s[7 - m];
        }

        let mut u = [0i32; 16];
        for (i, u) in u.iter_mut().enumerate() {
            let mut acc = 0i64;
            for m in (i..taps).step_by(16) {
                acc = acc
                    .wrapping_sub(i64::from(self.proto[m]).wrapping_mul(i64::from(self.hist[m])));
            }
            *u = r64(i128::from(acc.wrapping_mul(64)));
        }

        let mut o = [0i32; 8];
        for g in &FRONTEND {
            let pos = add(u[g.add[0]], u[g.add[1]]);
            let neg = sub(u[g.sub[0]], u[g.sub[1]]);
            let sum = add(pos, neg);
            let diff = sub(pos, neg);
            o[g.diff_out] = r64(i128::from(sat64(
                2 * i128::from(diff) * i128::from(g.diff_const),
            )));
            o[g.sum_out] = r64(i128::from(sat64(
                2 * i128::from(sum) * i128::from(g.sum_const),
            )));
        }

        let [k1, k2, k3, k4, k5, k6, k7] = DCT;
        let [x0, x1, x2, x3, x4, x5, x6, x7] = o;
        let s04 = add(x0, x4);
        let s26 = add(x2, x6);
        let s15 = add(x1, x5);
        let s37 = add(x3, x7);
        let a = dct_mul(sub(x0, x4), k1);
        let b = dct_mul(sub(x1, x5), k2);
        let c = dct_mul(sub(x3, x7), k3);
        let d = dct_mul(sub(x2, x6), k4);
        let ad = add(a, d);
        let bc = add(b, c);
        let s0 = add(s04, s26);
        let s1 = add(s15, s37);
        let y0 = add(s0, s1);
        let y1 = add(ad, bc);
        let g = dct_mul(sub(s04, s26), k5);
        let h = dct_mul(sub(s15, s37), k6);
        let y2 = add(g, h);
        let y4 = dct_mul(sub(s0, s1), k7);
        let e = dct_mul(sub(a, d), k5);
        let f = dct_mul(sub(b, c), k6);
        let ef = add(e, f);
        let y3 = add(ef, sub(ef, y1));
        let i = dct_mul(sub(ad, bc), k7);
        let y5 = add(i, sub(i, y3));
        let l = dct_mul(sub(g, h), k7);
        let y6 = add(l, sub(l, y2));
        let j = dct_mul(sub(e, f), k7);
        let t = add(j, sub(j, ef));
        let y7 = add(t, sub(t, y5));

        let mut y = [y0, y1, y2, y3, y4, y5, y6, y7];
        y[0] = r64(i128::from(i64::from(y[0]) << 31));
        for k in 1..8 {
            y[k] = sub(y[k], y[k - 1]);
        }
        out[..8].copy_from_slice(&y);
    }
}

#[derive(Debug, Clone, Copy)]
#[expect(
    clippy::large_enum_variant,
    reason = "kept inline so the engine copies without allocating"
)]
pub enum Analysis {
    Tree4(Tree4),
    Cosine8(Cosine8),
}

impl Analysis {
    pub fn block(&mut self, s: &[i32], out: &mut [i32]) {
        match self {
            Self::Tree4(f) => f.block(s, out),
            Self::Cosine8(f) => f.block(s, out),
        }
    }
}
