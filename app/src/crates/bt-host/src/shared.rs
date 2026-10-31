//! State the BTstack thread shares with the callers of [`crate::BtHost`]: what
//! the blocking calls wait for (v1 used a Windows event per wait) and what
//! they read afterwards. One mutex and one condition variable; the BTstack
//! thread holds the lock only to update a few fields.

use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use codec::caps::RemoteCaps;

use crate::{ConnectFailure, DiscoveredDevice, InitFailure};

#[derive(Default)]
pub struct State {
    // ---- init ----
    /// When hci_power_control(ON) was called
    pub power_on_at: Option<Instant>,
    /// Events from the controller since power on (tells one that never
    /// answers from one that is slow, e.g. downloading firmware)
    pub controller_events: u32,
    pub init_done: Option<Result<(), InitFailure>>,
    pub hci_ready: bool,
    pub hci_off: bool,
    /// A Realtek controller came up on its ROM code: the firmware download
    /// did not run or did not take (issue #81: the adapter opens, but
    /// connections and streams fail)
    pub realtek_rom: bool,
    /// The controller has LE Audio's isochronous channels (from its
    /// supported commands at HCI init, with LE Audio on or off)
    pub le_audio_capable: bool,
    /// The controller's own Bluetooth address ("AA:BB:CC:DD:EE:FF"), once it
    /// has answered Read BD_ADDR
    pub local_address: Option<String>,
    /// The HCI version the controller reports (Bluetooth core version:
    /// 12 = 5.3), once it has answered Read Local Version
    pub hci_version: Option<u8>,
    /// The headphones' battery level in percent (HFP), once they said
    pub battery: Option<u8>,

    // ---- one-shot completions, cleared by the caller before each request ----
    pub connect_done: Option<bool>,
    pub stream_done: Option<bool>,
    pub start_done: Option<bool>,
    pub released: bool,
    pub acl_down: bool,
    pub inquiry_done: bool,
    pub name_done: bool,
    pub probe_answered: bool,
    pub probe_pending: bool,

    /// Set by a user stop: ends waits for connections (stays set across
    /// reconnect attempts until the next user-initiated operation)
    pub cancel: bool,

    // ---- connection ----
    pub connected: bool,
    pub streaming: bool,
    pub disconnect_occurred: bool,
    /// HCI reason of the last lost connection (0: the device released A2DP
    /// but kept the link)
    pub link_lost_reason: u8,
    pub last_connect_failure: Option<ConnectFailure>,
    /// ACL link to the device being connected, and why it went down (0 =
    /// still up / none): a device that ends the link itself vs. a link lost
    pub target_acl_handle: Option<u16>,
    pub target_acl_down_reason: u8,
    /// HCI status of the last Authentication Complete on that link
    pub target_auth_status: Option<u8>,
    /// Device of the last ACL link that came up (also one the device
    /// opened itself, e.g. headphones turned back on)
    pub acl_up_from: Option<[u8; 6]>,
    /// Devices that asked to connect to the PC (Connection Request), until
    /// their link is up
    pub acl_requests: Vec<[u8; 6]>,
    /// The ACL links up now (handle, device, opened by the device itself
    /// and not streamed on yet)
    pub acl_links: Vec<(u16, [u8; 6], bool)>,
    /// ACL handle of the A2DP signaling channel
    pub a2dp_con_handle: Option<u16>,
    pub remote_caps: RemoteCaps,
    /// The devices of the scan, one entry each ([`State::add_found`])
    pub discovered: Vec<DiscoveredDevice>,
    /// How to page each device found (page scan repetition mode, clock
    /// offset): what a name request for it needs
    pub discovered_paging: Vec<([u8; 6], u8, u16)>,
    /// Counts the changes to `discovered` worth showing (a new device, a
    /// name): the scan reports the list on each one
    pub found_rev: u64,
    /// The user stopped the scan (or a connection is starting): its
    /// inquiry and name requests end
    pub scan_stop: bool,
    /// Media payload size of the stream (RTP header excluded)
    pub media_mtu: u16,
    /// The stack stopped working (see BtHost::stalled)
    pub stalled: bool,
}

impl State {
    /// The handle of the ACL link up to `addr`
    pub fn acl_link_to(&self, addr: &[u8; 6]) -> Option<u16> {
        self.acl_links.iter().find(|(_, a, _)| a == addr).map(|&(h, _, _)| h)
    }

    /// An inquiry result: a device answers many times during an inquiry
    /// (with RSSI / EIR every time it is heard), so a known one is updated
    /// in place. Its RSSI follows silently; true when the list changed in a
    /// way worth showing (new device, name, class).
    pub fn add_found(&mut self, d: DiscoveredDevice, psrm: u8, clock: u16) -> bool {
        match self.discovered_paging.iter_mut().find(|x| x.0 == d.addr) {
            Some(p) => *p = (d.addr, psrm, clock),
            None => self.discovered_paging.push((d.addr, psrm, clock)),
        }
        let changed = match self.discovered.iter_mut().find(|x| x.addr == d.addr) {
            Some(known) => {
                known.rssi = d.rssi;
                // A response without a name (no EIR, or a shortened one
                // before the complete one) keeps the name known
                let name = !d.name.is_empty() && d.name != known.name;
                if name {
                    known.name = d.name;
                }
                let class = d.class_of_device != 0 && d.class_of_device != known.class_of_device;
                if class {
                    known.class_of_device = d.class_of_device;
                }
                name || class
            }
            None => {
                self.discovered.push(d);
                true
            }
        };
        if changed {
            self.found_rev += 1;
        }
        changed
    }

    /// A remote name request answered: true when the list changed
    pub fn set_found_name(&mut self, addr: &[u8; 6], name: &str) -> bool {
        let Some(d) = self.discovered.iter_mut().find(|d| d.addr == *addr && d.name.is_empty()) else {
            return false;
        };
        d.name = name.to_string();
        self.found_rev += 1;
        true
    }
}

#[derive(Default)]
pub struct Shared {
    state: Mutex<State>,
    cv: Condvar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wait {
    Done,
    Cancelled,
    TimedOut,
}

impl Shared {
    pub fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Changes the state and wakes the waiters
    pub fn update<R>(&self, f: impl FnOnce(&mut State) -> R) -> R {
        let r = f(&mut self.lock());
        self.cv.notify_all();
        r
    }

    /// Waits until `done` holds, `timeout` passes or (if `cancellable`) a
    /// cancel is requested
    pub fn wait(&self, timeout: Duration, cancellable: bool, done: impl Fn(&State) -> bool) -> Wait {
        let deadline = Instant::now() + timeout;
        let mut s = self.lock();
        loop {
            if done(&s) {
                return Wait::Done;
            }
            if cancellable && s.cancel {
                return Wait::Cancelled;
            }
            let now = Instant::now();
            if now >= deadline {
                return Wait::TimedOut;
            }
            s = self.cv.wait_timeout(s, deadline - now).unwrap_or_else(|e| e.into_inner()).0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(addr: u8, name: &str, cod: u32, rssi: i8) -> DiscoveredDevice {
        DiscoveredDevice { addr: [0, 0, 0, 0, 0, addr], name: name.into(), class_of_device: cod, rssi }
    }

    #[test]
    fn a_link_the_device_opened_is_found_by_its_address() {
        let mut s = State::default();
        let (a, b) = ([0, 0, 0, 0, 0, 1], [0, 0, 0, 0, 0, 2]);
        s.acl_links.push((4, a, true));
        assert_eq!(s.acl_link_to(&a), Some(4));
        assert_eq!(s.acl_link_to(&b), None);
    }

    #[test]
    fn repeated_responses_are_one_device() {
        let mut s = State::default();
        assert!(s.add_found(dev(1, "", 0x240404, -60), 1, 0x1000));
        assert_eq!(s.found_rev, 1);
        // Heard again: RSSI and paging follow, nothing to show
        assert!(!s.add_found(dev(1, "", 0x240404, -50), 2, 0x2000));
        assert_eq!(s.found_rev, 1);
        assert_eq!(s.discovered, vec![dev(1, "", 0x240404, -50)]);
        assert_eq!(s.discovered_paging, vec![([0, 0, 0, 0, 0, 1], 2, 0x2000)]);
        // Its EIR now has the name
        assert!(s.add_found(dev(1, "Headphones", 0x240404, -55), 2, 0x2000));
        // A response without it keeps the name
        assert!(!s.add_found(dev(1, "", 0x240404, -58), 2, 0x2000));
        assert_eq!(s.discovered, vec![dev(1, "Headphones", 0x240404, -58)]);
        assert!(s.add_found(dev(2, "Phone", 0x5A020C, -70), 1, 0));
        assert_eq!(s.discovered.len(), 2);
        assert_eq!(s.discovered_paging.len(), 2);
        assert_eq!(s.found_rev, 3);
    }

    #[test]
    fn name_request_fills_a_missing_name_only() {
        let mut s = State::default();
        s.add_found(dev(1, "", 0x240404, -60), 1, 0);
        s.add_found(dev(2, "Named", 0x240404, -60), 1, 0);
        let rev = s.found_rev;
        assert!(s.set_found_name(&[0, 0, 0, 0, 0, 1], "Earbuds"));
        assert!(!s.set_found_name(&[0, 0, 0, 0, 0, 2], "Other"));
        assert!(!s.set_found_name(&[0, 0, 0, 0, 0, 3], "Unknown"));
        assert_eq!(s.discovered[0].name, "Earbuds");
        assert_eq!(s.discovered[1].name, "Named");
        assert_eq!(s.found_rev, rev + 1);
    }
}
