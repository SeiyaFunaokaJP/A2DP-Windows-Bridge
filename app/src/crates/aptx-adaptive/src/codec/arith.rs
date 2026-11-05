//! Arithmetic conventions (spec `codec-core.md` §0).

pub fn sat24(v: i64) -> i32 {
    v.clamp(-(1 << 23), (1 << 23) - 1) as i32
}

pub fn sat32(v: i64) -> i32 {
    v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

pub fn sat64(v: i128) -> i64 {
    v.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

pub fn add(a: i32, b: i32) -> i32 {
    a.saturating_add(b)
}

pub fn sub(a: i32, b: i32) -> i32 {
    a.saturating_sub(b)
}

/// Convergent rounding by 2^n, half to even.
pub fn cr(v: i64, n: u32) -> i64 {
    let t = ((v >> (n - 1)) + 1) >> 1;
    if v & ((1 << n) - 1) == 1 << (n - 1) {
        t & !1
    } else {
        t
    }
}

pub fn cr32(v: i32, n: u32) -> i32 {
    cr(i64::from(v), n) as i32
}

/// `sat24(lo32(CR23(v)))`, the most common rounding in the quantiser.
pub fn r23(v: i64) -> i32 {
    sat24(i64::from(cr(v, 23) as i32))
}

/// `R64(v)`: round the high word.
pub fn r64(v: i128) -> i32 {
    sat32(((i128::from(sat64(v)) + (1 << 31)) >> 32) as i64)
}

pub fn sext24(v: i32) -> i32 {
    (v << 8) >> 8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convergent_rounding() {
        assert_eq!(cr(3, 1), 2);
        assert_eq!(cr(1, 1), 0);
        assert_eq!(cr(5, 1), 2);
        assert_eq!(cr(-1, 1), 0);
        assert_eq!(cr(-3, 1), -2);
        assert_eq!(cr(7, 2), 2);
        assert_eq!(cr(6, 2), 2);
        assert_eq!(cr(10, 2), 2);
        assert_eq!(cr(11, 2), 3);
    }

    #[test]
    fn round_high_word() {
        assert_eq!(r64(1 << 31), 1);
        assert_eq!(r64((1 << 31) - 1), 0);
        assert_eq!(r64(i128::MAX), i32::MAX);
    }
}
