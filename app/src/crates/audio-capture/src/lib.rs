//! Audio sources for BlueCaster: WASAPI loopback of the default output device
//! or a chosen one (virtual device mode), per-application loopback, and the
//! test tone; plus the device list, endpoint volume and app list the UI and
//! core need around them.
//!
//! Port of v1's wasapi_capture, audio_device_enum, app_audio, test_tone and
//! capture_mode. The sources deliver audio in their native format
//! ([`Format`]: F32, I16 or I32, any channel count) on a capture thread
//! running at MMCSS "Audio" priority; the pipeline converts and downmixes.
//! All `unsafe` Win32/COM code lives in `win-sys` or the `wasapi` crate.

#![forbid(unsafe_code)]

mod apps;
mod devices;
mod format;
mod loopback;
mod silence;
mod tone;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread::{self, JoinHandle};

pub use apps::{
    APP_CAPTURE_MIN_BUILD, AudioApp, app_capture_supported, audio_apps, find_process, process_exited, windows_build,
};
pub use devices::{
    DeviceInfo, EndpointVolume, MixFormat, default_render_device, mix_format, mix_rate, render_devices,
    set_default_render_device,
};
pub use format::{DEFAULT_RATE, Format, SampleFormat};
pub use tone::RIGHT_HZ as TEST_TONE_RIGHT_HZ;
pub use tone::{AMPLITUDE as TEST_TONE_AMPLITUDE, LEFT_HZ as TEST_TONE_LEFT_HZ};

/// Where the audio comes from (v1 CaptureMode, plus the test tone)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// Loopback of the default render device (Console role), as it is when
    /// the capture is opened. A later change of the default device is not
    /// followed; reopen to follow it.
    SystemLoopback,
    /// Loopback of this render endpoint (virtual device mode)
    RenderDevice { id: String },
    /// Process loopback of this process and its child processes (Windows
    /// build >= [`APP_CAPTURE_MIN_BUILD`]). Float stereo at the requested
    /// rate (48 kHz by default), converted by Windows.
    Application { process_id: u32 },
    /// 1 kHz left / 1.5 kHz right, amplitude 0.5, f32 stereo, 10 ms blocks,
    /// paced in real time (48 kHz unless a rate is given)
    TestTone,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CaptureOptions {
    /// Capture at this rate instead of the device's mix rate (WASAPI
    /// resamples); for Application capture and the test tone, their rate.
    /// None = the device rate, or 48 kHz where there is none.
    pub preferred_rate: Option<u32>,
    /// Deliver silence ([`Sink::silence`]) while a loopback source plays
    /// nothing, so the stream keeps flowing. Without it a silent device
    /// delivers nothing at all.
    pub fill_silence: bool,
}

/// Receives the audio. Called on the capture thread, which runs at MMCSS
/// "Audio" priority: must be cheap (it copies into a lock-free ring) and
/// must not block.
pub trait Sink: Send + 'static {
    /// Captured audio: `frames` frames of `format`, interleaved
    fn audio(&mut self, data: &[u8], frames: usize, format: Format);
    /// `frames` frames of silence the source synthesised because nothing was
    /// playing (only with fill_silence). Separate so the caller can tell real
    /// audio from fill (AVRCP playback status uses "audio arrives = playing").
    fn silence(&mut self, frames: usize, format: Format);
}

/// Why a running capture ended on its own (see [`Running::ended`])
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndReason {
    /// The device was removed or disabled, or its format changed
    /// (AUDCLNT_E_DEVICE_INVALIDATED)
    DeviceInvalidated,
    /// The captured application exited
    ProcessExited,
    /// The Windows audio service stopped (AUDCLNT_E_SERVICE_NOT_RUNNING)
    ServiceStopped,
    /// Another capture error (HRESULT, 0 if none)
    Error(u32),
    /// The sink panicked
    Panicked,
}

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("no audio output device")]
    NoDevice,
    #[error("audio output device not found: {0}")]
    DeviceNotFound(String),
    #[error("application capture needs Windows 10 2004 (build {APP_CAPTURE_MIN_BUILD}) or later; this is build {0}")]
    UnsupportedBuild(u32),
    #[error("process {0} is not running")]
    ProcessNotFound(u32),
    #[error("activating the audio client failed: HRESULT 0x{hresult:08x} ({message})")]
    Activation { hresult: u32, message: String },
    #[error("unsupported audio format: {0}")]
    UnsupportedFormat(String),
    #[error("{context} failed: HRESULT 0x{hresult:08x} ({message})")]
    Windows { context: &'static str, hresult: u32, message: String },
    #[error("{context} failed: {message}")]
    Wasapi { context: &'static str, message: String },
    #[error("{0} timed out")]
    Timeout(&'static str),
    #[error("cannot start the capture thread: {0}")]
    Thread(String),
    #[error("the capture thread stopped unexpectedly")]
    ThreadGone,
}

impl CaptureError {
    /// The Windows error code behind this error, if there is one
    pub fn hresult(&self) -> Option<u32> {
        match self {
            CaptureError::Activation { hresult, .. } | CaptureError::Windows { hresult, .. } => Some(*hresult),
            _ => None,
        }
    }

    pub(crate) fn windows(context: &'static str, code: i32, message: String) -> CaptureError {
        CaptureError::Windows { context, hresult: code as u32, message }
    }

    pub(crate) fn from_wasapi(context: &'static str, e: wasapi::WasapiError) -> CaptureError {
        match e {
            wasapi::WasapiError::Windows(w) => Self::windows(context, w.code().0, w.message()),
            other => CaptureError::Wasapi { context, message: other.to_string() },
        }
    }

    pub(crate) fn activation(e: &wasapi::WasapiError) -> CaptureError {
        match e {
            wasapi::WasapiError::Windows(w) => {
                CaptureError::Activation { hresult: w.code().0 as u32, message: w.message() }
            }
            other => CaptureError::Activation { hresult: 0, message: other.to_string() },
        }
    }
}

/// State shared between a capture thread and its [`Running`]
#[derive(Debug, Default)]
pub(crate) struct Shared {
    stop: AtomicBool,
    ended: AtomicBool,
    end_reason: OnceLock<EndReason>,
    frames: AtomicU64,
    silence_frames: AtomicU64,
    discontinuities: AtomicU64,
    /// Of `silence_frames`: what the silence fill added because no packet
    /// came (not the packets the engine flagged as silent)
    filled_frames: AtomicU64,
    /// Longest wait between two captured packets since the owner last
    /// took it (µs)
    packet_gap_us: AtomicU64,
}

impl Shared {
    fn end(&self, reason: EndReason) {
        let _ = self.end_reason.set(reason);
        self.ended.store(true, Ordering::Release);
    }
}

/// Marks the capture as ended if its thread unwinds (a panicking sink), so
/// the owner notices instead of waiting for audio that never comes
struct PanicGuard(Arc<Shared>);

impl Drop for PanicGuard {
    fn drop(&mut self) {
        if thread::panicking() {
            self.0.end(EndReason::Panicked);
        }
    }
}

/// MMCSS "Audio" for the calling (capture) thread; without the service the
/// thread keeps normal priority, as in v1
pub(crate) fn register_mmcss() -> Option<win_sys::mmcss::MmcssGuard> {
    match win_sys::mmcss::MmcssGuard::register(win_sys::mmcss::MmcssTask::Audio) {
        Ok(g) => {
            tracing::info!("audio capture: MMCSS Audio (task {})", g.task_index());
            Some(g)
        }
        Err(e) => {
            tracing::warn!("audio capture: MMCSS registration failed: {e}");
            None
        }
    }
}

enum Backend {
    Loopback(loopback::Opened),
    Tone,
}

/// An opened source, not started yet: the format is known, nothing is
/// delivered. Dropping it closes the source.
pub struct Capture {
    format: Format,
    backend: Option<Backend>,
}

impl Capture {
    pub fn open(source: &Source, options: &CaptureOptions) -> Result<Capture, CaptureError> {
        let (format, backend) = match source {
            Source::SystemLoopback => {
                let o = loopback::Opened::open(loopback::Target::Device(None), options)?;
                (o.format, Backend::Loopback(o))
            }
            Source::RenderDevice { id } => {
                if id.is_empty() {
                    return Err(CaptureError::DeviceNotFound(String::new()));
                }
                let o = loopback::Opened::open(loopback::Target::Device(Some(id.clone())), options)?;
                (o.format, Backend::Loopback(o))
            }
            Source::Application { process_id } => {
                let o = loopback::Opened::open(loopback::Target::Process(*process_id), options)?;
                (o.format, Backend::Loopback(o))
            }
            Source::TestTone => {
                let rate = options.preferred_rate.filter(|&r| r >= 100).unwrap_or(DEFAULT_RATE);
                tracing::debug!("audio source: test tone, requested {:?}, using {rate} Hz", options.preferred_rate);
                (tone::format(rate), Backend::Tone)
            }
        };
        Ok(Capture { format, backend: Some(backend) })
    }

    /// The format the source will deliver
    pub fn format(&self) -> Format {
        self.format
    }

    /// Start delivering to `sink` on the capture thread
    pub fn start(mut self, sink: Box<dyn Sink>) -> Result<Running, CaptureError> {
        let format = self.format;
        let (thread, shared) = match self.backend.take().expect("backend present until start") {
            Backend::Loopback(o) => o.start(sink)?,
            Backend::Tone => {
                let shared = Arc::new(Shared::default());
                let thread_shared = shared.clone();
                let rate = format.sample_rate;
                let thread = thread::Builder::new()
                    .name("test-tone".into())
                    .spawn(move || {
                        let _guard = PanicGuard(thread_shared.clone());
                        tone::run(rate, sink, thread_shared)
                    })
                    .map_err(|e| CaptureError::Thread(e.to_string()))?;
                tracing::info!("audio source: test tone at {rate} Hz");
                (thread, shared)
            }
        };
        Ok(Running { format, shared, thread: Some(thread) })
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        if let Some(Backend::Loopback(o)) = self.backend.take() {
            o.close();
        }
    }
}

impl std::fmt::Debug for Capture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Capture").field("format", &self.format).finish()
    }
}

/// A started source: the capture thread is delivering to the sink.
/// Dropping it (or [`Running::stop`]) stops the thread and joins it.
#[derive(Debug)]
pub struct Running {
    format: Format,
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Running {
    /// Stop and join the capture thread
    pub fn stop(self) {}

    /// The source went away (app exited, device removed/invalidated); the
    /// thread has finished and nothing more will be delivered
    pub fn ended(&self) -> bool {
        self.shared.ended.load(Ordering::Acquire)
    }

    /// Why it ended, once [`Running::ended`] is true
    pub fn end_reason(&self) -> Option<EndReason> {
        self.shared.end_reason.get().copied()
    }

    pub fn format(&self) -> Format {
        self.format
    }

    /// Captured frames delivered through [`Sink::audio`] so far
    pub fn frames(&self) -> u64 {
        self.shared.frames.load(Ordering::Relaxed)
    }

    /// Fill frames delivered through [`Sink::silence`] so far
    pub fn silence_frames(&self) -> u64 {
        self.shared.silence_frames.load(Ordering::Relaxed)
    }

    /// Gaps WASAPI reported in the captured audio (data discontinuities,
    /// not counting the expected ones right after the start). v1 logged
    /// these from the capture thread; here the owner reads and logs them.
    pub fn discontinuities(&self) -> u64 {
        self.shared.discontinuities.load(Ordering::Relaxed)
    }

    /// Of [`Running::silence_frames`]: the silence the fill added because
    /// no packet came for two periods (a loopback capture whose device
    /// plays nothing, or a capture thread held back)
    pub fn filled_frames(&self) -> u64 {
        self.shared.filled_frames.load(Ordering::Relaxed)
    }

    /// The longest wait between two captured packets since the last call
    /// (zero for the test tone, which has no packets)
    pub fn take_packet_gap(&self) -> std::time::Duration {
        std::time::Duration::from_micros(self.shared.packet_gap_us.swap(0, Ordering::Relaxed))
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let started = std::time::Instant::now();
            // The thread notices within one period (≤ 100 ms)
            let _ = t.join();
            tracing::debug!(
                "audio capture: thread joined in {} ms, ended {:?}",
                started.elapsed().as_millis(),
                self.shared.end_reason.get()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::Duration;

    #[derive(Default)]
    struct Counts {
        frames: usize,
        silence: usize,
        calls: usize,
        first: Vec<u8>,
    }

    struct TestSink(Arc<Mutex<Counts>>);

    impl Sink for TestSink {
        fn audio(&mut self, data: &[u8], frames: usize, format: Format) {
            assert_eq!(data.len(), frames * format.bytes_per_frame());
            let mut c = self.0.lock().unwrap();
            if c.calls == 0 {
                c.first = data.to_vec();
            }
            c.frames += frames;
            c.calls += 1;
        }
        fn silence(&mut self, frames: usize, _format: Format) {
            self.0.lock().unwrap().silence += frames;
        }
    }

    #[test]
    fn test_tone_runs_in_real_time() {
        let opts = CaptureOptions { preferred_rate: Some(44_100), fill_silence: false };
        let cap = Capture::open(&Source::TestTone, &opts).unwrap();
        assert_eq!(cap.format(), Format { sample_rate: 44_100, channels: 2, sample_format: SampleFormat::F32 });
        let counts = Arc::new(Mutex::new(Counts::default()));
        let running = cap.start(Box::new(TestSink(counts.clone()))).unwrap();
        thread::sleep(Duration::from_millis(500));
        assert!(!running.ended());
        let frames = running.frames();
        running.stop();
        let c = counts.lock().unwrap();
        // One more block may arrive between reading the counter and stopping
        assert!(c.frames as u64 == frames || c.frames as u64 == frames + 441, "{} vs {frames}", c.frames);
        // 10 ms blocks of 441 frames, about 50 of them in 500 ms
        assert_eq!(c.frames % 441, 0);
        assert!((40..=60).contains(&c.calls), "{} blocks", c.calls);
        assert_eq!(c.silence, 0);
        // The first sample pair is sin(0) = 0 on both channels
        assert_eq!(&c.first[..8], &[0u8; 8]);
    }

    #[test]
    fn handles_move_between_threads() {
        // core opens a source on one thread and owns it from another
        fn send<T: Send>() {}
        send::<Capture>();
        send::<Running>();
        send::<CaptureError>();
    }

    #[test]
    fn test_tone_default_rate() {
        let cap = Capture::open(&Source::TestTone, &CaptureOptions::default()).unwrap();
        assert_eq!(cap.format().sample_rate, 48_000);
    }

    #[test]
    fn empty_device_id_is_rejected() {
        let err = Capture::open(&Source::RenderDevice { id: String::new() }, &CaptureOptions::default()).unwrap_err();
        assert!(matches!(err, CaptureError::DeviceNotFound(_)));
    }

    #[test]
    fn unknown_device_id_is_not_found() {
        let err = Capture::open(
            &Source::RenderDevice { id: "{0.0.0.00000000}.{00000000-0000-0000-0000-000000000000}".into() },
            &CaptureOptions::default(),
        )
        .unwrap_err();
        assert!(matches!(err, CaptureError::DeviceNotFound(_)), "{err}");
    }

    #[test]
    fn missing_process_is_reported() {
        // PIDs are multiples of 4; this one is never in use
        let err = Capture::open(&Source::Application { process_id: 3 }, &CaptureOptions::default()).unwrap_err();
        assert!(matches!(err, CaptureError::ProcessNotFound(3) | CaptureError::UnsupportedBuild(_)), "{err}");
    }

    #[test]
    fn application_capture_ends_when_the_app_exits() {
        if !app_capture_supported() {
            return;
        }
        // A process without audio: the capture delivers only silence fill,
        // and ends once the process is gone
        let mut child = std::process::Command::new("cmd").args(["/C", "ping -n 2 127.0.0.1 >NUL"]).spawn().unwrap();
        let opts = CaptureOptions { preferred_rate: Some(44_100), fill_silence: true };
        let cap = Capture::open(&Source::Application { process_id: child.id() }, &opts).unwrap();
        assert_eq!(cap.format(), Format { sample_rate: 44_100, channels: 2, sample_format: SampleFormat::F32 });
        let counts = Arc::new(Mutex::new(Counts::default()));
        let running = cap.start(Box::new(TestSink(counts.clone()))).unwrap();
        child.wait().unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !running.ended() && std::time::Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(running.ended());
        assert_eq!(running.end_reason(), Some(EndReason::ProcessExited));
        // Something flowed all along. (Process loopback delivers zero-filled
        // packets, not flagged silent, even for an app that plays nothing,
        // so they count as audio; the fill covers any gap.)
        let c = counts.lock().unwrap();
        assert!(c.frames + c.silence > 0);
        assert_eq!(c.silence as u64, running.silence_frames());
        assert_eq!(c.frames as u64, running.frames());
    }

    #[test]
    fn error_codes() {
        let e = CaptureError::windows("x", 0x8889_0004u32 as i32, "gone".into());
        assert_eq!(e.hresult(), Some(0x8889_0004));
        assert!(e.to_string().contains("0x88890004"));
        assert_eq!(CaptureError::NoDevice.hresult(), None);
    }
}
