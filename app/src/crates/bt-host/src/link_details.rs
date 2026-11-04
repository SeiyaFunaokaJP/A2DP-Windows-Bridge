//! Link details of each ACL link: the remote device's chip (version and
//! features), role and mode as the controller reports them, and the
//! readings polled with RSSI. The A2DP link's go to [`LinkStats::link`].
//!
//! Commands go out one at a time when the controller takes one, never more
//! than one of each per radio reading. One the controller does not list in
//! its supported commands is never sent (a command left unanswered would
//! hold BTstack's command slot for good); one it refuses as unknown is not
//! sent again until HCI starts over.

use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use btstack_sys::Stack;
use btstack_sys::consts::*;

use crate::events::{addr_at, addr_str, le16, u8_at};
use crate::stats::{LinkDetails, LinkMode, LinkStats, RemoteFeatures, RemoteVersion};

/// Unknown HCI Command / Unsupported Feature or Parameter Value: the
/// controller does not have it
const ERROR_CODE_UNKNOWN_HCI_COMMAND: u8 = 0x01;
const ERROR_CODE_UNSUPPORTED_FEATURE_OR_PARAMETER_VALUE: u8 = 0x11;

/// A reading of a link
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinkRead {
    FailedContacts,
    LinkQuality,
    RemoteVersion,
    SupervisionTimeout,
}

impl LinkRead {
    fn opcode(self) -> u16 {
        match self {
            LinkRead::FailedContacts => HCI_OPCODE_READ_FAILED_CONTACT_COUNTER,
            LinkRead::LinkQuality => HCI_OPCODE_READ_LINK_QUALITY,
            LinkRead::RemoteVersion => HCI_OPCODE_READ_REMOTE_VERSION_INFORMATION,
            LinkRead::SupervisionTimeout => HCI_OPCODE_READ_LINK_SUPERVISION_TIMEOUT,
        }
    }

    /// Its bit in Read Local Supported Commands: (octet, bit)
    fn supported_bit(self) -> (usize, u8) {
        match self {
            LinkRead::FailedContacts => (15, 2),
            LinkRead::LinkQuality => (15, 4),
            LinkRead::RemoteVersion => (2, 7),
            LinkRead::SupervisionTimeout => (11, 0),
        }
    }

    fn send(self, stack: &Stack, handle: u16) -> u8 {
        match self {
            LinkRead::FailedContacts => stack.cmd_read_failed_contact_counter(handle),
            LinkRead::LinkQuality => stack.cmd_read_link_quality(handle),
            LinkRead::RemoteVersion => stack.cmd_read_remote_version(handle),
            LinkRead::SupervisionTimeout => stack.cmd_read_link_supervision_timeout(handle),
        }
    }
}

#[derive(Debug, Clone)]
struct Acl {
    addr: [u8; 6],
    d: LinkDetails,
    /// The last Failed Contact Counter reading and when
    fcc_last: Option<(u16, Instant)>,
}

#[derive(Debug, Default)]
pub(crate) struct LinkTracker {
    links: HashMap<u16, Acl>,
    reads: VecDeque<(LinkRead, u16)>,
    /// Opcodes the controller refused as unknown in this HCI session
    refused: Vec<u16>,
    /// The controller's supported commands (Read Local Supported Commands
    /// during HCI init); None: not read, everything is tried
    supported: Option<[u8; 64]>,
}

/// Rises of a Failed Contact Counter from `prev` to `now`: it counts packets
/// given up on in a row and starts over at 0 when one gets through, so a
/// lower reading is a restart and counts as `now` new ones
pub(crate) fn failed_contact_rises(prev: u16, now: u16) -> u64 {
    if now >= prev { u64::from(now - prev) } else { u64::from(now) }
}

/// The largest ACL packet type the features allow ("3-DH5", "2-DH3", "DH1")
pub fn best_packet(f: &RemoteFeatures) -> &'static str {
    if f.edr_3m() || f.edr_2m() {
        let slots = if f.edr_five_slot() {
            5
        } else if f.edr_three_slot() {
            3
        } else {
            1
        };
        return match (f.edr_3m(), slots) {
            (true, 5) => "3-DH5",
            (true, 3) => "3-DH3",
            (true, _) => "3-DH1",
            (false, 5) => "2-DH5",
            (false, 3) => "2-DH3",
            (false, _) => "2-DH1",
        };
    }
    if f.five_slot() {
        "DH5"
    } else if f.three_slot() {
        "DH3"
    } else {
        "DH1"
    }
}

impl LinkTracker {
    /// HCI starts (again): commands refused before may work with another
    /// controller
    pub fn reset(&mut self) {
        // Read during the init that just ended
        let supported = self.supported.take();
        *self = LinkTracker { supported, ..Default::default() };
    }

    fn listed(&self, read: LinkRead) -> bool {
        let (octet, bit) = read.supported_bit();
        self.supported.is_none_or(|s| s[octet] & (1 << bit) != 0)
    }

    /// A link came up (Connection Complete, success)
    pub fn connected(&mut self, stack: &Stack, handle: u16, addr: [u8; 6]) {
        let d =
            LinkDetails { central: stack.acl_role_central(handle), mode: Some(LinkMode::Active), ..Default::default() };
        self.links.insert(handle, Acl { addr, d, fcc_last: None });
    }

    pub fn disconnected(&mut self, handle: u16) {
        self.links.remove(&handle);
        self.reads.retain(|&(_, h)| h != handle);
    }

    /// The A2DP signaling channel is up on `handle`: what is read once per
    /// link
    pub fn a2dp_opened(&mut self, stack: &Stack, handle: u16) {
        let Some(acl) = self.links.get_mut(&handle) else { return };
        acl.d.encryption_key_size = stack.acl_encryption_key_size(handle);
        if acl.d.central.is_none() {
            acl.d.central = stack.acl_role_central(handle);
        }
        let version = acl.d.remote_version.is_none();
        let features = acl.d.remote_features.is_none();
        if version {
            self.queue(LinkRead::RemoteVersion, handle);
        }
        self.queue(LinkRead::SupervisionTimeout, handle);
        if features {
            stack.remote_features_query(handle);
        }
    }

    /// The stream started: the per-stream counts start over
    pub fn stream_started(&mut self, handle: Option<u16>) {
        if let Some(acl) = handle.and_then(|h| self.links.get_mut(&h)) {
            acl.d.role_changes = 0;
            acl.d.sniff_entries = 0;
            acl.d.failed_contacts_stream = 0;
        }
    }

    /// With each RSSI reading of the A2DP link: the polled readings, and the
    /// key size if BTstack did not have it before
    pub fn poll(&mut self, stack: &Stack, handle: u16) {
        if let Some(acl) = self.links.get_mut(&handle)
            && acl.d.encryption_key_size.is_none()
        {
            acl.d.encryption_key_size = stack.acl_encryption_key_size(handle);
        }
        self.queue(LinkRead::FailedContacts, handle);
        self.queue(LinkRead::LinkQuality, handle);
    }

    fn queue(&mut self, read: LinkRead, handle: u16) {
        if self.listed(read) && !self.refused.contains(&read.opcode()) && !self.reads.contains(&(read, handle)) {
            self.reads.push_back((read, handle));
        }
    }

    /// Sends the next reading if the controller takes a command now (after
    /// each Command Complete / Status, and after queueing)
    pub fn send_next(&mut self, stack: &Stack) {
        while let Some(&(read, handle)) = self.reads.front() {
            if !self.links.contains_key(&handle) {
                self.reads.pop_front();
                continue;
            }
            if !stack.can_send_command_now() {
                return;
            }
            self.reads.pop_front();
            let status = read.send(stack, handle);
            if status == ERROR_CODE_SUCCESS {
                return;
            }
            tracing::debug!("link details: {read:?} not sent (0x{status:02X})");
        }
    }

    /// A command the controller did not take: unknown ones are not sent
    /// again
    fn refused(&mut self, opcode: u16, status: u8) {
        if matches!(status, ERROR_CODE_UNKNOWN_HCI_COMMAND | ERROR_CODE_UNSUPPORTED_FEATURE_OR_PARAMETER_VALUE)
            && !self.refused.contains(&opcode)
        {
            tracing::info!("link details: the controller does not support command 0x{opcode:04X} (0x{status:02X})");
            self.refused.push(opcode);
            self.reads.retain(|&(r, _)| r.opcode() != opcode);
        }
    }

    fn ours(opcode: u16) -> bool {
        [LinkRead::FailedContacts, LinkRead::LinkQuality, LinkRead::RemoteVersion, LinkRead::SupervisionTimeout]
            .iter()
            .any(|r| r.opcode() == opcode)
    }

    /// An HCI event; true when a link's details changed
    pub fn on_hci(&mut self, ev: u8, p: &[u8], streaming: bool) -> bool {
        match ev {
            HCI_EVENT_COMMAND_COMPLETE => {
                let (Some(opcode), Some(status)) = (le16(p, 3), u8_at(p, 5)) else { return false };
                if opcode == HCI_OPCODE_READ_LOCAL_SUPPORTED_COMMANDS {
                    if let (ERROR_CODE_SUCCESS, Some(bitmap)) = (status, p.get(6..70)) {
                        let mut s = [0u8; 64];
                        s.copy_from_slice(bitmap);
                        self.supported = Some(s);
                        let skipped: Vec<LinkRead> = [
                            LinkRead::FailedContacts,
                            LinkRead::LinkQuality,
                            LinkRead::RemoteVersion,
                            LinkRead::SupervisionTimeout,
                        ]
                        .into_iter()
                        .filter(|&r| !self.listed(r))
                        .collect();
                        if !skipped.is_empty() {
                            tracing::info!("link details: the controller does not list {skipped:?}, not read");
                        }
                    }
                    return false;
                }
                if !Self::ours(opcode) {
                    return false;
                }
                // Return parameters: status, handle, then the value
                let handle = le16(p, 6).map(|h| h & 0x0FFF);
                if status != ERROR_CODE_SUCCESS {
                    self.refused(opcode, status);
                }
                let ok = status == ERROR_CODE_SUCCESS;
                let Some(acl) = handle.and_then(|h| self.links.get_mut(&h)) else { return false };
                match opcode {
                    HCI_OPCODE_READ_FAILED_CONTACT_COUNTER => {
                        let v = le16(p, 8).filter(|_| ok);
                        let now = Instant::now();
                        if let Some(v) = v {
                            if let Some((prev, at)) = acl.fcc_last {
                                let rises = failed_contact_rises(prev, v);
                                acl.d.failed_contacts_stream += rises;
                                let secs = now.duration_since(at).as_secs_f64();
                                acl.d.failed_contacts_per_second = (secs > 0.0).then(|| rises as f64 / secs);
                            } else {
                                acl.d.failed_contacts_stream += u64::from(v);
                            }
                            tracing::trace!("link details: failed contacts {v}");
                        } else {
                            acl.d.failed_contacts_per_second = None;
                        }
                        acl.d.failed_contacts = v;
                        acl.fcc_last = v.map(|v| (v, now));
                    }
                    HCI_OPCODE_READ_LINK_QUALITY => {
                        acl.d.link_quality = u8_at(p, 8).filter(|_| ok);
                        tracing::trace!("link details: link quality {:?}", acl.d.link_quality);
                    }
                    HCI_OPCODE_READ_LINK_SUPERVISION_TIMEOUT => {
                        acl.d.supervision_timeout_slots = le16(p, 8).filter(|_| ok);
                        tracing::debug!(
                            "link details: supervision timeout {:?} slots",
                            acl.d.supervision_timeout_slots
                        );
                    }
                    _ => return false,
                }
                true
            }
            HCI_EVENT_COMMAND_STATUS => {
                // Read Remote Version answers with an event later; a refusal
                // comes as the status
                if let (Some(status), Some(opcode)) = (u8_at(p, 2), le16(p, 4))
                    && status != ERROR_CODE_SUCCESS
                    && Self::ours(opcode)
                {
                    self.refused(opcode, status);
                }
                false
            }
            HCI_EVENT_READ_REMOTE_VERSION_INFORMATION_COMPLETE => {
                let (Some(status), Some(handle), Some(lmp_version), Some(company_id), Some(lmp_subversion)) =
                    (u8_at(p, 2), le16(p, 3), u8_at(p, 5), le16(p, 6), le16(p, 8))
                else {
                    return false;
                };
                let Some(acl) = self.links.get_mut(&(handle & 0x0FFF)) else { return false };
                if status != ERROR_CODE_SUCCESS {
                    tracing::debug!("link details: remote version not read (0x{status:02X})");
                    return false;
                }
                acl.d.remote_version = Some(RemoteVersion { lmp_version, company_id, lmp_subversion });
                tracing::info!(
                    "remote controller of {}: company 0x{company_id:04X} ({}), LMP version {lmp_version}, subversion 0x{lmp_subversion:04X}",
                    addr_str(&acl.addr),
                    crate::chips::company_name(company_id).unwrap_or("unknown")
                );
                true
            }
            HCI_EVENT_READ_REMOTE_SUPPORTED_FEATURES_COMPLETE => {
                let (Some(status), Some(handle), Some(bytes)) = (u8_at(p, 2), le16(p, 3), p.get(5..13)) else {
                    return false;
                };
                let Some(acl) = self.links.get_mut(&(handle & 0x0FFF)) else { return false };
                if status != ERROR_CODE_SUCCESS {
                    return false;
                }
                let mut raw = [0u8; 8];
                raw.copy_from_slice(bytes);
                let f = RemoteFeatures(raw);
                acl.d.remote_features = Some(f);
                tracing::info!(
                    "remote features of {}: {raw:02X?} (EDR 2 Mbps {}, 3 Mbps {}, largest packet {})",
                    addr_str(&acl.addr),
                    f.edr_2m(),
                    f.edr_3m(),
                    best_packet(&f)
                );
                true
            }
            HCI_EVENT_ROLE_CHANGE => {
                let (Some(status), Some(addr), Some(role)) = (u8_at(p, 2), addr_at(p, 3), u8_at(p, 9)) else {
                    return false;
                };
                if status != ERROR_CODE_SUCCESS {
                    tracing::debug!("role change of {} failed (0x{status:02X})", addr_str(&addr));
                    return false;
                }
                let Some(acl) = self.links.values_mut().find(|a| a.addr == addr) else { return false };
                let central = role == 0;
                if acl.d.central != Some(central) {
                    acl.d.role_changes += 1;
                }
                acl.d.central = Some(central);
                tracing::info!(
                    "role change: {} with {}",
                    if central { "central" } else { "peripheral" },
                    addr_str(&addr)
                );
                true
            }
            HCI_EVENT_MODE_CHANGE => {
                let (Some(status), Some(handle), Some(mode), Some(interval)) =
                    (u8_at(p, 2), le16(p, 3), u8_at(p, 5), le16(p, 6))
                else {
                    return false;
                };
                if status != ERROR_CODE_SUCCESS {
                    return false;
                }
                let Some(acl) = self.links.get_mut(&(handle & 0x0FFF)) else { return false };
                let mode = LinkMode::from_hci(mode);
                if mode == Some(LinkMode::Sniff) && acl.d.mode != Some(LinkMode::Sniff) {
                    acl.d.sniff_entries += 1;
                }
                acl.d.mode = mode;
                acl.d.sniff_interval_slots = (mode == Some(LinkMode::Sniff)).then_some(interval);
                let line = format!("mode change of {}: {mode:?}, interval {interval} slots", addr_str(&acl.addr));
                if streaming {
                    tracing::info!("{line}");
                } else {
                    tracing::debug!("{line}");
                }
                true
            }
            HCI_EVENT_LINK_SUPERVISION_TIMEOUT_CHANGED => {
                let (Some(handle), Some(timeout)) = (le16(p, 2), le16(p, 4)) else { return false };
                let Some(acl) = self.links.get_mut(&(handle & 0x0FFF)) else { return false };
                acl.d.supervision_timeout_slots = Some(timeout);
                tracing::debug!("link details: supervision timeout changed to {timeout} slots");
                true
            }
            _ => false,
        }
    }

    /// Puts the details of `a2dp` (the A2DP link's handle) into the stats;
    /// nothing without it
    pub fn publish(&self, stats: &LinkStats, a2dp: Option<u16>) {
        let d = a2dp.and_then(|h| self.links.get(&h)).map(|a| a.d.clone()).unwrap_or_default();
        stats.set_link(d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_contacts_start_over_after_a_packet_gets_through() {
        assert_eq!(failed_contact_rises(0, 0), 0);
        assert_eq!(failed_contact_rises(2, 5), 3);
        // Reset to 0 by a packet that got through, then 1 more given up
        assert_eq!(failed_contact_rises(5, 1), 1);
    }

    #[test]
    fn largest_packet_by_features() {
        let f = |b0: u8, b3: u8, b4: u8, b5: u8| RemoteFeatures([b0, 0, 0, b3, b4, b5, 0, 0]);
        assert_eq!(best_packet(&f(0x03, 0x06, 0x80, 0x01)), "3-DH5");
        assert_eq!(best_packet(&f(0x03, 0x02, 0x80, 0x01)), "2-DH5");
        assert_eq!(best_packet(&f(0x03, 0x06, 0x80, 0x00)), "3-DH3");
        assert_eq!(best_packet(&f(0x03, 0x04, 0x00, 0x00)), "3-DH1");
        assert_eq!(best_packet(&f(0x03, 0x00, 0x00, 0x00)), "DH5");
        assert_eq!(best_packet(&f(0x01, 0x00, 0x00, 0x00)), "DH3");
        assert_eq!(best_packet(&f(0x00, 0x00, 0x00, 0x00)), "DH1");
    }

    fn tracker_with(handle: u16, addr: [u8; 6]) -> LinkTracker {
        let mut t = LinkTracker::default();
        let d = LinkDetails { central: Some(true), mode: Some(LinkMode::Active), ..Default::default() };
        t.links.insert(handle, Acl { addr, d, fcc_last: None });
        t
    }

    fn command_complete(opcode: u16, ret: &[u8]) -> Vec<u8> {
        let mut p = vec![HCI_EVENT_COMMAND_COMPLETE, (3 + ret.len()) as u8, 1];
        p.extend_from_slice(&opcode.to_le_bytes());
        p.extend_from_slice(ret);
        p
    }

    #[test]
    fn readings_and_refusals() {
        let mut t = tracker_with(0x0B, [1, 2, 3, 4, 5, 6]);
        let lq = command_complete(HCI_OPCODE_READ_LINK_QUALITY, &[0, 0x0B, 0, 200]);
        assert!(t.on_hci(HCI_EVENT_COMMAND_COMPLETE, &lq, true));
        assert_eq!(t.links[&0x0B].d.link_quality, Some(200));
        // The controller does not know the command: None, never asked again
        let fcc = command_complete(HCI_OPCODE_READ_FAILED_CONTACT_COUNTER, &[0x01, 0x0B, 0, 0, 0]);
        t.reads.push_back((LinkRead::FailedContacts, 0x0B));
        assert!(t.on_hci(HCI_EVENT_COMMAND_COMPLETE, &fcc, true));
        assert_eq!(t.links[&0x0B].d.failed_contacts, None);
        assert!(t.reads.is_empty());
        t.queue(LinkRead::FailedContacts, 0x0B);
        t.queue(LinkRead::LinkQuality, 0x0B);
        assert_eq!(t.reads, [(LinkRead::LinkQuality, 0x0B)]);
        // Another controller after HCI starts over
        t.reset();
        t.queue(LinkRead::FailedContacts, 0x0B);
        assert_eq!(t.reads.len(), 1);
    }

    #[test]
    fn commands_the_controller_does_not_list_are_not_sent() {
        let mut t = tracker_with(0x0B, [1; 6]);
        // Supported commands: Read Remote Version and Link Quality only
        let mut bitmap = [0u8; 64];
        bitmap[2] = 0x80;
        bitmap[15] = 0x10;
        let mut ret = vec![0];
        ret.extend_from_slice(&bitmap);
        let p = command_complete(HCI_OPCODE_READ_LOCAL_SUPPORTED_COMMANDS, &ret);
        assert!(!t.on_hci(HCI_EVENT_COMMAND_COMPLETE, &p, false));
        // Kept over the reset when HCI is up
        t.reset();
        for r in
            [LinkRead::FailedContacts, LinkRead::LinkQuality, LinkRead::RemoteVersion, LinkRead::SupervisionTimeout]
        {
            t.queue(r, 0x0B);
        }
        assert_eq!(t.reads, [(LinkRead::LinkQuality, 0x0B), (LinkRead::RemoteVersion, 0x0B)]);
    }

    #[test]
    fn failed_contacts_add_up_per_stream() {
        let mut t = tracker_with(0x0B, [1, 2, 3, 4, 5, 6]);
        let read = |t: &mut LinkTracker, v: u16| {
            let p = command_complete(HCI_OPCODE_READ_FAILED_CONTACT_COUNTER, &[0, 0x0B, 0, v as u8, (v >> 8) as u8]);
            t.on_hci(HCI_EVENT_COMMAND_COMPLETE, &p, true);
        };
        read(&mut t, 0);
        read(&mut t, 3);
        read(&mut t, 1);
        assert_eq!(t.links[&0x0B].d.failed_contacts, Some(1));
        assert_eq!(t.links[&0x0B].d.failed_contacts_stream, 4);
        t.stream_started(Some(0x0B));
        assert_eq!(t.links[&0x0B].d.failed_contacts_stream, 0);
    }

    #[test]
    fn remote_chip_role_and_mode() {
        let addr = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let mut t = tracker_with(0x0B, addr);
        // Status, handle, LMP version 12, Airoha, subversion 0x1234
        let v = [HCI_EVENT_READ_REMOTE_VERSION_INFORMATION_COMPLETE, 8, 0, 0x0B, 0, 12, 0x94, 0x00, 0x34, 0x12];
        assert!(t.on_hci(v[0], &v, false));
        let rv = t.links[&0x0B].d.remote_version.unwrap();
        assert_eq!((rv.lmp_version, rv.company_id, rv.lmp_subversion), (12, 0x0094, 0x1234));
        let f = [HCI_EVENT_READ_REMOTE_SUPPORTED_FEATURES_COMPLETE, 11, 0, 0x0B, 0, 0x03, 0, 0, 0x06, 0x80, 0x01, 0, 0];
        assert!(t.on_hci(f[0], &f, false));
        assert_eq!(best_packet(&t.links[&0x0B].d.remote_features.unwrap()), "3-DH5");
        // Role Change carries the address (little-endian on the wire)
        let mut r = vec![HCI_EVENT_ROLE_CHANGE, 8, 0];
        r.extend(addr.iter().rev());
        r.push(1);
        assert!(t.on_hci(r[0], &r, true));
        assert_eq!((t.links[&0x0B].d.central, t.links[&0x0B].d.role_changes), (Some(false), 1));
        // Sniff at 800 slots, then active
        let m = |mode: u8| [HCI_EVENT_MODE_CHANGE, 6, 0, 0x0B, 0, mode, 0x20, 0x03];
        assert!(t.on_hci(HCI_EVENT_MODE_CHANGE, &m(2), true));
        assert_eq!(t.links[&0x0B].d.sniff_interval_slots, Some(800));
        t.on_hci(HCI_EVENT_MODE_CHANGE, &m(0), true);
        t.on_hci(HCI_EVENT_MODE_CHANGE, &m(2), true);
        assert_eq!(t.links[&0x0B].d.sniff_entries, 2);
        assert_eq!(t.links[&0x0B].d.mode, Some(LinkMode::Sniff));
        let lsto = [HCI_EVENT_LINK_SUPERVISION_TIMEOUT_CHANGED, 4, 0x0B, 0, 0x80, 0x0C];
        assert!(t.on_hci(lsto[0], &lsto, true));
        assert_eq!(t.links[&0x0B].d.supervision_timeout_slots, Some(0x0C80));
        // Another link's events leave it alone
        let other = [HCI_EVENT_MODE_CHANGE, 6, 0, 0x0C, 0, 0, 0, 0];
        assert!(!t.on_hci(other[0], &other, true));
    }

    #[test]
    fn publishes_the_a2dp_link_only() {
        let t = &mut tracker_with(0x0B, [1; 6]);
        let stats = LinkStats::default();
        t.publish(&stats, Some(0x0B));
        assert_eq!(stats.link().central, Some(true));
        t.publish(&stats, None);
        assert_eq!(stats.link(), LinkDetails::default());
    }
}
