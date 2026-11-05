//! Non-uniform adaptive quantiser from `codec-core.md` §5.

#![expect(clippy::many_single_char_names, reason = "names follow the spec")]

use super::arith::{r23, sat24, sext24};
use super::tables::{LOG_TABLE, QUANT};

const STEP_RESET: i32 = 8;
const LD_RESET: i32 = 244;

#[derive(Debug, Clone, Copy)]
pub struct Band {
    step: i32,
    ld: i32,
    pub dith: i32,
    r: i32,
}

impl Default for Band {
    fn default() -> Self {
        Self {
            step: STEP_RESET,
            ld: LD_RESET,
            dith: 0,
            r: 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Mode {
    Normal,
    LowPower { carry: i32, top: bool },
}

impl Band {
    pub fn quantise(&mut self, x: i32, p: i32, b: u8, mode: &mut Mode) -> (i32, i32) {
        if b == 0 {
            self.step = STEP_RESET;
            self.ld = LD_RESET;
            self.r = 0;
            return (0, 0);
        }
        let b = u32::from(b);
        let bt = b.min(9);
        let e = b.saturating_sub(9);
        let tab = &QUANT[bt as usize - 2];
        let (t, dt) = (tab.threshold, tab.dither);
        let step = i64::from(self.step);

        let d = x.saturating_sub(p.wrapping_mul(256));
        let a = d.wrapping_abs();
        let big_a = i64::from(a) << 11;

        let mut idx = 0usize;
        let mut h = 1usize << (bt - 2);
        while h > 0 {
            if big_a - i64::from(t[idx + h]) * step >= 0 {
                idx += h;
            }
            h >>= 1;
        }
        let (mut lo, mut hi) = (t[idx], t[idx + 1]);
        let (dl, mut dh) = (dt[idx], dt[idx + 1]);
        let mut code = idx as i32;

        let mask = (1i32 << e) - 1;
        if e > 0 {
            let mut r = 0i32;
            let (mut lp, mut hp) = (lo, hi);
            for _ in 0..e {
                (lp, hp) = (lo, hi);
                let m = lo + ((hi - lo) >> 1);
                if big_a - i64::from(m) * step < 0 {
                    code *= 2;
                    r *= 2;
                    hi = m;
                } else {
                    code = 2 * code + 1;
                    r = 2 * r + 1;
                    lo = m;
                }
            }
            (lo, hi) = (lp, hp);
            dh -= mask.wrapping_mul(-1 - r).wrapping_mul(dh - dl) >> e;
        }

        let v = match mode {
            Mode::Normal => {
                let dith = i64::from(self.dith);
                let g = i64::from(sat24(i64::from(dl) - i64::from(dh)));
                let ee = i64::from(r23(dith * dith));
                let off = i64::from(r23(((1 << 21) - ee) * g));
                let thr = i64::from(sat24(off + i64::from(hi) + i64::from(lo)));
                let spread = dith * (i64::from(hi) - i64::from(lo));
                r23((thr << 23) + if d >= 0 { spread } else { -spread })
            }
            Mode::LowPower { carry, .. } => {
                sat24(i64::from(*carry) + i64::from(hi) + i64::from(lo))
            }
        };

        if (i64::from(a) << 12) - i64::from(v) * step < 0 {
            code -= 1;
        }
        let q = if d < 0 { !code } else { code };

        let c = q >> e;
        let j = if c >= 0 { c + 1 } else { -c } as usize;
        let mut rho = t[j];
        if e > 0 {
            let sigma = if q < 0 { -1 } else { 0 };
            let m9 = mask & (sigma ^ q);
            let m6 = mask & !m9;
            rho -= (t[j] - t[j - 1]).wrapping_mul(m6) >> e;
        }
        if c < 0 {
            rho = -rho;
        }
        if matches!(mode, Mode::Normal) && b <= 9 {
            rho = r23(i64::from(dt[j]) * i64::from(self.dith) + (i64::from(rho) << 23));
        }
        let new_r = sat24(i64::from(
            ((i64::from(self.step) * i64::from(rho)) >> 19) as i32,
        ));

        let incr = i64::from(tab.incr[j]);
        let ld_max = 6399 - 256 * b as i32;
        self.ld = r23(8_350_720 * i64::from(self.ld) + (incr << 23)).clamp(0, ld_max);
        self.step = LOG_TABLE[((self.ld >> 3) & 31) as usize] >> (24 - b as i32 - (self.ld >> 8));

        let r = match mode {
            Mode::LowPower { carry, top } => {
                *carry = self.ld;
                if *top { self.r } else { new_r }
            }
            Mode::Normal => new_r,
        };
        self.r = r;
        (q, r)
    }
}

/// Updates a channel's dither seed from its band-0 and band-1 codes and sets each band's
/// dither for the next block (§5.5).
pub fn dither(seed: &mut i32, q0: i32, q1: i32, bands: &mut [Band]) {
    *seed = ((((4 * q0) & 0xC) | (q1 & 3)) * 256).wrapping_add(seed.wrapping_mul(16));
    let p = i64::from(*seed >> 8) * 0x009E_3776;
    let pl = u64::from(p as u32);
    let v = ((pl >> 22) & 3) | ((((p as u64) >> 24).wrapping_add(pl) & 0x3F_FFFF) << 2);
    let shift = if bands.len() == 8 { 2 } else { 5 };
    for (k, band) in bands.iter_mut().enumerate() {
        band.dith = sext24(((v << (shift * k)) & 0xFF_FFFF) as i32);
    }
}
