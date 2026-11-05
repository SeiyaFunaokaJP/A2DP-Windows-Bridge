//! The developer override aac_latm_v0 brings back the StreamMuxConfig of
//! 2.0.2 and before (AudioMuxVersion 0). A test binary of its own: the
//! override is process-wide and would change the other AAC tests.

#![cfg(feature = "aac")]

use codec::aac::AacEncoder;
use codec::dev::{DevOverrides, set_overrides};
use codec::{Encoder, EncoderConfig, Pcm, PeerParams, Quality};

/// useSameStreamMux and audioMuxVersion of the first frame
fn first_bits() -> (u8, u8) {
    let cfg = EncoderConfig {
        sample_rate: 48000,
        channels: 2,
        quality: Quality::Standard,
        mtu: 660,
        bits_per_sample: 16,
        abr: false,
        peer: PeerParams::default(),
    };
    let mut enc = AacEncoder::new(&cfg).unwrap();
    let pcm = vec![0i16; 1024 * 2];
    let mut out = vec![0u8; enc.max_output_bytes()];
    let e = enc.encode(Pcm::I16(&pcm), &mut out).unwrap();
    assert!(e.bytes > 0);
    (out[0] >> 7, (out[0] >> 6) & 1)
}

#[test]
fn latm_version_follows_the_override() {
    // StreamMuxConfig present, AudioMuxVersion 1 (Android, PipeWire)
    assert_eq!(first_bits(), (0, 1));
    set_overrides(DevOverrides { aac_latm_v0: true, ..Default::default() });
    assert_eq!(first_bits(), (0, 0));
    set_overrides(DevOverrides::default());
    assert_eq!(first_bits(), (0, 1));
}
