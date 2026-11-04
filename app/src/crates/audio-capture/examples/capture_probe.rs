//! Real-hardware smoke test of audio-capture: lists the render devices and
//! audio apps, then runs each source for 2 seconds and prints its format
//! and what it delivered. Read-only: it never changes a device setting.
//!
//!     cargo run -p audio-capture --example capture_probe
//!
//! Play something during the system loopback part to see captured frames;
//! with nothing playing, the frames come as silence fill instead.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use audio_capture::{Capture, CaptureError, CaptureOptions, EndpointVolume, Format, Sink, Source};

#[derive(Default)]
struct Counters {
    audio_frames: AtomicU64,
    audio_calls: AtomicU64,
    silence_frames: AtomicU64,
    silence_calls: AtomicU64,
    /// Largest absolute f32 sample seen (bits), to tell sound from zeros
    peak_bits: AtomicU64,
}

struct ProbeSink(Arc<Counters>);

impl Sink for ProbeSink {
    fn audio(&mut self, data: &[u8], frames: usize, format: Format) {
        self.0.audio_frames.fetch_add(frames as u64, Ordering::Relaxed);
        self.0.audio_calls.fetch_add(1, Ordering::Relaxed);
        if format.sample_format == audio_capture::SampleFormat::F32 {
            let peak = data.as_chunks::<4>().0.iter().map(|&c| f32::from_le_bytes(c).abs()).fold(0f32, f32::max);
            self.0.peak_bits.fetch_max(peak.to_bits() as u64, Ordering::Relaxed);
        }
    }

    fn silence(&mut self, frames: usize, _format: Format) {
        self.0.silence_frames.fetch_add(frames as u64, Ordering::Relaxed);
        self.0.silence_calls.fetch_add(1, Ordering::Relaxed);
    }
}

fn run(label: &str, source: &Source, options: &CaptureOptions) {
    println!("\n== {label}: {source:?}, {options:?}");
    let t0 = Instant::now();
    let capture = match Capture::open(source, options) {
        Ok(c) => c,
        Err(e) => {
            println!("   open failed: {e}");
            return;
        }
    };
    let f = capture.format();
    println!(
        "   opened in {:?}: {} Hz, {} ch, {:?} ({} bytes/frame)",
        t0.elapsed(),
        f.sample_rate,
        f.channels,
        f.sample_format,
        f.bytes_per_frame()
    );
    let counters = Arc::new(Counters::default());
    let running = match capture.start(Box::new(ProbeSink(counters.clone()))) {
        Ok(r) => r,
        Err(e) => {
            println!("   start failed: {e}");
            return;
        }
    };
    let start = Instant::now();
    thread::sleep(Duration::from_secs(2));
    let elapsed = start.elapsed();
    let ended = running.end_reason();
    let discontinuities = running.discontinuities();
    let lost = running.lost_frames();
    let t_stop = Instant::now();
    running.stop();
    let stop_time = t_stop.elapsed();

    let audio = counters.audio_frames.load(Ordering::Relaxed);
    let silence = counters.silence_frames.load(Ordering::Relaxed);
    let expected = f.frames_in(elapsed) as u64;
    println!(
        "   {:.2} s: audio {} frames in {} calls, silence {} frames in {} calls, total {:.1}% of real time",
        elapsed.as_secs_f64(),
        audio,
        counters.audio_calls.load(Ordering::Relaxed),
        silence,
        counters.silence_calls.load(Ordering::Relaxed),
        (audio + silence) as f64 * 100.0 / expected.max(1) as f64
    );
    println!(
        "   peak {:.3}, discontinuities {discontinuities} ({lost} frames lost), ended {ended:?}, stop took {stop_time:?}",
        f32::from_bits(counters.peak_bits.load(Ordering::Relaxed) as u32),
    );
}

fn main() -> Result<(), CaptureError> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    // Compile-checked only: switching the default device changes the
    // user's system settings, so the probe never calls it
    let _never_called: fn(&str) -> Result<(), CaptureError> = audio_capture::set_default_render_device;

    println!(
        "Windows build {} (application capture {})",
        audio_capture::windows_build(),
        if audio_capture::app_capture_supported() { "supported" } else { "not supported" }
    );

    println!("\nRender devices:");
    for d in audio_capture::render_devices()? {
        // Opening (without starting) a loopback stream is passive: it shows
        // the format each device would deliver
        let opened =
            Capture::open(&Source::RenderDevice { id: d.id.clone() }, &CaptureOptions::default()).map(|c| c.format());
        println!(
            "  {}{} {}  [{}]
      mix rate {:?}, loopback format {:?}",
            if d.is_default { "*" } else { " " },
            if d.is_virtual { "v" } else { " " },
            d.name,
            d.id,
            audio_capture::mix_rate(Some(&d.id)),
            opened
        );
    }
    println!("default: {:?}, mix rate {:?}", audio_capture::default_render_device(), audio_capture::mix_rate(None));
    match EndpointVolume::open(None) {
        Ok(v) => println!("default endpoint volume {:.3}, muted {}", v.volume()?, v.muted()?),
        Err(e) => println!("endpoint volume: {e}"),
    }

    let apps = audio_capture::audio_apps();
    println!("\nApps with an audio session:");
    for a in &apps {
        println!("  {:>6}  {}  (find_process -> {:?})", a.process_id, a.name, audio_capture::find_process(&a.exe));
    }

    let fill = CaptureOptions { preferred_rate: None, fill_silence: true };
    run("system loopback", &Source::SystemLoopback, &fill);
    run(
        "system loopback at 44.1 kHz, no fill",
        &Source::SystemLoopback,
        &CaptureOptions { preferred_rate: Some(44_100), fill_silence: false },
    );
    if let Some(id) = audio_capture::default_render_device() {
        run("render device", &Source::RenderDevice { id }, &fill);
    }
    run("test tone", &Source::TestTone, &CaptureOptions::default());
    if let Some(app) = apps.first()
        && let Some(pid) = audio_capture::find_process(&app.exe)
    {
        run(&format!("application {}", app.exe), &Source::Application { process_id: pid }, &fill);
    }
    Ok(())
}
