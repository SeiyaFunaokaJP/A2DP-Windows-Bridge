//! The LE Audio (experimental) encode thread (v1 Stream's encoding half,
//! le_audio_stream.cpp):
//!
//! capture thread --[`CaptureInput`] (the same SPSC byte ring as A2DP)-->
//! LE encode thread --[`SduSink`] (bt-host's `LeSduSink`, the jitter
//! buffer [`le_audio::sdu_queue::SduQueue`])--> BTstack thread (ISO)
//!
//! Any captured PCM is downmixed to stereo (ITU-R BS.775 for more than two
//! channels, as v1), resampled to the stream's LC3 rate if it differs
//! (linear interpolation; the capture is normally asked for that rate
//! already), cut into LC3 frames (10 or 7.5 ms) and encoded once per output
//! channel: Left / Right take that side, Mono the average of both. The
//! frames of one interval make one SDU set: one SDU per CIS in plan order,
//! each holding its channels' frames in ascending Audio Location bit order
//! (BAP: the channels of a CIS's SDU are ordered by location).
//!
//! No allocation per round once a stream runs: the frame buffers are
//! preallocated and the sets are encoded into spare sets of the queue
//! ([`SduSink::spare`]).

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use bt_host::{LeSduSink, LinkStats};
use codec::lc3::{Lc3ChannelEncoder, Lc3Error, Lc3Params};
use le_audio::plan::{ChannelSource, StreamPlan};
use le_audio::sdu_queue::{SduQueue, SduSet};

use crate::{BufferPreset, CaptureInput, Control, InputFormat, SampleType};

/// Where the LE encode thread puts its SDU sets
pub trait SduSink: Send + 'static {
    /// One SDU per CIS for the next frame interval, in plan order
    fn push(&self, set: SduSet);

    /// A set to encode into (a sent one, given back by the sender), so that
    /// no round allocates; None: the encoder allocates one
    fn spare(&self) -> Option<SduSet> {
        None
    }
}

impl SduSink for LeSduSink {
    fn push(&self, set: SduSet) {
        LeSduSink::push(self, set);
    }

    fn spare(&self) -> Option<SduSet> {
        LeSduSink::spare(self)
    }
}

/// The jitter buffer itself (bt-host's SharedSduQueue; tests)
impl SduSink for Arc<Mutex<SduQueue>> {
    fn push(&self, set: SduSet) {
        self.lock().unwrap_or_else(|e| e.into_inner()).push(set);
    }

    fn spare(&self) -> Option<SduSet> {
        self.lock().unwrap_or_else(|e| e.into_inner()).spare()
    }
}

/// Where one output channel's LC3 frame goes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelSlot {
    /// CIS index (plan order) = SDU index in the set
    pub cis: usize,
    /// Byte offset of the frame in that SDU
    pub offset: usize,
    /// Audio location bit (0 = mono)
    pub allocation: u32,
    pub source: ChannelSource,
}

/// The LC3 parameters and SDU layout of a stream plan
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SduLayout {
    pub params: Lc3Params,
    /// PCM samples per frame and channel
    pub frame_samples: usize,
    /// SDU length per CIS (plan order): octets x channels
    pub sdu_lens: Vec<usize>,
    /// The output channels, one LC3 encoder each, CIS by CIS
    pub channels: Vec<ChannelSlot>,
}

impl SduLayout {
    pub fn from_plan(plan: &StreamPlan) -> Result<SduLayout, String> {
        let preset = &plan.preset;
        let params =
            Lc3Params { sample_rate: preset.sampling_hz, frame_us: u32::from(preset.frame_us), octets: preset.octets };
        let frame_samples = params.frame_samples().map_err(|e| {
            format!("LC3 {} Hz, {} us, {} octets: {e}", params.sample_rate, params.frame_us, params.octets)
        })?;
        if plan.cis.is_empty() {
            return Err("no CIS in the stream plan".into());
        }
        let octets = usize::from(params.octets);
        let mut sdu_lens = Vec::with_capacity(plan.cis.len());
        let mut channels = Vec::with_capacity(plan.cis.iter().map(|c| c.channels.len()).sum());
        for (c, cis) in plan.cis.iter().enumerate() {
            if cis.channels.is_empty() {
                return Err(format!("CIS {} has no channel", cis.cis_id));
            }
            // SDU order: ascending location bit (the plan lists them so;
            // sorted again here so the layout never depends on it)
            let mut order: Vec<_> = cis.channels.iter().collect();
            order.sort_by_key(|ch| ch.allocation);
            for (pos, ch) in order.into_iter().enumerate() {
                channels.push(ChannelSlot {
                    cis: c,
                    offset: pos * octets,
                    allocation: ch.allocation,
                    source: ch.source,
                });
            }
            sdu_lens.push(octets * cis.channels.len());
        }
        Ok(SduLayout { params, frame_samples, sdu_lens, channels })
    }

    /// A zeroed set of the right shape
    pub fn empty_set(&self) -> SduSet {
        self.sdu_lens.iter().map(|&n| vec![0; n]).collect()
    }

    fn fits(&self, set: &SduSet) -> bool {
        set.len() == self.sdu_lens.len() && set.iter().zip(&self.sdu_lens).all(|(s, &n)| s.len() == n)
    }
}

/// The silence set for bt-host's `le_start_stream` (`silence` closure):
/// one SDU per CIS, every channel an LC3 frame of digital silence (v1 Stream
/// encoded it once per stream). Zero bytes if the plan cannot be encoded
/// (then the stream fails in [`LePipeline::start`] anyway).
pub fn silence_set(plan: &StreamPlan) -> SduSet {
    let layout = match SduLayout::from_plan(plan) {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("LE Audio: no silence SDUs: {e}");
            return plan.cis.iter().map(|c| vec![0; usize::from(plan.preset.octets) * c.channels.len()]).collect();
        }
    };
    let octets = usize::from(layout.params.octets);
    let mut frame = vec![0u8; octets];
    if let Err(e) = codec::lc3::silence_frame(layout.params, &mut frame) {
        tracing::error!("LE Audio: LC3 silence: {e}");
    }
    let mut set = layout.empty_set();
    for ch in &layout.channels {
        set[ch.cis][ch.offset..ch.offset + octets].copy_from_slice(&frame);
    }
    set
}

/// -3 dB (v1: 0.707107f)
const MINUS_3DB: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// Stereo from one input frame's channels (v1: mono on both sides; more
/// than two channels downmixed as ITU-R BS.775 in WASAPI's channel order FL
/// FR FC LFE BL BR SL SR, channels beyond eight ignored)
pub fn downmix(ch: &[f32]) -> (f32, f32) {
    let at = |i: usize| ch.get(i).copied().unwrap_or(0.0);
    match ch.len() {
        0 => (0.0, 0.0),
        1 => (ch[0], ch[0]),
        2 => (ch[0], ch[1]),
        _ => {
            let (c, lfe) = (at(2), at(3));
            let (bl, br, sl, sr) = (at(4), at(5), at(6), at(7));
            let l = ch[0] + MINUS_3DB * c + MINUS_3DB * (bl + sl) + 0.5 * lfe;
            let r = ch[1] + MINUS_3DB * c + MINUS_3DB * (br + sr) + 0.5 * lfe;
            (l, r)
        }
    }
}

/// Linear interpolation from the capture rate to the LC3 rate (v1
/// push_pcm). Each input sample yields the output samples between the
/// previous input sample (position 0) and this one (position 1).
#[derive(Debug, Clone, PartialEq)]
pub struct LinearResampler {
    /// Input samples per output sample
    step: f64,
    passthrough: bool,
    /// Fractional input position of the next output sample
    pos: f64,
    last: (f32, f32),
}

impl LinearResampler {
    pub fn new(in_rate: u32, out_rate: u32) -> LinearResampler {
        LinearResampler {
            step: f64::from(in_rate) / f64::from(out_rate.max(1)),
            passthrough: in_rate == out_rate,
            pos: 0.0,
            last: (0.0, 0.0),
        }
    }

    pub fn passthrough(&self) -> bool {
        self.passthrough
    }

    /// One input sample pair; `out` gets the output sample pairs it yields
    #[inline]
    pub fn process(&mut self, l: f32, r: f32, out: &mut impl FnMut(f32, f32)) {
        if self.passthrough {
            out(l, r);
            return;
        }
        let (last_l, last_r) = self.last;
        while self.pos < 1.0 {
            let a = self.pos as f32;
            out(last_l + (l - last_l) * a, last_r + (r - last_r) * a);
            self.pos += self.step;
        }
        self.pos -= 1.0;
        self.last = (l, r);
    }

    pub fn reset(&mut self) {
        self.pos = 0.0;
        self.last = (0.0, 0.0);
    }
}

/// Float to 16-bit as v1 (clamped, lrintf: rounded to nearest, ties even)
#[inline]
fn to_i16(x: f32) -> i16 {
    (x.clamp(-1.0, 1.0) * 32767.0).round_ties_even() as i16
}

#[inline]
fn input_sample(raw: &[u8], i: usize, t: SampleType) -> f32 {
    match t {
        SampleType::F32 => f32::from_le_bytes([raw[i * 4], raw[i * 4 + 1], raw[i * 4 + 2], raw[i * 4 + 3]]),
        SampleType::I16 => f32::from(i16::from_le_bytes([raw[i * 2], raw[i * 2 + 1]])) / 32768.0,
        SampleType::I32 => {
            i32::from_le_bytes([raw[i * 4], raw[i * 4 + 1], raw[i * 4 + 2], raw[i * 4 + 3]]) as f32 / 2_147_483_648.0
        }
    }
}

/// One LC3 frame being filled, and its encoding into a set
struct FrameEncoder {
    layout: SduLayout,
    encoders: Vec<Lc3ChannelEncoder>,
    l: Vec<f32>,
    r: Vec<f32>,
    fill: usize,
    pcm16: Vec<i16>,
    sets: u64,
    errors: u64,
    clock: crate::EncodeClock,
}

impl FrameEncoder {
    #[inline]
    fn add(&mut self, l: f32, r: f32, sink: &dyn SduSink) {
        self.l[self.fill] = l;
        self.r[self.fill] = r;
        self.fill += 1;
        if self.fill == self.layout.frame_samples {
            self.fill = 0;
            self.encode_frame(sink);
        }
    }

    fn encode_frame(&mut self, sink: &dyn SduSink) {
        let mut set = match sink.spare() {
            Some(s) if self.layout.fits(&s) => s,
            _ => self.layout.empty_set(),
        };
        let octets = usize::from(self.layout.params.octets);
        let started = Instant::now();
        for (slot, enc) in self.layout.channels.iter().zip(&mut self.encoders) {
            for (s, (&l, &r)) in self.pcm16.iter_mut().zip(self.l.iter().zip(&self.r)) {
                *s = to_i16(match slot.source {
                    ChannelSource::Left => l,
                    ChannelSource::Right => r,
                    ChannelSource::Mono => 0.5 * (l + r),
                });
            }
            let out = &mut set[slot.cis][slot.offset..slot.offset + octets];
            if let Err(e) = enc.encode(&self.pcm16, out) {
                encode_error(&mut self.errors, e);
                out.fill(0);
            }
        }
        self.clock.add(started.elapsed(), u64::from(self.layout.params.frame_us) * 1000);
        self.sets += 1;
        tracing::trace!("LE encode thread: SDU set {} ({} SDUs)", self.sets, set.len());
        sink.push(set);
    }
}

#[cold]
fn encode_error(errors: &mut u64, e: Lc3Error) {
    *errors += 1;
    if *errors == 1 || errors.is_multiple_of(1000) {
        tracing::error!("LE encode thread: {e} ({errors} errors)");
    }
}

/// Downmix, resampling and LC3 encoding of a stream (no thread; the LE
/// encode thread drives it, tests call it directly)
pub struct LeEncoder {
    input: InputFormat,
    resampler: LinearResampler,
    frame: FrameEncoder,
}

impl LeEncoder {
    /// Encoders for the plan's channels, for audio arriving in `input`
    pub fn new(plan: &StreamPlan, input: InputFormat) -> Result<LeEncoder, String> {
        if input.channels == 0 || input.sample_rate == 0 {
            return Err(format!("capture format {} Hz, {} channels", input.sample_rate, input.channels));
        }
        let layout = SduLayout::from_plan(plan)?;
        let encoders = layout
            .channels
            .iter()
            .map(|_| Lc3ChannelEncoder::new(layout.params))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                format!(
                    "LE Audio: LC3 encoder configuration failed ({} Hz, {} octets): {e}",
                    layout.params.sample_rate, layout.params.octets
                )
            })?;
        let ns = layout.frame_samples;
        let rate = layout.params.sample_rate;
        tracing::info!(
            "LE Audio: LC3 encoders ready: {rate} Hz, {} us frames ({ns} samples), {} octets, {} channel(s)",
            layout.params.frame_us,
            layout.params.octets,
            layout.channels.len()
        );
        if input.sample_rate != rate {
            tracing::info!("LE Audio: capture at {} Hz, LC3 at {rate} Hz (resampling)", input.sample_rate);
        }
        tracing::debug!(
            "LE Audio: input {:?}, {}, SDU lengths {:?} (plan cis ids {:?})",
            input,
            match input.channels {
                1 => "mono on both sides",
                2 => "stereo",
                _ => "downmixed to stereo (ITU-R BS.775)",
            },
            layout.sdu_lens,
            plan.cis.iter().map(|c| c.cis_id).collect::<Vec<_>>()
        );
        for ch in &layout.channels {
            tracing::debug!(
                "LE Audio: channel {:?} -> CIS #{} offset {}, location 0x{:08x}",
                ch.source,
                ch.cis,
                ch.offset,
                ch.allocation
            );
        }
        Ok(LeEncoder {
            input,
            resampler: LinearResampler::new(input.sample_rate, rate),
            frame: FrameEncoder {
                layout,
                encoders,
                l: vec![0.0; ns],
                r: vec![0.0; ns],
                fill: 0,
                pcm16: vec![0; ns],
                sets: 0,
                errors: 0,
                clock: crate::EncodeClock::default(),
            },
        })
    }

    pub fn layout(&self) -> &SduLayout {
        &self.frame.layout
    }

    /// SDU sets encoded so far
    pub fn sets_encoded(&self) -> u64 {
        self.frame.sets
    }

    /// Captured audio (whole frames of the input format): every LC3 frame
    /// it completes goes to `sink` as one SDU set
    pub fn push(&mut self, raw: &[u8], sink: &dyn SduSink) {
        let in_ch = usize::from(self.input.channels);
        let t = self.input.sample;
        let used = in_ch.min(8);
        let mut ch = [0.0f32; 8];
        let Self { resampler, frame, .. } = self;
        for f in 0..raw.len() / (in_ch * t.bytes()) {
            for (c, v) in ch[..used].iter_mut().enumerate() {
                *v = input_sample(raw, f * in_ch + c, t);
            }
            let (l, r) = downmix(&ch[..used]);
            resampler.process(l, r, &mut |l, r| frame.add(l, r, sink));
        }
    }

    /// Drops the partly filled frame and the resampler's history (buffered
    /// audio of an old stream)
    pub fn reset(&mut self) {
        self.frame.fill = 0;
        self.resampler.reset();
    }
}

/// Setup of the LE encode thread
pub struct LePipelineConfig {
    /// The stream (preset and CIS / channel layout)
    pub plan: StreamPlan,
    pub input: InputFormat,
    pub buffer: BufferPreset,
}

/// A running LE encode thread
pub struct LePipeline {
    control: Arc<Control>,
    thread: Option<JoinHandle<()>>,
    layout: SduLayout,
}

impl LePipeline {
    /// Starts the LE encode thread; feed it through the returned
    /// [`CaptureInput`] as the A2DP pipeline
    pub fn start(
        config: LePipelineConfig,
        sink: impl SduSink,
        stats: Arc<LinkStats>,
    ) -> Result<(LePipeline, CaptureInput), String> {
        let LePipelineConfig { plan, input, buffer } = config;
        let encoder = LeEncoder::new(&plan, input)?;
        let layout = encoder.layout().clone();
        let ring = buffer.ring_bytes(&input);
        tracing::debug!(
            "LE pipeline: start, buffer {buffer:?}: capture ring {ring} bytes ({} ms)",
            (ring / input.frame_bytes().max(1)) as u64 * 1000 / u64::from(input.sample_rate.max(1))
        );
        let (producer, consumer) = rtrb::RingBuffer::new(ring);
        let control = Control::new();
        let thread = {
            let control = control.clone();
            let stats = stats.clone();
            std::thread::Builder::new()
                .name("le-encode".into())
                .spawn(move || le_encode_thread(encoder, consumer, sink, &stats, &control))
                .map_err(|e| format!("spawning the LE encode thread: {e}"))?
        };
        let capture = CaptureInput {
            producer,
            encode_thread: thread.thread().clone(),
            stats,
            control: control.clone(),
            frame_bytes: input.frame_bytes(),
        };
        Ok((LePipeline { control, thread: Some(thread), layout }, capture))
    }

    pub fn layout(&self) -> &SduLayout {
        &self.layout
    }

    /// Drops audio buffered for the old stream (after the stream was set up
    /// again)
    pub fn reset(&self) {
        tracing::debug!("LE pipeline: reset requested");
        self.control.reset.store(true, Ordering::Release);
        if let Some(t) = &self.thread {
            t.thread().unpark();
        }
    }

    /// Stops and joins the encode thread
    pub fn stop(mut self) {
        self.stop_thread();
    }

    fn stop_thread(&mut self) {
        if let Some(t) = self.thread.take() {
            self.control.stop.store(true, Ordering::Release);
            t.thread().unpark();
            let _ = t.join();
        }
    }
}

impl Drop for LePipeline {
    fn drop(&mut self) {
        self.stop_thread();
    }
}

fn le_encode_thread(
    mut encoder: LeEncoder,
    mut ring: rtrb::Consumer<u8>,
    sink: impl SduSink,
    stats: &LinkStats,
    control: &Control,
) {
    let _priority = crate::thread_priority();
    let frame_bytes = encoder.input.frame_bytes();
    // ~10 ms rounds, at least 128 frames (as the A2DP encode thread)
    let round_frames = (encoder.input.sample_rate as usize / 100).max(128);
    let mut read_buf = vec![0u8; round_frames * frame_bytes];
    let mut overflow_report = Instant::now();
    tracing::debug!(
        "LE encode thread: running, rounds of {round_frames} frames, priority hook {}",
        _priority.is_some()
    );
    // Debug summary window: start, rounds, frames, rounds behind, sets
    // encoded and errors at its start
    let mut window = (Instant::now(), 0u64, 0u64, 0u64, encoder.frame.sets, encoder.frame.errors);
    while !control.stop.load(Ordering::Acquire) {
        if control.reset.swap(false, Ordering::AcqRel) {
            tracing::debug!("LE encode thread: reset, dropping {} buffered bytes", ring.slots());
            let n = ring.slots();
            if let Ok(chunk) = ring.read_chunk(n) {
                chunk.commit_all();
            }
            encoder.reset();
        }
        control.report_overflows("LE encode thread", &mut overflow_report);
        if window.0.elapsed() >= crate::SUMMARY_INTERVAL {
            tracing::debug!(
                "LE encode thread: last {:.1} s: {} rounds, {} frames, {} behind rounds, {} SDU sets, {} encode \
                 errors; {} sets in total",
                window.0.elapsed().as_secs_f64(),
                window.1,
                window.2,
                window.3,
                encoder.frame.sets - window.4,
                encoder.frame.errors - window.5,
                encoder.frame.sets
            );
            window = (Instant::now(), 0, 0, 0, encoder.frame.sets, encoder.frame.errors);
        }
        let available = ring.slots() / frame_bytes;
        if available == 0 {
            std::thread::park_timeout(Duration::from_millis(10));
            continue;
        }
        window.1 += 1;
        window.2 += available.min(round_frames) as u64;
        window.3 += u64::from(available > round_frames);
        let bytes = available.min(round_frames) * frame_bytes;
        if let Ok(chunk) = ring.read_chunk(bytes) {
            let (a, b) = chunk.as_slices();
            read_buf[..a.len()].copy_from_slice(a);
            read_buf[a.len()..bytes].copy_from_slice(b);
            chunk.commit_all();
        }
        encoder.push(&read_buf[..bytes], &sink);
        encoder.frame.clock.flush(stats);
    }
    tracing::debug!(
        "LE encode thread: stopped after {} SDU sets, {} encode errors",
        encoder.frame.sets,
        encoder.frame.errors
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use le_audio::defs::{LOCATION_FRONT_LEFT, LOCATION_FRONT_RIGHT};
    use le_audio::plan::{ChannelPlan, CisPlan};
    use le_audio::presets::StreamPreset;

    pub(crate) fn preset(rate: u32, frame_us: u16, octets: u16) -> StreamPreset {
        StreamPreset {
            name: "test".into(),
            sampling_hz: rate,
            frame_us,
            octets,
            target_latency: 2,
            framing: 0,
            rtn: 2,
            max_transport_latency_ms: 10,
            presentation_delay_us: 40000,
        }
    }

    fn cis(id: u8, channels: &[(u32, ChannelSource)]) -> CisPlan {
        CisPlan {
            device_id: u32::from(id),
            ase_id: id,
            cis_id: id,
            acl: 0x40 + u16::from(id),
            cis_handle: 0x60 + u16::from(id),
            channels: channels.iter().map(|&(allocation, source)| ChannelPlan { allocation, source }).collect(),
        }
    }

    fn plan(preset: StreamPreset, cis: Vec<CisPlan>) -> StreamPlan {
        StreamPlan { preset, cig_id: 1, phy: 2, requested_contexts: 4, contexts: 4, presentation_delay_us: 40000, cis }
    }

    const L: (u32, ChannelSource) = (LOCATION_FRONT_LEFT, ChannelSource::Left);
    const R: (u32, ChannelSource) = (LOCATION_FRONT_RIGHT, ChannelSource::Right);
    const MONO: (u32, ChannelSource) = (0, ChannelSource::Mono);

    fn f32_bytes(v: &[f32]) -> Vec<u8> {
        v.iter().flat_map(|s| s.to_le_bytes()).collect()
    }

    #[test]
    fn downmix_mono_stereo_and_surround() {
        assert_eq!(downmix(&[0.25]), (0.25, 0.25));
        assert_eq!(downmix(&[0.25, -0.5]), (0.25, -0.5));
        // 5.1: FL FR FC LFE BL BR
        let (l, r) = downmix(&[0.1, 0.2, 0.3, 0.4, 0.05, 0.06]);
        assert!((l - (0.1 + MINUS_3DB * 0.3 + MINUS_3DB * 0.05 + 0.5 * 0.4)).abs() < 1e-6);
        assert!((r - (0.2 + MINUS_3DB * 0.3 + MINUS_3DB * 0.06 + 0.5 * 0.4)).abs() < 1e-6);
        // 7.1 adds SL / SR to the back channels
        let (l, r) = downmix(&[0.0, 0.0, 0.0, 0.0, 0.1, 0.2, 0.3, 0.4]);
        assert!((l - MINUS_3DB * 0.4).abs() < 1e-6);
        assert!((r - MINUS_3DB * 0.6).abs() < 1e-6);
        // 3.0: FC only
        let (l, r) = downmix(&[0.0, 0.0, 0.5]);
        assert!((l - 0.3535534).abs() < 1e-6 && (r - 0.3535534).abs() < 1e-6);
    }

    #[test]
    fn resample_passthrough_and_linear() {
        let mut rs = LinearResampler::new(48000, 48000);
        let mut out = Vec::new();
        rs.process(0.5, -0.5, &mut |l, r| out.push((l, r)));
        assert_eq!(out, vec![(0.5, -0.5)]);

        // 2:1 up: two outputs per input, between the previous input and
        // this one (positions 0 and 0.5)
        let mut rs = LinearResampler::new(24000, 48000);
        out.clear();
        for x in [1.0, 2.0, 3.0] {
            rs.process(x, -x, &mut |l, r| out.push((l, r)));
        }
        assert_eq!(out, vec![(0.0, 0.0), (0.5, -0.5), (1.0, -1.0), (1.5, -1.5), (2.0, -2.0), (2.5, -2.5)]);

        // 2:1 down: every other input
        let mut rs = LinearResampler::new(96000, 48000);
        out.clear();
        for x in [1.0, 2.0, 3.0, 4.0, 5.0] {
            rs.process(x, x, &mut |l, _| out.push((l, 0.0)));
        }
        assert_eq!(out.iter().map(|o| o.0).collect::<Vec<_>>(), vec![0.0, 2.0, 4.0]);
    }

    #[test]
    fn resample_44100_to_48000_keeps_the_rate_and_the_tone() {
        let mut rs = LinearResampler::new(44100, 48000);
        let mut out = Vec::new();
        for n in 0..44100 {
            let x = (std::f64::consts::TAU * 1000.0 * n as f64 / 44100.0).sin() as f32 * 0.5;
            rs.process(x, x, &mut |l, _| out.push(l));
        }
        assert!((47999..=48001).contains(&out.len()), "{} samples", out.len());
        // The output is the tone at 48 kHz, one input sample late
        let delay = 48000.0 / 44100.0;
        let mut err = 0.0f64;
        for (n, &y) in out.iter().enumerate().skip(10) {
            let t = (n as f64 - delay) / 48000.0;
            let want = (std::f64::consts::TAU * 1000.0 * t).sin() * 0.5;
            err = err.max((y as f64 - want).abs());
        }
        assert!(err < 0.01, "max error {err}");
    }

    #[test]
    fn layout_orders_channels_by_location_per_cis() {
        // One 2-channel CIS listed right-first, then one mono CIS, then two
        // 1-channel CIS
        let p = plan(preset(48000, 10000, 100), vec![cis(1, &[R, L]), cis(2, &[MONO]), cis(3, &[R]), cis(4, &[L])]);
        let layout = SduLayout::from_plan(&p).unwrap();
        assert_eq!(layout.sdu_lens, vec![200, 100, 100, 100]);
        assert_eq!(layout.frame_samples, 480);
        let slots: Vec<_> = layout.channels.iter().map(|c| (c.cis, c.offset, c.allocation, c.source)).collect();
        assert_eq!(
            slots,
            vec![
                (0, 0, LOCATION_FRONT_LEFT, ChannelSource::Left),
                (0, 100, LOCATION_FRONT_RIGHT, ChannelSource::Right),
                (1, 0, 0, ChannelSource::Mono),
                (2, 0, LOCATION_FRONT_RIGHT, ChannelSource::Right),
                (3, 0, LOCATION_FRONT_LEFT, ChannelSource::Left),
            ]
        );
        assert!(SduLayout::from_plan(&plan(preset(44100, 10000, 100), vec![cis(1, &[L])])).is_err());
        assert!(SduLayout::from_plan(&plan(preset(48000, 10000, 100), vec![])).is_err());
        assert!(SduLayout::from_plan(&plan(preset(48000, 10000, 100), vec![cis(1, &[])])).is_err());
    }

    /// Collects the pushed sets
    #[derive(Clone, Default)]
    struct Collect(Arc<Mutex<Vec<SduSet>>>);

    impl SduSink for Collect {
        fn push(&self, set: SduSet) {
            self.0.lock().unwrap().push(set);
        }
    }

    /// Each CIS's SDU carries its channels' frames in location order: what
    /// encoding the left / right / mono audio by hand gives
    #[test]
    fn sdus_carry_the_channels_in_location_order() {
        let rate = 48000;
        let p = plan(preset(rate, 10000, 100), vec![cis(1, &[R, L]), cis(2, &[MONO])]);
        let input = InputFormat { sample_rate: rate, channels: 2, sample: SampleType::F32 };
        let mut enc = LeEncoder::new(&p, input).unwrap();
        let sink = Collect::default();
        // Left: a tone, right: silence, for 3 frames and a bit
        let frames = 3 * 480 + 100;
        let tone: Vec<f32> = (0..frames).map(|n| (n as f32 * 0.13).sin() * 0.5).collect();
        let pcm: Vec<f32> = tone.iter().flat_map(|&l| [l, 0.0]).collect();
        // In uneven pieces, as capture delivers them
        let bytes = f32_bytes(&pcm);
        for piece in bytes.chunks(8 * 333) {
            enc.push(piece, &sink);
        }
        assert_eq!(enc.sets_encoded(), 3);
        let sets = sink.0.lock().unwrap();
        assert_eq!(sets.len(), 3);

        // Reference: the same frames encoded per channel directly
        let params = Lc3Params { sample_rate: rate, frame_us: 10000, octets: 100 };
        let mut left = Lc3ChannelEncoder::new(params).unwrap();
        let mut right = Lc3ChannelEncoder::new(params).unwrap();
        let mut mono = Lc3ChannelEncoder::new(params).unwrap();
        for (f, set) in sets.iter().enumerate() {
            let frame = &tone[f * 480..(f + 1) * 480];
            let l: Vec<i16> = frame.iter().map(|&x| to_i16(x)).collect();
            let m: Vec<i16> = frame.iter().map(|&x| to_i16(0.5 * x)).collect();
            let (mut fl, mut fr, mut fm) = ([0u8; 100], [0u8; 100], [0u8; 100]);
            left.encode(&l, &mut fl).unwrap();
            right.encode(&[0; 480], &mut fr).unwrap();
            mono.encode(&m, &mut fm).unwrap();
            assert_eq!(set.len(), 2);
            assert_eq!(&set[0][..100], &fl, "frame {f}: front left first");
            assert_eq!(&set[0][100..], &fr, "frame {f}: front right second");
            assert_eq!(&set[1][..], &fm, "frame {f}: mono CIS");
            assert_ne!(fl, fr);
        }
    }

    #[test]
    fn integer_input_and_reset() {
        let p = plan(preset(16000, 7500, 30), vec![cis(1, &[MONO])]);
        let input = InputFormat { sample_rate: 16000, channels: 1, sample: SampleType::I16 };
        let mut enc = LeEncoder::new(&p, input).unwrap();
        let sink = Collect::default();
        assert_eq!(enc.layout().frame_samples, 120);
        enc.push(&[0u8; 2 * 100], &sink);
        enc.reset();
        enc.push(&[0u8; 2 * 100], &sink);
        assert_eq!(enc.sets_encoded(), 0, "the reset dropped the first 100 samples");
        enc.push(&[0u8; 2 * 20], &sink);
        assert_eq!(enc.sets_encoded(), 1);
        assert_eq!(sink.0.lock().unwrap()[0], silence_set(&p));
        assert!(LeEncoder::new(&p, InputFormat { channels: 0, ..input }).is_err());
    }

    #[test]
    fn i16_and_i32_scale_like_float() {
        assert_eq!(input_sample(&16384i16.to_le_bytes(), 0, SampleType::I16), 0.5);
        assert_eq!(input_sample(&(-0x4000_0000i32).to_le_bytes(), 0, SampleType::I32), -0.5);
        assert_eq!(input_sample(&0.25f32.to_le_bytes(), 0, SampleType::F32), 0.25);
        assert_eq!(to_i16(2.0), 32767);
        assert_eq!(to_i16(-2.0), -32767);
        assert_eq!(to_i16(0.5), 16384); // 16383.5: ties to even
        assert_eq!(to_i16(1.5 / 32767.0), 2); // 1.5: ties to even
    }

    #[test]
    fn spare_sets_are_used() {
        let p = plan(preset(48000, 10000, 100), vec![cis(1, &[L]), cis(2, &[R])]);
        let input = InputFormat { sample_rate: 48000, channels: 2, sample: SampleType::F32 };
        let mut enc = LeEncoder::new(&p, input).unwrap();
        let queue = Arc::new(Mutex::new(SduQueue::new()));
        queue.lock().unwrap().configure(2, silence_set(&p));
        let spares_before = std::iter::from_fn(|| queue.lock().unwrap().spare()).count();
        queue.lock().unwrap().configure(2, silence_set(&p));
        enc.push(&f32_bytes(&vec![0.1; 2 * 480 * 2]), &queue);
        let q = queue.lock().unwrap();
        assert_eq!(q.len(), 2);
        assert_eq!(q.stats().sets_pushed, 2);
        drop(q);
        let spares_after = std::iter::from_fn(|| queue.lock().unwrap().spare()).count();
        assert_eq!(spares_after, spares_before - 2);
    }

    #[test]
    fn silence_set_has_one_sdu_per_cis() {
        let p = plan(preset(48000, 7500, 75), vec![cis(1, &[L, R]), cis(2, &[MONO])]);
        let s = silence_set(&p);
        assert_eq!(s.iter().map(Vec::len).collect::<Vec<_>>(), vec![150, 75]);
        let mut frame = [0u8; 75];
        codec::lc3::silence_frame(Lc3Params { sample_rate: 48000, frame_us: 7500, octets: 75 }, &mut frame).unwrap();
        assert_eq!(&s[0][..75], &frame);
        assert_eq!(&s[0][75..], &frame);
        assert_eq!(&s[1][..], &frame);
        // An unusable plan still gives one (zero) SDU per CIS
        let bad = plan(preset(44100, 10000, 100), vec![cis(1, &[L, R])]);
        assert_eq!(silence_set(&bad), vec![vec![0u8; 200]]);
    }

    /// The thread: captured audio in, SDU sets out, stop joins
    #[test]
    fn encode_thread_runs() {
        let p = plan(preset(48000, 7500, 75), vec![cis(1, &[L, R])]);
        let input = InputFormat { sample_rate: 48000, channels: 2, sample: SampleType::I16 };
        let queue = Arc::new(Mutex::new(SduQueue::new()));
        queue.lock().unwrap().configure(2, silence_set(&p));
        let stats = Arc::new(LinkStats::default());
        let config = LePipelineConfig { plan: p, input, buffer: BufferPreset::Standard };
        let (pipeline, mut capture) = LePipeline::start(config, queue.clone(), stats.clone()).unwrap();
        assert_eq!(pipeline.layout().sdu_lens, vec![150]);
        // 4 frames of 7.5 ms
        capture.push(&vec![0u8; 4 * 360 * 4]);
        let start = std::time::Instant::now();
        while queue.lock().unwrap().len() < 4 && start.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(5));
        }
        pipeline.stop();
        assert_eq!(queue.lock().unwrap().len(), 4);
        assert_eq!(stats.capture_frames.load(Ordering::Relaxed), 4 * 360);
        // Each set timed against its 7.5 ms of audio
        let enc = stats.encode.totals();
        assert_eq!((enc.calls, enc.audio_ns), (4, 4 * 7_500_000));
        assert!(enc.busy_ns > 0);
        let p = plan(preset(44100, 7500, 75), vec![cis(1, &[L])]);
        let config = LePipelineConfig { plan: p, input, buffer: BufferPreset::Standard };
        assert!(LePipeline::start(config, queue, stats).is_err());
    }
}
