//! libopenaptx 0.2.0: the aptX / aptX HD encoder (and decoder) BlueCaster uses
//! for aptX, aptX HD and aptX Low Latency.
//!
//! # Licence
//!
//! libopenaptx is LGPL-2.1-or-later (Aurelien Jacobs, Pali Rohár; derived from
//! FFmpeg). This crate builds it from the git submodule extern/libopenaptx as
//! a static library of its own (`openaptx`) and keeps it apart from the rest
//! of BlueCaster (Apache-2.0 OR MIT), so the LGPL obligations stay with this one
//! crate: ship the LGPL text, give notice of the library, and let users relink
//! with their own build of it (THIRD_PARTY_LICENSES.md, §2). The build stops on
//! any version other than 0.2.0, because 0.2.1 and later are GPL-3.0-or-later.
//! The Rust code in this crate is Apache-2.0 OR MIT like the rest of BlueCaster.
//!
//! # Layout
//!
//! - [`ffi`]: the C API of openaptx.h, declared by hand.
//! - [`AptxContext`]: a safe owner of an `aptx_context`.

use std::ffi::c_int;
use std::ptr::NonNull;

/// Hand-written declarations of openaptx.h (0.2.0)
#[allow(non_camel_case_types)]
pub mod ffi {
    use std::ffi::{c_int, c_uchar};
    use std::marker::{PhantomData, PhantomPinned};

    /// `struct aptx_context` (opaque, allocated by `aptx_init`)
    #[repr(C)]
    pub struct aptx_context {
        _private: [u8; 0],
        _marker: PhantomData<(*mut u8, PhantomPinned)>,
    }

    unsafe extern "C" {
        pub static aptx_major: c_int;
        pub static aptx_minor: c_int;
        pub static aptx_patch: c_int;

        /// `struct aptx_context *aptx_init(int hd);` hd = 0 aptX, 1 aptX HD;
        /// NULL when out of memory
        pub fn aptx_init(hd: c_int) -> *mut aptx_context;

        /// `void aptx_reset(struct aptx_context *ctx);`
        pub fn aptx_reset(ctx: *mut aptx_context);

        /// `void aptx_finish(struct aptx_context *ctx);` (frees the context)
        pub fn aptx_finish(ctx: *mut aptx_context);

        /// `size_t aptx_encode(struct aptx_context *ctx, const unsigned char *input,
        /// size_t input_size, unsigned char *output, size_t output_size, size_t *written);`
        pub fn aptx_encode(
            ctx: *mut aptx_context,
            input: *const c_uchar,
            input_size: usize,
            output: *mut c_uchar,
            output_size: usize,
            written: *mut usize,
        ) -> usize;

        /// `int aptx_encode_finish(struct aptx_context *ctx, unsigned char *output,
        /// size_t output_size, size_t *written);`
        pub fn aptx_encode_finish(
            ctx: *mut aptx_context,
            output: *mut c_uchar,
            output_size: usize,
            written: *mut usize,
        ) -> c_int;

        /// `size_t aptx_decode(struct aptx_context *ctx, const unsigned char *input,
        /// size_t input_size, unsigned char *output, size_t output_size, size_t *written);`
        pub fn aptx_decode(
            ctx: *mut aptx_context,
            input: *const c_uchar,
            input_size: usize,
            output: *mut c_uchar,
            output_size: usize,
            written: *mut usize,
        ) -> usize;

        /// `size_t aptx_decode_sync(struct aptx_context *ctx, const unsigned char *input,
        /// size_t input_size, unsigned char *output, size_t output_size, size_t *written,
        /// int *synced, size_t *dropped);`
        pub fn aptx_decode_sync(
            ctx: *mut aptx_context,
            input: *const c_uchar,
            input_size: usize,
            output: *mut c_uchar,
            output_size: usize,
            written: *mut usize,
            synced: *mut c_int,
            dropped: *mut usize,
        ) -> usize;

        /// `size_t aptx_decode_sync_finish(struct aptx_context *ctx);`
        pub fn aptx_decode_sync_finish(ctx: *mut aptx_context) -> usize;
    }
}

/// PCM bytes of one aptX group: 4 stereo samples as packed 24-bit signed
/// little-endian, LLLRRR x4 (what `aptx_encode` takes, `aptx_decode` returns)
pub const PCM_GROUP_BYTES: usize = 24;

/// PCM frames (samples per channel) of one aptX group
pub const GROUP_FRAMES: usize = 4;

/// Encoded bytes of one group: 4 for aptX (LLRR), 6 for aptX HD (LLLRRR)
pub const fn encoded_group_bytes(hd: bool) -> usize {
    if hd { 6 } else { 4 }
}

/// Version of the linked library (major, minor, patch); always (0, 2, 0)
pub fn version() -> (i32, i32, i32) {
    // SAFETY: plain `const int` objects defined in openaptx.c, never written
    unsafe { (ffi::aptx_major, ffi::aptx_minor, ffi::aptx_patch) }
}

/// An aptX or aptX HD codec context (encoder and decoder state)
pub struct AptxContext {
    ctx: NonNull<ffi::aptx_context>,
    hd: bool,
}

// SAFETY: the context is a plain heap block owned by this value; libopenaptx
// has no global or thread-local state, so it may move to another thread.
unsafe impl Send for AptxContext {}

impl AptxContext {
    /// A new context for aptX (`hd` false) or aptX HD; `None` when out of memory
    pub fn new(hd: bool) -> Option<AptxContext> {
        // SAFETY: aptx_init only allocates and initializes a new context
        let ctx = unsafe { ffi::aptx_init(c_int::from(hd)) };
        NonNull::new(ctx).map(|ctx| AptxContext { ctx, hd })
    }

    pub fn is_hd(&self) -> bool {
        self.hd
    }

    /// Encoded bytes per group of this context
    pub fn encoded_group_bytes(&self) -> usize {
        encoded_group_bytes(self.hd)
    }

    /// Resets the predictor and parity state for a new stream
    pub fn reset(&mut self) {
        // SAFETY: self.ctx is a live context from aptx_init
        unsafe { ffi::aptx_reset(self.ctx.as_ptr()) }
    }

    /// Encodes whole groups of `input` (packed 24-bit LLLRRR x4, see
    /// [`PCM_GROUP_BYTES`]) while they fit `output`. Returns (input bytes
    /// consumed, output bytes written).
    pub fn encode(&mut self, input: &[u8], output: &mut [u8]) -> (usize, usize) {
        let mut written = 0usize;
        // SAFETY: self.ctx is live; aptx_encode reads at most input_size bytes
        // of input and writes at most output_size bytes of output (it checks
        // both before each group), and both slices are valid for those sizes
        let consumed = unsafe {
            ffi::aptx_encode(
                self.ctx.as_ptr(),
                input.as_ptr(),
                input.len(),
                output.as_mut_ptr(),
                output.len(),
                &mut written,
            )
        };
        (consumed, written)
    }

    /// Flushes the encoder latency (90 samples, rounded to 92) as encoded
    /// silence. Returns (finished, bytes written); call again while not
    /// finished. The context is reset once finished.
    pub fn encode_finish(&mut self, output: &mut [u8]) -> (bool, usize) {
        let mut written = 0usize;
        // SAFETY: self.ctx is live; aptx_encode_finish writes at most
        // output_size bytes (checked before each group)
        let done =
            unsafe { ffi::aptx_encode_finish(self.ctx.as_ptr(), output.as_mut_ptr(), output.len(), &mut written) };
        (done != 0, written)
    }

    /// Decodes whole groups of `input` into packed 24-bit LLLRRR samples.
    /// Returns (input bytes consumed, output bytes written); consumed short of
    /// the whole groups in `input` means a parity error. The first 90 samples
    /// of a stream are the codec latency and are not output.
    ///
    /// `output` must hold at least one group ([`PCM_GROUP_BYTES`]); with less
    /// nothing is decoded. (At the end of the latency `aptx_decode` writes half
    /// a group without checking the output size, so smaller buffers are not
    /// passed to C at all.)
    pub fn decode(&mut self, input: &[u8], output: &mut [u8]) -> (usize, usize) {
        if output.len() < PCM_GROUP_BYTES {
            return (0, 0);
        }
        let mut written = 0usize;
        // SAFETY: self.ctx is live; aptx_decode reads whole groups within
        // input_size and writes a full group only when it fits output_size;
        // the unchecked half group at the end of the latency fits as well,
        // because output holds at least one group and nothing was written
        // before it in the same call
        let consumed = unsafe {
            ffi::aptx_decode(
                self.ctx.as_ptr(),
                input.as_ptr(),
                input.len(),
                output.as_mut_ptr(),
                output.len(),
                &mut written,
            )
        };
        (consumed, written)
    }
}

impl Drop for AptxContext {
    fn drop(&mut self) {
        // SAFETY: self.ctx came from aptx_init and is freed only here
        unsafe { ffi::aptx_finish(self.ctx.as_ptr()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_lgpl_release() {
        assert_eq!(version(), (0, 2, 0));
    }

    #[test]
    fn silence_round_trip() {
        for hd in [false, true] {
            let mut enc = AptxContext::new(hd).unwrap();
            let mut dec = AptxContext::new(hd).unwrap();
            let pcm = [0u8; PCM_GROUP_BYTES * 64];
            let mut coded = vec![0u8; encoded_group_bytes(hd) * 64];
            assert_eq!(enc.encode(&pcm, &mut coded), (pcm.len(), coded.len()));
            let mut out = vec![0u8; pcm.len()];
            let (consumed, written) = dec.decode(&coded, &mut out);
            assert_eq!(consumed, coded.len());
            // 90 samples of latency are swallowed
            assert_eq!(written, (64 * 4 - 90) * 6);
            // Silence comes back as (near) silence: within 1/32768 of full scale
            for s in out[..written].as_chunks::<3>().0 {
                let v = i32::from_le_bytes([0, s[0], s[1], s[2]]) >> 8;
                assert!(v.abs() < 256, "{v}");
            }
            let mut tail = vec![0u8; encoded_group_bytes(hd) * 32];
            let (done, n) = enc.encode_finish(&mut tail);
            assert!(done);
            assert_eq!(n, encoded_group_bytes(hd) * 23);
        }
    }

    #[test]
    fn small_buffers_are_safe() {
        let mut enc = AptxContext::new(false).unwrap();
        let pcm = [0u8; PCM_GROUP_BYTES];
        assert_eq!(enc.encode(&pcm, &mut [0u8; 3]), (0, 0));
        assert_eq!(enc.encode(&pcm[..23], &mut [0u8; 4]), (0, 0));
        let mut dec = AptxContext::new(false).unwrap();
        assert_eq!(dec.decode(&[0u8; 4], &mut [0u8; 12]), (0, 0));
    }
}
