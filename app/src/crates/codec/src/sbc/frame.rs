// Ported from Bluedroid's SBC encoder (sbc_encoder.c, sbc_encoder.h):
//
//   Copyright (C) 1999-2012 Broadcom Corporation
//
//   Licensed under the Apache License, Version 2.0 (the "License");
//   you may not use this file except in compliance with the License.
//   You may obtain a copy of the License at:
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
//   Unless required by applicable law or agreed to in writing, software
//   distributed under the License is distributed on an "AS IS" BASIS,
//   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//   See the License for the specific language governing permissions and
//   limitations under the License.

//! The general SBC frame encoder (SBC_Encoder_Init / SBC_Encoder): any
//! sampling frequency, channel mode, block and subband count, allocation
//! method and bitpool the SBC specification allows.

use super::analysis::Analysis;
use super::bit_alloc::{self, AllocParams};
use super::packing::{self, Frame};
use crate::{CodecKind, EncoderError};

const MAX_SUBBANDS: usize = 8;
const MAX_CHANNELS: usize = 2;
const MAX_BLOCKS: usize = 16;

/// SBC channel mode (the values are the 2-bit header field)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelMode {
    Mono = 0,
    DualChannel = 1,
    Stereo = 2,
    JointStereo = 3,
}

impl ChannelMode {
    pub const ALL: [ChannelMode; 4] =
        [ChannelMode::Mono, ChannelMode::DualChannel, ChannelMode::Stereo, ChannelMode::JointStereo];

    pub fn channels(self) -> usize {
        if self == ChannelMode::Mono { 1 } else { 2 }
    }

    /// Stereo and joint stereo share the bitpool between the channels
    fn shared_bitpool(self) -> bool {
        matches!(self, ChannelMode::Stereo | ChannelMode::JointStereo)
    }
}

/// SBC bit allocation method (the values are the header bit)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Allocation {
    Loudness = 0,
    Snr = 1,
}

/// Configuration of [`SbcFrameEncoder`]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SbcConfig {
    /// 16000, 32000, 44100 or 48000
    pub sample_rate: u32,
    pub channel_mode: ChannelMode,
    /// 4, 8, 12 or 16
    pub blocks: u8,
    /// 4 or 8
    pub subbands: u8,
    pub allocation: Allocation,
    /// Values above what the mode allows (16 * subbands per channel for mono
    /// and dual channel, 32 * subbands for stereo and joint stereo, at most
    /// 255) are lowered to it, as Bluedroid does
    pub bitpool: u8,
}

impl SbcConfig {
    pub fn channels(&self) -> usize {
        self.channel_mode.channels()
    }

    /// PCM frames (samples per channel) of one SBC frame
    pub fn pcm_frames_per_frame(&self) -> usize {
        self.blocks as usize * self.subbands as usize
    }

    /// The bitpool the encoder uses (SBC_Encoder_Init's limits)
    pub fn effective_bitpool(&self) -> u8 {
        let max = if self.channel_mode.shared_bitpool() {
            if self.subbands == 8 { 255 } else { 128 }
        } else {
            16 * self.subbands as u32
        };
        (self.bitpool as u32).min(max) as u8
    }

    /// Frame length in bytes by the formula of the A2DP specification
    /// (12.9), with the effective bitpool. The encoder's frames have this
    /// length (bitpool 0 aside, see [`SbcFrameEncoder::max_frame_len`]).
    pub fn frame_len(&self) -> usize {
        let ch = self.channels();
        let subbands = self.subbands as usize;
        let blocks = self.blocks as usize;
        let bitpool = self.effective_bitpool() as usize;
        let data_bits = match self.channel_mode {
            ChannelMode::Mono | ChannelMode::DualChannel => blocks * ch * bitpool,
            ChannelMode::Stereo => blocks * bitpool,
            ChannelMode::JointStereo => subbands + blocks * bitpool,
        };
        4 + (4 * subbands * ch) / 8 + data_bits.div_ceil(8)
    }

    fn validate(&self) -> Result<usize, EncoderError> {
        let unsupported = |what: String| Err(EncoderError::Unsupported { codec: CodecKind::Sbc, what });
        let freq_index = match self.sample_rate {
            16000 => 0,
            32000 => 1,
            44100 => 2,
            48000 => 3,
            r => return unsupported(format!("sample rate {r} Hz")),
        };
        if !matches!(self.blocks, 4 | 8 | 12 | 16) {
            return unsupported(format!("{} blocks", self.blocks));
        }
        if !matches!(self.subbands, 4 | 8) {
            return unsupported(format!("{} subbands", self.subbands));
        }
        Ok(freq_index)
    }
}

/// The Bluedroid SBC encoder, bit-exact: one SBC frame per
/// [`encode`](SbcFrameEncoder::encode). Holds the analysis filter history,
/// so a stream goes through one instance.
pub struct SbcFrameEncoder {
    config: SbcConfig,
    channels: usize,
    subbands: usize,
    blocks: usize,
    bitpool: u8,
    header: u8,
    alloc: AllocParams,
    analysis: Analysis,
    /// Subband samples per block, channel, subband (s32SbBuffer)
    sb: [i32; MAX_CHANNELS * MAX_SUBBANDS * MAX_BLOCKS],
    scale_factor: [i32; MAX_CHANNELS * MAX_SUBBANDS],
    bits: [i32; MAX_CHANNELS * MAX_SUBBANDS],
    bitneed: [i32; MAX_CHANNELS * MAX_SUBBANDS],
    join: [bool; MAX_SUBBANDS],
    lr_sum: [i32; MAX_BLOCKS],
    lr_diff: [i32; MAX_BLOCKS],
}

/// The scale factor (log2 of the peak over 2^15, 0..=15) for a block peak
#[inline(always)]
fn scale_factor(peak: i32) -> u32 {
    // C starts at 9 above 0x800000 only to save iterations
    let mut count = if peak > 0x80_0000 { 9 } else { 0 };
    while count < 15 && peak > (0x8000 << count) {
        count += 1;
    }
    count
}

impl SbcFrameEncoder {
    pub fn new(config: SbcConfig) -> Result<SbcFrameEncoder, EncoderError> {
        let freq_index = config.validate()?;
        let channels = config.channels();
        let subbands = config.subbands as usize;
        let bitpool = config.effective_bitpool();
        let header = ((freq_index as u8 & 3) << 6)
            | (((config.blocks - 4) & 12) << 2)
            | ((config.channel_mode as u8 & 3) << 2)
            | ((config.allocation as u8 & 1) << 1)
            | ((config.subbands >> 3) & 1);
        Ok(SbcFrameEncoder {
            config,
            channels,
            subbands,
            blocks: config.blocks as usize,
            bitpool,
            header,
            alloc: AllocParams {
                subbands,
                freq_index,
                snr: config.allocation == Allocation::Snr,
                bitpool: bitpool as i32,
            },
            analysis: Analysis::new(subbands, channels),
            sb: [0; MAX_CHANNELS * MAX_SUBBANDS * MAX_BLOCKS],
            scale_factor: [0; MAX_CHANNELS * MAX_SUBBANDS],
            bits: [0; MAX_CHANNELS * MAX_SUBBANDS],
            bitneed: [0; MAX_CHANNELS * MAX_SUBBANDS],
            join: [false; MAX_SUBBANDS],
            lr_sum: [0; MAX_BLOCKS],
            lr_diff: [0; MAX_BLOCKS],
        })
    }

    pub fn config(&self) -> &SbcConfig {
        &self.config
    }

    /// Encodes the next frames with another bitpool (lowered to what the
    /// mode allows, as [`SbcConfig::bitpool`]). Every SBC frame carries its
    /// bitpool in its header, so a stream may change it between frames; the
    /// analysis filter history stays.
    pub fn set_bitpool(&mut self, bitpool: u8) {
        self.config.bitpool = bitpool;
        self.bitpool = self.config.effective_bitpool();
        self.alloc.bitpool = self.bitpool as i32;
    }

    /// Interleaved samples one frame takes
    pub fn pcm_samples(&self) -> usize {
        self.blocks * self.subbands * self.channels
    }

    /// Largest frame [`encode`](SbcFrameEncoder::encode) writes: the A2DP
    /// formula's length, plus one with bitpool 0 (Bluedroid then ends a
    /// byte-aligned frame with an extra zero byte)
    pub fn max_frame_len(&self) -> usize {
        self.config.frame_len() + usize::from(self.bitpool == 0)
    }

    /// Encodes one frame from `pcm` ([`pcm_samples`](Self::pcm_samples)
    /// interleaved samples) into `out` (at least
    /// [`max_frame_len`](Self::max_frame_len) bytes); returns its length
    pub fn encode(&mut self, pcm: &[i16], out: &mut [u8]) -> Result<usize, EncoderError> {
        let expected = self.pcm_samples();
        if pcm.len() != expected {
            return Err(EncoderError::InputLength { expected, got: pcm.len() });
        }
        let need = self.max_frame_len();
        if out.len() < need {
            return Err(EncoderError::OutputTooSmall { have: out.len(), need });
        }

        let nsc = self.channels * self.subbands;
        let sb = &mut self.sb[..self.blocks * nsc];
        self.analysis.filter(pcm, self.subbands, self.channels, self.blocks, sb);

        // Scale factors per channel and subband. C keeps the largest
        // (maxBit) for SNR allocation in stereo; joint stereo below only
        // ever raises it.
        let mut max_bit = 0;
        for (i, scf) in self.scale_factor[..nsc].iter_mut().enumerate() {
            // abs32 of i32::MIN stays negative in C (MSVC wraps), so it never
            // raises the peak; wrapping_abs and max behave the same
            let peak = sb.iter().skip(i).step_by(nsc).fold(0, |m, &v| m.max(v.wrapping_abs()));
            let count = scale_factor(peak);
            *scf = count as i32;
            max_bit = max_bit.max(count);
        }

        if self.config.channel_mode == ChannelMode::JointStereo {
            self.joint_stereo(&mut max_bit);
        }

        match self.config.channel_mode {
            ChannelMode::Stereo | ChannelMode::JointStereo => {
                bit_alloc::stereo(self.alloc, max_bit as i32, &self.scale_factor, &mut self.bitneed, &mut self.bits)
            }
            ChannelMode::Mono | ChannelMode::DualChannel => {
                bit_alloc::mono(self.alloc, self.channels, &self.scale_factor, &mut self.bitneed, &mut self.bits)
            }
        }

        let frame = Frame {
            header: self.header,
            bitpool: self.bitpool,
            joint: self.config.channel_mode == ChannelMode::JointStereo,
            join: &self.join,
            channels: self.channels,
            subbands: self.subbands,
            blocks: self.blocks,
            scale_factor: &self.scale_factor,
            bits: &self.bits,
            sb: &self.sb,
        };
        Ok(packing::pack(&frame, out))
    }

    /// Per subband (the last one excepted, as in C), codes sum and
    /// difference instead of left and right when their scale factors add up
    /// to less
    fn joint_stereo(&mut self, max_bit: &mut u32) {
        let n = self.subbands;
        let nsc = 2 * n;
        let blocks = self.blocks;
        for s in 0..n - 1 {
            let mut max_sum = 0;
            let mut max_diff = 0;
            for blk in 0..blocks {
                let l = self.sb[blk * nsc + s];
                let r = self.sb[blk * nsc + n + s];
                let sum = l.wrapping_add(r) >> 1;
                let diff = l.wrapping_sub(r) >> 1;
                self.lr_sum[blk] = sum;
                self.lr_diff[blk] = diff;
                max_sum = max_sum.max(sum.wrapping_abs());
                max_diff = max_diff.max(diff.wrapping_abs());
            }
            let count_sum = scale_factor(max_sum);
            let count_diff = scale_factor(max_diff);
            if self.scale_factor[s] + self.scale_factor[n + s] > (count_sum + count_diff) as i32 {
                *max_bit = (*max_bit).max(count_sum).max(count_diff);
                self.scale_factor[s] = count_sum as i32;
                self.scale_factor[n + s] = count_diff as i32;
                for blk in 0..blocks {
                    self.sb[blk * nsc + s] = self.lr_sum[blk];
                    self.sb[blk * nsc + n + s] = self.lr_diff[blk];
                }
                self.join[s] = true;
            } else {
                self.join[s] = false;
            }
        }
        self.join[n - 1] = false;
    }
}
