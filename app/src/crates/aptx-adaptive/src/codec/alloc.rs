//! Bit allocation from `codec-core.md` §8.2–§8.6.

use super::tables::{
    MAX_BITS_4BAND_320, MAX_BITS_4BAND_370, MAX_BITS_4BAND_MAX, MAX_BITS_8BAND_96K_420,
    MAX_BITS_8BAND_96K_500, MAX_BITS_8BAND_96K_600, MAX_BITS_8BAND_96K_650, MAX_BITS_8BAND_96K_MAX,
    MAX_BITS_8BAND_250, MAX_BITS_8BAND_320, MAX_BITS_8BAND_370, MAX_BITS_8BAND_420,
    MAX_BITS_8BAND_520, MAX_BITS_8BAND_MAX, OFFSET_4BAND_DUAL_MONO, OFFSET_8BAND_DUAL_MONO,
    OFFSET_8BAND_DUAL_MONO_TONE,
};
use super::{CHANNELS, MAX_WIDTH, Params, Variant};

const NCH: u32 = CHANNELS as u32;

#[must_use]
pub fn bitrate_range(params: Params) -> (u32, u32) {
    match params.variant {
        Variant::Cfg0 | Variant::Cfg1 | Variant::Cfg4 => (100_000 * NCH, 300_000 * NCH),
        Variant::Cfg3 => (90_000, 500_000 * NCH),
    }
}

fn pool_rate(params: Params, target: u32) -> u32 {
    let (lo, hi) = match params.variant {
        Variant::Cfg0 | Variant::Cfg1 => (100_000 * NCH, 350_000 * NCH),
        Variant::Cfg3 => (90_000, 550_000 * NCH),
        Variant::Cfg4 => (100_000 * NCH, 210_000 * NCH),
    };
    target.clamp(lo, hi)
}

#[must_use]
pub fn pool(params: Params, target: u32) -> u32 {
    let r = u64::from(pool_rate(params, target));
    let (fs, s) = (u64::from(params.sample_rate), params.subbands() as u64);
    (r / fs * s + (r % fs) * s / fs) as u32
}

/// Per-band ceilings for clamped rate `r` (§8.3, config 4's tonal table from §10.F).
fn max_bits(params: Params, r: u32, tonal: bool) -> &'static [u8] {
    let pick = |ladder: &[(u32, &'static [u8])], top: &'static [u8]| {
        ladder
            .iter()
            .find(|(upto, _)| r <= *upto)
            .map_or(top, |l| l.1)
    };
    match (params.subbands(), params.sample_rate > 48_000) {
        (4, _) => pick(
            &[
                (319_999, &MAX_BITS_4BAND_320),
                (369_999, &MAX_BITS_4BAND_370),
            ],
            &MAX_BITS_4BAND_MAX,
        ),
        // Config 4 has no allocation escapes, so it must stay at 9 bits or less. §10.F's
        // tonal table is the 420 one, and its chain only reaches 420k past what a source sets
        // (target ≤ 5/4 × 320k); stop the chain there too, so 336k+ can't pick the 520 table.
        (_, false) if params.variant == Variant::Cfg4 && (tonal || r > 419_999) => {
            &MAX_BITS_8BAND_420
        }
        (_, false) if tonal => &MAX_BITS_8BAND_MAX,
        (_, true) if tonal => &MAX_BITS_8BAND_96K_MAX,
        (_, false) => pick(
            &[
                (249_999, &MAX_BITS_8BAND_250),
                (319_999, &MAX_BITS_8BAND_320),
                (369_999, &MAX_BITS_8BAND_370),
                (419_999, &MAX_BITS_8BAND_420),
                (519_999, &MAX_BITS_8BAND_520),
            ],
            &MAX_BITS_8BAND_MAX,
        ),
        (_, true) => pick(
            &[
                (419_999, &MAX_BITS_8BAND_96K_420),
                (499_999, &MAX_BITS_8BAND_96K_500),
                (599_999, &MAX_BITS_8BAND_96K_600),
                (649_999, &MAX_BITS_8BAND_96K_650),
            ],
            &MAX_BITS_8BAND_96K_MAX,
        ),
    }
}

fn scale_factor(a: u32) -> i32 {
    let j = ((u64::from(a) << 24) + (1 << 31)) >> 32;
    let j = j.min((1 << 31) - 1);
    let (mut lim, mut c) = (2u64, 0);
    while j > lim {
        lim *= 2;
        c += 1;
    }
    c
}

fn rate_idx(fs: u32) -> usize {
    [16_000, 32_000, 44_100, 48_000]
        .iter()
        .take_while(|&&r| fs > r)
        .count()
}

fn slice(p: i32, need: &[i32], maxb: &[u8], m: i32) -> i32 {
    let mut l = m + 1;
    if m <= -12 {
        return if p == 0 { l - 1 } else { l };
    }
    let (mut acc, mut prev) = (0, 0);
    let cnt = loop {
        let cnt: i32 = need
            .iter()
            .zip(maxb)
            .map(|(&n, &mb)| {
                if n == l {
                    2
                } else {
                    i32::from(n > l && i32::from(mb) + l - 1 > n)
                }
            })
            .sum();
        acc += prev;
        if cnt + acc >= p || l <= -10 {
            break cnt;
        }
        prev = cnt;
        l -= 1;
    };
    acc += cnt;
    if acc == p { l - 2 } else { l - 1 }
}

fn scale_factors(params: Params, x: &[i32], sf: &mut [i32]) -> bool {
    let (s, w) = (params.subbands(), params.width());
    let mut peak = [0u32; MAX_WIDTH];
    for block in x.chunks_exact(w) {
        for (pk, v) in peak.iter_mut().zip(block) {
            *pk = (*pk).max(v.unsigned_abs());
        }
    }
    for (f, &a) in sf.iter_mut().zip(&peak) {
        *f = scale_factor(a);
    }
    s == 8
        && sf.chunks_exact(s).any(|ch| {
            let hi = *ch.iter().max().expect("8 bands");
            let lo = *ch.iter().min().expect("8 bands");
            hi - lo > 10 && ch.iter().filter(|&&v| v < hi - 10).count() >= 6
        })
}

fn bit_need(params: Params, sf: &[i32], tonal: bool, need: &mut [i32]) {
    let w = params.width();
    let offsets: &[i8] = match (params.subbands(), tonal) {
        (4, _) => &OFFSET_4BAND_DUAL_MONO,
        (_, true) => &OFFSET_8BAND_DUAL_MONO_TONE,
        (_, false) => &OFFSET_8BAND_DUAL_MONO,
    };
    let off = &offsets[w * rate_idx(params.sample_rate)..][..w];
    for ((n, &f), &o) in need.iter_mut().zip(sf).zip(off) {
        let d = f - i32::from(o);
        *n = match (f, d) {
            (0, _) => -5,
            (_, 1..) => (d + 1) >> 1,
            _ => d,
        };
    }
}

fn distribute(s: usize, pool: i32, need: &[i32], maxb: &[i32], sl: i32, out: &mut [i32]) {
    let w = need.len();
    out.fill(0);
    out[0] = 2;
    out[s] = 2;
    let mut tot = 0;
    for i in 0..w {
        if need[i] >= sl + 2 {
            out[i] = (need[i] - sl).min(maxb[i]);
        }
        tot += out[i];
    }

    if tot < pool {
        'fill: for k in 0..s {
            for i in [k, s + k] {
                if out[i] > 1 && maxb[i] > out[i] {
                    out[i] += 1;
                    tot += 1;
                } else if need[i] == sl + 1 && pool > tot + 1 && maxb[i] > 0 {
                    out[i] = 2;
                    tot += 2;
                }
                if tot >= pool {
                    break 'fill;
                }
            }
        }
    }

    if tot < pool {
        for i in (0..w).cycle().take(32) {
            if out[i] > 1 && maxb[i] > out[i] {
                out[i] += 1;
                tot += 1;
            } else if out[i] == 0 && maxb[i] > 0 {
                out[i] = 2;
                tot += 2;
            }
            if tot >= pool {
                break;
            }
        }
    }

    if tot > pool {
        for i in (0..w).rev() {
            if out[i] > 2 {
                out[i] -= 1;
                tot -= 1;
            }
            if tot <= pool {
                break;
            }
        }
    }
}

/// The alternate flag (§10.E): config 3 stereo at a nominal bitrate under 180 kbps.
fn alternate(params: Params, nominal: u32) -> bool {
    params.variant == Variant::Cfg3 && nominal <= 179_999
}

/// Allocates one frame: `x` is its subband values, `width()` per block, before quantisation.
/// Writes `width()` allocations to `out` and returns the pool it used. `target` is the
/// rate-controlled bitrate and `nominal` the set one (§8.1a).
pub fn allocate(params: Params, target: u32, nominal: u32, x: &[i32], out: &mut [u8]) -> u32 {
    let (s, w) = (params.subbands(), params.width());
    let rate = pool_rate(params, target);
    let bits = pool(params, target);
    let alternate = alternate(params, nominal);

    let mut sf = [0i32; MAX_WIDTH];
    let tonal = scale_factors(params, x, &mut sf[..w]);
    if alternate {
        sf[s..w].fill(0);
    }
    let sf = &sf[..w];
    let mut need = [0i32; MAX_WIDTH];
    bit_need(params, sf, tonal, &mut need[..w]);
    let need = &need[..w];
    let top = need.iter().copied().max().unwrap_or(0).max(0);
    let caps = max_bits(params, rate, tonal);
    let sl = slice(bits as i32, need, caps, top);

    let mut maxb = [0i32; MAX_WIDTH];
    for (m, &c) in maxb.iter_mut().zip(caps) {
        *m = c.into();
    }
    let maxb = &maxb[..w];
    let mut alloc = [0i32; MAX_WIDTH];
    let alloc = &mut alloc[..w];
    distribute(s, bits as i32, need, maxb, sl, alloc);
    if params.digital_black() {
        digital_black(sf, maxb, s, alternate, alloc);
    }
    for (b, &v) in out.iter_mut().zip(alloc.iter()) {
        *b = v as u8;
    }
    bits
}

/// §8.6 step 5.
fn digital_black(sf: &[i32], maxb: &[i32], s: usize, alternate: bool, o: &mut [i32]) {
    let sum = |ch: usize| -> i32 {
        (ch * s..(ch + 1) * s)
            .filter(|&i| sf[i] != 0)
            .map(|i| o[i])
            .sum()
    };
    let (s0, s1) = (sum(0), sum(1));
    for (v, &f) in o.iter_mut().zip(sf) {
        if f == 0 {
            *v = 0;
        }
    }
    if s0 + s1 == 0 {
        return;
    }
    let mut total = s0 + s1;
    if !alternate {
        if s1 == 0 {
            o[s] = 2;
            total += 2;
        }
        if s0 == 0 {
            o[0] = 2;
            total += 2;
        }
    }
    // UNCERTAIN in the spec, never seen in the vectors.
    if (1..=6).contains(&total) {
        let mut extra = 7 - total;
        for pass_all in [false, true] {
            for (v, &mb) in o.iter_mut().zip(maxb) {
                if extra <= 0 {
                    return;
                }
                if pass_all || *v > 0 {
                    let add = extra.min(mb - *v).max(0);
                    *v += add;
                    extra -= add;
                    if *v == 1 {
                        *v = 2;
                        extra -= 1;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CFG0: Params = Params {
        variant: Variant::Cfg0,
        sample_rate: 48_000,
    };
    const CFG1: Params = Params {
        variant: Variant::Cfg1,
        ..CFG0
    };
    const CFG3: Params = Params {
        variant: Variant::Cfg3,
        ..CFG0
    };
    const CFG4: Params = Params {
        variant: Variant::Cfg4,
        ..CFG0
    };
    const CFG3_96K: Params = Params {
        variant: Variant::Cfg3,
        sample_rate: 96_000,
    };

    #[test]
    fn pools() {
        assert_eq!(pool(CFG0, 364_000), 30);
        assert_eq!(pool(CFG1, 265_000), 22);
        assert_eq!(pool(CFG3, 420_000), 70);
        // Clamped to 350000·2.
        assert_eq!(pool(CFG0, 2_000_000), pool(CFG0, 700_000));
        assert_eq!(pool(CFG3, 1), pool(CFG3, 90_000));
        // Config 4 caps the pool at 210000·2 (§10.F).
        assert_eq!(pool(CFG4, 600_000), pool(CFG4, 420_000));
    }

    #[test]
    fn ceilings_follow_rate() {
        assert_eq!(max_bits(CFG0, 319_999, false), MAX_BITS_4BAND_320);
        assert_eq!(max_bits(CFG0, 320_000, false), MAX_BITS_4BAND_370);
        assert_eq!(max_bits(CFG0, 364_000, false), MAX_BITS_4BAND_370);
        assert_eq!(max_bits(CFG0, 370_000, false), MAX_BITS_4BAND_MAX);
        assert_eq!(max_bits(CFG3, 420_000, false), MAX_BITS_8BAND_520);
        assert_eq!(max_bits(CFG3, 250_000, true), MAX_BITS_8BAND_MAX);
        assert_eq!(max_bits(CFG4, 250_000, true), MAX_BITS_8BAND_420);
        assert_eq!(max_bits(CFG4, 419_999, false), MAX_BITS_8BAND_420);
        assert_eq!(max_bits(CFG4, 420_000, false), MAX_BITS_8BAND_420);
        assert_eq!(max_bits(CFG4, 400_000, false), MAX_BITS_8BAND_420);
        assert_eq!(max_bits(CFG4, 369_999, false), MAX_BITS_8BAND_370);
        assert_eq!(max_bits(CFG3_96K, 420_000, false), MAX_BITS_8BAND_96K_500);
        assert_eq!(max_bits(CFG3_96K, 820_000, false), MAX_BITS_8BAND_96K_MAX);
    }

    /// Config 4 stays at 9 bits without allocation escapes, including a loud non-tonal band (§10.F).
    #[test]
    fn config_4_stays_at_9_bits() {
        let x: Vec<i32> = (0..64 * 16)
            .map(|i| if i % 8 == 0 { 1 << 30 } else { 1 << 20 })
            .collect();
        for rate in [336_000, 420_000, 600_000, u32::MAX] {
            let mut out = [0; 16];
            allocate(CFG4, rate, rate, &x, &mut out);
            assert!(out.iter().all(|&b| b <= 9), "{rate}: {out:?}");
        }
    }

    #[test]
    fn scale_factors() {
        assert_eq!(scale_factor(0), 0);
        assert_eq!(scale_factor(2 << 8), 0);
        assert_eq!(scale_factor(3 << 8), 1);
        assert_eq!(scale_factor(1 << 31), 22);
    }

    /// §8.6: silence on config 0 at 364 kbps gives 4/4/4/3 per channel, and on config 3
    /// all zero.
    #[test]
    fn silence() {
        let mut out = [0; 8];
        assert_eq!(allocate(CFG0, 364_000, 364_000, &[0; 64 * 8], &mut out), 30);
        assert_eq!(out, [4, 4, 4, 3, 4, 4, 4, 3]);
        let mut out = [9; 16];
        allocate(CFG3, 364_000, 364_000, &[0; 64 * 16], &mut out);
        assert_eq!(out, [0; 16]);
    }

    /// One quiet channel on config 3 keeps band 0 at 2 bits.
    #[test]
    fn digital_black_keeps_a_channel_alive() {
        let x: Vec<i32> = (0..64 * 16)
            .map(|i| if i % 16 < 8 { (i * 7919) << 12 } else { 0 })
            .collect();
        let mut out = [0; 16];
        allocate(CFG3, 420_000, 420_000, &x, &mut out);
        assert!(out[..8].iter().any(|&b| b > 2), "{out:?}");
        assert_eq!(out[8..], [2, 0, 0, 0, 0, 0, 0, 0]);
    }
}
