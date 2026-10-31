//! An LE Audio (experimental) streaming session: v1's
//! A2dpService::le_audio_streaming_thread_inner (a2dp_service_le_audio.cpp),
//! on the session thread of [`crate::Service`] for a [`StreamRequest`] whose
//! `le` is Some.
//!
//! Same lifecycle as the A2DP session ([`crate::stream`]): refuse when the
//! setting is off (error.le_audio_disabled) or the capture mode is "app"
//! (error.le_audio_capture); BTstack up with `HostConfig::le_audio` (wait
//! up to 5 s for `BtHost::le_ready`, else error.le_audio_unavailable);
//! find and connect the device and the extra devices (`le_find` first for
//! the address type, then `le_connect`; error.le_audio_connect); the stream
//! request from `LeRequest` (preset -> preset order, low latency ->
//! `le_set_low_latency`); `le_start_stream` (error.le_audio_stream);
//! capture at the LC3 rate into pipeline's LE encode thread, which pushes
//! SDU sets into the returned sink; VolumeSync against
//! `le_remote_volume` / `le_request_volume` (the LeHeadphones target) when
//! a module knows a volume; on `le_stream_lost` stop and set the stream up
//! again ("LE Audio stream restored"; up to 10 times); on stop
//! `le_stop_stream` and `le_disconnect_all`.
//!
//! [`LeAudioPath`] (capture -> LE encode thread -> the stream's SDU sink) is
//! shared with the command line's LE Audio flow (cli.rs).

use std::sync::Arc;
use std::time::{Duration, Instant};

use audio_capture::{Capture, CaptureOptions, Format, Running, Sink, Source};
use bt_host::{BtHost, LeAddr, LeDeviceId, LeRequest, LeStreamRequest, LeStreamStarted, LinkStats};
use le_audio::plan::StreamPlan;
use le_audio::sdu_queue::SduSet;
use pipeline::le::{LePipeline, LePipelineConfig};
use pipeline::{BufferPreset, CaptureInput};

use crate::messages::Msg;
use crate::service::Inner;
use crate::stream::{
    DefaultDeviceGuard, DiscontinuityLog, SourceRequest, StreamInfo, StreamRequest, StreamState, VolumeSync,
};

/// Waits for the LE core after BTstack started (v1: 50 x 100 ms)
const READY_WAIT: Duration = Duration::from_secs(5);
/// `le_find` before connecting, for the address type and name (v1 GUI)
const FIND_TIMEOUT: Duration = Duration::from_secs(8);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// Set-ups after a lost stream (v1's command line; its GUI tried 10 times,
/// up to 40 s each)
const RECONNECT_ATTEMPTS: u32 = 3;
const RECONNECT_WAIT: Duration = Duration::from_secs(3);
/// One scan for headphones that were turned off; short to see a stop
const OFF_FIND: Duration = Duration::from_secs(2);
/// The codec name the UI shows (v1)
pub const CODEC_NAME: &str = "LC3 (LE Audio)";

/// The silence set of a plan for `BtHost::le_start_stream`: one encoded
/// silent LC3 SDU per CIS
pub fn le_silence_set(plan: &StreamPlan) -> SduSet {
    pipeline::le::silence_set(plan)
}

/// v1's statistics line of the command line ("LE Audio: N SDU sets sent (M
/// SDUs), E encoded, S silence, U underruns, D dropped, X send errors")
pub fn le_stats_line(s: &bt_host::LeStreamStats) -> String {
    le_audio::sdu_queue::SendCounters {
        sets_encoded: s.sets_encoded,
        sets_sent: s.sets_sent,
        sdus_sent: s.sdus_sent,
        silence_sets: s.silence_sets,
        underruns: s.underruns,
        overflows: s.overflows,
        send_errors: s.send_errors,
    }
    .stats_message()
}

/// What an LE Audio stream was set up with (the UI's stream details)
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LeStreamDetails {
    /// BAP preset ("48_4_2"; GMAP low latency "48_4_gr")
    pub preset: String,
    pub frame_us: u32,
    /// LC3 octets per channel and frame
    pub octets: u32,
    /// Connected isochronous streams
    pub cis: u32,
    /// Devices streamed to (both earbuds of a coordinated set: 2)
    pub devices: u32,
    /// The GMAP low-latency configuration
    pub low_latency: bool,
    /// English, v1's StreamPlan::describe
    pub summary: String,
}

impl LeStreamDetails {
    pub fn of(started: &LeStreamStarted) -> LeStreamDetails {
        let status = &started.status;
        let plan = &status.plan;
        LeStreamDetails {
            preset: plan.preset.name.clone(),
            frame_us: plan.preset.frame_us.into(),
            octets: plan.preset.octets.into(),
            cis: plan.cis.len() as u32,
            devices: status.devices.len() as u32,
            low_latency: plan.preset.name.ends_with("_gr"),
            summary: status.summary.clone(),
        }
    }
}

/// The capture's end of the LE encode thread's ring
struct InputSink(CaptureInput);

impl Sink for InputSink {
    fn audio(&mut self, data: &[u8], _frames: usize, _format: Format) {
        self.0.push(data);
    }

    fn silence(&mut self, frames: usize, _format: Format) {
        self.0.push_silence(frames);
    }
}

/// Audio into a running LE stream: capture (or the test tone) at the LC3
/// rate (WASAPI converts to it), the LE encode thread (downmix, resample if
/// still needed, LC3 per channel), the SDU sets into the stream's sink
pub struct LeAudioPath {
    capture: Option<Running>,
    pipeline: Option<LePipeline>,
    format: Format,
    rate: u32,
}

impl LeAudioPath {
    /// The capture source for `source` (application capture is not
    /// supported with LE Audio: error.le_audio_capture before this)
    fn capture_source(source: &SourceRequest) -> Result<Source, String> {
        match source {
            SourceRequest::TestTone => Ok(Source::TestTone),
            SourceRequest::SystemLoopback => Ok(Source::SystemLoopback),
            SourceRequest::VirtualDevice { id, .. } if !id.is_empty() => Ok(Source::RenderDevice { id: id.clone() }),
            SourceRequest::VirtualDevice { .. } => Err("no virtual device selected".into()),
            SourceRequest::Application { .. } => Err("application capture is not supported with LE Audio".into()),
        }
    }

    /// Starts capture and encoding for the stream `started`
    pub fn start(
        source: &SourceRequest,
        started: &LeStreamStarted,
        stats: Arc<LinkStats>,
    ) -> Result<LeAudioPath, String> {
        let plan = &started.status.plan;
        let rate = plan.preset.sampling_hz;
        let test_tone = matches!(source, SourceRequest::TestTone);
        let options = CaptureOptions { preferred_rate: Some(rate), fill_silence: !test_tone };
        let capture = Capture::open(&LeAudioPath::capture_source(source)?, &options).map_err(|e| e.to_string())?;
        let format = capture.format();
        tracing::info!(
            "LE Audio: capture {} Hz, {} channel(s), {:?} -> LC3 at {rate} Hz",
            format.sample_rate,
            format.channels,
            format.sample_format
        );
        let (pipeline, input) = LePipeline::start(
            LePipelineConfig {
                plan: plan.clone(),
                input: crate::stream::input_format(format),
                buffer: BufferPreset::Standard,
            },
            started.sink.clone(),
            stats,
        )?;
        let running = match capture.start(Box::new(InputSink(input))) {
            Ok(r) => r,
            Err(e) => {
                pipeline.stop();
                return Err(e.to_string());
            }
        };
        Ok(LeAudioPath { capture: Some(running), pipeline: Some(pipeline), format, rate })
    }

    /// The captured format
    pub fn format(&self) -> Format {
        self.format
    }

    /// The LC3 rate
    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// Gaps WASAPI reported in the captured audio
    pub fn discontinuities(&self) -> u64 {
        self.capture.as_ref().map_or(0, Running::discontinuities)
    }

    /// Producer first, then the encode thread
    pub fn stop(mut self) {
        self.stop_now();
    }

    fn stop_now(&mut self) {
        if let Some(c) = self.capture.take() {
            c.stop();
        }
        if let Some(p) = self.pipeline.take() {
            p.stop();
        }
    }
}

impl Drop for LeAudioPath {
    fn drop(&mut self) {
        self.stop_now();
    }
}

/// The stream request of an [`LeRequest`]: an explicit preset is the only
/// one tried. GMAP low latency: the request's flag or (from a profile) the
/// setting, not with an explicit preset.
pub(crate) fn stream_request(le: &LeRequest) -> (LeStreamRequest, bool) {
    let mut request = LeStreamRequest::default();
    if let Some(p) = &le.preset {
        request.preset_order = vec![p.clone()];
    }
    // The profile's choice (or --le-low-latency); an explicit preset wins
    let low_latency = le.low_latency && le.preset.is_none();
    (request, low_latency)
}

/// Why a session refuses an LE request before BTstack is touched
pub(crate) fn refusal(le_enabled: bool, source: &SourceRequest) -> Option<&'static str> {
    if !le_enabled {
        Some("error.le_audio_disabled")
    } else if matches!(source, SourceRequest::Application { .. }) {
        Some("error.le_audio_capture")
    } else {
        None
    }
}

/// The session thread for `req` (`req.le` is Some). Returns when stopped or
/// on an error (state reported), as `stream::run` does.
pub(crate) fn run(inner: &Arc<Inner>, req: StreamRequest) {
    let le = req.le.clone().unwrap_or_default();
    let addr = bt_host::addr_str(&req.device);
    tracing::info!("LE Audio (experimental) stream to {addr} ({})", crate::diag::quoted(&req.device_name));
    tracing::debug!("le stream: request {req:?}");
    let stopped = || inner.stop_requested();
    let ready = || inner.notify(StreamState::Idle, Msg::new("status.ready"));
    let session_start = Instant::now();

    if let Some(key) = refusal(inner.le_wanted(), &req.source) {
        tracing::debug!("le stream: refused ({key}), LE wanted {}, source {}", inner.le_wanted(), req.source);
        return inner.notify(StreamState::Error, Msg::new(key));
    }

    inner.notify(StreamState::Connecting, Msg::new("status.initializing_btstack"));
    let host = match inner.ensure_host() {
        Ok(h) => h,
        Err(m) => return inner.notify(StreamState::Error, m),
    };
    let wait = Instant::now();
    let until = wait + READY_WAIT;
    while !host.le_ready() && Instant::now() < until && !stopped() {
        inner.wait_tick(Duration::from_millis(100));
    }
    tracing::debug!(
        "le stream: LE core ready {} after {} ms (stack up {} ms after the request)",
        host.le_ready(),
        wait.elapsed().as_millis(),
        wait.duration_since(session_start).as_millis()
    );
    if stopped() {
        return ready();
    }
    if !host.le_ready() {
        return inner.notify(StreamState::Error, Msg::new("error.le_audio_unavailable"));
    }

    let (request, low_latency) = stream_request(&le);
    tracing::debug!("le stream: stream request {request:?}, low latency {low_latency}");
    host.le_set_low_latency(low_latency);
    let mut devices = vec![req.device];
    devices.extend(le.extra_devices.iter().copied());
    tracing::debug!("le stream: devices {:?}", devices.iter().map(bt_host::addr_str).collect::<Vec<_>>());

    // Connect (the address type comes from the advertisement) and stream
    let connect_and_stream = || -> Result<LeStreamStarted, Msg> {
        inner.notify(StreamState::Connecting, Msg::new("status.connecting_device"));
        let t = Instant::now();
        let ids = connect_devices(&host, &devices, &req.device_name, &stopped)?;
        tracing::debug!("le stream: connected {ids:?} in {} ms, starting the stream", t.elapsed().as_millis());
        let t = Instant::now();
        let r = host.le_start_stream(ids, request.clone(), &le_silence_set).map_err(|e| {
            tracing::debug!("le stream: start failed after {} ms: {e}", t.elapsed().as_millis());
            host.le_stop_stream();
            Msg::with("error.le_audio_stream", &[&e])
        });
        if let Ok(s) = &r {
            tracing::debug!(
                "le stream: started in {} ms: preset {}, {} CIS, {} device(s)",
                t.elapsed().as_millis(),
                s.status.plan.preset.name,
                s.status.plan.cis.len(),
                s.status.devices.len()
            );
        }
        r
    };
    let mut started = match connect_and_stream() {
        Ok(s) => s,
        Err(m) => {
            host.le_disconnect_all();
            return if stopped() { ready() } else { inner.notify(StreamState::Error, m) };
        }
    };

    // Audio at the LC3 rate
    let switch_default = || match &req.source {
        SourceRequest::VirtualDevice { id, name, auto_switch: true } if !id.is_empty() => {
            DefaultDeviceGuard::switch_to(id, name)
        }
        _ => DefaultDeviceGuard(None),
    };
    let mut default_guard = switch_default();
    // Restored while the headphones are off: switched again when they are back
    let mut switch_default_again = false;
    let mut audio = match LeAudioPath::start(&req.source, &started, host.stats().clone()) {
        Ok(a) => Some(a),
        Err(e) => {
            tracing::error!("LE Audio: audio capture: {e}");
            host.le_stop_stream();
            host.le_disconnect_all();
            drop(default_guard);
            return inner.notify(StreamState::Error, Msg::new("error.wasapi_init"));
        }
    };

    let report = |started: &LeStreamStarted, audio: Option<&LeAudioPath>| {
        inner.notify(StreamState::Streaming, Msg::new("status.connected"));
        let details = LeStreamDetails::of(started);
        let plan = &started.status.plan;
        let channels = plan.channel_count() as u32;
        let rate = plan.preset.sampling_hz;
        let format = audio.map(LeAudioPath::format);
        inner.set_le_stream(Some(details));
        inner.stream_info(StreamInfo {
            codec: CODEC_NAME.into(),
            bitrate_kbps: plan.preset.kbps_per_channel() * channels,
            sample_rate: rate,
            channels,
            bits: 0,
            source_sample_rate: format.map_or(rate, |f| f.sample_rate),
            source_channels: format.map_or(2, |f| f.channels as u32),
            source_bits: format.map_or(16, |f| (f.sample_format.bytes_per_sample() * 8) as u32),
            requested_codec: None,
            requested_in_use: false,
            abr: false,
            bitrate_capped: false,
            source: req.source.info(),
            details: None,
        });
        tracing::info!("LE Audio streaming: {}", started.status.summary);
    };
    report(&started, audio.as_ref());
    inner.save_le_device(&addr, &req.device_name);

    // Windows volume <-> headset volume through the VCP module
    let sync_volume = req.source != SourceRequest::TestTone && req.volume_sync != VolumeSync::Off;
    let mut volume = crate::volume_sync::EndpointChoice::for_stream(&req.volume_sync, &req.source)
        .map(crate::volume_sync::VolumeSyncer::new);

    let mut lost_for_good = false;
    let mut discontinuities = DiscontinuityLog::default();
    while !stopped() {
        inner.wait_tick(Duration::from_millis(200));
        if stopped() {
            break;
        }
        if let Some(v) = volume.as_mut() {
            v.tick(sync_volume, &crate::volume_sync::LeHeadphones(&host));
        }
        discontinuities.tick(audio.as_ref().map_or(0, LeAudioPath::discontinuities));
        if !host.le_stream_lost() {
            continue;
        }

        // A device or CIS went away: tear down and set it up again
        tracing::debug!("le stream: lost after {} s of the session", session_start.elapsed().as_secs());
        if !inner.auto_reconnect() {
            tracing::info!("LE Audio stream lost: reconnecting is off, the session ends");
            // Not connected again by a Bluetooth restart either
            inner.waiting_for_device();
            inner.done_waiting_for_device(false);
            inner.notify(StreamState::Idle, Msg::new("status.device_off"));
            lost_for_good = true;
            break;
        }
        let off = device_turned_off(inner, &host, &devices);
        if !off {
            tracing::warn!("LE Audio stream lost, reconnecting");
            inner.notify(StreamState::Reconnecting, Msg::new("status.connection_lost"));
        }
        if let Some(a) = audio.take() {
            a.stop();
        }
        host.le_stop_stream();
        let mut lost_state = StreamState::Reconnecting;
        if off {
            // As the A2DP stream: nothing to retry until they advertise again
            tracing::info!("the headphones were turned off or disconnected themselves");
            inner.notify(StreamState::Idle, Msg::new("status.device_off"));
            // Windows plays through the device before meanwhile (#132)
            if default_guard.0.is_some() {
                default_guard = DefaultDeviceGuard(None);
                switch_default_again = true;
            }
            if !wait_for_device(inner, &host, req.device) {
                lost_for_good = true;
                break;
            }
            tracing::info!("the headphones are back, resuming");
            lost_state = StreamState::Connecting;
            inner.notify(lost_state, Msg::new("status.device_back"));
        }
        let mut restored = None;
        for attempt in 1..=RECONNECT_ATTEMPTS {
            if stopped() {
                break;
            }
            inner.notify(lost_state, Msg::with("status.reconnect_attempt", &[&attempt, &RECONNECT_ATTEMPTS]));
            tracing::debug!(
                "le stream: reconnect attempt {attempt}/{RECONNECT_ATTEMPTS} in {} ms",
                RECONNECT_WAIT.as_millis()
            );
            let until = Instant::now() + RECONNECT_WAIT;
            while Instant::now() < until && !stopped() {
                inner.wait_tick(Duration::from_millis(100));
            }
            if stopped() {
                break;
            }
            match connect_and_stream() {
                Ok(s) => {
                    restored = Some(s);
                    break;
                }
                Err(m) => tracing::warn!("LE Audio reconnect attempt {attempt} failed: {}", m.english()),
            }
        }
        let Some(s) = restored else {
            tracing::debug!("le stream: not restored (stop requested {})", stopped());
            if !stopped() {
                inner.notify(StreamState::Error, Msg::new("error.reconnect_failed"));
                lost_for_good = true;
            }
            break;
        };
        started = s;
        tracing::info!("LE Audio stream restored: {}", started.status.summary);
        audio = match LeAudioPath::start(&req.source, &started, host.stats().clone()) {
            Ok(a) => Some(a),
            Err(e) => {
                tracing::error!("LE Audio: audio capture: {e}");
                inner.notify(StreamState::Error, Msg::new("error.wasapi_init"));
                lost_for_good = true;
                break;
            }
        };
        if switch_default_again {
            default_guard = switch_default();
            switch_default_again = false;
        }
        report(&started, audio.as_ref());
    }

    tracing::info!("LE Audio streaming stopped, cleaning up");
    tracing::debug!(
        "le stream: ended after {} s (stop requested {}, lost for good {lost_for_good}), {}",
        session_start.elapsed().as_secs(),
        stopped(),
        le_stats_line(&host.le_stream_stats())
    );
    if let Some(a) = audio.take() {
        a.stop();
    }
    host.le_stop_stream();
    host.le_disconnect_all();
    if let Some(mut v) = volume.take() {
        v.stop();
    }
    drop(default_guard);
    inner.set_le_stream(None);
    if !lost_for_good {
        ready();
    } else if !inner.auto_reconnect() {
        // As the A2DP stream: free for the phone the headphones may go to
        inner.shutdown_host();
    }
}

/// A device of the stream ended the link itself (turned off, or its user
/// switched it away), as `stream::device_turned_off`; the disconnection may
/// come a moment after the stream broke
fn device_turned_off(inner: &Inner, host: &BtHost, devices: &[[u8; 6]]) -> bool {
    let until = Instant::now() + Duration::from_millis(1500);
    loop {
        let off = host
            .le_devices()
            .iter()
            .any(|d| devices.contains(&d.addr.addr) && !d.connected && matches!(d.last_error, 0x13 | 0x15));
        if off {
            return true;
        }
        if Instant::now() >= until || inner.stop_requested() {
            return false;
        }
        inner.wait_tick(Duration::from_millis(100));
    }
}

/// The headphones were turned off: waits until they advertise again (true).
/// False when the session ends instead (stopped, sleep, a stopped adapter).
fn wait_for_device(inner: &Inner, host: &BtHost, device: [u8; 6]) -> bool {
    inner.waiting_for_device();
    let t = Instant::now();
    let back = loop {
        if inner.stop_requested()
            || inner.power.suspending()
            || inner.power.stale()
            || host.stalled()
            || !inner.auto_reconnect()
        {
            break false;
        }
        if host.le_find(device, OFF_FIND).is_some() {
            break true;
        }
        inner.wait_tick(Duration::from_millis(200));
    };
    tracing::debug!("le device off: waited {} s, back {back}", t.elapsed().as_secs());
    inner.done_waiting_for_device(back);
    back
}

/// Finds (address type, name, current RPA) and connects each device in
/// order; the first one's name is the profile's when it has one
fn connect_devices(
    host: &BtHost,
    devices: &[[u8; 6]],
    first_name: &str,
    stopped: &dyn Fn() -> bool,
) -> Result<Vec<LeDeviceId>, Msg> {
    let mut ids = Vec::with_capacity(devices.len());
    for (i, &addr) in devices.iter().enumerate() {
        let mut target = LeAddr { addr, addr_type: LeAddr::PUBLIC };
        let mut name = if i == 0 { first_name.to_string() } else { String::new() };
        let t = Instant::now();
        if let Some(found) = host.le_find(addr, FIND_TIMEOUT) {
            tracing::debug!(
                "le connect: {} found in {} ms as {} \"{}\" rssi {}",
                bt_host::addr_str(&addr),
                t.elapsed().as_millis(),
                found.addr,
                found.name,
                found.rssi
            );
            // The address it advertises now (an RPA of a bonded device)
            target = found.addr;
            if name.is_empty() {
                name = found.name;
            }
        } else {
            tracing::debug!(
                "le connect: {} not found in {} ms, trying its public address",
                bt_host::addr_str(&addr),
                t.elapsed().as_millis()
            );
        }
        if stopped() {
            return Err(Msg::new("status.ready"));
        }
        let t = Instant::now();
        match host.le_connect(target, &name, CONNECT_TIMEOUT) {
            Ok(id) => {
                tracing::debug!("le connect: {target} connected as {id:?} in {} ms", t.elapsed().as_millis());
                ids.push(id);
            }
            Err(e) => {
                tracing::debug!("le connect: {target} failed after {} ms: {e}", t.elapsed().as_millis());
                return Err(Msg::with("error.le_audio_connect", &[&e]));
            }
        }
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refused_while_off_or_for_app_capture() {
        let app = SourceRequest::Application { exe: "x.exe".into() };
        assert_eq!(refusal(false, &SourceRequest::SystemLoopback), Some("error.le_audio_disabled"));
        assert_eq!(refusal(false, &app), Some("error.le_audio_disabled"));
        assert_eq!(refusal(true, &app), Some("error.le_audio_capture"));
        assert_eq!(refusal(true, &SourceRequest::SystemLoopback), None);
        assert_eq!(refusal(true, &SourceRequest::TestTone), None);
        let virt = SourceRequest::VirtualDevice { id: "{x}".into(), name: "CABLE".into(), auto_switch: true };
        assert_eq!(refusal(true, &virt), None);
        // The messages exist in English
        for key in ["error.le_audio_disabled", "error.le_audio_capture", "error.le_audio_unavailable"] {
            assert_ne!(Msg::new(key).english(), key);
        }
        assert!(Msg::with("error.le_audio_connect", &[&"timeout"]).english().contains("(timeout)"));
        assert!(Msg::with("error.le_audio_stream", &[&"no ISO buffers"]).english().contains("(no ISO buffers)"));
    }

    #[test]
    fn stats_line_as_v1() {
        let s = bt_host::LeStreamStats {
            sets_encoded: 490,
            sets_sent: 500,
            sdus_sent: 1000,
            silence_sets: 10,
            underruns: 1,
            overflows: 2,
            send_errors: 0,
            prefill: 5,
        };
        assert_eq!(
            le_stats_line(&s),
            "LE Audio: 500 SDU sets sent (1000 SDUs), 490 encoded, 10 silence, 1 underruns, 2 dropped, 0 send errors"
        );
    }

    #[test]
    fn preset_and_low_latency() {
        let (r, ll) = stream_request(&LeRequest::default());
        assert_eq!((r, ll), (LeStreamRequest::default(), false));
        // The profile's choice or the flag
        assert!(stream_request(&LeRequest { low_latency: true, ..LeRequest::default() }).1);
        // An explicit preset wins
        let le = LeRequest { preset: Some("48_2_2".into()), low_latency: true, ..LeRequest::default() };
        let (r, ll) = stream_request(&le);
        assert_eq!(r.preset_order, ["48_2_2"]);
        assert!(!ll);
    }

    #[test]
    fn app_capture_has_no_le_source() {
        assert!(LeAudioPath::capture_source(&SourceRequest::Application { exe: "x.exe".into() }).is_err());
        assert!(
            LeAudioPath::capture_source(&SourceRequest::VirtualDevice {
                id: String::new(),
                name: String::new(),
                auto_switch: false
            })
            .is_err()
        );
        assert!(matches!(LeAudioPath::capture_source(&SourceRequest::TestTone), Ok(Source::TestTone)));
    }
}
