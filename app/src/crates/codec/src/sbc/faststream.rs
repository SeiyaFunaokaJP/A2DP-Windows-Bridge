//! CSR FastStream (vendor 0x0000000A, codec 0x0001), the music direction:
//! SBC at 44.1 / 48 kHz, joint stereo, 16 blocks, 8 subbands, loudness,
//! bitpool 29 — fixed, the codec information carries only the direction and
//! the rates. As PipeWire's a2dp-codec-faststream.c: each 71-byte frame is
//! padded to an even 72 bytes, a packet carries three frames and has neither
//! a media payload header nor an RTP header. The voice backchannel (16 kHz
//! SBC from the headset) is not implemented.

use super::{Allocation, ChannelMode, SbcConfig, SbcFrameEncoder};
use crate::{CodecKind, Encoded, Encoder, EncoderConfig, EncoderDetails, EncoderError, Pcm, PcmFormat};

const BLOCKS: u8 = 16;
const SUBBANDS: u8 = 8;
const PCM_FRAMES: usize = BLOCKS as usize * SUBBANDS as usize;

/// The fixed bitpool (PipeWire codec_init)
pub const FASTSTREAM_BITPOOL: u8 = 29;
/// One frame as sent: 71 bytes of SBC, padded to 72
pub const FASTSTREAM_FRAME_BYTES: usize = 72;
/// Frames per packet (PipeWire max_frames)
pub const FASTSTREAM_FRAMES_PER_PACKET: usize = 3;
/// A media packet's payload: three padded frames
pub const FASTSTREAM_MAX_PAYLOAD: usize = FASTSTREAM_FRAMES_PER_PACKET * FASTSTREAM_FRAME_BYTES;

/// The FastStream encoder: one padded SBC frame (128 PCM frames) per
/// `encode`. Takes stereo, or mono that it sends on both channels.
pub struct FastStreamEncoder {
    frame: SbcFrameEncoder,
    sample_rate: u32,
    mono: bool,
    stereo: [i16; 2 * PCM_FRAMES],
}

impl FastStreamEncoder {
    pub fn new(config: &EncoderConfig) -> Result<FastStreamEncoder, EncoderError> {
        let unsupported = |what: String| EncoderError::Unsupported { codec: CodecKind::FastStream, what };
        if config.sample_rate != 44100 && config.sample_rate != 48000 {
            return Err(unsupported(format!("sample rate {} Hz (44100 or 48000)", config.sample_rate)));
        }
        if !matches!(config.channels, 1 | 2) {
            return Err(unsupported(format!("{} channels", config.channels)));
        }
        let frame = SbcFrameEncoder::new(SbcConfig {
            sample_rate: config.sample_rate,
            channel_mode: ChannelMode::JointStereo,
            blocks: BLOCKS,
            subbands: SUBBANDS,
            allocation: Allocation::Loudness,
            bitpool: FASTSTREAM_BITPOOL,
        })?;
        debug_assert_eq!(frame.max_frame_len().next_multiple_of(2), FASTSTREAM_FRAME_BYTES);
        Ok(FastStreamEncoder {
            frame,
            sample_rate: config.sample_rate,
            mono: config.channels == 1,
            stereo: [0; 2 * PCM_FRAMES],
        })
    }
}

impl Encoder for FastStreamEncoder {
    fn kind(&self) -> CodecKind {
        CodecKind::FastStream
    }

    fn pcm_format(&self) -> PcmFormat {
        PcmFormat::I16
    }

    fn pcm_frames_per_encode(&self) -> usize {
        PCM_FRAMES
    }

    fn max_output_bytes(&self) -> usize {
        FASTSTREAM_FRAME_BYTES
    }

    fn encode(&mut self, pcm: Pcm<'_>, out: &mut [u8]) -> Result<Encoded, EncoderError> {
        let Pcm::I16(pcm) = pcm else {
            return Err(EncoderError::Encode {
                codec: CodecKind::FastStream,
                what: "FastStream takes 16-bit PCM".into(),
            });
        };
        if out.len() < FASTSTREAM_FRAME_BYTES {
            return Err(EncoderError::OutputTooSmall { have: out.len(), need: FASTSTREAM_FRAME_BYTES });
        }
        let bytes = if self.mono {
            if pcm.len() != PCM_FRAMES {
                return Err(EncoderError::InputLength { expected: PCM_FRAMES, got: pcm.len() });
            }
            for (pair, &s) in self.stereo.as_chunks_mut::<2>().0.iter_mut().zip(pcm) {
                pair[0] = s;
                pair[1] = s;
            }
            self.frame.encode(&self.stereo, out)?
        } else {
            self.frame.encode(pcm, out)?
        };
        // Pad to an even length, as PipeWire
        let mut bytes = bytes;
        while bytes < FASTSTREAM_FRAME_BYTES {
            out[bytes] = 0;
            bytes += 1;
        }
        Ok(Encoded { bytes, frames: 1 })
    }

    /// The padded frames: 8 * 72 * rate / 128, in kbps (198 / 216)
    fn bitrate_kbps(&self) -> u32 {
        (8 * FASTSTREAM_FRAME_BYTES as u32 * self.sample_rate) / PCM_FRAMES as u32 / 1000
    }

    /// Frames as sent (padded); the SBC setup is fixed
    fn details(&self) -> EncoderDetails {
        let c = *self.frame.config();
        EncoderDetails {
            frame_bytes: Some(FASTSTREAM_FRAME_BYTES as u32),
            sbc: Some(SbcConfig { bitpool: c.effective_bitpool(), ..c }),
            sbc_abr: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PeerParams, Quality};

    fn config(sample_rate: u32, channels: u32) -> EncoderConfig {
        EncoderConfig {
            sample_rate,
            channels,
            quality: Quality::High,
            mtu: 1023,
            bits_per_sample: 16,
            abr: false,
            peer: PeerParams::default(),
        }
    }

    #[test]
    fn frames_and_bitrate() {
        // (rate, header byte 1, kbps): 44.1 / 48 kHz, 16 blocks, joint
        // stereo, loudness, 8 subbands
        for (rate, hdr, kbps) in [(44100, 0b1011_1101, 198), (48000, 0b1111_1101, 216)] {
            for ch in [1, 2] {
                let mut e = FastStreamEncoder::new(&config(rate, ch)).unwrap();
                assert_eq!(e.kind(), CodecKind::FastStream);
                assert_eq!((e.pcm_frames_per_encode(), e.max_output_bytes(), e.bitrate_kbps()), (128, 72, kbps));
                let pcm: Vec<i16> = (0..128 * ch as usize).map(|i| ((i * 7919) % 20000) as i16 - 10000).collect();
                let mut out = [0xFFu8; 72];
                for _ in 0..3 {
                    assert_eq!(e.encode(Pcm::I16(&pcm), &mut out).unwrap(), Encoded { bytes: 72, frames: 1 });
                    assert_eq!((out[0], out[1], out[2], out[71]), (0x9C, hdr, FASTSTREAM_BITPOOL, 0));
                }
            }
        }
        assert_eq!(FASTSTREAM_MAX_PAYLOAD, 216);
    }

    #[test]
    fn mono_is_sent_on_both_channels() {
        let pcm: Vec<i16> = (0..128).map(|i| (i * 100) as i16).collect();
        let stereo: Vec<i16> = pcm.iter().flat_map(|&s| [s, s]).collect();
        let (mut a, mut b) = ([0u8; 72], [0u8; 72]);
        FastStreamEncoder::new(&config(48000, 1)).unwrap().encode(Pcm::I16(&pcm), &mut a).unwrap();
        FastStreamEncoder::new(&config(48000, 2)).unwrap().encode(Pcm::I16(&stereo), &mut b).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn errors() {
        for (rate, ch) in [(32000, 2), (96000, 2), (48000, 3), (44100, 0)] {
            assert!(matches!(
                FastStreamEncoder::new(&config(rate, ch)),
                Err(EncoderError::Unsupported { codec: CodecKind::FastStream, .. })
            ));
        }
        let mut e = crate::create(CodecKind::FastStream, &config(48000, 2)).unwrap();
        assert!(matches!(
            e.encode(Pcm::I16(&[0; 256]), &mut [0u8; 71]),
            Err(EncoderError::OutputTooSmall { have: 71, need: 72 })
        ));
        assert!(matches!(e.encode(Pcm::I16(&[0; 128]), &mut [0u8; 72]), Err(EncoderError::InputLength { .. })));
        assert!(matches!(e.encode(Pcm::I32(&[0; 256]), &mut [0u8; 72]), Err(EncoderError::Encode { .. })));
        let mut e = FastStreamEncoder::new(&config(48000, 1)).unwrap();
        assert!(matches!(e.encode(Pcm::I16(&[0; 256]), &mut [0u8; 72]), Err(EncoderError::InputLength { .. })));
    }
}
