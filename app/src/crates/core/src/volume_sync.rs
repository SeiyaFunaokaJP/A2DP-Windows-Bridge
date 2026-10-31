//! Volume sync: the headphones' volume (AVRCP absolute volume, 0..127) and a
//! Windows output device's master volume act as one volume — v1's
//! volume_sync.cpp.
//!
//! While streaming, the volume keys and the Windows volume flyout change the
//! headphones' volume, and the headphones' own buttons and gestures move the
//! Windows slider. Loopback capture is taken before the master volume, so the
//! audio is attenuated once, on the headphones, and keeps its full resolution.
//!
//! Only the level is synced, not mute: muting the output device is how the PC
//! is kept silent while the headphones play (loopback capture is not affected
//! by the endpoint's mute), so mute is never read nor written here
//! (mute_output.rs mutes it for the session).
//!
//! The Windows side is [`EndpointVolume`](audio_capture::EndpointVolume),
//! which is `!Send` and bound to one device: a [`VolumeSyncer`] lives on the
//! session thread and reopens the endpoint when the default output device
//! changes (device choice [`EndpointChoice::Default`]). Both sides are traits
//! ([`Headphones`], [`EndpointProvider`]) so the logic is tested with fakes.
//!
//! LE Audio (experimental): the headphone side is the LE core's volume (the
//! VCP module, [`LeHeadphones`]); `--le-sim-volume` runs the same sync
//! against a simulated Windows volume ([`SimulatedWindowsVolume`], v1
//! le_audio::vcp::SimulatedWindowsVolume) for the emulator tests.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use le_audio::vcp_scale::SimVolumeSpec;

use crate::stream::{SourceRequest, VolumeSync};

/// After a Windows change is sent to the headphones, their volume may still
/// report older values for a moment (queued command, confirmation in
/// flight). Wait before reading it as a change made on the headphones.
pub const HOLD: Duration = Duration::from_secs(1);

/// The headphone side: where the headphone volume lives (AVRCP absolute
/// volume, 0..127)
pub trait Headphones {
    /// None while unknown (AVRCP not connected yet, or the link dropped)
    fn remote_volume(&self) -> Option<u8>;
    /// Asks the headphones to take this volume (0..127)
    fn request_volume(&self, volume: u8);
}

impl Headphones for bt_host::BtHost {
    fn remote_volume(&self) -> Option<u8> {
        bt_host::BtHost::remote_volume(self)
    }

    fn request_volume(&self, volume: u8) {
        bt_host::BtHost::request_volume(self, volume)
    }
}

/// LE Audio (experimental): the headset volume through the LE core's
/// modules (VCP), on the 0..127 scale of AVRCP
pub struct LeHeadphones<'a>(pub &'a bt_host::BtHost);

impl Headphones for LeHeadphones<'_> {
    fn remote_volume(&self) -> Option<u8> {
        self.0.le_remote_volume()
    }

    fn request_volume(&self, volume: u8) {
        self.0.le_request_volume(volume);
    }
}

/// The Windows side: the master volume of one output device
pub trait Endpoint {
    /// The endpoint ID this volume belongs to
    fn device_id(&self) -> &str;
    /// Master volume scalar 0.0..=1.0 (the Windows slider); None if it
    /// cannot be read (device removed, ...)
    fn volume(&self) -> Option<f32>;
    /// Sets the master volume scalar; false if it failed
    fn set_volume(&self, scalar: f32) -> bool;
    /// The volume it had is given back when syncing stops (not for the
    /// simulated volume, as in v1)
    fn restores(&self) -> bool {
        true
    }
    /// Whether the device is muted; None if it cannot be read
    fn muted(&self) -> Option<bool> {
        None
    }
    /// Sets the device's mute; false if it failed
    fn set_muted(&self, _muted: bool) -> bool {
        false
    }
}

/// Opens [`Endpoint`]s and tells which device is the default output
pub trait EndpointProvider {
    type Endpoint: Endpoint;
    /// ID of the default output device (Console role), if there is one
    fn default_device(&self) -> Option<String>;
    /// Opens the volume of this endpoint
    fn open(&self, device_id: &str) -> Result<Self::Endpoint, String>;
}

/// The real endpoints, through `audio-capture` (COM on the calling thread)
#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsEndpoints;

impl Endpoint for audio_capture::EndpointVolume {
    fn device_id(&self) -> &str {
        audio_capture::EndpointVolume::device_id(self)
    }

    fn volume(&self) -> Option<f32> {
        audio_capture::EndpointVolume::volume(self).ok()
    }

    fn set_volume(&self, scalar: f32) -> bool {
        audio_capture::EndpointVolume::set_volume(self, scalar).is_ok()
    }

    fn muted(&self) -> Option<bool> {
        audio_capture::EndpointVolume::muted(self).ok()
    }

    fn set_muted(&self, muted: bool) -> bool {
        audio_capture::EndpointVolume::set_muted(self, muted).is_ok()
    }
}

impl EndpointProvider for WindowsEndpoints {
    type Endpoint = audio_capture::EndpointVolume;

    fn default_device(&self) -> Option<String> {
        audio_capture::default_render_device()
    }

    fn open(&self, device_id: &str) -> Result<audio_capture::EndpointVolume, String> {
        audio_capture::EndpointVolume::open(Some(device_id)).map_err(|e| e.to_string())
    }
}

/// Which output device's volume is synced
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndpointChoice {
    /// The Windows default output device, followed when it changes
    Default,
    /// This endpoint
    Device(String),
}

impl EndpointChoice {
    /// The device of a stream's volume sync setting, None = no sync. Auto is
    /// the device the audio comes from: the virtual device in virtual device
    /// mode, else the default output device (in application mode that is
    /// where the volume keys act). The test tone has no Windows volume.
    pub fn for_stream(sync: &VolumeSync, source: &SourceRequest) -> Option<EndpointChoice> {
        match (sync, source) {
            (VolumeSync::Off, _) | (_, SourceRequest::TestTone) => None,
            (VolumeSync::Device(id), _) if !id.is_empty() => Some(EndpointChoice::Device(id.clone())),
            (VolumeSync::Auto, SourceRequest::VirtualDevice { id, .. }) if !id.is_empty() => {
                Some(EndpointChoice::Device(id.clone()))
            }
            // A "device" profile without a device: the default one, as v1
            _ => Some(EndpointChoice::Default),
        }
    }
}

/// Windows scalar -> AVRCP level (v1: rounded, linear)
pub fn scalar_to_level(scalar: f32) -> u8 {
    (scalar * 127.0 + 0.5).clamp(0.0, 127.0) as u8
}

/// AVRCP level -> Windows scalar
pub fn level_to_scalar(level: u8) -> f32 {
    level.min(127) as f32 / 127.0
}

/// Level as the percentage the Windows slider shows (logs)
pub fn percent(level: u8) -> u32 {
    (level as u32 * 100 + 63) / 127
}

struct Attached<E> {
    endpoint: E,
    /// The device's volume before syncing changed it, given back on stop
    original: Option<f32>,
}

/// Keeps the two volumes equal; call [`tick`](VolumeSyncer::tick) a few
/// times per second while streaming (the session loop's 200 ms tick).
///
/// When the headphones' volume first becomes known, Windows takes it over;
/// after that, whichever side changed is copied to the other. Stopping (or
/// dropping, or `tick(false, ..)`) gives the device its previous volume back.
pub struct VolumeSyncer<P: EndpointProvider = WindowsEndpoints> {
    provider: P,
    choice: EndpointChoice,
    attached: Option<Attached<P::Endpoint>>,
    /// Levels last seen in sync (0..127); None = not in sync yet
    last_windows: Option<u8>,
    last_headphones: Option<u8>,
    /// Headphones settling after a Windows change
    hold_until: Option<Instant>,
    /// Device that could not be opened (logged once, not every tick)
    failed: Option<String>,
}

impl VolumeSyncer<WindowsEndpoints> {
    /// Syncs with a real output device. COM is initialised by the endpoint
    /// on this thread; keep the syncer on the thread that ticks it.
    pub fn new(choice: EndpointChoice) -> Self {
        VolumeSyncer::with_provider(WindowsEndpoints, choice)
    }
}

impl<P: EndpointProvider> VolumeSyncer<P> {
    pub fn with_provider(provider: P, choice: EndpointChoice) -> Self {
        tracing::debug!("volume sync: on, device {choice:?}");
        VolumeSyncer {
            provider,
            choice,
            attached: None,
            last_windows: None,
            last_headphones: None,
            hold_until: None,
            failed: None,
        }
    }

    /// The device being synced, once attached
    pub fn device_id(&self) -> Option<&str> {
        self.attached.as_ref().map(|a| a.endpoint.device_id())
    }

    /// One step. `enabled` false stops syncing (the device gets its volume
    /// back); `headphones` is passed per tick because the session replaces
    /// its stack (and with it the headphone side) on a restart.
    pub fn tick(&mut self, enabled: bool, headphones: &dyn Headphones) {
        self.tick_at(enabled, headphones, Instant::now());
    }

    fn tick_at(&mut self, enabled: bool, headphones: &dyn Headphones, now: Instant) {
        if !enabled {
            self.stop();
            return;
        }
        let Some(hp) = headphones.remote_volume() else {
            if self.last_headphones.is_some() {
                tracing::debug!("volume sync: headphone volume unknown now (no absolute volume), waiting for it");
            }
            // No AVRCP volume (yet, or the link dropped): when it comes
            // back, Windows takes over the headphones' volume again
            self.last_windows = None;
            self.last_headphones = None;
            return;
        };
        let hp = hp.min(127);
        if !self.attach() {
            return;
        }
        let Some(windows) = self.read_level() else { return };

        let (Some(last_windows), Some(last_headphones)) = (self.last_windows, self.last_headphones) else {
            // Start in sync with what the headphones play at
            tracing::info!("volume sync: Windows volume {}% -> {}% (headphones)", percent(windows), percent(hp));
            self.set_windows(hp);
            return;
        };
        if windows != last_windows {
            tracing::debug!(
                "volume sync: Windows -> headphones {windows} ({}%), was {last_windows}, headphones at {hp}",
                percent(windows)
            );
            // Volume keys, the flyout or the mixer: the headphones follow
            headphones.request_volume(windows);
            self.last_windows = Some(windows);
            self.last_headphones = Some(windows);
            self.hold_until = Some(now + HOLD);
            return;
        }
        if self.hold_until.is_some_and(|until| now < until) {
            if hp != last_headphones {
                tracing::trace!("volume sync: headphones report {hp} while settling, ignored (echo)");
            }
            return;
        }
        if hp != last_headphones {
            tracing::debug!(
                "volume sync: headphones -> Windows {hp} ({}%), was {last_headphones}, Windows at {windows}",
                percent(hp)
            );
            // The headphones' buttons or gestures, or the app's slider
            self.set_windows(hp);
        }
    }

    /// Stops syncing: the device gets the volume it had before
    pub fn stop(&mut self) {
        self.detach();
    }

    /// Opens the chosen endpoint; with the default device, reopens when the
    /// default changed (the old device gets its volume back)
    fn attach(&mut self) -> bool {
        let id = match &self.choice {
            EndpointChoice::Device(id) => id.clone(),
            EndpointChoice::Default => match self.provider.default_device() {
                Some(id) => id,
                None => {
                    tracing::trace!("volume sync: no default output device");
                    return false;
                }
            },
        };
        if self.attached.as_ref().is_some_and(|a| a.endpoint.device_id() == id) {
            return true;
        }
        tracing::debug!("volume sync: opening {id} (was {:?})", self.attached.as_ref().map(|a| a.endpoint.device_id()));
        self.detach();
        match self.provider.open(&id) {
            Ok(endpoint) => {
                tracing::info!(
                    "volume sync: syncing the headphone volume with the {}",
                    if self.choice == EndpointChoice::Default {
                        "default output device"
                    } else {
                        "selected output device"
                    }
                );
                self.attached = Some(Attached { endpoint, original: None });
                self.failed = None;
                true
            }
            Err(e) => {
                if self.failed.as_deref() != Some(id.as_str()) {
                    tracing::warn!("volume sync: cannot open the output device's volume: {e}");
                    self.failed = Some(id);
                }
                false
            }
        }
    }

    fn detach(&mut self) {
        if let Some(a) = self.attached.take()
            && let Some(original) = a.original
            && a.endpoint.restores()
        {
            // Outside streaming the device plays on its own again: give it
            // its previous volume back
            if a.endpoint.set_volume(original) {
                tracing::info!("volume sync: Windows volume restored to {}%", percent(scalar_to_level(original)));
            } else {
                tracing::debug!("volume sync: restoring the Windows volume failed");
            }
        }
        self.last_windows = None;
        self.last_headphones = None;
        self.hold_until = None;
    }

    fn read_level(&self) -> Option<u8> {
        self.attached.as_ref()?.endpoint.volume().map(scalar_to_level)
    }

    fn set_windows(&mut self, level: u8) {
        let Some(a) = self.attached.as_mut() else { return };
        if a.original.is_none() {
            a.original = a.endpoint.volume();
            tracing::debug!("volume sync: Windows volume before syncing {:?}", a.original);
        }
        let ok = a.endpoint.set_volume(level_to_scalar(level));
        // What the device actually took (it may have volume steps)
        let taken = a.endpoint.volume().map(scalar_to_level);
        tracing::debug!("volume sync: Windows set to {level}: ok {ok}, took {taken:?}");
        self.last_windows = Some(taken.unwrap_or(level));
        self.last_headphones = Some(level);
    }
}

impl<P: EndpointProvider> Drop for VolumeSyncer<P> {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The endpoint ID of the simulated Windows volume
pub const SIMULATED_DEVICE: &str = "simulated";

/// Test only: one simulated output device, the default one, whose volume
/// (0..127) lives in memory; the real Windows volume is never touched. Clones
/// share the volume.
#[derive(Debug, Clone, Default)]
pub struct SimulatedEndpoints {
    level: Rc<Cell<u8>>,
}

impl SimulatedEndpoints {
    pub fn new(level: u8) -> SimulatedEndpoints {
        SimulatedEndpoints { level: Rc::new(Cell::new(level.min(127))) }
    }

    /// The simulated Windows volume (0..127)
    pub fn level(&self) -> u8 {
        self.level.get()
    }

    /// A change made "in Windows" (the volume keys)
    pub fn set_level(&self, level: u8) {
        self.level.set(level.min(127));
    }
}

/// The simulated device's volume ([`SimulatedEndpoints`])
#[derive(Debug)]
pub struct SimulatedEndpoint {
    level: Rc<Cell<u8>>,
}

impl Endpoint for SimulatedEndpoint {
    fn device_id(&self) -> &str {
        SIMULATED_DEVICE
    }

    fn volume(&self) -> Option<f32> {
        Some(level_to_scalar(self.level.get()))
    }

    fn set_volume(&self, scalar: f32) -> bool {
        let level = scalar_to_level(scalar);
        self.level.set(level);
        // The emulator tests read these lines (tools/emu/le_audio_tests/vcp.py)
        tracing::info!("VCP test: simulated Windows volume set to {level}/127 (by VolumeSync)");
        true
    }

    fn restores(&self) -> bool {
        false
    }
}

impl EndpointProvider for SimulatedEndpoints {
    type Endpoint = SimulatedEndpoint;

    fn default_device(&self) -> Option<String> {
        Some(SIMULATED_DEVICE.into())
    }

    fn open(&self, device_id: &str) -> Result<SimulatedEndpoint, String> {
        if device_id == SIMULATED_DEVICE {
            Ok(SimulatedEndpoint { level: self.level.clone() })
        } else {
            Err(format!("no simulated device {device_id}"))
        }
    }
}

/// Test only (`--cli --le-audio --le-sim-volume <spec>`, v1
/// le_audio::vcp::SimulatedWindowsVolume): VolumeSync with the headset
/// against a simulated Windows volume that starts at `spec.start` and
/// changes to each step's level at its time after the first tick (the
/// volume keys), so the emulator tests exercise the real sync logic.
pub struct SimulatedWindowsVolume {
    spec: SimVolumeSpec,
    endpoints: SimulatedEndpoints,
    syncer: Option<VolumeSyncer<SimulatedEndpoints>>,
    started: Option<Instant>,
    next: usize,
}

impl SimulatedWindowsVolume {
    pub fn new(spec: SimVolumeSpec) -> SimulatedWindowsVolume {
        let endpoints = SimulatedEndpoints::new(spec.start);
        SimulatedWindowsVolume { spec, endpoints, syncer: None, started: None, next: 0 }
    }

    /// From `--le-sim-volume "<level>[,<seconds>:<level>...]"`; None if
    /// malformed
    pub fn parse(spec: &str) -> Option<SimulatedWindowsVolume> {
        SimVolumeSpec::parse(spec).map(SimulatedWindowsVolume::new)
    }

    /// The simulated Windows volume now (0..127)
    pub fn level(&self) -> u8 {
        self.endpoints.level()
    }

    /// Call every 100-200 ms while streaming; the first call starts it
    pub fn tick(&mut self, headphones: &dyn Headphones) {
        self.tick_at(headphones, Instant::now());
    }

    fn tick_at(&mut self, headphones: &dyn Headphones, now: Instant) {
        let level = self.endpoints.level();
        let started = *self.started.get_or_insert_with(|| {
            tracing::info!("VCP test: simulated Windows volume {level}/127 (the real Windows volume is not used)");
            now
        });
        let elapsed = now.saturating_duration_since(started).as_millis();
        while let Some(&(at_ms, level)) = self.spec.steps.get(self.next) {
            if elapsed < u128::from(at_ms) {
                break;
            }
            self.endpoints.set_level(level);
            tracing::info!("VCP test: simulated Windows volume changed to {}/127 (volume keys)", level.min(127));
            self.next += 1;
        }
        let endpoints = &self.endpoints;
        let syncer =
            self.syncer.get_or_insert_with(|| VolumeSyncer::with_provider(endpoints.clone(), EndpointChoice::Default));
        syncer.tick_at(true, headphones, now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::rc::Rc;

    /// Output devices: volume scalar and mute per ID, and the default
    #[derive(Default)]
    struct Devices {
        volume: RefCell<HashMap<String, f32>>,
        muted: RefCell<HashMap<String, bool>>,
        default: RefCell<Option<String>>,
        /// Volume steps the device snaps to (0 = continuous)
        steps: Cell<u32>,
        opens: Cell<u32>,
    }

    impl Devices {
        fn with(ids: &[(&str, f32)]) -> Rc<Devices> {
            let d = Rc::new(Devices::default());
            for (id, v) in ids {
                d.volume.borrow_mut().insert(id.to_string(), *v);
                d.muted.borrow_mut().insert(id.to_string(), false);
            }
            *d.default.borrow_mut() = ids.first().map(|(id, _)| id.to_string());
            d
        }
        fn level(&self, id: &str) -> u8 {
            scalar_to_level(self.volume.borrow()[id])
        }
        fn set_level(&self, id: &str, level: u8) {
            self.volume.borrow_mut().insert(id.into(), level_to_scalar(level));
        }
    }

    struct FakeEndpoint {
        id: String,
        devices: Rc<Devices>,
    }

    impl Endpoint for FakeEndpoint {
        fn device_id(&self) -> &str {
            &self.id
        }
        fn volume(&self) -> Option<f32> {
            self.devices.volume.borrow().get(&self.id).copied()
        }
        fn set_volume(&self, scalar: f32) -> bool {
            let steps = self.devices.steps.get();
            let v = if steps > 0 { (scalar * steps as f32).round() / steps as f32 } else { scalar };
            self.devices.volume.borrow_mut().insert(self.id.clone(), v.clamp(0.0, 1.0));
            true
        }
    }

    struct FakeProvider(Rc<Devices>);

    impl EndpointProvider for FakeProvider {
        type Endpoint = FakeEndpoint;
        fn default_device(&self) -> Option<String> {
            self.0.default.borrow().clone()
        }
        fn open(&self, id: &str) -> Result<FakeEndpoint, String> {
            if !self.0.volume.borrow().contains_key(id) {
                return Err(format!("no device {id}"));
            }
            self.0.opens.set(self.0.opens.get() + 1);
            Ok(FakeEndpoint { id: id.into(), devices: self.0.clone() })
        }
    }

    /// Headphones that take a requested volume when told to (AVRCP
    /// confirmation), so tests can model the delay
    #[derive(Default)]
    struct FakeHeadphones {
        volume: Cell<Option<u8>>,
        requests: RefCell<Vec<u8>>,
    }

    impl FakeHeadphones {
        fn at(v: u8) -> FakeHeadphones {
            FakeHeadphones { volume: Cell::new(Some(v)), requests: RefCell::default() }
        }
        fn confirm(&self) {
            if let Some(&v) = self.requests.borrow().last() {
                self.volume.set(Some(v));
            }
        }
    }

    impl Headphones for FakeHeadphones {
        fn remote_volume(&self) -> Option<u8> {
            self.volume.get()
        }
        fn request_volume(&self, v: u8) {
            self.requests.borrow_mut().push(v);
        }
    }

    fn syncer(devices: &Rc<Devices>, choice: EndpointChoice) -> VolumeSyncer<FakeProvider> {
        VolumeSyncer::with_provider(FakeProvider(devices.clone()), choice)
    }

    #[test]
    fn mapping_round_trips() {
        for level in 0..=127u8 {
            assert_eq!(scalar_to_level(level_to_scalar(level)), level);
        }
        assert_eq!(scalar_to_level(-0.5), 0);
        assert_eq!(scalar_to_level(1.5), 127);
        assert_eq!(scalar_to_level(0.5), 64);
        assert_eq!(percent(127), 100);
        assert_eq!(percent(64), 50);
        assert_eq!(percent(0), 0);
    }

    #[test]
    fn windows_takes_the_headphones_volume_first() {
        let d = Devices::with(&[("spk", 0.8)]);
        let hp = FakeHeadphones::at(40);
        let mut s = syncer(&d, EndpointChoice::Default);
        let t0 = Instant::now();
        s.tick_at(true, &hp, t0);
        assert_eq!(d.level("spk"), 40);
        assert!(hp.requests.borrow().is_empty());
        assert_eq!(s.device_id(), Some("spk"));
    }

    #[test]
    fn nothing_before_avrcp_volume_is_known() {
        let d = Devices::with(&[("spk", 0.8)]);
        let hp = FakeHeadphones::default();
        let mut s = syncer(&d, EndpointChoice::Default);
        s.tick_at(true, &hp, Instant::now());
        assert_eq!(d.level("spk"), scalar_to_level(0.8));
        assert_eq!(d.opens.get(), 0);
    }

    #[test]
    fn windows_change_goes_to_the_headphones_with_echo_suppression() {
        let d = Devices::with(&[("spk", 0.5)]);
        let hp = FakeHeadphones::at(64);
        let mut s = syncer(&d, EndpointChoice::Default);
        let t0 = Instant::now();
        s.tick_at(true, &hp, t0);

        // Volume key: Windows 64 -> 80
        d.set_level("spk", 80);
        s.tick_at(true, &hp, t0 + Duration::from_millis(200));
        assert_eq!(*hp.requests.borrow(), vec![80]);

        // The headphones still report the old value for a moment: not a
        // change made on the headphones, Windows keeps 80
        s.tick_at(true, &hp, t0 + Duration::from_millis(400));
        s.tick_at(true, &hp, t0 + Duration::from_millis(1100));
        assert_eq!(d.level("spk"), 80);

        // Confirmed: nothing more to do, no echo back to the headphones
        hp.confirm();
        s.tick_at(true, &hp, t0 + Duration::from_millis(1400));
        s.tick_at(true, &hp, t0 + Duration::from_millis(1600));
        assert_eq!(d.level("spk"), 80);
        assert_eq!(*hp.requests.borrow(), vec![80]);
    }

    #[test]
    fn headphone_change_goes_to_windows() {
        let d = Devices::with(&[("spk", 0.5)]);
        let hp = FakeHeadphones::at(64);
        let mut s = syncer(&d, EndpointChoice::Default);
        let t0 = Instant::now();
        s.tick_at(true, &hp, t0);

        // Button on the headphones
        hp.volume.set(Some(30));
        s.tick_at(true, &hp, t0 + Duration::from_millis(200));
        assert_eq!(d.level("spk"), 30);
        // No echo: Windows moved because of us, the headphones are not asked
        s.tick_at(true, &hp, t0 + Duration::from_millis(400));
        assert!(hp.requests.borrow().is_empty());
    }

    #[test]
    fn headphone_change_after_the_hold_is_taken() {
        let d = Devices::with(&[("spk", 0.5)]);
        let hp = FakeHeadphones::at(64);
        let mut s = syncer(&d, EndpointChoice::Default);
        let t0 = Instant::now();
        s.tick_at(true, &hp, t0);
        d.set_level("spk", 70);
        s.tick_at(true, &hp, t0 + Duration::from_millis(200));
        hp.confirm();
        // Within the hold a headphone change waits ...
        hp.volume.set(Some(20));
        s.tick_at(true, &hp, t0 + Duration::from_millis(600));
        assert_eq!(d.level("spk"), 70);
        // ... and is applied after it
        s.tick_at(true, &hp, t0 + Duration::from_millis(1300));
        assert_eq!(d.level("spk"), 20);
    }

    #[test]
    fn device_volume_steps_do_not_echo() {
        let d = Devices::with(&[("spk", 0.5)]);
        d.steps.set(10); // 10% steps: 33 cannot be set exactly
        let hp = FakeHeadphones::at(33);
        let mut s = syncer(&d, EndpointChoice::Default);
        let t0 = Instant::now();
        s.tick_at(true, &hp, t0);
        let taken = d.level("spk");
        assert_ne!(taken, 33);
        // What the device took is the reference: no request goes out
        s.tick_at(true, &hp, t0 + Duration::from_millis(200));
        assert!(hp.requests.borrow().is_empty());
    }

    #[test]
    fn mute_is_left_alone() {
        let d = Devices::with(&[("spk", 0.5)]);
        let hp = FakeHeadphones::at(64);
        let mut s = syncer(&d, EndpointChoice::Default);
        let t0 = Instant::now();
        s.tick_at(true, &hp, t0);
        // Muting the PC keeps its speakers silent; the headphones play on
        d.muted.borrow_mut().insert("spk".into(), true);
        s.tick_at(true, &hp, t0 + Duration::from_millis(200));
        assert!(hp.requests.borrow().is_empty());
        // Headphones at 0 set the level to 0 but do not mute Windows
        hp.volume.set(Some(0));
        s.tick_at(true, &hp, t0 + Duration::from_millis(400));
        assert_eq!(d.level("spk"), 0);
        d.muted.borrow_mut().insert("spk".into(), false);
        s.tick_at(true, &hp, t0 + Duration::from_millis(600));
        assert!(!d.muted.borrow()["spk"]);
        assert_eq!(d.level("spk"), 0);
        s.stop();
        assert!(!d.muted.borrow()["spk"]);
    }

    #[test]
    fn stop_restores_the_original_volume() {
        let d = Devices::with(&[("spk", 0.75)]);
        let hp = FakeHeadphones::at(20);
        let mut s = syncer(&d, EndpointChoice::Default);
        s.tick_at(true, &hp, Instant::now());
        assert_eq!(d.level("spk"), 20);
        s.tick_at(false, &hp, Instant::now());
        assert_eq!(d.volume.borrow()["spk"], 0.75);
        // Dropping also restores
        let mut s = syncer(&d, EndpointChoice::Default);
        s.tick_at(true, &hp, Instant::now());
        drop(s);
        assert_eq!(d.volume.borrow()["spk"], 0.75);
    }

    #[test]
    fn link_drop_resyncs_from_the_headphones() {
        let d = Devices::with(&[("spk", 0.5)]);
        let hp = FakeHeadphones::at(64);
        let mut s = syncer(&d, EndpointChoice::Default);
        let t0 = Instant::now();
        s.tick_at(true, &hp, t0);
        hp.volume.set(None);
        d.set_level("spk", 100); // changed while the link was down
        s.tick_at(true, &hp, t0 + Duration::from_millis(200));
        hp.volume.set(Some(50));
        s.tick_at(true, &hp, t0 + Duration::from_millis(400));
        // The headphones' volume wins after the link is back
        assert_eq!(d.level("spk"), 50);
        assert!(hp.requests.borrow().is_empty());
    }

    #[test]
    fn follows_the_default_device() {
        let d = Devices::with(&[("spk", 0.9), ("usb", 0.6)]);
        let hp = FakeHeadphones::at(40);
        let mut s = syncer(&d, EndpointChoice::Default);
        let t0 = Instant::now();
        s.tick_at(true, &hp, t0);
        assert_eq!(d.level("spk"), 40);

        // The default changes: the old device gets its volume back and the
        // new one takes the headphones' volume
        *d.default.borrow_mut() = Some("usb".into());
        s.tick_at(true, &hp, t0 + Duration::from_millis(200));
        assert_eq!(d.volume.borrow()["spk"], 0.9);
        assert_eq!(d.level("usb"), 40);
        assert_eq!(s.device_id(), Some("usb"));
        assert!(hp.requests.borrow().is_empty());
        s.stop();
        assert_eq!(d.volume.borrow()["usb"], 0.6);
    }

    #[test]
    fn fixed_device_ignores_the_default() {
        let d = Devices::with(&[("spk", 0.9), ("cable", 0.6)]);
        let hp = FakeHeadphones::at(40);
        let mut s = syncer(&d, EndpointChoice::Device("cable".into()));
        let t0 = Instant::now();
        s.tick_at(true, &hp, t0);
        *d.default.borrow_mut() = Some("cable".into());
        *d.default.borrow_mut() = Some("spk".into());
        s.tick_at(true, &hp, t0 + Duration::from_millis(200));
        assert_eq!(d.level("cable"), 40);
        assert_eq!(d.level("spk"), scalar_to_level(0.9));
        assert_eq!(d.opens.get(), 1);
    }

    #[test]
    fn missing_device_is_retried_quietly() {
        let d = Devices::with(&[("spk", 0.5)]);
        let hp = FakeHeadphones::at(40);
        let mut s = syncer(&d, EndpointChoice::Device("gone".into()));
        s.tick_at(true, &hp, Instant::now());
        s.tick_at(true, &hp, Instant::now());
        assert_eq!(s.device_id(), None);
        d.volume.borrow_mut().insert("gone".into(), 1.0);
        s.tick_at(true, &hp, Instant::now());
        assert_eq!(d.level("gone"), 40);
    }

    #[test]
    fn simulated_volume_takes_the_headset_volume_then_follows_its_steps() {
        // --le-sim-volume 64,3:90 against a headset at 50/127 (VCP 100/255)
        let spec = SimVolumeSpec { start: 64, steps: vec![(3000, 90)] };
        let mut sim = SimulatedWindowsVolume::new(spec);
        let hp = FakeHeadphones::at(50);
        let t0 = Instant::now();
        sim.tick_at(&hp, t0);
        // Windows takes the headset's volume over, nothing is written
        assert_eq!(sim.level(), 50);
        assert!(hp.requests.borrow().is_empty());
        sim.tick_at(&hp, t0 + Duration::from_millis(2900));
        assert!(hp.requests.borrow().is_empty());
        // The volume keys at 3 s: the headset follows
        sim.tick_at(&hp, t0 + Duration::from_millis(3000));
        assert_eq!(sim.level(), 90);
        assert_eq!(*hp.requests.borrow(), vec![90]);
        hp.confirm();
        sim.tick_at(&hp, t0 + Duration::from_millis(4500));
        assert_eq!(*hp.requests.borrow(), vec![90]);
        // A button on the headset: the simulated Windows volume follows
        hp.volume.set(Some(100));
        sim.tick_at(&hp, t0 + Duration::from_millis(4700));
        assert_eq!(sim.level(), 100);
        assert_eq!(*hp.requests.borrow(), vec![90]);
    }

    #[test]
    fn simulated_volume_from_the_command_line() {
        // As the emulator cases give it
        let sim = SimulatedWindowsVolume::parse("64,3:90").unwrap();
        assert_eq!(sim.level(), 64);
        assert_eq!(sim.spec.steps, [(3000, 90)]);
        assert!(SimulatedWindowsVolume::parse("64").is_some());
        assert!(SimulatedWindowsVolume::parse("200").is_none());
        assert!(SimulatedWindowsVolume::parse("64,x").is_none());
    }

    #[test]
    fn simulated_volume_waits_for_the_headset_volume() {
        let mut sim = SimulatedWindowsVolume::new(SimVolumeSpec { start: 64, steps: Vec::new() });
        let hp = FakeHeadphones::default();
        sim.tick_at(&hp, Instant::now());
        assert_eq!(sim.level(), 64);
        hp.volume.set(Some(20));
        sim.tick_at(&hp, Instant::now());
        assert_eq!(sim.level(), 20);
        assert!(hp.requests.borrow().is_empty());
    }

    #[test]
    fn simulated_endpoints_are_not_restored() {
        let sim = SimulatedEndpoints::new(64);
        assert_eq!(sim.default_device().as_deref(), Some(SIMULATED_DEVICE));
        assert!(sim.open("other").is_err());
        let hp = FakeHeadphones::at(30);
        let mut s = VolumeSyncer::with_provider(sim.clone(), EndpointChoice::Default);
        s.tick_at(true, &hp, Instant::now());
        assert_eq!(sim.level(), 30);
        s.stop();
        assert_eq!(sim.level(), 30);
    }

    #[test]
    fn choice_for_stream() {
        let virt = SourceRequest::VirtualDevice { id: "cable".into(), name: "CABLE".into(), auto_switch: true };
        let app = SourceRequest::Application { exe: "x.exe".into() };
        assert_eq!(EndpointChoice::for_stream(&VolumeSync::Off, &SourceRequest::SystemLoopback), None);
        assert_eq!(EndpointChoice::for_stream(&VolumeSync::Auto, &SourceRequest::TestTone), None);
        assert_eq!(
            EndpointChoice::for_stream(&VolumeSync::Auto, &SourceRequest::SystemLoopback),
            Some(EndpointChoice::Default)
        );
        assert_eq!(EndpointChoice::for_stream(&VolumeSync::Auto, &app), Some(EndpointChoice::Default));
        assert_eq!(EndpointChoice::for_stream(&VolumeSync::Auto, &virt), Some(EndpointChoice::Device("cable".into())));
        assert_eq!(
            EndpointChoice::for_stream(&VolumeSync::Device("spk".into()), &virt),
            Some(EndpointChoice::Device("spk".into()))
        );
        assert_eq!(EndpointChoice::for_stream(&VolumeSync::Device(String::new()), &app), Some(EndpointChoice::Default));
    }
}
