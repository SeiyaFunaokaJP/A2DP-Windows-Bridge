//! A streaming session: connect, pick the codec, set up capture, encoder and
//! stream, then keep streaming and reconnect when the link drops — v1's
//! A2dpService::streaming_thread_func_inner, on the session thread of
//! [`crate::Service`].

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use audio_capture::{Capture, CaptureOptions, Format, Running, SampleFormat, Sink, Source};
use bt_host::{BtHost, ConfigureError, ConnectFailure, MediaSink, ReconnectError};
use codec::caps::{self, ConfigError, Configuration, RemoteCaps, Unavailable};
use codec::{CodecKind, Encoder, EncoderConfig, PeerParams, Quality};
use pipeline::{BufferPreset, CaptureInput, InputFormat, Pipeline, PipelineConfig, SampleType};

use crate::health::{CaptureReading, StreamHealth};
use crate::messages::Msg;
use crate::mute_output::OutputMute;
use crate::service::Inner;

/// What to stream (from a profile, or the command line)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamRequest {
    pub device: [u8; 6],
    pub device_name: String,
    /// None = Auto
    pub codec: Option<CodecKind>,
    pub codec_fallback: bool,
    /// Experimental codecs may be used (settings `experimental_codecs`):
    /// off, Auto and a fallback never pick them and a chosen one falls back
    pub experimental_codecs: bool,
    /// The device's codec order (the profile's `codec_order`; the default
    /// under Auto-select): what Auto picks first, and what a chosen codec
    /// the device does not offer falls back to (the first one it offers)
    pub auto_order: codec::AutoOrder,
    pub quality: Quality,
    pub abr: bool,
    /// SBC / SBC XQ: the bitpool wanted (0 = by quality)
    pub sbc_bitpool: u8,
    /// The same for SBC XQ
    pub sbcxq_bitpool: u8,
    /// SBC / SBC XQ: the lowest bitpool wanted (0 = Auto), each codec its
    /// own; [`StreamRequest::sbc_params`] puts it in the SBC setup
    pub sbc_min_bitpool: u8,
    pub sbcxq_min_bitpool: u8,
    /// SBC / SBC XQ: blocks, subbands, allocation, channel mode wanted (the
    /// defaults when not set; what the device does not list falls back).
    /// Its `min_bitpool` is not used: see [`StreamRequest::sbc_params`].
    pub sbc: codec::sbc::SbcParams,
    /// None = the device's rate
    pub sample_rate: Option<u32>,
    /// None = auto (24-bit where the codec carries it)
    pub bit_depth: Option<u8>,
    pub source: SourceRequest,
    pub volume_sync: VolumeSync,
    /// System Loopback mutes the default output device while streaming
    /// (settings `mute_output`)
    pub mute_output: bool,
    pub max_media_payload: u16,
    /// LE Audio (experimental) instead of A2DP: the le_stream session
    /// streams to `device` and these extra devices. None: A2DP.
    pub le: Option<bt_host::LeRequest>,
}

impl StreamRequest {
    /// The SBC setup of a `kind` stream (SBC or SBC XQ): [`Self::sbc`] with
    /// that codec's minimum bitpool, as the SET_CONFIGURATION and the
    /// encoder take it
    pub fn sbc_params(&self, kind: CodecKind) -> codec::sbc::SbcParams {
        let min_bitpool = match kind {
            CodecKind::Sbc => self.sbc_min_bitpool,
            CodecKind::SbcXq => self.sbcxq_min_bitpool,
            _ => 0,
        };
        codec::sbc::SbcParams { min_bitpool, ..self.sbc }
    }

    /// Whether a session capturing `source` mutes the default output device.
    /// Not Application capture: other apps may play there on purpose.
    pub fn mutes_output(&self, source: &SourceRequest) -> bool {
        self.mute_output && *source == SourceRequest::SystemLoopback
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceRequest {
    SystemLoopback,
    VirtualDevice { id: String, name: String, auto_switch: bool },
    Application { exe: String },
    TestTone,
}

impl SourceRequest {
    /// The output device a session of this source makes Windows' default
    pub fn default_switch(&self) -> Option<&str> {
        match self {
            SourceRequest::VirtualDevice { id, auto_switch: true, .. } if !id.is_empty() => Some(id),
            _ => None,
        }
    }

    /// The source as the UI shows it (not waiting: the session says when
    /// an application is not running)
    pub fn info(&self) -> SourceInfo {
        let (kind, name) = match self {
            SourceRequest::SystemLoopback => ("loopback", String::new()),
            SourceRequest::VirtualDevice { name, .. } => ("virtual", name.clone()),
            SourceRequest::Application { exe } => ("app", exe.clone()),
            SourceRequest::TestTone => ("tone", String::new()),
        };
        SourceInfo { kind, name, waiting: false }
    }
}

impl std::fmt::Display for SourceRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceRequest::SystemLoopback => f.write_str("system output"),
            SourceRequest::VirtualDevice { name, .. } => write!(f, "virtual device {}", crate::diag::quoted(name)),
            SourceRequest::Application { exe } => write!(f, "app {exe}"),
            SourceRequest::TestTone => f.write_str("test tone"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VolumeSync {
    Off,
    /// The device the audio comes from (the default output in application
    /// mode)
    Auto,
    Device(String),
}

/// What the session reports about the stream
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StreamInfo {
    pub codec: String,
    pub bitrate_kbps: u32,
    pub sample_rate: u32,
    pub channels: u32,
    /// Bits per sample the codec carries (16, 24); 0: not reported (LE
    /// Audio)
    pub bits: u32,
    pub source_sample_rate: u32,
    pub source_channels: u32,
    pub source_bits: u32,
    /// The profile's codec the device did not offer (codec fallback)
    pub requested_codec: Option<String>,
    /// ... maybe because another device streams on its endpoint
    pub requested_in_use: bool,
    /// Adaptive bit rate on (LDAC, LHDC V5, SBC and SBC XQ)
    pub abr: bool,
    /// The device's maximum bit rate lowered the profile's (LHDC V5)
    pub bitrate_capped: bool,
    /// Where the audio comes from (sent again when an application capture
    /// starts or stops waiting for the app)
    pub source: SourceInfo,
    /// What the codec runs with (A2DP; None for LE Audio)
    pub details: Option<Box<CodecDetails>>,
}

/// What an A2DP stream's codec runs with beyond the summary (Diagnostics)
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CodecDetails {
    /// Bytes a packet may carry after its RTP header: media payload
    /// header and codec data (codecs without an RTP header use those 12
    /// bytes too; FastStream and aptX LL packets are shorter by design)
    pub mtu: u32,
    /// Packets start with an RTP header (not classic aptX, aptX LL,
    /// FastStream)
    pub rtp: bool,
    /// Bytes of the media payload header (frame count ...)
    pub media_header: u32,
    /// PCM frames one codec frame stands for
    pub frame_samples: u32,
    /// Bytes of every codec frame, when they all have the same length
    pub frame_bytes: Option<u32>,
    /// Codec frames per packet, when that follows from the frame length
    pub frames_per_packet: Option<u32>,
    pub sbc: Option<SbcDetails>,
    pub aac: Option<AacDetails>,
    /// Any other codec: what the device's endpoint of it lists, as SBC and
    /// AAC say theirs (issue #86)
    pub device: Option<CodecDeviceCaps>,
}

/// SBC, SBC XQ and FastStream frames
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SbcDetails {
    /// "mono", "dual_channel", "stereo", "joint_stereo"
    pub channel_mode: String,
    pub blocks: u32,
    pub subbands: u32,
    /// "loudness", "snr"
    pub allocation: String,
    /// The bitpool the encoder uses
    pub bitpool: u32,
    /// The bitpool range of the configuration (SET_CONFIGURATION); None
    /// for FastStream (its codec information has none)
    pub config_bitpool: Option<(u32, u32)>,
    /// What the device's SBC endpoint offers, when it said
    pub device: Option<SbcDeviceCaps>,
    /// SBC / SBC XQ under ABR: the range the bitpool moves in (floor,
    /// ceiling); `bitpool` is then the one in use now
    pub abr_bitpool: Option<(u32, u32)>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SbcDeviceCaps {
    pub sample_rates: Vec<u32>,
    pub channel_modes: Vec<String>,
    pub blocks: Vec<u32>,
    pub subbands: Vec<u32>,
    pub allocations: Vec<String>,
    pub min_bitpool: u32,
    pub max_bitpool: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AacDetails {
    /// "MPEG-2 AAC-LC" or "MPEG-4 AAC-LC"
    pub object_type: String,
    /// Constant bit rate (the encoder never uses VBR)
    pub bit_rate: u32,
    /// What the device's AAC endpoint offers, when it said
    pub device: Option<AacDeviceCaps>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AacDeviceCaps {
    pub object_types: Vec<String>,
    pub sample_rates: Vec<u32>,
    pub channels: Vec<u32>,
    /// Its peak bit rate in bps (0 = not said)
    pub max_bitrate: u32,
    pub vbr: bool,
}

fn channel_mode_key(m: codec::sbc::ChannelMode) -> String {
    use codec::sbc::ChannelMode::*;
    match m {
        Mono => "mono",
        DualChannel => "dual_channel",
        Stereo => "stereo",
        JointStereo => "joint_stereo",
    }
    .into()
}

fn allocation_key(a: codec::sbc::Allocation) -> String {
    match a {
        codec::sbc::Allocation::Loudness => "loudness",
        codec::sbc::Allocation::Snr => "snr",
    }
    .into()
}

/// What a device's SBC endpoint offers, as Diagnostics and the profile
/// editor show it
pub fn sbc_device_caps(d: &caps::SbcCapabilities) -> SbcDeviceCaps {
    SbcDeviceCaps {
        sample_rates: d.sample_rates(),
        channel_modes: d.channel_modes().into_iter().map(channel_mode_key).collect(),
        blocks: d.block_lengths().into_iter().map(u32::from).collect(),
        subbands: d.subband_counts().into_iter().map(u32::from).collect(),
        allocations: d.allocations().into_iter().map(allocation_key).collect(),
        min_bitpool: d.min_bitpool.into(),
        max_bitpool: d.max_bitpool.into(),
    }
}

/// What the stream's codec runs with: the configuration sent, what the
/// device offered and the encoder's setup
pub fn codec_details(
    kind: CodecKind,
    config: &Configuration,
    remote: &RemoteCaps,
    encoder: &dyn Encoder,
    mtu: u16,
    sample_rate: u32,
) -> CodecDetails {
    let e = encoder.details();
    let raw = pipeline::packetizer::max_raw(kind, mtu, sample_rate) as u32;
    let frames_per_packet = e.frame_bytes.filter(|&b| b > 0).map(|b| {
        let n = raw / b;
        // SBC counts frames in 4 bits
        if kind.media_header_len() == 1 { n.min(15) } else { n }
    });
    let sbc = e.sbc.map(|c| SbcDetails {
        channel_mode: channel_mode_key(c.channel_mode),
        blocks: c.blocks.into(),
        subbands: c.subbands.into(),
        allocation: allocation_key(c.allocation),
        bitpool: c.bitpool.into(),
        config_bitpool: match config.request {
            caps::ConfigRequest::Sbc { min_bitpool, max_bitpool, .. } => Some((min_bitpool.into(), max_bitpool.into())),
            _ => None,
        },
        device: remote.sbc_caps.filter(|_| kind != CodecKind::FastStream).as_ref().map(sbc_device_caps),
        abr_bitpool: e.sbc_abr.map(|(lo, hi)| (lo.into(), hi.into())),
    });
    let aac = match config.request {
        caps::ConfigRequest::Aac { object_type, bit_rate, .. } => Some(AacDetails {
            object_type: object_type.name().into(),
            bit_rate,
            device: remote.aac_caps.map(|d| AacDeviceCaps {
                object_types: d.object_type_names().into_iter().map(String::from).collect(),
                sample_rates: d.sample_rates(),
                channels: d.channel_counts().into_iter().map(u32::from).collect(),
                max_bitrate: d.bit_rate,
                vbr: d.vbr,
            }),
        }),
        _ => None,
    };
    let device = (!matches!(kind, CodecKind::Sbc | CodecKind::SbcXq | CodecKind::Aac | CodecKind::FastStream))
        .then(|| codec_device_caps(remote).into_iter().find(|c| c.codec == kind.key()))
        .flatten();
    CodecDetails {
        mtu: raw + kind.media_header_len() as u32,
        rtp: kind.uses_rtp(),
        media_header: kind.media_header_len() as u32,
        frame_samples: encoder.pcm_frames_per_codec_frame() as u32,
        frame_bytes: e.frame_bytes,
        frames_per_packet,
        sbc,
        aac,
        device,
    }
}

/// The audio source of a stream
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SourceInfo {
    /// "loopback", "virtual", "app" (profile capture modes) or "tone"
    pub kind: &'static str,
    /// The virtual device's name or the application's executable
    pub name: String,
    /// Application capture: the application is not running
    pub waiting: bool,
}

/// What a device offered at its last connection (kept in
/// device_codecs.json, [`crate::devices::load_codecs`])
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KnownCodecs {
    /// Codec keys ([`codec_keys`])
    pub keys: Vec<String>,
    /// Its SBC endpoint's capabilities, when it said
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sbc: Option<SbcDeviceCaps>,
    /// What each codec it offered lists (the codec settings offer only
    /// that, issue #86); empty when last recorded by an older version
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub caps: Vec<CodecDeviceCaps>,
}

impl KnownCodecs {
    /// What a device offered, from its capabilities
    pub fn of(remote: &caps::RemoteCaps) -> KnownCodecs {
        KnownCodecs {
            keys: codec_keys(remote),
            sbc: remote.sbc_caps.as_ref().map(sbc_device_caps),
            caps: codec_device_caps(remote),
        }
    }
}

/// What a device's endpoint of one codec lists, as far as the codec
/// settings need it
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CodecDeviceCaps {
    /// Codec key ([`codec_keys`])
    pub codec: String,
    /// Sample rates it lists, lowest first (empty: not said)
    #[serde(default)]
    pub sample_rates: Vec<u32>,
    /// Bit depths it lists (empty: not said, or no choice)
    #[serde(default)]
    pub bit_depths: Vec<u32>,
    /// Its largest bit rate in bps (0 = not said)
    #[serde(default)]
    pub max_bitrate: u32,
}

/// What each codec a device offered lists
pub fn codec_device_caps(remote: &caps::RemoteCaps) -> Vec<CodecDeviceCaps> {
    CodecKind::ALL
        .into_iter()
        .filter(|&k| remote.has(k))
        .map(|k| CodecDeviceCaps {
            codec: k.key().to_string(),
            sample_rates: remote.listed_sample_rates(k),
            bit_depths: remote.listed_bit_depths(k),
            max_bitrate: remote.listed_max_bitrate(k),
        })
        .collect()
}

/// Codec keys ("ldac", "sbc", ...) of what a device offered
pub fn codec_keys(remote: &caps::RemoteCaps) -> Vec<String> {
    CodecKind::ALL.into_iter().filter(|&k| remote.has(k)).map(|k| k.key().to_string()).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum StreamState {
    Idle,
    Connecting,
    Streaming,
    Reconnecting,
    Error,
}

/// Feeds captured audio into the pipeline. Shared so an application
/// capture can be replaced (the app quits and starts again) while the
/// pipeline keeps running; only one capture writes at a time, so the lock is
/// never contended.
#[derive(Clone)]
struct SharedInput(Arc<Mutex<CaptureInput>>);

impl Sink for SharedInput {
    fn audio(&mut self, data: &[u8], _frames: usize, _format: Format) {
        if let Ok(mut input) = self.0.try_lock() {
            input.push(data);
        }
    }

    fn silence(&mut self, frames: usize, _format: Format) {
        if let Ok(mut input) = self.0.try_lock() {
            input.push_silence(frames);
        }
    }
}

pub(crate) fn input_format(f: Format) -> InputFormat {
    InputFormat {
        sample_rate: f.sample_rate,
        channels: f.channels,
        sample: match f.sample_format {
            SampleFormat::F32 => SampleType::F32,
            SampleFormat::I16 => SampleType::I16,
            SampleFormat::I32 => SampleType::I32,
        },
    }
}

/// Application capture that follows the app: attaches by executable name and
/// notices when the process exits, so it can attach again when it is
/// started anew
struct AppSource {
    exe: String,
    sample_rate: u32,
    running: Option<Running>,
    process_id: u32,
    failed_pid: u32,
}

impl AppSource {
    fn attach(&mut self, input: &SharedInput) -> bool {
        let Some(pid) = audio_capture::find_process(&self.exe) else {
            tracing::trace!("app capture: {} not found", self.exe);
            return false;
        };
        tracing::debug!("app capture: attaching to {} (PID {pid}) at {} Hz", self.exe, self.sample_rate);
        let options = CaptureOptions { preferred_rate: Some(self.sample_rate), fill_silence: true };
        let started = Capture::open(&Source::Application { process_id: pid }, &options)
            .and_then(|c| c.start(Box::new(input.clone())));
        match started {
            Ok(r) => {
                tracing::info!("capturing app {} (PID {pid})", self.exe);
                self.running = Some(r);
                self.process_id = pid;
                true
            }
            Err(e) => {
                if self.failed_pid != pid {
                    tracing::warn!("cannot capture {} (PID {pid}): {e}", self.exe);
                }
                self.failed_pid = pid;
                false
            }
        }
    }

    fn exited(&self) -> bool {
        self.running.is_some()
            && (audio_capture::process_exited(self.process_id) || self.running.as_ref().is_some_and(|r| r.ended()))
    }

    fn detach(&mut self) {
        if let Some(r) = self.running.take() {
            r.stop();
        }
    }
}

/// Device capture (loopback of a device, or the test tone). Windows ends a
/// loopback capture when the device is removed, disabled or its format
/// changes (AUDCLNT_E_DEVICE_INVALIDATED): it is opened again, until the
/// device is back
struct DeviceSource {
    source: Source,
    options: CaptureOptions,
    running: Option<Running>,
    /// Logged once per outage
    failed: bool,
}

impl DeviceSource {
    fn ended(&self) -> bool {
        self.running.as_ref().is_some_and(Running::ended)
    }

    fn detach(&mut self) {
        if let Some(r) = self.running.take() {
            r.stop();
        }
    }

    /// Opens the source again; the caller starts it (its format may differ)
    fn reopen(&mut self) -> Option<Capture> {
        match Capture::open(&self.source, &self.options) {
            Ok(c) => Some(c),
            Err(e) => {
                if !self.failed {
                    tracing::warn!("audio capture: cannot open the device again yet: {e}");
                }
                self.failed = true;
                None
            }
        }
    }

    fn start(&mut self, capture: Capture, input: &SharedInput) -> bool {
        match capture.start(Box::new(input.clone())) {
            Ok(r) => {
                tracing::info!("audio capture: device opened again, capturing");
                self.running = Some(r);
                self.failed = false;
                true
            }
            Err(e) => {
                if !self.failed {
                    tracing::warn!("audio capture: cannot start the device again yet: {e}");
                }
                self.failed = true;
                false
            }
        }
    }
}

enum ActiveSource {
    Capture(DeviceSource),
    App(AppSource),
}

impl ActiveSource {
    /// An application capture whose app is not running
    fn waiting(&self) -> bool {
        matches!(self, ActiveSource::App(a) if a.running.is_none())
    }

    fn running(&self) -> Option<&Running> {
        match self {
            ActiveSource::Capture(d) => d.running.as_ref(),
            ActiveSource::App(a) => a.running.as_ref(),
        }
    }

    /// For the stream health log (takes the capture's packet gap)
    fn health(&self) -> CaptureReading {
        self.running().map_or_else(CaptureReading::default, |r| CaptureReading {
            filled_frames: r.filled_frames(),
            discontinuities: r.discontinuities(),
            packet_gap: r.take_packet_gap(),
        })
    }

    fn stop(&mut self) {
        match self {
            ActiveSource::Capture(d) => d.detach(),
            ActiveSource::App(a) => a.detach(),
        }
    }
}

/// Opens a source at the stream's rate, not started yet: a device's capture,
/// or None for an application ([`start_source`] attaches to it). The error is
/// what ends the session.
fn open_source(request: &SourceRequest, rate: u32) -> Result<Option<(DeviceSource, Capture)>, Msg> {
    let source = match request {
        SourceRequest::TestTone => Source::TestTone,
        SourceRequest::SystemLoopback => Source::SystemLoopback,
        SourceRequest::VirtualDevice { id, .. } => {
            if id.is_empty() {
                return Err(Msg::new("error.no_virtual_device_selected"));
            }
            Source::RenderDevice { id: id.clone() }
        }
        SourceRequest::Application { exe } => {
            let build = audio_capture::windows_build();
            if build < audio_capture::APP_CAPTURE_MIN_BUILD {
                return Err(Msg::with("error.app_capture_unsupported", &[&build]));
            }
            if exe.is_empty() {
                return Err(Msg::new("error.no_app_selected"));
            }
            return Ok(None);
        }
    };
    let options = CaptureOptions { preferred_rate: Some(rate), fill_silence: *request != SourceRequest::TestTone };
    match Capture::open(&source, &options) {
        Ok(c) => Ok(Some((DeviceSource { source, options, running: None, failed: false }, c))),
        Err(e) => {
            tracing::error!("audio capture: {e}");
            let key = if matches!(request, SourceRequest::VirtualDevice { .. }) {
                "error.virtual_capture_init"
            } else {
                "error.wasapi_init"
            };
            Err(Msg::new(key))
        }
    }
}

/// What the source [`open_source`] opened delivers. Application capture:
/// float stereo at the requested rate, known before the app runs.
fn source_format(opened: &Option<(DeviceSource, Capture)>, rate: u32) -> Format {
    opened
        .as_ref()
        .map_or(Format { sample_rate: rate, channels: 2, sample_format: SampleFormat::F32 }, |(_, c)| c.format())
}

/// Starts what [`open_source`] opened, into the running pipeline
fn start_source(
    opened: Option<(DeviceSource, Capture)>,
    request: &SourceRequest,
    rate: u32,
    input: &SharedInput,
) -> Option<ActiveSource> {
    match (opened, request) {
        (Some((mut device, capture)), _) => match capture.start(Box::new(input.clone())) {
            Ok(r) => {
                device.running = Some(r);
                Some(ActiveSource::Capture(device))
            }
            Err(e) => {
                tracing::error!("audio capture start: {e}");
                None
            }
        },
        (None, SourceRequest::Application { exe }) => {
            let mut app =
                AppSource { exe: exe.clone(), sample_rate: rate, running: None, process_id: 0, failed_pid: 0 };
            if !app.attach(input) {
                // Not running yet is fine: the capture attaches once it is up
                tracing::info!("{exe} is not running yet, waiting for it");
            }
            Some(ActiveSource::App(app))
        }
        _ => None,
    }
}

/// The capture's format changed under a running stream: a new encoder and
/// pipeline take over, with the same stream settings. Without a pipeline
/// (between reconnects) the next one is started with the new format. The
/// error is what ends the session.
fn restart_pipeline(
    pipeline: &mut Option<Pipeline>,
    kind: CodecKind,
    encoder_config: &EncoderConfig,
    format: Format,
    channels: u32,
    input: &SharedInput,
    host: &BtHost,
) -> Result<(), Msg> {
    let Some(p) = pipeline.take() else { return Ok(()) };
    let Some(sink) = p.stop() else { return Err(Msg::new("error.encode_thread")) };
    let encoder = match codec::create(kind, encoder_config) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!("encoder: {e}");
            host.return_media_sink(sink);
            return Err(Msg::new("error.encoder_init"));
        }
    };
    let (p, new_input) = start_pipeline(encoder, format, channels, encoder_config.mtu, sink, host);
    if let Ok(mut i) = input.0.lock() {
        *i = new_input;
    }
    *pipeline = Some(p);
    Ok(())
}

/// Restores the default output device when the session ends (virtual
/// device mode switched it)
pub(crate) struct DefaultDeviceGuard(pub(crate) Option<String>);

impl DefaultDeviceGuard {
    /// Follows the session's source: its device becomes the default output
    /// when it asks for that, keeping the device to restore; any other source
    /// plays through the device from before, restored now. Before the source
    /// opens: System Loopback captures whatever is the default.
    fn follow(&mut self, source: &SourceRequest) {
        match source {
            SourceRequest::VirtualDevice { id, name, auto_switch: true } if !id.is_empty() => {
                let restore = self.0.take();
                *self = DefaultDeviceGuard::switch_to(id, name);
                self.0 = restore.or(self.0.take());
            }
            _ => *self = DefaultDeviceGuard(None),
        }
    }

    /// Makes `id` the default output device. The device before is restored
    /// at the end even when the switch failed: Windows may have switched
    /// one role (Console, Multimedia) before the other failed.
    pub(crate) fn switch_to(id: &str, name: &str) -> DefaultDeviceGuard {
        // The session before kept it switched for this one: its device to
        // restore is this one's
        let handed = handover().restore.take();
        let before = audio_capture::default_render_device();
        let name = crate::diag::quoted(name);
        tracing::debug!("default output: switching to {name} ({id}), was {before:?}, handed over {handed:?}");
        if let Err(e) = audio_capture::set_default_render_device(id) {
            tracing::warn!("failed to make {name} the default output device: {e}");
        }
        DefaultDeviceGuard(handed.or_else(|| restore_target(before, id)))
    }
}

/// The device to restore: the one before, unless that is the one switched to
fn restore_target(before: Option<String>, switched_to: &str) -> Option<String> {
    before.filter(|b| b != switched_to)
}

/// A session stopped for one that switches the default output to the same
/// device (its settings changed, issue #203): Windows stays on that device
/// through the reconnect instead of playing through the speakers for a few
/// seconds, and the device to restore passes to the new session
#[derive(Debug, Default, PartialEq)]
struct DefaultHandover {
    /// The device the session about to start switches to (set while the
    /// running one stops)
    next: Option<String>,
    /// The device to restore that the stopped session left to the new one
    restore: Option<String>,
}

static HANDOVER: Mutex<DefaultHandover> = Mutex::new(DefaultHandover { next: None, restore: None });

fn handover() -> std::sync::MutexGuard<'static, DefaultHandover> {
    HANDOVER.lock().unwrap_or_else(|e| e.into_inner())
}

impl DefaultHandover {
    /// A session ends with `restore` to restore while `current` is the
    /// default output: kept for the next session when it switches to the
    /// current one; otherwise the device to restore now
    fn end(&mut self, current: Option<&str>, restore: String) -> Option<String> {
        if self.next.is_some() && self.next.as_deref() == current {
            self.restore = Some(restore);
            None
        } else {
            Some(restore)
        }
    }

    /// The device left to restore that no session took over (the new one
    /// ended before switching)
    fn unclaimed(&mut self) -> Option<String> {
        if self.next.is_some() { None } else { self.restore.take() }
    }
}

/// The session about to start switches to `next` (None: it does not, or no
/// session starts): set while the running one stops
pub(crate) fn expect_default_switch(next: Option<&str>) {
    handover().next = next.map(str::to_string);
}

/// After a session ended: restores the device a stopped session left to
/// one that never switched
pub(crate) fn restore_unclaimed_default() {
    let unclaimed = handover().unclaimed();
    if let Some(id) = unclaimed {
        restore_default(&id);
    }
}

fn restore_default(id: &str) {
    tracing::debug!("default output: restoring {id}");
    if let Err(e) = audio_capture::set_default_render_device(id) {
        tracing::warn!("could not restore the default output device: {e}");
    }
}

/// Logs the gaps WASAPI reports in the captured audio as they increase, at
/// most every 2 s (v1 WasapiCapture: "Data discontinuity (count=N)"; the
/// capture already skips the ones right after it starts)
#[derive(Default)]
pub(crate) struct DiscontinuityLog {
    reported: u64,
    last: Option<Instant>,
}

const DISCONTINUITY_LOG_INTERVAL: Duration = Duration::from_secs(2);

impl DiscontinuityLog {
    /// The capture's count now; Some((count, new since the last log)) when
    /// it is time to log. A new capture (count below the last) starts over.
    pub(crate) fn check(&mut self, count: u64, now: Instant) -> Option<(u64, u64)> {
        if count < self.reported {
            self.reported = 0;
        }
        if count == self.reported
            || self.last.is_some_and(|t| now.saturating_duration_since(t) < DISCONTINUITY_LOG_INTERVAL)
        {
            return None;
        }
        let new = count - self.reported;
        self.reported = count;
        self.last = Some(now);
        Some((count, new))
    }

    pub(crate) fn tick(&mut self, count: u64) {
        if let Some((count, new)) = self.check(count, Instant::now()) {
            tracing::warn!("capture: data discontinuity (count={count}, {new} new)");
        }
    }
}

impl Drop for DefaultDeviceGuard {
    fn drop(&mut self) {
        if let Some(id) = self.0.take() {
            let mut h = handover();
            let current = h.next.is_some().then(audio_capture::default_render_device).flatten();
            let now = h.end(current.as_deref(), id);
            drop(h);
            match now {
                Some(id) => restore_default(&id),
                None => tracing::debug!("default output: kept for the next session"),
            }
        }
    }
}

/// Why a stack is restarted after sleep (also in the log and the flow)
pub(crate) const RESUMED: &str = "the PC resumed from sleep";
const ADAPTER_STOPPED: &str = "the Bluetooth adapter stopped working";

/// The capture rate a profile's Auto sample rate asks for (issue #86)
const AUTO_SAMPLE_RATE: u32 = 48000;

/// A new stack (sleep, a stopped adapter): the adapter may take a while to
/// come back
const RESTART_ATTEMPTS: u32 = 10;
/// A link that faded (out of range) on a working stack: headphones that do
/// not answer three pages are not about to (turned off ones are not retried)
const LINK_LOSS_ATTEMPTS: u32 = 3;

fn reconnect_attempts(restarted: bool) -> u32 {
    if restarted { RESTART_ATTEMPTS } else { LINK_LOSS_ATTEMPTS }
}

/// What the streaming loop does about the link
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkCheck {
    Fine,
    /// The device dropped the link: connect it again on the same stack
    Reconnect,
    /// The stack cannot be used any more: start a new one first
    Restart(&'static str),
    /// The PC is going to sleep and took the adapter (or the link) with it:
    /// stop the stack and wait for the resume, spending no attempt
    SleepThrough,
}

fn check_link(lost: bool, stalled: bool, stale: bool, suspending: bool) -> LinkCheck {
    if suspending {
        // A stack left from an earlier sleep is replaced after this one;
        // while the adapter still works, the stream goes on
        return if lost || stalled { LinkCheck::SleepThrough } else { LinkCheck::Fine };
    }
    if stale {
        LinkCheck::Restart(RESUMED)
    } else if stalled {
        LinkCheck::Restart(ADAPTER_STOPPED)
    } else if lost {
        LinkCheck::Reconnect
    } else {
        LinkCheck::Fine
    }
}

/// The headphones ended the link themselves (turned off). Headphones that
/// switch off release A2DP first and drop the link a moment later, so a
/// release is given that moment before reconnecting.
fn device_turned_off(inner: &Inner, host: &BtHost) -> bool {
    let until = Instant::now() + Duration::from_millis(1500);
    loop {
        if host.lost_to_the_device() {
            return true;
        }
        if Instant::now() >= until {
            return false;
        }
        inner.wait_tick(Duration::from_millis(100));
    }
}

/// The headphones were turned off: nothing is sent until they connect to
/// the PC again (true). False when the session ends instead: stopped, or
/// the stack has to start anew (sleep, a stopped adapter), which the next
/// connection does. Another saved device that connects to the PC meanwhile
/// is announced ([`ServiceEvent::OtherDeviceBack`]): the core starts its
/// session, which ends this one (issue #86: with two devices, turning on
/// the other one connected nothing).
fn wait_for_device(inner: &Inner, host: &BtHost, device: [u8; 6], pipeline: &mut Option<Pipeline>) -> bool {
    inner.waiting_for_device();
    host.device_came_back(device);
    if let Some(p) = pipeline.take()
        && let Some(s) = p.stop()
    {
        host.return_media_sink(s);
    }
    let t = Instant::now();
    let back = loop {
        inner.wait_tick(Duration::from_millis(200));
        if inner.stop_requested() || inner.power.suspending() || inner.power.stale() || host.stalled() {
            break false;
        }
        match host.link_came_up() {
            Some(a) if a == device => break true,
            Some(a) if is_saved(inner, a) => {
                tracing::info!("device off: {} connected to the PC instead", bt_host::addr_str(&a));
                inner.other_device_back(a);
            }
            Some(a) => tracing::debug!("device off: {} connected, not a saved device", bt_host::addr_str(&a)),
            None => {}
        }
    };
    tracing::debug!("device off: waited {} s, back {back}", t.elapsed().as_secs());
    inner.done_waiting_for_device(back);
    back
}

/// A device saved on Audio Devices (classic)
fn is_saved(inner: &Inner, addr: [u8; 6]) -> bool {
    let a = bt_host::addr_str(&addr);
    crate::devices::load(inner.config_dir()).iter().any(|d| d.address.eq_ignore_ascii_case(&a))
}

/// The PC is going to sleep: the stack is shut down quietly (the pipeline's
/// media sink belongs to it) and the session waits for the resume. Returns
/// why the stack is then started anew, None when the session was stopped.
fn sleep_through(inner: &Inner, pipeline: &mut Option<Pipeline>) -> Option<&'static str> {
    tracing::info!("the PC is going to sleep, reconnecting after it resumes");
    if let Some(p) = pipeline.take() {
        let _ = p.stop();
    }
    inner.shutdown_host_for_sleep();
    let t = Instant::now();
    if !inner.wait_for_resume() {
        tracing::debug!("sleep: stopped while waiting for the resume");
        return None;
    }
    tracing::debug!("sleep: resumed after {} s, stack stale {}", t.elapsed().as_secs(), inner.power.stale());
    Some(if inner.power.stale() { RESUMED } else { ADAPTER_STOPPED })
}

/// A Bluetooth failure on a Realtek adapter that runs its ROM code is the
/// missing firmware, whatever the step that failed (issue #81: the adapter
/// opens and HCI comes up, then nothing connects or streams)
fn rom_or(host: &BtHost, msg: Msg) -> Msg {
    if !host.realtek_rom_firmware() {
        return msg;
    }
    tracing::warn!("{}: the Realtek firmware is not loaded", msg.english());
    Msg::new("firmware.rom_running")
}

fn connect_error(f: Option<ConnectFailure>) -> Msg {
    Msg::new(match f {
        Some(ConnectFailure::Stalled) => "error.btstack_no_answer",
        Some(ConnectFailure::NoAnswer) => "error.connect_no_answer",
        Some(ConnectFailure::LinkLost) => "error.connect_link_lost",
        Some(ConnectFailure::Refused) => "error.connect_refused",
        Some(ConnectFailure::AuthFailed) => "error.connect_auth_failed",
        _ => "error.pairing_hint",
    })
}

fn peer_params(remote: &RemoteCaps, kind: CodecKind, channels: u32, req: &StreamRequest) -> PeerParams {
    let (opus_frame_ms, opus_channels, opus_max_bitrate) = remote.opus_params(kind, channels);
    // The SET_CONFIGURATION took the same (configuration_with)
    let sbc = match kind {
        CodecKind::Sbc | CodecKind::SbcXq => {
            let (sbc, notes) = remote.sbc_params(kind, req.sbc_params(kind));
            for n in notes {
                tracing::warn!("{n}");
            }
            sbc
        }
        _ => Default::default(),
    };
    tracing::debug!(
        "peer: {} sbc bitpool {}..{} (wanted {}/{} xq), aac max {} bps, opus {opus_frame_ms} ms {opus_channels} ch max {opus_max_bitrate}, lhdc v5 limits {:?}",
        kind.name(),
        remote.sbc_min_bitpool(),
        remote.sbc_max_bitpool(),
        req.sbc_bitpool,
        req.sbcxq_bitpool,
        remote.aac_max_bitrate(),
        remote.lhdcv5_bitrate_limits()
    );
    PeerParams {
        lhdcv5_bitrate_limits: remote.lhdcv5_bitrate_limits(),
        sbc_min_bitpool: remote.sbc_min_bitpool(),
        sbc_max_bitpool: remote.sbc_max_bitpool(),
        sbc_bitpool: match kind {
            CodecKind::Sbc => req.sbc_bitpool,
            CodecKind::SbcXq => req.sbcxq_bitpool,
            _ => 0,
        },
        sbc,
        opus_frame_ms,
        opus_channels,
        opus_max_bitrate,
        aac_max_bitrate: if kind == CodecKind::Aac { remote.aac_max_bitrate() } else { 0 },
        aptxad_r2: kind == CodecKind::AptxAdaptive && remote.aptxad_r2(),
    }
}

#[derive(Debug)]
enum ConfigFailure {
    /// No configuration of the codec for this capture
    Build(ConfigError),
    /// BTstack or the device refused it, or no answer
    Host(ConfigureError),
}

/// SET_CONFIGURATION of a `kind` stream; the media payload size
#[allow(clippy::too_many_arguments)]
fn configure_stream(
    host: &BtHost,
    remote: &RemoteCaps,
    kind: CodecKind,
    sample_rate: u32,
    channels: u32,
    bits: u8,
    vbr: bool,
    sbc: codec::sbc::SbcParams,
) -> Result<(Configuration, u16), ConfigFailure> {
    let config =
        remote.configuration_with(kind, sample_rate, channels, bits, vbr, sbc).map_err(ConfigFailure::Build)?;
    let mtu = host.configure(&config).map_err(ConfigFailure::Host)?;
    Ok((config, mtu))
}

/// SBC XQ refused by the device (one that lists dual channel but does not
/// take it): worth one more try with plain SBC when the codec was Auto or
/// codec fallback is on
fn sbc_xq_fallback(kind: CodecKind, failure: &ConfigFailure, requested: Option<CodecKind>, fallback: bool) -> bool {
    kind == CodecKind::SbcXq
        && matches!(failure, ConfigFailure::Host(ConfigureError::Refused | ConfigureError::NoAnswer))
        && (requested.is_none() || fallback)
}

/// SBC / SBC XQ adaptive bitpool: only when the profile asks for that codec
/// itself with ABR on. A profile's ABR was LDAC's and LHDC V5's: Auto, or a
/// fallback from LDAC, landing on SBC keeps today's fixed bitpool.
fn sbc_abr(req: &StreamRequest, kind: CodecKind) -> bool {
    req.abr && matches!(kind, CodecKind::Sbc | CodecKind::SbcXq) && req.codec == Some(kind)
}

/// Starts the encode thread; ABR as the encoder runs it
fn start_pipeline(
    encoder: Box<dyn Encoder>,
    format: Format,
    channels: u32,
    mtu: u16,
    sink: MediaSink,
    host: &BtHost,
) -> (Pipeline, CaptureInput) {
    let abr = encoder.abr_active();
    Pipeline::start(
        PipelineConfig {
            encoder,
            input: input_format(format),
            channels,
            mtu,
            abr,
            buffer: BufferPreset::Standard,
            mute: Some(Box::new(host.volume_is_zero())),
        },
        sink,
        host.stats().clone(),
    )
}

/// Marks the stack as having carried a stream to the device when the
/// session ends (after any restart during it)
struct StreamedMark<'a>(&'a Inner, [u8; 6]);

impl Drop for StreamedMark<'_> {
    fn drop(&mut self) {
        self.0.mark_streamed(self.1);
    }
}

/// The session thread. Returns when stopped or on an error (state reported).
pub(crate) fn run(inner: &Arc<Inner>, req: StreamRequest) {
    let addr = bt_host::addr_str(&req.device);
    tracing::info!(
        "stream: device {addr} codec {} quality {:?} bit depth {} rate {} source {}",
        req.codec.map_or("auto", |c| c.name()),
        req.quality,
        req.bit_depth.map_or("auto".into(), |b| format!("{b}-bit")),
        req.sample_rate.map_or("device's".into(), |r| format!("{r} Hz")),
        req.source
    );
    tracing::debug!("stream: request {req:?}");
    let stopped = || inner.stop_requested();
    let session_start = Instant::now();

    inner.notify(StreamState::Connecting, Msg::new("status.initializing_btstack"));
    let mut host = match inner.host_for_stream(req.device) {
        Ok(h) => h,
        Err(m) => {
            tracing::debug!("stream: no stack after {} ms: {}", session_start.elapsed().as_millis(), m.english());
            return inner.notify(StreamState::Error, m);
        }
    };
    tracing::debug!(
        "stream: stack ready in {} ms, media payload limit {}",
        session_start.elapsed().as_millis(),
        req.max_media_payload
    );
    host.set_media_payload_limit(req.max_media_payload);
    if stopped() {
        return inner.notify(StreamState::Idle, Msg::new("status.ready"));
    }

    // Media keys now, the device's remembered volume below
    host.set_media_keys(inner.media_keys());

    // Remembered headphone volume, set as soon as AVRCP connects
    let restore = inner.saved_volume(&addr);
    tracing::debug!("stream: volume to restore {restore:?}");
    host.set_restore_volume(restore);

    // ---- connect ----
    inner.notify(StreamState::Connecting, Msg::new("status.connecting_device"));
    let connect = |h: &BtHost| {
        let t = Instant::now();
        let r = h.connect_retrying(req.device, &stopped, &mut |attempt, attempts| {
            tracing::debug!("connect: retry {attempt}/{attempts} after {} ms", t.elapsed().as_millis());
            inner.notify(StreamState::Connecting, Msg::with("status.connecting_retry", &[&attempt, &attempts]));
        });
        match &r {
            Ok(_) => tracing::debug!("connect: {addr} connected in {} ms", t.elapsed().as_millis()),
            Err(f) => tracing::debug!(
                "connect: {addr} failed after {} ms: {f:?} (last failure {:?})",
                t.elapsed().as_millis(),
                h.last_connect_failure()
            ),
        }
        r
    };
    let mut result = connect(&host);
    if result.is_err() && !stopped() && host.last_connect_failure() == Some(ConnectFailure::Stalled) {
        tracing::debug!("connect: the stack stalled, restarting it and connecting again");
        // Not the device: the stack it went through stopped working
        inner.notify(StreamState::Connecting, Msg::new("status.initializing_btstack"));
        host = match inner.restart_host("the Bluetooth adapter stopped working", req.max_media_payload) {
            Ok(h) => h,
            Err(m) => return inner.notify(StreamState::Error, m),
        };
        if stopped() {
            return inner.notify(StreamState::Idle, Msg::new("status.ready"));
        }
        inner.notify(StreamState::Connecting, Msg::new("status.connecting_device"));
        result = connect(&host);
    }
    let mut remote = match result {
        Ok(caps) => caps,
        Err(_) if stopped() => return inner.notify(StreamState::Idle, Msg::new("status.ready")),
        Err(f) => return inner.notify(StreamState::Error, rom_or(&host, connect_error(Some(f)))),
    };
    // However the session ends, the next one starts on a new stack
    let _streamed = StreamedMark(inner, req.device);
    // Huawei earbuds that do not list LDAC: asked for, not for Auto, as the
    // mode stays set on them and trades connection stability for it. No LDAC
    // from it: earbuds that had it off get it back, and a connection after it.
    if req.codec == Some(CodecKind::Ldac)
        && !remote.has(CodecKind::Ldac)
        && remote.audio_sinks_in_use == 0
        && crate::huawei::is_huawei(&req.device_name)
        && !stopped()
    {
        inner.notify(StreamState::Connecting, Msg::new("status.huawei_ldac"));
        if let Some(before) = crate::huawei::ask_for_ldac(&host, req.device) {
            let connect_again = || {
                host.disconnect();
                for _ in 0..10 {
                    if !stopped() {
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
                inner.notify(StreamState::Connecting, Msg::new("status.connecting_device"));
                connect(&host)
            };
            let mut result = connect_again();
            if before == 0
                && result.as_ref().is_ok_and(|caps| !caps.has(CodecKind::Ldac))
                && !stopped()
                && crate::huawei::set_connection_first(&host, req.device)
            {
                result = connect_again();
            }
            remote = match result {
                Ok(caps) => caps,
                Err(_) if stopped() => return inner.notify(StreamState::Idle, Msg::new("status.ready")),
                Err(f) => return inner.notify(StreamState::Error, rom_or(&host, connect_error(Some(f)))),
            };
        }
    }
    inner.record_codecs(&addr, KnownCodecs::of(&remote));
    tracing::info!("device connected, choosing the codec");
    tracing::debug!(
        "codec: device offers [{}], best {:?}, audio sinks {} ({} in use), aptX Adaptive {}",
        codec_keys(&remote).join(", "),
        remote.best().map(CodecKind::name),
        remote.audio_sinks,
        remote.audio_sinks_in_use,
        remote.aptx_adaptive
    );
    tracing::trace!("codec: device capabilities {remote:?}");
    if stopped() {
        host.disconnect();
        return inner.notify(StreamState::Idle, Msg::new("status.ready"));
    }

    // ---- codec ----
    if !req.auto_order.is_default() {
        tracing::info!("codec: the device's order set by the user: {}", req.auto_order.names());
    }
    let choice = match caps::choose_in(&remote, req.codec, req.codec_fallback, req.experimental_codecs, &req.auto_order)
    {
        Ok(c) => c,
        Err(why) => {
            tracing::debug!(
                "codec: no codec for request {:?} (fallback {}, experimental {}): {why:?}",
                req.codec.map(CodecKind::name),
                req.codec_fallback,
                req.experimental_codecs
            );
            host.disconnect();
            let msg = match why {
                None => Msg::new("error.no_compatible_codec"),
                Some((codec, Unavailable::SepInUse { .. })) => Msg::with("error.codec_sep_in_use", &[&codec.name()]),
                Some((codec, Unavailable::AdaptiveOnly)) => Msg::with("error.codec_adaptive_only", &[&codec.name()]),
                Some((codec, Unavailable::NotSupported)) => Msg::with("error.codec_not_supported", &[&codec.name()]),
                Some((codec, Unavailable::Experimental)) => Msg::with("error.codec_experimental_off", &[&codec.name()]),
            };
            return inner.notify(StreamState::Error, msg);
        }
    };
    let mut kind = choice.kind;
    let mut replaced = choice.replaced;
    if let Some((wanted, why)) = &replaced {
        tracing::warn!("codec fallback: {} not available ({why:?}), using {}", wanted.name(), kind.name());
    }
    tracing::info!("codec selected: {}", kind.name());
    tracing::debug!(
        "codec: requested {:?} (fallback {}), chose {} ({})",
        req.codec.map(CodecKind::name),
        req.codec_fallback,
        kind.name(),
        if req.codec.is_none() {
            "auto: best the device offers"
        } else if replaced.is_some() {
            "fallback"
        } else {
            "as requested"
        }
    );

    // ---- audio source ----
    inner.notify(StreamState::Connecting, Msg::new("status.initializing_audio"));
    // The rate on Auto (the profile's 0, Auto-select too): 48 kHz, each
    // codec's picker below going to the best the device lists without it
    // (44.1 kHz, say). Not the highest it lists nor the Windows mix rate
    // (issue #86: 96 kHz brought latency and trouble with no audible gain,
    // and Windows mixes at 48 kHz); 96 kHz when chosen. WASAPI resamples.
    let mut preferred = req.sample_rate.unwrap_or(AUTO_SAMPLE_RATE);
    tracing::debug!(
        "capture rate: {preferred} Hz preferred ({})",
        if req.sample_rate.is_some() { "profile" } else { "Auto" }
    );
    if preferred > kind.max_sample_rate() {
        tracing::info!(
            "limiting the capture rate from {preferred} to {} Hz for {}",
            kind.max_sample_rate(),
            kind.name()
        );
        preferred = kind.max_sample_rate();
    }
    // aptX family, AAC: 44.1 / 48 kHz as the sink lists them; LHDC V5, LDAC:
    // their rates;
    // Opus, Opus 05: 48 kHz only; aptX Adaptive: 48 kHz, 96 kHz when the
    // profile asks for it (HQ, an R2 sink that lists it). WASAPI resamples
    // to the requested rate.
    let picked = match kind {
        CodecKind::Aptx | CodecKind::AptxHd | CodecKind::AptxLl => remote.pick_aptx_sample_rate(kind, preferred),
        CodecKind::LhdcV5 => remote.pick_lhdcv5_sample_rate(preferred),
        CodecKind::Ldac => remote.pick_ldac_sample_rate(preferred),
        CodecKind::FastStream => remote.pick_faststream_sample_rate(preferred),
        CodecKind::AptxAdaptive => remote.pick_aptxad_sample_rate(preferred, req.quality.for_codec(kind)),
        CodecKind::Opus | CodecKind::Opus05 => 48000,
        CodecKind::Aac => remote.pick_aac_sample_rate(preferred),
        CodecKind::Sbc | CodecKind::SbcXq if preferred != 44100 => 48000,
        _ => preferred,
    };
    if picked != preferred {
        tracing::info!("{}: capturing at {picked} Hz instead of {preferred} Hz (supported rate)", kind.name());
        preferred = picked;
    }

    let mut default_guard = DefaultDeviceGuard(None);
    // Restored while the headphones are off: switched again when they are back
    let mut switch_default_again = false;
    default_guard.follow(&req.source);
    let opened = match open_source(&req.source, preferred) {
        Ok(o) => o,
        Err(m) => {
            host.disconnect();
            return inner.notify(StreamState::Error, m);
        }
    };
    let format = source_format(&opened, preferred);
    let channels = (format.channels as u32).min(2);
    tracing::info!(
        "capture: {} Hz, {} channel(s), {:?} -> {channels} channel(s)",
        format.sample_rate,
        format.channels,
        format.sample_format
    );

    // LDAC, aptX HD and LHDC V5 carry 24-bit unless the profile says 16
    // (LHDC V5 only when the sink has 24-bit); aptX Adaptive always
    let bits: u8 = match (kind, req.bit_depth) {
        (CodecKind::AptxAdaptive, _) => 24,
        (_, Some(16)) => 16,
        (CodecKind::Ldac | CodecKind::AptxHd, _) => 24,
        (CodecKind::LhdcV5, _) if remote.lhdcv5_24bit() => 24,
        _ => 16,
    };

    // AAC with variable bit rate: asked for by name, and the sink lists it
    let vbr = req.abr && req.codec == Some(CodecKind::Aac) && kind == CodecKind::Aac && remote.aac_vbr();
    tracing::debug!(
        "codec: {bits}-bit (profile {:?}), aac vbr {vbr} (abr {}, device vbr {})",
        req.bit_depth,
        req.abr,
        remote.aac_vbr()
    );

    // ---- stream configuration ----
    let t = Instant::now();
    let mut configured =
        configure_stream(&host, &remote, kind, format.sample_rate, channels, bits, vbr, req.sbc_params(kind));
    if let Err(failure) = &configured {
        tracing::debug!("configure: {} failed after {} ms: {failure:?}", kind.name(), t.elapsed().as_millis());
    }
    if let Err(failure) = &configured
        && !stopped()
        && sbc_xq_fallback(kind, failure, req.codec, req.codec_fallback)
    {
        // The sink lists dual channel but refuses it. BTstack's A2DP setup
        // of this connection ends with the refusal: connect again and take
        // plain SBC (SBC XQ is a dual-channel SBC: same rates, channels and
        // bits)
        tracing::warn!("SBC XQ: the device refused its configuration ({failure:?}), connecting again for SBC");
        host.disconnect();
        inner.notify(StreamState::Connecting, Msg::new("status.connecting_device"));
        remote = match connect(&host) {
            Ok(caps) => caps,
            Err(_) if stopped() => return inner.notify(StreamState::Idle, Msg::new("status.ready")),
            Err(f) => return inner.notify(StreamState::Error, rom_or(&host, connect_error(Some(f)))),
        };
        kind = CodecKind::Sbc;
        if req.codec.is_some() && replaced.is_none() {
            replaced = Some((CodecKind::SbcXq, Unavailable::NotSupported));
        }
        configured =
            configure_stream(&host, &remote, kind, format.sample_rate, channels, bits, vbr, req.sbc_params(kind));
        if let Err(failure) = &configured {
            tracing::debug!("configure: SBC after SBC XQ failed: {failure:?}");
        }
    }
    let (mut config, mtu) = match configured {
        Ok(c) => {
            tracing::debug!("configure: {:?} in {} ms, media payload {}", c.0, t.elapsed().as_millis(), c.1);
            c
        }
        Err(e) => {
            if let ConfigFailure::Build(e) = &e {
                tracing::error!("{e}");
            }
            host.disconnect();
            return if stopped() {
                inner.notify(StreamState::Idle, Msg::new("status.ready"))
            } else {
                inner.notify(StreamState::Error, rom_or(&host, Msg::new("error.codec_configure")))
            };
        }
    };
    let kind = kind;
    let fallback_text = replaced.as_ref().map(|(wanted, why)| {
        let key = match why {
            Unavailable::SepInUse { .. } => "status.connected_fallback_in_use",
            Unavailable::Experimental => "status.connected_fallback_experimental",
            _ => "status.connected_fallback",
        };
        Msg::with(key, &[&wanted.name(), &kind.name()])
    });
    // The qualities each codec offers (issue #86): one it does not (a
    // profile saved before) is High, the codec's highest bit rate
    let quality = req.quality.for_codec(kind);
    if quality != req.quality {
        tracing::info!("{}: quality {:?} is not one of its own, using {quality:?}", kind.name(), req.quality);
    }
    let mut encoder_config = EncoderConfig {
        sample_rate: format.sample_rate,
        channels,
        quality,
        mtu,
        bits_per_sample: bits,
        // For AAC: variable bit rate, where the sink lists it
        abr: req.abr
            && (matches!(kind, CodecKind::Ldac | CodecKind::LhdcV5 | CodecKind::AptxAdaptive)
                || (kind == CodecKind::Aac && vbr))
            || sbc_abr(&req, kind),
        peer: peer_params(&remote, kind, channels, &req),
    };
    tracing::debug!("encoder config: {encoder_config:?}");
    let encoder = match codec::create(kind, &encoder_config) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!("encoder: {e}");
            host.disconnect();
            return inner.notify(StreamState::Error, Msg::new("error.encoder_init"));
        }
    };
    let bitrate = encoder.bitrate_kbps();
    let bitrate_capped = encoder.bitrate_capped();
    // ABR as the encoder runs it (LDAC keeps a fixed quality when its ABR
    // module fails to start)
    let abr = encoder.abr_active() || vbr;
    tracing::info!("encoder ready: {} {bitrate} kbps, mtu {mtu}, {bits}-bit, ABR {abr}", kind.name());

    if !host.start_stream() {
        tracing::debug!("stream: start failed (stop requested {})", stopped());
        host.disconnect();
        return if stopped() {
            inner.notify(StreamState::Idle, Msg::new("status.ready"))
        } else {
            inner.notify(StreamState::Error, rom_or(&host, Msg::new("error.stream_start")))
        };
    }

    tracing::debug!("stream: started {} ms after the request", session_start.elapsed().as_millis());
    inner.notify(StreamState::Streaming, fallback_text.clone().unwrap_or_else(|| Msg::new("status.connected")));
    let mut info = StreamInfo {
        codec: kind.name().into(),
        bitrate_kbps: bitrate,
        sample_rate: format.sample_rate,
        channels,
        bits: bits as u32,
        source_sample_rate: format.sample_rate,
        source_channels: format.channels as u32,
        source_bits: (format.sample_format.bytes_per_sample() * 8) as u32,
        requested_codec: replaced.as_ref().map(|(w, _)| w.name().to_string()),
        requested_in_use: matches!(replaced, Some((_, Unavailable::SepInUse { .. }))),
        abr,
        bitrate_capped,
        source: req.source.info(),
        details: Some(Box::new(codec_details(kind, &config, &remote, &*encoder, mtu, format.sample_rate))),
    };
    inner.save_device(&addr, &req.device_name);

    // ---- pipeline and capture ----
    let Some(sink) = host.take_media_sink() else {
        host.disconnect();
        return inner.notify(StreamState::Error, Msg::new("error.encode_thread"));
    };
    let (pipeline, input) = start_pipeline(encoder, format, channels, mtu, sink, &host);
    let input = SharedInput(Arc::new(Mutex::new(input)));
    let Some(mut active) = start_source(opened, &req.source, preferred, &input) else {
        if let Some(s) = pipeline.stop() {
            host.return_media_sink(s);
        }
        host.disconnect();
        return inner.notify(StreamState::Error, Msg::new("error.audio_capture_start"));
    };
    info.source.waiting = active.waiting();
    inner.stream_info(info.clone());

    // Headphone volume and a Windows volume act as one (volume_sync.rs)
    let mut volume_choice = crate::volume_sync::EndpointChoice::for_stream(&req.volume_sync, &req.source);
    let mut volume = volume_choice.clone().map(crate::volume_sync::VolumeSyncer::new);
    let mut output_mute = OutputMute::new();
    output_mute.tick(req.mutes_output(&req.source));

    // A reconnect builds the configuration anew from what the device lists
    // then (v1 read its capabilities again)
    let rebuild = |r: &RemoteCaps| match r.configuration_with(
        kind,
        format.sample_rate,
        channels,
        bits,
        vbr,
        req.sbc_params(kind),
    ) {
        Ok(c) => Some(c),
        Err(e) => {
            tracing::warn!("reconnect: {e}");
            None
        }
    };

    // ---- streaming ----
    let mut pipeline = Some(pipeline);
    // What the session captures: the request's until another source is chosen
    let mut current_source = req.source.clone();
    let mut app_ticks = 0u32;
    // What the capture delivers now: a device opened again may come back
    // with other channels or samples (the rate is the stream's, converted
    // by Windows)
    let mut input_format = format;
    let mut reopen_ticks = 0u32;
    let mut lost_for_good = false;
    let mut discontinuities = DiscontinuityLog::default();
    let mut health = StreamHealth::start(host.stats().clone(), format.sample_rate, active.health());
    while !stopped() {
        inner.wait_tick(Duration::from_millis(200));
        if stopped() {
            break;
        }
        if let Some(v) = volume.as_mut() {
            v.tick(true, &*host);
        }
        output_mute.tick(req.mutes_output(&current_source));
        inner.remember_volume(&addr, host.remote_volume());

        // Another source chosen: only the capture changes, the encoder (at
        // the same rate) and the link go on. A source that cannot start ends
        // the session as it would at its start.
        if let Some(next) = inner.take_source_change() {
            tracing::info!("audio source: {current_source} -> {next}");
            // Stopped first: a driver's endpoints may not capture side by side
            active.stop();
            default_guard.follow(&next);
            let opened = match open_source(&next, format.sample_rate) {
                Ok(o) => o,
                Err(m) => {
                    inner.notify(StreamState::Error, m);
                    lost_for_good = true;
                    break;
                }
            };
            let new_format = source_format(&opened, format.sample_rate);
            if new_format != input_format {
                tracing::debug!("audio source: format {input_format:?} -> {new_format:?}, starting the pipeline anew");
                input_format = new_format;
                info.source_channels = new_format.channels as u32;
                info.source_bits = (new_format.sample_format.bytes_per_sample() * 8) as u32;
                let renewed =
                    restart_pipeline(&mut pipeline, kind, &encoder_config, input_format, channels, &input, &host);
                if let Err(m) = renewed {
                    inner.notify(StreamState::Error, m);
                    lost_for_good = true;
                    break;
                }
            }
            let Some(started) = start_source(opened, &next, format.sample_rate, &input) else {
                inner.notify(StreamState::Error, Msg::new("error.audio_capture_start"));
                lost_for_good = true;
                break;
            };
            active = started;
            reopen_ticks = 0;
            let choice = crate::volume_sync::EndpointChoice::for_stream(&req.volume_sync, &next);
            if choice != volume_choice {
                if let Some(mut v) = volume.take() {
                    v.stop();
                }
                volume = choice.clone().map(crate::volume_sync::VolumeSyncer::new);
                volume_choice = choice;
            }
            info.source = next.info();
            info.source.waiting = active.waiting();
            inner.stream_info(info.clone());
            current_source = next;
        }

        // Application capture follows the app: it may quit and start again
        if let ActiveSource::App(app) = &mut active {
            if app.exited() {
                tracing::info!("{} exited, waiting for it to start again", app.exe);
                app.detach();
            }
            app_ticks += 1;
            if app.running.is_none() && app_ticks.is_multiple_of(5) {
                app.attach(&input);
            }
        }
        // Device capture that Windows ended (device removed, disabled, its
        // format changed or taken in exclusive mode) is opened again: at
        // once, every second for ten tries, then every ten seconds
        if let ActiveSource::Capture(dev) = &mut active {
            if dev.ended() {
                let why = dev.running.as_ref().and_then(Running::end_reason);
                tracing::warn!("audio capture ended ({why:?}), opening the device again");
                dev.detach();
                reopen_ticks = 0;
            }
            if dev.running.is_none() {
                reopen_ticks += 1;
                let every = if reopen_ticks <= 50 { 5 } else { 50 };
                if reopen_ticks % every == 1
                    && let Some(capture) = dev.reopen()
                {
                    let new_format = capture.format();
                    if new_format != input_format {
                        tracing::info!(
                            "audio capture: format {} Hz {} ch {:?} -> {} Hz {} ch {:?}, starting the pipeline anew",
                            input_format.sample_rate,
                            input_format.channels,
                            input_format.sample_format,
                            new_format.sample_rate,
                            new_format.channels,
                            new_format.sample_format
                        );
                        input_format = new_format;
                        info.source_channels = new_format.channels as u32;
                        info.source_bits = (new_format.sample_format.bytes_per_sample() * 8) as u32;
                        let renewed = restart_pipeline(
                            &mut pipeline,
                            kind,
                            &encoder_config,
                            input_format,
                            channels,
                            &input,
                            &host,
                        );
                        if let Err(m) = renewed {
                            inner.notify(StreamState::Error, m);
                            lost_for_good = true;
                            break;
                        }
                        inner.stream_info(info.clone());
                    }
                    dev.start(capture, &input);
                }
            }
        }
        if active.waiting() != info.source.waiting {
            info.source.waiting = active.waiting();
            inner.stream_info(info.clone());
        }
        discontinuities.tick(active.running().map_or(0, Running::discontinuities));
        health.tick(|| active.health());

        // Sleep, or an adapter that stopped working, leaves a stack that
        // reports no disconnect: it is started anew and the device connected
        // again on it
        let lost = host.check_disconnected();
        let check = check_link(lost, host.stalled(), inner.power.stale(), inner.power.suspending());
        // Headphones turned back on: connecting to them is no failure
        let mut back_on = false;
        let mut restart_why = match check {
            LinkCheck::Fine => continue,
            LinkCheck::Reconnect if device_turned_off(inner, &host) => {
                // As Windows and Android: the headphones went off, nothing
                // to retry; when they are back on they connect to the PC
                // and the stream resumes
                tracing::info!("the headphones were turned off or disconnected themselves");
                inner.notify(StreamState::Idle, Msg::new("status.device_off"));
                // Windows plays through the device before (its speakers)
                // meanwhile, as after Disconnect (#132)
                if default_guard.0.is_some() {
                    default_guard = DefaultDeviceGuard(None);
                    switch_default_again = true;
                }
                output_mute.tick(false);
                if !wait_for_device(inner, &host, req.device, &mut pipeline) {
                    lost_for_good = true;
                    break;
                }
                tracing::info!("the headphones connected again, resuming");
                back_on = true;
                None
            }
            LinkCheck::Reconnect => {
                tracing::warn!("connection lost, reconnecting");
                None
            }
            LinkCheck::Restart(RESUMED) => {
                tracing::info!("{RESUMED}, reconnecting");
                Some(RESUMED)
            }
            LinkCheck::Restart(why) => {
                tracing::warn!("{why}, reconnecting");
                Some(why)
            }
            LinkCheck::SleepThrough => {
                inner.notify(StreamState::Reconnecting, Msg::new("status.connection_lost"));
                let Some(why) = sleep_through(inner, &mut pipeline) else { break };
                tracing::info!("{why}, reconnecting");
                Some(why)
            }
        };
        tracing::debug!("reconnect: link check {check:?}, restart reason {restart_why:?}");
        let (lost_state, lost_msg) = if back_on {
            (StreamState::Connecting, "status.device_back")
        } else {
            (StreamState::Reconnecting, "status.connection_lost")
        };
        inner.notify(lost_state, Msg::new(lost_msg));
        let reconnect_start = Instant::now();

        let mut reconnected = None;
        let mut codec_gone = false;
        let mut fresh_stack = false;
        let mut attempt = 0u32;
        let mut restarted = false;
        loop {
            if stopped() {
                break;
            }
            // Sleep that comes while reconnecting: no attempt is spent on it
            if inner.power.suspending() {
                let Some(why) = sleep_through(inner, &mut pipeline) else { break };
                tracing::info!("{why}, reconnecting");
                restart_why = Some(why);
            }
            restarted |= restart_why.is_some();
            let attempts = reconnect_attempts(restarted);
            if attempt >= attempts {
                break;
            }
            attempt += 1;
            inner.notify(lost_state, Msg::with("status.reconnect_attempt", &[&attempt, &attempts]));
            tracing::debug!("reconnect: attempt {attempt}/{attempts} in 3000 ms");
            // React to a stop within the wait
            let until = Instant::now() + Duration::from_secs(3);
            while Instant::now() < until && !stopped() {
                inner.wait_tick(Duration::from_millis(100));
            }
            if stopped() {
                break;
            }
            if inner.power.suspending() {
                tracing::debug!("reconnect: going to sleep during the wait, attempt {attempt} not counted");
                // Went to sleep in the wait: the next round sleeps through
                attempt -= 1;
                continue;
            }
            if restart_why.is_none() {
                if inner.power.stale() {
                    restart_why = Some(RESUMED);
                } else if host.stalled() {
                    restart_why = Some(ADAPTER_STOPPED);
                }
            }
            if let Some(why) = restart_why.take() {
                tracing::debug!("reconnect: attempt {attempt} restarts the stack: {why}");
                // The pipeline's media sink belongs to the old stack
                if let Some(p) = pipeline.take() {
                    let _ = p.stop();
                }
                let t = Instant::now();
                match inner.restart_host(why, req.max_media_payload) {
                    Ok(h) => {
                        tracing::debug!("reconnect: new stack in {} ms", t.elapsed().as_millis());
                        host = h;
                        fresh_stack = true;
                    }
                    Err(_) if inner.power.suspending() => {
                        tracing::debug!(
                            "reconnect: stack restart failed going to sleep, attempt {attempt} not counted"
                        );
                        // The adapter went away for sleep meanwhile
                        attempt -= 1;
                        continue;
                    }
                    Err(m) => {
                        tracing::debug!(
                            "reconnect: stack restart failed after {} ms: {}",
                            t.elapsed().as_millis(),
                            m.english()
                        );
                        tracing::warn!("reconnect attempt {attempt} failed (no Bluetooth adapter)");
                        restart_why = Some("the last restart failed");
                        continue;
                    }
                }
                host.set_restore_volume(inner.saved_volume(&addr));
                host.set_media_keys(inner.media_keys());
                if stopped() {
                    break;
                }
            }
            // A new stack knows neither the device nor the stream: connect
            // and configure in full
            let t = Instant::now();
            let result = if fresh_stack {
                match host.connect(req.device) {
                    Ok(caps) => host.configure_and_start(caps, &config, &rebuild),
                    Err(f) => {
                        tracing::debug!("reconnect: connect on the new stack failed: {f:?}");
                        Err(ReconnectError::Failed)
                    }
                }
            } else {
                host.reconnect(&rebuild)
            };
            tracing::debug!(
                "reconnect: attempt {attempt} ({}) took {} ms: {:?}",
                if fresh_stack { "new stack" } else { "same stack" },
                t.elapsed().as_millis(),
                result.as_ref().map(|r| r.mtu)
            );
            match result {
                Ok(r) => {
                    tracing::info!("reconnected on attempt {attempt}");
                    tracing::debug!(
                        "reconnect: back after {} ms, config {:?}",
                        reconnect_start.elapsed().as_millis(),
                        r.config
                    );
                    reconnected = Some(r);
                    break;
                }
                Err(ReconnectError::CodecGone) => {
                    tracing::debug!("reconnect: the device no longer offers {}", kind.name());
                    codec_gone = true;
                    break;
                }
                Err(ReconnectError::Failed) => {}
            }
            if inner.power.suspending() {
                tracing::info!("reconnect attempt {attempt} ended by sleep, not counted");
                attempt -= 1;
                continue;
            }
            tracing::warn!("reconnect attempt {attempt} failed");
        }
        let Some(r) = reconnected else {
            tracing::debug!(
                "reconnect: gave up after {attempt} attempt(s), {} ms (stopped {}, codec gone {codec_gone}, restart pending {restart_why:?})",
                reconnect_start.elapsed().as_millis(),
                stopped()
            );
            if !stopped() {
                if codec_gone {
                    inner.notify(StreamState::Error, Msg::with("error.codec_not_supported", &[&kind.name()]));
                } else if restart_why.is_none() {
                    // The adapter works, the headphones never answered: off
                    // or out of range, not an error of A2DPWB
                    inner.notify(StreamState::Idle, Msg::new("status.device_unreachable"));
                } else {
                    inner.notify(StreamState::Error, Msg::new("error.reconnect_failed"));
                }
                lost_for_good = true;
            }
            break;
        };
        // The encoder follows the new stream: its MTU and the device's
        // settings (e.g. its SBC bitpool range) may have changed
        let new_config =
            EncoderConfig { mtu: r.mtu, peer: peer_params(&r.remote, kind, channels, &req), ..encoder_config };
        if r.mtu != encoder_config.mtu {
            tracing::info!("reconnect: media payload size {} -> {}", encoder_config.mtu, r.mtu);
        }
        let changed = new_config != encoder_config;
        tracing::debug!(
            "reconnect: encoder config changed {changed}, pipeline {}",
            if pipeline.is_some() && !changed { "reset" } else { "started anew" }
        );
        encoder_config = new_config;
        config = r.config;
        // Resume with the same encoder when nothing changed; otherwise, and
        // on a new stack (its media sink is new), with a new pipeline
        if let Some(p) = pipeline.as_ref()
            && !changed
        {
            p.reset();
            // The Idle while the headphones were off cleared the stream
            // info (the UI's, the connected balloon's): the same again
            if back_on {
                inner.stream_info(info.clone());
            }
        } else {
            let sink = match pipeline.take() {
                Some(p) => p.stop(),
                None => host.take_media_sink(),
            };
            let Some(sink) = sink else {
                inner.notify(StreamState::Error, Msg::new("error.encode_thread"));
                lost_for_good = true;
                break;
            };
            let encoder = match codec::create(kind, &encoder_config) {
                Ok(e) => e,
                Err(e) => {
                    tracing::error!("encoder: {e}");
                    host.return_media_sink(sink);
                    inner.notify(StreamState::Error, Msg::new("error.encoder_init"));
                    lost_for_good = true;
                    break;
                }
            };
            if changed {
                tracing::info!(
                    "encoder set up again: {} {} kbps, mtu {}, ABR {}",
                    kind.name(),
                    encoder.bitrate_kbps(),
                    r.mtu,
                    encoder.abr_active()
                );
            }
            info.bitrate_kbps = encoder.bitrate_kbps();
            info.bitrate_capped = encoder.bitrate_capped();
            info.abr = encoder.abr_active() || vbr;
            info.details =
                Some(Box::new(codec_details(kind, &config, &r.remote, &*encoder, r.mtu, format.sample_rate)));
            let (p, new_input) = start_pipeline(encoder, input_format, channels, r.mtu, sink, &host);
            if let Ok(mut i) = input.0.lock() {
                *i = new_input;
            }
            pipeline = Some(p);
            inner.stream_info(info.clone());
        }
        if switch_default_again && let SourceRequest::VirtualDevice { id, name, .. } = &current_source {
            default_guard = DefaultDeviceGuard::switch_to(id, name);
            switch_default_again = false;
        }
        inner.notify(StreamState::Streaming, Msg::new("status.reconnected"));
    }

    // ---- cleanup: producer first, then the encode thread, then the stream ----
    tracing::info!("streaming stopped, cleaning up");
    tracing::debug!(
        "stream: ended after {} s (stop requested {}, lost for good {lost_for_good})",
        session_start.elapsed().as_secs(),
        stopped()
    );
    health.finish(active.health());
    active.stop();
    if let Some(p) = pipeline.take()
        && let Some(s) = p.stop()
    {
        host.return_media_sink(s);
    }
    if let Some(mut v) = volume.take() {
        v.stop();
    }
    host.stop_stream();
    host.disconnect();
    host.set_restore_volume(None);
    drop(output_mute);
    drop(default_guard);
    if !lost_for_good {
        inner.notify(StreamState::Idle, Msg::new("status.ready"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sbc_abr_only_for_a_profile_of_that_codec() {
        let req = |codec: &str, abr: bool| {
            crate::profiles::Profile {
                device_address: "AA:BB:CC:DD:EE:FF".into(),
                codec: codec.into(),
                abr,
                ..Default::default()
            }
            .to_request(1023)
            .unwrap()
        };
        assert!(sbc_abr(&req("sbc", true), CodecKind::Sbc));
        assert!(sbc_abr(&req("sbcxq", true), CodecKind::SbcXq));
        // ABR off, or another codec asked for (Auto, a fallback from LDAC)
        assert!(!sbc_abr(&req("sbc", false), CodecKind::Sbc));
        assert!(!sbc_abr(&req("auto", true), CodecKind::Sbc));
        assert!(!sbc_abr(&req("ldac", true), CodecKind::Sbc));
        assert!(!sbc_abr(&req("sbcxq", true), CodecKind::Sbc));
        assert!(!sbc_abr(&req("faststream", true), CodecKind::FastStream));
    }

    #[test]
    fn a_working_link_is_left_alone() {
        assert_eq!(check_link(false, false, false, false), LinkCheck::Fine);
    }

    #[test]
    fn failures_while_awake_reconnect_or_restart() {
        assert_eq!(check_link(true, false, false, false), LinkCheck::Reconnect);
        assert_eq!(check_link(false, true, false, false), LinkCheck::Restart(ADAPTER_STOPPED));
        assert_eq!(check_link(true, true, false, false), LinkCheck::Restart(ADAPTER_STOPPED));
    }

    #[test]
    fn after_resume_the_stack_is_restarted() {
        assert_eq!(check_link(false, false, true, false), LinkCheck::Restart(RESUMED));
        assert_eq!(check_link(true, true, true, false), LinkCheck::Restart(RESUMED));
    }

    #[test]
    fn going_to_sleep_sleeps_through_a_lost_adapter_or_link() {
        // The hardware log of 2026-10-17: transfers cancelled after the
        // going-to-sleep notification
        assert_eq!(check_link(false, true, false, true), LinkCheck::SleepThrough);
        assert_eq!(check_link(true, false, false, true), LinkCheck::SleepThrough);
        assert_eq!(check_link(true, true, false, true), LinkCheck::SleepThrough);
        // A stack still stale from an earlier sleep is not restarted now
        assert_eq!(check_link(false, true, true, true), LinkCheck::SleepThrough);
    }

    #[test]
    fn sources_as_the_ui_shows_them() {
        assert_eq!(SourceRequest::TestTone.info(), SourceInfo { kind: "tone", name: String::new(), waiting: false });
        let app = SourceRequest::Application { exe: "Spotify.exe".into() }.info();
        assert_eq!((app.kind, app.name.as_str()), ("app", "Spotify.exe"));
        let dev = SourceRequest::VirtualDevice { id: "{x}".into(), name: "CABLE".into(), auto_switch: true }.info();
        assert_eq!((dev.kind, dev.name.as_str()), ("virtual", "CABLE"));
        assert_eq!(SourceRequest::SystemLoopback.info().kind, "loopback");
    }

    #[test]
    fn codec_keys_of_a_device() {
        let mut remote = caps::RemoteCaps::default();
        assert!(codec_keys(&remote).is_empty());
        remote.add_sbc(1);
        remote.add_aac(2);
        let keys = codec_keys(&remote);
        assert!(keys.contains(&"sbc".to_string()) && keys.contains(&"aac".to_string()));
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn going_to_sleep_keeps_a_working_stream() {
        assert_eq!(check_link(false, false, false, true), LinkCheck::Fine);
        assert_eq!(check_link(false, false, true, true), LinkCheck::Fine);
    }

    #[test]
    fn the_device_before_is_restored_unless_it_was_the_target() {
        assert_eq!(restore_target(Some("{a}".into()), "{b}"), Some("{a}".into()));
        assert_eq!(restore_target(Some("{b}".into()), "{b}"), None);
        assert_eq!(restore_target(None, "{b}"), None);
    }

    #[test]
    fn the_default_output_stays_switched_for_a_session_to_the_same_device() {
        // Settings changed: the next session switches to the same device
        let mut h = DefaultHandover { next: Some("{cable}".into()), restore: None };
        assert_eq!(h.end(Some("{cable}"), "{speakers}".into()), None);
        assert_eq!(h.restore.as_deref(), Some("{speakers}"));
        // Not left to restore while the next session starts
        assert_eq!(h.unclaimed(), None);
        // It never switched (it failed before): restored when it ends
        h.next = None;
        assert_eq!(h.unclaimed().as_deref(), Some("{speakers}"));
        assert_eq!(h, DefaultHandover::default());
        // Another device next, or no session next: restored now
        let mut h = DefaultHandover { next: Some("{other}".into()), restore: None };
        assert_eq!(h.end(Some("{cable}"), "{speakers}".into()).as_deref(), Some("{speakers}"));
        let mut h = DefaultHandover::default();
        assert_eq!(h.end(Some("{cable}"), "{speakers}".into()).as_deref(), Some("{speakers}"));
        assert_eq!(h, DefaultHandover::default());
    }

    #[test]
    fn only_an_auto_switched_virtual_device_is_made_the_default() {
        let virt =
            |id: &str, auto_switch| SourceRequest::VirtualDevice { id: id.into(), name: "CABLE".into(), auto_switch };
        assert_eq!(virt("{x}", true).default_switch(), Some("{x}"));
        assert_eq!(virt("{x}", false).default_switch(), None);
        assert_eq!(virt("", true).default_switch(), None);
        assert_eq!(SourceRequest::SystemLoopback.default_switch(), None);
    }

    #[test]
    fn discontinuities_are_logged_as_they_increase_at_most_every_2_s() {
        let t0 = Instant::now();
        let at = |ms: u64| t0 + Duration::from_millis(ms);
        let mut log = DiscontinuityLog::default();
        assert_eq!(log.check(0, at(0)), None);
        assert_eq!(log.check(1, at(200)), Some((1, 1)));
        // More within 2 s: held back, reported together after
        assert_eq!(log.check(3, at(400)), None);
        assert_eq!(log.check(4, at(2000)), None);
        assert_eq!(log.check(4, at(2200)), Some((4, 3)));
        // No change: nothing to say, however long
        assert_eq!(log.check(4, at(9000)), None);
        assert_eq!(log.check(5, at(9200)), Some((5, 1)));
        // A new capture counts from 0 again
        assert_eq!(log.check(2, at(12000)), Some((2, 2)));
    }

    #[test]
    fn a_refused_sbc_xq_falls_back_to_sbc_when_allowed() {
        let refused = ConfigFailure::Host(ConfigureError::Refused);
        let no_answer = ConfigFailure::Host(ConfigureError::NoAnswer);
        let xq = CodecKind::SbcXq;
        assert!(sbc_xq_fallback(xq, &refused, None, false));
        assert!(sbc_xq_fallback(xq, &no_answer, None, false));
        assert!(sbc_xq_fallback(xq, &refused, Some(xq), true));
        // Asked for SBC XQ without codec fallback: an error, as before
        assert!(!sbc_xq_fallback(xq, &refused, Some(xq), false));
        // Not connected any more, or no configuration of it: no retry
        assert!(!sbc_xq_fallback(xq, &ConfigFailure::Host(ConfigureError::NotConnected), None, false));
        assert!(!sbc_xq_fallback(xq, &ConfigFailure::Build(ConfigError::NotOffered(xq)), None, false));
        // Other codecs are not retried
        assert!(!sbc_xq_fallback(CodecKind::Ldac, &refused, None, true));
    }
}
