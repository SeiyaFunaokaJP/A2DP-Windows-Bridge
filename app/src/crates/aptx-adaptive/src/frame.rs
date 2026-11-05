//! Output framing (spec `a2dp-integration.md` §7D): the payload is byte-swapped in 16-bit
//! units.

pub fn swap16(src: &[u8], dst: &mut [u8]) {
    let dst = &mut dst[..src.len()];
    let (pairs, tail) = src.as_chunks::<2>();
    let (out, out_tail) = dst.as_chunks_mut::<2>();
    for (o, p) in out.iter_mut().zip(pairs) {
        *o = [p[1], p[0]];
    }
    out_tail.copy_from_slice(tail);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swaps_pairs() {
        let mut dst = [0; 6];
        swap16(&[0x00, 0x81, 0xa1, 0xd3, 0x10], &mut dst);
        assert_eq!(dst, [0x81, 0x00, 0xd3, 0xa1, 0x10, 0]);
    }
}
