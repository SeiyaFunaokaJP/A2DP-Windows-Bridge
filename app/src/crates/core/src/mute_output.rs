//! Keeps the PC silent while streaming (issue #223): System Loopback copies
//! what the default output device plays, so that device would play along
//! with the headphones. Its mute does not reach the capture (see
//! volume_sync.rs), so a session mutes it and unmutes it at the end.
//!
//! Only a device this session muted is unmuted: one the user had muted stays
//! so. If BlueCaster exits unexpectedly, the device stays muted.
//!
//! Windows' mute key (or the mute in its volume flyout) unmutes that device,
//! and the PC would play (issue #86). An unmute of the device this session
//! muted is taken as that key: the device is muted again at once and the
//! headphones go silent ([`key_silenced`]); pressed again, they play again.
//! Windows' volume keys unmute too: an unmute with a new volume is taken as
//! one of them, so the output is muted again and the headphones play.
//! A watcher thread looks every [`KEY_WATCH`] so the speakers hardly sound.
//!
//! [`OutputMute`] lives on the session thread, like the
//! [`VolumeSyncer`](crate::volume_sync::VolumeSyncer), and uses the same
//! [`EndpointProvider`], so the logic is tested with fakes.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::volume_sync::{Endpoint, EndpointProvider, WindowsEndpoints};

/// A session stopped for one that mutes too (its settings changed): the
/// device stays muted through the reconnect instead of playing for a few
/// seconds, and unmuting it passes to the new session (as DefaultHandover)
#[derive(Debug, Default, PartialEq)]
pub struct MuteHandover {
    /// The session about to start mutes (set while the running one stops)
    next: bool,
    /// The device the stopped session left muted
    device: Option<String>,
}

static HANDOVER: Mutex<MuteHandover> = Mutex::new(MuteHandover { next: false, device: None });

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The headphones are silent: Windows' mute key was pressed while this
/// session keeps the output muted (one session streams at a time)
static KEY_SILENCE: AtomicBool = AtomicBool::new(false);

/// Whether the stream sends silence for Windows' mute key (the encode
/// thread asks, as for volume 0)
pub(crate) fn key_silenced() -> bool {
    KEY_SILENCE.load(Ordering::Acquire)
}

/// How often the watcher thread looks at the device's mute
const KEY_WATCH: Duration = Duration::from_millis(15);

/// The device this session muted and its volume when last looked at, for
/// the watcher; the lock also keeps the session thread and the watcher from
/// taking one key press twice
#[derive(Debug, Default)]
struct Watched {
    device: Option<String>,
    volume: Option<f32>,
}

type Ours = Arc<Mutex<Watched>>;

/// Looks at the device this session muted, under the [`Ours`] lock. Unmuted
/// in Windows: muted again; with the volume as it was, the mute key (the
/// headphones' silence turns over), else a volume key (they play).
fn key_pressed<E: Endpoint>(endpoint: &E, watched: &mut Watched, silence: &AtomicBool) {
    let volume = endpoint.volume();
    let changed = volume != watched.volume;
    watched.volume = volume;
    if endpoint.muted() != Some(false) {
        return;
    }
    if !endpoint.set_muted(true) {
        tracing::warn!("output mute: unmuted in Windows and cannot be muted again");
        return;
    }
    let silent = if changed {
        silence.store(false, Ordering::Release);
        false
    } else {
        !silence.fetch_xor(true, Ordering::AcqRel)
    };
    tracing::info!(
        "output mute: Windows' {} key, the output stays muted and the headphones {}",
        if changed { "volume" } else { "mute" },
        if silent { "go silent" } else { "play" }
    );
}

/// Watches the device this session muted between the session's ticks
struct KeyWatch {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl KeyWatch {
    fn start(ours: Ours) -> Option<KeyWatch> {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::Builder::new()
            .name("mute-key".into())
            .spawn(move || {
                let mut open: Option<audio_capture::EndpointVolume> = None;
                while !flag.load(Ordering::Acquire) {
                    std::thread::sleep(KEY_WATCH);
                    let mut watched = lock(&ours);
                    let Some(id) = watched.device.clone() else {
                        open = None;
                        continue;
                    };
                    if open.as_ref().map(audio_capture::EndpointVolume::device_id) != Some(id.as_str()) {
                        open = audio_capture::EndpointVolume::open(Some(&id)).ok();
                    }
                    if let Some(e) = &open {
                        key_pressed(e, &mut watched, &KEY_SILENCE);
                    }
                }
            })
            .map_err(|e| tracing::warn!("output mute: no mute key watcher: {e}"))
            .ok()?;
        Some(KeyWatch { stop, thread: Some(thread) })
    }
}

impl Drop for KeyWatch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Whether the session about to start mutes: set while the running one stops
pub(crate) fn expect_session(mutes: bool) {
    lock(&HANDOVER).next = mutes;
}

/// After a session ended: unmutes the device a stopped session left to one
/// that never got as far as taking it
pub(crate) fn unmute_unclaimed() {
    unclaimed(&HANDOVER, &WindowsEndpoints);
}

fn unclaimed<P: EndpointProvider>(handover: &Mutex<MuteHandover>, provider: &P) {
    let device = {
        let mut h = lock(handover);
        if h.next { None } else { h.device.take() }
    };
    if let Some(id) = device {
        unmute_device(provider, &id);
    }
}

fn unmute<E: Endpoint>(endpoint: &E) {
    tracing::debug!("output mute: unmuting {}", endpoint.device_id());
    if !endpoint.set_muted(false) {
        tracing::warn!("could not unmute the output device");
    }
}

fn unmute_device<P: EndpointProvider>(provider: &P, id: &str) {
    match provider.open(id) {
        Ok(endpoint) => unmute(&endpoint),
        Err(e) => tracing::warn!("could not unmute the output device: {e}"),
    }
}

/// Mutes `endpoint`; true if this did it (it was not muted already)
fn mute<E: Endpoint>(endpoint: &E) -> bool {
    match endpoint.muted() {
        Some(false) if endpoint.set_muted(true) => {
            tracing::info!("output mute: default output device muted");
            true
        }
        Some(true) => {
            tracing::debug!("output mute: {} was muted already", endpoint.device_id());
            false
        }
        _ => {
            tracing::warn!("could not mute the default output device");
            false
        }
    }
}

struct Attached<E> {
    endpoint: E,
    /// This session muted it, so it unmutes it
    ours: bool,
}

/// Mutes the default output device, following it when it changes; call
/// [`tick`](OutputMute::tick) a few times per second while streaming.
/// Dropping it unmutes.
pub struct OutputMute<P: EndpointProvider = WindowsEndpoints> {
    provider: P,
    attached: Option<Attached<P::Endpoint>>,
    /// The device a stopped session left muted, not taken yet
    inherited: Option<String>,
    /// Device that could not be opened (logged once, not every tick)
    failed: Option<String>,
    handover: &'static Mutex<MuteHandover>,
    /// The device this session muted, shared with the watcher
    ours: Ours,
    silence: &'static AtomicBool,
    watch: Option<KeyWatch>,
}

impl OutputMute<WindowsEndpoints> {
    pub fn new() -> Self {
        let mut m = OutputMute::with_provider(WindowsEndpoints, &HANDOVER, &KEY_SILENCE);
        m.watch = KeyWatch::start(m.ours.clone());
        m
    }
}

impl Default for OutputMute<WindowsEndpoints> {
    fn default() -> Self {
        OutputMute::new()
    }
}

impl<P: EndpointProvider> OutputMute<P> {
    pub fn with_provider(provider: P, handover: &'static Mutex<MuteHandover>, silence: &'static AtomicBool) -> Self {
        let inherited = lock(handover).device.take();
        silence.store(false, Ordering::Release);
        OutputMute {
            provider,
            attached: None,
            inherited,
            failed: None,
            handover,
            ours: Ours::default(),
            silence,
            watch: None,
        }
    }

    /// One step. `enabled` false unmutes (another source, the headphones
    /// gone: the PC is what plays then); true mutes again.
    pub fn tick(&mut self, enabled: bool) {
        if !enabled {
            self.release();
            return;
        }
        let Some(id) = self.provider.default_device() else {
            return;
        };
        if let Some(a) = self.attached.as_ref().filter(|a| a.endpoint.device_id() == id) {
            // Unmuted in Windows (its mute key): the watcher may have been first
            if a.ours {
                key_pressed(&a.endpoint, &mut lock(&self.ours), self.silence);
            }
            return;
        }
        let endpoint = match self.provider.open(&id) {
            Ok(endpoint) => endpoint,
            Err(e) => {
                if self.failed.as_deref() != Some(id.as_str()) {
                    tracing::warn!("output mute: cannot open the output device's volume: {e}");
                    self.failed = Some(id);
                }
                return;
            }
        };
        self.failed = None;
        self.release_attached();
        let ours = if self.inherited.as_deref() == Some(id.as_str()) {
            self.inherited = None;
            true
        } else {
            mute(&endpoint)
        };
        if ours {
            *lock(&self.ours) = Watched { device: Some(id), volume: endpoint.volume() };
        }
        self.attached = Some(Attached { endpoint, ours });
        self.release_inherited();
    }

    fn release(&mut self) {
        self.release_attached();
        self.release_inherited();
    }

    fn release_attached(&mut self) {
        // The watcher lets go first, so it does not take the unmute for the key
        *lock(&self.ours) = Watched::default();
        self.silence.store(false, Ordering::Release);
        if let Some(a) = self.attached.take().filter(|a| a.ours) {
            unmute(&a.endpoint);
        }
    }

    fn release_inherited(&mut self) {
        if let Some(id) = self.inherited.take() {
            unmute_device(&self.provider, &id);
        }
    }
}

impl<P: EndpointProvider> Drop for OutputMute<P> {
    fn drop(&mut self) {
        self.watch = None;
        {
            let mut h = lock(self.handover);
            if h.next {
                let ours = self.attached.as_ref().filter(|a| a.ours).map(|a| a.endpoint.device_id().to_string());
                if let Some(id) = ours.or(self.inherited.take()) {
                    tracing::debug!("output mute: {id} kept for the next session");
                    h.device = Some(id);
                    self.attached = None;
                }
            }
        }
        self.release();
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    use super::*;

    #[derive(Default)]
    struct Devices {
        default: RefCell<Option<String>>,
        /// Device -> muted
        muted: RefCell<HashMap<String, bool>>,
        /// Muting fails
        refuse: RefCell<bool>,
        /// Device -> volume scalar
        volume: RefCell<HashMap<String, f32>>,
    }

    impl Devices {
        fn with(ids: &[&str]) -> Rc<Devices> {
            let d = Rc::new(Devices::default());
            for id in ids {
                d.muted.borrow_mut().insert(id.to_string(), false);
            }
            *d.default.borrow_mut() = ids.first().map(|s| s.to_string());
            d
        }
        fn muted(&self, id: &str) -> bool {
            self.muted.borrow()[id]
        }
        fn set(&self, id: &str, muted: bool) {
            self.muted.borrow_mut().insert(id.into(), muted);
        }
        fn make_default(&self, id: &str) {
            *self.default.borrow_mut() = Some(id.into());
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
        fn set_volume(&self, _scalar: f32) -> bool {
            false
        }
        fn muted(&self) -> Option<bool> {
            self.devices.muted.borrow().get(&self.id).copied()
        }
        fn set_muted(&self, muted: bool) -> bool {
            if *self.devices.refuse.borrow() {
                return false;
            }
            self.devices.set(&self.id, muted);
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
            if !self.0.muted.borrow().contains_key(id) {
                return Err(format!("no device {id}"));
            }
            Ok(FakeEndpoint { id: id.into(), devices: self.0.clone() })
        }
    }

    fn handover() -> &'static Mutex<MuteHandover> {
        Box::leak(Box::default())
    }

    fn session(devices: &Rc<Devices>, h: &'static Mutex<MuteHandover>) -> OutputMute<FakeProvider> {
        OutputMute::with_provider(FakeProvider(devices.clone()), h, Box::leak(Box::default()))
    }

    #[test]
    fn mutes_the_default_device_and_unmutes_at_the_end() {
        let d = Devices::with(&["a", "b"]);
        let mut m = session(&d, handover());
        m.tick(true);
        assert!(d.muted("a") && !d.muted("b"));
        drop(m);
        assert!(!d.muted("a"));
    }

    #[test]
    fn a_device_the_user_had_muted_stays_muted() {
        let d = Devices::with(&["a"]);
        d.set("a", true);
        let mut m = session(&d, handover());
        m.tick(true);
        drop(m);
        assert!(d.muted("a"));
    }

    #[test]
    fn windows_mute_key_silences_the_headphones_and_keeps_the_output_muted() {
        let d = Devices::with(&["a"]);
        let mut m = session(&d, handover());
        m.tick(true);
        assert!(!m.silence.load(Ordering::Acquire));
        // The key unmutes the device: muted again, the headphones silent
        d.set("a", false);
        m.tick(true);
        assert!(d.muted("a") && m.silence.load(Ordering::Acquire));
        // Pressed again: they play again, the output still muted
        d.set("a", false);
        m.tick(true);
        assert!(d.muted("a") && !m.silence.load(Ordering::Acquire));
        d.set("a", false);
        m.tick(true);
        assert!(m.silence.load(Ordering::Acquire));
        // The session ends (or the mute is turned off): silence ends too
        m.tick(false);
        assert!(!d.muted("a") && !m.silence.load(Ordering::Acquire));
    }

    #[test]
    fn windows_volume_keys_keep_the_output_muted_and_the_headphones_playing() {
        let d = Devices::with(&["a"]);
        d.volume.borrow_mut().insert("a".into(), 0.5);
        let mut m = session(&d, handover());
        m.tick(true);
        // The mute key: silent
        d.set("a", false);
        m.tick(true);
        assert!(m.silence.load(Ordering::Acquire));
        // Volume up unmutes too, with a new volume: muted again, playing
        d.volume.borrow_mut().insert("a".into(), 0.6);
        d.set("a", false);
        m.tick(true);
        assert!(d.muted("a") && !m.silence.load(Ordering::Acquire));
        // A volume change alone (volume sync) changes nothing
        d.volume.borrow_mut().insert("a".into(), 0.4);
        m.tick(true);
        assert!(d.muted("a") && !m.silence.load(Ordering::Acquire));
    }

    #[test]
    fn a_device_the_user_had_muted_is_not_watched() {
        let d = Devices::with(&["a"]);
        d.set("a", true);
        let mut m = session(&d, handover());
        m.tick(true);
        d.set("a", false);
        m.tick(true);
        assert!(!d.muted("a") && !m.silence.load(Ordering::Acquire));
    }

    #[test]
    fn follows_the_default_device() {
        let d = Devices::with(&["a", "b"]);
        let mut m = session(&d, handover());
        m.tick(true);
        d.make_default("b");
        m.tick(true);
        assert!(!d.muted("a") && d.muted("b"));
        drop(m);
        assert!(!d.muted("b"));
    }

    #[test]
    fn disabled_unmutes_and_enabled_mutes_again() {
        let d = Devices::with(&["a"]);
        let mut m = session(&d, handover());
        m.tick(true);
        m.tick(false);
        assert!(!d.muted("a"));
        m.tick(true);
        assert!(d.muted("a"));
    }

    #[test]
    fn a_device_that_cannot_be_muted_is_not_unmuted_later() {
        let d = Devices::with(&["a"]);
        *d.refuse.borrow_mut() = true;
        let mut m = session(&d, handover());
        m.tick(true);
        assert!(!d.muted("a"));
        *d.refuse.borrow_mut() = false;
        d.set("a", true);
        drop(m);
        assert!(d.muted("a"));
    }

    #[test]
    fn a_missing_device_is_skipped() {
        let d = Devices::with(&["a"]);
        d.make_default("gone");
        let mut m = session(&d, handover());
        m.tick(true);
        m.tick(true);
        d.make_default("a");
        m.tick(true);
        assert!(d.muted("a"));
    }

    #[test]
    fn the_next_session_takes_the_device_over() {
        let d = Devices::with(&["a"]);
        let h = handover();
        let mut first = session(&d, h);
        first.tick(true);
        lock(h).next = true;
        drop(first);
        lock(h).next = false;
        assert!(d.muted("a"));
        let mut second = session(&d, h);
        second.tick(true);
        drop(second);
        assert!(!d.muted("a"), "the second session unmutes what the first muted");
    }

    #[test]
    fn a_next_session_on_another_device_unmutes_the_one_left() {
        let d = Devices::with(&["a", "b"]);
        let h = handover();
        let mut first = session(&d, h);
        first.tick(true);
        lock(h).next = true;
        drop(first);
        lock(h).next = false;
        d.make_default("b");
        let mut second = session(&d, h);
        second.tick(true);
        assert!(!d.muted("a") && d.muted("b"));
    }

    #[test]
    fn a_next_session_that_does_not_mute_unmutes_the_one_left() {
        let d = Devices::with(&["a"]);
        let h = handover();
        let mut first = session(&d, h);
        first.tick(true);
        lock(h).next = true;
        drop(first);
        lock(h).next = false;
        let mut second = session(&d, h);
        second.tick(false);
        assert!(!d.muted("a"));
    }

    #[test]
    fn a_next_session_that_never_starts_leaves_it_to_unclaimed() {
        let d = Devices::with(&["a"]);
        let h = handover();
        let mut first = session(&d, h);
        first.tick(true);
        lock(h).next = true;
        drop(first);
        // The stopped session's thread ends while the next is still expected
        unclaimed(h, &FakeProvider(d.clone()));
        assert!(d.muted("a"));
        lock(h).next = false;
        unclaimed(h, &FakeProvider(d.clone()));
        assert!(!d.muted("a"));
    }

    #[test]
    fn a_device_the_user_had_muted_is_not_handed_over() {
        let d = Devices::with(&["a"]);
        d.set("a", true);
        let h = handover();
        let mut first = session(&d, h);
        first.tick(true);
        lock(h).next = true;
        drop(first);
        assert_eq!(lock(h).device, None);
    }
}
