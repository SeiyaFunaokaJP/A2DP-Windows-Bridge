//! Packs codec frames into A2DP media payloads.
//!
//! Frames are collected until the next would not fit the payload size; each
//! round of the encode thread ends with a flush, so a packet never waits for
//! the next round (as v1). The media payload header goes in front: SBC,
//! LDAC and Opus (and Opus 05) carry the frame count in the low 4 bits (no fragmentation),
//! LHDC V5 the frame count shifted left by 2 and a sequence number; the
//! others none. LDAC, Opus and AAC send each encoder output as a packet of its
//! own. FastStream packets are always three padded SBC frames (PipeWire), so
//! a round does not flush a partly filled one ([`Packetizer::end_round`]).
//! aptX Adaptive's encoder outputs whole media packets, its own RTP header
//! included: each goes out as is.

use bt_host::media::{MAX_MEDIA_PACKET_SIZE, MediaPacket, RTP_HEADER_SIZE};
use codec::CodecKind;

/// Each encoder output is a packet of its own. libldac sizes each output for
/// one packet (frames per 2-DH5 / 3-DH5): send it as is, as Android does,
/// rather than merging outputs. Opus: one frame per packet (Android;
/// PipeWire's decoder takes no other). AAC: one LATM AudioMuxElement per
/// packet; concatenated ones (small frames of quiet audio) are not
/// decodable, and sinks such as the JBL Tune 510BT stay silent (issue #87).
/// aptX Adaptive: one RTP packet per output.
fn one_output_per_packet(kind: CodecKind) -> bool {
    matches!(kind, CodecKind::Ldac | CodecKind::Opus | CodecKind::Opus05 | CodecKind::Aac | CodecKind::AptxAdaptive)
}

pub struct Packetizer {
    kind: CodecKind,
    header: usize,
    /// Largest codec data per packet (header excluded)
    max_raw: usize,
    packet: MediaPacket,
    frames: u32,
    first_ts: u32,
    lhdc_seq: u8,
}

/// Codec data that fits one media payload of `mtu` bytes (RTP header
/// excluded) for `kind`
pub fn max_raw(kind: CodecKind, mtu: u16, sample_rate: u32) -> usize {
    let mtu = mtu as usize;
    match kind {
        CodecKind::Sbc | CodecKind::SbcXq | CodecKind::Ldac | CodecKind::Opus | CodecKind::Opus05 => mtu - 1,
        CodecKind::LhdcV5 => mtu - 2,
        // The encoder's RTP header and one codec packet
        CodecKind::AptxAdaptive => (mtu + RTP_HEADER_SIZE).min(MAX_MEDIA_PACKET_SIZE),
        // No RTP header, no media payload header: three 72-byte frames
        CodecKind::FastStream => (mtu + RTP_HEADER_SIZE).min(codec::sbc::FASTSTREAM_MAX_PAYLOAD),
        // aptX and aptX LL go without the RTP header: the whole L2CAP MTU is
        // payload; whole 4-byte aptX frames
        CodecKind::Aptx | CodecKind::AptxLl => {
            let mut max = (mtu + RTP_HEADER_SIZE).min(MAX_MEDIA_PACKET_SIZE);
            if kind == CodecKind::AptxLl && sample_rate > 0 {
                // Low latency: packets of at most ~7.5 ms like PipeWire (4
                // bytes per 4 samples: samples = bytes)
                let ll = (sample_rate as usize * 75) / 10000;
                if ll >= 4 {
                    max = max.min(ll);
                }
            }
            max & !3
        }
        _ => mtu,
    }
}

impl Packetizer {
    pub fn new(kind: CodecKind, mtu: u16, sample_rate: u32) -> Packetizer {
        let header = kind.media_header_len();
        let max_raw = max_raw(kind, mtu, sample_rate).min(MAX_MEDIA_PACKET_SIZE - header);
        tracing::debug!(
            "packetizer: {kind}, mtu {mtu}, {sample_rate} Hz, media header {header} bytes, up to {max_raw} codec \
             bytes per packet, rtp {}, {}",
            kind.uses_rtp(),
            if one_output_per_packet(kind) { "one encoder output per packet" } else { "frames merged up to the limit" }
        );
        Packetizer {
            kind,
            header,
            max_raw,
            packet: MediaPacket { len: header as u16, rtp: kind.uses_rtp(), ..Default::default() },
            frames: 0,
            first_ts: 0,
            lhdc_seq: 0,
        }
    }

    pub fn reset(&mut self) {
        self.packet.len = self.header as u16;
        self.frames = 0;
        self.lhdc_seq = 0;
    }

    fn raw_len(&self) -> usize {
        self.packet.len as usize - self.header
    }

    /// Adds one encoder output (`frames` codec frames whose first sample has
    /// RTP timestamp `timestamp`); returns the packets sent through `send`
    pub fn push(&mut self, data: &[u8], frames: u32, timestamp: u32, send: &mut dyn FnMut(&MediaPacket)) -> u32 {
        let mut sent = 0;
        // SBC and LDAC count frames in 4 bits: a packet holds at most 15
        let count_full = self.header == 1 && self.frames + frames > 15;
        if (self.raw_len() + data.len() > self.max_raw || count_full) && self.frames > 0 {
            sent += self.flush(send);
        }
        if self.frames == 0 && self.raw_len() == 0 {
            self.first_ts = timestamp;
        }
        let at = self.packet.len as usize;
        if at + data.len() <= MAX_MEDIA_PACKET_SIZE {
            self.packet.data[at..at + data.len()].copy_from_slice(data);
            self.packet.len += data.len() as u16;
            self.frames += frames;
        } else {
            tracing::debug!(
                "packetizer: {} byte encoder output dropped, larger than a packet ({} free)",
                data.len(),
                MAX_MEDIA_PACKET_SIZE - at
            );
        }
        if one_output_per_packet(self.kind) {
            sent += self.flush(send);
        }
        sent
    }

    /// The end of an encode round: sends what is collected, except a
    /// FastStream packet that has room for more frames; returns the packets
    /// sent (0 or 1)
    pub fn end_round(&mut self, send: &mut dyn FnMut(&MediaPacket)) -> u32 {
        if self.kind == CodecKind::FastStream && self.raw_len() < self.max_raw {
            return 0;
        }
        self.flush(send)
    }

    /// Sends what is collected; returns the packets sent (0 or 1)
    pub fn flush(&mut self, send: &mut dyn FnMut(&MediaPacket)) -> u32 {
        if self.frames == 0 {
            return 0;
        }
        match self.kind {
            CodecKind::Sbc | CodecKind::SbcXq | CodecKind::Ldac | CodecKind::Opus | CodecKind::Opus05 => {
                self.packet.data[0] = (self.frames & 0x0F) as u8
            }
            CodecKind::LhdcV5 => {
                // Android A2DP_VendorBuildCodecHeaderLhdcV5: frames << 2,
                // latency bits 1..0 = 0; then the packet sequence number
                self.packet.data[0] = ((self.frames & 0x3F) << 2) as u8;
                self.packet.data[1] = self.lhdc_seq;
                self.lhdc_seq = self.lhdc_seq.wrapping_add(1);
            }
            _ => {}
        }
        self.packet.timestamp = self.first_ts;
        self.packet.marker = self.packet.rtp && codec::dev::overrides().rtp_marker;
        tracing::trace!(
            "packetizer: packet timestamp {}, {} frames, {} bytes, header {:02x?}",
            self.packet.timestamp,
            self.frames,
            self.packet.len,
            &self.packet.data[..self.header]
        );
        send(&self.packet);
        self.packet.len = self.header as u16;
        self.frames = 0;
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sbc_frames_fill_the_mtu() {
        // 119-byte SBC frames (48k joint stereo bitpool 53), mtu 679: 5 fit
        let mut p = Packetizer::new(CodecKind::Sbc, 679, 48000);
        let mut out = Vec::new();
        let frame = [0x9Cu8; 119];
        let mut ts = 0;
        for _ in 0..12 {
            p.push(&frame, 1, ts, &mut |pk| out.push((pk.payload().to_vec(), pk.timestamp)));
            ts += 128;
        }
        p.flush(&mut |pk| out.push((pk.payload().to_vec(), pk.timestamp)));
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].0[0], 5);
        assert_eq!(out[0].0.len(), 1 + 5 * 119);
        assert_eq!(out[1].1, 5 * 128);
        assert_eq!(out[2].0[0], 2);
    }

    #[test]
    fn sbc_xq_frames_fill_the_mtu() {
        // 200-byte SBC XQ frames (44.1k dual channel bitpool 47): 5 in 1023,
        // with SBC's media payload header and RTP
        let mut p = Packetizer::new(CodecKind::SbcXq, 1023, 44100);
        let mut out = Vec::new();
        for i in 0..6 {
            p.push(&[0x9Cu8; 200], 1, i * 128, &mut |pk| out.push((pk.payload().to_vec(), pk.rtp)));
        }
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].0[0], out[0].0.len(), out[0].1), (5, 1 + 5 * 200, true));
    }

    #[test]
    fn sbc_frames_of_changing_length_share_a_packet() {
        // SBC ABR changes the bitpool between frames: 119-byte frames
        // (bitpool 53), then 115 (51) and 51 (19) bytes, mtu 679 (678 bytes
        // of frames)
        let mut p = Packetizer::new(CodecKind::Sbc, 679, 48000);
        let mut out = Vec::new();
        let mut ts = 0;
        let mut push = |p: &mut Packetizer, len: usize, bitpool: u8, out: &mut Vec<(Vec<u8>, u32)>| {
            let mut frame = vec![0u8; len];
            frame[0] = 0x9C;
            frame[2] = bitpool;
            p.push(&frame, 1, ts, &mut |pk| out.push((pk.payload().to_vec(), pk.timestamp)));
            ts += 128;
        };
        for _ in 0..3 {
            push(&mut p, 119, 53, &mut out);
        }
        for _ in 0..2 {
            push(&mut p, 115, 51, &mut out);
        }
        // 3 * 119 + 2 * 115 = 587: one more 115 does not fit, a 51 does
        push(&mut p, 51, 19, &mut out);
        assert!(out.is_empty());
        push(&mut p, 115, 51, &mut out);
        p.flush(&mut |pk| out.push((pk.payload().to_vec(), pk.timestamp)));
        assert_eq!(out.len(), 2);
        let (first, ts0) = &out[0];
        assert_eq!((first[0], first.len(), *ts0), (6, 1 + 3 * 119 + 2 * 115 + 51, 0));
        // Each frame keeps its own header and bitpool
        let starts = [0, 119, 238, 357, 472, 587].map(|o| 1 + o);
        let bitpools: Vec<u8> = starts.iter().map(|&o| first[o + 2]).collect();
        assert!(starts.iter().all(|&o| first[o] == 0x9C));
        assert_eq!(bitpools, [53, 53, 53, 51, 51, 19]);
        assert_eq!((out[1].0[0], out[1].0.len(), out[1].1), (1, 1 + 115, 6 * 128));
    }

    #[test]
    fn faststream_packets_are_three_frames() {
        assert_eq!(max_raw(CodecKind::FastStream, 1023, 48000), 216);
        let mut p = Packetizer::new(CodecKind::FastStream, 1023, 48000);
        let mut out = Vec::new();
        // Rounds of 3.75 frames (10 ms at 48 kHz): never a partial packet
        for round in 0..8u32 {
            let frames = if round % 4 == 3 { 3 } else { 4 };
            for _ in 0..frames {
                p.push(&[0x9Cu8; 72], 1, 0, &mut |pk| out.push((pk.payload().len(), pk.rtp)));
            }
            p.end_round(&mut |pk| out.push((pk.payload().len(), pk.rtp)));
        }
        // 30 frames: 10 packets of 216 bytes, no RTP, no header
        assert_eq!(out, vec![(216, false); 10]);
        // Other codecs flush at the end of every round
        let mut p = Packetizer::new(CodecKind::Sbc, 1023, 48000);
        p.push(&[0x9Cu8; 119], 1, 0, &mut |_| {});
        assert_eq!(p.end_round(&mut |_| {}), 1);
    }

    #[test]
    fn frame_count_fits_four_bits() {
        // Small mono SBC frames: 46 bytes, 22 would fit 1023 bytes
        let mut p = Packetizer::new(CodecKind::Sbc, 1023, 48000);
        let mut out = Vec::new();
        for i in 0..20 {
            p.push(&[0u8; 46], 1, i * 128, &mut |pk| out.push(pk.payload()[0]));
        }
        p.flush(&mut |pk| out.push(pk.payload()[0]));
        assert_eq!(out, vec![15, 5]);
    }

    #[test]
    fn lhdc_header_and_sequence() {
        let mut p = Packetizer::new(CodecKind::LhdcV5, 1023, 48000);
        let mut out = Vec::new();
        p.push(&[1, 2, 3], 3, 0, &mut |pk| out.push(pk.payload().to_vec()));
        p.flush(&mut |pk| out.push(pk.payload().to_vec()));
        p.push(&[4], 1, 720, &mut |pk| out.push(pk.payload().to_vec()));
        p.flush(&mut |pk| out.push(pk.payload().to_vec()));
        assert_eq!(out[0], vec![3 << 2, 0, 1, 2, 3]);
        assert_eq!(out[1], vec![1 << 2, 1, 4]);
    }

    #[test]
    fn opus_one_frame_per_packet() {
        // Small (low bit rate) Opus frames are not merged; Opus 05 the same
        for kind in [CodecKind::Opus, CodecKind::Opus05] {
            let mut p = Packetizer::new(kind, 1011, 48000);
            let mut out = Vec::new();
            p.push(&[0xFC, 1, 2], 1, 0, &mut |pk| out.push((pk.payload().to_vec(), pk.timestamp)));
            p.push(&[0xFC, 3], 1, 960, &mut |pk| out.push((pk.payload().to_vec(), pk.timestamp)));
            p.flush(&mut |pk| out.push((pk.payload().to_vec(), pk.timestamp)));
            assert_eq!(out, vec![(vec![1, 0xFC, 1, 2], 0), (vec![1, 0xFC, 3], 960)], "{kind}");
            assert_eq!(max_raw(kind, 1011, 48000), 1010);
        }
    }

    #[test]
    fn aptx_adaptive_packets_go_out_as_encoded() {
        // The encoder's RTP header and codec packet, no media payload header,
        // no BTstack RTP header, one packet per output
        let mut p = Packetizer::new(CodecKind::AptxAdaptive, 1011, 48000);
        let mut out = Vec::new();
        let rtp = [0x80, 0x60, 0, 7, 0, 0, 0x02, 0xA0, 0, 0, 0, 0xA3];
        let packet: Vec<u8> = rtp.iter().copied().chain([0x81, 0x00, 0xD0, 0xA1]).collect();
        p.push(&packet, 1, 0, &mut |pk| out.push((pk.payload().to_vec(), pk.rtp)));
        p.push(&packet, 1, 672, &mut |pk| out.push((pk.payload().to_vec(), pk.rtp)));
        assert_eq!(p.end_round(&mut |pk| out.push((pk.payload().to_vec(), pk.rtp))), 0);
        assert_eq!(out, vec![(packet.clone(), false), (packet, false)]);
        assert_eq!(max_raw(CodecKind::AptxAdaptive, 1011, 48000), 1023);
    }

    #[test]
    fn aac_one_frame_per_packet() {
        // Small AAC frames (silence) are not merged into one packet
        let mut p = Packetizer::new(CodecKind::Aac, 660, 48000);
        let mut out = Vec::new();
        p.push(&[0x47, 1, 2], 1, 0, &mut |pk| out.push((pk.payload().to_vec(), pk.timestamp)));
        p.push(&[0x47, 3], 1, 1024, &mut |pk| out.push((pk.payload().to_vec(), pk.timestamp)));
        p.flush(&mut |pk| out.push((pk.payload().to_vec(), pk.timestamp)));
        assert_eq!(out, vec![(vec![0x47, 1, 2], 0), (vec![0x47, 3], 1024)]);
    }

    #[test]
    fn aptx_ll_packets_are_short_and_whole_frames() {
        assert_eq!(max_raw(CodecKind::AptxLl, 1023, 48000), 360);
        assert_eq!(max_raw(CodecKind::Aptx, 1023, 48000), 1032);
        assert_eq!(max_raw(CodecKind::Aptx, 1679, 48000), 1688);
        assert!(!CodecKind::Aptx.uses_rtp());
    }
}
