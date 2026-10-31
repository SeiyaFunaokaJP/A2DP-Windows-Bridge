//! Keeps the PC silent while System Loopback streams (issue #223): the
//! capture copies what the default output device plays, so that device would
//! play along with the headphones. Its mute does not reach the capture (see
//! volume_sync.rs), so a session mutes it and unmutes it at the end.
//!
//! Only a device this session muted is unmuted: one the user had muted stays
//! so. An unmute by the user while streaming is left alone until the default
//! device changes. If A2DPWB exits unexpectedly, the device stays muted.
//!
//! [`OutputMute`] lives on the session thread, like the
//! [`VolumeSyncer`](crate::volume_sync::VolumeSyncer), and uses the same
//! [`EndpointProvider`], so the logic is tested with fakes.

use std::sync::{Mutex, MutexGuard};

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

fn lock(handover: &Mutex<MuteHandover>) -> MutexGuard<'_, MuteHandover> {
    handover.lock().unwrap_or_else(|e| e.into_inner())
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
}

impl OutputMute<WindowsEndpoints> {
    pub fn new() -> Self {
        OutputMute::with_provider(WindowsEndpoints, &HANDOVER)
    }
}

impl Default for OutputMute<WindowsEndpoints> {
    fn default() -> Self {
        OutputMute::new()
    }
}

impl<P: EndpointProvider> OutputMute<P> {
    pub fn with_provider(provider: P, handover: &'static Mutex<MuteHandover>) -> Self {
        let inherited = lock(handover).device.take();
        OutputMute { provider, attached: None, inherited, failed: None, handover }
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
        if self.attached.as_ref().is_some_and(|a| a.endpoint.device_id() == id) {
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
        self.attached = Some(Attached { endpoint, ours });
        self.release_inherited();
    }

    fn release(&mut self) {
        self.release_attached();
        self.release_inherited();
    }

    fn release_attached(&mut self) {
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
            None
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
        OutputMute::with_provider(FakeProvider(devices.clone()), h)
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
    fn an_unmute_by_the_user_is_left_alone() {
        let d = Devices::with(&["a"]);
        let mut m = session(&d, handover());
        m.tick(true);
        d.set("a", false);
        m.tick(true);
        assert!(!d.muted("a"));
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
